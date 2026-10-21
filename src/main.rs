//! SteelSeries GG for Linux - CLI
//!
//! A complete open-source replacement for SteelSeries GG on Linux.

mod cli;
mod daemon;

use clap::Parser;

use crate::cli::{
    ActuationAction, Cli, Commands, PerKeyAction, PerformanceAction, PollrateAction, ProfileAction, RgbAction,
    parse_key_name, require_color,
};
use crate::daemon::{cmd_daemon, cmd_server};
use tokio::task::yield_now;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

use steelseries_gg::config::Config;
use steelseries_gg::device_state::{DeviceId, DeviceStateStore, KeyboardState};
use steelseries_gg::devices::keyboards::Keyboard;
use steelseries_gg::devices::keyboards::onboard::OnboardProfileCodec;
use steelseries_gg::devices::{
    DeviceInfo, DeviceManager, DeviceType, KeyAddress, KeyId,
    diagnostics::{init_global_diagnostics, with_global_diagnostics},
    discovery::print_device_summary,
};
use steelseries_gg::fs_utils::{secure_write, secure_write_async};
use steelseries_gg::profiles::{KeyboardProfile, Profile, ProfileManager};
use steelseries_gg::rgb::{Color, Effect, RgbController, WaveDirection};
use steelseries_gg::validation::RgbValidator;
use steelseries_gg::{Error, Result};

use std::collections::HashMap;
use std::io::IsTerminal;
use std::time::{Duration, Instant};

use colored::Colorize;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tabled::{Table, Tabled};

#[cfg(feature = "audio")]
use crate::cli::{AudioAction, parse_channel};

#[cfg(any(feature = "audio", feature = "sonar"))]
use crate::cli::normalize_volume;

#[cfg(feature = "sonar")]
use crate::cli::{SonarAction, StreamerAction, parse_sonar_channel};

use crate::cli::OnboardAction;

#[cfg(feature = "audio")]
use steelseries_gg::audio::AudioMixer;

#[cfg(feature = "sonar")]
use steelseries_gg::audio::SonarClient;

/// Generate a comprehensive bug report with diagnostic information.
async fn cmd_bug_report(output: &str, include_hid_logs: bool, include_performance: bool) -> Result<()> {
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Setup logging
    let level = if cli.debug { Level::DEBUG } else { Level::INFO };
    let subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .with_target(false)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Initialize HID diagnostics if requested
    if cli.debug_hid {
        init_global_diagnostics(true)?;
        info!("HID diagnostics enabled - logging to timestamped file");
    } else {
        init_global_diagnostics(false)?;
    }

    match cli.command {
        Commands::Devices => {
            let manager = new_device_manager()?;
            cmd_devices(&manager)?;
        }

        Commands::Rgb { action } => {
            let manager = new_device_manager()?;
            cmd_rgb(&manager, action).await?;
        }

        Commands::Actuation { action } => {
            let manager = new_device_manager()?;
            cmd_actuation(&manager, action).await?;
        }

        Commands::Profile { action } => {
            cmd_profile(action).await?;
        }

        #[cfg(feature = "audio")]
        Commands::Audio { action } => {
            cmd_audio(action)?;
        }

        #[cfg(feature = "sonar")]
        Commands::Sonar { action } => {
            cmd_sonar(action).await?;
        }

        Commands::Pollrate { action } => {
            cmd_pollrate(action).await?;
        }

        Commands::Onboard { action } => {
            // The subcommands that run on a keyboard get the device manager, which is how they get the
            // keyboard: the selection runs before the command, so a write reaches the on-board selection
            // messages instead of the no-keyboard guard below it.
            if action.needs_keyboard() {
                let mut manager = new_device_manager()?;
                cmd_onboard(Some(&mut manager), action).await?;
            } else {
                cmd_onboard(None, action).await?;
            }
        }

        Commands::Server { port } => {
            cmd_server(port).await?;
        }

        Commands::Validate {
            benchmark,
            timeout,
            output,
            json,
        } => {
            let manager = new_device_manager()?;
            cmd_validate(&manager, benchmark, timeout, output, json).await?;
        }

        Commands::Performance { action } => {
            let manager = new_device_manager()?;
            cmd_performance(&manager, action).await?;
        }

        Commands::BugReport {
            output,
            include_hid_logs,
            include_performance,
        } => {
            cmd_bug_report(&output, include_hid_logs, include_performance).await?;
        }

        Commands::Status { device, refresh } => {
            let manager = new_device_manager()?;
            cmd_status(&manager, &device, refresh).await?;
        }

        Commands::HidLogs { file, device } => {
            cmd_hid_logs(file, device.as_deref()).await?;
        }

        Commands::Daemon => {
            let manager = new_device_manager()?;
            cmd_daemon(manager).await?;
        }

        Commands::TestDevice {
            device,
            benchmark,
            verbose,
        } => {
            let manager = new_device_manager()?;
            cmd_test_device(&manager, &device, benchmark, verbose).await?;
        }

        Commands::VerifyPerformance {
            duration,
            effect,
            output,
        } => {
            let manager = new_device_manager()?;
            cmd_verify_performance(&manager, duration, &effect, output).await?;
        }

        Commands::Fuzz { start, end, delay } => {
            use steelseries_gg::devices::fuzz::{FuzzParams, PayloadPattern, fuzz_keyboard_protocol};

            let manager = new_device_manager()?;
            let params = FuzzParams {
                start_cmd: start,
                end_cmd: end,
                delay_ms: delay,
                payload_pattern: PayloadPattern::Zeros, // Default to zeros for now
            };

            fuzz_keyboard_protocol(&manager, params)?;
        }
    }

    // Display HID diagnostic summary if enabled
    if let Some(summary) = with_global_diagnostics(|diag| diag.get_summary()) {
        info!("HID Diagnostic Summary:\n{}", summary);
    }

    Ok(())
}

/// Create a `DeviceManager`, applying the `[device]` control-endpoint override from
/// `config.toml` if one is set. A missing or unparseable config file falls back to
/// auto-detection rather than failing the command, matching prior behavior where device
/// commands never depended on config.toml at all.
fn new_device_manager() -> Result<DeviceManager> {
    let mut manager = DeviceManager::new()?;
    if let Ok(config) = Config::load() {
        manager.set_device_override(config.device);
    }
    Ok(manager)
}

