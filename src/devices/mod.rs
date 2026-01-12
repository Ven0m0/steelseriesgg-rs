//! Device discovery and management for SteelSeries peripherals.

pub mod discovery;
pub mod headsets;
pub mod keyboards;

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::{Error, Result, STEELSERIES_VENDOR_ID};
use hidapi::HidDevice;

pub use discovery::DeviceManager;

/// Type of SteelSeries device.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum DeviceType {
    Keyboard,
    Headset,
    Unknown,
}

impl fmt::Display for DeviceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeviceType::Keyboard => write!(f, "Keyboard"),
            DeviceType::Headset => write!(f, "Headset"),
            DeviceType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Information about a detected SteelSeries device.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DeviceInfo {
    /// Device name
    pub name: String,

    /// Device type
    pub device_type: DeviceType,

    /// USB Vendor ID
    pub vendor_id: u16,

    /// USB Product ID
    pub product_id: u16,

    /// Interface number
    pub interface_number: i32,

    /// Serial number (if available)
    pub serial_number: Option<String>,

    /// Manufacturer string
    pub manufacturer: Option<String>,

    /// HID device path
    pub path: String,
}

impl DeviceInfo {
    /// Check if this is a SteelSeries device.
    pub fn is_steelseries(&self) -> bool {
        self.vendor_id == STEELSERIES_VENDOR_ID
    }
}

/// Trait for all SteelSeries devices.
pub trait Device: Send + Sync {
    /// Get device information.
    fn info(&self) -> &DeviceInfo;

    /// Get device type.
    fn device_type(&self) -> DeviceType;

    /// Initialize the device.
    fn initialize(&mut self) -> Result<()>;

    /// Close the device connection.
    fn close(&mut self) -> Result<()>;

    /// Check if device is connected.
    fn is_connected(&self) -> bool;

    /// Send raw HID data to the device.
    fn send_raw(&mut self, data: &[u8]) -> Result<()>;

    /// Receive raw HID data from the device.
    fn receive_raw(&mut self, buf: &mut [u8]) -> Result<usize>;
}

/// Write a padded HID report, handling the optional leading report ID byte and
/// constraining to the standard 64/65 byte HID buffers.
pub fn write_padded_report(
    device: &HidDevice,
    data: &[u8],
    report_len: usize,
    include_report_id: bool,
) -> Result<()> {
    use tracing::debug;

    if report_len == 0 || report_len > 65 {
        return Err(Error::DeviceCommunication(format!(
            "Invalid report length {} (expected 1-65)",
            report_len
        )));
    }

    let mut report = [0u8; 65];
    let offset = if include_report_id { 1 } else { 0 };
    let effective_len = report_len.min(report.len());
    let copy_len = data.len().min(effective_len.saturating_sub(offset));

    if copy_len > 0 {
        report[offset..offset + copy_len].copy_from_slice(&data[..copy_len]);
    }

    debug!(
        "Writing HID report: len={}, offset={}, data={:02x?}",
        effective_len,
        offset,
        &report[..effective_len.min(32)] // Show first 32 bytes to avoid log spam
    );

    device.write(&report[..effective_len])?;
    Ok(())
}

/// Known SteelSeries device product IDs.
pub mod product_ids {
    // Keyboards - Apex Series
    pub const APEX_PRO: u16 = 0x1610;
    pub const APEX_PRO_TKL: u16 = 0x1614;
    // NOTE: On actual Apex Pro TKL (2023) hardware we observe PID 0x1628.
    // The previously documented 0x1618 value does not match the device on this
    // system, so we treat 0x1628 as the canonical 2023 TKL PID here.
    pub const APEX_PRO_TKL_2023: u16 = 0x1628;
    pub const APEX_3: u16 = 0x161A;
    pub const APEX_3_TKL: u16 = 0x1622;
    pub const APEX_5: u16 = 0x161C;
    pub const APEX_7: u16 = 0x1612;
    pub const APEX_7_TKL: u16 = 0x1616;

