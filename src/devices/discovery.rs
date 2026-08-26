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
use super::product_ids::{
    APEX_3_TKL, APEX_PRO_TKL_2023, APEX_PRO_TKL_2023_WIRELESS, APEX_PRO_TKL_2023_WIRELESS_2, APEX_PRO_TKL_2024,
    APEX_PRO_TKL_WIRELESS_2024, APEX_PRO_TKL_WIRELESS_2024_DONGLE,
};
use super::{
    DeviceInfo, DeviceType, STEELSERIES_CONTROL_USAGE_PAGE, device_name_from_product_id, device_type_from_product_id,
};
use crate::{Error, Result, STEELSERIES_VENDOR_ID};

/// Score a HID collection as a candidate control (RGB/actuation) endpoint. Higher scores win;
/// `None` means this collection is never a control endpoint.
///
/// Windows splits one USB HID interface into several device nodes (one per top-level
/// collection) that share the same interface number, so `interface_number` alone cannot
/// identify the control collection. Ranking is by usage page instead:
///
/// 1. `STEELSERIES_CONTROL_USAGE_PAGE` (`0xFFC0`) — the confirmed SteelSeries control page.
/// 2. Any other vendor-defined page (`>= 0xFF00`) — unconfirmed but plausible, tie-broken by
///    the lowest interface number so a deterministic choice is made when multiple exist.
/// 3. The legacy per-PID interface table, for hardware where no vendor-defined page was seen
///    (kept as a fallback since the wireless PIDs are unverified against this ranking).
/// 4. Standard OS-facing pages (Generic Desktop `0x0001`, Consumer `0x000C`) never qualify.
fn control_score(usage_page: u16, interface_number: i32, product_id: u16, device_type: DeviceType) -> Option<u32> {
    if usage_page == STEELSERIES_CONTROL_USAGE_PAGE {
        return Some(3_000_000 - interface_number.max(0) as u32);
    }
    if usage_page >= 0xFF00 {
        return Some(2_000_000 - interface_number.max(0) as u32);
    }

    let fallback_interface = match device_type {
        DeviceType::Keyboard
            if product_id == APEX_PRO_TKL_2023_WIRELESS || product_id == APEX_PRO_TKL_2023_WIRELESS_2 =>
        {
            3
        }
        DeviceType::Keyboard => 1,
        DeviceType::Headset => 3,
        DeviceType::Unknown => return None,
    };
    if interface_number == fallback_interface {
        Some(1_000_000)
    } else {
        None
    }
}

