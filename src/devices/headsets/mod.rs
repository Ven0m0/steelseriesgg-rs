//! Headset device support (Arctis series).

pub mod nova7;

use super::{Device, DeviceInfo, DeviceType, write_padded_report};
use crate::{Error, Result};
use hidapi::HidDevice;
use parking_lot::Mutex;
use serde::Serialize;
use std::sync::Arc;

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

    /// Which settings the model takes, so a front end can leave out controls it lacks. The default is none.
    fn capabilities(&self) -> HeadsetCapabilities {
        HeadsetCapabilities::default()
    }

    /// Snapshot of battery, link and ChatMix state. The default asks for the battery and ChatMix separately and
    /// reports `connected` and `charging` as unknown.
    fn status(&mut self) -> Result<HeadsetStatus> {
        Ok(HeadsetStatus {
            connected: None,
            charging: None,
            battery_percent: self.battery_level()?,
            chat_mix: Some(self.chat_mix()?),
        })
    }
}

/// Settings a headset model takes. A step count is the number of distinct levels the headset has, so
/// `ssgg`'s 0-100 scale maps onto `0..steps`; 0 means the model has no such setting.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct HeadsetCapabilities {
    pub sidetone_steps: u8,
    pub mic_volume_steps: u8,
    pub auto_off: bool,
    pub chat_mix: bool,
    /// Whether the model answers a status query (battery, connection, charging).
    pub status: bool,
}

/// Headset state as far as the protocol reports it; `None` marks a field the headset cannot report.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct HeadsetStatus {
    pub connected: Option<bool>,
    pub charging: Option<bool>,
    /// Battery charge, 0-100.
    pub battery_percent: Option<u8>,
    /// ChatMix balance, -1.0 = full game, 1.0 = full chat.
    pub chat_mix: Option<f32>,
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

/// A headset ssgg detects but knows no protocol for: it can be listed and raw-accessed, and every
/// [`Headset`] command answers unsupported. Families with a protocol live in [`nova7`].
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
        let device = self
            .device
            .as_ref()
            .ok_or(Error::DeviceCommunication("Device not connected".to_string()))?;
        let device = device.lock();

        // Headsets typically use 64-byte reports with no report ID prefix.
        write_padded_report(&device, data, 64, false)
    }

    /// Receive a HID report from the headset.
    fn receive_report(&mut self, buf: &mut [u8]) -> Result<usize> {
        let device = self
            .device
            .as_ref()
            .ok_or(Error::DeviceCommunication("Device not connected".to_string()))?;
        let device = device.lock();

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

impl GenericHeadset {
    fn no_protocol(&self, what: &str) -> Error {
        Error::DeviceCommunication(format!(
            "{what} is not supported on {} ({:04X}): ssgg knows no protocol for this headset",
            self.info.name, self.info.product_id
        ))
    }
}

/// Every command answers unsupported. The `06 xx` guesses this type used to send were removed on 2026-10-25:
/// GG's decrypted device specs show other frames for every model that has these settings (on the Arctis 7,
/// `06 33` is microphone volume, not the EQ preset that was sent), so a guess could change the wrong setting.
impl Headset for GenericHeadset {
    fn battery_level(&mut self) -> Result<Option<u8>> {
        Err(self.no_protocol("Battery level"))
    }

    fn set_sidetone(&mut self, _level: u8) -> Result<()> {
        Err(self.no_protocol("Sidetone"))
    }

    fn set_mic_volume(&mut self, _volume: u8) -> Result<()> {
        Err(self.no_protocol("Microphone volume"))
    }

    fn set_mic_mute(&mut self, _muted: bool) -> Result<()> {
        Err(self.no_protocol("Microphone mute"))
    }

    fn set_eq_preset(&mut self, _preset: EqPreset) -> Result<()> {
        Err(self.no_protocol("EQ presets"))
    }

    fn chat_mix(&mut self) -> Result<f32> {
        Err(self.no_protocol("ChatMix"))
    }

    fn set_auto_off(&mut self, _minutes: u8) -> Result<()> {
        Err(self.no_protocol("Auto-off"))
    }
}