    // Headsets - Arctis Series
    pub const ARCTIS_1: u16 = 0x12AD; // Note: ARCTIS_7 (2017) also uses this ID
    pub const ARCTIS_1_WIRELESS: u16 = 0x12B3;
    pub const ARCTIS_5: u16 = 0x12AA;
    pub const ARCTIS_7_2019: u16 = 0x12CF;
    pub const ARCTIS_9: u16 = 0x12C2;
    pub const ARCTIS_PRO: u16 = 0x1252;
    pub const ARCTIS_PRO_WIRELESS: u16 = 0x1290;
    pub const ARCTIS_NOVA_PRO: u16 = 0x12E0;
    pub const ARCTIS_NOVA_PRO_WIRELESS: u16 = 0x12E4;
    pub const ARCTIS_NOVA_5: u16 = 0x12EA;
    pub const ARCTIS_NOVA_3: u16 = 0x12EC;
    pub const ARCTIS_NOVA_1: u16 = 0x12EE;
}

/// Get device type from product ID.
pub fn device_type_from_product_id(product_id: u16) -> DeviceType {
    use product_ids::*;

    match product_id {
        // Keyboards
        APEX_PRO | APEX_PRO_TKL | APEX_PRO_TKL_2023 | APEX_3 | APEX_3_TKL | APEX_5 | APEX_7
        | APEX_7_TKL => DeviceType::Keyboard,

        // Headsets
        ARCTIS_1  // Note: This ID covers ARCTIS_1 and ARCTIS_7 (2017)
        | ARCTIS_1_WIRELESS
        | ARCTIS_5
        | ARCTIS_7_2019
        | ARCTIS_9
        | ARCTIS_PRO
        | ARCTIS_PRO_WIRELESS
        | ARCTIS_NOVA_PRO
        | ARCTIS_NOVA_PRO_WIRELESS
        | ARCTIS_NOVA_5
        | ARCTIS_NOVA_3
        | ARCTIS_NOVA_1 => DeviceType::Headset,

        _ => DeviceType::Unknown,
    }
}

/// Get device name from product ID.
pub fn device_name_from_product_id(product_id: u16) -> &'static str {
    use product_ids::*;

    match product_id {
        APEX_PRO => "Apex Pro",
        APEX_PRO_TKL => "Apex Pro TKL",
        APEX_PRO_TKL_2023 => "Apex Pro TKL (2023)",
        APEX_3 => "Apex 3",
        APEX_3_TKL => "Apex 3 TKL",
        APEX_5 => "Apex 5",
        APEX_7 => "Apex 7",
        APEX_7_TKL => "Apex 7 TKL",
        ARCTIS_1 => "Arctis 1 / Arctis 7 (2017)",
        ARCTIS_1_WIRELESS => "Arctis 1 Wireless",
        ARCTIS_5 => "Arctis 5",
        ARCTIS_7_2019 => "Arctis 7 (2019)",
        ARCTIS_9 => "Arctis 9",
        ARCTIS_PRO => "Arctis Pro",
        ARCTIS_PRO_WIRELESS => "Arctis Pro Wireless",
        ARCTIS_NOVA_PRO => "Arctis Nova Pro",
        ARCTIS_NOVA_PRO_WIRELESS => "Arctis Nova Pro Wireless",
        ARCTIS_NOVA_5 => "Arctis Nova 5",
        ARCTIS_NOVA_3 => "Arctis Nova 3",
        ARCTIS_NOVA_1 => "Arctis Nova 1",
        _ => "Unknown SteelSeries Device",
    }
}

/// Get RGB zone count for keyboard product IDs.
/// Returns the number of independently controllable RGB zones.
pub fn zone_count_for_product_id(product_id: u16) -> usize {
    use product_ids::*;

    match product_id {
        APEX_3 => 10,                                                // Apex 3 - 10 zones
        APEX_3_TKL => 9,                                             // Apex 3 TKL - 9 zones
        APEX_PRO_TKL_2023 => 9, // Apex Pro TKL (2023) - 9 zones
        APEX_PRO | APEX_PRO_TKL | APEX_5 | APEX_7 | APEX_7_TKL => 1, // Single zone for now
        _ => 1,                 // Default single zone
    }
}
