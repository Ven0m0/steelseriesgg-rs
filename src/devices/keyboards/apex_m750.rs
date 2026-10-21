//! [EXPERIMENTAL] Apex M750 solid-colour lighting (issue #276).
//!
//! The protocol comes from a community fork and was never run on hardware: three enable feature reports,
//! then 513-byte direct frames of 132 `R G B` slots on HID interface 2. The slot-to-key map is unknown, so
//! only solid colour and off are offered; per-key requests are refused rather than guessed.
//! Nothing here sends APPLY (`0x09`) or any save command.

use super::{GenericKeyboard, Keyboard};
use crate::devices::hid_reports::{
    APEX_M_REPORT_SIZE, APEX_M_SLOT_COUNT, ApexMDirectCommand, ApexMEnableCommand, ApexMEnableStep, HidCommand,
    HidDeviceType,
};
use crate::devices::key_mapping::{KeyAddress, KeyId};
use crate::devices::{Device, DeviceInfo, DeviceType};
use crate::rgb::Color;
use crate::{Error, Result};
use async_trait::async_trait;

/// USB interface that carries the M750's lighting feature reports.
pub const CONTROL_INTERFACE: i32 = 2;

pub struct ApexM750 {
    inner: GenericKeyboard,
    enabled: bool,
    /// Software brightness (0-100); no brightness command is known for this model.
    brightness: u8,
    /// Last requested (unscaled) colour, resent when the brightness changes.
    last_color: Option<Color>,
}

impl ApexM750 {
    /// Create from a generic keyboard.
    pub fn new(keyboard: GenericKeyboard) -> Self {
        Self {
            inner: keyboard,
            enabled: false,
            brightness: 100,
            last_color: None,
        }
    }

    fn unsupported() -> Error {
        Error::DeviceCommunication("Apex M750 supports solid colour only: its slot-to-key map is unknown".to_string())
    }

    fn send_report(&self, command: impl HidCommand) -> Result<()> {
        // Serialised directly: `HidReportBuilder` would warn about an unknown command byte on every frame,
        // because byte 1 of these reports is 00.
        let mut buf = [0u8; APEX_M_REPORT_SIZE];
        let size = command.serialize(&mut buf, HidDeviceType::Keyboard)?;
        self.inner.send_feature(&buf[..size], size)
    }

    /// Send the three enable reports once per session, before the first frame.
    fn ensure_enabled(&mut self) -> Result<()> {
        if self.enabled {
            return Ok(());
        }
        for step in ApexMEnableStep::ALL {
            self.send_report(ApexMEnableCommand { step })?;
        }
        self.enabled = true;
        tracing::info!("Sent Apex M750 enable sequence (EXPERIMENTAL)");
        Ok(())
    }

    fn send_solid(&mut self, color: Color) -> Result<()> {
        self.ensure_enabled()?;
        let shown = color.scale(f32::from(self.brightness) / 100.0);
        self.send_report(ApexMDirectCommand {
            slots: &[shown; APEX_M_SLOT_COUNT],
        })?;
        self.last_color = Some(color);
        Ok(())
    }

    fn set_solid_brightness(&mut self, level: u8) -> Result<()> {
        self.brightness = level.min(100);
        match self.last_color {
            Some(color) => self.send_solid(color),
            None => Ok(()),
        }
    }
}

impl Device for ApexM750 {
    fn info(&self) -> &DeviceInfo {
        self.inner.info()
    }

    fn device_type(&self) -> DeviceType {
        self.inner.device_type()
    }

    fn initialize(&mut self) -> Result<()> {
        // GenericKeyboard::initialize sends APPLY (0x09), which this model must never receive.
        Ok(())
    }

    fn close(&mut self) -> Result<()> {
        self.inner.close()
    }

    fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    fn send_raw(&mut self, data: &[u8]) -> Result<()> {
        self.inner.send_raw(data)
    }

