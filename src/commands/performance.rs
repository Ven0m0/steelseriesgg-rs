use std::collections::HashMap;
use std::time::{Duration, Instant};

use tokio::task::yield_now;

use steelseries_gg::devices::{DeviceInfo, DeviceManager, DeviceType};
use steelseries_gg::fs_utils::{secure_write, secure_write_async};
use steelseries_gg::rgb::{Color, Effect, RgbController, WaveDirection};
use steelseries_gg::{Error, Result};

use crate::cli::PerformanceAction;
use crate::deduped_devices;

pub(crate) async fn cmd_performance(manager: &DeviceManager, action: PerformanceAction) -> Result<()> {
    let keyboards = manager.keyboards();

    if keyboards.is_empty() {
        println!("⚠️  No SteelSeries keyboards found for performance monitoring");
        return Ok(());
    }

    match action {
        PerformanceAction::Stats { monitor, output, json } => {
            if let Some(interval_seconds) = monitor {
                println!("🔄 Monitoring RGB performance (press Ctrl+C to stop)...");
                loop {
                    display_performance_stats(manager, &keyboards, json)?;

                    if let Some(ref output_path) = output {
                        export_performance_stats(manager, &keyboards, output_path, json).await?;
                    }

                    tokio::time::sleep(Duration::from_secs(interval_seconds)).await;
                }
            } else {
                display_performance_stats(manager, &keyboards, json)?;

                if let Some(output_path) = output {
                    export_performance_stats(manager, &keyboards, &output_path, json).await?;
                    println!("📄 Performance stats exported to: {}", output_path);
                }
            }
        }

        PerformanceAction::Enable => {
            println!("🚀 Enabling performance optimizations...");
            let mut enabled_count = 0;

            for device_info in &keyboards {
                match manager.open_keyboard(device_info) {
                    Ok(mut keyboard) => {
                        keyboard.set_performance_optimization(true);
                        enabled_count += 1;
                        println!("   ✅ {} - Performance optimizations enabled", device_info.name);
                    }
                    Err(e) => {
                        eprintln!("   ❌ {} - Failed to open: {}", device_info.name, e);
                    }
                }
            }

            println!(
                "✅ Performance optimizations enabled on {}/{} keyboards",
                enabled_count,
                keyboards.len()
            );
        }

        PerformanceAction::Disable => {
            println!("⏸️  Disabling performance optimizations...");
            let mut disabled_count = 0;

            for device_info in &keyboards {
                match manager.open_keyboard(device_info) {
                    Ok(mut keyboard) => {
                        keyboard.set_performance_optimization(false);
                        disabled_count += 1;
                        println!("   ✅ {} - Performance optimizations disabled", device_info.name);
                    }
                    Err(e) => {
                        eprintln!("   ❌ {} - Failed to open: {}", device_info.name, e);
                    }
                }
            }

            println!(
                "✅ Performance optimizations disabled on {}/{} keyboards",
                disabled_count,
                keyboards.len()
            );
        }

        PerformanceAction::Cleanup => {
            println!("🧹 Cleaning up performance caches...");
            let mut cleaned_count = 0;

            for device_info in &keyboards {
                match manager.open_keyboard(device_info) {
                    Ok(mut keyboard) => {
                        keyboard.cleanup_rgb_caches();
                        cleaned_count += 1;
                        println!("   ✅ {} - Caches cleaned", device_info.name);
                    }
                    Err(e) => {
                        eprintln!("   ❌ {} - Failed to open: {}", device_info.name, e);
                    }
                }
            }

            println!("✅ Cleaned caches on {}/{} keyboards", cleaned_count, keyboards.len());
        }

        PerformanceAction::Benchmark { duration, output } => {
            println!("🏃 Running performance benchmark for {} seconds...", duration);

            let start_time = Instant::now();
            let mut benchmark_results = HashMap::new();

            // Run benchmark on each keyboard
            for &device_info in &keyboards {
                match manager.open_keyboard(device_info) {
                    Ok(mut keyboard) => {
                        println!("   🔬 Benchmarking: {}", device_info.name);
                        // Enable performance optimizations for benchmark
                        keyboard.set_performance_optimization(true);

                        let device_start = Instant::now();
                        let mut operation_count = 0;
                        let mut failed_count = 0;

                        let colors = [Color::RED, Color::GREEN, Color::BLUE, Color::WHITE, Color::BLACK];

                        // Run rapid RGB operations for benchmark duration
                        while device_start.elapsed().as_secs() < duration {
                            for color in colors {
                                match keyboard.set_color(color).await {
                                    Ok(()) => operation_count += 1,
                                    Err(_) => failed_count += 1,
                                }

                                // Small delay to prevent overwhelming the device
                                yield_now().await;
                            }
                        }

                        let ops_per_second = operation_count as f64 / duration as f64;
                        benchmark_results.insert(device_info.name.to_string(), ops_per_second);

                        println!("      Operations/second: {:.1}", ops_per_second);
                        if failed_count > 0 {
                            println!("      Failed operations (excluded): {}", failed_count);
                        }

                        // Display performance stats if available
                        if let Some(stats) = keyboard.get_rgb_performance_stats() {
                            println!("      Cache hit rate: {:.1}%", stats.cache_hit_rate * 100.0);
                            println!("      Avg computation: {:.2}μs", stats.avg_computation_time_us);
                            println!("      Current refresh rate: {:.1} Hz", stats.current_refresh_rate);
                        }
                    }
                    Err(e) => {
                        eprintln!("   ❌ {} - Failed to benchmark: {}", device_info.name, e);
                    }
                }
            }

            // Export benchmark results if requested
            if let Some(output_path) = output {
                let benchmark_data = serde_json::json!({
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "duration_seconds": duration,
                    "results": benchmark_results
                });

                secure_write(&output_path, serde_json::to_string_pretty(&benchmark_data)?)
                    .map_err(|e| Error::DeviceCommunication(format!("Failed to write benchmark: {}", e)))?;

                println!("📄 Benchmark results exported to: {}", output_path);
            }

            println!("✅ Benchmark completed in {:.1}s", start_time.elapsed().as_secs_f64());
        }
    }

    Ok(())
}

