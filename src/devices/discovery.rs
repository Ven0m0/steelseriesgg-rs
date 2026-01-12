//! Device discovery and enumeration.

use hidapi::HidApi;
use std::collections::HashMap;
use tracing::{debug, info};

use super::keyboards::{GenericKeyboard, Keyboard};
use super::{device_name_from_product_id, device_type_from_product_id, DeviceInfo, DeviceType};
use crate::{Error, Result, STEELSERIES_VENDOR_ID};

/// Manages connected SteelSeries devices.
pub struct DeviceManager {
    api: HidApi,
    devices: HashMap<String, DeviceInfo>,
    /// Cache of device paths indexed by (vendor_id, product_id, interface_number) for O(1) lookup
    device_cache: HashMap<(u16, u16, i32), String>,
}

impl DeviceManager {
    /// Create a new device manager.
    pub fn new() -> Result<Self> {
        let api = HidApi::new()?;
        let mut manager = Self {
            api,
            devices: HashMap::new(),
            device_cache: HashMap::new(),
        };
        manager.refresh()?;
        Ok(manager)
    }

    /// Refresh the list of connected devices.
    pub fn refresh(&mut self) -> Result<()> {
        self.devices.clear();
        self.device_cache.clear();
        self.api.refresh_devices()?;

        for device in self.api.device_list() {
            if device.vendor_id() != STEELSERIES_VENDOR_ID {
                continue;
            }

            let product_id = device.product_id();
            let device_type = device_type_from_product_id(product_id);
            let name = device_name_from_product_id(product_id).to_string();

            // Avoid double allocation: to_string_lossy returns Cow, into_owned is more efficient
            let path = device.path().to_string_lossy().into_owned();

            let info = DeviceInfo {
                name,
                device_type,
                vendor_id: device.vendor_id(),
                product_id,
                interface_number: device.interface_number(),
                serial_number: device.serial_number().map(|s| s.to_string()),
                manufacturer: device.manufacturer_string().map(|s| s.to_string()),
                path: path.clone(),
            };

            debug!(
                "Found device: {} (PID: {:#06x}, Interface: {})",
                info.name, info.product_id, info.interface_number
            );

            // Cache the device path for fast lookup
            let cache_key = (
                device.vendor_id(),
                device.product_id(),
                device.interface_number(),
            );
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
        self.devices
            .values()
            .filter(|d| d.device_type == device_type)
            .collect()
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
        Ok(Box::new(generic_keyboard))
    }
}

/// Print a summary of connected devices.
pub fn print_device_summary(manager: &DeviceManager) {
    let devices = manager.devices();

    if devices.is_empty() {
        println!("No SteelSeries devices found.");
        return;
    }

    // Group by (vendor_id, product_id, serial_number)
    let mut grouped: HashMap<(u16, u16, Option<String>), Vec<DeviceInfo>> = HashMap::new();

    for device in devices {
        let key = (
            device.vendor_id,
            device.product_id,
            device.serial_number.clone(),
        );
        grouped.entry(key).or_default().push(device.clone());
    }

    // Convert to sorted vec
    let mut device_groups: Vec<_> = grouped.into_iter().collect();
    device_groups.sort_by(|a, b| {
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
        // Sort interfaces within group
        interfaces.sort_by_key(|d| d.interface_number);

        let device = &interfaces[0]; // Representative device

        println!("  {}. {} [{}]", i + 1, device.name, device.device_type);

        // Collect interface numbers as comma-separated string
        let interface_list: Vec<String> = interfaces
            .iter()
            .map(|d| d.interface_number.to_string())
            .collect();

        println!(
            "     VID: {:#06x}, PID: {:#06x}, Interfaces: {}",
            device.vendor_id,
            device.product_id,
            interface_list.join(", ")
        );

        if let Some(ref serial) = device.serial_number {
            println!("     Serial: {}", serial);
        }

        println!();
    }
}
