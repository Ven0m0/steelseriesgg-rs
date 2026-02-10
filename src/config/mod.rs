//! Configuration management for SteelSeries GG.

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::Result;

/// USB polling rate configuration.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct PollRateConfig {
    /// Mouse polling rate in Hz (125, 500, 1000, 2000, or 4000)
    pub mouse_hz: Option<u32>,

    /// Keyboard polling rate in Hz (125, 500, 1000, 2000, or 4000)
    pub keyboard_hz: Option<u32>,
}

/// Application configuration.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Config {
    /// GameSense server settings
    pub gamesense: GameSenseConfig,

    /// Audio mixer settings
    #[cfg(feature = "audio")]
    pub audio: AudioConfig,

    /// Default profile name
    pub default_profile: Option<String>,

    /// Enable debug logging
    pub debug: bool,

    /// USB polling rate settings
    pub poll_rate: PollRateConfig,
}

/// GameSense server configuration.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GameSenseConfig {
    /// Enable GameSense HTTP server
    pub enabled: bool,

    /// Server bind address
    pub bind_address: String,

    /// Server port (default: 27301)
    pub port: u16,
}

impl Default for GameSenseConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bind_address: "127.0.0.1".to_string(),
            port: 27301,
        }
    }
}

/// Audio mixer configuration.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg(feature = "audio")]
pub struct AudioConfig {
    /// Enable audio mixer
    pub enabled: bool,

    /// Default master volume (0.0 - 1.0)
    pub master_volume: f32,

    /// Channel volumes
    pub channels: ChannelVolumes,
}

#[cfg(feature = "audio")]
impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            master_volume: 1.0,
            channels: ChannelVolumes::default(),
        }
    }
}

/// Individual channel volume settings.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg(feature = "audio")]
pub struct ChannelVolumes {
    pub game: f32,
    pub chat: f32,
    pub media: f32,
    pub aux: f32,
    pub mic: f32,
}

#[cfg(feature = "audio")]
impl Default for ChannelVolumes {
    fn default() -> Self {
        Self {
            game: 1.0,
            chat: 1.0,
            media: 1.0,
            aux: 1.0,
            mic: 1.0,
        }
    }
}

impl Config {
    /// Get the configuration directory path.
    pub fn config_dir() -> Option<PathBuf> {
        ProjectDirs::from("com", "steelseries-gg", "ssgg").map(|dirs| dirs.config_dir().to_path_buf())
    }

    /// Get the config file path.
    pub fn config_path() -> Option<PathBuf> {
        Self::config_dir().map(|dir| dir.join("config.toml"))
    }

    /// Load configuration from file asynchronously, or return default if not found.
    pub async fn load_async() -> Result<Self> {
        let path = match Self::config_path() {
            Some(p) => p,
            None => return Ok(Self::default()),
        };

        match tokio::fs::read_to_string(&path).await {
            Ok(content) => {
                let config: Config = toml::from_str(&content)?;
                Ok(config)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    /// Load configuration from file, or return default if not found.
    pub fn load() -> Result<Self> {
        let path = match Self::config_path() {
            Some(p) => p,
            None => return Ok(Self::default()),
        };

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to file.
    pub fn save(&self) -> Result<()> {
        let dir = match Self::config_dir() {
            Some(d) => d,
            None => return Err("Could not determine config directory".into()),
        };

        std::fs::create_dir_all(&dir)?;

        let path = dir.join("config.toml");
        let content = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, content)?;

        Ok(())
    }
}
