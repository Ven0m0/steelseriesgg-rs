//! Mouse support.
//!
//! Only the Modern family of the community rivalcfg project is covered (see [`modern`]). Prime and Prime+,
//! the older Rival, Sensei and Kana families and the older wireless families use other command sets and are
//! not implemented. No mouse command has been run on hardware.

pub mod dpi_tables;
pub mod modern;

use super::Device;
use crate::Result;
use crate::rgb::Color;

/// Trait for mouse-specific functionality. Every setting here is applied to the running mouse only; nothing
/// is saved to its on-board memory.
pub trait Mouse: Device {
    /// Set the DPI of the active stage. Rejects a value outside the model's range or between its steps.
    fn set_dpi(&mut self, dpi: u16) -> Result<()>;

    /// Set the USB polling rate in Hz (125, 250, 500 or 1000).
    fn set_poll_rate(&mut self, hz: u16) -> Result<()>;

    /// Set every LED zone to one steady colour.
    fn set_color(&mut self, color: Color) -> Result<()>;

    /// Battery level (0-100), or `None` for a model without a battery or a reply that cannot be read.
    fn battery_level(&mut self) -> Result<Option<u8>>;
}
