//! SteelSeries GG for Linux - CLI
//!
//! A complete open-source replacement for SteelSeries GG on Linux.

use clap::{Parser, Subcommand};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use steelseries_gg::config::Config;
use steelseries_gg::device_state::{DeviceId, DeviceStateStore, KeyboardState};
use steelseries_gg::devices::keyboards::Keyboard;
use steelseries_gg::devices::{
    discovery::print_device_summary, DeviceInfo, DeviceManager, DeviceType,
};
use steelseries_gg::gamesense::GameSenseServer;
use steelseries_gg::profiles::{KeyboardProfile, Profile, ProfileManager};
use steelseries_gg::rgb::{Color, Effect, RgbController, WaveDirection};

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

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

    /// Run as a daemon (device control + GameSense server)
    Daemon,
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
#[cfg(feature = "sonar")]
fn normalize_volume(level: u8) -> f32 {
    (level.min(100) as f32) / 100.0
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Setup logging
    let level = if cli.debug { Level::DEBUG } else { Level::INFO };
    let subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .with_target(false)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    match cli.command {
        Commands::Devices => {
            let manager = DeviceManager::new()?;
            cmd_devices(&manager)?;
        }

        Commands::Rgb { action } => {
            let manager = DeviceManager::new()?;
            cmd_rgb(&manager, action)?;
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

        Commands::Daemon => {
            let manager = DeviceManager::new()?;
            cmd_daemon(manager).await?;
        }
    }

    Ok(())
}

fn cmd_devices(manager: &DeviceManager) -> anyhow::Result<()> {
    print_device_summary(manager);
    Ok(())
}

fn cmd_rgb(manager: &DeviceManager, action: RgbAction) -> anyhow::Result<()> {
    // Find the first keyboard
    let keyboard_info = manager
        .first_device_of_type(DeviceType::Keyboard)
        .ok_or_else(|| anyhow::anyhow!("No keyboard found"))?;

    println!("Using keyboard: {}", keyboard_info.name);

    // Open device state store for persistence
    let mut state_store = DeviceStateStore::new()?;
    let device_id = DeviceId::from(keyboard_info);

    // Open the keyboard using the abstraction layer
    let mut keyboard = manager.open_keyboard(keyboard_info)?;

    // Initialize the device
    keyboard.initialize()?;

    match action {
        RgbAction::Color { color } => {
            let color =
                parse_color(&color).ok_or_else(|| anyhow::anyhow!("Invalid color: {}", color))?;

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
    }

    Ok(())
}

fn cmd_profile(action: ProfileAction) -> anyhow::Result<()> {
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
                let mut state_store = DeviceStateStore::new()?;

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

fn cmd_pollrate(action: PollrateAction) -> anyhow::Result<()> {
    use steelseries_gg::pollrate::{get_poll_rate, set_poll_rate, DeviceType, PollRate};

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
fn cmd_audio(action: AudioAction) -> anyhow::Result<()> {
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
                .ok_or_else(|| anyhow::anyhow!("Invalid channel: {}", channel))?;

            let volume = (level.min(100) as f32) / 100.0;
            mixer.set_volume(ch, volume)?;
            println!("{} volume set to {}%", ch, level.min(100));
        }

        AudioAction::Mute { channel, state } => {
            let ch = parse_channel(&channel)
                .ok_or_else(|| anyhow::anyhow!("Invalid channel: {}", channel))?;

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
async fn cmd_sonar(action: SonarAction) -> anyhow::Result<()> {
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
            let client = SonarClient::new().await?;
            let volume = normalize_volume(level);
            let channel_lower = channel.to_ascii_lowercase();

            match channel_lower.as_str() {
                "master" => client.set_classic_master_volume(volume).await?,
                "game" => client.set_classic_game_volume(volume).await?,
                "chat" => client.set_classic_chat_volume(volume).await?,
                "media" => client.set_classic_media_volume(volume).await?,
                "aux" => client.set_classic_aux_volume(volume).await?,
                _ => {
                    return Err(anyhow::anyhow!(
                        "Invalid channel: {}. Use master, game, chat, media, or aux",
                        channel
                    ));
                }
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
                    let volume = normalize_volume(level);
                    let channel_lower = channel.to_ascii_lowercase();

                    match channel_lower.as_str() {
                        "master" => client.set_monitoring_master_volume(volume).await?,
                        "game" => client.set_monitoring_game_volume(volume).await?,
                        "chat" => client.set_monitoring_chat_volume(volume).await?,
                        _ => {
                            return Err(anyhow::anyhow!(
                                "Invalid channel: {}. Use master, game, or chat",
                                channel
                            ));
                        }
                    }

                    println!("Monitoring {} volume set to {}%", channel, level.min(100));
                }

                StreamerAction::Streaming { channel, level } => {
                    let volume = normalize_volume(level);
                    let channel_lower = channel.to_ascii_lowercase();

                    match channel_lower.as_str() {
                        "master" => client.set_streaming_master_volume(volume).await?,
                        _ => {
                            return Err(anyhow::anyhow!(
                                "Invalid channel: {}. Only master is currently supported",
                                channel
                            ));
                        }
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

async fn cmd_server(port: u16) -> anyhow::Result<()> {
    info!("Starting GameSense server on port {}", port);

    let server = GameSenseServer::new("127.0.0.1", port)?;
    server.run().await?;

    Ok(())
}

/// Daemon state for managing connected devices and RGB controllers.
struct DaemonState {
    keyboards: HashMap<String, (Box<dyn Keyboard>, RgbController, DeviceInfo)>,
    gamesense_overlays: HashMap<String, (Color, std::time::Instant)>, // zone -> (color, expiry)
}

impl DaemonState {
    fn new() -> Self {
        Self {
            keyboards: HashMap::new(),
            gamesense_overlays: HashMap::new(),
        }
    }
}

async fn cmd_daemon(manager: DeviceManager) -> anyhow::Result<()> {
    info!("Starting SteelSeries GG daemon");

    let config = Config::load()?;
    let mut state_store = DeviceStateStore::new()?;

    // Apply saved polling rates
    {
        use steelseries_gg::pollrate::{set_poll_rate, DeviceType, PollRate};

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
    let daemon_state = Arc::new(RwLock::new(DaemonState::new()));

    // Discover and open keyboards
    let keyboards_info = manager.keyboards();
    if keyboards_info.is_empty() {
        info!("No keyboards found");
    } else {
        let mut state = daemon_state.write().await;
        for kb_info in keyboards_info {
            match manager.open_keyboard(kb_info) {
                Ok(keyboard) => {
                    let zone_count = keyboard.zone_count();
                    let rgb_controller = RgbController::new(zone_count);
                    let serial = kb_info
                        .serial_number
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string());

                    info!("Opened keyboard: {} (zones: {})", kb_info.name, zone_count);
                    state
                        .keyboards
                        .insert(serial.clone(), (keyboard, rgb_controller, kb_info.clone()));

                    // Load state from store if available
                    let device_id = DeviceId::from(kb_info);
                    if let Some(device_state) = state_store.get(&device_id) {
                        if let Some(ref kb_state) = device_state.keyboard {
                            if let Some((_, controller, _)) = state.keyboards.get_mut(&serial) {
                                controller.set_effect(kb_state.effect.clone());
                                controller.set_brightness(kb_state.brightness as f32 / 100.0);
                                info!(
                                    "Loaded state for {}: brightness={}%, effect={:?}",
                                    kb_info.name, kb_state.brightness, kb_state.effect
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to open keyboard {}: {}", kb_info.name, e);
                }
            }
        }
    }

    // Start GameSense server in background if enabled
    if config.gamesense.enabled {
        let gs_bind = config.gamesense.bind_address.clone();
        let gs_port = config.gamesense.port;
        let daemon_state_clone = daemon_state.clone();

        tokio::spawn(async move {
            match GameSenseServer::new(&gs_bind, gs_port) {
                Ok(server) => {
                    // Set RGB callback to update overlays
                    server
                        .set_rgb_callback(move |zone: &str, r: u8, g: u8, b: u8| {
                            let state = daemon_state_clone.clone();
                            let zone_owned = zone.to_string();
                            tokio::spawn(async move {
                                let mut state = state.write().await;
                                let color = Color::new(r, g, b);
                                let expiry = std::time::Instant::now() + Duration::from_secs(30);
                                state
                                    .gamesense_overlays
                                    .insert(zone_owned.clone(), (color, expiry));
                                tracing::debug!("GameSense overlay: {} = {:?}", zone_owned, color);
                            });
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
                    let mut state = daemon_state.write().await;
                    for (_serial, (_keyboard, controller, info)) in state.keyboards.iter_mut() {
                        controller.set_effect(keyboard_profile.effect.clone());
                        controller.set_brightness(keyboard_profile.brightness as f32 / 100.0);

                        // Persist to state store
                        let device_id = DeviceId::from(&*info);
                        let _ = state_store.update_keyboard(
                            device_id,
                            KeyboardState {
                                effect: keyboard_profile.effect.clone(),
                                brightness: keyboard_profile.brightness,
                            },
                        );

                        info!("Applied profile to keyboard: {}", info.name);
                    }
                }
            }
        }
    }

    info!("Daemon running. Press Ctrl+C to stop.");

    // Spawn RGB animation loop
    let daemon_state_anim = daemon_state.clone();
    let animation_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(33)); // ~30 FPS
        let mut last_write = std::time::Instant::now();

        loop {
            interval.tick().await;

            let mut state = daemon_state_anim.write().await;

            // Clean up expired overlays
            let now = std::time::Instant::now();
            state
                .gamesense_overlays
                .retain(|_, (_, expiry)| *expiry > now);

            // Rate limiting: only write every 50ms to avoid USB spam
            if last_write.elapsed() < Duration::from_millis(50) {
                continue;
            }

            let overlays: Vec<(String, (Color, std::time::Instant))> = state
                .gamesense_overlays
                .iter()
                .map(|(zone, (color, expiry))| (zone.clone(), (*color, *expiry)))
                .collect();

            for (serial, (keyboard, controller, _info)) in state.keyboards.iter_mut() {
                let mut colors = controller.compute_colors();

                // Apply GameSense overlays using simple zone mapping.
                if !overlays.is_empty() {
                    let zone_count = colors.len();
                    for (zone, (overlay_color, _)) in overlays.iter() {
                        let zl = zone.to_ascii_lowercase();

                        let apply_all = zl == "all" || zl == "keyboard";
                        if apply_all {
                            for c in colors.iter_mut() {
                                *c = *overlay_color;
                            }
                            continue;
                        }

                        let idx = zl
                            .strip_prefix("zone")
                            .and_then(|rest| rest.parse::<usize>().ok())
                            .or_else(|| zl.parse::<usize>().ok());

                        if let Some(one_based) = idx {
                            if one_based >= 1 && one_based <= zone_count {
                                colors[one_based - 1] = *overlay_color;
                            }
                        }
                    }
                }

                if let Err(e) = keyboard.set_zone_colors(&colors) {
                    tracing::warn!("Failed to update keyboard {}: {}", serial, e);
                }
            }

            last_write = now;
        }
    });

    // Set up graceful shutdown on SIGTERM (systemd stop) and SIGINT (Ctrl+C)
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

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

    // Abort animation task
    animation_task.abort();

    info!("Daemon stopped.");
    Ok(())
}
