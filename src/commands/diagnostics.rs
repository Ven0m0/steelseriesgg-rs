use std::io::IsTerminal;
use std::time::{Duration, Instant};

use colored::Colorize;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tabled::{Table, Tabled};
use tracing::info;

use steelseries_gg::devices::{
    DeviceInfo, DeviceManager, DeviceType,
    diagnostics::{init_global_diagnostics, with_global_diagnostics},
};
use steelseries_gg::fs_utils::secure_write_async;
use steelseries_gg::validation::RgbValidator;
use steelseries_gg::{Error, Result};

use crate::{deduped_devices, new_device_manager};

/// Generate a comprehensive bug report with diagnostic information.
pub(crate) async fn cmd_bug_report(output: &str, include_hid_logs: bool, include_performance: bool) -> Result<()> {
    use steelseries_gg::diagnostics_export::{collect_bug_report, export_bug_report};

    println!("Collecting diagnostic information...");
    println!("  - System information (OS, kernel, memory, CPU)");
    println!("  - Device states (connected devices, current settings)");

    if include_hid_logs {
        println!("  - HID communication logs");
    }

    if include_performance {
        println!("  - Performance metrics snapshot: NOT AVAILABLE yet (needs the daemon); will be omitted");
    }

    println!();

    // Collect bug report (async, uses spawn_blocking internally for sysinfo)
    let report = collect_bug_report(include_hid_logs, include_performance).await?;

    // Export to file (async file I/O)
    export_bug_report(&report, output).await?;

    // Print privacy warning
    use colored::Colorize;
    println!();
    println!("{}", "WARNING:".yellow().bold());
    println!("  This report may contain:");
    println!("    - Device serial numbers and identifiers");
    println!("    - System paths and usernames");
    println!("    - Performance data and timing information");
    println!();
    println!("  Please review the file before sharing publicly.");

    Ok(())
}

pub(crate) async fn cmd_validate(
    manager: &DeviceManager,
    benchmark: bool,
    timeout: u64,
    output: Option<String>,
    json: bool,
) -> Result<()> {
    info!("Starting RGB system validation...");

    let timeout_duration = Duration::from_secs(timeout);
    let mut validator = RgbValidator::new().with_timeout(timeout_duration);

    if benchmark {
        validator = validator.with_benchmarks();
        info!("Performance benchmarks enabled");
    }

    let keyboards = manager.keyboards();

    if keyboards.is_empty() {
        println!("⚠️  No SteelSeries keyboards found for validation");
        return Ok(());
    }

    println!("🔍 Found {} keyboard(s) for validation", keyboards.len());
    let mut all_reports = Vec::new();

    for device_info in keyboards {
        println!(
            "\n📋 Validating: {} (PID: 0x{:04x})",
            device_info.name, device_info.product_id
        );

        match manager.open_keyboard(device_info) {
            Ok(mut keyboard) => {
                let report = validator.validate_keyboard(&mut *keyboard).await;

                // Display summary
                let status = if report.is_healthy() {
                    "✅ HEALTHY"
                } else {
                    "❌ ISSUES DETECTED"
                };
                println!("   Status: {}", status);
                println!("   Health Score: {:.1}%", report.health_score);
                println!(
                    "   Tests: {}/{} passed",
                    report.results.iter().filter(|r| r.passed).count(),
                    report.results.len()
                );

                if report.capabilities.per_key_rgb {
                    println!("   🎹 Per-key RGB: Supported ({} keys)", report.capabilities.key_count);
                } else {
                    println!("   🌈 Zone RGB: {} zones", report.capabilities.zone_count);
                }

                if benchmark {
                    println!(
                        "   🚀 Performance: {:.1}ms avg, {:.0} fps effective",
                        report.performance.avg_effect_compute_ms + report.performance.avg_hid_communication_ms,
                        report.performance.effective_refresh_rate
                    );
                }

                // Show failed tests if any
                let failed_tests = report.failed_tests();
                if !failed_tests.is_empty() {
                    println!("   ❌ Failed tests:");
                    for test in failed_tests {
                        println!(
                            "      - {}: {}",
                            test.name,
                            test.error.as_ref().unwrap_or(&"Unknown error".to_string())
                        );
                    }
                }

                all_reports.push(report);
            }
            Err(e) => {
                eprintln!("❌ Failed to open {}: {}", device_info.name, e);
            }
        }
    }

    // Export report if requested
    if let Some(output_path) = output {
        info!("Exporting validation report to: {}", output_path);

        let export_content = if json {
            // Export as JSON
            serde_json::to_string_pretty(&all_reports)
                .map_err(|e| Error::DeviceCommunication(format!("JSON serialization failed: {}", e)))?
        } else {
            // Export as human-readable text
            let mut content = String::new();
            content.push_str("# SteelSeries RGB Validation Report\n\n");
            content.push_str(&format!("Generated: {}\n", chrono::Utc::now().to_rfc3339()));
            content.push_str(&format!("Total devices: {}\n\n", all_reports.len()));

            for (i, report) in all_reports.iter().enumerate() {
                content.push_str(&format!("## Device {} - {}\n", i + 1, report.device_info.name));
                content.push_str(&format!("- Product ID: 0x{:04x}\n", report.device_info.product_id));
                content.push_str(&format!("- Health Score: {:.1}%\n", report.health_score));
                content.push_str(&format!(
                    "- Status: {}\n",
                    if report.is_healthy() {
                        "Healthy"
                    } else {
                        "Issues Detected"
                    }
                ));

                content.push_str("\n### Test Results\n");
                for result in &report.results {
                    let status = if result.passed { "✅" } else { "❌" };
                    content.push_str(&format!("- {} {} ({:.0}ms)\n", status, result.name, result.duration_ms));

                    if let Some(error) = &result.error {
                        content.push_str(&format!("  Error: {}\n", error));
                    }

                    for note in &result.notes {
                        content.push_str(&format!("  Note: {}\n", note));
                    }
                }
                content.push('\n');
            }
            content
        };

        secure_write_async(output_path.clone(), export_content)
            .await
            .map_err(|e| Error::FileSystemError(format!("Failed to write report: {}", e)))?;

        println!("\n📄 Validation report exported to: {}", output_path);
    }

    // Overall summary
    let total_healthy = all_reports.iter().filter(|r| r.is_healthy()).count();
    println!("\n📊 Validation Summary:");
    println!("   Total devices: {}", all_reports.len());
    println!("   Healthy devices: {}/{}", total_healthy, all_reports.len());

    if all_reports.is_empty() {
        println!("   ⚠️  No devices could be opened for validation - check errors above");
    } else if total_healthy == all_reports.len() {
        println!("   🎉 All devices passed validation!");
    } else {
        println!("   ⚠️  Some devices have issues - check details above");
    }

    Ok(())
}

