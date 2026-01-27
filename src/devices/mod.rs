//! Device discovery and management for SteelSeries peripherals.

pub mod diagnostics;
pub mod discovery;
pub mod fuzz;
pub mod headsets;
pub mod hid_reports;
pub mod key_mapping;
pub mod keyboards;
pub mod zone_mapping;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

use crate::{Error, Result, STEELSERIES_VENDOR_ID};
use hidapi::HidDevice;

pub use discovery::DeviceManager;
pub use hid_reports::{
    ApplyCommand, BrightnessCommand, CommandCode, HEADSET_REPORT_SIZE, HidCommand, HidDeviceType,
    HidReportBuilder, KEYBOARD_REPORT_SIZE, MAX_RGB_ZONES, PerKeyAddressingMode, PerKeyRgbBuilder,
    PerKeyRgbCommand, RgbZoneCommand,
};
pub use key_mapping::{
    KeyAddress, KeyId, KeyMapping, KeyMappingDatabase, KeyMappingStats, KeyboardLayout,
};
pub use zone_mapping::{ZoneEffect, ZoneFallback, ZoneInfo, ZoneMapping, ZonePosition};

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

    /// Optimized write that batches RGB zone updates and reduces syscalls.
    fn optimized_write(&mut self, reports: &[&[u8]]) -> Result<()> {
        // Default implementation falls back to individual sends
        // Implementations can override for true batching
        for report in reports {
            self.send_raw(report)?;
        }
        Ok(())
    }
}

/// Report cache entry for deduplication.
#[derive(Debug, Clone)]
struct CachedReport {
    #[allow(dead_code)]
    data: Vec<u8>,
    last_sent: Instant,
}

/// HID communication optimizer for reducing CPU usage.
pub struct HidOptimizer {
    /// Cache of recently sent reports to avoid duplicates
    report_cache: Mutex<HashMap<Vec<u8>, CachedReport>>,
    /// Device connectivity cache
    connectivity_cache: Mutex<HashMap<String, (bool, Instant)>>,
    /// Cache timeout for reports (ms)
    cache_timeout: Duration,
}

impl Default for HidOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

impl HidOptimizer {
    /// Create a new HID optimizer.
    pub fn new() -> Self {
        Self {
            report_cache: Mutex::new(HashMap::new()),
            connectivity_cache: Mutex::new(HashMap::new()),
            cache_timeout: Duration::from_millis(50), // 50ms cache timeout
        }
    }

    /// Check if report is a duplicate and should be skipped.
    pub fn is_duplicate_report(&self, data: &[u8]) -> bool {
        let cache = self.report_cache.lock();
        if let Some(cached) = cache.get(data) {
            return cached.last_sent.elapsed() < self.cache_timeout;
        }
        false
    }

    /// Mark report as sent in cache.
    pub fn mark_report_sent(&self, data: &[u8]) {
        let mut cache = self.report_cache.lock();
        // Clean old entries periodically
        if cache.len() > 100 {
            let now = Instant::now();
            cache.retain(|_, cached| {
                now.duration_since(cached.last_sent) < self.cache_timeout * 2
            });
        }

        cache.insert(
            data.to_vec(),
            CachedReport {
                data: data.to_vec(),
                last_sent: Instant::now(),
            },
        );
    }

    /// Check cached device connectivity status.
    pub fn is_device_connected(&self, device_path: &str) -> Option<bool> {
        let cache = self.connectivity_cache.lock();
        if let Some((connected, timestamp)) = cache.get(device_path) {
            if timestamp.elapsed() < Duration::from_secs(5) {
                return Some(*connected);
            }
        }
        None
    }

    /// Update device connectivity cache.
    pub fn update_connectivity_cache(&self, device_path: &str, connected: bool) {
        let mut cache = self.connectivity_cache.lock();
        cache.insert(device_path.to_string(), (connected, Instant::now()));
    }
}

// Global optimizer instance using OnceLock for thread-safe initialization
use std::sync::OnceLock;
static HID_OPTIMIZER: OnceLock<HidOptimizer> = OnceLock::new();

/// Get the global HID optimizer instance.
fn get_hid_optimizer() -> &'static HidOptimizer {
    HID_OPTIMIZER.get_or_init(HidOptimizer::new)
}

