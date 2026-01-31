//! SteelSeries GG for Linux - CLI
//!
//! A complete open-source replacement for SteelSeries GG on Linux.

use clap::{Parser, Subcommand};
use tracing::{Level, debug, info, warn};
use tracing_subscriber::FmtSubscriber;

use steelseries_gg::config::Config;
use steelseries_gg::device_state::{DeviceId, DeviceStateStore, KeyboardState};
use steelseries_gg::devices::headsets::Headset;
use steelseries_gg::devices::keyboards::Keyboard;
use steelseries_gg::devices::{
    DeviceInfo, DeviceManager, DeviceType, KeyAddress, KeyId,
    diagnostics::{init_global_diagnostics, with_global_diagnostics},
    discovery::{DeviceFingerprint, HotPlugEvent, print_device_summary},
};
use steelseries_gg::gamesense::GameSenseServer;
use steelseries_gg::profiles::{KeyboardProfile, Profile, ProfileManager};
use steelseries_gg::rgb::{Color, Effect, RgbController, WaveDirection};
use steelseries_gg::validation::RgbValidator;
use steelseries_gg::{Error, Result};

use std::collections::HashMap;
use std::io::IsTerminal;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use colored::Colorize;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tabled::{Table, Tabled};

#[cfg(feature = "audio")]
use steelseries_gg::audio::{AudioMixer, Channel};

#[cfg(feature = "sonar")]
use steelseries_gg::audio::SonarClient;

/// SteelSeries GG for Linux - Control your SteelSeries devices
#[derive(Parser)]
#[command(name = "ssgg")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,

    /// Enable HID communication debugging and diagnostics
    #[arg(long)]
    debug_hid: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List connected SteelSeries devices
    Devices,

    /// Control RGB lighting
    Rgb {
        #[command(subcommand)]
        action: RgbAction,
    },

    /// Control actuation points on compatible keyboards
    Actuation {
        #[command(subcommand)]
        action: ActuationAction,
    },

    /// Manage profiles
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },

    /// Control audio mixer (Sonar replacement)
    #[cfg(feature = "audio")]
    Audio {
        #[command(subcommand)]
        action: AudioAction,
    },

    /// Control SteelSeries Sonar (direct API access)
    #[cfg(feature = "sonar")]
    Sonar {
        #[command(subcommand)]
        action: SonarAction,
    },

    /// Configure USB polling rate (requires sudo)
    Pollrate {
        #[command(subcommand)]
        action: PollrateAction,
    },

    /// Start the GameSense server
    Server {
        /// Port to listen on
        #[arg(short, long, default_value = "27301")]
        port: u16,
    },

    /// Run validation tests on connected devices
    Validate {
        /// Enable performance benchmarks
        #[arg(short, long)]
        benchmark: bool,

        /// Test timeout in seconds
        #[arg(short, long, default_value = "30")]
        timeout: u64,

        /// Export validation report to file
        #[arg(short, long)]
        output: Option<String>,

        /// JSON output format
        #[arg(long)]
        json: bool,
    },

    /// Monitor and control RGB performance optimizations
    Performance {
        #[command(subcommand)]
        action: PerformanceAction,
    },

    /// Generate a comprehensive bug report with diagnostic information
    BugReport {
        /// Output file path
        #[arg(short, long, default_value = "ssgg_bug_report.json")]
        output: String,

        /// Include HID communication logs (if available)
        #[arg(long)]
        include_hid_logs: bool,

        /// Include performance metrics snapshot (if daemon is running)
        #[arg(long)]
        include_performance: bool,
    },

    /// Show real-time device connection status
    Status {
        /// Filter by device type (keyboard, headset, or all)
        #[arg(short, long, default_value = "all")]
        device: String,

        /// Refresh interval in milliseconds
        #[arg(short, long, default_value = "1000")]
        refresh: u64,
    },

    /// View HID communication logs with filtering
    HidLogs {
        /// Enable file logging
        #[arg(short, long)]
        file: bool,

        /// Filter by device type (keyboard, headset, or all)
        #[arg(short, long)]
        device: Option<String>,
    },

    /// Run as a daemon (device control + GameSense server)
    Daemon,

    /// Run automated device tests to verify responsiveness
    TestDevice {
        /// Device name or path to test
        device: String,

        /// Enable performance benchmarks
        #[arg(short, long)]
        benchmark: bool,

        /// Show detailed output including passing tests
        #[arg(short, long)]
        verbose: bool,
    },

    /// Verify RGB performance metrics over time
    VerifyPerformance {
        /// Duration to monitor in seconds
        #[arg(short, long, default_value = "30")]
        duration: u64,

        /// Effect to test (breathing, spectrum, wave, etc.)
        #[arg(short, long, default_value = "breathing")]
        effect: String,

        /// Export metrics to JSON file
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Protocol Fuzzer (Developer Tool)
    #[command(hide = true)]
    Fuzz {
        /// Start command byte
        #[arg(short, long, default_value = "0x00", value_parser = parse_hex_u8)]
        start: u8,

        /// End command byte
        #[arg(short, long, default_value = "0xFF", value_parser = parse_hex_u8)]
        end: u8,

        /// Delay between commands in ms
        #[arg(short, long, default_value = "100")]
        delay: u64,
    },
}

#[derive(Subcommand)]
enum RgbAction {
    /// Set a static color
    Color {
        /// Color as hex (e.g., FF0000) or name (red, green, blue, etc.)
        color: String,
    },

    /// Set brightness (0-100)
    Brightness {
        /// Brightness level
        level: u8,
    },

    /// Set a lighting effect
    Effect {
        /// Effect name: static, breathing, spectrum, wave, off
        name: String,

        /// Effect speed (0.1 - 5.0)
        #[arg(short, long, default_value = "1.0")]
        speed: f32,
    },

    /// Turn off all LEDs
    Off,

    /// Per-key RGB control (requires supported keyboard with key mapping)
    Perkey {
        #[command(subcommand)]
        action: PerKeyAction,
    },
}

#[derive(Subcommand)]
enum ActuationAction {
    /// Set actuation point for all keys (global)
    Set {
        /// Actuation point in millimeters (0.1 to 4.0mm, e.g., 1.2, 2.5, 3.6)
        mm: f32,
    },

    /// Set actuation point in 0.1mm increments (e.g., 4 = 0.4mm, 25 = 2.5mm)
    SetValue {
        /// Actuation value in 0.1mm units (1-40)
        value: u8,
    },
}

#[derive(Subcommand)]
enum PerKeyAction {
    /// Set a single key to a specific color
    SetKey {
        /// Key name (e.g., A, Enter, Space, F1, etc.) - case insensitive
        key: String,
        /// Color as hex (e.g., FF0000) or name (red, green, blue, etc.)
        color: String,
    },

    /// Set multiple keys to specific colors
    SetKeys {
        /// Key-color pairs in format "key:color,key:color" (e.g., "A:red,S:green,D:blue")
        keys: String,
    },

    /// Set a rectangular region of keys to the same color using matrix coordinates
    SetRegion {
        /// Starting row (0-based)
        start_row: u8,
        /// Starting column (0-based)
        start_col: u8,
        /// Number of rows
        rows: u8,
        /// Number of columns
        cols: u8,
        /// Color as hex (e.g., FF0000) or name (red, green, blue, etc.)
        color: String,
    },

    /// Turn off all per-key RGB (set all keys to black)
    Clear,

    /// Test individual key matrix positions (direct addressing)
    TestMatrix {
        /// Row number (0-based)
        row: u8,
        /// Column number (0-based)
        col: u8,
        /// Color as hex (e.g., FF0000) or name (red, green, blue, etc.)
        color: String,
    },

    /// Test a pattern across the keyboard
    TestPattern {
        /// Pattern name: rainbow, checkerboard, wave, test
        pattern: String,
    },

    /// Show keyboard mapping information
    ShowMapping,

    /// Show per-key RGB support status
    Status,
}

#[derive(Subcommand)]
enum ProfileAction {
    /// List all profiles
    List,

    /// Load a profile
    Load {
        /// Profile name
        name: String,
    },

    /// Save current settings as a profile
    Save {
        /// Profile name
        name: String,
    },

    /// Delete a profile
    Delete {
        /// Profile name
        name: String,
    },
}

#[cfg(feature = "audio")]
#[derive(Subcommand)]
enum AudioAction {
    /// Show current mixer state
    Status,

    /// Set channel volume
    Volume {
        /// Channel: master, game, chat, media, aux, mic
        channel: String,

        /// Volume level (0-100)
        level: u8,
    },

    /// Mute/unmute a channel
    Mute {
        /// Channel: master, game, chat, media, aux, mic
        channel: String,

        /// Mute state (true/false), omit to toggle
        #[arg(short, long)]
        state: Option<bool>,
    },

    /// Set chat mix balance
    ChatMix {
        /// Balance (-100 = game, 0 = balanced, 100 = chat)
        balance: i8,
    },
}

#[cfg(feature = "sonar")]
#[derive(Subcommand)]
enum SonarAction {
    /// Show current Sonar status and volumes
    Status,

    /// Discover the Sonar API port
    Discover,

