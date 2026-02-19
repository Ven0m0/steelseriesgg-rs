use hidapi::HidApi;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, info, warn};

use super::headsets::{GenericHeadset, Headset};
use super::hid_reports::ConnectionHealth;
use super::keyboards::apex::Apex3Tkl;
use super::keyboards::apex_pro_tkl_2023::ApexProTkl2023;
use super::keyboards::{GenericKeyboard, Keyboard};
use super::product_ids::{APEX_3_TKL, APEX_PRO_TKL_2023};
use super::{DeviceInfo, DeviceType, device_name_from_product_id, device_type_from_product_id};
use crate::{Error, Result, STEELSERIES_VENDOR_ID};

/// Device fingerprint for tracking devices across disconnections
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DeviceFingerprint {
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial_number: Option<String>,
    pub interface_number: i32,
}

impl DeviceFingerprint {
    /// Create a fingerprint from device info
    pub fn from_device_info(info: &DeviceInfo) -> Self {
        Self {
            vendor_id: info.vendor_id,
            product_id: info.product_id,
            serial_number: info.serial_number.clone(),
            interface_number: info.interface_number,
        }
    }

    /// Create a unique identifier string
    pub fn to_id(&self) -> String {
        match &self.serial_number {
            Some(serial) => format!(
                "{}:{:04x}:{:04x}:{}",
                self.vendor_id, self.product_id, self.interface_number, serial
            ),
            None => format!(
                "{}:{:04x}:{:04x}:{}",
                self.vendor_id, self.product_id, self.interface_number, "no_serial"
            ),
        }
    }
}

/// Hot-plug event types
#[derive(Debug, Clone)]
pub enum HotPlugEvent {
    /// Device was connected
    DeviceAdded {
        fingerprint: DeviceFingerprint,
        info: DeviceInfo,
        timestamp: Instant,
    },
    /// Device was disconnected
    DeviceRemoved {
        fingerprint: DeviceFingerprint,
        last_seen: Instant,
        timestamp: Instant,
    },
}

/// Hot-plug event callback type
pub type HotPlugCallback = Box<dyn Fn(HotPlugEvent) + Send + Sync>;

/// Device registry entry for tracking device lifecycle
#[derive(Debug, Clone)]
pub struct DeviceRegistryEntry {
    pub info: DeviceInfo,
    pub fingerprint: DeviceFingerprint,
    pub first_seen: Instant,
    pub last_seen: Instant,
    pub connection_count: u32,
    pub health: ConnectionHealth,
}

impl DeviceRegistryEntry {
    /// Create a new registry entry
    pub fn new(info: DeviceInfo, fingerprint: DeviceFingerprint) -> Self {
        let now = Instant::now();
        Self {
            info,
            fingerprint,
            first_seen: now,
            last_seen: now,
            connection_count: 1,
            health: ConnectionHealth::default(),
        }
    }

    /// Update the entry when device is seen again
    pub fn update_seen(&mut self) {
        self.last_seen = Instant::now();
    }

    /// Increment connection count
    pub fn increment_connection_count(&mut self) {
        self.connection_count += 1;
        self.last_seen = Instant::now();
    }
}

/// Hot-plug monitoring configuration
#[derive(Debug, Clone)]
pub struct HotPlugConfig {
    /// Polling interval for device enumeration
    pub poll_interval: Duration,
    /// Maximum rate of event processing (events per second)
    pub max_event_rate: u32,
    /// Enable device fingerprinting for reconnection tracking
    pub enable_fingerprinting: bool,
    /// Debounce time for rapid connect/disconnect cycles
    pub debounce_time: Duration,
}

impl Default for HotPlugConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(2),
            max_event_rate: 10, // 10 events per second max
            enable_fingerprinting: true,
            debounce_time: Duration::from_millis(500),
        }
    }
}

/// Manages connected SteelSeries devices with hot-plug monitoring.
pub struct DeviceManager {
    api: HidApi,
    devices: HashMap<String, DeviceInfo>,
    /// Cache of device paths indexed by (vendor_id, product_id, interface_number) for O(1) lookup
    device_cache: HashMap<(u16, u16, i32), String>,
    /// Device registry for tracking device lifecycle across reconnections
    device_registry: Arc<RwLock<HashMap<String, DeviceRegistryEntry>>>,
    /// Hot-plug monitoring configuration
    hotplug_config: HotPlugConfig,
    /// Hot-plug event callback
    hotplug_callback: Option<Arc<HotPlugCallback>>,
    /// Event debounce tracking
    pending_events: Arc<RwLock<HashMap<DeviceFingerprint, Instant>>>,
}