/// All connected devices, deduplicated to one representative per physical device.
///
/// `DeviceManager::devices()` returns every raw HID collection hidapi enumerates — on Windows
/// that is several entries per physical keyboard/headset (see `DeviceManager::devices_by_type`).
/// CLI listings should show one row per physical device, so this unions the deduped lists
/// instead of using `manager.devices()` directly.
pub(crate) fn deduped_devices(manager: &DeviceManager) -> Vec<&DeviceInfo> {
    let mut all = manager.devices_by_type(DeviceType::Keyboard);
    all.extend(manager.devices_by_type(DeviceType::Headset));
    all.extend(manager.devices_by_type(DeviceType::Unknown));
    all
}

fn cmd_devices(manager: &DeviceManager) -> Result<()> {
    print_device_summary(manager);
    Ok(())
}

/// SteelSeries GG's engine rewrites the whole keyboard about 19 times a second, so any one-shot
/// colour sent while it runs is replaced within a frame or two.
#[cfg(windows)]
fn warn_if_gg_engine_running() {
    use sysinfo::{ProcessesToUpdate, System};

    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    if sys
        .processes_by_name(std::ffi::OsStr::new("SteelSeriesEngine.exe"))
        .next()
        .is_some()
    {
        eprintln!(
            "{}",
            "Warning: SteelSeries GG is running and keeps overwriting the keyboard lighting. \
             Close GG (including its tray icon) for this colour to stay."
                .yellow()
        );
    }
}

#[cfg(not(windows))]
fn warn_if_gg_engine_running() {}

async fn cmd_rgb(manager: &DeviceManager, action: RgbAction) -> Result<()> {
    // Find the first keyboard
    let keyboard_info = manager
        .first_device_of_type(DeviceType::Keyboard)
        .ok_or_else(|| Error::Other("No keyboard found".to_string()))?;

    println!("Using keyboard: {}", keyboard_info.name);
    warn_if_gg_engine_running();

    // Open device state store for persistence
    let state_store = DeviceStateStore::new_async().await?;
    let device_id = DeviceId::from(keyboard_info);

    // Open the keyboard using the abstraction layer
    let mut keyboard = manager.open_keyboard(keyboard_info)?;

    // Initialize the device
    keyboard.initialize()?;

    match action {
        RgbAction::Color { color } => {
            let color = require_color(&color)?;

            println!("Setting color to {}", color);
            keyboard.set_color(color).await?;
            keyboard.apply().await?; // Apply the color change

            // Persist the effect to state store
            state_store.update_keyboard_effect(device_id, Effect::Static { color })?;
            println!("Done!");
            println!(
                "Note: LEDs should now display {} color. Device accepted the command.",
                color
            );
        }

        RgbAction::Brightness { level } => {
            let level = level.min(100);
            println!("Setting brightness to {}%", level);
            keyboard.set_brightness(level).await?;

            // Persist brightness to state store
            state_store.update_keyboard_brightness(device_id, level)?;
            println!("Done!");
        }

        RgbAction::Effect { name, speed } => {
            let name_lower = name.to_ascii_lowercase();
            let effect = match name_lower.as_str() {
                "breathing" => Effect::Breathing {
                    color: Color::PURPLE,
                    speed,
                },
                "spectrum" => Effect::Spectrum { speed },
                "wave" => Effect::Wave {
                    colors: Color::DEFAULT_COLORS.to_vec(),
                    speed,
                    direction: WaveDirection::LeftToRight,
                },
                "off" => Effect::Off,
                _ => Effect::Static { color: Color::WHITE },
            };

            println!("Setting effect: {:?}", effect);
            // Persist effect to state store
            state_store.update_keyboard_effect(device_id, effect)?;
            // Note: Full effect implementation would require a background loop
            println!("(Note: Animated effects require running as daemon)");
        }

        RgbAction::Off => {
            println!("Turning off LEDs");
            keyboard.set_color(Color::BLACK).await?;
            keyboard.apply().await?; // Apply the off state

            // Persist the off state
            state_store.update_keyboard_effect(device_id, Effect::Off)?;
            println!("Done!");
            println!("Note: LEDs should now be off (black/dark). Device accepted the command.");
        }

        RgbAction::Perkey { action } => {
            cmd_per_key_rgb(&mut keyboard, action).await?;
        }
    }

    Ok(())
}

async fn cmd_actuation(manager: &DeviceManager, action: ActuationAction) -> Result<()> {
    // Find the first keyboard
    let keyboard_info = manager
        .first_device_of_type(DeviceType::Keyboard)
        .ok_or_else(|| Error::Other("No keyboard found".to_string()))?;

    println!("Using keyboard: {}", keyboard_info.name);

    // Open the keyboard using the abstraction layer
    let mut keyboard = manager.open_keyboard(keyboard_info)?;

    // Initialize the device
    keyboard.initialize()?;

    match action {
        ActuationAction::Set { mm } => {
            println!("Setting actuation point to {:.1}mm", mm);

            // Validate range
            if !(0.1..=4.0).contains(&mm) {
                return Err(Error::Other(
                    "Actuation point must be between 0.1mm and 4.0mm".to_string(),
                ));
            }

            keyboard.set_actuation_point_mm(mm)?;
            keyboard.apply().await?;
            println!("Actuation point set successfully!");
        }

        ActuationAction::SetValue { value } => {
            println!("Setting actuation value to {}", value);

            // Validate range
            if !(1..=40).contains(&value) {
                return Err(Error::InvalidConfig(
                    "Actuation point value must be between 1 and 40".to_string(),
                ));
            }

            keyboard.set_actuation_point(value)?;
            keyboard.apply().await?;
            println!("Actuation value set successfully!");
        }
    }

    Ok(())
}