struct StatsMapSerializer<'a> {
    manager: &'a DeviceManager,
    keyboards: &'a [&'a DeviceInfo],
}

impl<'a> serde::Serialize for StatsMapSerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        for device_info in self.keyboards {
            if let Ok(keyboard) = self.manager.open_keyboard(device_info)
                && let Some(stats) = keyboard.get_rgb_performance_stats()
            {
                map.serialize_entry(&device_info.name, stats)?;
            }
        }
        map.end()
    }
}

fn display_performance_stats(manager: &DeviceManager, keyboards: &[&DeviceInfo], json: bool) -> Result<()> {
    if json {
        let stats_map = StatsMapSerializer { manager, keyboards };
        println!("{}", serde_json::to_string_pretty(&stats_map)?);
    } else {
        println!("📊 RGB Performance Statistics:");
        println!();

        for device_info in keyboards {
            match manager.open_keyboard(device_info) {
                Ok(keyboard) => {
                    println!("🎹 {}", device_info.name);

                    if let Some(stats) = keyboard.get_rgb_performance_stats() {
                        println!("   💾 Cache hit rate: {:.1}%", stats.cache_hit_rate * 100.0);
                        println!("   ⚡ Avg computation: {:.2}μs", stats.avg_computation_time_us);
                        println!("   🔄 Current refresh rate: {:.1} Hz", stats.current_refresh_rate);
                        println!("   📈 Total computations: {}", stats.total_computations);
                        println!("   🔀 Operations batched: {}", stats.hid_operations_batched);
                        println!("   💽 Allocations saved: {}", stats.allocations_saved);
                        println!(
                            "   📊 Memory utilization: {:.1}%",
                            stats.memory_pool_utilization * 100.0
                        );

                        if let Some(frame_time) = keyboard.get_optimal_frame_time() {
                            println!("   🕐 Optimal frame time: {:.1}ms", frame_time.as_millis());
                        }
                    } else {
                        println!("   ⚠️  Performance stats not available (optimizations disabled)");
                    }
                    println!();
                }
                Err(e) => {
                    println!("❌ {}: Failed to open - {}", device_info.name, e);
                    println!();
                }
            }
        }
    }

    Ok(())
}

async fn export_performance_stats(
    manager: &DeviceManager,
    keyboards: &[&DeviceInfo],
    output_path: &str,
    json: bool,
) -> Result<()> {
    if json {
        let stats_map = StatsMapSerializer { manager, keyboards };
        let export_data = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "devices": stats_map
        });

        secure_write_async(output_path.to_string(), serde_json::to_string_pretty(&export_data)?)
            .await
            .map_err(|e| Error::FileSystemError(format!("Failed to write stats: {}", e)))?;
    } else {
        let mut content = String::new();
        content.push_str("# RGB Performance Statistics Report\n\n");
        content.push_str(&format!("Generated: {}\n\n", chrono::Utc::now().to_rfc3339()));

        for device_info in keyboards {
            if let Ok(keyboard) = manager.open_keyboard(device_info) {
                content.push_str(&format!("## {}\n", device_info.name));
                content.push_str(&format!("- Product ID: 0x{:04x}\n", device_info.product_id));

                if let Some(stats) = keyboard.get_rgb_performance_stats() {
                    content.push_str(&format!("- Cache hit rate: {:.1}%\n", stats.cache_hit_rate * 100.0));
                    content.push_str(&format!(
                        "- Avg computation time: {:.2}μs\n",
                        stats.avg_computation_time_us
                    ));
                    content.push_str(&format!(
                        "- Current refresh rate: {:.1} Hz\n",
                        stats.current_refresh_rate
                    ));
                    content.push_str(&format!("- Total computations: {}\n", stats.total_computations));
                    content.push_str(&format!("- Operations batched: {}\n", stats.hid_operations_batched));
                    content.push_str(&format!("- Allocations saved: {}\n", stats.allocations_saved));
                    content.push_str(&format!(
                        "- Memory pool utilization: {:.1}%\n",
                        stats.memory_pool_utilization * 100.0
                    ));
                } else {
                    content.push_str("- Performance stats: Not available\n");
                }
                content.push('\n');
            }
        }

        secure_write_async(output_path.to_string(), content)
            .await
            .map_err(|e| Error::FileSystemError(format!("Failed to write stats: {}", e)))?;
    }

    Ok(())
}

