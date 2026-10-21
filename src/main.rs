//! SteelSeries GG for Linux - CLI
//!
//! A complete open-source replacement for SteelSeries GG on Linux.

mod cli;
mod commands;
mod daemon;
mod gui;

use clap::Parser;

use crate::cli::{
    ActuationAction, Cli, Commands, HeadsetAction, MouseAction, PerKeyAction, PollrateAction, ProfileAction,
    RemapAction, RgbAction, parse_key_name, require_color,
};
use crate::commands::{
    cmd_bug_report, cmd_hid_logs, cmd_onboard, cmd_performance, cmd_status, cmd_test_device, cmd_validate,
    cmd_verify_performance,
};
use crate::daemon::{cmd_daemon, cmd_server};
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

use steelseries_gg::config::Config;
use steelseries_gg::device_state::{DeviceId, DeviceStateStore, KeyboardState};
use steelseries_gg::devices::keyboards::Keyboard;
use steelseries_gg::devices::{
    DeviceInfo, DeviceManager, DeviceType, KeyAddress, KeyId,
    diagnostics::{init_global_diagnostics, with_global_diagnostics},
    discovery::print_device_summary,
};
use steelseries_gg::profiles::{KeyboardProfile, Profile, ProfileManager};
use steelseries_gg::rgb::{Color, Effect, WaveDirection};
use steelseries_gg::{Error, Result};

use colored::Colorize;

#[cfg(feature = "audio")]
use crate::cli::{AudioAction, parse_channel};

#[cfg(any(feature = "audio", feature = "sonar"))]
use crate::cli::normalize_volume;

#[cfg(feature = "sonar")]
use crate::cli::{SonarAction, StreamerAction, parse_sonar_channel};

#[cfg(feature = "audio")]
use steelseries_gg::audio::AudioMixer;

