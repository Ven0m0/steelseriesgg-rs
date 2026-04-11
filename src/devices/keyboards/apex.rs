//! Apex keyboard specific implementations.

use super::{GenericKeyboard, Keyboard};
use crate::Result;
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
    /// Zone count for Apex 3 TKL (10-zone RGB).
    pub const ZONE_COUNT: usize = 9;

    /// Product ID for Apex 3 TKL.
    pub const PRODUCT_ID: u16 = 0x1622;

    /// HID command: RGB effect control
    pub const CMD_RGB_EFFECT: u8 = 0x23;

    /// HID command: OSD navigation
    pub const CMD_OSD_NAV: u8 = 0x24;

    /// Create from a generic keyboard.
    pub fn new(keyboard: GenericKeyboard) -> Self {
        Self { inner: keyboard }
    }

    /// Set reactive lighting mode.
    pub fn set_reactive_mode(&mut self, enabled: bool) -> Result<()> {
        let data = if enabled {
            [0x25, 0x01] // Enable reactive
        } else {
            [0x25, 0x00] // Disable reactive
        };
        self.inner.send_raw(&data)
    }

    /// Set color shift effect.
    pub fn set_color_shift(&mut self, color1: Color, color2: Color, speed: u8) -> Result<()> {
        let data = [
            0x26, // Color shift command
            color1.r,
            color1.g,
            color1.b,
            color2.r,
            color2.g,
            color2.b,
            speed.min(100),
        ];
        self.inner.send_raw(&data)
    }

    /// Experimental: RGB effect control - hardware behavior not fully verified.
    #[doc = "Experimental: Hardware behavior not fully verified"]
    pub fn set_rgb_effect(&mut self, effect_id: u8, params: &[u8]) -> Result<()> {
        let mut data = vec![0u8; 65];
        data[0] = 0x00; // Report ID
        data[1] = Self::CMD_RGB_EFFECT;
        data[2] = effect_id;

        // Copy parameters into the report
        for (i, &param) in params.iter().enumerate() {
            if i + 3 >= 65 {
                break; // Prevent buffer overflow
            }
            data[i + 3] = param;
        }

        self.inner.send_raw(&data)
    }

    /// Experimental: OSD navigation command - hardware behavior not fully verified.
    #[doc = "Experimental: Hardware behavior not fully verified"]
    pub fn send_osd_command(&mut self, command: u8) -> Result<()> {
        let data = [0x00, Self::CMD_OSD_NAV, command];
        self.inner.send_raw(&data)
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
        self.inner.initialize()
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
    async fn set_color(&mut self, color: Color) -> Result<()> {
        self.inner.set_color(color).await
    }

    async fn set_zone_colors(&mut self, colors: &[Color]) -> Result<()> {
        self.inner.set_zone_colors(colors).await
    }

    async fn set_key_color(&mut self, key_id: KeyId, color: Color) -> Result<()> {
        self.inner.set_key_color(key_id, color).await
    }

    async fn set_key_colors(&mut self, key_colors: &[(KeyId, Color)]) -> Result<()> {
        self.inner.set_key_colors(key_colors).await
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