async fn cmd_per_key_rgb(keyboard: &mut Box<dyn Keyboard>, action: PerKeyAction) -> Result<()> {
    match action {
        PerKeyAction::SetKey { key, color } => {
            // Parse the key name to KeyId
            let key_id = parse_key_name(&key).ok_or_else(|| Error::Other(format!("Unknown key: {}", key)))?;

            // Parse the color
            let color = require_color(&color)?;

            // Check if per-key RGB is supported
            if !keyboard.supports_per_key_rgb() {
                println!("Warning: Per-key RGB not supported on this keyboard");
                println!("Available key mapping: None");
                println!("Falling back to zone-based RGB (setting entire keyboard)");
                keyboard.set_color(color).await?;
                keyboard.apply().await?;
                return Ok(());
            }

            println!("Setting key '{}' to color {}", key, color);
            keyboard.set_key_color(key_id, color).await?;
            keyboard.apply().await?;
            println!("Done!");
        }

        PerKeyAction::SetKeys { keys } => {
            // Parse key-color pairs: "A:red,S:green,D:blue"
            let mut key_colors = Vec::new();
            for pair in keys.split(',') {
                let parts: Vec<&str> = pair.split(':').collect();
                if parts.len() != 2 {
                    return Err(Error::Other(format!("Invalid key:color pair: {}", pair)));
                }

                let key_name = parts[0].trim();
                let color_name = parts[1].trim();

                let key_id =
                    parse_key_name(key_name).ok_or_else(|| Error::Other(format!("Unknown key: {}", key_name)))?;

                let color = require_color(color_name)?;

                key_colors.push((key_id, color));
            }

            if key_colors.is_empty() {
                return Err(Error::Other("No valid key:color pairs found".to_string()));
            }

            // Check if per-key RGB is supported
            if !keyboard.supports_per_key_rgb() {
                println!("Warning: Per-key RGB not supported on this keyboard");
                println!("Falling back to setting first color on entire keyboard");
                keyboard.set_color(key_colors[0].1).await?;
                keyboard.apply().await?;
                return Ok(());
            }

            println!("Setting {} keys to their respective colors", key_colors.len());
            keyboard.set_key_colors(&key_colors).await?;
            keyboard.apply().await?;
            println!("Done!");
        }

        PerKeyAction::SetRegion {
            start_hid,
            count,
            color,
        } => {
            let color = require_color(&color)?;

            println!(
                "Setting {} keys starting from HID code 0x{:02X} to color {}",
                count, start_hid, color
            );

            keyboard.set_key_region(start_hid, count, color).await?;
            keyboard.apply().await?;
            println!("Done!");
        }

        PerKeyAction::Clear => {
            println!("Clearing all per-key RGB (setting all keys to black)");
            keyboard.clear_per_key_rgb().await?;
            keyboard.apply().await?;
            println!("Done!");
        }

        PerKeyAction::TestMatrix { hid_code, color } => {
            let color = require_color(&color)?;

            println!("Testing HID code 0x{:02X} with color {}", hid_code, color);
            let address = KeyAddress::new(hid_code);
            keyboard.set_key_color_direct(address, color).await?;
            keyboard.apply().await?;
            println!("Done! If no key lights up, this HID code might not exist.");
        }

        PerKeyAction::TestPattern { pattern } => {
            let pattern_name = pattern.to_ascii_lowercase();
            match pattern_name.as_str() {
                "rainbow" => {
                    println!("Testing rainbow pattern across keyboard");
                    test_rainbow_pattern(keyboard).await?;
                }
                "checkerboard" => {
                    println!("Testing checkerboard pattern");
                    test_checkerboard_pattern(keyboard).await?;
                }
                "wave" => {
                    println!("Testing wave pattern");
                    test_wave_pattern(keyboard).await?;
                }
                "test" => {
                    println!("Testing basic key positions");
                    test_basic_positions(keyboard).await?;
                }
                _ => {
                    return Err(Error::Other(format!(
                        "Unknown pattern: {}. Available: rainbow, checkerboard, wave, test",
                        pattern
                    )));
                }
            }
            keyboard.apply().await?;
            println!("Pattern applied!");
        }

        PerKeyAction::ShowMapping => {
            if let Some(mapping) = keyboard.get_key_mapping() {
                println!("Key mapping for {}:", mapping.name);
                println!("Layout: {:?}", mapping.layout);

                let stats = mapping.get_stats();
                println!("Statistics: {}", stats);

                // Show some sample key mappings
                let sample_keys = [KeyId::A, KeyId::S, KeyId::D, KeyId::Enter, KeyId::Space, KeyId::Escape];
                println!("\nSample key mappings:");
                for key_id in sample_keys.iter() {
                    if let Some(address) = mapping.get_key_address(*key_id) {
                        println!("  {} -> {}", key_id, address);
                    }
                }

                if mapping.total_keys > 6 {
                    println!("  ... and {} more keys", mapping.total_keys - 6);
                }
            } else {
                println!("No key mapping available for this keyboard");
                println!("Per-key RGB control is not supported");
            }
        }

        PerKeyAction::Status => {
            println!("Per-key RGB Status:");
            let perkey_supported = keyboard.supports_per_key_rgb();
            if perkey_supported {
                println!("Provisional: true (key mapping exists, hardware protocol unverified)");
                println!("Note: Per-key RGB protocol is experimental - actual hardware behavior may vary");
            } else {
                println!("Provisional: false");
            }

            if let Some(mapping) = keyboard.get_key_mapping() {
                println!("Key mapping: {} ({:?})", mapping.name, mapping.layout);
                println!("Total keys: {}", mapping.total_keys);
                let stats = mapping.get_stats();
                println!("Matrix utilization: {:.1}%", stats.utilization);
            } else {
                println!("Key mapping: None available");
            }

            println!("Zone count: {}", keyboard.zone_count());
        }
    }

    Ok(())
}

// Helper functions for pattern testing
async fn test_rainbow_pattern(keyboard: &mut Box<dyn Keyboard>) -> Result<()> {
    let colors = Color::RAINBOW_COLORS;

    if keyboard.supports_per_key_rgb() {
        // Use logical key mapping if available
        if let Some(mapping) = keyboard.get_key_mapping() {
            let all_keys = mapping.get_all_keys();
            let mut key_colors = Vec::new();

            for (i, key_id) in all_keys.iter().enumerate() {
                let color = colors[i % colors.len()];
                key_colors.push((*key_id, color));
            }

            keyboard.set_key_colors(&key_colors).await?;
        } else {
            // Fallback to HID code addressing
            let mut direct_colors = Vec::new();
            for hid_code in 0..120u8 {
                let color_idx = hid_code % colors.len() as u8;
                direct_colors.push((KeyAddress::new(hid_code), colors[color_idx as usize]));
            }
            keyboard.set_key_colors_direct(&direct_colors).await?;
        }
    } else {
        // Fallback to zone-based rainbow
        let zone_colors: Vec<Color> = (0..keyboard.zone_count()).map(|i| colors[i % colors.len()]).collect();
        keyboard.set_zone_colors(&zone_colors).await?;
    }

    Ok(())
}