/// Write a padded HID report, handling the optional leading report ID byte and
/// constraining to the standard 64/65 byte HID buffers.
/// This is the optimized version that includes caching and deduplication.
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

    let report_data = &report[..effective_len];

    // Check for duplicate reports to reduce unnecessary I/O
    if get_hid_optimizer().is_duplicate_report(report_data) {
        debug!("Skipping duplicate HID report");
        return Ok(());
    }

    debug!(
        "Writing optimized HID report: len={}, offset={}, data={:02x?}",
        effective_len,
        offset,
        &report_data[..effective_len.min(16)] // Show first 16 bytes to reduce log overhead
    );

    // Perform the actual write
    let write_result = device.write(report_data);

    // Update cache regardless of write success to avoid retry storms
    get_hid_optimizer().mark_report_sent(report_data);

    write_result.map_err(Into::into).map(|_| ())
}

/// Batch write multiple HID reports to reduce syscall overhead.
pub fn batch_write_reports(
    device: &HidDevice,
    reports: &[&[u8]],
    report_len: usize,
    include_report_id: bool,
) -> Result<()> {
    use tracing::trace;

    if reports.is_empty() {
        return Ok(());
    }

    // Prepare all reports in advance to minimize lock duration
    let mut prepared_reports = Vec::with_capacity(reports.len());
    let offset = if include_report_id { 1 } else { 0 };
    let effective_len = report_len.min(65);

    for &data in reports {
        let mut report = [0u8; 65];
        let copy_len = data.len().min(effective_len.saturating_sub(offset));

        if copy_len > 0 {
            report[offset..offset + copy_len].copy_from_slice(&data[..copy_len]);
        }

        // Skip duplicate reports
        let report_data = &report[..effective_len];
        if !get_hid_optimizer().is_duplicate_report(report_data) {
            prepared_reports.push(report);
        }
    }

    if prepared_reports.is_empty() {
        trace!("All reports were duplicates, skipping batch write");
        return Ok(());
    }

    trace!(
        "Batch writing {} HID reports (filtered from {})",
        prepared_reports.len(),
        reports.len()
    );

    // Perform batched writes with minimal error handling overhead
    for report in &prepared_reports {
        let report_data = &report[..effective_len];

        // Quick write without individual duplicate checking
        if let Err(e) = device.write(report_data) {
            // On error, still mark remaining reports as sent to avoid retries
            for remaining_report in &prepared_reports {
                let remaining_data = &remaining_report[..effective_len];
                get_hid_optimizer().mark_report_sent(remaining_data);
            }
            return Err(e.into());
        }

        // Mark as sent
        get_hid_optimizer().mark_report_sent(report_data);
    }

    Ok(())
}

/// Write coalesced reports for rapid successive updates (animation scenarios).
pub fn write_coalesced_reports(
    device: &HidDevice,
    data_buffer: &[u8],
    chunk_size: usize,
    report_len: usize,
    include_report_id: bool,
    coalesce_delay: Duration,
) -> Result<()> {
    use std::thread;
    use tracing::trace;

    if chunk_size == 0 {
        return Err(Error::DeviceCommunication("Invalid chunk size".to_string()));
    }

    // Coalesce rapid updates by waiting briefly
    if coalesce_delay > Duration::ZERO {
        thread::sleep(coalesce_delay);
    }

    let chunks: Vec<&[u8]> = data_buffer.chunks(chunk_size).collect();

    trace!(
        "Writing {} coalesced report chunks with {}ms delay",
        chunks.len(),
        coalesce_delay.as_millis()
    );

    batch_write_reports(device, &chunks, report_len, include_report_id)
}

/// Optimized write with mutex lock duration optimization.
/// Prepares the report outside the critical section to minimize lock time.
pub fn optimized_write_with_prep<F>(
    device: &HidDevice,
    data: &[u8],
    report_len: usize,
    include_report_id: bool,
    prep_fn: F,
) -> Result<()>
where
    F: FnOnce(&[u8]) -> Vec<u8>,
{
    use tracing::trace;

    // Prepare the report outside any locks
    let prepared_data = prep_fn(data);

    // Check for duplicates before the critical section
    if get_hid_optimizer().is_duplicate_report(&prepared_data) {
        trace!("Skipping duplicate optimized report");
        return Ok(());
    }

    // Minimize time in critical section
    write_padded_report(device, &prepared_data, report_len, include_report_id)
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