/// Run automated device tests to verify responsiveness.
pub(crate) async fn cmd_test_device(
    manager: &DeviceManager,
    device: &str,
    benchmark: bool,
    verbose: bool,
) -> Result<()> {
    use std::io::{self, IsTerminal};
    use steelseries_gg::validation::{RgbValidator, print_test_results};

    // Create validator with benchmark mode if requested
    let mut validator = RgbValidator::new();
    if benchmark {
        validator = validator.with_benchmarks();
    }

    // Try to find and open the device by name or path
    let device_lower = device.to_lowercase();
    let device_info = deduped_devices(manager)
        .into_iter()
        .find(|d| d.path.contains(device) || d.name.to_lowercase().contains(&device_lower))
        .ok_or_else(|| {
            Error::Other(format!(
                "Device '{}' not found. Use 'ssgg devices' to list available devices.",
                device
            ))
        })?;

    println!("\nTesting device: {}\n", device_info.name);

    // Open device and run validation based on type
    let report = match device_info.device_type {
        DeviceType::Keyboard => {
            let mut keyboard = manager.open_keyboard(device_info)?;
            validator.validate_keyboard(&mut *keyboard).await
        }
        _ => {
            return Err(Error::Other(format!(
                "Device type {:?} not supported for testing yet. Only keyboards supported.",
                device_info.device_type
            )));
        }
    };

    // Display results with colored output if terminal supports it
    let use_colors = io::stdout().is_terminal();
    print_test_results(&report, verbose, use_colors);

    // Exit with appropriate code based on health
    if report.is_healthy() {
        std::process::exit(0);
    } else {
        std::process::exit(1);
    }
}

// Format metrics with color coding using Display structs to avoid intermediate allocations
struct FpsDisplay(f32, f32);
impl std::fmt::Display for FpsDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.1}/{:.1}", self.0, self.1)
    }
}

struct FrameTimeDisplay(f32);
impl std::fmt::Display for FrameTimeDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.2}ms", self.0)
    }
}

struct Colored<'a, T: std::fmt::Display> {
    val: &'a T,
    color_code: &'static str,
}
impl<'a, T: std::fmt::Display> std::fmt::Display for Colored<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\x1b[{}m{}\x1b[0m", self.color_code, self.val)
    }
}

enum MaybeColored<'a, T: std::fmt::Display> {
    Colored(Colored<'a, T>),
    Plain(&'a T),
}
impl<'a, T: std::fmt::Display> std::fmt::Display for MaybeColored<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Colored(c) => c.fmt(f),
            Self::Plain(p) => p.fmt(f),
        }
    }
}