#[cfg(feature = "sonar")]
use steelseries_gg::audio::SonarClient;

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

        Commands::Headset { action } => {
            let manager = new_device_manager()?;
            cmd_headset(&manager, action)?;
        }

        Commands::Mouse { action } => {
            let manager = new_device_manager()?;
            cmd_mouse(&manager, action)?;
        }

        Commands::Remap { action } => {
            cmd_remap(action).await?;
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

        Commands::Gui { port } => {
            let manager = new_device_manager()?;
            gui::cmd_gui(&manager, port).await?;
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
pub(crate) fn new_device_manager() -> Result<DeviceManager> {
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
    all.extend(manager.devices_by_type(DeviceType::Mouse));
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

fn cmd_headset(manager: &DeviceManager, action: HeadsetAction) -> Result<()> {
    let headset_info = manager
        .first_device_of_type(DeviceType::Headset)
        .ok_or_else(|| Error::Other("No supported headset found".to_string()))?;

    println!("Using headset: {}", headset_info.name);
    warn_if_gg_engine_running();

    let mut headset = manager.open_headset(headset_info)?;
    headset.initialize()?;

    eprintln!(
        "{}",
        "[EXPERIMENTAL] Headset commands come from community forks and are untested on hardware.".yellow()
    );

    match action {
        HeadsetAction::Status => {
            let status = headset.status()?;
            let yes_no = |v: Option<bool>| match v {
                Some(true) => "yes".to_string(),
                Some(false) => "no".to_string(),
                None => "unknown".to_string(),
            };
            println!("Connected: {}", yes_no(status.connected));
            println!("Charging:  {}", yes_no(status.charging));
            match status.battery_percent {
                Some(percent) => println!("Battery:   {}%", percent),
                None => println!("Battery:   unknown"),
            }
            match status.chat_mix {
                Some(mix) => println!("ChatMix:   {:+.2} (-1 = game, +1 = chat)", mix),
                None => println!("ChatMix:   unsupported"),
            }
        }
        HeadsetAction::Sidetone { level } => {
            headset.set_sidetone(level)?;
            println!("Sidetone request sent ({}).", level);
        }
        HeadsetAction::AutoOff { minutes } => {
            headset.set_auto_off(minutes)?;
            println!("Auto-off request sent ({} min).", minutes);
        }
        HeadsetAction::MicVolume { level } => {
            headset.set_mic_volume(level)?;
            println!("Microphone volume request sent ({}).", level);
        }
    }

    Ok(())
}

fn cmd_mouse(manager: &DeviceManager, action: MouseAction) -> Result<()> {
    let mouse_info = manager
        .first_device_of_type(DeviceType::Mouse)
        .ok_or_else(|| Error::Other("No supported mouse found".to_string()))?;

    println!("Using mouse: {}", mouse_info.name);
    eprintln!(
        "{}",
        "[EXPERIMENTAL] Mouse commands come from a community rivalcfg-derived fork and are untested on hardware. Settings apply to the running mouse and are not saved to its memory."
            .yellow()
    );
    warn_if_gg_engine_running();

    let mut mouse = manager.open_mouse(mouse_info)?;
    mouse.initialize()?;

    match action {
        MouseAction::Status => match mouse.battery_level()? {
            Some(percent) => println!("Battery: {}%", percent),
            None => println!("Battery: unknown or not a wireless model"),
        },
        MouseAction::Dpi { value } => {
            mouse.set_dpi(value)?;
            println!("DPI request sent ({}).", value);
        }
        MouseAction::PollRate { hz } => {
            mouse.set_poll_rate(hz)?;
            println!("Polling rate request sent ({} Hz).", hz);
        }
        MouseAction::Color { color } => {
            mouse.set_color(require_color(&color)?)?;
            println!("Colour request sent ({}).", color);
        }
    }

    Ok(())
}

async fn cmd_remap(action: RemapAction) -> Result<()> {
    let config = Config::load_async().await?;
    let bindings = config.remap.bindings()?;
    let product_id = config.remap.product_id()?;

    match action {
        RemapAction::List => {
            if bindings.is_empty() {
                println!("No bindings configured. Add a [remap.bindings] table to config.toml.");
            }
            print_remap_bindings(&bindings);
            Ok(())
        }
        RemapAction::Run => {
            if bindings.is_empty() {
                return Err(Error::InvalidConfig(
                    "no key bindings configured; add a [remap.bindings] table to config.toml".to_string(),
                ));
            }
            run_remap(bindings, product_id).await
        }
    }
}

fn print_remap_bindings(bindings: &steelseries_gg::input::BindingSet) {
    use steelseries_gg::input::Action;

    for (source, action) in bindings.sorted() {
        let target = match action {
            Action::Key(key) => key.to_string(),
            Action::Combo(keys) => keys.iter().map(|k| k.to_string()).collect::<Vec<_>>().join(" + "),
        };
        println!("  {source} -> {target}");
    }
}

#[cfg(not(all(feature = "remap", target_os = "linux")))]
async fn run_remap(_bindings: steelseries_gg::input::BindingSet, _product_id: Option<u16>) -> Result<()> {
    Err(Error::Other(
        "the key remapper is not available in this build (it needs Linux and the `remap` cargo feature)".to_string(),
    ))
}

#[cfg(all(feature = "remap", target_os = "linux"))]
async fn run_remap(bindings: steelseries_gg::input::BindingSet, product_id: Option<u16>) -> Result<()> {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::signal::unix::{SignalKind, signal};

    eprintln!(
        "{}",
        "[EXPERIMENTAL] The key remapper has not been run against real hardware.".yellow()
    );
    println!("Active bindings:");
    print_remap_bindings(&bindings);
    println!("Escape chord: hold left Ctrl + right Ctrl + left Shift + right Shift for 2 seconds to stop.");
    println!("Press Ctrl-C to stop.");

    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&stop);
    let mut worker =
        tokio::task::spawn_blocking(move || steelseries_gg::input::linux::run(&bindings, product_id, &worker_stop));
    let mut terminate = signal(SignalKind::terminate())?;

    tokio::select! {
        result = &mut worker => return result.map_err(|e| Error::Other(e.to_string()))?,
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate.recv() => {}
    }

    stop.store(true, Ordering::SeqCst);
    worker.await.map_err(|e| Error::Other(e.to_string()))?
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
