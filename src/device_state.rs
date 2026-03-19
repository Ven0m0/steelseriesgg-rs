//! Device state persistence for tracking current device settings.
//!
//! Since SteelSeries devices generally don't expose complete "read back current settings"
//! functionality, this module maintains a store of the last-applied settings keyed by
//! device identity. We prefer stable identifiers (path/interface) when serial numbers
//! are missing to avoid collisions across multiple identical devices.
//!
//! This module provides async device state persistence with write-behind caching to prevent
//! blocking RGB updates during high-frequency operations.

use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::Hash;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, error, trace};

use crate::config::Config;
use crate::devices::DeviceInfo;
use crate::rgb::Effect;
use crate::{Error, Result};

/// Hash a device path to a stable 32-bit value for serialization.
/// Uses a simple FNV-like hash for better performance than DefaultHasher.
#[inline]
fn hash_path(path: &str) -> u32 {
    const FNV_OFFSET_BASIS: u32 = 2166136261; // FNV offset basis
    const FNV_PRIME: u32 = 16777619; // FNV prime

    let mut hash = FNV_OFFSET_BASIS;
    for byte in path.bytes() {
        hash = hash.wrapping_mul(FNV_PRIME);
        hash ^= byte as u32;
    }
    hash
}

/// Unique identifier for a device.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub struct DeviceId {
    pub vendor_id: u16,
    pub product_id: u16,
    #[serde(default)]
    pub interface_number: i32,
    pub serial_number: Option<String>,
    /// HID path (if available); optional for backward compatibility with older state files.
    #[serde(default)]
    pub path: Option<String>,
}

impl DeviceId {
    /// Convert DeviceId to a stable string key for JSON serialization.
    ///
    /// Format: `{vendor_id:04x}:{product_id:04x}:{interface}:{serial}:{path_hash}`
    pub fn to_key(&self) -> String {
        let serial = self.serial_number.as_deref().unwrap_or("none");
        let path_hash = self
            .path
            .as_ref()
            .map(|p| format!("{:08x}", hash_path(p)))
            .unwrap_or_else(|| "none".to_string());

        format!(
            "{:04x}:{:04x}:{}:{}:{}",
            self.vendor_id, self.product_id, self.interface_number, serial, path_hash
        )
    }

    /// Parse a DeviceId from a string key (reverse of to_key).
    pub fn from_key(key: &str) -> Result<Self> {
        let parts: Vec<&str> = key.split(':').collect();
        if parts.len() != 5 {
            return Err(Error::InvalidConfig(format!("Invalid device key: {}", key)));
        }

        Ok(Self {
            vendor_id: u16::from_str_radix(parts[0], 16)
                .map_err(|e| Error::InvalidConfig(format!("Invalid vendor_id: {}", e)))?,
            product_id: u16::from_str_radix(parts[1], 16)
                .map_err(|e| Error::InvalidConfig(format!("Invalid product_id: {}", e)))?,
            interface_number: parts[2]
                .parse()
                .map_err(|e| Error::InvalidConfig(format!("Invalid interface: {}", e)))?,
            serial_number: if parts[3] == "none" {
                None
            } else {
                Some(parts[3].to_string())
            },
            path: if parts[4] == "none" {
                None
            } else {
                // Can't recover original path from hash, so keep it as None
                None
            },
        })
    }
}

impl From<&DeviceInfo> for DeviceId {
    fn from(info: &DeviceInfo) -> Self {
        Self {
            vendor_id: info.vendor_id,
            product_id: info.product_id,
            interface_number: info.interface_number,
            serial_number: info.serial_number.clone(),
            path: Some(info.path.clone()),
        }
    }
}

/// Keyboard device state.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct KeyboardState {
    /// Current RGB effect.
    pub effect: Effect,
    /// Current brightness (0-100).
    pub brightness: u8,
}

impl Default for KeyboardState {
    fn default() -> Self {
        Self {
            effect: Effect::Static {
                color: crate::rgb::Color::WHITE,
            },
            brightness: 100,
        }
    }
}

/// Headset device state.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct HeadsetState {
    /// Sidetone level (0-100).
    pub sidetone: u8,
    /// Microphone volume (0-100).
    pub mic_volume: u8,
    /// Microphone muted.
    pub mic_muted: bool,
    /// EQ preset name.
    pub eq_preset: String,
    /// Auto-off timeout in minutes.
    pub auto_off_minutes: u8,
}

impl Default for HeadsetState {
    fn default() -> Self {
        Self {
            sidetone: 50,
            mic_volume: 100,
            mic_muted: false,
            eq_preset: "Flat".to_string(),
            auto_off_minutes: 15,
        }
    }
}