async fn test_checkerboard_pattern(keyboard: &mut Box<dyn Keyboard>) -> Result<()> {
    let mut direct_colors = Vec::new();

    for hid_code in 0..120u8 {
        let color = if hid_code % 2 == 0 { Color::WHITE } else { Color::BLACK };
        direct_colors.push((KeyAddress::new(hid_code), color));
    }

    if keyboard.supports_per_key_rgb() {
        keyboard.set_key_colors_direct(&direct_colors).await?;
    } else {
        println!("Checkerboard pattern requires per-key RGB support");
        keyboard.set_color(Color::WHITE).await?;
    }

    Ok(())
}

async fn test_wave_pattern(keyboard: &mut Box<dyn Keyboard>) -> Result<()> {
    let mut direct_colors = Vec::new();

    for hid_code in 0..120u8 {
        let wave_pos = (hid_code as f32 / 120.0) * 2.0 * std::f32::consts::PI;
        let intensity = ((wave_pos.sin() + 1.0) / 2.0 * 255.0) as u8;
        let color = Color::new(intensity, 0, 255 - intensity);
        direct_colors.push((KeyAddress::new(hid_code), color));
    }

    if keyboard.supports_per_key_rgb() {
        keyboard.set_key_colors_direct(&direct_colors).await?;
    } else {
        println!("Wave pattern requires per-key RGB support");
        keyboard.set_color(Color::PURPLE).await?;
    }

    Ok(())
}

async fn test_basic_positions(keyboard: &mut Box<dyn Keyboard>) -> Result<()> {
    // Test various key positions using HID codes
    let test_positions = [
        (KeyAddress::new(0x29), Color::RED),    // ESC key
        (KeyAddress::new(0x04), Color::GREEN),  // A key
        (KeyAddress::new(0x16), Color::BLUE),   // S key
        (KeyAddress::new(0x2C), Color::YELLOW), // SPACE key
        (KeyAddress::new(0x28), Color::WHITE),  // ENTER key
    ];

    if keyboard.supports_per_key_rgb() {
        keyboard.set_key_colors_direct(&test_positions).await?;
    } else {
        println!("Basic position test requires per-key RGB support");
        keyboard.set_color(Color::WHITE).await?;
    }

    Ok(())
}

async fn cmd_profile(action: ProfileAction) -> Result<()> {
    let mut profile_manager = ProfileManager::new().await?;

    match action {
        ProfileAction::List => {
            let profiles = profile_manager.list();
            if profiles.is_empty() {
                println!("No profiles found.");
            } else {
                println!("Profiles:");
                for name in profiles {
                    println!("  - {}", name);
                }
            }
        }

        ProfileAction::Load { name } => {
            if let Some(profile) = profile_manager.get(&name) {
                println!("Loading profile: {}", profile.name);

                let device_manager = new_device_manager()?;
                let state_store = DeviceStateStore::new_async().await?;

                // Apply keyboard settings if present
                if let Some(ref keyboard_profile) = profile.keyboard {
                    if let Some(keyboard_info) = device_manager.first_device_of_type(DeviceType::Keyboard) {
                        let mut keyboard = device_manager.open_keyboard(keyboard_info)?;
                        let device_id = DeviceId::from(keyboard_info);

                        // Apply brightness
                        keyboard.set_brightness(keyboard_profile.brightness).await?;

                        // Apply effect - for static effects, apply immediately
                        match &keyboard_profile.effect {
                            Effect::Static { color } => {
                                keyboard.set_color(*color).await?;
                            }
                            Effect::Off => {
                                keyboard.set_color(Color::BLACK).await?;
                            }
                            _ => {
                                println!("Note: Animated effects require running as daemon");
                            }
                        }

                        // Persist to state store
                        state_store.update_keyboard(
                            device_id,
                            KeyboardState {
                                effect: keyboard_profile.effect.clone(),
                                brightness: keyboard_profile.brightness,
                            },
                        )?;

                        println!("Applied keyboard settings");
                    } else {
                        println!("No keyboard found");
                    }
                }

                println!("Profile loaded!");
            } else {
                println!("Profile not found: {}", name);
            }
        }

        ProfileAction::Save { name } => {
            let mut profile = Profile::new(name.clone());
            let state_store = DeviceStateStore::new_async().await?;
            let device_manager = new_device_manager()?;

            // Capture keyboard settings from state store
            if let Some(keyboard_info) = device_manager.first_device_of_type(DeviceType::Keyboard) {
                let device_id = DeviceId::from(keyboard_info);
                if let Some(device_state) = state_store.get(&device_id)
                    && let Some(ref keyboard_state) = device_state.keyboard
                {
                    profile.keyboard = Some(KeyboardProfile {
                        effect: keyboard_state.effect.clone(),
                        brightness: keyboard_state.brightness,
                    });
                    println!("Captured keyboard settings");
                }
            }

            let saved_profile = profile_manager.set(profile)?;
            println!("Profile saved: {}", saved_profile.name);
        }

        ProfileAction::Delete { name } => {
            profile_manager.delete_async(&name).await?;
            println!("Profile deleted: {}", name);
        }
    }

    Ok(())
}