    /// Get audio devices
    Devices,

    /// Get current mode (classic or streamer)
    Mode,

    /// Set volume for a channel (classic mode)
    Volume {
        /// Channel: master, game, chat, media, aux
        channel: String,

        /// Volume level (0-100)
        level: u8,
    },

    /// Get chat mix settings
    ChatMix,

    /// Control streamer mode
    Streamer {
        #[command(subcommand)]
        action: StreamerAction,
    },

    /// Get all configurations
    Configs,
}

#[cfg(feature = "sonar")]
#[derive(Subcommand)]
enum StreamerAction {
    /// Set monitoring volume for a channel
    Monitoring {
        /// Channel: master, game, chat
        channel: String,

        /// Volume level (0-100)
        level: u8,
    },

    /// Set streaming volume for a channel
    Streaming {
        /// Channel: master, game, chat
        channel: String,

        /// Volume level (0-100)
        level: u8,
    },
}

#[derive(Subcommand)]
enum PollrateAction {
    /// Set mouse polling rate
    Mouse {
        /// Polling rate in Hz (125, 500, 1000, 2000, 4000)
        /// Note: Rates above 1000Hz require hardware support
        rate: u32,

        /// Save to config and apply on daemon startup
        #[arg(long)]
        persistent: bool,
    },

    /// Set keyboard polling rate
    Keyboard {
        /// Polling rate in Hz (125, 500, 1000, 2000, 4000)
        /// Note: Rates above 1000Hz require hardware support
        rate: u32,

        /// Save to config and apply on daemon startup
        #[arg(long)]
        persistent: bool,
    },

    /// Show current polling rates
    Status,
}

#[derive(Subcommand)]
enum PerformanceAction {
    /// Show current performance statistics
    Stats {
        /// Continuously monitor (update every N seconds)
        #[arg(short, long)]
        monitor: Option<u64>,

        /// Export stats to file
        #[arg(short, long)]
        output: Option<String>,

        /// JSON output format
        #[arg(long)]
        json: bool,
    },

    /// Enable performance optimizations
    Enable,

    /// Disable performance optimizations
    Disable,

    /// Cleanup performance caches
    Cleanup,

    /// Run performance benchmark
    Benchmark {
        /// Duration in seconds
        #[arg(short, long, default_value = "10")]
        duration: u64,

        /// Export results to file
        #[arg(short, long)]
        output: Option<String>,
    },
}

fn parse_color(s: &str) -> Option<Color> {
    // Try named colors
    let s_lower = s.to_ascii_lowercase();
    match s_lower.as_str() {
        "red" => return Some(Color::RED),
        "green" => return Some(Color::GREEN),
        "blue" => return Some(Color::BLUE),
        "white" => return Some(Color::WHITE),
        "black" | "off" => return Some(Color::BLACK),
        "cyan" => return Some(Color::CYAN),
        "magenta" => return Some(Color::MAGENTA),
        "yellow" => return Some(Color::YELLOW),
        "orange" => return Some(Color::ORANGE),
        "purple" => return Some(Color::PURPLE),
        "pink" => return Some(Color::PINK),
        _ => {}
    }

    // Try hex
    let hex = s.trim_start_matches('#');
    if hex.len() == 6 {
        if let Ok(val) = u32::from_str_radix(hex, 16) {
            return Some(Color::from_hex(val));
        }
    }

    None
}

fn parse_hex_u8(s: &str) -> std::result::Result<u8, String> {
    let s = s.trim_start_matches("0x");
    u8::from_str_radix(s, 16).map_err(|e| format!("Invalid hex value: {}", e))
}

#[cfg(feature = "audio")]
fn parse_channel(s: &str) -> Option<Channel> {
    let s_lower = s.to_ascii_lowercase();
    match s_lower.as_str() {
        "master" => Some(Channel::Master),
        "game" => Some(Channel::Game),
        "chat" => Some(Channel::Chat),
        "media" => Some(Channel::Media),
        "aux" => Some(Channel::Aux),
        "mic" => Some(Channel::Mic),
        _ => None,
    }
}

/// Convert a volume level (0-100) to a normalized float (0.0-1.0)
#[cfg(feature = "audio")]
fn normalize_volume(level: u8) -> f32 {
    (level.min(100) as f32) / 100.0
}

/// Parse and validate a Sonar channel name
#[cfg(feature = "sonar")]
fn parse_sonar_channel<'a>(channel: &'a str, valid_channels: &[&str]) -> Result<&'a str> {
    let channel_lower = channel.to_ascii_lowercase();
    if valid_channels.contains(&channel_lower.as_str()) {
        Ok(channel)
    } else {
        Err(Error::Other(format!(
            "Invalid channel: {}. Valid channels: {}",
            channel,
            valid_channels.join(", ")
        )))
    }
}

/// Parse a zone identifier (e.g., "zone1", "2", "all") into a zone index (0-based).
/// Returns None for "all"/"keyboard" which should apply to all zones.
#[inline]
fn parse_zone_number(zone: &str) -> Option<usize> {
    // Fast path: check common cases without string allocation
    if zone.eq_ignore_ascii_case("all") || zone.eq_ignore_ascii_case("keyboard") {
        return None;
    }

    // Try parsing as "zone<number>" or just "<number>"
    // Use case-insensitive prefix check to avoid allocation
    let number_part = if zone.len() > 4 && zone[0..4].eq_ignore_ascii_case("zone") {
        &zone[4..]
    } else {
        zone
    };

    number_part.parse::<usize>().ok().and_then(|one_based| {
        // Convert 1-based to 0-based index
        if one_based > 0 {
            Some(one_based - 1)
        } else {
            None
        }
    })
}

/// Generate a comprehensive bug report with diagnostic information.
async fn cmd_bug_report(
    output: &str,
    include_hid_logs: bool,
    include_performance: bool,
) -> Result<()> {
    use steelseries_gg::diagnostics_export::{collect_bug_report, export_bug_report};

    println!("Collecting diagnostic information...");
    println!("  - System information (OS, kernel, memory, CPU)");
    println!("  - Device states (connected devices, current settings)");

    if include_hid_logs {
        println!("  - HID communication logs");
    }

    if include_performance {
        println!("  - Performance metrics snapshot");
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
            let manager = DeviceManager::new()?;
            cmd_devices(&manager)?;
        }

        Commands::Rgb { action } => {
            let manager = DeviceManager::new()?;
            cmd_rgb(&manager, action)?;
        }

        Commands::Actuation { action } => {
            let manager = DeviceManager::new()?;
            cmd_actuation(&manager, action)?;
        }

        Commands::Profile { action } => {
            cmd_profile(action)?;
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
            cmd_pollrate(action)?;
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
            let manager = DeviceManager::new()?;
            cmd_validate(&manager, benchmark, timeout, output, json)?;
        }

        Commands::Performance { action } => {
            let manager = DeviceManager::new()?;
            cmd_performance(&manager, action)?;
        }

        Commands::BugReport {
            output,
            include_hid_logs,
            include_performance,
        } => {
            cmd_bug_report(&output, include_hid_logs, include_performance).await?;
        }

        Commands::Status { device, refresh } => {
            let manager = DeviceManager::new()?;
            cmd_status(&manager, &device, refresh).await?;
        }

        Commands::HidLogs { file, device } => {
            cmd_hid_logs(file, device.as_deref()).await?;
        }

        Commands::Daemon => {
            let manager = DeviceManager::new()?;
            cmd_daemon(manager).await?;
        }

        Commands::TestDevice {
            device,
            benchmark,
            verbose,
        } => {
            let manager = DeviceManager::new()?;
            cmd_test_device(&manager, &device, benchmark, verbose)?;
        }

        Commands::VerifyPerformance {
            duration,
            effect,
            output,
        } => {
            let manager = DeviceManager::new()?;
            cmd_verify_performance(&manager, duration, &effect, output).await?;
        }

        Commands::Fuzz { start, end, delay } => {
            use steelseries_gg::devices::fuzz::{
                FuzzParams, PayloadPattern, fuzz_keyboard_protocol,
            };

            let manager = DeviceManager::new()?;
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

fn cmd_devices(manager: &DeviceManager) -> Result<()> {
    print_device_summary(manager);
    Ok(())
}

fn cmd_rgb(manager: &DeviceManager, action: RgbAction) -> Result<()> {
    // Find the first keyboard
    let keyboard_info = manager
        .first_device_of_type(DeviceType::Keyboard)
        .ok_or_else(|| Error::Other("No keyboard found".to_string()))?;

    println!("Using keyboard: {}", keyboard_info.name);

    // Open device state store for persistence
    let state_store = DeviceStateStore::new()?;
    let device_id = DeviceId::from(keyboard_info);

    // Open the keyboard using the abstraction layer
    let mut keyboard = manager.open_keyboard(keyboard_info)?;

    // Initialize the device
    keyboard.initialize()?;

    match action {
        RgbAction::Color { color } => {
            let color = parse_color(&color)
                .ok_or_else(|| Error::Other(format!("Invalid color: {}", color)))?;

            println!("Setting color to {}", color);
            keyboard.set_color(color)?;
            keyboard.apply()?; // Apply the color change

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
            keyboard.set_brightness(level)?;

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
                    colors: vec![
                        Color::RED,
                        Color::ORANGE,
                        Color::YELLOW,
                        Color::GREEN,
                        Color::CYAN,
                        Color::BLUE,
                        Color::PURPLE,
                    ],
                    speed,
                    direction: WaveDirection::LeftToRight,
                },
                "off" => Effect::Off,
                _ => Effect::Static {
                    color: Color::WHITE,
                },
            };

            println!("Setting effect: {:?}", effect);
            // Persist effect to state store
            state_store.update_keyboard_effect(device_id, effect)?;
            // Note: Full effect implementation would require a background loop
            println!("(Note: Animated effects require running as daemon)");
        }

        RgbAction::Off => {
            println!("Turning off LEDs");
            keyboard.set_color(Color::BLACK)?;
            keyboard.apply()?; // Apply the off state

            // Persist the off state
            state_store.update_keyboard_effect(device_id, Effect::Off)?;
            println!("Done!");
            println!("Note: LEDs should now be off (black/dark). Device accepted the command.");
        }

        RgbAction::Perkey { action } => {
            cmd_per_key_rgb(&mut keyboard, action)?;
        }
    }

    Ok(())
}