/// Pick one representative `DeviceInfo` per physical device of `device_type` from `devices`,
/// preferring the control-endpoint candidate recorded in `control_cache` for that
/// (vendor_id, product_id) when one exists.
///
/// Pure and side-effect free (no `HidApi` dependency) so it is directly testable, and so its
/// result does not depend on `devices`' `HashMap` iteration order — the source of the D1 bug
/// where the representative HID collection changed on every process restart.
fn pick_representatives<'a>(
    devices: &'a HashMap<String, DeviceInfo>,
    control_cache: &'a HashMap<(u16, u16), DeviceInfo>,
    device_type: DeviceType,
) -> Vec<&'a DeviceInfo> {
    let mut seen: std::collections::HashSet<(u16, Option<&str>)> = std::collections::HashSet::new();
    let mut result: Vec<&DeviceInfo> = devices
        .values()
        .filter(|d| d.device_type == device_type)
        .filter(|d| seen.insert((d.product_id, d.serial_number.as_deref())))
        .map(|d| control_cache.get(&(d.vendor_id, d.product_id)).unwrap_or(d))
        .collect();
    result.sort_unstable_by(|a, b| {
        a.product_id
            .cmp(&b.product_id)
            .then_with(|| a.serial_number.cmp(&b.serial_number))
    });
    result
}

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
    /// Best-scoring control-endpoint path per (vendor_id, product_id), chosen via `control_score`.
    /// This is the single source of truth for "which HID collection is the control endpoint" —
    /// `open_device` and `devices_by_type` both read from it so they agree on the same device.
    control_cache: HashMap<(u16, u16), DeviceInfo>,
    /// Manual control-endpoint override, set via `set_device_override`. Empty by default.
    device_override: crate::config::DeviceConfig,
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
            control_cache: HashMap::new(),
            device_override: crate::config::DeviceConfig::default(),
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
        self.control_cache.clear();

        self.api.refresh_devices()?;

        // Track the best-scoring candidate seen so far per (vendor_id, product_id).
        let mut best_scores: HashMap<(u16, u16), u32> = HashMap::new();

        for device in self.api.device_list() {
            if device.vendor_id() != STEELSERIES_VENDOR_ID {
                continue;
            }

            let product_id = device.product_id();
            let device_type = device_type_from_product_id(product_id);

            // Use static str and convert to String only once
            let name = std::borrow::Cow::Borrowed(device_name_from_product_id(product_id));

            // Avoid double allocation: to_string_lossy returns Cow, into_owned is more efficient
            let path = device.path().to_string_lossy().into_owned();

            let info = DeviceInfo {
                name,
                device_type,
                vendor_id: device.vendor_id(),
                product_id,
                interface_number: device.interface_number(),
                usage_page: device.usage_page(),
                usage: device.usage(),
                serial_number: device.serial_number().map(|s: &str| s.to_string()),
                manufacturer: device.manufacturer_string().map(|s: &str| s.to_string()),
                path: path.clone(),
            };

            debug!(
                "Found device: {} (PID: {:#06x}, Interface: {}, UsagePage: {:#06x}, Usage: {:#04x})",
                info.name, info.product_id, info.interface_number, info.usage_page, info.usage
            );

            let cache_key = (info.vendor_id, info.product_id);
            if let Some(score) = control_score(
                info.usage_page,
                info.interface_number,
                info.product_id,
                info.device_type,
            ) {
                let is_better = best_scores.get(&cache_key).is_none_or(|&best| score > best);
                if is_better {
                    best_scores.insert(cache_key, score);
                    self.control_cache.insert(cache_key, info.clone());
                }
            }

            self.devices.insert(path, info);
        }

        info!("Found {} SteelSeries device(s)", self.devices.len());
        Ok(())
    }

    /// Get all connected devices.
    pub fn devices(&self) -> Vec<&DeviceInfo> {
        self.devices.values().collect()
    }

    /// Get devices of a specific type, deduplicated by physical device.
    ///
    /// USB HID devices (e.g. Apex Pro TKL 2023) expose multiple HID interfaces
    /// under a single physical device.  Each interface appears as a separate entry
    /// from hidapi, all sharing the same product ID and serial number.  This method
    /// returns one representative `DeviceInfo` per physical device so callers don't
    /// accidentally open or list the same device multiple times.
    ///
    /// The representative is always the control-endpoint candidate chosen by `control_score`
    /// during `refresh` (falling back to whichever collection was seen first if none scored),
    /// so the result is deterministic across runs — unlike picking an arbitrary entry out of a
    /// `HashMap`, whose iteration order is randomized per process.
    pub fn devices_by_type(&self, device_type: DeviceType) -> Vec<&DeviceInfo> {
        pick_representatives(&self.devices, &self.control_cache, device_type)
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

    /// Resolve the control-endpoint `DeviceInfo` for a (vendor_id, product_id) pair.
    ///
    /// Checks `device_override` first (from `config.toml`'s `[device]` section), then falls
    /// back to the `control_score`-ranked candidate computed during `refresh`.
    fn resolve_control(&self, vendor_id: u16, product_id: u16) -> Option<&DeviceInfo> {
        if let Some(path) = &self.device_override.control_path
            && let Some(d) = self.devices.get(path)
            && d.vendor_id == vendor_id
            && d.product_id == product_id
        {
            return Some(d);
        }
        if let Some(usage_page) = self.device_override.control_usage_page
            && let Some(d) = self
                .devices
                .values()
                .find(|d| d.vendor_id == vendor_id && d.product_id == product_id && d.usage_page == usage_page)
        {
            return Some(d);
        }
        self.control_cache.get(&(vendor_id, product_id))
    }

    /// Set a manual control-endpoint override (typically loaded from `config.toml`'s `[device]`
    /// section). Takes priority over `control_score`'s ranking when a match is found; falls back
    /// to automatic ranking otherwise.
    pub fn set_device_override(&mut self, device_config: crate::config::DeviceConfig) {
        self.device_override = device_config;
    }

    /// Path of the resolved control-endpoint collection for a (vendor_id, product_id) pair, if
    /// any was found. Used by `print_device_summary` to mark which of several HID collections
    /// is actually used for communication.
    pub fn control_path(&self, vendor_id: u16, product_id: u16) -> Option<&str> {
        self.resolve_control(vendor_id, product_id).map(|d| d.path.as_str())
    }

    /// Open a device for communication.
    pub fn open_device(&self, info: &DeviceInfo) -> Result<hidapi::HidDevice> {
        let control = self.resolve_control(info.vendor_id, info.product_id);

        let path = match control {
            Some(c) => &c.path,
            None => {
                return Err(Error::DeviceNotFound(format!(
                    "{} (no control-endpoint collection found for VID:{:#06x} PID:{:#06x})",
                    info.name, info.vendor_id, info.product_id
                )));
            }
        };

        debug!(
            "Opening device: {} (VID:{:#06x}, PID:{:#06x}, Interface:{}, UsagePage:{:#06x})",
            info.name,
            info.vendor_id,
            info.product_id,
            control.map(|c| c.interface_number).unwrap_or(-1),
            control.map(|c| c.usage_page).unwrap_or(0),
        );

        use std::ffi::CString;
        let c_path = CString::new(path.as_str())
            .map_err(|e| Error::DeviceCommunication(format!("Invalid device path: {}", e)))?;
        self.api.open_path(&c_path).map_err(Error::from)
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

        // Wireless Apex 2023 uses raw hidraw feature reports, not hidapi.
        // Try opening via hidapi; if it fails for wireless, create a dummy-device keyboard
        // that only works through the raw feature report path.
        let hid_device = match self.open_device(info) {
            Ok(dev) => dev,
            Err(e)
                if matches!(
                    info.product_id,
                    APEX_PRO_TKL_2023_WIRELESS
                        | APEX_PRO_TKL_2023_WIRELESS_2
                        | APEX_PRO_TKL_WIRELESS_2024_DONGLE
                        | APEX_PRO_TKL_WIRELESS_2024
                ) =>
            {
                tracing::warn!("hidapi open failed for wireless keyboard (expected): {e}. Using raw hidraw path.");
                return Ok(Box::new(ApexProTkl2023::new_wireless_raw(info.clone())));
            }
            Err(e) => return Err(e),
        };
        let generic_keyboard = GenericKeyboard::new(info.clone(), hid_device);

        // Wrap in specific implementation if available
        match info.product_id {
            APEX_3_TKL => Ok(Box::new(Apex3Tkl::new(generic_keyboard))),
            APEX_PRO_TKL_2023
            | APEX_PRO_TKL_2023_WIRELESS
            | APEX_PRO_TKL_2023_WIRELESS_2
            | APEX_PRO_TKL_2024
            | APEX_PRO_TKL_WIRELESS_2024_DONGLE
            | APEX_PRO_TKL_WIRELESS_2024 => Ok(Box::new(ApexProTkl2023::new(generic_keyboard))),
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
                        let (returned_api, res) = Self::poll_devices(api, &device_registry, &pending_events,
                                                         &callback, &config, &mut current_devices,
                                                         &mut last_event_time).await;
                        api = returned_api;
                        if let Err(e) = res {
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
        mut api: HidApi,
        device_registry: &Arc<RwLock<HashMap<String, DeviceRegistryEntry>>>,
        pending_events: &Arc<RwLock<HashMap<DeviceFingerprint, Instant>>>,
        callback: &Option<Arc<HotPlugCallback>>,
        config: &HotPlugConfig,
        current_devices: &mut HashMap<DeviceFingerprint, DeviceInfo>,
        last_event_time: &mut Instant,
    ) -> (HidApi, Result<()>) {
        // Rate limiting
        let now = Instant::now();
        let time_since_last = now.duration_since(*last_event_time);
        let min_interval = Duration::from_millis(1000 / config.max_event_rate as u64);

        if time_since_last < min_interval {
            return (api, Ok(()));
        }

        let (returned_api, res) = tokio::task::spawn_blocking(move || {
            // Refresh device list
            if let Err(e) = api.refresh_devices() {
                warn!("Failed to refresh HID device list: {}", e);
                return (api, Ok(HashMap::new()));
            }

            let mut new_devices: HashMap<DeviceFingerprint, DeviceInfo> = HashMap::new();

            // Enumerate current devices
            for device in api.device_list() {
                if device.vendor_id() != crate::STEELSERIES_VENDOR_ID {
                    continue;
                }

                let product_id = device.product_id();
                let device_type = crate::devices::device_type_from_product_id(product_id);
                let interface_number = device.interface_number();
                let usage_page = device.usage_page();

                // Skip HID collections that are not a control endpoint (e.g. the OS-facing
                // Generic Desktop / Consumer collections Windows exposes alongside the vendor
                // control page), so hot-plug fires one event per physical device, not per
                // collection.
                if control_score(usage_page, interface_number, product_id, device_type).is_none() {
                    continue;
                }

                let name = std::borrow::Cow::Owned(crate::devices::device_name_from_product_id(product_id).to_string());
                let path = device.path().to_string_lossy().into_owned();

                let info = crate::devices::DeviceInfo {
                    name,
                    device_type,
                    vendor_id: device.vendor_id(),
                    product_id,
                    interface_number,
                    usage_page,
                    usage: device.usage(),
                    serial_number: device.serial_number().map(|s: &str| s.to_string()),
                    manufacturer: device.manufacturer_string().map(|s: &str| s.to_string()),
                    path,
                };

                let fingerprint = DeviceFingerprint::from_device_info(&info);
                new_devices.insert(fingerprint, info);
            }

            (api, Ok(new_devices))
        })
        .await
        .unwrap_or_else(|e| {
            warn!("Task panicked: {}", e);
            // Recreating HidApi is the only way to keep the hot-plug loop alive after a
            // panicked poll; if the HID subsystem itself is gone there is no recovery path,
            // so this is an explicitly fatal, documented panic (CLAUDE.md rule 8 exception).
            let fallback_api = hidapi::HidApi::new()
                .expect("HID subsystem unavailable after poll task panic; cannot continue hot-plug monitoring");
            (
                fallback_api,
                Err(crate::Error::DeviceCommunication("Blocking task panicked".into())),
            )
        });

        let new_devices = match res {
            Ok(d) => d,
            Err(e) => return (returned_api, Err(e)),
        };

        api = returned_api;

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
        if let Some(callback_arc) = callback {
            for event in events_to_fire {
                callback_arc(event);
                *last_event_time = now;
            }
        }

        // Clean up expired pending events
        Self::cleanup_pending_events(pending_events, config).await;

        (api, Ok(()))
    }

    /// Check if an event should be debounced
    async fn should_debounce_event(
        pending_events: &Arc<RwLock<HashMap<DeviceFingerprint, Instant>>>,
        fingerprint: &DeviceFingerprint,
        config: &HotPlugConfig,
    ) -> bool {
        let now = Instant::now();
        let mut pending = pending_events.write().await;

        if let Some(last_event) = pending.get_mut(fingerprint) {
            let debounced = now.duration_since(*last_event) < config.debounce_time;
            // Update the timestamp to extend debounce and avoid cloning fingerprint
            *last_event = now;
            return debounced;
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
    let mut grouped: HashMap<(u16, u16, Option<&str>), Vec<&DeviceInfo>> = HashMap::with_capacity(devices.len());

    for device in devices {
        let key = (device.vendor_id, device.product_id, device.serial_number.as_deref());
        grouped.entry(key).or_default().push(device);
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
        println!("     VID: {:#06x}, PID: {:#06x}", device.vendor_id, device.product_id);

        let control_path = manager.control_path(device.vendor_id, device.product_id);
        for interface in &interfaces {
            let marker = if control_path == Some(interface.path.as_str()) {
                " <- control endpoint"
            } else {
                ""
            };
            println!(
                "       Interface {}: usage_page={:#06x}, usage={:#04x}{}",
                interface.interface_number, interface.usage_page, interface.usage, marker
            );
        }

        if let Some(ref serial) = device.serial_number {
            println!("     Serial: {}", serial);
        }

        println!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_device_info(serial: Option<String>) -> DeviceInfo {
        DeviceInfo {
            name: std::borrow::Cow::Borrowed("Test Device"),
            device_type: DeviceType::Keyboard,
            vendor_id: 0x1038,
            product_id: 0x1612,
            interface_number: 1,
            usage_page: STEELSERIES_CONTROL_USAGE_PAGE,
            usage: 0x01,
            serial_number: serial,
            manufacturer: Some("SteelSeries".to_string()),
            path: "path/to/device".to_string(),
        }
    }

    /// Apex Pro TKL 2023 (PID 0x1628) HID collection, as measured via `SetupDiEnumDeviceInterfaces`
    /// + `HidP_GetCaps` on Windows, 2026-08-18. Ten collections share one physical device.
    fn apex_pro_tkl_2023_collection(interface_number: i32, usage_page: u16, usage: u16, path: &str) -> DeviceInfo {
        DeviceInfo {
            name: std::borrow::Cow::Borrowed("Apex Pro TKL (2023)"),
            device_type: DeviceType::Keyboard,
            vendor_id: crate::STEELSERIES_VENDOR_ID,
            product_id: APEX_PRO_TKL_2023,
            interface_number,
            usage_page,
            usage,
            serial_number: None,
            manufacturer: Some("SteelSeries".to_string()),
            path: path.to_string(),
        }
    }

    /// The exact 10 collections measured for PID 0x1628, as `(interface, usage_page, usage, path)`.
    fn apex_pro_tkl_2023_table() -> Vec<(i32, u16, u16, &'static str)> {
        vec![
            (0, 0x0001, 0x02, "mi_00&col01"), // mouse
            (0, 0x0001, 0x06, "mi_00&col02"), // OS keyboard
            (0, 0x000C, 0x01, "mi_00&col03"), // consumer
            (1, 0xFFC0, 0x01, "mi_01"),       // control endpoint: out=65, feature=645
            (2, 0x0001, 0x02, "mi_02&col01"),
            (2, 0x0001, 0x06, "mi_02&col02"),
            (2, 0x000C, 0x01, "mi_02&col03"),
            (3, 0x000C, 0x01, "mi_03&col01"),
            (3, 0x0001, 0x02, "mi_03&col02"),
            (4, 0xFFC1, 0x01, "mi_04"), // vendor-defined but input-only (out=0, feature=0)
        ]
    }

    #[test]
    fn test_control_score_prefers_confirmed_vendor_page_over_other_vendor_pages() {
        let confirmed = control_score(0xFFC0, 1, APEX_PRO_TKL_2023, DeviceType::Keyboard).unwrap();
        let other_vendor = control_score(0xFFC1, 4, APEX_PRO_TKL_2023, DeviceType::Keyboard).unwrap();
        assert!(confirmed > other_vendor, "0xFFC0 must outrank 0xFFC1");
    }

    #[test]
    fn test_control_score_rejects_os_facing_pages_off_the_fallback_interface() {
        // Generic Desktop (mouse and OS keyboard) and Consumer pages never rank via the vendor-
        // page tiers; on any interface other than the legacy per-PID fallback (1, for a
        // non-wireless keyboard) they score None entirely. On the Apex Pro TKL 2023 this covers
        // mi_00, mi_02, and mi_03's collections (interfaces 0, 2, 3).
        assert_eq!(control_score(0x0001, 0, APEX_PRO_TKL_2023, DeviceType::Keyboard), None);
        assert_eq!(control_score(0x0001, 2, APEX_PRO_TKL_2023, DeviceType::Keyboard), None);
        assert_eq!(control_score(0x000C, 3, APEX_PRO_TKL_2023, DeviceType::Keyboard), None);
    }

    #[test]
    fn test_control_score_fallback_tier_is_usage_page_agnostic_on_the_fallback_interface() {
        // Tier 3 (the legacy per-PID interface table) matches by interface number alone, same as
        // the code it replaces — it does not know the difference between a vendor page and an
        // OS-facing page on that interface. This is only safe because, on every PID this crate
        // has verified, nothing OS-facing sits on the fallback interface (confirmed for the Apex
        // Pro TKL 2023: interface 1 carries only the 0xFFC0 control collection). A future PID
        // where that assumption doesn't hold would need a page check added to this tier.
        assert_eq!(
            control_score(0x0001, 1, APEX_PRO_TKL_2023, DeviceType::Keyboard),
            Some(1_000_000)
        );
    }

    #[test]
    fn test_control_score_falls_back_to_per_pid_interface_when_no_vendor_page() {
        // A hypothetical keyboard with no vendor-defined page at all still resolves via the
        // legacy per-PID interface table (interface 1 for a non-wireless keyboard).
        let other_pid = 0x9999;
        assert_eq!(
            control_score(0x0001, 1, other_pid, DeviceType::Keyboard),
            Some(1_000_000)
        );
        assert_eq!(control_score(0x0001, 0, other_pid, DeviceType::Keyboard), None);
    }

    #[test]
    fn test_control_score_wireless_fallback_uses_interface_3() {
        assert_eq!(
            control_score(0x0001, 3, APEX_PRO_TKL_2023_WIRELESS, DeviceType::Keyboard),
            Some(1_000_000)
        );
        assert_eq!(
            control_score(0x0001, 1, APEX_PRO_TKL_2023_WIRELESS, DeviceType::Keyboard),
            None
        );
    }

    #[test]
    fn test_control_score_picks_mi_01_from_full_apex_pro_tkl_2023_table() {
        let scores: Vec<(&str, Option<u32>)> = apex_pro_tkl_2023_table()
            .into_iter()
            .map(|(iface, usage_page, _usage, path)| {
                (
                    path,
                    control_score(usage_page, iface, APEX_PRO_TKL_2023, DeviceType::Keyboard),
                )
            })
            .collect();

        let best = scores.iter().max_by_key(|(_, score)| score.unwrap_or(0)).unwrap();
        assert_eq!(best.0, "mi_01", "mi_01 (0xFFC0) must outscore every other collection");

        // The eight Generic Desktop / Consumer collections never qualify (none of them sit on
        // interface 1, the per-PID fallback for a non-wireless keyboard); only mi_01 (0xFFC0)
        // and mi_04 (0xFFC1) score.
        let none_count = scores.iter().filter(|(_, s)| s.is_none()).count();
        assert_eq!(none_count, 8);
    }

    #[test]
    fn test_devices_by_type_returns_one_representative_for_ten_collections() {
        let mut devices: HashMap<String, DeviceInfo> = HashMap::new();
        for (iface, usage_page, usage, path) in apex_pro_tkl_2023_table() {
            devices.insert(
                path.to_string(),
                apex_pro_tkl_2023_collection(iface, usage_page, usage, path),
            );
        }

        let mut control_cache: HashMap<(u16, u16), DeviceInfo> = HashMap::new();
        control_cache.insert(
            (crate::STEELSERIES_VENDOR_ID, APEX_PRO_TKL_2023),
            apex_pro_tkl_2023_collection(1, 0xFFC0, 0x01, "mi_01"),
        );

        let result = pick_representatives(&devices, &control_cache, DeviceType::Keyboard);
        assert_eq!(
            result.len(),
            1,
            "ten collections of one physical device must dedupe to one entry"
        );
        assert_eq!(result[0].path, "mi_01");
        assert_eq!(result[0].usage_page, 0xFFC0);
    }

    #[test]
    fn test_devices_by_type_representative_is_stable_regardless_of_map_insertion_order() {
        // D1: representative selection must not depend on `HashMap` iteration order, which is
        // randomized per instance. Build the same 10 collections in forward and reverse
        // insertion order and confirm both pick the identical representative.
        let mut control_cache: HashMap<(u16, u16), DeviceInfo> = HashMap::new();
        control_cache.insert(
            (crate::STEELSERIES_VENDOR_ID, APEX_PRO_TKL_2023),
            apex_pro_tkl_2023_collection(1, 0xFFC0, 0x01, "mi_01"),
        );

        let table = apex_pro_tkl_2023_table();

        let mut forward: HashMap<String, DeviceInfo> = HashMap::new();
        for &(iface, usage_page, usage, path) in &table {
            forward.insert(
                path.to_string(),
                apex_pro_tkl_2023_collection(iface, usage_page, usage, path),
            );
        }

        let mut reversed: HashMap<String, DeviceInfo> = HashMap::new();
        for &(iface, usage_page, usage, path) in table.iter().rev() {
            reversed.insert(
                path.to_string(),
                apex_pro_tkl_2023_collection(iface, usage_page, usage, path),
            );
        }

        let rep_forward = pick_representatives(&forward, &control_cache, DeviceType::Keyboard);
        let rep_reversed = pick_representatives(&reversed, &control_cache, DeviceType::Keyboard);

        assert_eq!(rep_forward.len(), 1);
        assert_eq!(rep_reversed.len(), 1);
        assert_eq!(rep_forward[0].path, rep_reversed[0].path);

        // The regression this guards against: a stable representative means a stable DeviceId
        // key, so persisted RGB state/profiles are found again on the next launch.
        let key_forward = crate::device_state::DeviceId::from(rep_forward[0]).to_key();
        let key_reversed = crate::device_state::DeviceId::from(rep_reversed[0]).to_key();
        assert_eq!(key_forward, key_reversed, "DeviceId key must be stable across runs");
    }

    #[test]
    fn test_fingerprint_creation() {
        let info = create_test_device_info(Some("serial123".to_string()));
        let fingerprint = DeviceFingerprint::from_device_info(&info);

        assert_eq!(fingerprint.vendor_id, 0x1038);
        assert_eq!(fingerprint.product_id, 0x1612);
        assert_eq!(fingerprint.interface_number, 1);
        assert_eq!(fingerprint.serial_number, Some("serial123".to_string()));
    }

    #[test]
    fn test_fingerprint_to_id_with_serial() {
        let info = create_test_device_info(Some("serial123".to_string()));
        let fingerprint = DeviceFingerprint::from_device_info(&info);

        // format: vid(dec):pid(hex):iface(hex):serial
        // 4152 = 0x1038
        assert_eq!(fingerprint.to_id(), "4152:1612:0001:serial123");
    }

    #[test]
    fn test_fingerprint_to_id_no_serial() {
        let info = create_test_device_info(None);
        let fingerprint = DeviceFingerprint::from_device_info(&info);

        assert_eq!(fingerprint.to_id(), "4152:1612:0001:no_serial");
    }

    #[test]
    fn test_fingerprint_equality() {
        let info1 = create_test_device_info(Some("serial1".to_string()));
        let fp1 = DeviceFingerprint::from_device_info(&info1);
        let fp2 = DeviceFingerprint::from_device_info(&info1); // Same info

        let mut info2 = info1.clone();
        info2.serial_number = Some("serial2".to_string());
        let fp3 = DeviceFingerprint::from_device_info(&info2);

        assert_eq!(fp1, fp2);
        assert_ne!(fp1, fp3);

        // Test hashing
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher1 = DefaultHasher::new();
        fp1.hash(&mut hasher1);
        let hash1 = hasher1.finish();

        let mut hasher2 = DefaultHasher::new();
        fp2.hash(&mut hasher2);
        let hash2 = hasher2.finish();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_registry_entry_lifecycle() {
        let info = create_test_device_info(Some("serial1".to_string()));
        let fingerprint = DeviceFingerprint::from_device_info(&info);

        let mut entry = DeviceRegistryEntry::new(info.clone(), fingerprint.clone());

        assert_eq!(entry.connection_count, 1);
        let initial_seen = entry.last_seen;

        // Sleep briefly to ensure time passes
        std::thread::sleep(std::time::Duration::from_millis(10));

        entry.update_seen();
        assert!(entry.last_seen > initial_seen);
        assert_eq!(entry.connection_count, 1); // Count shouldn't change

        let after_update = entry.last_seen;
        std::thread::sleep(std::time::Duration::from_millis(10));

        entry.increment_connection_count();
        assert_eq!(entry.connection_count, 2);
        assert!(entry.last_seen > after_update);
    }
}