/// On-board profile commands of a keyboard's on-board protocol. The read collects a
/// profile, `decode` and `encode` work on files, `write` and `load` send frames: all of it runs through the
/// interfaces in src/devices/keyboards/onboard.rs and names no keyboard family - which family a device
/// speaks is what `Keyboard::onboard_codec` answers, and which one a file belongs to is what the codec
/// registry answers.
async fn cmd_onboard(mut manager: Option<&mut DeviceManager>, action: OnboardAction) -> Result<()> {
    use steelseries_gg::devices::keyboards::onboard::{self as onboard, codec_for_bytes, codec_for_json};

    match action {
        OnboardAction::Read(args) => {
            let Some(manager) = manager.as_mut() else {
                return Err(Error::Other("the on-board profile read needs a device".to_string()));
            };
            let keyboard = select_onboard_keyboard(manager, args.device, "read")?;
            // The selection above picked a keyboard whose codec answers, so asking again cannot fail.
            let Some(codec) = keyboard.onboard_codec() else {
                return Err(Error::DeviceCommunication(
                    "the selected keyboard answers no on-board profile codec".to_string(),
                ));
            };
            // GG names its configs 1-5; the family codec maps that number to the slot it stores.
            let slot = codec.slot_for_config(args.profile)?;
            let label = onboard_device_label(keyboard.as_ref());
            println!("reading on the {label}");
            println!();

            if args.verify {
                let first = keyboard.read_onboard_profile(slot).await?;
                let second = keyboard.read_onboard_profile(slot).await?;
                for line in codec.image_lines(&first, &label) {
                    println!("{line}");
                }
                for line in codec.image_lines(&second, &label) {
                    println!("{line}");
                }
                match codec.diff(&first, &second) {
                    Some(offset) => {
                        println!("the two reads differ at profile byte {offset}: treat this pairing as unverified")
                    }
                    None => println!("the two reads agree on all {} profile bytes", first.data.len()),
                }
                let json = onboard_json_path(codec, args.out.as_deref(), &first.data, args.profile);
                let mut profile = codec.decode(&first.data)?;
                // A read's JSON names the keyboard it ran on; a file decode names none.
                first.stamp_identity(&mut profile);
                secure_write_async(json.clone(), codec.profile_json(&profile)?).await?;
                println!("JSON decode report of the first read written to {}", json.display());
                write_onboard_report(codec, args.report.as_ref(), &first).await?;
                if let Some(bin) = args.bin.as_ref() {
                    secure_write_async(bin.clone(), first.data.clone()).await?;
                    let second_path = bin.with_extension("second.bin");
                    secure_write_async(second_path.clone(), second.data.clone()).await?;
                    println!(
                        "both profile images written: {} and {}",
                        bin.display(),
                        second_path.display()
                    );
                    write_onboard_extra(bin, &first).await?;
                }
            } else {
                let image = keyboard.read_onboard_profile(slot).await?;
                for line in codec.image_lines(&image, &label) {
                    println!("{line}");
                }
                if args.hex {
                    print!("{}", codec.hex(&image.data, 0, image.data.len()));
                }
                let json = onboard_json_path(codec, args.out.as_deref(), &image.data, args.profile);
                let mut profile = codec.decode(&image.data)?;
                image.stamp_identity(&mut profile);
                secure_write_async(json.clone(), codec.profile_json(&profile)?).await?;
                println!("JSON decode report written to {}", json.display());
                write_onboard_report(codec, args.report.as_ref(), &image).await?;
                if let Some(bin) = args.bin.as_ref() {
                    secure_write_async(bin.clone(), image.data.clone()).await?;
                    println!("profile image written to {}", bin.display());
                    write_onboard_extra(bin, &image).await?;
                }
            }
        }

        OnboardAction::Decode(args) => {
            let data = read_file(&args.file).await?;
            let codec = codec_for_bytes(&data).ok_or_else(no_family)?;
            println!("schema word: {:04X}", onboard::schema_of(&data));
            let profile = codec.decode(&data)?;
            for line in codec.profile_lines(&profile) {
                println!("{line}");
            }
            if args.hex {
                let from = args.hex_from;
                let to = (from + args.hex_len).min(data.len());
                print!("{}", codec.hex(&data, from, to));
            }
            let json = onboard_json_path(codec, args.out.as_deref(), &data, 1);
            secure_write_async(json.clone(), codec.profile_json(&profile)?).await?;
            println!("JSON decode report written to {}", json.display());
        }
        OnboardAction::Encode(args) => {
            let text = read_text(&args.file).await?;
            let codec = codec_for_json(&text).ok_or_else(no_family)?;
            let data = codec
                .encode_json(&text)
                .map_err(|e| Error::Other(format!("encode {}: {e}", args.file.display())))?;
            // Read the bytes back through the codec's decoder, so what is printed is what the file says,
            // not what the JSON claimed.
            let profile = codec.decode(&data)?;
            println!("{}", codec.summary_line(&profile));
            let out = args.out.unwrap_or_else(|| args.file.with_extension("bin"));
            secure_write_async(out.clone(), data.clone()).await?;
            println!(
                "profile bytes written to {} ({} bytes, checksum {:08X})",
                out.display(),
                data.len(),
                profile.crc_computed
            );
            if let Some(reference) = args.check.as_ref() {
                let wanted = read_file(reference).await?;
                for line in codec.check_lines(&data, &wanted, &reference.display().to_string()) {
                    println!("{line}");
                }
            }
            if args.hex {
                let from = args.hex_from;
                let to = (from + args.hex_len).min(data.len());
                print!("{}", codec.hex(&data, from, to));
            }
        }

        OnboardAction::Write(args) => {
            // The payload source: a decode report encoded here, or payload bytes taken as they are. Both
            // paths pick the family from the bytes' own schema word.
            let (file_codec, payload, source) = match args.bin.as_ref() {
                Some(path) => {
                    let bytes = read_file(path).await?;
                    let codec = codec_for_bytes(&bytes).ok_or_else(no_family)?;
                    (codec, bytes, path.display().to_string())
                }
                None => {
                    let path = args.file.as_ref().ok_or_else(|| {
                        Error::InvalidConfig("a write names a profile JSON, or payload bytes with --bin".to_string())
                    })?;
                    let text = read_text(path).await?;
                    let codec = codec_for_json(&text).ok_or_else(no_family)?;
                    let payload = codec
                        .encode_json(&text)
                        .map_err(|e| Error::Other(format!("encode {}: {e}", path.display())))?;
                    (codec, payload, path.display().to_string())
                }
            };

            let slot = file_codec.slot_for_config(args.profile)?;
            let plan = file_codec
                .write_plan(slot, &payload)
                .map_err(|e| Error::Other(format!("frame the write of {source}: {e}")))?;

            if args.dry_run {
                for line in file_codec.write_banner_lines() {
                    println!("{line}");
                }
                println!();
                for line in onboard::write_plan_lines(&plan) {
                    println!("{line}");
                }
                println!();
                println!(
                    "dry run: no keyboard was opened and nothing was sent; the plan of {} payload bytes is {} \
                     frames for slot {} (GG config {})",
                    payload.len(),
                    plan.frames.len(),
                    slot,
                    args.profile
                );
                return Ok(());
            }

            if !args.yes {
                return Err(Error::InvalidConfig(
                    "an on-board profile write rewrites the slot's stored profile: pass --yes to send it, or \
                     --dry-run to print the plan"
                        .to_string(),
                ));
            }

            let Some(manager) = manager.as_mut() else {
                return Err(Error::Other(
                    "an on-board profile write needs a keyboard: --dry-run prints the plan without one".to_string(),
                ));
            };
            let keyboard = select_onboard_keyboard(manager, args.device, "write")?;
            // The keyboard's own codec frames the write: geometry and file id come from the device the write
            // runs on, not from a codec picked off the payload's schema word.
            let codec = keyboard.onboard_codec().ok_or_else(|| {
                Error::DeviceCommunication(
                    "the selected keyboard answers no on-board profile codec: a write needs the keyboard's own \
                     family geometry"
                        .to_string(),
                )
            })?;
            if codec.family_name() != file_codec.family_name() {
                return Err(Error::InvalidConfig(format!(
                    "{} is a profile of {} ({} payload bytes, schema {:04X}); the selected keyboard speaks {} \
                     - write a profile of the keyboard's own family",
                    source,
                    file_codec.family_name(),
                    payload.len(),
                    onboard::schema_of(&payload),
                    codec.family_name()
                )));
            }
            let slot = codec.slot_for_config(args.profile)?;
            for line in codec.write_banner_lines() {
                println!("{line}");
            }
            println!();
            let plan = codec
                .write_plan(slot, &payload)
                .map_err(|e| Error::Other(format!("frame the write of {source}: {e}")))?;
            for line in onboard::write_plan_lines(&plan) {
                println!("{line}");
            }
            println!();

            let label = onboard_device_label(keyboard.as_ref());
            println!(
                "writing on the {label}: this rewrites the profile file of slot {slot} and makes the keyboard \
                 run that slot"
            );
            let stats = keyboard.write_onboard_profile(slot, &payload).await?;
            let planned = codec.write_stats(&plan);
            println!(
                "write finished: {} of {} frames sent, {} of {} payload bytes, {} of {} pushes, {} of {} \
                 answers read back",
                stats.frames_sent,
                planned.frames_sent,
                stats.bytes_written,
                planned.bytes_written,
                stats.pushes,
                planned.pushes,
                stats.acks,
                planned.acks
            );
            if stats != planned {
                println!("the write did not do every frame its plan listed: read the slot back before trusting it");
            }
            println!(
                "slot {slot} (GG config {}) written and deployed on the {label}",
                args.profile
            );
        }

        OnboardAction::Load(args) => {
            let Some(manager) = manager.as_mut() else {
                return Err(Error::Other("the on-board profile deploy needs a keyboard".to_string()));
            };
            let keyboard = select_onboard_keyboard(manager, args.device, "deploy")?;
            let Some(codec) = keyboard.onboard_codec() else {
                return Err(Error::DeviceCommunication(
                    "the selected keyboard answers no on-board profile codec".to_string(),
                ));
            };
            let slot = codec.slot_for_config(args.profile)?;
            let label = onboard_device_label(keyboard.as_ref());
            keyboard.load_onboard_profile(slot).await?;
            println!(
                "slot {slot} (GG config {}) deployed on the {label}: the keyboard now runs it",
                args.profile
            );
        }
    }

    Ok(())
}

