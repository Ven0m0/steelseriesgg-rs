//! Device state persistence for tracking current device settings.
//!
//! Since SteelSeries devices generally don't expose complete "read back current settings"
//! functionality, this module maintains a store of the last-applied settings keyed by
//! device identity. We prefer stable identifiers (path/interface) when serial numbers
//! are missing to avoid collisions across multiple identical devices.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use crate::config::Config;
use crate::devices::DeviceInfo;
use crate::rgb::Effect;
use crate::{Error, Result};

/// Hash a device path to a stable 32-bit value for serialization.
fn hash_path(path: &str) -> u32 {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    hasher.finish() as u32
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

fn effects_equal(a: &Effect, b: &Effect) -> bool {
    use Effect::*;

    match (a, b) {
        (Static { color: c1 }, Static { color: c2 }) => c1 == c2,
        (
            Breathing {
                color: c1,
                speed: s1,
            },
            Breathing {
                color: c2,
                speed: s2,
            },
        ) => c1 == c2 && s1 == s2,
        (Spectrum { speed: s1 }, Spectrum { speed: s2 }) => s1 == s2,
        (
            Wave {
                colors: c1,
                speed: s1,
                direction: d1,
            },
            Wave {
                colors: c2,
                speed: s2,
                direction: d2,
            },
        ) => c1 == c2 && s1 == s2 && d1 == d2,
        (
            Reactive {
                color: c1,
                duration: d1,
            },
            Reactive {
                color: c2,
                duration: d2,
            },
        ) => c1 == c2 && d1 == d2,
        (Gradient { start: s1, end: e1 }, Gradient { start: s2, end: e2 }) => s1 == s2 && e1 == e2,
        (Custom { colors: c1 }, Custom { colors: c2 }) => c1 == c2,
        (Off, Off) => true,
        _ => false,
    }
}

fn keyboard_states_equal(a: &KeyboardState, b: &KeyboardState) -> bool {
    a.brightness == b.brightness && effects_equal(&a.effect, &b.effect)
}

fn headset_states_equal(a: &HeadsetState, b: &HeadsetState) -> bool {
    a.sidetone == b.sidetone
        && a.mic_volume == b.mic_volume
        && a.mic_muted == b.mic_muted
        && a.eq_preset == b.eq_preset
        && a.auto_off_minutes == b.auto_off_minutes
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
#[derive(Clone, Debug, Deserialize, Serialize)]
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
#[derive(Clone, Debug, Deserialize, Serialize)]
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
        let map = states
            .iter()
            .map(|(id, state)| (id.to_key(), state.clone()))
            .collect();
        SerializableStates(map)
    }
}

/// Manager for device state persistence.
pub struct DeviceStateStore {
    states: HashMap<DeviceId, DeviceState>,
    state_file: PathBuf,
}

impl DeviceStateStore {
    /// Create a new device state store.
    pub fn new() -> Result<Self> {
        let state_file = Config::config_dir()
            .ok_or_else(|| {
                Error::InvalidConfig("could not determine config directory".to_string())
            })?
            .join("device_state.json");

        if let Some(parent) = state_file.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut store = Self {
            states: HashMap::new(),
            state_file,
        };

        // Load existing state if available
        if store.state_file.exists() {
            store.load()?;
        }

        Ok(store)
    }

    /// Load device states from disk.
    fn load(&mut self) -> Result<()> {
        let content = std::fs::read_to_string(&self.state_file)?;

        // Try new format first (string-keyed map)
        if let Ok(serializable) = serde_json::from_str::<SerializableStates>(&content) {
            // Convert back to DeviceId-keyed map
            self.states = serializable
                .0
                .into_iter()
                .filter_map(|(key, state)| DeviceId::from_key(&key).ok().map(|id| (id, state)))
                .collect();
            return Ok(());
        }

        // Try legacy format (direct HashMap<DeviceId, DeviceState>) for backward compat
        // This will likely fail on current broken files, which is expected
        self.states = serde_json::from_str(&content)?;
        Ok(())
    }

    /// Save device states to disk.
    fn save(&self) -> Result<()> {
        let serializable = SerializableStates::from(&self.states);
        let content = serde_json::to_string_pretty(&serializable)?;
        std::fs::write(&self.state_file, content)?;
        Ok(())
    }

    /// Get the state for a device.
    pub fn get(&self, id: &DeviceId) -> Option<&DeviceState> {
        if let Some(state) = self.states.get(id) {
            return Some(state);
        }

        self.states
            .iter()
            .find(|(existing, _)| Self::id_loosely_matches(existing, id))
            .map(|(_, state)| state)
    }

    /// Get mutable reference to device state.
    pub fn get_mut(&mut self, id: &DeviceId) -> Option<&mut DeviceState> {
        // Try exact match first (single lookup)
        if let Some(state) = self.states.get_mut(id) {
            return Some(state);
        }

        // Fall back to loose matching
        let key = self
            .states
            .keys()
            .find(|existing| Self::id_loosely_matches(existing, id))
            .cloned();

        key.and_then(move |k| self.states.get_mut(&k))
    }

    /// Get or create device state.
    pub fn get_or_create(&mut self, id: DeviceId) -> &mut DeviceState {
        // Check if exact match exists
        if self.states.contains_key(&id) {
            // Safe to unwrap as we just checked for the key's existence.
            return self.states.get_mut(&id).unwrap();
        }

        // Fall back to legacy identifiers (pre-path) to retain previously saved state
        let existing_key = self
            .states
            .keys()
            .find(|existing| Self::id_loosely_matches(existing, &id))
            .cloned();

        if let Some(key) = existing_key {
            // Safe unwrap: we just found this key exists
            return self.states.get_mut(&key).unwrap();
        }

        self.states.entry(id).or_default()
    }

    /// Update keyboard state for a device.
    pub fn update_keyboard(&mut self, id: DeviceId, keyboard: KeyboardState) -> Result<()> {
        let state = self.get_or_create(id);
        if state
            .keyboard
            .as_ref()
            .map(|existing| keyboard_states_equal(existing, &keyboard))
            .unwrap_or(false)
        {
            return Ok(());
        }

        state.keyboard = Some(keyboard);
        self.save()
    }

    /// Update headset state for a device.
    pub fn update_headset(&mut self, id: DeviceId, headset: HeadsetState) -> Result<()> {
        let state = self.get_or_create(id);
        if state
            .headset
            .as_ref()
            .map(|existing| headset_states_equal(existing, &headset))
            .unwrap_or(false)
        {
            return Ok(());
        }

        state.headset = Some(headset);
        self.save()
    }

    /// Update keyboard effect for a device.
    pub fn update_keyboard_effect(&mut self, id: DeviceId, effect: Effect) -> Result<()> {
        let state = self.get_or_create(id);
        if let Some(ref mut keyboard) = state.keyboard {
            if effects_equal(&keyboard.effect, &effect) {
                return Ok(());
            }
            keyboard.effect = effect;
        } else {
            state.keyboard = Some(KeyboardState {
                effect,
                ..Default::default()
            });
        }
        self.save()
    }

    /// Update keyboard brightness for a device.
    pub fn update_keyboard_brightness(&mut self, id: DeviceId, brightness: u8) -> Result<()> {
        let state = self.get_or_create(id);
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
        self.save()
    }

    /// List all devices with stored state.
    pub fn list_devices(&self) -> Vec<&DeviceId> {
        self.states.keys().collect()
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