impl DeviceManager {
    /// Create a new device manager with default hot-plug configuration.
    pub fn new() -> Result<Self> {
        Self::new_with_config(HotPlugConfig::default())
    }

    /// Create a new device manager with custom hot-plug configuration.
    pub fn new_with_config(config: HotPlugConfig) -> Result<Self> {
        let api = HidApi::new()?;
        let mut manager = Self {
            api,
            devices: HashMap::new(),
            device_cache: HashMap::new(),
            device_registry: Arc::new(RwLock::new(HashMap::new())),
            hotplug_config: config,
            hotplug_callback: None,
            pending_events: Arc::new(RwLock::new(HashMap::new())),
        };
        manager.refresh()?;
        Ok(manager)
    }

    /// Refresh the list of connected devices.
    pub fn refresh(&mut self) -> Result<()> {
        // Pre-allocate capacity based on previous discovery to reduce reallocations
        let prev_capacity = self.devices.capacity().max(8); // Minimum reasonable capacity
        self.devices.clear();
        self.devices.reserve(prev_capacity);
        self.device_cache.clear();
        self.device_cache.reserve(prev_capacity);

        self.api.refresh_devices()?;

        for device in self.api.device_list() {
            if device.vendor_id() != STEELSERIES_VENDOR_ID {
                continue;
            }

            let product_id = device.product_id();
            let device_type = device_type_from_product_id(product_id);

            // Use static str and convert to String only once
            let name = device_name_from_product_id(product_id).to_string();

            // Avoid double allocation: to_string_lossy returns Cow, into_owned is more efficient
            let path = device.path().to_string_lossy().into_owned();

            let info = DeviceInfo {
                name,
                device_type,
                vendor_id: device.vendor_id(),
                product_id,
                interface_number: device.interface_number(),
                serial_number: device.serial_number().map(|s: &str| s.to_string()),
                manufacturer: device.manufacturer_string().map(|s: &str| s.to_string()),
                path: path.clone(),
            };

            debug!(
                "Found device: {} (PID: {:#06x}, Interface: {})",
                info.name, info.product_id, info.interface_number
            );

            // Cache the device path for fast lookup
            let cache_key = (device.vendor_id(), device.product_id(), device.interface_number());
            self.device_cache.insert(cache_key, path.clone());

            self.devices.insert(path, info);
        }

        info!("Found {} SteelSeries device(s)", self.devices.len());
        Ok(())
    }

    /// Get all connected devices.
    pub fn devices(&self) -> Vec<&DeviceInfo> {
        self.devices.values().collect()
    }

    /// Get devices of a specific type.
    pub fn devices_by_type(&self, device_type: DeviceType) -> Vec<&DeviceInfo> {
        self.devices.values().filter(|d| d.device_type == device_type).collect()
    }

    /// Get all keyboards.
    pub fn keyboards(&self) -> Vec<&DeviceInfo> {
        self.devices_by_type(DeviceType::Keyboard)
    }

    /// Get all headsets.
    pub fn headsets(&self) -> Vec<&DeviceInfo> {
        self.devices_by_type(DeviceType::Headset)
    }

    /// Get a device by its path.
    pub fn device_by_path(&self, path: &str) -> Option<&DeviceInfo> {
        self.devices.get(path)
    }

    /// Get the first device of a specific type.
    pub fn first_device_of_type(&self, device_type: DeviceType) -> Option<&DeviceInfo> {
        self.devices_by_type(device_type).into_iter().next()
    }

    /// Open a device for communication.
    pub fn open_device(&self, info: &DeviceInfo) -> Result<hidapi::HidDevice> {
        // Find the device with the matching interface for control
        // Most SteelSeries devices use interface 1 for control
        let control_interface = match info.device_type {
            DeviceType::Keyboard => 1,
            DeviceType::Headset => 3,
            DeviceType::Unknown => info.interface_number,
        };

        debug!(
            "Opening device: {} (VID:{:#06x}, PID:{:#06x}, Interface:{})",
            info.name, info.vendor_id, info.product_id, control_interface
        );

        // Use cache for O(1) lookup instead of O(n) iteration
        let cache_key = (info.vendor_id, info.product_id, control_interface);
        if let Some(path) = self.device_cache.get(&cache_key) {
            debug!("Using cached device path: {}", path);
            // Try to open by path directly - convert String to CStr
            use std::ffi::CString;
            let c_path = CString::new(path.as_str())
                .map_err(|e| Error::DeviceCommunication(format!("Invalid device path: {}", e)))?;
            return self.api.open_path(&c_path).map_err(Error::from);
        }

        // Fallback to iteration if not in cache (shouldn't happen after refresh)
        for device in self.api.device_list() {
            if device.vendor_id() == info.vendor_id
                && device.product_id() == info.product_id
                && device.interface_number() == control_interface
            {
                return device.open_device(&self.api).map_err(Error::from);
            }
        }

        Err(Error::DeviceNotFound(format!(
            "{} (interface {})",
            info.name, control_interface
        )))
    }