/// The error every file-only on-board command answers when the build has no family codec registered.
fn no_family() -> Error {
    Error::Other("no on-board profile family is registered: decode and encode have nothing to read with".to_string())
}

/// Bytes of a profile file: `onboard decode`'s input, `onboard encode --check`'s target.
async fn read_file(path: &std::path::Path) -> Result<Vec<u8>> {
    tokio::fs::read(path)
        .await
        .map_err(|e| Error::Other(format!("read {}: {e}", path.display())))
}

/// Text of a profile JSON: `onboard encode`'s input.
async fn read_text(path: &std::path::Path) -> Result<String> {
    tokio::fs::read_to_string(path)
        .await
        .map_err(|e| Error::Other(format!("read {}: {e}", path.display())))
}

/// The keyboard an on-board command runs on: the selection the device manager makes, turned into the
/// messages the on-board commands answer with when it does not yield exactly one keyboard. `what` names the
/// command in those messages. Read, write and deploy all run on the keyboard's own handle, so all three use
/// this one selection.
fn select_onboard_keyboard(
    manager: &mut steelseries_gg::devices::discovery::DeviceManager,
    device: Option<u16>,
    what: &str,
) -> Result<Box<dyn Keyboard>> {
    use steelseries_gg::devices::discovery::OnboardKeyboard;

    match manager
        .open_onboard_keyboard(device)
        .map_err(|e| Error::Other(format!("select the on-board profile keyboard: {e}")))?
    {
        OnboardKeyboard::Keyboard(keyboard) => Ok(keyboard),
        OnboardKeyboard::NotFound { product_id } => Err(Error::DeviceNotFound(match product_id {
            Some(pid) => format!(
                "no keyboard with product ID {pid:04X} answers the on-board profile protocol: the {what} runs \
                 on a keyboard's own handle, only keyboards are candidates, and the only keyboard that answers \
                 the protocol is the Apex 7 (0x1612), the board it was measured on. `ssgg devices` lists what \
                 was found; a keyboard listed there that cannot be opened is one whose control interface \
                 another program holds"
            ),
            None => format!(
                "no connected keyboard answers the on-board profile protocol: the {what} runs on a keyboard's \
                 own handle, and only keyboards are candidates (a mouse or a headset is not). A keyboard whose \
                 control interface is taken is not a candidate either: GG's engine (Windows), this crate's \
                 daemon, or an `onboard read` running elsewhere can hold it. Stop what holds it and retry"
            ),
        })),
        OnboardKeyboard::Ambiguous { keyboards } => {
            let listed = keyboards
                .iter()
                .map(|(pid, name)| format!("{name} ({pid:04X})"))
                .collect::<Vec<_>>()
                .join(", ");
            let advice = if keyboards.windows(2).all(|pair| pair[0].0 == pair[1].0) {
                "they share one product ID, which --device cannot tell apart: disconnect all but one"
            } else {
                "name one with --device <pid>"
            };
            Err(Error::InvalidConfig(format!(
                "more than one connected keyboard answers the on-board profile protocol: {listed} - {advice}"
            )))
        }
    }
}

