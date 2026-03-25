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

        #[cfg(unix)]
        {
            use std::fs;
            use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

            match fs::symlink_metadata(&dir) {
                // Path does not exist: create securely with 0o700.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    fs::DirBuilder::new().recursive(true).mode(0o700).create(&dir)?;
                }
                // Path exists: ensure it's a real directory and tighten permissions.
                Ok(metadata) => {
                    if !metadata.file_type().is_dir() {
                        return Err("Config directory path is not a directory".into());
                    }

                    let perms = metadata.permissions();
                    // If any group/other bits are set, tighten to 0o700.
                    if perms.mode() & 0o077 != 0 {
                        let mut new_perms = perms;
                        new_perms.set_mode(0o700);
                        fs::set_permissions(&dir, new_perms)?;
                    }
                }
                // Any other I/O error while inspecting metadata is propagated.
                Err(e) => return Err(e.into()),
            }
        }
        #[cfg(not(unix))]
        {
            std::fs::create_dir_all(&dir)?;
        }

        let path = dir.join("config.toml");
        let content = toml::to_string_pretty(self).map_err(|e| e.to_string())?;

        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

            // Refuse to operate on a symlinked config file.
            if let Ok(metadata) = std::fs::symlink_metadata(&path) {
                if metadata.file_type().is_symlink() {
                    return Err("Refusing to write config.toml because it is a symlink".into());
                }
            }

            let mut options = std::fs::OpenOptions::new();
            options
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW);

            // Set file creation mode to 600 (rw-------) without following symlinks.
            let mut file = match options.open(&path) {
                Ok(file) => file,
                Err(err) => {
                    // ELOOP indicates a symlink in the path when O_NOFOLLOW is set.
                    if err.raw_os_error() == Some(libc::ELOOP) {
                        return Err("Refusing to write config.toml because it is (or contains) a symlink".into());
                    }
                    return Err(err.into());
                }
            };

            // Ensure permissions are 600 even if file already existed.
            let mut perms = file.metadata()?.permissions();
            perms.set_mode(0o600);
            file.set_permissions(perms)?;
            file.write_all(content.as_bytes())?;
        }
        #[cfg(not(unix))]
        {
            std::fs::write(&path, content)?;
        }

        Ok(())
    }
}