/// Aggregate device state.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct DeviceState {
    pub keyboard: Option<KeyboardState>,
    pub headset: Option<HeadsetState>,
}

/// Wrapper for serializing HashMap<DeviceId, DeviceState> to JSON with string keys.
#[derive(Deserialize, Serialize)]
#[serde(transparent)]
struct SerializableStates(HashMap<String, DeviceState>);

impl From<&HashMap<DeviceId, DeviceState>> for SerializableStates {
    fn from(states: &HashMap<DeviceId, DeviceState>) -> Self {
        let map = states.iter().map(|(id, state)| (id.to_key(), state.clone())).collect();
        SerializableStates(map)
    }
}

/// Manager for device state persistence with async write-behind caching.
pub struct DeviceStateStore {
    states: Arc<RwLock<HashMap<DeviceId, DeviceState>>>,
    state_file: PathBuf,
    dirty_flag: Arc<Mutex<bool>>,
    write_behind_handle: Option<tokio::task::JoinHandle<()>>,
}
impl Drop for DeviceStateStore {
    fn drop(&mut self) {
        if let Some(handle) = self.write_behind_handle.take() {
            handle.abort();
        }
    }
}

impl DeviceStateStore {
    /// Create a new device state store with async persistence.
    pub fn new() -> Result<Self> {
        let state_file = Config::config_dir()
            .ok_or_else(|| Error::InvalidConfig("could not determine config directory".to_string()))?
            .join("device_state.json");

        Self::with_path(state_file)
    }

    /// Create a new device state store with a specific path (useful for testing).
    pub fn with_path(state_file: PathBuf) -> Result<Self> {
        if let Some(parent) = state_file.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let states = Arc::new(RwLock::new(HashMap::new()));
        let dirty_flag = Arc::new(Mutex::new(false));

        let mut store = Self {
            states: Arc::clone(&states),
            state_file: state_file.clone(),
            dirty_flag: Arc::clone(&dirty_flag),
            write_behind_handle: None,
        };

        // Load existing state if available
        if store.state_file.exists() {
            store.load_sync()?;
        }

        // Start the write-behind background task
        let write_handle = Self::start_write_behind_task(states, state_file, dirty_flag);
        store.write_behind_handle = Some(write_handle);

        debug!("DeviceStateStore initialized with async persistence");
        Ok(store)
    }

    /// Start background task for write-behind caching with 5-second flush interval.
    fn start_write_behind_task(
        states: Arc<RwLock<HashMap<DeviceId, DeviceState>>>,
        state_file: PathBuf,
        dirty_flag: Arc<Mutex<bool>>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(5));

