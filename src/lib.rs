//! # SteelSeries GG for Linux
//!
//! A complete open-source replacement for SteelSeries GG on Linux.
//!
//! ## Features
//!
//! - **Device Control**: RGB lighting and more for SteelSeries devices
//! - **Audio Mixer**: Multi-channel audio mixing (Game, Chat, Media, Aux, Mic)
//! - **GameSense**: HTTP API server for game integration and reactive lighting
//! - **Profiles**: Save and load device configurations
//!
//! ## Supported Devices
//!
//! - Keyboards: Apex Pro, Apex 3 TKL, and more
//! - Headsets: Arctis Nova Pro, Arctis 7/9, and more

pub mod config;
pub mod device_state;
pub mod devices;
pub mod error;
pub mod gamesense;
pub mod pollrate;
pub mod profiles;
pub mod rgb;

#[cfg(any(feature = "audio", feature = "sonar"))]
pub mod audio;

pub use error::{Error, Result};

/// SteelSeries USB Vendor ID (official USB-IF assignment)
/// Used to filter HID devices during enumeration
pub const STEELSERIES_VENDOR_ID: u16 = 0x1038;
pub const STEELSERIES_PRODUCT_ID: u16 = 0x1628;

/// Re-export commonly used types
pub mod prelude {
    pub use crate::devices::{Device, DeviceInfo, DeviceManager, DeviceType};
    pub use crate::error::{Error, Result};
    pub use crate::rgb::{Color, Effect, RgbController};
}