/// Where a device-driven read ran: the keyboard's name and the collection the manager opened.
fn onboard_device_label(keyboard: &dyn Keyboard) -> String {
    format!("{} ({})", keyboard.info().name, keyboard.info().path)
}

/// Where an on-board dump is written: `--out` when given, otherwise the file name the family's codec
/// yields from the profile name inside the payload (`Config 1` -> `config_1.json`). `config` is GG's
/// config number, used only when the payload carries no name to build a file name from.
fn onboard_json_path(
    codec: &dyn OnboardProfileCodec,
    given: Option<&std::path::Path>,
    data: &[u8],
    config: u8,
) -> std::path::PathBuf {
    given.map_or_else(
        || std::path::PathBuf::from(codec.dump_name(data, config)),
        std::path::PathBuf::from,
    )
}

/// The region a read image carries behind its payload, written beside the image as `<out>.extra.bin`.
/// The read fills the whole profile region, and only the payload is decoded; this file is the rest of the
/// region, recorded for a read against GG's capture. Nothing in ssgg decodes these bytes.
async fn write_onboard_extra(
    bin: &std::path::Path,
    image: &steelseries_gg::devices::keyboards::onboard::ProfileImage,
) -> Result<()> {
    let Some(region) = steelseries_gg::devices::keyboards::apex_7_profile::extra_region(&image.data) else {
        return Ok(());
    };
    let extra_path = bin.with_extension("extra.bin");
    secure_write_async(extra_path.clone(), image.data[region.offset..].to_vec()).await?;
    println!(
        "the {} bytes behind the payload written to {} (recorded, decoded by nothing)",
        region.len,
        extra_path.display()
    );
    Ok(())
}

/// The per-pull read record as the family codec writes it: chunk headers, framing stats, the image hash.
/// Capture diagnostics for comparing a read with GG's capture; the decoded profile is what `--out` writes.
async fn write_onboard_report(
    codec: &dyn OnboardProfileCodec,
    path: Option<&std::path::PathBuf>,
    image: &steelseries_gg::devices::keyboards::onboard::ProfileImage,
) -> Result<()> {
    let Some(path) = path else {
        return Ok(());
    };
    secure_write_async(path.to_path_buf(), codec.image_json(image)?).await?;
    println!("read record written to {}", path.display());
    Ok(())
}

async fn cmd_pollrate(action: PollrateAction) -> Result<()> {
    use steelseries_gg::pollrate::{DeviceType, PollRate, get_poll_rate, set_poll_rate};

    match action {
        PollrateAction::Mouse { rate, persistent } => {
            let poll_rate = PollRate::from_hz(rate)?;

            // Warn about hardware requirements for high poll rates
            if poll_rate.requires_hardware_support() {
                println!("⚠ Warning: {} requires hardware support", poll_rate.description());
                println!("  Your mouse may not support this rate or may ignore the setting.");
                println!();
            }

            set_poll_rate(DeviceType::Mouse, poll_rate).await?;
            println!("Mouse polling rate set to {} Hz", rate);

            if persistent {
                let mut config = Config::load_async().await?;
                config.poll_rate.mouse_hz = Some(rate);
                config.save_async().await?;
                println!("Setting saved to config (will apply on daemon startup)");
            }
        }

        PollrateAction::Keyboard { rate, persistent } => {
            let poll_rate = PollRate::from_hz(rate)?;

            // Warn about hardware requirements for high poll rates
            if poll_rate.requires_hardware_support() {
                println!("⚠ Warning: {} requires hardware support", poll_rate.description());
                println!("  Your keyboard may not support this rate or may ignore the setting.");
                println!();
            }

            set_poll_rate(DeviceType::Keyboard, poll_rate).await?;
            println!("Keyboard polling rate set to {} Hz", rate);

            if persistent {
                let mut config = Config::load_async().await?;
                config.poll_rate.keyboard_hz = Some(rate);
                config.save_async().await?;
                println!("Setting saved to config (will apply on daemon startup)");
            }
        }

        PollrateAction::Status => {
            fn print_pollrate_result(label: &str, result: steelseries_gg::error::Result<PollRate>) {
                match result {
                    Ok(rate) => println!("  {label}: {} Hz", rate.to_hz()),
                    Err(e) if e.to_string().contains("not supported by this device's HID driver") => {
                        println!("  {label}: unsupported on this driver — {e}");
                    }
                    Err(e) => println!("  {label}: Error: {e}"),
                }
            }

            println!("Current USB Polling Rates:");
            println!();

            print_pollrate_result("Mouse   ", get_poll_rate(DeviceType::Mouse).await);
            print_pollrate_result("Keyboard", get_poll_rate(DeviceType::Keyboard).await);

            println!();
            println!("Note: Changes require root privileges (sudo)");
        }
    }

    Ok(())
}

#[cfg(feature = "audio")]
fn cmd_audio(action: AudioAction) -> Result<()> {
    let mut mixer = AudioMixer::new()?;

    match action {
        AudioAction::Status => {
            println!("Audio Mixer Status:");
            println!();
            for (channel, state) in mixer.all_channels() {
                let mute_str = if state.muted { " (muted)" } else { "" };
                println!(
                    "  {:8} {:3}%{}",
                    channel.to_string(),
                    (state.volume * 100.0) as u8,
                    mute_str
                );
            }
            println!();
            println!("  Chat Mix: {:+.0}%", mixer.chat_mix() * 100.0);
        }

        AudioAction::Volume { channel, level } => {
            let ch = parse_channel(&channel).ok_or_else(|| Error::Other(format!("Invalid channel: {}", channel)))?;

            let volume = normalize_volume(level);
            mixer.set_volume(ch, volume)?;
            println!("{} volume set to {}%", ch, level.min(100));
        }

        AudioAction::Mute { channel, state } => {
            let ch = parse_channel(&channel).ok_or_else(|| Error::Other(format!("Invalid channel: {}", channel)))?;

            let muted = match state {
                Some(s) => {
                    mixer.set_mute(ch, s)?;
                    s
                }
                None => mixer.toggle_mute(ch)?,
            };

            println!("{} {}", ch, if muted { "muted" } else { "unmuted" });
        }

        AudioAction::ChatMix { balance } => {
            let balance = (balance.clamp(-100, 100) as f32) / 100.0;
            mixer.set_chat_mix(balance)?;
            println!("Chat mix set to {:+.0}%", balance * 100.0);
        }
    }

    Ok(())
}

