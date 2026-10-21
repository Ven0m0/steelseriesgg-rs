//! Configuration management for SteelSeries GG.

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::input::{Action, BindingSet, InputKey};
use crate::{Error, Result};

/// USB polling rate configuration.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct PollRateConfig {
    /// Mouse polling rate in Hz (125, 250, 500, 1000, 2000, or 4000)
    pub mouse_hz: Option<u32>,

    /// Keyboard polling rate in Hz (125, 250, 500, 1000, 2000, or 4000)
    pub keyboard_hz: Option<u32>,
}

/// Manual override for HID control-endpoint selection.
///
/// Auto-detection ranks HID collections by usage page (see
/// `devices::STEELSERIES_CONTROL_USAGE_PAGE`); this lets a user pin the exact collection when
/// auto-detection picks the wrong one on hardware the ranking hasn't been verified against.
/// Both fields default to `None`, in which case auto-detection runs unchanged.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct DeviceConfig {
    /// Pin the control endpoint to an exact HID path (as reported by `ssgg devices`).
    /// Takes priority over `control_usage_page` when both are set.
    pub control_path: Option<String>,

    /// Pin the control endpoint to a usage page, e.g. `0xFFC0`.
    pub control_usage_page: Option<u16>,
}

/// A binding target in `[remap.bindings]`: one key name, or an array of key names for a chord.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum BindingTarget {
    Key(String),
    Combo(Vec<String>),
}

/// USB product id as written in the config: an integer or a hex string such as `"0x1628"`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ProductIdSetting {
    Number(u16),
    Hex(String),
}

/// Key remapper settings. Only remapping one key to another and one key to a chord exist.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct RemapConfig {
    /// Only grab keyboards with this USB product id; all SteelSeries keyboards when unset.
    pub device: Option<ProductIdSetting>,

    /// Source key name to target key name, or to an array of key names (a chord, at most
    /// `input::MAX_COMBO_LEN` keys).
    pub bindings: BTreeMap<String, BindingTarget>,
}

impl RemapConfig {
    /// Parse and check the product id filter.
    pub fn product_id(&self) -> Result<Option<u16>> {
        match &self.device {
            None => Ok(None),
            Some(ProductIdSetting::Number(id)) => Ok(Some(*id)),
            Some(ProductIdSetting::Hex(text)) => {
                let digits = text.trim();
                let digits = digits
                    .strip_prefix("0x")
                    .or_else(|| digits.strip_prefix("0X"))
                    .unwrap_or(digits);
                u16::from_str_radix(digits, 16)
                    .map(Some)
                    .map_err(|_| Error::InvalidConfig(format!("remap.device '{text}' is not a hex USB product id")))
            }
        }
    }

    /// Resolve every key name and validate the bindings.
    pub fn bindings(&self) -> Result<BindingSet> {
        let mut pairs = Vec::with_capacity(self.bindings.len());
        for (source, target) in &self.bindings {
            let action = match target {
                BindingTarget::Key(name) => Action::Key(InputKey::parse(name)?),
                BindingTarget::Combo(names) => {
                    Action::Combo(names.iter().map(|n| InputKey::parse(n)).collect::<Result<_>>()?)
                }
            };
            pairs.push((InputKey::parse(source)?, action));
        }
        BindingSet::from_pairs(pairs)
    }

    /// Check everything a load can reject: the product id and the bindings.
    pub fn validate(&self) -> Result<()> {
        self.product_id()?;
        self.bindings().map(|_| ())
    }
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

    /// Manual HID device selection override
    pub device: DeviceConfig,

    /// Key remapper settings (`ssgg remap`, needs the `remap` feature to run)
    #[serde(default)]
    pub remap: RemapConfig,
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
                config.remap.validate()?;
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
        config.remap.validate()?;
        Ok(config)
    }

    /// Save configuration to file asynchronously.
    pub async fn save_async(&self) -> Result<()> {
        let config = self.clone();
        tokio::task::spawn_blocking(move || config.save())
            .await
            .map_err(|e| crate::error::Error::Other(e.to_string()))?
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

#[cfg(test)]
mod tests {
    use super::*;

    fn remap_from(toml_text: &str) -> RemapConfig {
        toml::from_str::<RemapConfig>(toml_text).unwrap()
    }

    #[test]
    fn config_without_remap_section_defaults_to_empty() {
        let full = toml::to_string_pretty(&Config::default()).unwrap();
        let without = full.split("[remap").next().unwrap();
        assert!(!without.contains("remap"));
        let parsed: Config = toml::from_str(without).unwrap();
        assert_eq!(parsed.remap, RemapConfig::default());
        assert!(parsed.remap.bindings.is_empty());
        assert!(parsed.remap.validate().is_ok());
    }

    #[test]
    fn remap_section_round_trips_through_config() {
        let config = Config {
            remap: remap_from(
                r#"
            device = "0x1628"
            [bindings]
            caps_lock = "esc"
            f13 = ["left_ctrl", "c"]
            "#,
            ),
            ..Config::default()
        };
        let text = toml::to_string_pretty(&config).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back.remap, config.remap);
        assert_eq!(back.remap.product_id().unwrap(), Some(0x1628));
        assert_eq!(back.remap.bindings().unwrap().len(), 2);
    }

    #[test]
    fn product_id_accepts_integer_and_hex_forms() {
        assert_eq!(remap_from("device = 5672").product_id().unwrap(), Some(5672));
        assert_eq!(remap_from("device = \"1628\"").product_id().unwrap(), Some(0x1628));
        assert_eq!(remap_from("device = \"0X1612\"").product_id().unwrap(), Some(0x1612));
        assert_eq!(remap_from("").product_id().unwrap(), None);
        assert!(remap_from("device = \"zz\"").product_id().is_err());
        assert!(remap_from("device = \"0x12345\"").product_id().is_err());
    }

    #[test]
    fn validation_rejects_bad_bindings() {
        let bad = [
            "[bindings]\nnot_a_key = \"esc\"",
            "[bindings]\ncaps_lock = \"nope\"",
            "[bindings]\ncaps_lock = \"caps_lock\"",
            "[bindings]\ncaps_lock = []",
            "[bindings]\ncaps_lock = [\"a\", \"b\", \"c\", \"d\", \"e\"]",
            "[bindings]\nesc = \"a\"\nescape = \"b\"",
        ];
        for text in bad {
            assert!(remap_from(text).validate().is_err(), "{text}");
        }
    }

    #[test]
    fn validation_error_names_the_offender() {
        let err = remap_from("[bindings]\ncapslokc = \"esc\"")
            .validate()
            .unwrap_err()
            .to_string();
        assert!(err.contains("capslokc"), "{err}");
    }

    #[test]
    fn valid_bindings_resolve_to_actions() {
        let set = remap_from("[bindings]\nCaps_Lock = \"ESC\"\nf13 = [\"left_ctrl\", \"c\"]")
            .bindings()
            .unwrap();
        let caps = InputKey::parse("caps_lock").unwrap();
        assert_eq!(set.get(caps), Some(&Action::Key(InputKey::parse("esc").unwrap())));
    }
}
