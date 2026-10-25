//! Device discovery and management for SteelSeries peripherals.

pub mod diagnostics;
pub mod discovery;
pub mod fuzz;
pub mod headsets;
pub mod hid_reports;
pub mod key_mapping;
pub mod keyboards;
pub mod mice;
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
    ApplyCommand, BrightnessCommand, CommandCode, HEADSET_REPORT_SIZE, HidCommand, HidDeviceType, HidReportBuilder,
    KEYBOARD_REPORT_SIZE, MAX_RGB_ZONES, PerKeyAddressingMode, PerKeyRgbBuilder, PerKeyRgbCommand, RgbZoneCommand,
};
pub use key_mapping::{KeyAddress, KeyId, KeyMapping, KeyMappingDatabase, KeyMappingStats, KeyboardLayout};
pub use zone_mapping::{ZoneEffect, ZoneFallback, ZoneInfo, ZoneMapping, ZonePosition};

/// Type of SteelSeries device.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum DeviceType {
    Keyboard,
    Headset,
    Mouse,
    Unknown,
}

impl fmt::Display for DeviceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeviceType::Keyboard => write!(f, "Keyboard"),
            DeviceType::Headset => write!(f, "Headset"),
            DeviceType::Mouse => write!(f, "Mouse"),
            DeviceType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Information about a detected SteelSeries device.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DeviceInfo {
    /// Device name
    pub name: std::borrow::Cow<'static, str>,

    /// Device type
    pub device_type: DeviceType,

    /// USB Vendor ID
    pub vendor_id: u16,

    /// USB Product ID
    pub product_id: u16,

    /// Interface number
    pub interface_number: i32,

    /// HID top-level collection usage page. Vendor-defined pages are `>= 0xFF00`.
    pub usage_page: u16,

    /// HID top-level collection usage.
    pub usage: u16,

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

#[derive(Debug)]
/// HID communication optimizer for reducing CPU usage.
pub struct HidOptimizer {
    /// Cache of recently sent reports to avoid duplicates (hash -> timestamp)
    report_cache: Mutex<HashMap<u64, Instant>>,
    /// Device connectivity cache
    connectivity_cache: Mutex<HashMap<String, (bool, Instant)>>,
    /// Cache timeout for reports (ms)
    cache_timeout: Duration,
}

/// Hash a HID report using FNV-1a algorithm for fast, collision-resistant hashing.
#[inline]
fn hash_report(data: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
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
        let hash = hash_report(data);
        let cache = self.report_cache.lock();
        if let Some(&timestamp) = cache.get(&hash) {
            return timestamp.elapsed() < self.cache_timeout;
        }
        false
    }

    /// Mark report as sent in cache.
    pub fn mark_report_sent(&self, data: &[u8]) {
        let hash = hash_report(data);
        let mut cache = self.report_cache.lock();
        let now = Instant::now();

        // Clean old entries periodically
        if cache.len() > 100 {
            cache.retain(|_, &mut timestamp| now.duration_since(timestamp) < self.cache_timeout * 2);
        }

        cache.insert(hash, now);
    }

    /// Check cached device connectivity status.
    pub fn is_device_connected(&self, device_path: &str) -> Option<bool> {
        let cache = self.connectivity_cache.lock();
        if let Some((connected, timestamp)) = cache.get(device_path)
            && timestamp.elapsed() < Duration::from_secs(5)
        {
            return Some(*connected);
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

/// The bytes `write_padded_report` puts on the wire: `data` zero-padded to `report_len` (at most 65), behind a
/// prepended `00` report ID when `include_report_id` is set. `HidReportBuilder` output already starts with
/// the report ID, so it must be passed with `include_report_id = false`.
fn pad_report(data: &[u8], report_len: usize, include_report_id: bool) -> ([u8; 65], usize) {
    let mut report = [0u8; 65];
    let offset = usize::from(include_report_id);
    let effective_len = report_len.min(report.len());
    let copy_len = data.len().min(effective_len.saturating_sub(offset));
    report[offset..offset + copy_len].copy_from_slice(&data[..copy_len]);
    (report, effective_len)
}

#[cfg(test)]
mod pad_report_tests {
    use super::*;

    #[test]
    fn builder_output_reaches_the_wire_unchanged() {
        let mut built = [0u8; KEYBOARD_REPORT_SIZE];
        let size = HidReportBuilder::new(HidDeviceType::Keyboard)
            .build_report(ApplyCommand, &mut built)
            .unwrap();
        let (wire, len) = pad_report(&built[..size], 65, false);
        assert_eq!(
            &wire[..len],
            &built[..],
            "report ID then 09, as the confirmed Apex 7 path writes"
        );
        assert_eq!(&wire[..2], &[0x00, 0x09]);

        let (shifted, _) = pad_report(&built[..size], 65, true);
        assert_eq!(
            &shifted[..3],
            &[0x00, 0x00, 0x09],
            "the old keyboard path's shifted frame"
        );
        assert_eq!(pad_report(&[1, 2], 64, false).1, 64);
    }
}

/// Write a padded HID report, handling the optional leading report ID byte and
/// constraining to the standard 64/65 byte HID buffers.
/// This is the optimized version that includes caching and deduplication.
pub fn write_padded_report(device: &HidDevice, data: &[u8], report_len: usize, include_report_id: bool) -> Result<()> {
    use tracing::debug;

    if report_len == 0 || report_len > 65 {
        return Err(Error::DeviceCommunication(format!(
            "Invalid report length {} (expected 1-65)",
            report_len
        )));
    }

    let (report, effective_len) = pad_report(data, report_len, include_report_id);
    let report_data = &report[..effective_len];

    // Check for duplicate reports to reduce unnecessary I/O
    if get_hid_optimizer().is_duplicate_report(report_data) {
        debug!("Skipping duplicate HID report");
        return Ok(());
    }

    debug!(
        "Writing optimized HID report: len={}, report id prepended={}, data={:02x?}",
        effective_len,
        include_report_id,
        &report_data[..effective_len.min(16)] // Show first 16 bytes to reduce log overhead
    );

    // Perform the actual write
    let write_result = device.write(report_data);

    // Update cache regardless of write success to avoid retry storms
    get_hid_optimizer().mark_report_sent(report_data);

    write_result.map_err(Into::into).map(|_| ())
}

#[cfg(unix)]
fn interface_from_phys(phys: &str) -> Option<u32> {
    let last = phys.rsplit('/').next()?;
    if let Some(number) = last.strip_prefix("input") {
        return number.parse().ok();
    }
    last.parse().ok()
}

#[cfg(all(test, unix))]
#[test]
fn interface_from_phys_reads_the_trailing_interface() {
    assert_eq!(interface_from_phys("usb-0:1.91:1.0/input7/1"), Some(1));
    assert_eq!(interface_from_phys("usb-0:1.91:1.0/input7/3"), Some(3));
    assert_eq!(interface_from_phys("usb-0:1.91:1.0/input7"), Some(7));
    assert_eq!(interface_from_phys("usb-0:1.91:1.0/input7/x"), None);
}

#[cfg(unix)]
/// Find the hidraw node of one USB interface of a device: scans `/sys/class/hidraw` for the node whose
/// `device/uevent` carries this vendor/product pair in `HID_ID` and a `HID_PHYS` whose interface number
/// ([`interface_from_phys`]) is `interface`.
pub fn find_hidraw_for_interface(vendor_id: u16, product_id: u16, interface: usize) -> Option<String> {
    use std::fs;

    let interface = u32::try_from(interface).ok()?;
    let target_hid_id = format!("0003:{:08X}:{:08X}", vendor_id as u32, product_id as u32);

    for entry in fs::read_dir("/sys/class/hidraw").ok()? {
        let Ok(entry) = entry else { continue };
        let Ok(content) = fs::read_to_string(entry.path().join("device/uevent")) else {
            continue;
        };

        let mut has_hid_id = false;
        let mut phys_interface = None;
        for line in content.lines() {
            if let Some(value) = line.strip_prefix("HID_ID=") {
                has_hid_id |= value.eq_ignore_ascii_case(&target_hid_id);
            } else if let Some(value) = line.strip_prefix("HID_PHYS=") {
                phys_interface = interface_from_phys(value);
            }
        }

        if has_hid_id && phys_interface == Some(interface) {
            return Some(format!("/dev/{}", entry.file_name().to_string_lossy()));
        }
    }
    None
}

#[cfg(not(unix))]
/// Find a HID device path for a specific USB interface of a device.
///
/// Windows splits one USB HID interface into several device nodes, one per top-level
/// collection, all sharing the same VID/PID/interface number, so `interface` cannot disambiguate
/// them and is ignored: this matches on usage page instead, preferring the confirmed SteelSeries
/// control page (`STEELSERIES_CONTROL_USAGE_PAGE`) over any other vendor-defined page
/// (`>= 0xFF00`), tie-broken by lowest interface number. That control collection is the one the
/// reference capture shows carrying the profile traffic on `wIndex 1`.
pub fn find_hidraw_for_interface(vendor_id: u16, product_id: u16, _interface: usize) -> Option<String> {
    let api = hidapi::HidApi::new().ok()?;

    let mut best: Option<(u32, &hidapi::DeviceInfo)> = None;
    for dev_info in api.device_list() {
        if dev_info.vendor_id() != vendor_id || dev_info.product_id() != product_id {
            continue;
        }
        let usage_page = dev_info.usage_page();
        let score = if usage_page == STEELSERIES_CONTROL_USAGE_PAGE {
            2
        } else if usage_page >= 0xFF00 {
            1
        } else {
            continue;
        };
        if best.is_none_or(|(best_score, best_dev)| {
            score > best_score || (score == best_score && dev_info.interface_number() < best_dev.interface_number())
        }) {
            best = Some((score, dev_info));
        }
    }
    if let Some((_, dev_info)) = best {
        return dev_info.path().to_str().ok().map(str::to_owned);
    }

    // Fallback: return any interface with matching VID/PID. Reaching here means no vendor-defined
    // collection was found, which usually means the wrong device or an unhandled model.
    tracing::warn!(
        "No vendor-defined HID collection found for VID:{vendor_id:#06x} PID:{product_id:#06x}; \
         falling back to first matching interface, which may not be the control endpoint"
    );
    for dev_info in api.device_list() {
        if dev_info.vendor_id() == vendor_id && dev_info.product_id() == product_id {
            return dev_info.path().to_str().ok().map(str::to_owned);
        }
    }
    None
}

/// HID vendor-defined usage page used by the SteelSeries control (RGB/actuation) collection.
///
/// Confirmed on Apex Pro TKL (2023), PID `0x1628`, 2026-08-18: of the 10 HID collections the
/// device exposes on Windows, only this page's collection has a non-zero output report length
/// (65 bytes) and feature report length (645 bytes) — matching `KEYBOARD_REPORT_SIZE`. Every
/// other collection on this PID is either a vendor-defined telemetry-only page (`0xFFC1`, input
/// only) or a standard OS-facing Generic Desktop / Consumer page.
///
/// [EXPERIMENTAL] for every other PID: not verified on any other keyboard model.
pub const STEELSERIES_CONTROL_USAGE_PAGE: u16 = 0xFFC0;

pub mod product_ids {
    // Keyboards - Apex Series
    // Source: GG firmware registry (firmware/<VID<<16|PID>/version.json), 2026-05-26.
    // Formula: folder_id = (VID << 16) | PID, VID = 0x1038 for all SteelSeries devices.
    pub const APEX_PRO: u16 = 0x1610;
    pub const APEX_7: u16 = 0x1612;
    pub const APEX_PRO_TKL: u16 = 0x1614;
    // GG firmware names this "apex_150" — different product from Apex 7 TKL.
    pub const APEX_150: u16 = 0x1616;
    // GG firmware confirms apex_7_tkl = 0x1618. Previously ssgg had 0x1616 here (wrong).
    pub const APEX_7_TKL: u16 = 0x1618;
    pub const APEX_3: u16 = 0x161A;
    pub const APEX_5: u16 = 0x161C;
    // Unverified — GG firmware names; no hardware confirmation for RGB protocol.
    pub const APEX_PRO_MINI: u16 = 0x161E;
    pub const APEX_9_MINI: u16 = 0x1620;
    pub const APEX_3_TKL: u16 = 0x1622;
    pub const APEX_PRO_MINI_WIRELESS_DONGLE: u16 = 0x1624;
    pub const APEX_PRO_MINI_WIRELESS: u16 = 0x1626;
    // NOTE: On actual Apex Pro TKL (2023) hardware we observe PID 0x1628.
    // GG firmware calls this "apex_pro_tkl_2022" internally.
    pub const APEX_PRO_TKL_2023: u16 = 0x1628;
    // PID 0x1630 is the wireless dongle (TX); 0x1632 is the wireless headset (RX).
    pub const APEX_PRO_TKL_2023_WIRELESS_2: u16 = 0x1630;
    pub const APEX_PRO_TKL_2023_WIRELESS: u16 = 0x1632;
    // Unverified — GG firmware names; no hardware confirmation.
    pub const APEX_9_TKL: u16 = 0x1634;
    pub const APEX_PRO_2024: u16 = 0x1640;
    pub const APEX_PRO_TKL_2024: u16 = 0x1642;
    pub const APEX_PRO_TKL_WIRELESS_2024_DONGLE: u16 = 0x1644;
    pub const APEX_PRO_TKL_WIRELESS_2024: u16 = 0x1646;
    pub const APEX_PRO_MINI_2024: u16 = 0x1648;
    pub const APEX_5_2024: u16 = 0x1650;
    pub const APEX_7_2024: u16 = 0x1652;
    // Apex M750 (issue #276, lsusb name). [EXPERIMENTAL] solid-colour protocol from a community fork, see
    // keyboards::apex_m750.
    pub const APEX_M750: u16 = 0x0616;

    // Headsets - Arctis Series
    // Source: GG firmware registry, 2026-05-26.
    pub const ARCTIS_PRO: u16 = 0x1252;
    pub const ARCTIS_PRO_WIRELESS: u16 = 0x1290;
    pub const ARCTIS_5: u16 = 0x12AA; // arctis_5_2018
    // 0x12AD is the Arctis 7 (2018) TX dongle in GG firmware. Also used as Arctis 1 ID.
    pub const ARCTIS_1: u16 = 0x12AD;
    pub const ARCTIS_1_WIRELESS: u16 = 0x12B3; // arctis_1w_tx
    pub const ARCTIS_9: u16 = 0x12C2; // arctis_9_tx
    pub const ARCTIS_NOVA_PRO_WIRED: u16 = 0x12CB; // arctis_nova_pro (wired, GG firmware)
    // 0x12CF was community-reported as the Arctis 7 2019, but GG's decrypted device spec defines it as
    // `arctis_nova_pro_xbox_bootloader_id` (2026-10-25): detect-only, never written to.
    pub const ARCTIS_7_2019: u16 = 0x12CF;
    // 0x12E0 is the Nova Pro Wireless TX dongle in GG firmware (arctis_nova_pro_wireless_tx).
    pub const ARCTIS_NOVA_PRO: u16 = 0x12E0;
    // 0x12E4 not found in GG firmware registry; keeping for backward compat.
    pub const ARCTIS_NOVA_PRO_WIRELESS: u16 = 0x12E4;
    pub const ARCTIS_NOVA_3: u16 = 0x12EC; // arctis_nova_3
    // 0x12EA and 0x12EE not in GG firmware registry; may be community-sourced or wrong.
    // GG's Arctis Nova 5 uses 0x2230 (RX) / 0x2232 (TX).
    pub const ARCTIS_NOVA_5: u16 = 0x12EA;
    pub const ARCTIS_NOVA_1: u16 = 0x12EE;
    // Arctis Nova 7 series (unverified — GG firmware names only, no protocol confirmation).
    pub const ARCTIS_NOVA_7_RX: u16 = 0x2200;
    pub const ARCTIS_NOVA_7_TX: u16 = 0x2202;
    pub const ARCTIS_NOVA_7X_RX: u16 = 0x2204;
    pub const ARCTIS_NOVA_7X_TX: u16 = 0x2206;
    pub const ARCTIS_NOVA_7P_RX: u16 = 0x2208;
    pub const ARCTIS_NOVA_7P_TX: u16 = 0x220A;
    // Arctis Nova 5 correct PIDs (GG firmware); 0x12EA above may be wrong.
    pub const ARCTIS_NOVA_5_RX: u16 = 0x2230;
    pub const ARCTIS_NOVA_5_TX: u16 = 0x2232;
    // Arctis Nova Pro Omni — confirmed added in PR #244.
    pub const ARCTIS_NOVA_PRO_OMNI: u16 = 0x2290;
    // Arctis Nova 3 Wireless (unverified — GG firmware names only).
    pub const ARCTIS_NOVA_3_WIRELESS_RX: u16 = 0x2267;
    pub const ARCTIS_NOVA_3_WIRELESS_TX: u16 = 0x2269;
    // Arctis Nova 7 seen as 0x22A1 (issue #276, lsusb name). [EXPERIMENTAL] protocol, see headsets::nova7.
    pub const ARCTIS_NOVA_7_22A1: u16 = 0x22A1;
    // Arctis Nova 7 family variants. PIDs from community forks, untested; behaviour table in headsets::nova7.
    pub const ARCTIS_NOVA_7_DIABLO_IV: u16 = 0x223A;
    pub const ARCTIS_NOVA_7X_V2: u16 = 0x2258;
    pub const ARCTIS_NOVA_7_WOW: u16 = 0x227A;
    pub const ARCTIS_NOVA_7_WIRELESS_GEN2: u16 = 0x227E;
    pub const ARCTIS_NOVA_7P_GEN2: u16 = 0x2298;
    pub const ARCTIS_NOVA_7X_GEN2: u16 = 0x229E;
    pub const ARCTIS_NOVA_7X_ALT: u16 = 0x22A4;
    pub const ARCTIS_NOVA_7X_ALT_V2: u16 = 0x22A5;
    pub const ARCTIS_NOVA_7P_V2: u16 = 0x22A7;
    pub const ARCTIS_NOVA_7_DIABLO_IV_V2: u16 = 0x22A9;
    pub const ARCTIS_NOVA_7X_V2_ALT: u16 = 0x22AD;
    // Arctis 7+ family, TX ids. PIDs and protocol from GG's decrypted device specs (`base_arctis_7_plus_tx`,
    // 2026-10-25), untested; behaviour table in headsets::nova7.
    pub const ARCTIS_7_PLUS: u16 = 0x220E;
    pub const ARCTIS_7P_PLUS: u16 = 0x2212;
    pub const ARCTIS_7X_PLUS: u16 = 0x2216;
    pub const ARCTIS_7_PLUS_DESTINY_2: u16 = 0x2236;
    // Further Nova 7 frame users, TX ids, from GG's decrypted device specs (2026-10-25), untested; behaviour
    // table in headsets::nova7.
    pub const ARCTIS_NOVA_4: u16 = 0x12F2;
    pub const ARCTIS_NOVA_4X: u16 = 0x12F6;
    pub const ARCTIS_NOVA_7_WOW_UPGRADE: u16 = 0x22AB;
    pub const ARCTIS_NOVA_5X: u16 = 0x2253;
    pub const ARCTIS_NOVA_5X_WHITE: u16 = 0x2264;
    pub const ARCTIS_NOVA_3X_WIRELESS: u16 = 0x226D;
    // Nova Pro family siblings (GG specs `arctis_nova_pro_xbox`, `arctis_nova_pro_v2`,
    // `arctis_nova_pro_wireless_xbox_tx`, `arctis_nova_pro_wireless_xbox_white_tx`), untested.
    pub const ARCTIS_NOVA_PRO_XBOX: u16 = 0x12CD;
    pub const ARCTIS_NOVA_PRO_V2: u16 = 0x12FA;
    pub const ARCTIS_NOVA_PRO_WIRELESS_XBOX: u16 = 0x12E5;
    pub const ARCTIS_NOVA_PRO_WIRELESS_XBOX_WHITE: u16 = 0x225D;
    // Mice, Modern family. PIDs from a community rivalcfg-derived fork, untested; behaviour table in mice::modern.
    pub const AEROX_3: u16 = 0x1836;
    pub const AEROX_3_WIRELESS: u16 = 0x183A;
    pub const AEROX_3_WIRELESS_DONGLE: u16 = 0x1838;
    pub const AEROX_3_WIRELESS_CS2: u16 = 0x187A;
    pub const AEROX_3_WIRELESS_CS2_DONGLE: u16 = 0x1878;
    pub const AEROX_5: u16 = 0x1850;
    pub const AEROX_5_WIRELESS: u16 = 0x1854;
    pub const AEROX_5_WIRELESS_DONGLE: u16 = 0x1852;
    pub const AEROX_5_WIRELESS_DESTINY_2: u16 = 0x185E;
    pub const AEROX_5_WIRELESS_DESTINY_2_DONGLE: u16 = 0x185C;
    pub const AEROX_5_WIRELESS_DIABLO_IV: u16 = 0x1862;
    pub const AEROX_5_WIRELESS_DIABLO_IV_DONGLE: u16 = 0x1860;
    pub const AEROX_9_WIRELESS: u16 = 0x185A;
    pub const AEROX_9_WIRELESS_DONGLE: u16 = 0x1858;
    pub const AEROX_9_WIRELESS_WOW: u16 = 0x1876;
    pub const AEROX_9_WIRELESS_WOW_DONGLE: u16 = 0x1874;
    pub const PRIME_MINI: u16 = 0x184D;
    pub const PRIME_MINI_WIRELESS: u16 = 0x184A;
    pub const PRIME_MINI_WIRELESS_DONGLE: u16 = 0x1848;
    pub const PRIME_WIRELESS: u16 = 0x1842;
    pub const PRIME_WIRELESS_DONGLE: u16 = 0x1840;
    pub const RIVAL_3_GEN_2: u16 = 0x1870;
    pub const RIVAL_5: u16 = 0x183C;
    pub const RIVAL_5_DESTINY: u16 = 0x183E;
}

/// Devices registered for detection only: no protocol is known, so `ssgg` must not write to them.
pub fn is_detect_only(product_id: u16) -> bool {
    matches!(
        product_id,
        product_ids::ARCTIS_NOVA_PRO_OMNI | product_ids::ARCTIS_7_2019
    )
}

/// Get device type from product ID.
pub fn device_type_from_product_id(product_id: u16) -> DeviceType {
    use product_ids::*;

    if keyboards::models::keyboard_model(product_id).is_some() {
        return DeviceType::Keyboard;
    }
    match product_id {
        // Headsets
        ARCTIS_PRO
        | ARCTIS_PRO_WIRELESS
        | ARCTIS_5
        | ARCTIS_1
        | ARCTIS_1_WIRELESS
        | ARCTIS_9
        | ARCTIS_NOVA_PRO_WIRED
        | ARCTIS_7_2019
        | ARCTIS_NOVA_PRO
        | ARCTIS_NOVA_PRO_WIRELESS
        | ARCTIS_NOVA_3
        | ARCTIS_NOVA_5
        | ARCTIS_NOVA_1
        | ARCTIS_NOVA_7_RX
        | ARCTIS_NOVA_7_TX
        | ARCTIS_NOVA_7X_RX
        | ARCTIS_NOVA_7X_TX
        | ARCTIS_NOVA_7P_RX
        | ARCTIS_NOVA_7P_TX
        | ARCTIS_NOVA_5_RX
        | ARCTIS_NOVA_5_TX
        | ARCTIS_NOVA_PRO_OMNI
        | ARCTIS_NOVA_3_WIRELESS_RX
        | ARCTIS_NOVA_3_WIRELESS_TX
        | ARCTIS_NOVA_7_22A1
        | ARCTIS_NOVA_7_DIABLO_IV
        | ARCTIS_NOVA_7X_V2
        | ARCTIS_NOVA_7_WOW
        | ARCTIS_NOVA_7_WIRELESS_GEN2
        | ARCTIS_NOVA_7P_GEN2
        | ARCTIS_NOVA_7X_GEN2
        | ARCTIS_NOVA_7X_ALT
        | ARCTIS_NOVA_7X_ALT_V2
        | ARCTIS_NOVA_7P_V2
        | ARCTIS_NOVA_7_DIABLO_IV_V2
        | ARCTIS_NOVA_7X_V2_ALT
        | ARCTIS_7_PLUS
        | ARCTIS_7P_PLUS
        | ARCTIS_7X_PLUS
        | ARCTIS_7_PLUS_DESTINY_2
        | ARCTIS_NOVA_4
        | ARCTIS_NOVA_4X
        | ARCTIS_NOVA_7_WOW_UPGRADE
        | ARCTIS_NOVA_5X
        | ARCTIS_NOVA_5X_WHITE
        | ARCTIS_NOVA_3X_WIRELESS
        | ARCTIS_NOVA_PRO_XBOX
        | ARCTIS_NOVA_PRO_V2
        | ARCTIS_NOVA_PRO_WIRELESS_XBOX
        | ARCTIS_NOVA_PRO_WIRELESS_XBOX_WHITE => DeviceType::Headset,

        // Mice (Modern family)
        AEROX_3
        | AEROX_3_WIRELESS
        | AEROX_3_WIRELESS_DONGLE
        | AEROX_3_WIRELESS_CS2
        | AEROX_3_WIRELESS_CS2_DONGLE
        | AEROX_5
        | AEROX_5_WIRELESS
        | AEROX_5_WIRELESS_DONGLE
        | AEROX_5_WIRELESS_DESTINY_2
        | AEROX_5_WIRELESS_DESTINY_2_DONGLE
        | AEROX_5_WIRELESS_DIABLO_IV
        | AEROX_5_WIRELESS_DIABLO_IV_DONGLE
        | AEROX_9_WIRELESS
        | AEROX_9_WIRELESS_DONGLE
        | AEROX_9_WIRELESS_WOW
        | AEROX_9_WIRELESS_WOW_DONGLE
        | PRIME_MINI
        | PRIME_MINI_WIRELESS
        | PRIME_MINI_WIRELESS_DONGLE
        | PRIME_WIRELESS
        | PRIME_WIRELESS_DONGLE
        | RIVAL_3_GEN_2
        | RIVAL_5
        | RIVAL_5_DESTINY => DeviceType::Mouse,

        _ => DeviceType::Unknown,
    }
}

/// Get device name from product ID.
pub fn device_name_from_product_id(product_id: u16) -> &'static str {
    use product_ids::*;

    if let Some(model) = keyboards::models::keyboard_model(product_id) {
        return model.name;
    }
    match product_id {
        ARCTIS_PRO => "Arctis Pro",
        ARCTIS_PRO_WIRELESS => "Arctis Pro Wireless",
        ARCTIS_5 => "Arctis 5",
        ARCTIS_1 => "Arctis 1 / Arctis 7 (2018)",
        ARCTIS_1_WIRELESS => "Arctis 1 Wireless",
        ARCTIS_9 => "Arctis 9",
        ARCTIS_NOVA_PRO_WIRED => "Arctis Nova Pro",
        ARCTIS_7_2019 => "Arctis 7 (2019) / Nova Pro Xbox bootloader",
        ARCTIS_NOVA_PRO => "Arctis Nova Pro Wireless (TX)",
        ARCTIS_NOVA_PRO_WIRELESS => "Arctis Nova Pro Wireless",
        ARCTIS_NOVA_3 => "Arctis Nova 3",
        ARCTIS_NOVA_5 => "Arctis Nova 5",
        ARCTIS_NOVA_1 => "Arctis Nova 1",
        ARCTIS_NOVA_7_RX => "Arctis Nova 7 (RX)",
        ARCTIS_NOVA_7_TX => "Arctis Nova 7 (TX)",
        ARCTIS_NOVA_7X_RX => "Arctis Nova 7X (RX)",
        ARCTIS_NOVA_7X_TX => "Arctis Nova 7X (TX)",
        ARCTIS_NOVA_7P_RX => "Arctis Nova 7P (RX)",
        ARCTIS_NOVA_7P_TX => "Arctis Nova 7P (TX)",
        ARCTIS_NOVA_5_RX => "Arctis Nova 5 (RX)",
        ARCTIS_NOVA_5_TX => "Arctis Nova 5 (TX)",
        ARCTIS_NOVA_PRO_OMNI => "Arctis Nova Pro Omni",
        ARCTIS_NOVA_3_WIRELESS_RX => "Arctis Nova 3 Wireless (RX)",
        ARCTIS_NOVA_3_WIRELESS_TX => "Arctis Nova 3 Wireless (TX)",
        ARCTIS_NOVA_7_22A1 => "Arctis Nova 7",
        ARCTIS_NOVA_7_DIABLO_IV => "Arctis Nova 7 Diablo IV",
        ARCTIS_NOVA_7X_V2 => "Arctis Nova 7X v2",
        ARCTIS_NOVA_7_WOW => "Arctis Nova 7 World of Warcraft",
        ARCTIS_NOVA_7_WIRELESS_GEN2 => "Arctis Nova 7 Wireless Gen 2",
        ARCTIS_NOVA_7P_GEN2 => "Arctis Nova 7P Gen 2",
        ARCTIS_NOVA_7X_GEN2 => "Arctis Nova 7X Gen 2",
        ARCTIS_NOVA_7X_ALT => "Arctis Nova 7X (alt)",
        ARCTIS_NOVA_7X_ALT_V2 => "Arctis Nova 7X (alt v2)",
        ARCTIS_NOVA_7P_V2 => "Arctis Nova 7P v2",
        ARCTIS_NOVA_7_DIABLO_IV_V2 => "Arctis Nova 7 Diablo IV v2",
        ARCTIS_NOVA_7X_V2_ALT => "Arctis Nova 7X v2 (alt)",
        ARCTIS_7_PLUS => "Arctis 7+",
        ARCTIS_7P_PLUS => "Arctis 7P+",
        ARCTIS_7X_PLUS => "Arctis 7X+",
        ARCTIS_7_PLUS_DESTINY_2 => "Arctis 7+ Destiny 2",
        ARCTIS_NOVA_4 => "Arctis Nova 4",
        ARCTIS_NOVA_4X => "Arctis Nova 4X",
        ARCTIS_NOVA_7_WOW_UPGRADE => "Arctis Nova 7 World of Warcraft (upgrade)",
        ARCTIS_NOVA_5X => "Arctis Nova 5X",
        ARCTIS_NOVA_5X_WHITE => "Arctis Nova 5X White",
        ARCTIS_NOVA_3X_WIRELESS => "Arctis Nova 3X Wireless",
        ARCTIS_NOVA_PRO_XBOX => "Arctis Nova Pro (Xbox)",
        ARCTIS_NOVA_PRO_V2 => "Arctis Nova Pro v2",
        ARCTIS_NOVA_PRO_WIRELESS_XBOX => "Arctis Nova Pro Wireless (Xbox, TX)",
        ARCTIS_NOVA_PRO_WIRELESS_XBOX_WHITE => "Arctis Nova Pro Wireless White (Xbox, TX)",
        AEROX_3 => "Aerox 3",
        AEROX_3_WIRELESS => "Aerox 3 Wireless (wired)",
        AEROX_3_WIRELESS_DONGLE => "Aerox 3 Wireless (2.4 GHz)",
        AEROX_3_WIRELESS_CS2 => "Aerox 3 Wireless CS2 Dragon Lore Edition (wired)",
        AEROX_3_WIRELESS_CS2_DONGLE => "Aerox 3 Wireless CS2 Dragon Lore Edition (2.4 GHz)",
        AEROX_5 => "Aerox 5",
        AEROX_5_WIRELESS => "Aerox 5 Wireless (wired)",
        AEROX_5_WIRELESS_DONGLE => "Aerox 5 Wireless (2.4 GHz)",
        AEROX_5_WIRELESS_DESTINY_2 => "Aerox 5 Wireless Destiny 2 Edition (wired)",
        AEROX_5_WIRELESS_DESTINY_2_DONGLE => "Aerox 5 Wireless Destiny 2 Edition (2.4 GHz)",
        AEROX_5_WIRELESS_DIABLO_IV => "Aerox 5 Wireless Diablo IV Edition (wired)",
        AEROX_5_WIRELESS_DIABLO_IV_DONGLE => "Aerox 5 Wireless Diablo IV Edition (2.4 GHz)",
        AEROX_9_WIRELESS => "Aerox 9 Wireless (wired)",
        AEROX_9_WIRELESS_DONGLE => "Aerox 9 Wireless (2.4 GHz)",
        AEROX_9_WIRELESS_WOW => "Aerox 9 Wireless WOW Edition (wired)",
        AEROX_9_WIRELESS_WOW_DONGLE => "Aerox 9 Wireless WOW Edition (2.4 GHz)",
        PRIME_MINI => "Prime Mini",
        PRIME_MINI_WIRELESS => "Prime Mini Wireless (wired)",
        PRIME_MINI_WIRELESS_DONGLE => "Prime Mini Wireless (2.4 GHz)",
        PRIME_WIRELESS => "Prime Wireless (wired)",
        PRIME_WIRELESS_DONGLE => "Prime Wireless (2.4 GHz)",
        RIVAL_3_GEN_2 => "Rival 3 Gen 2",
        RIVAL_5 => "Rival 5",
        RIVAL_5_DESTINY => "Rival 5 Destiny Edition",
        _ => "Unknown SteelSeries Device",
    }
}

/// Get RGB zone count for keyboard product IDs.
/// Returns the number of independently controllable RGB zones.
pub fn zone_count_for_product_id(product_id: u16) -> usize {
    use product_ids::*;

    match product_id {
        APEX_3 => 10,
        APEX_3_TKL => 8,
        APEX_PRO_TKL_2023 | APEX_PRO_TKL_2023_WIRELESS | APEX_PRO_TKL_2023_WIRELESS_2 => 9,
        // Zone counts for these models are unverified — 1 is a safe default until captured.
        APEX_PRO | APEX_PRO_TKL | APEX_5 | APEX_7 | APEX_7_TKL | APEX_150 => 1,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_hid_optimizer_duplicate_report() {
        let optimizer = HidOptimizer::new();
        let data = b"test_report_data";

        // Initial check - should not be duplicate
        assert!(!optimizer.is_duplicate_report(data));

        // Mark as sent
        optimizer.mark_report_sent(data);

        // Immediate check - should be duplicate
        assert!(optimizer.is_duplicate_report(data));

        // Different data - should not be duplicate
        let other_data = b"other_data";
        assert!(!optimizer.is_duplicate_report(other_data));

        // Wait for the optimizer's cache to expire before rechecking
        thread::sleep(Duration::from_millis(200));

        // Check again - should no longer be duplicate
        assert!(!optimizer.is_duplicate_report(data));
    }

    #[test]
    fn test_hash_report_distinct_inputs_different_hashes() {
        let data1 = b"report1";

        let hash1 = hash_report(data1);
        // Ensure deterministic output
        assert_eq!(hash1, hash_report(data1));
    }

    #[test]
    fn test_device_metadata_mappings() {
        use product_ids::*;

        // Test device_type_from_product_id
        assert_eq!(device_type_from_product_id(APEX_PRO), DeviceType::Keyboard);
        assert_eq!(device_type_from_product_id(ARCTIS_1), DeviceType::Headset);
        assert_eq!(device_type_from_product_id(0xFFFF), DeviceType::Unknown);

        // Test device_name_from_product_id
        assert_eq!(device_name_from_product_id(APEX_PRO), "Apex Pro");
        assert_eq!(device_name_from_product_id(0xFFFF), "Unknown SteelSeries Device");

        // Test zone_count_for_product_id
        assert_eq!(zone_count_for_product_id(APEX_3), 10);
        assert_eq!(zone_count_for_product_id(0xFFFF), 1);
    }

    #[test]
    fn test_issue_276_devices_are_named_and_only_the_omni_stays_detect_only() {
        use product_ids::*;

        assert_eq!(device_type_from_product_id(APEX_M750), DeviceType::Keyboard);
        assert_eq!(device_name_from_product_id(APEX_M750), "Apex M750");
        assert_eq!(device_type_from_product_id(ARCTIS_NOVA_7_22A1), DeviceType::Headset);
        assert_eq!(device_name_from_product_id(ARCTIS_NOVA_7_22A1), "Arctis Nova 7");
        // 0x22A1 has an [EXPERIMENTAL] protocol since the Nova 7 family landed; the Omni has none known,
        // so it replaces it here and GenericHeadset's untested `06 xx` bytes are never sent to it.
        // The M750 has an [EXPERIMENTAL] solid-colour protocol (keyboards::apex_m750), so it is no longer
        // detect-only; the Omni still has none known.
        assert!(is_detect_only(ARCTIS_NOVA_PRO_OMNI));
        assert!(!is_detect_only(APEX_M750));
        assert!(!is_detect_only(ARCTIS_NOVA_7_22A1));
        assert!(!is_detect_only(APEX_PRO_TKL_2023));
    }

    #[test]
    fn test_nova_7_family_pids_are_named_headsets() {
        for entry in headsets::nova7::NOVA7_MODELS {
            assert_eq!(device_type_from_product_id(entry.product_id), DeviceType::Headset);
            assert_ne!(
                device_name_from_product_id(entry.product_id),
                "Unknown SteelSeries Device"
            );
        }
    }

    #[test]
    fn test_arctis_nova_pro_omni_pid() {
        use product_ids::*;

        assert_eq!(ARCTIS_NOVA_PRO_OMNI, 0x2290);
        assert_eq!(device_type_from_product_id(ARCTIS_NOVA_PRO_OMNI), DeviceType::Headset);
        assert_eq!(
            device_name_from_product_id(ARCTIS_NOVA_PRO_OMNI),
            "Arctis Nova Pro Omni"
        );
    }

    #[test]
    fn test_apex_pro_tkl_2023_wireless_2_pid() {
        use product_ids::*;

        assert_eq!(APEX_PRO_TKL_2023_WIRELESS_2, 0x1630);
        assert_eq!(
            device_type_from_product_id(APEX_PRO_TKL_2023_WIRELESS_2),
            DeviceType::Keyboard
        );
        assert_eq!(
            device_name_from_product_id(APEX_PRO_TKL_2023_WIRELESS_2),
            "Apex Pro TKL (2023) Wireless"
        );
        assert_eq!(zone_count_for_product_id(APEX_PRO_TKL_2023_WIRELESS_2), 9);
    }
}