/// Show device status with optional live monitoring.
pub(crate) async fn cmd_status(_initial_manager: &DeviceManager, device_filter: &str, refresh_ms: u64) -> Result<()> {
    #[derive(Tabled)]
    struct DeviceRow {
        #[tabled(rename = "Name")]
        name: String,
        #[tabled(rename = "Type")]
        device_type: String,
        #[tabled(rename = "VID:PID")]
        vid_pid: String,
        #[tabled(rename = "Path")]
        path: String,
        #[tabled(rename = "Status")]
        status: String,
    }

    // Check if stdout is a TTY
    let is_tty = std::io::stdout().is_terminal();

    // Get initial device list (create new manager for mutations)
    let mut manager = new_device_manager()?;
    manager.refresh()?;

    // Filter devices by type
    let filter_type = match device_filter.to_lowercase().as_str() {
        "keyboard" => Some(DeviceType::Keyboard),
        "headset" => Some(DeviceType::Headset),
        "mouse" => Some(DeviceType::Mouse),
        "all" => None,
        _ => {
            return Err(Error::Other(format!(
                "Invalid device filter: {}. Use 'keyboard', 'headset', 'mouse', or 'all'",
                device_filter
            )));
        }
    };

    if is_tty {
        // TTY mode: Real-time updates with progress bars
        let multi_progress = MultiProgress::new();
        let spinner_style = ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] {prefix}: {msg}")
            .map_err(|e| Error::Other(format!("Template error: {}", e)))?;

        // Create progress bars for each device
        let mut progress_bars: Vec<(DeviceInfo, ProgressBar)> = Vec::new();

        for device_info in deduped_devices(&manager) {
            if let Some(filter) = filter_type
                && device_info.device_type != filter
            {
                continue;
            }

            let pb = multi_progress.add(ProgressBar::new_spinner());
            pb.set_style(spinner_style.clone());
            pb.set_prefix(device_info.name.to_string());

            let device_type_str = match device_info.device_type {
                DeviceType::Keyboard => "Keyboard",
                DeviceType::Headset => "Headset",
                DeviceType::Mouse => "Mouse",
                DeviceType::Unknown => "Unknown",
            };

            pb.set_message(format!("Status: Connected | Type: {}", device_type_str));
            progress_bars.push((device_info.clone(), pb));
        }

        if progress_bars.is_empty() {
            println!("No devices found matching filter: {}", device_filter);
            return Ok(());
        }

        println!("Monitoring {} device(s) - Press Ctrl+C to stop", progress_bars.len());

        // Create refresh interval
        let mut interval = tokio::time::interval(Duration::from_millis(refresh_ms));

        // Monitor until Ctrl+C
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // Refresh device manager
                    manager.refresh()?;

                    // Update progress bars
                    for (device_info, pb) in &progress_bars {
                        let device_type_str = match device_info.device_type {
                            DeviceType::Keyboard => "Keyboard",
                            DeviceType::Headset => "Headset",
                            DeviceType::Mouse => "Mouse",
                            DeviceType::Unknown => "Unknown",
                        };

                        // Check if device still exists
                        let still_connected = manager
                            .devices()
                            .iter()
                            .any(|d| d.path == device_info.path);

                        if still_connected {
                            pb.set_message(format!(
                                "Status: {} | Type: {}",
                                "Connected".green(),
                                device_type_str
                            ));
                        } else {
                            pb.set_message(format!(
                                "Status: {} | Type: {}",
                                "Disconnected".red(),
                                device_type_str
                            ));
                        }
                        pb.tick();
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    println!("
            Stopping device monitoring...");
                    break;
                }
            }
        }
    } else {
        // Non-TTY mode: Single table output
        let mut rows: Vec<DeviceRow> = Vec::new();

        for device_info in deduped_devices(&manager) {
            if let Some(filter) = filter_type
                && device_info.device_type != filter
            {
                continue;
            }

            let device_type_str = match device_info.device_type {
                DeviceType::Keyboard => "Keyboard",
                DeviceType::Headset => "Headset",
                DeviceType::Mouse => "Mouse",
                DeviceType::Unknown => "Unknown",
            };

            rows.push(DeviceRow {
                name: device_info.name.to_string(),
                device_type: device_type_str.to_string(),
                vid_pid: format!("{:04X}:{:04X}", device_info.vendor_id, device_info.product_id),
                path: device_info.path.clone(),
                status: "Connected".to_string(),
            });
        }

        if rows.is_empty() {
            println!("No devices found matching filter: {}", device_filter);
        } else {
            let table = Table::new(rows).to_string();
            println!("{}", table);
        }
    }

    Ok(())
}

