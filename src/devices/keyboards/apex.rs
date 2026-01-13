//! Apex keyboard specific implementations.

use super::GenericKeyboard;
use crate::Result;
use crate::devices::Device;
use crate::rgb::Color;

/// Apex 3 TKL specific implementation.
pub struct Apex3Tkl {
    inner: GenericKeyboard,
}

impl Apex3Tkl {
    /// Zone count for Apex 3 TKL (10-zone RGB).
    pub const ZONE_COUNT: usize = 9;

    /// Product ID for Apex 3 TKL.
    pub const PRODUCT_ID: u16 = 0x1622;

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
}

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
