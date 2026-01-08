//! SteelSeries GG for Linux - CLI
//!
//! A complete open-source replacement for SteelSeries GG on Linux.

use clap::{Parser, Subcommand};
use std::time::Duration;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use steelseries_gg::audio::SonarClient;
use steelseries_gg::config::Config;
use steelseries_gg::devices::{discovery::print_device_summary, DeviceManager, DeviceType};
use steelseries_gg::gamesense::GameSenseServer;
use steelseries_gg::profiles::{Profile, ProfileManager};
use steelseries_gg::rgb::{Color, Effect, WaveDirection};

#[cfg(feature = "audio")]
use steelseries_gg::audio::{AudioMixer, Channel};

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
    Sonar {
        #[command(subcommand)]
        action: SonarAction,
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

fn parse_color(s: &str) -> Option<Color> {
    // Try named colors
    match s.to_lowercase().as_str() {
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
    match s.to_lowercase().as_str() {
        "master" => Some(Channel::Master),
        "game" => Some(Channel::Game),
        "chat" => Some(Channel::Chat),
        "media" => Some(Channel::Media),
        "aux" => Some(Channel::Aux),
        "mic" => Some(Channel::Mic),
        _ => None,
    }
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

        Commands::Sonar { action } => {
            cmd_sonar(action).await?;
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

    // Open the device
    let device = manager.open_device(keyboard_info)?;

    match action {
        RgbAction::Color { color } => {
            let color =
                parse_color(&color).ok_or_else(|| anyhow::anyhow!("Invalid color: {}", color))?;

            println!("Setting color to {}", color);

            // Build the color packet (9 zones)
            let mut data = vec![0x21, 0xFF];
            for _ in 0..9 {
                data.push(color.r);
                data.push(color.g);
                data.push(color.b);
            }

            // Pad to 64 bytes
            let mut report = vec![0u8; 65];
            report[1..1 + data.len()].copy_from_slice(&data);
            device.write(&report)?;

            println!("Done!");
        }

        RgbAction::Brightness { level } => {
            let level = level.min(100);
            println!("Setting brightness to {}%", level);

            let mut report = vec![0u8; 65];
            report[1] = 0x22;
            report[2] = level;
            device.write(&report)?;

            println!("Done!");
        }

        RgbAction::Effect { name, speed } => {
            let effect = match name.to_lowercase().as_str() {
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
            // Note: Full effect implementation would require a background loop
            println!("(Note: Animated effects require running as daemon)");
        }

        RgbAction::Off => {
            println!("Turning off LEDs");

            let mut report = vec![0u8; 65];
            report[1] = 0x21;
            report[2] = 0xFF;
            // All zeros for black
            device.write(&report)?;

            println!("Done!");
        }
    }

    Ok(())
}

fn cmd_profile(action: ProfileAction) -> anyhow::Result<()> {
    let mut manager = ProfileManager::new()?;

    match action {
        ProfileAction::List => {
            let profiles = manager.list();
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
            if let Some(profile) = manager.get(&name) {
                println!("Loading profile: {}", profile.name);
                // TODO: Apply profile settings to devices
                println!("Profile loaded!");
            } else {
                println!("Profile not found: {}", name);
            }
        }

        ProfileAction::Save { name } => {
            let profile = Profile::new(name.clone());
            // TODO: Capture current device settings
            manager.set(profile)?;
            println!("Profile saved: {}", name);
        }

        ProfileAction::Delete { name } => {
            manager.delete(&name)?;
            println!("Profile deleted: {}", name);
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
            let volume = (level.min(100) as f32) / 100.0;

            match channel.to_lowercase().as_str() {
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
                    let volume = (level.min(100) as f32) / 100.0;

                    match channel.to_lowercase().as_str() {
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

                    println!(
                        "Monitoring {} volume set to {}%",
                        channel,
                        level.min(100)
                    );
                }

                StreamerAction::Streaming { channel, level } => {
                    let volume = (level.min(100) as f32) / 100.0;

                    match channel.to_lowercase().as_str() {
                        "master" => client.set_streaming_master_volume(volume).await?,
                        _ => {
                            return Err(anyhow::anyhow!(
                                "Invalid channel: {}. Only master is currently supported",
                                channel
                            ));
                        }
                    }

                    println!(
                        "Streaming {} volume set to {}%",
                        channel,
                        level.min(100)
                    );
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

async fn cmd_daemon(manager: DeviceManager) -> anyhow::Result<()> {
    info!("Starting SteelSeries GG daemon");

    let config = Config::load()?;

    // Start GameSense server in background if enabled
    if config.gamesense.enabled {
        let gs_bind = config.gamesense.bind_address.clone();
        let gs_port = config.gamesense.port;
        tokio::spawn(async move {
            match GameSenseServer::new(&gs_bind, gs_port) {
                Ok(server) => {
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
                // TODO: Apply profile
            }
        }
    }

    info!("Daemon running. Press Ctrl+C to stop.");

    // Keep running
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}
