//! Headset device support (Arctis series).

use super::{Device, DeviceInfo, DeviceType, write_padded_report};
use crate::{Error, Result};
use hidapi::HidDevice;
use std::sync::{Arc, Mutex};

/// Trait for headset-specific functionality.
pub trait Headset: Device {
    /// Get battery level (0-100, or None if wired/unknown).
    fn battery_level(&mut self) -> Result<Option<u8>>;

    /// Set sidetone level (0-100).
    fn set_sidetone(&mut self, level: u8) -> Result<()>;

    /// Set microphone volume (0-100).
    fn set_mic_volume(&mut self, volume: u8) -> Result<()>;

    /// Mute/unmute microphone.
    fn set_mic_mute(&mut self, muted: bool) -> Result<()>;

    /// Set equalizer preset.
    fn set_eq_preset(&mut self, preset: EqPreset) -> Result<()>;

    /// Get ChatMix balance (-1.0 = full game, 1.0 = full chat).
    fn chat_mix(&mut self) -> Result<f32>;

    /// Set auto-off timeout in minutes (0 = disabled).
    fn set_auto_off(&mut self, minutes: u8) -> Result<()>;
}

/// Equalizer presets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EqPreset {
    Flat,
    Bass,
    Focus,
    Smiley,
    Custom,
}

/// Generic SteelSeries headset implementation.
pub struct GenericHeadset {
    info: DeviceInfo,
    device: Option<Arc<Mutex<HidDevice>>>,
}

impl GenericHeadset {
    /// Create a new headset instance.
    pub fn new(info: DeviceInfo, device: HidDevice) -> Self {
        Self {
            info,
            device: Some(Arc::new(Mutex::new(device))),
        }
    }

    /// Send a HID report to the headset.
    fn send_report(&mut self, data: &[u8]) -> Result<()> {
        let device = self.device.as_ref().ok_or(Error::DeviceCommunication(
            "Device not connected".to_string(),
        ))?;
        let device = device
            .lock()
            .map_err(|e| Error::DeviceCommunication(format!("Device lock poisoned: {}", e)))?;

        // Headsets typically use 64-byte reports with no report ID prefix.
        write_padded_report(&device, data, 64, false)
    }

    /// Receive a HID report from the headset.
    fn receive_report(&mut self, buf: &mut [u8]) -> Result<usize> {
        let device = self.device.as_ref().ok_or(Error::DeviceCommunication(
            "Device not connected".to_string(),
        ))?;
        let device = device
            .lock()
            .map_err(|e| Error::DeviceCommunication(format!("Device lock poisoned: {}", e)))?;

        let len = device.read_timeout(buf, 1000)?;
        Ok(len)
    }
}

impl Device for GenericHeadset {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::Headset
    }

    fn initialize(&mut self) -> Result<()> {
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
        self.receive_report(buf)
    }
}

impl Headset for GenericHeadset {
    fn battery_level(&mut self) -> Result<Option<u8>> {
        // Request battery status
        self.send_report(&[0x06, 0x18])?;

        let mut buf = [0u8; 64];
        let len = self.receive_report(&mut buf)?;

        if len >= 3 && buf[0] == 0x06 {
            // Battery level is typically in buf[2]
            let level = buf[2].min(100);
            Ok(Some(level))
        } else {
            Ok(None)
        }
    }

    fn set_sidetone(&mut self, level: u8) -> Result<()> {
        let level = level.min(100);
        let data = [0x06, 0x35, level];
        self.send_report(&data)
    }

    fn set_mic_volume(&mut self, volume: u8) -> Result<()> {
        let volume = volume.min(100);
        let data = [0x06, 0x37, volume];
        self.send_report(&data)
    }

    fn set_mic_mute(&mut self, muted: bool) -> Result<()> {
        let data = [0x06, 0x39, if muted { 0x01 } else { 0x00 }];
        self.send_report(&data)
    }

    fn set_eq_preset(&mut self, preset: EqPreset) -> Result<()> {
        let preset_code = match preset {
            EqPreset::Flat => 0x00,
            EqPreset::Bass => 0x01,
            EqPreset::Focus => 0x02,
            EqPreset::Smiley => 0x03,
            EqPreset::Custom => 0x04,
        };

        let data = [0x06, 0x33, preset_code];
        self.send_report(&data)
    }

    fn chat_mix(&mut self) -> Result<f32> {
        // Request ChatMix status
        self.send_report(&[0x06, 0x45])?;

        let mut buf = [0u8; 64];
        let len = self.receive_report(&mut buf)?;

        if len >= 3 && buf[0] == 0x06 {
            // ChatMix value: 0 = full game, 128 = balanced, 255 = full chat
            let raw = buf[2] as f32;
            let normalized = (raw - 128.0) / 128.0;
            Ok(normalized.clamp(-1.0, 1.0))
        } else {
            Ok(0.0) // Default to balanced
        }
    }

    fn set_auto_off(&mut self, minutes: u8) -> Result<()> {
        let data = [0x06, 0x31, minutes];
        self.send_report(&data)
    }
}