    /// Get a reference to the HID API.
    pub fn api(&self) -> &HidApi {
        &self.api
    }

    /// Open a keyboard device and return a boxed Keyboard trait object.
    /// This is a convenience wrapper around open_device that returns a properly
    /// initialized keyboard instance.
    pub fn open_keyboard(&self, info: &DeviceInfo) -> Result<Box<dyn Keyboard>> {
        if info.device_type != DeviceType::Keyboard {
            return Err(Error::DeviceCommunication(format!(
                "Device {} is not a keyboard",
                info.name
            )));
        }

        let hid_device = self.open_device(info)?;
        let generic_keyboard = GenericKeyboard::new(info.clone(), hid_device);

        // Wrap in specific implementation if available
        match info.product_id {
            APEX_3_TKL => Ok(Box::new(Apex3Tkl::new(generic_keyboard))),
            APEX_PRO_TKL_2023 => Ok(Box::new(ApexProTkl2023::new(generic_keyboard))),
            _ => Ok(Box::new(generic_keyboard)),
        }
    }

    /// Open a headset device and return a boxed Headset trait object.
    pub fn open_headset(&self, info: &DeviceInfo) -> Result<Box<dyn Headset>> {
        if info.device_type != DeviceType::Headset {
            return Err(Error::DeviceCommunication(format!(
                "Device {} is not a headset",
                info.name
            )));
        }

        let hid_device = self.open_device(info)?;
        Ok(Box::new(GenericHeadset::new(info.clone(), hid_device)))
    }

    // === Hot-plug monitoring methods ===

    /// Set a callback for hot-plug events
    pub fn set_hotplug_callback<F>(&mut self, callback: F)
    where
        F: Fn(HotPlugEvent) + Send + Sync + 'static,
    {
        self.hotplug_callback = Some(Arc::new(Box::new(callback)));
    }

    /// Get the current hot-plug configuration
    pub fn hotplug_config(&self) -> &HotPlugConfig {
        &self.hotplug_config
    }

    /// Update hot-plug configuration
    pub fn set_hotplug_config(&mut self, config: HotPlugConfig) {
        self.hotplug_config = config;
    }

