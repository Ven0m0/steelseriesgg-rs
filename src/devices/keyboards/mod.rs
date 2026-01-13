//! Keyboard device support (Apex series).

pub mod apex;

use super::{Device, DeviceInfo, DeviceType, write_padded_report, zone_count_for_product_id};
use crate::rgb::Color;
use crate::{Error, Result};
use hidapi::HidDevice;
use std::sync::{Arc, Mutex};

/// Trait for keyboard-specific functionality.
pub trait Keyboard: Device {
    /// Set the entire keyboard to a single color.
    fn set_color(&mut self, color: Color) -> Result<()>;

    /// Set colors for individual zones.
    fn set_zone_colors(&mut self, colors: &[Color]) -> Result<()>;

    /// Get the number of RGB zones.
    fn zone_count(&self) -> usize;

    /// Set keyboard brightness (0-100).
    fn set_brightness(&mut self, brightness: u8) -> Result<()>;

    /// Apply the current RGB settings.
    fn apply(&mut self) -> Result<()>;
}

/// Generic SteelSeries keyboard implementation.
pub struct GenericKeyboard {
    info: DeviceInfo,
    device: Option<Arc<Mutex<HidDevice>>>,
    zone_count: usize,
}

impl GenericKeyboard {
    /// Create a new keyboard instance.
    pub fn new(info: DeviceInfo, device: HidDevice) -> Self {
        // Use centralized zone count mapping
        let zone_count = zone_count_for_product_id(info.product_id);

        Self {
            info,
            device: Some(Arc::new(Mutex::new(device))),
            zone_count,
        }
    }

    /// Send a HID report to the keyboard.
    fn send_report(&mut self, data: &[u8]) -> Result<()> {
        use tracing::debug;

        debug!("Sending HID report ({} bytes): {:02x?}", data.len(), data);

        let device = self.device.as_ref().ok_or(Error::DeviceCommunication(
            "Device not connected".to_string(),
        ))?;
        let device = device
            .lock()
            .map_err(|e| Error::DeviceCommunication(format!("Device lock poisoned: {}", e)))?;

        let result = write_padded_report(&device, data, 65, true);

        match &result {
            Ok(_) => debug!("HID report sent successfully"),
            Err(e) => debug!("HID report failed: {:?}", e),
        }

        result
    }
}

impl Device for GenericKeyboard {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::Keyboard
    }

    fn initialize(&mut self) -> Result<()> {
        // Send initialization sequence if needed
        // Some SteelSeries keyboards need a "save" or "commit" command
        // Try sending 0x09 command which is sometimes used for "save/apply"
        let init_cmd = [0x09];
        let _ = self.send_report(&init_cmd); // Don't fail if this doesn't work
        Ok(())
    }

    fn close(&mut self) -> Result<()> {
        self.device = None;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.device.is_some()
    }

    fn send_raw(&mut self, data: &[u8]) -> Result<()> {
        self.send_report(data)
    }

    fn receive_raw(&mut self, buf: &mut [u8]) -> Result<usize> {
        let device = self.device.as_ref().ok_or(Error::DeviceCommunication(
            "Device not connected".to_string(),
        ))?;
        let device = device
            .lock()
            .map_err(|e| Error::DeviceCommunication(format!("Device lock poisoned: {}", e)))?;

        let len = device.read(buf)?;
        Ok(len)
    }
}

impl Keyboard for GenericKeyboard {
    fn set_color(&mut self, color: Color) -> Result<()> {
        // Create color data for all zones
        let colors = vec![color; self.zone_count];
        self.set_zone_colors(&colors)
    }

    fn set_zone_colors(&mut self, colors: &[Color]) -> Result<()> {
        // Build the RGB packet
        // Format: [0x21, 0xFF, R, G, B, R, G, B, ...] for each zone
        let mut data = vec![0x21, 0xFF];

        for color in colors.iter().take(self.zone_count) {
            data.push(color.r);
            data.push(color.g);
            data.push(color.b);
        }

        // Pad remaining zones with zeros
        for _ in colors.len()..self.zone_count {
            data.extend_from_slice(&[0, 0, 0]);
        }

        self.send_report(&data)
    }

    fn zone_count(&self) -> usize {
        self.zone_count
    }

    fn set_brightness(&mut self, brightness: u8) -> Result<()> {
        // Clamp brightness to 0-100
        let brightness = brightness.min(100);

        // Send brightness command
        let data = [0x22, brightness];
        self.send_report(&data)
    }

    fn apply(&mut self) -> Result<()> {
        // Some keyboards need an explicit apply/save command after changes
        // Common SteelSeries commands: 0x09 (save), 0x28 (commit), 0x2c (update)
        // Try 0x09 which is documented as "save" in some SteelSeries devices
        let apply_cmd = [0x09];
        let _ = self.send_report(&apply_cmd); // Don't fail if device doesn't support it
        Ok(())
    }
}
