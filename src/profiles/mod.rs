//! Profile management for saving and loading device configurations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::warn;

use crate::config::Config;
use crate::rgb::{Color, Effect};
use crate::{Error, Result};

/// A device configuration profile.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Profile {
    /// Profile name.
    pub name: String,

    /// Optional description.
    pub description: Option<String>,

    /// Keyboard settings.
    #[serde(default)]
    pub keyboard: Option<KeyboardProfile>,

    /// Headset settings.
    #[serde(default)]
    pub headset: Option<HeadsetProfile>,
}

/// Keyboard-specific profile settings.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KeyboardProfile {
    /// RGB effect.
    pub effect: Effect,

    /// Brightness (0-100).
    pub brightness: u8,
}

impl Default for KeyboardProfile {
    fn default() -> Self {
        Self {
            effect: Effect::Static { color: Color::WHITE },
            brightness: 100,
        }
    }
}

/// Headset-specific profile settings.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HeadsetProfile {
    /// Sidetone level (0-100).
    pub sidetone: u8,

    /// Microphone volume (0-100).
    pub mic_volume: u8,

    /// EQ preset name.
    pub eq_preset: String,

    /// Auto-off timeout in minutes.
    pub auto_off_minutes: u8,
}

impl Default for HeadsetProfile {
    fn default() -> Self {
        Self {
            sidetone: 50,
            mic_volume: 100,
            eq_preset: "Flat".to_string(),
            auto_off_minutes: 15,
        }
    }
}

impl Profile {
    /// Create a new profile with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            keyboard: None,
            headset: None,
        }
    }

    /// Create a default profile with all device settings.
    pub fn default_full() -> Self {
        Self {
            name: "Default".to_string(),
            description: Some("Default profile with standard settings".to_string()),
            keyboard: Some(KeyboardProfile::default()),
            headset: Some(HeadsetProfile::default()),
        }
    }
}

/// Manager for loading and saving profiles.
pub struct ProfileManager {
    profiles: HashMap<String, Profile>,
    profiles_dir: PathBuf,
}

impl ProfileManager {
    /// Create a new profile manager.
    pub fn new() -> Result<Self> {
        let profiles_dir = Config::config_dir()
            .ok_or_else(|| Error::Profile("could not determine config directory".to_string()))?
            .join("profiles");

        std::fs::create_dir_all(&profiles_dir)?;

        let mut manager = Self {
            profiles: HashMap::new(),
            profiles_dir,
        };

        manager.load_all()?;
        Ok(manager)
    }

    /// Load all profiles from disk.
    pub fn load_all(&mut self) -> Result<()> {
        self.profiles.clear();

        if !self.profiles_dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(&self.profiles_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "json").unwrap_or(false) {
                match std::fs::read_to_string(&path) {
                    Ok(content) => match serde_json::from_str::<Profile>(&content) {
                        Ok(profile) => {
                            self.profiles.insert(profile.name.clone(), profile);
                        }
                        Err(err) => {
                            warn!("Skipping invalid profile file {}: {}", path.display(), err)
                        }
                    },
                    Err(err) => warn!("Failed to read profile file {}: {}", path.display(), err),
                }
            }
        }

        Ok(())
    }

    /// Sanitize a profile name for use as a filename.
    /// Removes or replaces characters that are invalid in filenames.
    pub(crate) fn sanitize_filename(name: &str) -> String {
        name.chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                c if c.is_control() => '_',
                c => c,
            })
            .collect::<String>()
            .trim()
            .to_string()
    }

    /// Save a profile to disk.
    pub fn save(&self, profile: &Profile) -> Result<()> {
        let filename = Self::sanitize_filename(&profile.name);
        if filename.is_empty() {
            return Err(Error::Profile("Profile name cannot be empty".to_string()));
        }
        let path = self.profiles_dir.join(format!("{}.json", filename));
        let content = serde_json::to_string_pretty(profile)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create(true).truncate(true).mode(0o600);

            // Set file creation mode to 600 (rw-------)
            let mut file = options.open(&path)?;

            // Ensure permissions are 600 even if file already existed
            let mut perms = file.metadata()?.permissions();
            use std::os::unix::fs::PermissionsExt;
            if perms.mode() & 0o777 != 0o600 {
                perms.set_mode(0o600);
                file.set_permissions(perms)?;
            }

            use std::io::Write;
            file.write_all(content.as_bytes())?;
        }
        #[cfg(not(unix))]
        {
            std::fs::write(&path, content)?;
        }

        Ok(())
    }

    /// Get a profile by name.
    pub fn get(&self, name: &str) -> Option<&Profile> {
        self.profiles.get(name)
    }

    /// Add or update a profile.
    pub fn set(&mut self, profile: Profile) -> Result<()> {
        self.save(&profile)?;
        self.profiles.insert(profile.name.clone(), profile);
        Ok(())
    }

    /// Delete a profile.
    pub fn delete(&mut self, name: &str) -> Result<()> {
        let filename = Self::sanitize_filename(name);
        let path = self.profiles_dir.join(format!("{}.json", filename));
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        self.profiles.remove(name);
        Ok(())
    }

    /// List all profile names.
    pub fn list(&self) -> Vec<&str> {
        self.profiles.keys().map(|s| s.as_str()).collect()
    }

    /// Get all profiles.
    pub fn all(&self) -> &HashMap<String, Profile> {
        &self.profiles
    }
}

// Note: `Default` is intentionally not implemented for `ProfileManager`
// because creating a manager may fail at runtime (e.g., filesystem issues).
// Callers should explicitly use `ProfileManager::new()` and handle the
// returned `Result` instead of relying on `T::default()`.

#[cfg(test)]
mod tests;
