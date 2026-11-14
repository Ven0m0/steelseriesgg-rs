//! Apex keyboard specific implementations.

use super::{GenericKeyboard, Keyboard};
use crate::Result;
use crate::devices::hid_reports::{
    APEX_3_REPORT_SIZE, Apex3Command, Apex8ZoneBrightnessCommand, HidDeviceType, HidReportBuilder,
};
use crate::devices::key_mapping::{KeyAddress, KeyId, KeyMapping};
use crate::devices::zone_mapping::{ZoneEffect, ZoneMapping};
use crate::devices::{Device, DeviceInfo, DeviceType};
use crate::rgb::{Color, PerKeyEffect};
use async_trait::async_trait;

/// Apex 3 TKL specific implementation.
pub struct Apex3Tkl {
    inner: GenericKeyboard,
}

impl Apex3Tkl {
    /// Zone count for Apex 3 TKL (8 zones, as advertised and as modelled by OpenRGB).
    pub const ZONE_COUNT: usize = 8;

    /// Product ID for Apex 3 TKL.
    pub const PRODUCT_ID: u16 = 0x1622;

    /// Create from a generic keyboard.
    pub fn new(keyboard: GenericKeyboard) -> Self {
        Self { inner: keyboard }
    }
}

// Delegate Device trait
impl Device for Apex3Tkl {
    fn info(&self) -> &DeviceInfo {
        self.inner.info()
    }

    fn device_type(&self) -> DeviceType {
        self.inner.device_type()
    }

    fn initialize(&mut self) -> Result<()> {
        // [EXPERIMENTAL] OpenRGB's 8-zone controller never sends the APPLY/save command (0x09), only
        // `21 FF <R G B x8>` and `23 <level>`. The 3.0.4 firmware has no 0x09 handler (static,
        // Ghidra 2026-11-14, docs/development/firmware-notes.md), so omitting it is unlikely to be
        // what fixes issues #173, #257; untested on hardware.
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

// Delegate Keyboard trait
crate::impl_keyboard_with_delegation!(Apex3Tkl, {
    async fn apply(&mut self) -> Result<()> {
        // [EXPERIMENTAL] See `initialize`: the 8-zone frame takes effect without APPLY (0x09).
        Ok(())
    }

    async fn set_color(&mut self, color: Color) -> Result<()> {
        self.inner.set_color(color).await
    }

    async fn set_zone_colors(&mut self, colors: &[Color]) -> Result<()> {
        self.inner.set_zone_colors(colors).await
    }

    async fn set_brightness(&mut self, brightness: u8) -> Result<()> {
        self.inner
            .send_command_async(Apex8ZoneBrightnessCommand::from_percent(brightness))
            .await
    }

    async fn set_key_color(&mut self, key_id: KeyId, color: Color) -> Result<()> {
        self.inner.set_key_color(key_id, color).await
    }

    async fn set_key_colors(&mut self, key_colors: &[(KeyId, Color)]) -> Result<()> {
        self.inner.set_key_colors(key_colors).await
    }

    async fn set_key_color_direct(&mut self, address: KeyAddress, color: Color) -> Result<()> {
        self.inner.set_key_color_direct(address, color).await
    }

    async fn set_key_colors_direct(&mut self, key_colors: &[(KeyAddress, Color)]) -> Result<()> {
        self.inner.set_key_colors_direct(key_colors).await
    }

    async fn set_key_region(&mut self, start_hid: u8, count: u8, color: Color) -> Result<()> {
        self.inner.set_key_region(start_hid, count, color).await
    }

    fn read_actuation_point(&mut self) -> Result<u8> {
        self.inner.read_actuation_point()
    }

    fn set_actuation_point(&mut self, value: u8) -> Result<()> {
        self.inner.set_actuation_point(value)
    }

    fn set_actuation_point_mm(&mut self, mm: f32) -> Result<()> {
        self.inner.set_actuation_point_mm(mm)
    }
});

impl std::ops::Deref for Apex3Tkl {
    type Target = GenericKeyboard;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for Apex3Tkl {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// [EXPERIMENTAL] Apex 3 (`0x161A`): ten zones driven with GG's `set_led_colors` (`00 0B 00 <RGB x10>`) and
/// `global_brightness` (`00 0A 00 <level>`) in 33-byte output reports, from GG's decrypted device spec (`apex_3`,
/// 2026-10-25), untested on hardware. Never sends APPLY: on this board `00 09 00` is `save_to_flash`. GG also
/// sends a driver-mode frame (`00 03 00 00 01`) at start; its effect beyond button routing is not in the spec, so
/// it is not sent.
pub struct Apex3 {
    inner: GenericKeyboard,
}

impl Apex3 {
    /// Zones of the Apex 3 (GG `z1_color` .. `z10_color`).
    pub const ZONE_COUNT: usize = 10;

    /// Create from a generic keyboard.
    pub fn new(keyboard: GenericKeyboard) -> Self {
        Self { inner: keyboard }
    }

    /// The frame `set_zone_colors` sends: the given colours, the last one repeated over missing zones.
    pub fn colors_frame(colors: &[Color]) -> Apex3Command {
        let last = colors.last().copied().unwrap_or(Color::BLACK);
        Apex3Command::Colors(std::array::from_fn(|i| colors.get(i).copied().unwrap_or(last)))
    }

    fn send(&self, command: Apex3Command) -> Result<()> {
        let mut buf = [0u8; APEX_3_REPORT_SIZE];
        let size = HidReportBuilder::new(HidDeviceType::Keyboard).build_report(command, &mut buf)?;
        // Exact 33 bytes: the generic 65-byte path would not match this board's output report.
        self.inner.write_profile_packet(&buf[..size])
    }
}

impl Device for Apex3 {
    fn info(&self) -> &DeviceInfo {
        self.inner.info()
    }

    fn device_type(&self) -> DeviceType {
        self.inner.device_type()
    }

    fn initialize(&mut self) -> Result<()> {
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

crate::impl_keyboard_with_delegation!(Apex3, {
    async fn apply(&mut self) -> Result<()> {
        Ok(())
    }

    async fn set_color(&mut self, color: Color) -> Result<()> {
        self.send(Apex3Command::Colors([color; Apex3::ZONE_COUNT]))
    }

    async fn set_zone_colors(&mut self, colors: &[Color]) -> Result<()> {
        self.send(Apex3::colors_frame(colors))
    }

    async fn set_brightness(&mut self, brightness: u8) -> Result<()> {
        self.send(Apex3Command::Brightness(brightness.min(100)))
    }
});

/// Zone mapping for Apex keyboards.
#[derive(Clone, Copy, Debug)]
pub enum ApexZone {
    /// Left side of keyboard
    Left = 0,
    /// Left-center area
    LeftCenter = 1,
    /// Center area
    Center = 2,
    /// Right-center area
    RightCenter = 3,
    /// Right side of keyboard
    Right = 4,
    /// Function row
    FunctionRow = 5,
    /// Number row
    NumberRow = 6,
    /// WASD cluster
    Wasd = 7,
    /// Arrow keys
    ArrowKeys = 8,
}

impl ApexZone {
    /// Get all zones.
    pub fn all() -> &'static [ApexZone] {
        &[
            ApexZone::Left,
            ApexZone::LeftCenter,
            ApexZone::Center,
            ApexZone::RightCenter,
            ApexZone::Right,
            ApexZone::FunctionRow,
            ApexZone::NumberRow,
            ApexZone::Wasd,
            ApexZone::ArrowKeys,
        ]
    }
}