    /// Start hot-plug monitoring in the background
    /// Returns a channel sender that can be used to stop monitoring
    pub async fn start_hotplug_monitoring(&self) -> Result<mpsc::Sender<()>> {
        let (stop_tx, mut stop_rx) = mpsc::channel::<()>(1);

        // Clone necessary data for the monitoring task
        let device_registry = self.device_registry.clone();
        let pending_events = self.pending_events.clone();
        let callback = self.hotplug_callback.clone();
        let config = self.hotplug_config.clone();

        // Create a new HidApi instance for the monitoring thread
        let mut api = HidApi::new()?;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.poll_interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            let mut current_devices: HashMap<DeviceFingerprint, DeviceInfo> = HashMap::new();
            let mut last_event_time = Instant::now();

            debug!(
                "Started hot-plug monitoring with {}ms interval",
                config.poll_interval.as_millis()
            );

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = Self::poll_devices(&mut api, &device_registry, &pending_events,
                                                         &callback, &config, &mut current_devices,
                                                         &mut last_event_time).await {
                            warn!("Error during device polling: {}", e);
                        }
                    }
                    _ = stop_rx.recv() => {
                        debug!("Stopping hot-plug monitoring");
                        break;
                    }
                }
            }
        });

        Ok(stop_tx)
    }

    /// Poll for device changes (internal method for hot-plug monitoring)
    async fn poll_devices(
        api: &mut HidApi,
        device_registry: &Arc<RwLock<HashMap<String, DeviceRegistryEntry>>>,
        pending_events: &Arc<RwLock<HashMap<DeviceFingerprint, Instant>>>,
        callback: &Option<Arc<HotPlugCallback>>,
        config: &HotPlugConfig,
        current_devices: &mut HashMap<DeviceFingerprint, DeviceInfo>,
        last_event_time: &mut Instant,
    ) -> Result<()> {
        // Rate limiting
        let now = Instant::now();
        let time_since_last = now.duration_since(*last_event_time);
        let min_interval = Duration::from_millis(1000 / config.max_event_rate as u64);

        if time_since_last < min_interval {
            return Ok(());
        }

        // Refresh device list
        if let Err(e) = api.refresh_devices() {
            warn!("Failed to refresh HID device list: {}", e);
            return Ok(());
        }

        let mut new_devices: HashMap<DeviceFingerprint, DeviceInfo> = HashMap::new();

        // Enumerate current devices
        for device in api.device_list() {
            if device.vendor_id() != STEELSERIES_VENDOR_ID {
                continue;
            }

            let product_id = device.product_id();
            let device_type = device_type_from_product_id(product_id);
            let name = device_name_from_product_id(product_id).to_string();
            let path = device.path().to_string_lossy().into_owned();

            let info = DeviceInfo {
                name,
                device_type,
                vendor_id: device.vendor_id(),
                product_id,
                interface_number: device.interface_number(),
                serial_number: device.serial_number().map(|s: &str| s.to_string()),
                manufacturer: device.manufacturer_string().map(|s: &str| s.to_string()),
                path,
            };

            let fingerprint = DeviceFingerprint::from_device_info(&info);
            new_devices.insert(fingerprint, info);
        }

        // Process device changes
        let mut events_to_fire = Vec::new();

        // Check for new devices
        for (fingerprint, info) in &new_devices {
            if !current_devices.contains_key(fingerprint) {
                // Check debounce
                if Self::should_debounce_event(pending_events, fingerprint, config).await {
                    continue;
                }

                // Device added
                let event = HotPlugEvent::DeviceAdded {
                    fingerprint: fingerprint.clone(),
                    info: info.clone(),
                    timestamp: now,
                };
                events_to_fire.push(event);

                // Update registry
                Self::update_device_registry_added(device_registry, fingerprint, info).await;

                debug!(
                    "Hot-plug detected device added: {} ({})",
                    info.name,
                    fingerprint.to_id()
                );
            } else {
                // Device still present, update registry
                Self::update_device_registry_seen(device_registry, fingerprint).await;
            }
        }

        // Check for removed devices
        for (fingerprint, info) in current_devices.iter() {
            if !new_devices.contains_key(fingerprint) {
                // Check debounce
                if Self::should_debounce_event(pending_events, fingerprint, config).await {
                    continue;
                }

                // Device removed
                let last_seen = Self::get_device_last_seen(device_registry, fingerprint)
                    .await
                    .unwrap_or(now);

                let event = HotPlugEvent::DeviceRemoved {
                    fingerprint: fingerprint.clone(),
                    last_seen,
                    timestamp: now,
                };
                events_to_fire.push(event);

                debug!(
                    "Hot-plug detected device removed: {} ({})",
                    info.name,
                    fingerprint.to_id()
                );
            }
        }

        // Update current device list
        *current_devices = new_devices;

        // Fire events
        if let Some(ref callback_arc) = callback {
            for event in events_to_fire {
                callback_arc(event);
                *last_event_time = now;
            }
        }

        // Clean up expired pending events
        Self::cleanup_pending_events(pending_events, config).await;

        Ok(())
    }

    /// Check if an event should be debounced
    async fn should_debounce_event(
        pending_events: &Arc<RwLock<HashMap<DeviceFingerprint, Instant>>>,
        fingerprint: &DeviceFingerprint,
        config: &HotPlugConfig,
    ) -> bool {
        let now = Instant::now();
        let mut pending = pending_events.write().await;

        if let Some(&last_event) = pending.get(fingerprint) {
            if now.duration_since(last_event) < config.debounce_time {
                // Update the timestamp to extend debounce
                pending.insert(fingerprint.clone(), now);
                return true;
            }
        }

        // Record this event time
        pending.insert(fingerprint.clone(), now);
        false
    }

    /// Clean up expired pending events
    async fn cleanup_pending_events(
        pending_events: &Arc<RwLock<HashMap<DeviceFingerprint, Instant>>>,
        config: &HotPlugConfig,
    ) {
        let now = Instant::now();
        let mut pending = pending_events.write().await;

        pending.retain(|_, &mut timestamp| now.duration_since(timestamp) < config.debounce_time * 2);
    }

    /// Update device registry when a device is added
    async fn update_device_registry_added(
        device_registry: &Arc<RwLock<HashMap<String, DeviceRegistryEntry>>>,
        fingerprint: &DeviceFingerprint,
        info: &DeviceInfo,
    ) {
        let mut registry = device_registry.write().await;
        let device_id = fingerprint.to_id();

        match registry.get_mut(&device_id) {
            Some(entry) => {
                // Device reconnected
                entry.increment_connection_count();
                entry.info = info.clone(); // Update info in case path changed
            }
            None => {
                // New device
                let entry = DeviceRegistryEntry::new(info.clone(), fingerprint.clone());
                registry.insert(device_id, entry);
            }
        }
    }

    /// Update device registry when a device is seen
    async fn update_device_registry_seen(
        device_registry: &Arc<RwLock<HashMap<String, DeviceRegistryEntry>>>,
        fingerprint: &DeviceFingerprint,
    ) {
        let mut registry = device_registry.write().await;
        let device_id = fingerprint.to_id();

        if let Some(entry) = registry.get_mut(&device_id) {
            entry.update_seen();
        }
    }

    /// Get the last seen time for a device
    async fn get_device_last_seen(
        device_registry: &Arc<RwLock<HashMap<String, DeviceRegistryEntry>>>,
        fingerprint: &DeviceFingerprint,
    ) -> Option<Instant> {
        let registry = device_registry.read().await;
        let device_id = fingerprint.to_id();

        registry.get(&device_id).map(|entry| entry.last_seen)
    }

    /// Get a snapshot of the device registry
    pub async fn get_device_registry(&self) -> HashMap<String, DeviceRegistryEntry> {
        self.device_registry.read().await.clone()
    }

    /// Get registry entry for a specific device
    pub async fn get_device_registry_entry(&self, fingerprint: &DeviceFingerprint) -> Option<DeviceRegistryEntry> {
        let registry = self.device_registry.read().await;
        registry.get(&fingerprint.to_id()).cloned()
    }

    /// Clear the device registry (for testing or reset)
    pub async fn clear_device_registry(&self) {
        self.device_registry.write().await.clear();
    }
}

