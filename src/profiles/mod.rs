//! Profile management for saving and loading device configurations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(unix)]
use std::fs::OpenOptions;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
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
    pub async fn new() -> Result<Self> {
        let profiles_dir = Config::config_dir()
            .ok_or_else(|| Error::Profile("could not determine config directory".to_string()))?
            .join("profiles");

        #[cfg(unix)]
        {
            if profiles_dir.exists() {
                let metadata = std::fs::symlink_metadata(&profiles_dir)?;
                let file_type = metadata.file_type();
                if file_type.is_symlink() || !metadata.is_dir() {
                    return Err(Error::Profile(
                        "profiles path exists but is not a directory".to_string(),
                    ));
                }

                let mut perms = metadata.permissions();
                perms.set_mode(0o700);
                std::fs::set_permissions(&profiles_dir, perms)?;
            } else {
                std::fs::DirBuilder::new()
                    .recursive(true)
                    .mode(0o700)
                    .create(&profiles_dir)?;
            }
        }
        #[cfg(not(unix))]
        std::fs::create_dir_all(&profiles_dir)?;

        let mut manager = Self {
            profiles: HashMap::new(),
            profiles_dir,
        };

        manager.load_all().await?;
        Ok(manager)
    }

    /// Create a test profile manager with a specific directory.
    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn with_dir(dir: PathBuf) -> Self {
        Self {
            profiles: HashMap::new(),
            profiles_dir: dir,
        }
    }

    /// Load all profiles from disk.
    pub async fn load_all(&mut self) -> Result<()> {
        self.profiles.clear();

        let exists = tokio::fs::try_exists(&self.profiles_dir).await?;
        if !exists {
            return Ok(());
        }

        let mut dir = tokio::fs::read_dir(&self.profiles_dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();

            if path.extension().map(|e| e == "json").unwrap_or(false) {
                match tokio::fs::read_to_string(&path).await {
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
            let mut options = OpenOptions::new();
            options.write(true).create(true).truncate(true).mode(0o600);

            let file = crate::fs_utils::secure_open(&path, &options).map_err(|e| Error::Profile(e.to_string()))?;
            let mut perms = file.metadata()?.permissions();
            perms.set_mode(0o600);
            file.set_permissions(perms)?;
            std::io::Write::write_all(&mut &file, content.as_bytes())?;
        }
        #[cfg(not(unix))]
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Get a profile by name.
    pub fn get(&self, name: &str) -> Option<&Profile> {
        self.profiles.get(name)
    }

    /// Add or update a profile.
    pub fn set(&mut self, profile: Profile) -> Result<&Profile> {
        self.save(&profile)?;
        let name = profile.name.clone();
        use std::collections::hash_map::Entry;
        let profile_ref = match self.profiles.entry(name) {
            Entry::Occupied(mut e) => {
                e.insert(profile);
                e.into_mut()
            }
            Entry::Vacant(e) => e.insert(profile),
        };
        Ok(profile_ref)
    }

    /// Delete a profile.
    pub fn delete(&mut self, name: &str) -> Result<()> {
        let filename = Self::sanitize_filename(name);
        let path = self.profiles_dir.join(format!("{}.json", filename));
        match std::fs::remove_file(&path) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        self.profiles.remove(name);
        Ok(())
    }

    /// Delete a profile asynchronously.
    pub async fn delete_async(&mut self, name: &str) -> Result<()> {
        let filename = Self::sanitize_filename(name);
        let path = self.profiles_dir.join(format!("{}.json", filename));
        match tokio::fs::remove_file(&path).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
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
mod tests {
    use super::*;
    use crate::rgb::{Color, Effect};

    #[test]
    fn test_profile_creation() {
        let profile = Profile::new("test");
        assert_eq!(profile.name, "test");
        assert!(profile.description.is_none());
        assert!(profile.keyboard.is_none());
        assert!(profile.headset.is_none());
    }

    #[test]
    fn test_profile_serialization() {
        let mut profile = Profile::new("Gaming");
        profile.description = Some("My gaming profile".to_string());
        profile.keyboard = Some(KeyboardProfile {
            effect: Effect::Static { color: Color::RED },
            brightness: 80,
        });

        let json = serde_json::to_string(&profile).unwrap();
        let deserialized: Profile = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, "Gaming");
        assert_eq!(deserialized.description.as_deref(), Some("My gaming profile"));
        assert!(deserialized.keyboard.is_some());
        assert_eq!(deserialized.keyboard.as_ref().unwrap().brightness, 80);
    }

    #[test]
    fn test_profile_filename_sanitization() {
        let dangerous_names = vec![
            "test/profile",
            "test\\profile",
            "test:profile",
            "test*profile",
            "test?profile",
            "test\"profile",
            "test<profile",
            "test>profile",
            "test|profile",
        ];

        for name in dangerous_names {
            let sanitized = ProfileManager::sanitize_filename(name);
            assert!(!sanitized.contains('/'));
            assert!(!sanitized.contains('\\'));
            assert!(!sanitized.contains(':'));
            assert!(!sanitized.contains('*'));
            assert!(!sanitized.contains('?'));
            assert!(!sanitized.contains('"'));
            assert!(!sanitized.contains('<'));
            assert!(!sanitized.contains('>'));
            assert!(!sanitized.contains('|'));
        }
    }

    #[test]
    fn test_keyboard_profile_default() {
        let profile = KeyboardProfile::default();
        assert_eq!(profile.brightness, 100);
        match profile.effect {
            Effect::Static { color } => assert_eq!(color, Color::WHITE),
            _ => panic!("Expected static white effect"),
        }
    }

    #[test]
    fn test_headset_profile_default() {
        let profile = HeadsetProfile::default();
        assert_eq!(profile.sidetone, 50);
        assert_eq!(profile.mic_volume, 100);
        assert_eq!(profile.eq_preset, "Flat");
        assert_eq!(profile.auto_off_minutes, 15);
    }

    #[test]
    fn test_profile_with_both_devices() {
        let mut profile = Profile::new("Complete");
        profile.keyboard = Some(KeyboardProfile {
            effect: Effect::Spectrum { speed: 1.0 },
            brightness: 75,
        });
        profile.headset = Some(HeadsetProfile {
            sidetone: 60,
            mic_volume: 90,
            eq_preset: "Bass Boost".to_string(),
            auto_off_minutes: 30,
        });

        let json = serde_json::to_string_pretty(&profile).unwrap();
        let deserialized: Profile = serde_json::from_str(&json).unwrap();

        assert!(deserialized.keyboard.is_some());
        assert!(deserialized.headset.is_some());
        assert_eq!(deserialized.headset.as_ref().unwrap().sidetone, 60);
    }

    #[test]
    #[cfg(unix)]
    fn test_secure_profile_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = tempfile::tempdir().unwrap();
        let profiles_dir = temp_dir.path().join("profiles");

        if !profiles_dir.exists() {
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&profiles_dir)
                .unwrap();
        }

        let dir_metadata = std::fs::symlink_metadata(&profiles_dir).unwrap();
        assert_eq!(dir_metadata.permissions().mode() & 0o777, 0o700);

        let manager = ProfileManager::with_dir(profiles_dir.clone());
        let profile = Profile::new("SecretProfile");
        manager.save(&profile).unwrap();

        let path = profiles_dir.join("SecretProfile.json");
        let file_metadata = std::fs::symlink_metadata(&path).unwrap();
        assert_eq!(file_metadata.permissions().mode() & 0o777, 0o600);
    }

    #[tokio::test]
    #[ignore = "benchmark-style test; run explicitly with `cargo test -- --ignored`"]
    async fn benchmark_profile_deletion() {
        use std::time::Instant;

        let temp_dir = tempfile::tempdir().unwrap();
        let mut manager = ProfileManager::with_dir(temp_dir.path().to_path_buf());

        let num_profiles = 1000;
        for i in 0..num_profiles {
            let profile = Profile::new(format!("Profile_{}", i));
            manager.set(profile).unwrap();
        }

        let start = Instant::now();
        for i in 0..num_profiles {
            manager.delete_async(&format!("Profile_{}", i)).await.unwrap();
        }
        println!("Deleted {} profiles in {:?}", num_profiles, start.elapsed());
    }
}
