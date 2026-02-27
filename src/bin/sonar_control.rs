use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use steelseries_gg::audio::sonar::{ChannelVolumes, ClassicVolume, SonarChannel, SonarClient};

#[derive(Parser)]
#[command(name = "sonar_control")]
#[command(about = "Control SteelSeries Sonar audio settings")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List all volume settings
    List {
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },
    /// Set volume for a specific channel
    SetVolume {
        /// Channel to configure
        #[arg(value_enum)]
        channel: CliChannel,

        /// Volume level (0.0 - 1.0)
        value: f32,

        /// Operation mode
        #[arg(long, default_value = "classic")]
        mode: CliMode,
    },
    /// Get volume for a specific channel
    GetVolume {
        /// Channel to query
        #[arg(value_enum)]
        channel: CliChannel,

        /// Operation mode
        #[arg(long, default_value = "classic")]
        mode: CliMode,
    },
    /// Mute a channel (Classic mode only)
    Mute {
        /// Channel to mute
        #[arg(value_enum)]
        channel: CliChannel,
    },
    /// Unmute a channel (Classic mode only)
    Unmute {
        /// Channel to unmute
        #[arg(value_enum)]
        channel: CliChannel,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum CliChannel {
    Master,
    Game,
    Chat,
    Media,
    Aux,
    /// Chat Capture (Streamer mode only)
    ChatCapture,
}

impl From<CliChannel> for SonarChannel {
    fn from(c: CliChannel) -> Self {
        match c {
            CliChannel::Master => SonarChannel::Master,
            CliChannel::Game => SonarChannel::Game,
            CliChannel::Chat => SonarChannel::Chat,
            CliChannel::Media => SonarChannel::Media,
            CliChannel::Aux => SonarChannel::Aux,
            CliChannel::ChatCapture => SonarChannel::ChatCapture,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum CliMode {
    Classic,
    Monitoring,
    Streaming,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing subscriber for library logs, but keep stdout clean for CLI output
    // unless RUST_LOG is set.
    if std::env::var("RUST_LOG").is_ok() {
        tracing_subscriber::fmt::init();
    }

    let cli = Cli::parse();

    let client = SonarClient::new()
        .await
        .context("Failed to connect to SteelSeries Sonar. Is it running?")?;

    match cli.command {
        Command::List { json } => {
            let classic = client.get_classic_volumes().await?;
            let streamer = client.get_streamer_volumes().await?;

            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "classic": classic,
                        "streamer": streamer
                    })
                );
            } else {
                println!("Classic Mode:");
                print_classic_volumes(&classic);
                println!("\nStreamer Mode - Monitoring:");
                print_channel_volumes(&streamer.monitoring);
                println!("\nStreamer Mode - Streaming:");
                print_channel_volumes(&streamer.streaming);
            }
        }
        Command::SetVolume {
            channel,
            value,
            mode,
        } => {
            let sonar_channel = channel.into();
            match mode {
                CliMode::Classic => {
                    if matches!(channel, CliChannel::ChatCapture) {
                        anyhow::bail!("ChatCapture channel is not available in Classic mode");
                    }
                    client.set_channel_volume(sonar_channel, value).await?;
                }
                CliMode::Monitoring => {
                    client
                        .set_monitoring_channel_volume(sonar_channel, value)
                        .await?;
                }
                CliMode::Streaming => {
                    client
                        .set_streaming_channel_volume(sonar_channel, value)
                        .await?;
                }
            }
            println!(
                "Set {} volume for {:?} to {:.2}",
                mode_str(mode),
                channel,
                value
            );
        }
        Command::GetVolume { channel, mode } => {
            let sonar_channel = channel.into();
            let vol = match mode {
                CliMode::Classic => {
                    if matches!(channel, CliChannel::ChatCapture) {
                        anyhow::bail!("ChatCapture channel is not available in Classic mode");
                    }
                    client.get_channel_volume(sonar_channel).await?
                }
                CliMode::Monitoring => client.get_monitoring_channel_volume(sonar_channel).await?,
                CliMode::Streaming => client.get_streaming_channel_volume(sonar_channel).await?,
            };
            println!("{:.2}", vol);
        }
        Command::Mute { channel } => {
            if matches!(channel, CliChannel::ChatCapture) {
                anyhow::bail!("ChatCapture channel is not available in Classic mode");
            }
            client.set_channel_mute(channel.into(), true).await?;
            println!("Muted {:?}", channel);
        }
        Command::Unmute { channel } => {
            if matches!(channel, CliChannel::ChatCapture) {
                anyhow::bail!("ChatCapture channel is not available in Classic mode");
            }
            client.set_channel_mute(channel.into(), false).await?;
            println!("Unmuted {:?}", channel);
        }
    }

    Ok(())
}

fn mode_str(mode: CliMode) -> &'static str {
    match mode {
        CliMode::Classic => "Classic",
        CliMode::Monitoring => "Monitoring",
        CliMode::Streaming => "Streaming",
    }
}

fn print_classic_volumes(vol: &ClassicVolume) {
    println!("  Master: {:.2}", vol.master);
    println!("  Game:   {:.2}", vol.game);
    println!("  Chat:   {:.2}", vol.chat);
    println!("  Media:  {:.2}", vol.media);
    println!("  Aux:    {:.2}", vol.aux);
}

fn print_channel_volumes(vol: &ChannelVolumes) {
    println!("  Master:       {:.2}", vol.master);
    println!("  Game:         {:.2}", vol.game);
    println!("  Chat:         {:.2}", vol.chat);
    println!("  Media:        {:.2}", vol.media);
    println!("  Aux:          {:.2}", vol.aux);
    println!("  Chat Capture: {:.2}", vol.chat_capture);
}