#[cfg(feature = "sonar")]
async fn cmd_sonar(action: SonarAction) -> Result<()> {
    match action {
        SonarAction::Status => {
            println!("Connecting to SteelSeries Sonar...");
            let client = SonarClient::new().await?;
            println!("Connected to Sonar API: {}", client.base_url());
            println!();

            // Get mode
            match client.get_mode().await {
                Ok(mode) => println!("Mode: {:?}", mode),
                Err(e) => println!("Failed to get mode: {}", e),
            }

            // Get classic volumes
            match client.get_classic_volumes().await {
                Ok(volumes) => {
                    println!("\nClassic Mode Volumes:");
                    println!("  Master: {:.0}%", volumes.master * 100.0);
                    println!("  Game:   {:.0}%", volumes.game * 100.0);
                    println!("  Chat:   {:.0}%", volumes.chat * 100.0);
                    println!("  Media:  {:.0}%", volumes.media * 100.0);
                    println!("  Aux:    {:.0}%", volumes.aux * 100.0);
                }
                Err(e) => println!("\nFailed to get classic volumes: {}", e),
            }

            // Get chat mix
            match client.get_chat_mix().await {
                Ok(chat_mix) => println!("\nChat Mix: {:.0}%", chat_mix.value * 100.0),
                Err(e) => println!("\nFailed to get chat mix: {}", e),
            }
        }

        SonarAction::Discover => {
            println!("Discovering Sonar API port...");
            let client = SonarClient::new().await?;
            println!("Sonar API found at: {}", client.base_url());
        }

        SonarAction::Devices => {
            let client = SonarClient::new().await?;
            let devices = client.get_audio_devices().await?;

            println!("Audio Devices:");
            for device in devices {
                println!("  [{}] {} ({})", device.id, device.name, device.device_type);
            }
        }

        SonarAction::Mode => {
            let client = SonarClient::new().await?;
            let mode = client.get_mode().await?;
            println!("Current mode: {:?}", mode);
        }

        SonarAction::Volume { channel, level } => {
            const VALID_CHANNELS: &[&str] = &["master", "game", "chat", "media", "aux"];
            parse_sonar_channel(&channel, VALID_CHANNELS)?;

            let client = SonarClient::new().await?;
            let volume = normalize_volume(level);
            let channel_lower = channel.to_ascii_lowercase();

            match channel_lower.as_str() {
                "master" => client.set_classic_master_volume(volume).await?,
                "game" => client.set_classic_game_volume(volume).await?,
                "chat" => client.set_classic_chat_volume(volume).await?,
                "media" => client.set_classic_media_volume(volume).await?,
                "aux" => client.set_classic_aux_volume(volume).await?,
                _ => unreachable!("Validated by parse_sonar_channel"),
            }

            println!("{} volume set to {}%", channel, level.min(100));
        }

        SonarAction::ChatMix => {
            let client = SonarClient::new().await?;
            let chat_mix = client.get_chat_mix().await?;
            println!("Chat Mix: {:.0}%", chat_mix.value * 100.0);
        }

        SonarAction::Streamer { action } => {
            let client = SonarClient::new().await?;

            match action {
                StreamerAction::Monitoring { channel, level } => {
                    const VALID_CHANNELS: &[&str] = &["master", "game", "chat"];
                    parse_sonar_channel(&channel, VALID_CHANNELS)?;

                    let volume = normalize_volume(level);
                    let channel_lower = channel.to_ascii_lowercase();

                    match channel_lower.as_str() {
                        "master" => client.set_monitoring_master_volume(volume).await?,
                        "game" => client.set_monitoring_game_volume(volume).await?,
                        "chat" => client.set_monitoring_chat_volume(volume).await?,
                        _ => unreachable!("Validated by parse_sonar_channel"),
                    }

                    println!("Monitoring {} volume set to {}%", channel, level.min(100));
                }

                StreamerAction::Streaming { channel, level } => {
                    const VALID_CHANNELS: &[&str] = &["master"];
                    parse_sonar_channel(&channel, VALID_CHANNELS)?;

                    let volume = normalize_volume(level);
                    let channel_lower = channel.to_ascii_lowercase();

                    match channel_lower.as_str() {
                        "master" => client.set_streaming_master_volume(volume).await?,
                        _ => unreachable!("Validated by parse_sonar_channel"),
                    }

                    println!("Streaming {} volume set to {}%", channel, level.min(100));
                }
            }
        }

        SonarAction::Configs => {
            let client = SonarClient::new().await?;
            let configs = client.get_configs().await?;

            println!("Audio Configurations:");
            for config in configs {
                let selected = if config.selected { " (selected)" } else { "" };
                println!("  [{}] {}{}", config.id, config.name, selected);
            }
        }
    }

    Ok(())
}

async fn cmd_validate(
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

async fn cmd_performance(manager: &DeviceManager, action: PerformanceAction) -> Result<()> {
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

/// Show device status with optional live monitoring.
async fn cmd_status(_initial_manager: &DeviceManager, device_filter: &str, refresh_ms: u64) -> Result<()> {
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
        "all" => None,
        _ => {
            return Err(Error::Other(format!(
                "Invalid device filter: {}. Use 'keyboard', 'headset', or 'all'",
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
async fn cmd_hid_logs(file_logging: bool, device_filter: Option<&str>) -> Result<()> {
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

/// Run automated device tests to verify responsiveness.
async fn cmd_test_device(manager: &DeviceManager, device: &str, benchmark: bool, verbose: bool) -> Result<()> {
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
async fn cmd_verify_performance(
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