            loop {
                interval.tick().await;

                let should_write = {
                    let mut dirty = dirty_flag.lock();
                    if *dirty {
                        *dirty = false;
                        true
                    } else {
                        false
                    }
                };

                if should_write {
                    let result = Self::save_async(&states, &state_file).await;

                    if let Err(e) = result {
                        error!("Failed to write device state to disk: {}", e);
                    } else {
                        trace!("Device state persisted to disk asynchronously");
                    }
                }
            }
        })
    }

    /// Async save operation with atomic write using temp file.
    async fn save_async(
        states: &Arc<RwLock<HashMap<DeviceId, DeviceState>>>,
        state_file: &std::path::Path,
    ) -> Result<()> {
        let serializable = {
            let states_guard = states.read();
            SerializableStates::from(&*states_guard)
        }; // Lock released here

        let content = serde_json::to_string_pretty(&serializable)
            .map_err(|e| Error::SerializationError(format!("Failed to serialize state: {}", e)))?;

        // Use atomic write with temp file to prevent corruption
        let temp_file = state_file.with_extension("tmp");

        // Use blocking file operations in a spawn_blocking task
        let temp_file_clone = temp_file.clone();
        let state_file_clone = state_file.to_path_buf();

        tokio::task::spawn_blocking(move || -> Result<()> {
            std::fs::write(&temp_file_clone, content)
                .map_err(|e| Error::FileSystemError(format!("Failed to write temp file: {}", e)))?;

            std::fs::rename(&temp_file_clone, &state_file_clone)
                .map_err(|e| Error::FileSystemError(format!("Failed to rename temp file: {}", e)))?;

            Ok(())
        })
        .await
        .map_err(|e| Error::Other(format!("Task join error: {}", e)))??;

        Ok(())
    }

    /// Load device states from disk synchronously (used during initialization).
    fn load_sync(&mut self) -> Result<()> {
        let content = std::fs::read_to_string(&self.state_file)?;

        // Try new format first (string-keyed map)
        let loaded_states = if let Ok(serializable) = serde_json::from_str::<SerializableStates>(&content) {
            // Convert back to DeviceId-keyed map
            serializable
                .0
                .into_iter()
                .filter_map(|(key, state)| DeviceId::from_key(&key).ok().map(|id| (id, state)))
                .collect()
        } else {
            // Try legacy format (direct HashMap<DeviceId, DeviceState>) for backward compat
            // This will likely fail on current broken files, which is expected
            serde_json::from_str(&content)?
        };

        // Update the async states
        *self.states.write() = loaded_states;
        debug!("Loaded device states from disk synchronously");
        Ok(())
    }

    /// Mark state as dirty for async persistence.
    /// This is non-blocking and triggers background write-behind.
    fn mark_dirty(&self) {
        if let Some(mut dirty) = self.dirty_flag.try_lock() {
            *dirty = true;
        } else {
            // If lock is contended, spawn async task to set dirty flag
            let dirty_flag = Arc::clone(&self.dirty_flag);
            tokio::spawn(async move {
                let mut dirty = dirty_flag.lock();
                *dirty = true;
            });
        }
    }

    /// Force immediate async save (for shutdown scenarios).
    pub async fn save(&self) -> Result<()> {
        Self::save_async(&self.states, &self.state_file).await
    }

    /// Get the state for a device.
    pub fn get(&self, id: &DeviceId) -> Option<DeviceState> {
        let states = self.states.read();

        if let Some(state) = states.get(id) {
            return Some(state.clone());
        }

        states
            .iter()
            .find(|(existing, _)| Self::id_loosely_matches(existing, id))
            .map(|(_, state)| state.clone())
    }

    /// Get or create device state.
    pub async fn get_or_create(&self, id: DeviceId) -> DeviceState {
        let mut states = self.states.write();

        // Check if we need to migrate a legacy key first
        let legacy_key = if !states.contains_key(&id) {
            // Look for legacy identifiers (pre-path) to retain previously saved state
            states
                .keys()
                .find(|existing| Self::id_loosely_matches(existing, &id))
                .cloned()
        } else {
            None
        };

        // If we found a legacy key, migrate it to the new key
        if let Some(key) = legacy_key {
            if let Some(existing_state) = states.remove(&key) {
                states.insert(id.clone(), existing_state);
            }
        }

        // Now safely get or create the state
        states.entry(id).or_default().clone()
    }

    /// Update keyboard state for a device.
    pub fn update_keyboard(&self, id: DeviceId, keyboard: KeyboardState) -> Result<()> {
        let mut states = self.states.write();

        // Get or create the device state
        let state = states.entry(id).or_default();

        // Check if update is needed
        if state
            .keyboard
            .as_ref()
            .map(|existing| existing == &keyboard)
            .unwrap_or(false)
        {
            return Ok(());
        }

        state.keyboard = Some(keyboard);
        drop(states); // Release lock before marking dirty

        // Mark for async persistence
        self.mark_dirty();
        Ok(())
    }

    /// Update headset state for a device.
    pub fn update_headset(&self, id: DeviceId, headset: HeadsetState) -> Result<()> {
        let mut states = self.states.write();

        let state = states.entry(id).or_default();

        if state
            .headset
            .as_ref()
            .map(|existing| existing == &headset)
            .unwrap_or(false)
        {
            return Ok(());
        }

        state.headset = Some(headset);
        drop(states);

        self.mark_dirty();
        Ok(())
    }

    /// Update keyboard effect for a device.
    pub fn update_keyboard_effect(&self, id: DeviceId, effect: Effect) -> Result<()> {
        let mut states = self.states.write();

        let state = states.entry(id).or_default();

        if let Some(ref mut keyboard) = state.keyboard {
            if keyboard.effect == effect {
                return Ok(());
            }
            keyboard.effect = effect;
        } else {
            state.keyboard = Some(KeyboardState {
                effect,
                ..Default::default()
            });
        }

        drop(states);
        self.mark_dirty();
        Ok(())
    }

    /// Update keyboard brightness for a device.
    pub fn update_keyboard_brightness(&self, id: DeviceId, brightness: u8) -> Result<()> {
        let mut states = self.states.write();

        let state = states.entry(id).or_default();

        if let Some(ref mut keyboard) = state.keyboard {
            if keyboard.brightness == brightness {
                return Ok(());
            }
            keyboard.brightness = brightness;
        } else {
            state.keyboard = Some(KeyboardState {
                brightness,
                ..Default::default()
            });
        }

        drop(states);
        self.mark_dirty();
        Ok(())
    }

    /// List all devices with stored state.
    pub fn list_devices(&self) -> Vec<DeviceId> {
        let states = self.states.read();
        states.keys().cloned().collect()
    }

    /// Shutdown the store and flush pending writes.
    pub async fn shutdown(&mut self) -> Result<()> {
        // Cancel the write-behind task
        if let Some(handle) = self.write_behind_handle.take() {
            handle.abort();
        }

        // Force one final write if there are pending changes
        let is_dirty = {
            let dirty = self.dirty_flag.lock();
            *dirty
        };

        if is_dirty {
            self.save().await?;
        }

        debug!("DeviceStateStore shut down successfully");
        Ok(())
    }

    /// Compare device identifiers while tolerating missing path information to keep
    /// backward compatibility with previously persisted state.
    ///
    /// Interface number 0 is treated as a wildcard to support legacy state files that
    /// were saved before interface numbers were properly tracked (pre-path era).
    fn id_loosely_matches(existing: &DeviceId, candidate: &DeviceId) -> bool {
        let interface_matches = existing.interface_number == 0
            || candidate.interface_number == 0
            || existing.interface_number == candidate.interface_number;

        existing.vendor_id == candidate.vendor_id
            && existing.product_id == candidate.product_id
            && interface_matches
            && existing.serial_number == candidate.serial_number
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_persistence() -> Result<()> {
        let dir = tempdir().unwrap();
        let state_file = dir.path().join("device_state.json");

        let store = DeviceStateStore::with_path(state_file.clone())?;

        let device_id = DeviceId {
            vendor_id: 0x1038,
            product_id: 0x1234,
            interface_number: 1,
            serial_number: Some("serial123".to_string()),
            path: Some("path/to/device".to_string()),
        };

        let keyboard_state = KeyboardState {
            effect: Effect::Static {
                color: crate::rgb::Color::WHITE,
            },
            brightness: 80,
        };

        store.update_keyboard(device_id.clone(), keyboard_state.clone())?;

        // Force save
        store.save().await?;

        // Create new store and verify load
        let store2 = DeviceStateStore::with_path(state_file.clone())?;
        let loaded_state = store2.get(&device_id).expect("Should have state");

        assert_eq!(loaded_state.keyboard.unwrap(), keyboard_state);

        Ok(())
    }

    #[tokio::test]
    async fn test_write_behind() -> Result<()> {
        let dir = tempdir().unwrap();
        let state_file = dir.path().join("device_state.json");

        let store = DeviceStateStore::with_path(state_file.clone())?;

        let device_id = DeviceId {
            vendor_id: 0x1038,
            product_id: 0x5678,
            interface_number: 0,
            serial_number: None,
            path: None,
        };

        let headset_state = HeadsetState {
            sidetone: 75,
            mic_volume: 90,
            mic_muted: true,
            eq_preset: "Bass Boost".to_string(),
            auto_off_minutes: 30,
        };

        store.update_headset(device_id.clone(), headset_state.clone())?;

        // Check dirty flag
        {
            let dirty = store.dirty_flag.lock();
            assert!(*dirty, "Dirty flag should be set after update");
        }

        // Force save to simulate write-behind execution and verify persistence
        store.save().await?;

        let content = std::fs::read_to_string(&state_file)?;
        assert!(content.contains("Bass Boost"));

        Ok(())
    }

    #[tokio::test]
    async fn test_legacy_migration() -> Result<()> {
        let dir = tempdir().unwrap();
        let state_file = dir.path().join("device_state.json");

        // Pre-populate with legacy state (no path/interface)
        let legacy_id_str = "1038:1111:0:serial:none";
        let initial_json = format!(
            r#"{{
                "{}": {{
                    "keyboard": {{
                        "effect": {{ "Static": {{ "color": {{ "r": 255, "g": 255, "b": 255 }} }} }},
                        "brightness": 100
                    }},
                    "headset": null
                }}
            }}"#,
            legacy_id_str
        );

        std::fs::write(&state_file, initial_json)?;

        let store = DeviceStateStore::with_path(state_file.clone())?;

        // New device ID with path/interface
        let new_device_id = DeviceId {
            vendor_id: 0x1038,
            product_id: 0x1111,
            interface_number: 1,
            serial_number: Some("serial".to_string()),
            path: Some("new/path".to_string()),
        };

        // Verify migration
        let state = store.get_or_create(new_device_id.clone()).await;
        assert!(state.keyboard.is_some(), "Should have found legacy state");

        Ok(())
    }

    #[tokio::test]
    async fn test_update_keyboard_effect() -> Result<()> {
        let dir = tempdir().unwrap();
        let state_file = dir.path().join("device_state.json");
        let store = DeviceStateStore::with_path(state_file)?;

        let device_id = DeviceId {
            vendor_id: 0x1038,
            product_id: 0x1234,
            interface_number: 1,
            serial_number: Some("serial123".to_string()),
            path: Some("path/to/device".to_string()),
        };

        let effect = Effect::Static {
            color: crate::rgb::Color::RED,
        };

        // 1. New device - should create KeyboardState
        store.update_keyboard_effect(device_id.clone(), effect.clone())?;
        {
            let states = store.states.read();
            let state = states.get(&device_id).unwrap();
            let keyboard = state.keyboard.as_ref().unwrap();
            assert_eq!(keyboard.effect, effect);
            assert_eq!(keyboard.brightness, 100); // Default brightness

            let dirty = store.dirty_flag.lock();
            assert!(*dirty, "Should be dirty after new device update");
        }

        // Reset dirty flag
        {
            let mut dirty = store.dirty_flag.lock();
            *dirty = false;
        }

        // 2. Existing device - should update effect
        let new_effect = Effect::Spectrum { speed: 1.0 };
        store.update_keyboard_effect(device_id.clone(), new_effect.clone())?;
        {
            let states = store.states.read();
            let state = states.get(&device_id).unwrap();
            assert_eq!(state.keyboard.as_ref().unwrap().effect, new_effect);

            let dirty = store.dirty_flag.lock();
            assert!(*dirty, "Should be dirty after effect change");
        }

        // Reset dirty flag
        {
            let mut dirty = store.dirty_flag.lock();
            *dirty = false;
        }

        // 3. Same effect - should be no-op
        store.update_keyboard_effect(device_id.clone(), new_effect.clone())?;
        {
            let dirty = store.dirty_flag.lock();
            assert!(!*dirty, "Should NOT be dirty after same effect update");
        }

        Ok(())
    }

    #[test]
    fn test_device_id_from_key_invalid() {
        // Wrong number of parts
        assert!(DeviceId::from_key("1038:1234:1:serial").is_err());
        assert!(DeviceId::from_key("1038:1234:1:serial:none:extra").is_err());

        // Invalid hex vendor_id
        assert!(DeviceId::from_key("103G:1234:1:serial:none").is_err());

        // Invalid hex product_id
        assert!(DeviceId::from_key("1038:XXXX:1:serial:none").is_err());

        // Invalid interface integer
        assert!(DeviceId::from_key("1038:1234:A:serial:none").is_err());
    }

    #[tokio::test]
    async fn test_update_keyboard_brightness() -> Result<()> {
        let dir = tempdir().unwrap();
        let state_file = dir.path().join("device_state.json");
        let store = DeviceStateStore::with_path(state_file)?;

        let device_id = DeviceId {
            vendor_id: 0x1038,
            product_id: 0x1234,
            interface_number: 1,
            serial_number: Some("serial123".to_string()),
            path: Some("path/to/device".to_string()),
        };

        // 1. New device - should create KeyboardState
        store.update_keyboard_brightness(device_id.clone(), 50)?;
        {
            let states = store.states.read();
            let state = states.get(&device_id).unwrap();
            let keyboard = state.keyboard.as_ref().unwrap();
            assert_eq!(keyboard.brightness, 50);
            assert_eq!(keyboard.effect, Effect::default());

            let dirty = store.dirty_flag.lock();
            assert!(*dirty, "Should be dirty after new device update");
        }

        // Reset dirty flag
        {
            let mut dirty = store.dirty_flag.lock();
            *dirty = false;
        }

        // 2. Existing device - should update brightness
        store.update_keyboard_brightness(device_id.clone(), 75)?;
        {
            let states = store.states.read();
            let state = states.get(&device_id).unwrap();
            assert_eq!(state.keyboard.as_ref().unwrap().brightness, 75);

            let dirty = store.dirty_flag.lock();
            assert!(*dirty, "Should be dirty after brightness change");
        }

        // Reset dirty flag
        {
            let mut dirty = store.dirty_flag.lock();
            *dirty = false;
        }

        // 3. Same brightness - should be no-op
        store.update_keyboard_brightness(device_id.clone(), 75)?;
        {
            let dirty = store.dirty_flag.lock();
            assert!(!*dirty, "Should NOT be dirty after same brightness update");
        }

        Ok(())
    }
}