/// Verify RGB performance metrics over time.
pub(crate) async fn cmd_verify_performance(
    manager: &DeviceManager,
    duration: u64,
    effect_name: &str,
    output: Option<String>,
) -> Result<()> {
    use std::io::{self, IsTerminal, Write};
    use steelseries_gg::performance::PerformanceMonitor;
    use tokio::time::{Duration, interval};

    // Find first keyboard device
    let device_info = manager
        .keyboards()
        .first()
        .map(|&d| d.clone())
        .ok_or_else(|| Error::Other("No keyboard device found. Connect a keyboard and try again.".to_string()))?;

    println!("\nMonitoring RGB performance for: {}", device_info.name);
    println!("Duration: {}s | Effect: {}\n", duration, effect_name);

    // Parse effect name to Effect enum
    let effect = match effect_name.to_lowercase().as_str() {
        "breathing" => Effect::Breathing {
            color: Color::CYAN,
            speed: 1.0,
        },
        "spectrum" => Effect::Spectrum { speed: 0.5 },
        "wave" => Effect::Wave {
            colors: vec![Color::RED, Color::GREEN, Color::BLUE],
            speed: 1.0,
            direction: WaveDirection::LeftToRight,
        },
        "static" => Effect::Static { color: Color::RED },
        _ => {
            return Err(Error::Other(format!(
                "Unknown effect '{}'. Valid: breathing, spectrum, wave, static",
                effect_name
            )));
        }
    };

    // Open keyboard and create RGB controller
    let mut keyboard = manager.open_keyboard(&device_info)?;
    let zone_count = keyboard.zone_count();
    let mut controller = RgbController::new(zone_count);
    controller.set_effect(effect.clone());
    controller.set_brightness(0.8);

    // Create performance monitor
    let mut monitor = PerformanceMonitor::new();

    // Determine target FPS for effect

    let complexity = steelseries_gg::performance::calculate_effect_complexity(&effect);
    monitor.set_effect_complexity(complexity);

    let use_colors = io::stdout().is_terminal();
    let start_time = Instant::now();
    let mut frame_interval = interval(Duration::from_millis(16)); // ~60 FPS
    let mut print_interval = interval(Duration::from_secs(1));

    println!("Starting performance monitoring...\n");

    loop {
        tokio::select! {
            _ = frame_interval.tick() => {
                let frame_start = Instant::now();

                // Compute colors
                let compute_start = Instant::now();
                let colors = controller.compute_colors();
                let compute_time = compute_start.elapsed();

                // Send to device - use single color for all zones
                keyboard.set_color(colors[0]).await?;
                keyboard.apply().await?;

                // Record timing
                let frame_duration = frame_start.elapsed();
                monitor.record_frame_timing(frame_duration, compute_time);
            }

            _ = print_interval.tick() => {
                let metrics = monitor.metrics();
                let elapsed = start_time.elapsed().as_secs();

                // Format metrics with color coding using Display structs to avoid intermediate allocations
                let fps_display = FpsDisplay(metrics.actual_fps, metrics.target_fps);
                let fps_colored = if use_colors {
                    let fps_ratio = metrics.actual_fps / metrics.target_fps;
                    let code = if fps_ratio >= 0.8 { "32" } else if fps_ratio >= 0.6 { "33" } else { "31" };
                    MaybeColored::Colored(Colored { val: &fps_display, color_code: code })
                } else {
                    MaybeColored::Plain(&fps_display)
                };

                let frame_time_display = FrameTimeDisplay(metrics.frame_time);
                let frame_time_colored = if use_colors {
                    let code = if metrics.frame_time <= 20.0 { "32" } else { "33" };
                    MaybeColored::Colored(Colored { val: &frame_time_display, color_code: code })
                } else {
                    MaybeColored::Plain(&frame_time_display)
                };

                let dropped_colored = if use_colors && metrics.dropped_frames > 0 {
                    let drop_rate = metrics.dropped_frames as f32 / metrics.total_frames as f32;
                    let code = if drop_rate > 0.05 { "31" } else { "33" };
                    MaybeColored::Colored(Colored { val: &metrics.dropped_frames, color_code: code })
                } else {
                    MaybeColored::Plain(&metrics.dropped_frames)
                };

                print!(
                    "\r[{:02}s] FPS: {} | Frame: {} | Cache: {:.1}% | Dropped: {}     ",
                    elapsed,
                    fps_colored,
                    frame_time_colored,
                    metrics.cache_hit_rate * 100.0,
                    dropped_colored
                );
                io::stdout().flush().ok();

                // Check if duration reached
                if elapsed >= duration {
                    break;
                }
            }
        }
    }

    println!("\n\nPerformance Monitoring Complete\n");
    println!("================================================================================");

    let metrics = monitor.metrics();

    // Print final summary
    println!("Summary:");
    println!("  Total frames:     {}", metrics.total_frames);
    println!(
        "  Average FPS:      {:.1} (target: {:.1})",
        metrics.actual_fps, metrics.target_fps
    );
    println!("  Average frame time: {:.2}ms", metrics.frame_time);
    println!("  Cache hit rate:   {:.1}%", metrics.cache_hit_rate * 100.0);
    println!(
        "  Dropped frames:   {} ({:.2}%)",
        metrics.dropped_frames,
        (metrics.dropped_frames as f32 / metrics.total_frames as f32) * 100.0
    );

    // Export to JSON if requested
    if let Some(output_path) = output {
        let json = serde_json::to_string_pretty(&metrics)
            .map_err(|e| Error::Other(format!("Failed to serialize metrics: {}", e)))?;
        secure_write(&output_path, json)
            .map_err(|e| Error::Other(format!("Failed to write {}: {}", output_path, e)))?;
        println!("\nMetrics exported to: {}", output_path);
    }

    Ok(())
}