    fn receive_raw(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.inner.receive_raw(buf)
    }
}

// Hand-written instead of `impl_keyboard_with_delegation!`: the macro forwards the per-key effect and zone
// effect methods to `GenericKeyboard`, which send its generic `0x21` frames and APPLY (`0x09`). The trait
// defaults for those methods send nothing.
#[async_trait]
impl Keyboard for ApexM750 {
    fn zone_count(&self) -> usize {
        self.inner.zone_count()
    }

    async fn clear_per_key_rgb(&mut self) -> Result<()> {
        self.set_color(Color::BLACK).await
    }

    async fn set_color(&mut self, color: Color) -> Result<()> {
        self.send_solid(color)
    }

    async fn set_zone_colors(&mut self, colors: &[Color]) -> Result<()> {
        self.send_solid(colors.first().copied().unwrap_or(Color::BLACK))
    }

    async fn apply(&mut self) -> Result<()> {
        Ok(())
    }

    async fn set_brightness(&mut self, brightness: u8) -> Result<()> {
        self.set_solid_brightness(brightness)
    }

    async fn set_key_color(&mut self, _key_id: KeyId, _color: Color) -> Result<()> {
        Err(Self::unsupported())
    }

    async fn set_key_colors(&mut self, _key_colors: &[(KeyId, Color)]) -> Result<()> {
        Err(Self::unsupported())
    }

    async fn set_key_color_direct(&mut self, _address: KeyAddress, _color: Color) -> Result<()> {
        Err(Self::unsupported())
    }

    async fn set_key_colors_direct(&mut self, _key_colors: &[(KeyAddress, Color)]) -> Result<()> {
        Err(Self::unsupported())
    }

    async fn set_key_region(&mut self, _start_hid: u8, _count: u8, _color: Color) -> Result<()> {
        Err(Self::unsupported())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::product_ids;

    fn keyboard() -> ApexM750 {
        let info = DeviceInfo {
            name: std::borrow::Cow::Borrowed("Test Apex M750"),
            device_type: DeviceType::Keyboard,
            vendor_id: crate::STEELSERIES_VENDOR_ID,
            product_id: product_ids::APEX_M750,
            interface_number: CONTROL_INTERFACE,
            usage_page: 0xFF00,
            usage: 0x01,
            serial_number: None,
            manufacturer: Some("SteelSeries".to_string()),
            path: "/test/device/path".to_string(),
        };
        ApexM750::new(GenericKeyboard::new_without_device(info))
    }

    #[tokio::test]
    async fn per_key_requests_are_refused_not_guessed() {
        let mut keyboard = keyboard();
        let address = KeyAddress::new(0x04);
        assert!(keyboard.set_key_color(KeyId::A, Color::RED).await.is_err());
        assert!(keyboard.set_key_colors(&[(KeyId::A, Color::RED)]).await.is_err());
        assert!(keyboard.set_key_color_direct(address, Color::RED).await.is_err());
        assert!(keyboard.set_key_colors_direct(&[(address, Color::RED)]).await.is_err());
        assert!(keyboard.set_key_region(0x04, 2, Color::RED).await.is_err());
        assert!(!keyboard.enabled, "a refused request sends nothing");
    }

    #[tokio::test]
    async fn solid_colour_fails_cleanly_without_a_device_and_does_not_mark_enabled() {
        let mut keyboard = keyboard();
        assert!(keyboard.set_color(Color::RED).await.is_err());
        assert!(!keyboard.enabled);
        assert_eq!(keyboard.last_color, None);
    }

    #[tokio::test]
    async fn brightness_before_any_colour_only_stores_the_level() {
        let mut keyboard = keyboard();
        keyboard.set_brightness(40).await.unwrap();
        assert_eq!(keyboard.brightness, 40);
        keyboard.set_brightness(250).await.unwrap();
        assert_eq!(keyboard.brightness, 100);
    }
}