/// Print a summary of connected devices.
pub fn print_device_summary(manager: &DeviceManager) {
    let devices = manager.devices();

    if devices.is_empty() {
        println!("No SteelSeries devices found.");
        return;
    }

    // Group by (vendor_id, product_id, serial_number) - use pre-allocated HashMap
    let mut grouped: HashMap<(u16, u16, Option<String>), Vec<DeviceInfo>> = HashMap::with_capacity(devices.len());

    for device in devices {
        let key = (device.vendor_id, device.product_id, device.serial_number.clone());
        grouped.entry(key).or_default().push(device.clone());
    }

    // Convert to sorted vec with pre-allocated capacity
    let mut device_groups: Vec<_> = Vec::with_capacity(grouped.len());
    device_groups.extend(grouped);

    // Optimize sorting by sorting in-place with a single comparison
    device_groups.sort_unstable_by(|a, b| {
        let dev_a = &a.1[0]; // First device in group
        let dev_b = &b.1[0];
        dev_a
            .device_type
            .cmp(&dev_b.device_type)
            .then_with(|| dev_a.name.cmp(&dev_b.name))
            .then_with(|| dev_a.interface_number.cmp(&dev_b.interface_number))
    });

    println!("Found {} SteelSeries device(s):\n", device_groups.len());

    for (i, (_key, mut interfaces)) in device_groups.into_iter().enumerate() {
        // Sort interfaces within group - use unstable sort for better performance
        interfaces.sort_unstable_by_key(|d| d.interface_number);

        let device = &interfaces[0]; // Representative device

        println!("  {}. {} [{}]", i + 1, device.name, device.device_type);

        // More efficient interface list building with pre-allocated capacity and direct formatting
        let mut interface_list = String::with_capacity(interfaces.len() * 4); // Estimate capacity
        for (idx, interface) in interfaces.iter().enumerate() {
            if idx > 0 {
                interface_list.push_str(", ");
            }
            interface_list.push_str(&interface.interface_number.to_string());
        }

        println!(
            "     VID: {:#06x}, PID: {:#06x}, Interfaces: {}",
            device.vendor_id, device.product_id, interface_list
        );

        if let Some(ref serial) = device.serial_number {
            println!("     Serial: {}", serial);
        }

        println!();
    }
}