/// View or manage HID communication logs.
pub(crate) async fn cmd_hid_logs(file_logging: bool, device_filter: Option<&str>) -> Result<()> {
    // Initialize diagnostics with recording enabled
    init_global_diagnostics(true)?;

    // Enable file logging if requested
    if file_logging {
        with_global_diagnostics(|diag| diag.enable_file_logging())
            .ok_or_else(|| Error::Other("Failed to access diagnostics".to_string()))??;

        info!("HID logging enabled - output will be written to timestamped file");
    }

    // Parse device filter
    let _filter_type = if let Some(filter) = device_filter {
        match filter.to_lowercase().as_str() {
            "keyboard" => Some(DeviceType::Keyboard),
            "headset" => Some(DeviceType::Headset),
            "all" => None,
            _ => {
                return Err(Error::Other(format!(
                    "Invalid device filter: {}. Use 'keyboard', 'headset', or 'all'",
                    filter
                )));
            }
        }
    } else {
        None
    };

    println!("HID Communication Log Viewer");
    println!("============================");
    if let Some(filter) = device_filter {
        println!("Filter: {}", filter);
    }
    println!("Press Ctrl+C to stop and view summary\n");

    // Create interval for periodic summary display
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    let start_time = Instant::now();

    loop {
        tokio::select! {
            _ = interval.tick() => {
                // Get diagnostic summary
                if let Some(summary) = with_global_diagnostics(|diag| {
                    let analysis = diag.analyze_timing_patterns();
                    format!(
                        "Ops: {} | Failed: {} | Avg Send: {:.2}ms | Avg Recv: {:.2}ms | Elapsed: {:.1}s",
                        analysis.total_operations,
                        analysis.failed_operations,
                        analysis.avg_send_time.as_secs_f64() * 1000.0,
                        analysis.avg_receive_time.as_secs_f64() * 1000.0,
                        start_time.elapsed().as_secs_f64()
                    )
                }) {
                    print!("{}", summary);
                    std::io::Write::flush(&mut std::io::stdout())
                        .map_err(Error::Io)?;
                }
            }
            _ = tokio::signal::ctrl_c() => {
                println!("

Final Summary:");
                println!("==============");

                if let Some(summary) = with_global_diagnostics(|diag| diag.get_summary()) {
                    println!("{}", summary);
                } else {
                    println!("No diagnostic data collected.");
                }

                break;
            }
        }
    }

    Ok(())
}