fn cmd_actuation(manager: &DeviceManager, action: ActuationAction) -> Result<()> {
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
            keyboard.apply()?;
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
            keyboard.apply()?;
            println!("Actuation value set successfully!");
        }
    }

    Ok(())
}

fn cmd_per_key_rgb(keyboard: &mut Box<dyn Keyboard>, action: PerKeyAction) -> Result<()> {
    match action {
        PerKeyAction::SetKey { key, color } => {
            // Parse the key name to KeyId
            let key_id = parse_key_name(&key)
                .ok_or_else(|| Error::Other(format!("Unknown key: {}", key)))?;

            // Parse the color
            let color = parse_color(&color)
                .ok_or_else(|| Error::Other(format!("Invalid color: {}", color)))?;

            // Check if per-key RGB is supported
            if !keyboard.supports_per_key_rgb() {
                println!("Warning: Per-key RGB not supported on this keyboard");
                println!("Available key mapping: None");
                println!("Falling back to zone-based RGB (setting entire keyboard)");
                keyboard.set_color(color)?;
                keyboard.apply()?;
                return Ok(());
            }

            println!("Setting key '{}' to color {}", key, color);
            keyboard.set_key_color(key_id, color)?;
            keyboard.apply()?;
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

                let key_id = parse_key_name(key_name)
                    .ok_or_else(|| Error::Other(format!("Unknown key: {}", key_name)))?;

                let color = parse_color(color_name)
                    .ok_or_else(|| Error::Other(format!("Invalid color: {}", color_name)))?;

                key_colors.push((key_id, color));
            }

            if key_colors.is_empty() {
                return Err(Error::Other("No valid key:color pairs found".to_string()));
            }

            // Check if per-key RGB is supported
            if !keyboard.supports_per_key_rgb() {
                println!("Warning: Per-key RGB not supported on this keyboard");
                println!("Falling back to setting first color on entire keyboard");
                keyboard.set_color(key_colors[0].1)?;
                keyboard.apply()?;
                return Ok(());
            }

            println!(
                "Setting {} keys to their respective colors",
                key_colors.len()
            );
            keyboard.set_key_colors(&key_colors)?;
            keyboard.apply()?;
            println!("Done!");
        }

        PerKeyAction::SetRegion {
            start_row,
            start_col,
            rows,
            cols,
            color,
        } => {
            let color = parse_color(&color)
                .ok_or_else(|| Error::Other(format!("Invalid color: {}", color)))?;

            println!(
                "Setting region ({},{}) to ({},{}) to color {}",
                start_row,
                start_col,
                start_row + rows - 1,
                start_col + cols - 1,
                color
            );

            keyboard.set_key_region(start_row, start_col, rows, cols, color)?;
            keyboard.apply()?;
            println!("Done!");
        }

        PerKeyAction::Clear => {
            println!("Clearing all per-key RGB (setting all keys to black)");
            keyboard.clear_per_key_rgb()?;
            keyboard.apply()?;
            println!("Done!");
        }

        PerKeyAction::TestMatrix { row, col, color } => {
            let color = parse_color(&color)
                .ok_or_else(|| Error::Other(format!("Invalid color: {}", color)))?;

            println!(
                "Testing matrix position ({}, {}) with color {}",
                row, col, color
            );
            let address = KeyAddress::new(row, col);
            keyboard.set_key_color_direct(address, color)?;
            keyboard.apply()?;
            println!("Done! If no key lights up, this matrix position might not exist.");
        }

        PerKeyAction::TestPattern { pattern } => {
            let pattern_name = pattern.to_ascii_lowercase();
            match pattern_name.as_str() {
                "rainbow" => {
                    println!("Testing rainbow pattern across keyboard");
                    test_rainbow_pattern(keyboard)?;
                }
                "checkerboard" => {
                    println!("Testing checkerboard pattern");
                    test_checkerboard_pattern(keyboard)?;
                }
                "wave" => {
                    println!("Testing wave pattern");
                    test_wave_pattern(keyboard)?;
                }
                "test" => {
                    println!("Testing basic key positions");
                    test_basic_positions(keyboard)?;
                }
                _ => {
                    return Err(Error::Other(format!(
                        "Unknown pattern: {}. Available: rainbow, checkerboard, wave, test",
                        pattern
                    )));
                }
            }
            keyboard.apply()?;
            println!("Pattern applied!");
        }

        PerKeyAction::ShowMapping => {
            if let Some(mapping) = keyboard.get_key_mapping() {
                println!("Key mapping for {}:", mapping.name);
                println!("Layout: {:?}", mapping.layout);
                println!(
                    "Matrix dimensions: {}x{}",
                    mapping.matrix_rows, mapping.matrix_cols
                );

                let stats = mapping.get_stats();
                println!("Statistics: {}", stats);

                // Show some sample key mappings
                let sample_keys = [
                    KeyId::A,
                    KeyId::S,
                    KeyId::D,
                    KeyId::Enter,
                    KeyId::Space,
                    KeyId::Escape,
                ];
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
            println!("Supported: {}", keyboard.supports_per_key_rgb());

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
fn test_rainbow_pattern(keyboard: &mut Box<dyn Keyboard>) -> Result<()> {
    let colors = [
        Color::RED,
        Color::ORANGE,
        Color::YELLOW,
        Color::GREEN,
        Color::CYAN,
        Color::BLUE,
        Color::PURPLE,
        Color::MAGENTA,
    ];

    if keyboard.supports_per_key_rgb() {
        // Use logical key mapping if available
        if let Some(mapping) = keyboard.get_key_mapping() {
            let all_keys = mapping.get_all_keys();
            let mut key_colors = Vec::new();

            for (i, key_id) in all_keys.iter().enumerate() {
                let color = colors[i % colors.len()];
                key_colors.push((*key_id, color));
            }

            keyboard.set_key_colors(&key_colors)?;
        } else {
            // Fallback to matrix addressing
            let mut direct_colors = Vec::new();
            for row in 0..6 {
                for col in 0..17 {
                    let color_idx = (row * 17 + col) % colors.len() as u8;
                    direct_colors.push((KeyAddress::new(row, col), colors[color_idx as usize]));
                }
            }
            keyboard.set_key_colors_direct(&direct_colors)?;
        }
    } else {
        // Fallback to zone-based rainbow
        let zone_colors: Vec<Color> = (0..keyboard.zone_count())
            .map(|i| colors[i % colors.len()])
            .collect();
        keyboard.set_zone_colors(&zone_colors)?;
    }

    Ok(())
}

fn test_checkerboard_pattern(keyboard: &mut Box<dyn Keyboard>) -> Result<()> {
    let mut direct_colors = Vec::new();

    for row in 0..6 {
        for col in 0..17 {
            let color = if (row + col) % 2 == 0 {
                Color::WHITE
            } else {
                Color::BLACK
            };
            direct_colors.push((KeyAddress::new(row, col), color));
        }
    }

    if keyboard.supports_per_key_rgb() {
        keyboard.set_key_colors_direct(&direct_colors)?;
    } else {
        println!("Checkerboard pattern requires per-key RGB support");
        keyboard.set_color(Color::WHITE)?;
    }

    Ok(())
}

fn test_wave_pattern(keyboard: &mut Box<dyn Keyboard>) -> Result<()> {
    let mut direct_colors = Vec::new();

    for row in 0..6 {
        for col in 0..17 {
            let wave_pos = (col as f32 / 17.0) * 2.0 * std::f32::consts::PI;
            let intensity = ((wave_pos.sin() + 1.0) / 2.0 * 255.0) as u8;
            let color = Color::new(intensity, 0, 255 - intensity);
            direct_colors.push((KeyAddress::new(row, col), color));
        }
    }

    if keyboard.supports_per_key_rgb() {
        keyboard.set_key_colors_direct(&direct_colors)?;
    } else {
        println!("Wave pattern requires per-key RGB support");
        keyboard.set_color(Color::PURPLE)?;
    }

    Ok(())
}

fn test_basic_positions(keyboard: &mut Box<dyn Keyboard>) -> Result<()> {
    // Test corners and center positions
    let test_positions = [
        (KeyAddress::new(0, 0), Color::RED),     // Top-left
        (KeyAddress::new(0, 16), Color::GREEN),  // Top-right
        (KeyAddress::new(5, 0), Color::BLUE),    // Bottom-left
        (KeyAddress::new(5, 16), Color::YELLOW), // Bottom-right
        (KeyAddress::new(2, 8), Color::WHITE),   // Center
    ];

    if keyboard.supports_per_key_rgb() {
        keyboard.set_key_colors_direct(&test_positions)?;
    } else {
        println!("Basic position test requires per-key RGB support");
        keyboard.set_color(Color::WHITE)?;
    }

    Ok(())
}

fn parse_key_name(name: &str) -> Option<KeyId> {
    let name_upper = name.to_ascii_uppercase();
    match name_upper.as_str() {
        // Letters
        "A" => Some(KeyId::A),
        "B" => Some(KeyId::B),
        "C" => Some(KeyId::C),
        "D" => Some(KeyId::D),
        "E" => Some(KeyId::E),
        "F" => Some(KeyId::F),
        "G" => Some(KeyId::G),
        "H" => Some(KeyId::H),
        "I" => Some(KeyId::I),
        "J" => Some(KeyId::J),
        "K" => Some(KeyId::K),
        "L" => Some(KeyId::L),
        "M" => Some(KeyId::M),
        "N" => Some(KeyId::N),
        "O" => Some(KeyId::O),
        "P" => Some(KeyId::P),
        "Q" => Some(KeyId::Q),
        "R" => Some(KeyId::R),
        "S" => Some(KeyId::S),
        "T" => Some(KeyId::T),
        "U" => Some(KeyId::U),
        "V" => Some(KeyId::V),
        "W" => Some(KeyId::W),
        "X" => Some(KeyId::X),
        "Y" => Some(KeyId::Y),
        "Z" => Some(KeyId::Z),

        // Numbers
        "1" => Some(KeyId::Key1),
        "2" => Some(KeyId::Key2),
        "3" => Some(KeyId::Key3),
        "4" => Some(KeyId::Key4),
        "5" => Some(KeyId::Key5),
        "6" => Some(KeyId::Key6),
        "7" => Some(KeyId::Key7),
        "8" => Some(KeyId::Key8),
        "9" => Some(KeyId::Key9),
        "0" => Some(KeyId::Key0),

        // Function keys
        "F1" => Some(KeyId::F1),
        "F2" => Some(KeyId::F2),
        "F3" => Some(KeyId::F3),
        "F4" => Some(KeyId::F4),
        "F5" => Some(KeyId::F5),
        "F6" => Some(KeyId::F6),
        "F7" => Some(KeyId::F7),
        "F8" => Some(KeyId::F8),
        "F9" => Some(KeyId::F9),
        "F10" => Some(KeyId::F10),
        "F11" => Some(KeyId::F11),
        "F12" => Some(KeyId::F12),

        // Special keys
        "ENTER" | "RETURN" => Some(KeyId::Enter),
        "SPACE" | "SPACEBAR" => Some(KeyId::Space),
        "ESC" | "ESCAPE" => Some(KeyId::Escape),
        "TAB" => Some(KeyId::Tab),
        "SHIFT" => Some(KeyId::LeftShift),
        "CTRL" => Some(KeyId::LeftCtrl),
        "ALT" => Some(KeyId::LeftAlt),
        "WIN" | "WINDOWS" => Some(KeyId::LeftWin),
        "CAPS" | "CAPSLOCK" => Some(KeyId::CapsLock),
        "BACKSPACE" => Some(KeyId::Backspace),

        // Arrows
        "UP" | "ARROWUP" => Some(KeyId::ArrowUp),
        "DOWN" | "ARROWDOWN" => Some(KeyId::ArrowDown),
        "LEFT" | "ARROWLEFT" => Some(KeyId::ArrowLeft),
        "RIGHT" | "ARROWRIGHT" => Some(KeyId::ArrowRight),

        // Punctuation
        ";" | "SEMICOLON" => Some(KeyId::Semicolon),
        "'" | "QUOTE" => Some(KeyId::Quote),
        "," | "COMMA" => Some(KeyId::Comma),
        "." | "PERIOD" => Some(KeyId::Period),
        "/" | "SLASH" => Some(KeyId::Slash),
        "[" | "LEFTBRACKET" => Some(KeyId::LeftBracket),
        "]" | "RIGHTBRACKET" => Some(KeyId::RightBracket),
        "\\" | "BACKSLASH" => Some(KeyId::Backslash),
        "-" | "MINUS" => Some(KeyId::Minus),
        "=" | "EQUAL" => Some(KeyId::Equal),
        "`" | "BACKTICK" => Some(KeyId::Backtick),

        _ => None,
    }
}

fn cmd_profile(action: ProfileAction) -> Result<()> {
    let mut profile_manager = ProfileManager::new()?;

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

                let device_manager = DeviceManager::new()?;
                let state_store = DeviceStateStore::new()?;

                // Apply keyboard settings if present
                if let Some(ref keyboard_profile) = profile.keyboard {
                    if let Some(keyboard_info) =
                        device_manager.first_device_of_type(DeviceType::Keyboard)
                    {
                        let mut keyboard = device_manager.open_keyboard(keyboard_info)?;
                        let device_id = DeviceId::from(keyboard_info);

                        // Apply brightness
                        keyboard.set_brightness(keyboard_profile.brightness)?;

                        // Apply effect - for static effects, apply immediately
                        match &keyboard_profile.effect {
                            Effect::Static { color } => {
                                keyboard.set_color(*color)?;
                            }
                            Effect::Off => {
                                keyboard.set_color(Color::BLACK)?;
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
            let state_store = DeviceStateStore::new()?;
            let device_manager = DeviceManager::new()?;

            // Capture keyboard settings from state store
            if let Some(keyboard_info) = device_manager.first_device_of_type(DeviceType::Keyboard) {
                let device_id = DeviceId::from(keyboard_info);
                if let Some(device_state) = state_store.get(&device_id) {
                    if let Some(ref keyboard_state) = device_state.keyboard {
                        profile.keyboard = Some(KeyboardProfile {
                            effect: keyboard_state.effect.clone(),
                            brightness: keyboard_state.brightness,
                        });
                        println!("Captured keyboard settings");
                    }
                }
            }

            profile_manager.set(profile)?;
            println!("Profile saved: {}", name);
        }

        ProfileAction::Delete { name } => {
            profile_manager.delete(&name)?;
            println!("Profile deleted: {}", name);
        }
    }

    Ok(())
}

fn cmd_pollrate(action: PollrateAction) -> Result<()> {
    use steelseries_gg::pollrate::{DeviceType, PollRate, get_poll_rate, set_poll_rate};

    match action {
        PollrateAction::Mouse { rate, persistent } => {
            let poll_rate = PollRate::from_hz(rate)?;

            // Warn about hardware requirements for high poll rates
            if poll_rate.requires_hardware_support() {
                println!(
                    "⚠ Warning: {} requires hardware support",
                    poll_rate.description()
                );
                println!("  Your mouse may not support this rate or may ignore the setting.");
                println!();
            }

            set_poll_rate(DeviceType::Mouse, poll_rate)?;
            println!("Mouse polling rate set to {} Hz", rate);

            if persistent {
                let mut config = Config::load()?;
                config.poll_rate.mouse_hz = Some(rate);
                config.save()?;
                println!("Setting saved to config (will apply on daemon startup)");
            }
        }

        PollrateAction::Keyboard { rate, persistent } => {
            let poll_rate = PollRate::from_hz(rate)?;

            // Warn about hardware requirements for high poll rates
            if poll_rate.requires_hardware_support() {
                println!(
                    "⚠ Warning: {} requires hardware support",
                    poll_rate.description()
                );
                println!("  Your keyboard may not support this rate or may ignore the setting.");
                println!();
            }

            set_poll_rate(DeviceType::Keyboard, poll_rate)?;
            println!("Keyboard polling rate set to {} Hz", rate);

            if persistent {
                let mut config = Config::load()?;
                config.poll_rate.keyboard_hz = Some(rate);
                config.save()?;
                println!("Setting saved to config (will apply on daemon startup)");
            }
        }

        PollrateAction::Status => {
            println!("Current USB Polling Rates:");
            println!();

            match get_poll_rate(DeviceType::Mouse) {
                Ok(rate) => println!("  Mouse:    {} Hz", rate.to_hz()),
                Err(e) => println!("  Mouse:    Error: {}", e),
            }

            match get_poll_rate(DeviceType::Keyboard) {
                Ok(rate) => println!("  Keyboard: {} Hz", rate.to_hz()),
                Err(e) => println!("  Keyboard: Error: {}", e),
            }

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
            let ch = parse_channel(&channel)
                .ok_or_else(|| Error::Other(format!("Invalid channel: {}", channel)))?;

            let volume = normalize_volume(level);
            mixer.set_volume(ch, volume)?;
            println!("{} volume set to {}%", ch, level.min(100));
        }

        AudioAction::Mute { channel, state } => {
            let ch = parse_channel(&channel)
                .ok_or_else(|| Error::Other(format!("Invalid channel: {}", channel)))?;

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

fn cmd_validate(
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

    let devices = manager.devices();
    let keyboards: Vec<_> = devices
        .into_iter()
        .filter(|info| info.device_type == DeviceType::Keyboard)
        .collect();

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
                let report = validator.validate_keyboard(&mut *keyboard);

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
                    println!(
                        "   🎹 Per-key RGB: Supported ({} keys)",
                        report.capabilities.key_count
                    );
                } else {
                    println!("   🌈 Zone RGB: {} zones", report.capabilities.zone_count);
                }

                if benchmark {
                    println!(
                        "   🚀 Performance: {:.1}ms avg, {:.0} fps effective",
                        report.performance.avg_effect_compute_ms
                            + report.performance.avg_hid_communication_ms,
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
            serde_json::to_string_pretty(&all_reports).map_err(|e| {
                Error::DeviceCommunication(format!("JSON serialization failed: {}", e))
            })?
        } else {
            // Export as human-readable text
            let mut content = String::new();
            content.push_str("# SteelSeries RGB Validation Report\n\n");
            content.push_str(&format!("Generated: {}\n", chrono::Utc::now().to_rfc3339()));
            content.push_str(&format!("Total devices: {}\n\n", all_reports.len()));

            for (i, report) in all_reports.iter().enumerate() {
                content.push_str(&format!(
                    "## Device {} - {}\n",
                    i + 1,
                    report.device_info.name
                ));
                content.push_str(&format!(
                    "- Product ID: 0x{:04x}\n",
                    report.device_info.product_id
                ));
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
                    content.push_str(&format!(
                        "- {} {} ({:.0}ms)\n",
                        status, result.name, result.duration_ms
                    ));

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

        std::fs::write(&output_path, export_content)
            .map_err(|e| Error::DeviceCommunication(format!("Failed to write report: {}", e)))?;

        println!("\n📄 Validation report exported to: {}", output_path);
    }

    // Overall summary
    let total_healthy = all_reports.iter().filter(|r| r.is_healthy()).count();
    println!("\n📊 Validation Summary:");
    println!("   Total devices: {}", all_reports.len());
    println!(
        "   Healthy devices: {}/{}",
        total_healthy,
        all_reports.len()
    );

    if total_healthy == all_reports.len() {
        println!("   🎉 All devices passed validation!");
    } else {
        println!("   ⚠️  Some devices have issues - check details above");
    }

    Ok(())
}

fn cmd_performance(manager: &DeviceManager, action: PerformanceAction) -> Result<()> {
    let keyboards: Vec<_> = manager
        .devices()
        .into_iter()
        .filter(|info| info.device_type == DeviceType::Keyboard)
        .collect();

    if keyboards.is_empty() {
        println!("⚠️  No SteelSeries keyboards found for performance monitoring");
        return Ok(());
    }

    match action {
        PerformanceAction::Stats {
            monitor,
            output,
            json,
        } => {
            if let Some(interval_seconds) = monitor {
                println!("🔄 Monitoring RGB performance (press Ctrl+C to stop)...");
                loop {
                    display_performance_stats(manager, &keyboards, json)?;

                    if let Some(ref output_path) = output {
                        export_performance_stats(manager, &keyboards, output_path, json)?;
                    }

                    std::thread::sleep(Duration::from_secs(interval_seconds));
                }
            } else {
                display_performance_stats(manager, &keyboards, json)?;

                if let Some(output_path) = output {
                    export_performance_stats(manager, &keyboards, &output_path, json)?;
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
                        println!(
                            "   ✅ {} - Performance optimizations enabled",
                            device_info.name
                        );
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
                        println!(
                            "   ✅ {} - Performance optimizations disabled",
                            device_info.name
                        );
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

            println!(
                "✅ Cleaned caches on {}/{} keyboards",
                cleaned_count,
                keyboards.len()
            );
        }

        PerformanceAction::Benchmark { duration, output } => {
            println!(
                "🏃 Running performance benchmark for {} seconds...",
                duration
            );

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

                        // Run rapid RGB operations for benchmark duration
                        while device_start.elapsed().as_secs() < duration {
                            let colors = vec![
                                Color::RED,
                                Color::GREEN,
                                Color::BLUE,
                                Color::WHITE,
                                Color::BLACK,
                            ];

                            for color in colors {
                                let _ = keyboard.set_color(color);
                                operation_count += 1;

                                // Small delay to prevent overwhelming the device
                                std::thread::sleep(Duration::from_millis(1));
                            }
                        }

                        let ops_per_second = operation_count as f64 / duration as f64;
                        benchmark_results.insert(device_info.name.clone(), ops_per_second);

                        println!("      Operations/second: {:.1}", ops_per_second);

                        // Display performance stats if available
                        if let Some(stats) = keyboard.get_rgb_performance_stats() {
                            println!("      Cache hit rate: {:.1}%", stats.cache_hit_rate * 100.0);
                            println!(
                                "      Avg computation: {:.2}μs",
                                stats.avg_computation_time_us
                            );
                            println!(
                                "      Current refresh rate: {:.1} Hz",
                                stats.current_refresh_rate
                            );
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

                std::fs::write(&output_path, serde_json::to_string_pretty(&benchmark_data)?)
                    .map_err(|e| {
                        Error::DeviceCommunication(format!("Failed to write benchmark: {}", e))
                    })?;

                println!("📄 Benchmark results exported to: {}", output_path);
            }

            println!(
                "✅ Benchmark completed in {:.1}s",
                start_time.elapsed().as_secs_f64()
            );
        }
    }

    Ok(())
}

fn display_performance_stats(
    manager: &DeviceManager,
    keyboards: &[&DeviceInfo],
    json: bool,
) -> Result<()> {
    if json {
        let mut all_stats = HashMap::new();

        for device_info in keyboards {
            if let Ok(keyboard) = manager.open_keyboard(device_info) {
                if let Some(stats) = keyboard.get_rgb_performance_stats() {
                    all_stats.insert(device_info.name.clone(), stats.clone());
                }
            }
        }

        println!("{}", serde_json::to_string_pretty(&all_stats)?);
    } else {
        println!("📊 RGB Performance Statistics:");
        println!();

        for device_info in keyboards {
            match manager.open_keyboard(device_info) {
                Ok(keyboard) => {
                    println!("🎹 {}", device_info.name);

                    if let Some(stats) = keyboard.get_rgb_performance_stats() {
                        println!("   💾 Cache hit rate: {:.1}%", stats.cache_hit_rate * 100.0);
                        println!(
                            "   ⚡ Avg computation: {:.2}μs",
                            stats.avg_computation_time_us
                        );
                        println!(
                            "   🔄 Current refresh rate: {:.1} Hz",
                            stats.current_refresh_rate
                        );
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

fn export_performance_stats(
    manager: &DeviceManager,
    keyboards: &[&DeviceInfo],
    output_path: &str,
    json: bool,
) -> Result<()> {
    if json {
        let mut all_stats = HashMap::new();

        for device_info in keyboards {
            if let Ok(keyboard) = manager.open_keyboard(device_info) {
                if let Some(stats) = keyboard.get_rgb_performance_stats() {
                    all_stats.insert(device_info.name.clone(), stats.clone());
                }
            }
        }

        let export_data = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "devices": all_stats
        });

        std::fs::write(output_path, serde_json::to_string_pretty(&export_data)?)
            .map_err(|e| Error::DeviceCommunication(format!("Failed to write stats: {}", e)))?;
    } else {
        let mut content = String::new();
        content.push_str("# RGB Performance Statistics Report\n\n");
        content.push_str(&format!(
            "Generated: {}\n\n",
            chrono::Utc::now().to_rfc3339()
        ));

        for device_info in keyboards {
            if let Ok(keyboard) = manager.open_keyboard(device_info) {
                content.push_str(&format!("## {}\n", device_info.name));
                content.push_str(&format!("- Product ID: 0x{:04x}\n", device_info.product_id));

                if let Some(stats) = keyboard.get_rgb_performance_stats() {
                    content.push_str(&format!(
                        "- Cache hit rate: {:.1}%\n",
                        stats.cache_hit_rate * 100.0
                    ));
                    content.push_str(&format!(
                        "- Avg computation time: {:.2}μs\n",
                        stats.avg_computation_time_us
                    ));
                    content.push_str(&format!(
                        "- Current refresh rate: {:.1} Hz\n",
                        stats.current_refresh_rate
                    ));
                    content.push_str(&format!(
                        "- Total computations: {}\n",
                        stats.total_computations
                    ));
                    content.push_str(&format!(
                        "- Operations batched: {}\n",
                        stats.hid_operations_batched
                    ));
                    content.push_str(&format!(
                        "- Allocations saved: {}\n",
                        stats.allocations_saved
                    ));
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

        std::fs::write(output_path, content)
            .map_err(|e| Error::DeviceCommunication(format!("Failed to write stats: {}", e)))?;
    }

    Ok(())
}

async fn cmd_server(port: u16) -> Result<()> {
    info!("Starting GameSense server on port {}", port);

    let server = GameSenseServer::new("127.0.0.1", port)?;
    server.run().await?;

    Ok(())
}

/// Daemon state for managing connected devices and RGB controllers.
struct DaemonState {
    keyboards: HashMap<String, (Box<dyn Keyboard>, RgbController, DeviceInfo)>,
    headsets: HashMap<String, (Box<dyn Headset>, DeviceInfo)>,
    gamesense_overlays: HashMap<String, (Color, std::time::Instant)>, // zone -> (color, expiry)
    /// Device fingerprints for tracking devices across reconnections
    device_fingerprints: HashMap<String, DeviceFingerprint>,
    /// Profile manager for applying settings to reconnected devices
    profile_manager: Option<ProfileManager>,
    /// State store for persisting device states
    state_store: DeviceStateStore,
}

impl DaemonState {
    fn new() -> Result<Self> {
        let state_store = DeviceStateStore::new()?;
        let profile_manager = ProfileManager::new().ok(); // Optional - don't fail if profiles unavailable

        Ok(Self {
            keyboards: HashMap::new(),
            headsets: HashMap::new(),
            gamesense_overlays: HashMap::new(),
            device_fingerprints: HashMap::new(),
            profile_manager,
            state_store,
        })
    }

    /// Handle device addition event
    async fn handle_device_added(
        &mut self,
        device_manager: &DeviceManager,
        fingerprint: &DeviceFingerprint,
        info: &DeviceInfo,
    ) -> Result<()> {
        let serial = info
            .serial_number
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        match info.device_type {
            DeviceType::Keyboard => {
                info!(
                    "Hot-plug: Adding keyboard: {} ({})",
                    info.name,
                    fingerprint.to_id()
                );

                // Open the keyboard device
                match device_manager.open_keyboard(info) {
                    Ok(mut keyboard) => {
                        // Initialize the device
                        if let Err(e) = keyboard.initialize() {
                            warn!("Failed to initialize keyboard {}: {}", info.name, e);
                            return Err(e);
                        }

                        let zone_count = keyboard.zone_count();
                        let mut rgb_controller = RgbController::new(zone_count);

                        // Try to restore state from state store
                        let device_id = DeviceId::from(info);
                        if let Some(device_state) = self.state_store.get(&device_id) {
                            if let Some(ref keyboard_state) = device_state.keyboard {
                                rgb_controller.set_effect(keyboard_state.effect.clone());
                                rgb_controller
                                    .set_brightness(keyboard_state.brightness as f32 / 100.0);

                                // Apply the restored effect if it's static
                                match &keyboard_state.effect {
                                    Effect::Static { color } => {
                                        if let Err(e) = keyboard.set_color(*color) {
                                            warn!(
                                                "Failed to apply restored color to {}: {}",
                                                info.name, e
                                            );
                                        }
                                    }
                                    Effect::Off => {
                                        if let Err(e) = keyboard.set_color(Color::BLACK) {
                                            warn!("Failed to turn off {}: {}", info.name, e);
                                        }
                                    }
                                    _ => {
                                        info!(
                                            "Restored animated effect for {} (will be applied by animation loop)",
                                            info.name
                                        );
                                    }
                                }

                                info!(
                                    "Restored state for {}: brightness={}%, effect={:?}",
                                    info.name, keyboard_state.brightness, keyboard_state.effect
                                );
                            }
                        } else if let Some(ref profile_manager) = self.profile_manager {
                            // Try to apply default profile
                            if let Some(default_profile) = profile_manager.get("default") {
                                if let Some(ref keyboard_profile) = default_profile.keyboard {
                                    rgb_controller.set_effect(keyboard_profile.effect.clone());
                                    rgb_controller
                                        .set_brightness(keyboard_profile.brightness as f32 / 100.0);

                                    match &keyboard_profile.effect {
                                        Effect::Static { color } => {
                                            if let Err(e) = keyboard.set_color(*color) {
                                                warn!(
                                                    "Failed to apply default profile color to {}: {}",
                                                    info.name, e
                                                );
                                            }
                                        }
                                        Effect::Off => {
                                            if let Err(e) = keyboard.set_color(Color::BLACK) {
                                                warn!(
                                                    "Failed to turn off {} (default profile): {}",
                                                    info.name, e
                                                );
                                            }
                                        }
                                        _ => {
                                            info!(
                                                "Applied default animated profile for {} (will be handled by animation loop)",
                                                info.name
                                            );
                                        }
                                    }

                                    info!(
                                        "Applied default profile to {}: brightness={}%, effect={:?}",
                                        info.name,
                                        keyboard_profile.brightness,
                                        keyboard_profile.effect
                                    );
                                }
                            }
                        }

                        // Store device information
                        self.keyboards
                            .insert(serial.clone(), (keyboard, rgb_controller, info.clone()));
                        self.device_fingerprints.insert(serial, fingerprint.clone());

                        info!(
                            "Successfully added keyboard: {} (zones: {})",
                            info.name, zone_count
                        );
                    }
                    Err(e) => {
                        warn!("Failed to open keyboard {}: {}", info.name, e);
                        return Err(e);
                    }
                }
            }
            DeviceType::Headset => {
                info!(
                    "Hot-plug: Adding headset: {} ({})",
                    info.name,
                    fingerprint.to_id()
                );

                match device_manager.open_headset(info) {
                    Ok(mut headset) => {
                        // Initialize the device
                        if let Err(e) = headset.initialize() {
                            warn!("Failed to initialize headset {}: {}", info.name, e);
                            return Err(e);
                        }

                        // Store device information
                        self.headsets.insert(serial.clone(), (headset, info.clone()));
                        self.device_fingerprints.insert(serial, fingerprint.clone());

                        info!("Successfully added headset: {}", info.name);
                    }
                    Err(e) => {
                        warn!("Failed to open headset {}: {}", info.name, e);
                        return Err(e);
                    }
                }
            }
            DeviceType::Unknown => {
                debug!("Hot-plug: Ignoring unknown device: {}", info.name);
            }
        }

        Ok(())
    }

    /// Handle device removal event
    async fn handle_device_removed(
        &mut self,
        fingerprint: &DeviceFingerprint,
        last_seen: std::time::Instant,
    ) -> Result<()> {
        // Find which device was removed by matching fingerprint
        let mut device_to_remove: Option<String> = None;

        for (serial, stored_fingerprint) in &self.device_fingerprints {
            if stored_fingerprint == fingerprint {
                device_to_remove = Some(serial.clone());
                break;
            }
        }

        if let Some(serial) = device_to_remove {
            let mut removed = false;

            if let Some((_keyboard, rgb_controller, info)) = self.keyboards.remove(&serial) {
                info!(
                    "Hot-plug: Removing keyboard: {} ({})",
                    info.name,
                    fingerprint.to_id()
                );

                // Save final state before removal
                let device_id = DeviceId::from(&info);
                let final_state = KeyboardState {
                    effect: rgb_controller.effect().clone(),
                    brightness: (rgb_controller.brightness() * 100.0) as u8,
                };

                if let Err(e) = self.state_store.update_keyboard(device_id, final_state) {
                    warn!("Failed to save final state for {}: {}", info.name, e);
                } else {
                    debug!("Saved final state for {}", info.name);
                }

                removed = true;
                let elapsed = std::time::Instant::now().duration_since(last_seen);
                info!(
                    "Successfully removed keyboard: {} (was connected for {:.1}s)",
                    info.name,
                    elapsed.as_secs_f64()
                );
            }

            if !removed {
                if let Some((_headset, info)) = self.headsets.remove(&serial) {
                    info!(
                        "Hot-plug: Removing headset: {} ({})",
                        info.name,
                        fingerprint.to_id()
                    );

                    removed = true;
                    let elapsed = std::time::Instant::now().duration_since(last_seen);
                    info!(
                        "Successfully removed headset: {} (was connected for {:.1}s)",
                        info.name,
                        elapsed.as_secs_f64()
                    );
                }
            }

            if removed {
                self.device_fingerprints.remove(&serial);
            } else {
                debug!(
                    "Hot-plug: Device {} was already removed or not found",
                    fingerprint.to_id()
                );
            }
        } else {
            debug!(
                "Hot-plug: Could not find device to remove: {}",
                fingerprint.to_id()
            );
        }

        Ok(())
    }

    /// Get current device count for monitoring
    fn device_count(&self) -> usize {
        self.keyboards.len() + self.headsets.len()
    }

    /// Get list of connected device names
    fn device_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .keyboards
            .values()
            .map(|(_, _, info)| info.name.clone())
            .collect();

        names.extend(self.headsets.values().map(|(_, info)| info.name.clone()));
        names
    }
}

async fn cmd_daemon(mut manager: DeviceManager) -> Result<()> {
    info!("Starting SteelSeries GG daemon");

    let config = Config::load()?;

    // Apply saved polling rates
    {
        use steelseries_gg::pollrate::{DeviceType, PollRate, set_poll_rate};

        if let Some(mouse_hz) = config.poll_rate.mouse_hz {
            if let Ok(rate) = PollRate::from_hz(mouse_hz) {
                match set_poll_rate(DeviceType::Mouse, rate) {
                    Ok(()) => info!("Applied mouse poll rate: {} Hz", mouse_hz),
                    Err(e) => tracing::warn!("Failed to set mouse poll rate: {}", e),
                }
            }
        }

        if let Some(keyboard_hz) = config.poll_rate.keyboard_hz {
            if let Ok(rate) = PollRate::from_hz(keyboard_hz) {
                match set_poll_rate(DeviceType::Keyboard, rate) {
                    Ok(()) => info!("Applied keyboard poll rate: {} Hz", keyboard_hz),
                    Err(e) => tracing::warn!("Failed to set keyboard poll rate: {}", e),
                }
            }
        }
    }

    // Initialize daemon state
    let daemon_state = Arc::new(RwLock::new(DaemonState::new()?));

    // Set up hot-plug monitoring
    let hotplug_daemon_state = daemon_state.clone();
    manager.set_hotplug_callback(move |event| {
        let state_clone = hotplug_daemon_state.clone();
        tokio::spawn(async move {
            if let Ok(mut state) = state_clone.try_write() {
                // Handle hot-plug events without blocking
                match event {
                    HotPlugEvent::DeviceAdded {
                        fingerprint,
                        info,
                        timestamp,
                    } => {
                        info!(
                            "Hot-plug event: Device added at {:.3}s: {} ({})",
                            timestamp.elapsed().as_secs_f64(),
                            info.name,
                            fingerprint.to_id()
                        );

                        // Note: We can't access the DeviceManager here, so we'll log the event
                        // and let the periodic refresh handle actual device initialization
                        debug!("Deferring device initialization to refresh cycle");
                    }
                    HotPlugEvent::DeviceRemoved {
                        fingerprint,
                        last_seen,
                        timestamp,
                    } => {
                        info!(
                            "Hot-plug event: Device removed at {:.3}s: {} (last seen {:.3}s ago)",
                            timestamp.elapsed().as_secs_f64(),
                            fingerprint.to_id(),
                            timestamp.duration_since(last_seen).as_secs_f64()
                        );

                        if let Err(e) = state.handle_device_removed(&fingerprint, last_seen).await {
                            warn!("Failed to handle device removal: {}", e);
                        }
                    }
                }
            } else {
                debug!("Hot-plug event received but daemon state is busy, skipping");
            }
        });
    });

    // Start hot-plug monitoring
    let hotplug_stop_tx = manager.start_hotplug_monitoring().await?;

    // Discover and open devices initially
    let devices_info = manager.devices();
    if devices_info.is_empty() {
        info!("No SteelSeries devices found initially");
    } else {
        let mut state = daemon_state.write().await;
        for info in devices_info {
            let fingerprint = DeviceFingerprint::from_device_info(info);
            if let Err(e) = state
                .handle_device_added(&manager, &fingerprint, info)
                .await
            {
                warn!("Failed to add initial device {}: {}", info.name, e);
            }
        }
        let device_count = state.device_count();
        let device_names = state.device_names();
        info!(
            "Initialized {} device(s): {}",
            device_count,
            device_names.join(", ")
        );
    }

    // Start GameSense server in background if enabled
    if config.gamesense.enabled {
        let gs_bind = config.gamesense.bind_address.clone();
        let gs_port = config.gamesense.port;
        let daemon_state_clone = daemon_state.clone();

        tokio::spawn(async move {
            match GameSenseServer::new(&gs_bind, gs_port) {
                Ok(server) => {
                    // Set RGB callback to update overlays with optimized async handling
                    server
                        .set_rgb_callback(move |zone: &str, r: u8, g: u8, b: u8| {
                            let state = &daemon_state_clone; // Use reference instead of double-cloning
                            let zone_owned = zone.to_string();

                            // Use a more efficient approach - avoid spawning tasks for simple operations
                            let color = Color::new(r, g, b);
                            let expiry = std::time::Instant::now() + Duration::from_secs(30);

                            // Use try_write to avoid blocking if the lock is busy
                            if let Ok(mut state_guard) = state.try_write() {
                                state_guard
                                    .gamesense_overlays
                                    .insert(zone_owned.clone(), (color, expiry));
                                tracing::debug!("GameSense overlay: {} = {:?}", zone_owned, color);
                            } else {
                                // If we can't get the lock immediately, spawn a task
                                let state_clone = daemon_state_clone.clone();
                                let zone_deferred = zone_owned.clone();
                                tokio::spawn(async move {
                                    let mut state = state_clone.write().await;
                                    state
                                        .gamesense_overlays
                                        .insert(zone_deferred.clone(), (color, expiry));
                                    tracing::debug!(
                                        "GameSense overlay (deferred): {} = {:?}",
                                        zone_deferred,
                                        color
                                    );
                                });
                            }
                        })
                        .await;

                    if let Err(e) = server.run().await {
                        tracing::error!("GameSense server error: {}", e);
                    }
                }
                Err(e) => tracing::error!("Failed to create GameSense server: {}", e),
            }
        });

        info!(
            "GameSense server started on {}:{}",
            config.gamesense.bind_address, config.gamesense.port
        );
    } else {
        info!("GameSense server disabled in config");
    }

    // Use provided device manager
    print_device_summary(&manager);

    // Load default profile if configured
    if let Some(ref profile_name) = config.default_profile {
        if let Ok(profile_manager) = ProfileManager::new() {
            if let Some(profile) = profile_manager.get(profile_name) {
                info!("Loading default profile: {}", profile.name);

                // Apply keyboard settings if present
                if let Some(ref keyboard_profile) = profile.keyboard {
                    let keyboard_state = KeyboardState {
                        effect: keyboard_profile.effect.clone(),
                        brightness: keyboard_profile.brightness,
                    };

                    // Collect device info first to avoid borrow conflicts
                    let device_infos: Vec<DeviceInfo> = {
                        let mut state = daemon_state.write().await;
                        let mut infos = Vec::new();

                        for (_keyboard, controller, info) in state.keyboards.values_mut() {
                            controller.set_effect(keyboard_profile.effect.clone());
                            controller.set_brightness(keyboard_profile.brightness as f32 / 100.0);
                            infos.push(info.clone());
                            info!("Applied profile to keyboard: {}", info.name);
                        }

                        infos
                    };

                    // Update state store separately
                    {
                        let state = daemon_state.read().await;
                        for info in device_infos {
                            let device_id = DeviceId::from(&info);
                            let _ = state
                                .state_store
                                .update_keyboard(device_id, keyboard_state.clone());
                        }
                    }
                }
            }
        }
    }

    info!("Daemon running. Press Ctrl+C to stop.");

    // Spawn RGB animation loop with adaptive timing and performance monitoring
    let daemon_state_anim = daemon_state.clone();
    let animation_task = tokio::spawn(async move {
        use steelseries_gg::performance::{
            PerformanceMonitor, calculate_effect_complexity, estimate_memory_usage,
        };

        // Initialize performance monitor for adaptive timing
        let mut performance_monitor = PerformanceMonitor::new();

        // Start with base interval (50ms for compatibility, will be adapted)
        let mut interval = tokio::time::interval(Duration::from_millis(50));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        // Frame timing tracking
        let mut last_frame_start = Instant::now();
        let mut frames_processed = 0u64;

        loop {
            let frame_start = Instant::now();
            interval.tick().await;

            // Determine current effect complexity by examining active effects
            let current_complexity = {
                let state = daemon_state_anim.read().await;
                let mut max_complexity = steelseries_gg::performance::EffectComplexity::Simple;

                for (_, (_, controller, _)) in state.keyboards.iter() {
                    let effect = controller.effect();
                    let effect_complexity = calculate_effect_complexity(effect);
                    if effect_complexity as u8 > max_complexity as u8 {
                        max_complexity = effect_complexity;
                    }
                }
                max_complexity
            };

            // Update performance monitor with current complexity
            performance_monitor.set_effect_complexity(current_complexity);

            // Track frame timing from previous iteration
            if frames_processed > 0 {
                let frame_duration = frame_start.duration_since(last_frame_start);
                // Estimate computation time (we'll measure actual computation below)
                let computation_start = Instant::now();

                // Update memory usage periodically
                if frames_processed.is_multiple_of(60) {
                    performance_monitor.update_memory_usage(estimate_memory_usage());
                }

                // Record performance metrics
                let computation_time = computation_start.elapsed(); // Will be updated with actual time
                performance_monitor.record_frame_timing(frame_duration, computation_time);

                // Log performance summary periodically
                if frames_processed.is_multiple_of(300) {
                    // Every 5 seconds at 60fps
                    tracing::debug!(
                        "RGB Performance: {}",
                        performance_monitor.performance_summary()
                    );
                }
            }

            let computation_start = Instant::now();

            // Split the work: read operations first, then write operations
            let (keyboards_data, overlays, _now) = {
                let mut state = daemon_state_anim.write().await;

                // Clean up expired overlays while we have the lock
                let now = std::time::Instant::now();
                state
                    .gamesense_overlays
                    .retain(|_, (_, expiry)| *expiry > now);

                // Collect data needed for RGB computation to minimize lock time
                let keyboards_data: Vec<_> = state
                    .keyboards
                    .iter_mut()
                    .map(|(serial, (_, controller, info))| {
                        (
                            serial.clone(),
                            controller.compute_colors().to_vec(),
                            info.clone(),
                        )
                    })
                    .collect();

                // Clone overlays only if there are any
                let overlays = if state.gamesense_overlays.is_empty() {
                    HashMap::new()
                } else {
                    state.gamesense_overlays.clone()
                };

                (keyboards_data, overlays, now)
            };

            // Process RGB updates for each keyboard without holding the lock
            for (serial, mut colors, _info) in keyboards_data {
                // Apply GameSense overlays using simple zone mapping
                if !overlays.is_empty() {
                    let zone_count = colors.len();
                    for (zone, (overlay_color, _)) in overlays.iter() {
                        match parse_zone_number(zone) {
                            None => {
                                // Apply to all zones
                                for c in colors.iter_mut() {
                                    *c = *overlay_color;
                                }
                            }
                            Some(idx) if idx < zone_count => {
                                // Apply to specific zone
                                colors[idx] = *overlay_color;
                            }
                            Some(_) => {
                                // Index out of bounds, ignore
                            }
                        }
                    }
                }

                // Apply colors to hardware - get keyboard reference with minimal lock time
                {
                    let mut state = daemon_state_anim.write().await;
                    if let Some((keyboard, _, _)) = state.keyboards.get_mut(&serial) {
                        if let Err(e) = keyboard.set_zone_colors(&colors) {
                            tracing::warn!("Failed to update keyboard {}: {}", serial, e);
                        }
                    }
                    // Lock is automatically dropped here
                }
            }

            // Record actual computation time
            let computation_time = computation_start.elapsed();
            performance_monitor.record_frame_timing(frame_start.elapsed(), computation_time);

            // Calculate optimal timing for next iteration
            let optimal_interval = performance_monitor.calculate_optimal_timing();

            // Update interval if it has changed significantly (>2ms difference)
            let current_interval_ms = interval.period().as_millis() as u64;
            let optimal_interval_ms = optimal_interval.as_millis() as u64;

            if optimal_interval_ms.abs_diff(current_interval_ms) > 2 {
                // Create new interval with optimal timing
                interval = tokio::time::interval(optimal_interval);
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

                tracing::debug!(
                    "Adapted RGB timing: {}ms -> {}ms (complexity: {:?})",
                    current_interval_ms,
                    optimal_interval_ms,
                    current_complexity
                );
            }

            // Performance degradation detection and recovery
            if performance_monitor.is_performance_degraded() {
                tracing::warn!(
                    "RGB performance degraded, applying recovery measures: {}",
                    performance_monitor.performance_summary()
                );

                // Temporary graceful degradation - increase interval by 50%
                let degraded_interval = optimal_interval.mul_f32(1.5);
                interval = tokio::time::interval(degraded_interval);
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            }

            last_frame_start = frame_start;
            frames_processed += 1;
        }
    });

    // Set up graceful shutdown on SIGTERM (systemd stop) and SIGINT (Ctrl+C)
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut sigterm = signal(SignalKind::terminate())?;
        let mut sigint = signal(SignalKind::interrupt())?;

        tokio::select! {
            _ = sigterm.recv() => {
                info!("Received SIGTERM, shutting down gracefully...");
            }
            _ = sigint.recv() => {
                info!("Received SIGINT, shutting down gracefully...");
            }
        }
    }

    #[cfg(not(unix))]
    {
        use tokio::signal;
        signal::ctrl_c().await?;
        info!("Received Ctrl+C, shutting down gracefully...");
    }

    // Stop hot-plug monitoring
    if let Err(e) = hotplug_stop_tx.send(()).await {
        debug!("Hot-plug monitoring task already stopped: {}", e);
    } else {
        info!("Stopped hot-plug monitoring");
    }

    // Abort animation task
    animation_task.abort();

    // Save final device states
    {
        let state = daemon_state.read().await;
        for (_keyboard, controller, info) in state.keyboards.values() {
            let device_id = DeviceId::from(info);
            let final_state = KeyboardState {
                effect: controller.effect().clone(),
                brightness: (controller.brightness() * 100.0) as u8,
            };

            if let Err(e) = state.state_store.update_keyboard(device_id, final_state) {
                warn!("Failed to save final state for {}: {}", info.name, e);
            } else {
                debug!("Saved final state for {}", info.name);
            }
        }
        info!("Saved final states for {} device(s)", state.device_count());
    }

    info!("Daemon stopped.");
    Ok(())
}

/// Show device status with optional live monitoring.
async fn cmd_status(
    _initial_manager: &DeviceManager,
    device_filter: &str,
    refresh_ms: u64,
) -> Result<()> {
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
    let mut manager = DeviceManager::new()?;
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
        let spinner_style =
            ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] {prefix}: {msg}")
                .map_err(|e| Error::Other(format!("Template error: {}", e)))?;

        // Create progress bars for each device
        let mut progress_bars: Vec<(DeviceInfo, ProgressBar)> = Vec::new();

        for device_info in manager.devices() {
            if let Some(filter) = filter_type {
                if device_info.device_type != filter {
                    continue;
                }
            }

            let pb = multi_progress.add(ProgressBar::new_spinner());
            pb.set_style(spinner_style.clone());
            pb.set_prefix(device_info.name.clone());

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

        println!(
            "Monitoring {} device(s) - Press Ctrl+C to stop",
            progress_bars.len()
        );

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

        for device_info in manager.devices() {
            if let Some(filter) = filter_type {
                if device_info.device_type != filter {
                    continue;
                }
            }

            let device_type_str = match device_info.device_type {
                DeviceType::Keyboard => "Keyboard",
                DeviceType::Headset => "Headset",
                DeviceType::Unknown => "Unknown",
            };

            rows.push(DeviceRow {
                name: device_info.name.clone(),
                device_type: device_type_str.to_string(),
                vid_pid: format!(
                    "{:04X}:{:04X}",
                    device_info.vendor_id, device_info.product_id
                ),
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
fn cmd_test_device(
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
    let device_info = manager
        .devices()
        .into_iter()
        .find(|d| d.name.to_lowercase().contains(&device.to_lowercase()) || d.path.contains(device))
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
            validator.validate_keyboard(&mut *keyboard)
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

/// Verify RGB performance metrics over time.
async fn cmd_verify_performance(
    manager: &DeviceManager,
    duration: u64,
    effect_name: &str,
    output: Option<String>,
) -> Result<()> {
    use colored::Colorize;
    use std::io::{self, IsTerminal, Write};
    use steelseries_gg::performance::PerformanceMonitor;
    use tokio::time::{Duration, interval};

    // Find first keyboard device
    let device_info = manager
        .keyboards()
        .first()
        .map(|&d| d.clone())
        .ok_or_else(|| {
            Error::Other("No keyboard device found. Connect a keyboard and try again.".to_string())
        })?;

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
    use steelseries_gg::performance::calculate_effect_complexity;
    let complexity = calculate_effect_complexity(&effect);
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
                keyboard.set_color(colors[0])?;
                keyboard.apply()?;

                // Record timing
                let frame_duration = frame_start.elapsed();
                monitor.record_frame_timing(frame_duration, compute_time);
            }

            _ = print_interval.tick() => {
                let metrics = monitor.metrics();
                let elapsed = start_time.elapsed().as_secs();

                // Format metrics with color coding
                let fps_str = format!("{:.1}/{:.1}", metrics.actual_fps, metrics.target_fps);
                let fps_colored = if use_colors {
                    let fps_ratio = metrics.actual_fps / metrics.target_fps;
                    if fps_ratio >= 0.8 {
                        fps_str.green()
                    } else if fps_ratio >= 0.6 {
                        fps_str.yellow()
                    } else {
                        fps_str.red()
                    }
                } else {
                    fps_str.into()
                };

                let frame_time_str = format!("{:.2}ms", metrics.frame_time);
                let frame_time_colored = if use_colors {
                    if metrics.frame_time <= 20.0 {
                        frame_time_str.green()
                    } else {
                        frame_time_str.yellow()
                    }
                } else {
                    frame_time_str.into()
                };

                let dropped_str = format!("{}", metrics.dropped_frames);
                let dropped_colored = if use_colors && metrics.dropped_frames > 0 {
                    let drop_rate = metrics.dropped_frames as f32 / metrics.total_frames as f32;
                    if drop_rate > 0.05 {
                        dropped_str.red()
                    } else {
                        dropped_str.yellow()
                    }
                } else {
                    dropped_str.into()
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
        use std::fs;
        let json = serde_json::to_string_pretty(&metrics)
            .map_err(|e| Error::Other(format!("Failed to serialize metrics: {}", e)))?;
        fs::write(&output_path, json)
            .map_err(|e| Error::Other(format!("Failed to write {}: {}", output_path, e)))?;
        println!("\nMetrics exported to: {}", output_path);
    }

    Ok(())
}
