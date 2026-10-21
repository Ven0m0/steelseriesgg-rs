//! Apex Pro TKL 2023 specific implementations.

use super::{GenericKeyboard, Keyboard};
use crate::Result;
use crate::devices::hid_reports::{
    ActuationCommand, HidCommand, HidDeviceType, HidReportBuilder, KEYBOARD_REPORT_SIZE,
};
use crate::devices::key_mapping::{KeyAddress, KeyId, KeyMapping};
use crate::devices::product_ids;
use crate::devices::zone_mapping::{ZoneEffect, ZoneMapping};
use crate::devices::{Device, DeviceInfo, DeviceType};
use crate::rgb::{Color, PerKeyEffect};
use async_trait::async_trait;
use std::ops::{Deref, DerefMut};

/// Apex 2023 new protocol constants (from OpenRGB reverse engineering).
const APEX_2023_PACKET_LENGTH: usize = 643;
const APEX_2023_PACKET_ID_INIT: u8 = 0x4B;
const APEX_2023_PACKET_ID_DIRECT_WIRED: u8 = 0x40;
const APEX_2023_PACKET_ID_DIRECT_WIRELESS: u8 = 0x61;
/// [EXPERIMENTAL] Direct-mode packet ID of the original Apex Pro (PID 0x1610): OpenRGB's
/// `APEX_GEN1_PACKET_ID_DIRECT`, same 643-byte layout and key table, no init. Untested here (issue #283).
const APEX_GEN1_PACKET_ID_DIRECT: u8 = 0x3A;

/// TKL key HID codes (from OpenRGB SteelSeriesApexController.cpp).
const TKL_KEYS: &[u8] = &[
    0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16,
    0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29,
    0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x3B, 0x3C, 0x3D,
    0x3E, 0x3F, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F, 0x50,
    0x51, 0x52, 0x64, 0xE0, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xF0, 0x31, 0x87, 0x88, 0x89, 0x8A, 0x8B, 0x53,
    0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F, 0x60, 0x61, 0x62, 0x63,
    // The SteelSeries logo LED. Omitting it leaves the logo lit in the previous
    // profile colour while the rest of the board follows direct mode.
    0xFB,
];

/// [EXPERIMENTAL] Key table SteelSeries GG sends to the wired Apex Pro TKL 2023 (PID 0x1628).
/// Read from a USBPcap capture of GG (2026-10-14, firmware 1.19.7); order is GG's, count byte was
/// always 0x54. See docs/development/apex-pro-tkl-2023-gg-capture.md.
const WIRED_2023_KEYS: &[u8] = &[
    0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16,
    0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x29, 0x2A,
    0x2B, 0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x3F,
    0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F, 0x50, 0x51, 0x52, 0xE0, 0xE1, 0xE2,
    0xE3, 0xE4, 0xE5, 0xE7, 0xF0, 0x28, 0x31, 0xE6,
];

/// [EXPERIMENTAL] Report-ID byte plus the 644 bytes GG puts on the wire for the wired 2023 model.
const WIRED_2023_PACKET_LENGTH: usize = 645;

/// [EXPERIMENTAL] Slot order of the per-key tables in GG's 12,500-byte profile blob for the wired
/// Apex Pro TKL 2023 (PID 0x1628): index = slot, value = HID usage. Physical rows, left to right.
///
/// Source: GG USBPcap captures and GG's engine database (2026-10-14, firmware 1.19.7).
/// - Captured with a single-key edit: slots 1, 16, 24, 27, 29, 37, 41, 42, 44, 53, 55, 56, 57, 58, 60, 63-67.
/// - Matched as a set against GG's stored config: ISO/JIS keys (13, 40, 43, 54, 59, 61, 62; identity within
///   the set follows the JIS/ISO layout) and Tab/Caps (14, 28).
/// - Remaining slots follow the row order; slot 0 (grave) is the only key left for it.
///
/// The checksum at offset 12320 is solved (`apex_pro_tkl_2023_profile`), but writing the blob stays blocked:
/// no read-back command is known.
pub const WIRED_2023_PROFILE_SLOTS: [u8; 68] = [
    // ` 1 2 3 4 5 6 7 8 9 0 - = Yen
    0x35, 0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x2D, 0x2E, 0x89,
    // Tab Q W E R T Y U I O P [ ] \
    0x2B, 0x14, 0x1A, 0x08, 0x15, 0x17, 0x1C, 0x18, 0x0C, 0x12, 0x13, 0x2F, 0x30, 0x31,
    // Caps A S D F G H J K L ; ' ISO-# Enter
    0x39, 0x04, 0x16, 0x07, 0x09, 0x0A, 0x0B, 0x0D, 0x0E, 0x0F, 0x33, 0x34, 0x32, 0x28,
    // LShift ISO-\ Z X C V B N M , . / Ro RShift
    0xE1, 0x64, 0x1D, 0x1B, 0x06, 0x19, 0x05, 0x11, 0x10, 0x36, 0x37, 0x38, 0x87, 0xE5,
    // LCtrl LGui LAlt Muhenkan Space Henkan Kana RAlt RGui Fn RCtrl Backspace
    0xE0, 0xE3, 0xE2, 0x8B, 0x2C, 0x8A, 0x88, 0xE6, 0xE7, 0xF0, 0xE4, 0x2A,
];

/// [EXPERIMENTAL] Profile-blob offset of the actuation-point table (GG `hall_thresholds.h`).
/// Each per-key table has 70 slots; slots 68-69 are unused.
pub const PROFILE_ACTUATION_OFFSET: usize = 1084;
/// [EXPERIMENTAL] Reset point (GG `hall_thresholds.l`).
pub const PROFILE_RESET_OFFSET: usize = 1154;
/// [EXPERIMENTAL] Release mode (GG `release_mode`): 0 off, 2 Rapid Trigger, 3 Key Guard.
pub const PROFILE_RELEASE_MODE_OFFSET: usize = 1446;
/// [EXPERIMENTAL] Rapid Trigger sensitivity (GG `rapid_trigger_sensitivity`).
pub const PROFILE_RT_SENSITIVITY_OFFSET: usize = 1516;

/// [EXPERIMENTAL] Slot of an analog key in the wired 2023 profile tables, or `None` for keys
/// without one (Esc, F-row, navigation cluster).
pub fn profile_slot(usage: u8) -> Option<usize> {
    WIRED_2023_PROFILE_SLOTS.iter().position(|&u| u == usage)
}

/// Colour of `usage` in a per-key request (last entry wins), else its colour in `previous`, else black.
fn per_key_color(key_colors: &[(u8, Color)], previous: &[(u8, Color)], usage: u8) -> Color {
    [key_colors, previous]
        .into_iter()
        .find_map(|list| list.iter().rev().find(|(k, _)| *k == usage))
        .map_or(Color::BLACK, |(_, c)| *c)
}

/// File holding the last direct frame ssgg sent, as `[usage][R][G][B]` entries. The board has no
/// colour read-back, so this is the only way a per-key request can leave the other keys alone.
/// shortcut: only knows what ssgg sent; GG or on-board changes since then are invisible, upgrade if a
/// read-back command is found.
fn last_frame_path(product_id: u16) -> Option<std::path::PathBuf> {
    crate::config::Config::config_dir().map(|dir| dir.join(format!("direct-frame-{product_id:04x}.bin")))
}

fn parse_last_frame(bytes: &[u8]) -> Vec<(u8, Color)> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|e| (e[0], Color::new(e[1], e[2], e[3])))
        .collect()
}

/// Apex Pro TKL 2023 implementation; also drives the other models that take direct-mode lighting
/// frames as feature reports (Gen 3, and `[EXPERIMENTAL]` the original Apex Pro).
pub struct ApexProTkl2023 {
    inner: GenericKeyboard,
    initialized_new_protocol: bool,
}

impl ApexProTkl2023 {
    /// Product ID for Apex Pro TKL 2023.
    pub const PRODUCT_ID: u16 = 0x1628;

    /// Create from a generic keyboard.
    pub fn new(keyboard: GenericKeyboard) -> Self {
        Self {
            inner: keyboard,
            initialized_new_protocol: false,
        }
    }

    /// Create for wireless model without a hidapi device handle.
    /// Uses raw hidraw feature reports only.
    pub fn new_wireless_raw(info: DeviceInfo) -> Self {
        Self {
            inner: GenericKeyboard::new_without_device(info),
            initialized_new_protocol: false,
        }
    }

    /// Whether this device uses the new 2023 per-key direct protocol.
    fn uses_new_protocol(&self) -> bool {
        self.direct_packet_id().is_some()
    }

    /// The wired 2023 model (`0x1628`) uses GG's own key table and frame size, and its 0x4B init
    /// was not seen in the capture.
    fn is_wired_2023(&self) -> bool {
        self.inner.info().product_id == product_ids::APEX_PRO_TKL_2023
    }

    /// The original Apex Pro speaks OpenRGB's "Gen 1" direct protocol, which needs no init report.
    fn is_gen1(&self) -> bool {
        self.inner.info().product_id == product_ids::APEX_PRO
    }

    fn direct_keys(&self) -> &'static [u8] {
        if self.is_wired_2023() {
            WIRED_2023_KEYS
        } else {
            TKL_KEYS
        }
    }

    fn direct_packet_length(&self) -> usize {
        if self.is_wired_2023() {
            WIRED_2023_PACKET_LENGTH
        } else {
            APEX_2023_PACKET_LENGTH
        }
    }

    /// Whether this is a wireless variant.
    fn is_wireless(&self) -> bool {
        matches!(
            self.inner.info().product_id,
            product_ids::APEX_PRO_TKL_2023_WIRELESS
                | product_ids::APEX_PRO_TKL_2023_WIRELESS_2
                | product_ids::APEX_PRO_TKL_WIRELESS_2024_DONGLE
                | product_ids::APEX_PRO_TKL_WIRELESS_2024
        )
    }

    /// Direct-mode packet ID, or `None` if the model does not speak the 643-byte
    /// feature-report protocol.
    ///
    /// Verified on Apex Pro TKL Wireless Gen 3 hardware: the dongle (`0x1644`) and
    /// the same keyboard on USB cable (`0x1646`) both answer to the wireless packet
    /// ID `0x61`, while the wired-only Gen 3 model (`0x1642`) uses `0x40`.
    fn direct_packet_id(&self) -> Option<u8> {
        Some(match self.inner.info().product_id {
            product_ids::APEX_PRO_TKL_2023_WIRELESS
            | product_ids::APEX_PRO_TKL_2023_WIRELESS_2
            | product_ids::APEX_PRO_TKL_WIRELESS_2024_DONGLE
            | product_ids::APEX_PRO_TKL_WIRELESS_2024 => APEX_2023_PACKET_ID_DIRECT_WIRELESS,
            product_ids::APEX_PRO_TKL_2024 | product_ids::APEX_PRO_TKL_2023 => APEX_2023_PACKET_ID_DIRECT_WIRED,
            product_ids::APEX_PRO => APEX_GEN1_PACKET_ID_DIRECT,
            _ => return None,
        })
    }

    /// Send the 0x4B initialization feature report required by new protocol.
    fn ensure_new_protocol_init(&mut self) -> Result<()> {
        if self.initialized_new_protocol || self.is_wired_2023() || self.is_gen1() {
            return Ok(());
        }
        let mut buf = vec![0u8; APEX_2023_PACKET_LENGTH];
        buf[0] = 0x00; // Report ID
        buf[1] = APEX_2023_PACKET_ID_INIT;
        self.inner.send_feature(&buf, APEX_2023_PACKET_LENGTH)?;
        // The controller needs a moment before it will honour direct-mode packets.
        std::thread::sleep(std::time::Duration::from_millis(50));
        self.initialized_new_protocol = true;
        tracing::info!("Sent Apex 2023 new protocol init (0x4B)");
        Ok(())
    }

    /// Send one direct-mode feature report covering every key in `direct_keys()`.
    fn send_direct_frame(&mut self, color_for: impl Fn(u8) -> Color) -> Result<()> {
        self.ensure_new_protocol_init()?;
        let buf = self.build_direct_frame(color_for);
        self.inner.send_feature(&buf, buf.len())?;
        self.save_last_frame(&buf);
        Ok(())
    }

    /// Remember the entries of a frame that reached the board. Best effort: a failed save only costs
    /// the next per-key request its merge base, so it is logged and not propagated.
    fn save_last_frame(&self, frame: &[u8]) {
        let Some(path) = last_frame_path(self.inner.info().product_id) else {
            return;
        };
        let entries = &frame[3..3 + self.direct_keys().len() * 4];
        let saved = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .map_err(crate::Error::from)
            .and_then(|()| crate::fs_utils::secure_write(&path, entries));
        if let Err(e) = saved {
            tracing::warn!("Could not save the last direct frame to {}: {e}", path.display());
        }
    }

    fn load_last_frame(&self) -> Vec<(u8, Color)> {
        last_frame_path(self.inner.info().product_id)
            .and_then(|path| std::fs::read(path).ok())
            .map(|bytes| parse_last_frame(&bytes))
            .unwrap_or_default()
    }

    fn build_direct_frame(&self, color_for: impl Fn(u8) -> Color) -> Vec<u8> {
        let packet_id = self.direct_packet_id().unwrap_or(APEX_2023_PACKET_ID_DIRECT_WIRED);

        let keys = self.direct_keys();
        let packet_length = self.direct_packet_length();
        let mut buf = vec![0u8; packet_length];
        buf[0] = 0x00; // Report ID
        buf[1] = packet_id;
        buf[2] = keys.len() as u8;

        for (i, &hid_code) in keys.iter().enumerate() {
            let color = color_for(hid_code);
            let offset = 3 + i * 4;
            buf[offset] = hid_code;
            buf[offset + 1] = color.r;
            buf[offset + 2] = color.g;
            buf[offset + 3] = color.b;
        }
        buf
    }

    /// Send per-key direct RGB using the new 643-byte feature report protocol.
    fn send_direct_rgb_new_protocol(&mut self, color: Color) -> Result<()> {
        self.send_direct_frame(|_| color)
    }

    /// [EXPERIMENTAL] Per-key colours as one direct frame. A frame replaces every key and the board has
    /// no known read-back, so unlisted keys keep the colour of the last frame ssgg sent (black if none).
    fn send_direct_key_colors(&mut self, key_colors: &[(u8, Color)]) -> Result<()> {
        for (usage, _) in key_colors.iter().filter(|(u, _)| !self.direct_keys().contains(u)) {
            tracing::warn!("HID usage {usage:#04x} is not addressable in direct mode; skipped");
        }
        if !key_colors.iter().any(|(usage, _)| self.direct_keys().contains(usage)) {
            return Err(crate::Error::DeviceCommunication(
                "None of the requested keys is addressable in direct mode".to_string(),
            ));
        }
        let previous = self.load_last_frame();
        if previous.is_empty() {
            tracing::warn!("No saved frame; keys not named are sent black");
        }
        self.send_direct_frame(|usage| per_key_color(key_colors, &previous, usage))
    }

    /// Set actuation point for all keys (global).
    /// Value is in 0.1mm increments (e.g. 4 = 0.4mm, 36 = 3.6mm).
    /// Send actuation command and update cache
    fn send_actuation_command(&mut self, command: ActuationCommand) -> Result<()> {
        let report_builder = HidReportBuilder::new(HidDeviceType::Keyboard);
        let mut buffer = [0u8; KEYBOARD_REPORT_SIZE];
        let size = report_builder.build_report(command.clone(), &mut buffer)?;
        self.inner.send_raw(&buffer[..size])?;
        self.inner.update_cached_actuation_point(command.actuation_point);
        Ok(())
    }

    pub fn set_actuation_point(&mut self, value: u8) -> Result<()> {
        let command = ActuationCommand::new(value);
        command.validate()?;
        self.send_actuation_command(command)
    }

    /// Set actuation point in millimeters.
    /// Precision is limited to 0.1mm increments.
    pub fn set_actuation_point_mm(&mut self, mm: f32) -> Result<()> {
        let command = ActuationCommand::from_mm(mm);
        self.set_actuation_point(command.actuation_point)
    }
}

// Delegate Device trait
impl Device for ApexProTkl2023 {
    fn info(&self) -> &DeviceInfo {
        self.inner.info()
    }

    fn device_type(&self) -> DeviceType {
        self.inner.device_type()
    }

    fn initialize(&mut self) -> Result<()> {
        // Wireless variant uses feature reports for init, not output reports.
        // The 0x4B init is sent lazily in ensure_new_protocol_init().
        if self.is_wireless() || self.is_gen1() {
            return Ok(());
        }
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
crate::impl_keyboard_with_delegation!(ApexProTkl2023, {
    async fn set_color(&mut self, color: Color) -> Result<()> {
        if self.uses_new_protocol() {
            return self.send_direct_rgb_new_protocol(color);
        }
        self.inner.set_color(color).await
    }

    async fn set_zone_colors(&mut self, colors: &[Color]) -> Result<()> {
        if self.uses_new_protocol() {
            // For zone colors, just use the first color for all keys via direct protocol
            let color = colors.first().copied().unwrap_or(Color::BLACK);
            return self.send_direct_rgb_new_protocol(color);
        }
        self.inner.set_zone_colors(colors).await
    }

    async fn apply(&mut self) -> Result<()> {
        // Direct mode is live-streamed. The APPLY/save command (0x09) makes the
        // controller fall back to its stored profile, discarding the colours we
        // just sent, so the board visibly ignores every RGB command.
        if self.uses_new_protocol() {
            return Ok(());
        }
        self.inner.apply().await
    }

    async fn set_brightness(&mut self, brightness: u8) -> Result<()> {
        self.inner.set_brightness(brightness).await
    }

    async fn set_key_color(&mut self, key_id: KeyId, color: Color) -> Result<()> {
        self.set_key_colors(&[(key_id, color)]).await
    }

    async fn set_key_colors(&mut self, key_colors: &[(KeyId, Color)]) -> Result<()> {
        if !self.uses_new_protocol() {
            return self.inner.set_key_colors(key_colors).await;
        }
        let mapping = self.inner.get_key_mapping().ok_or_else(|| {
            crate::Error::DeviceCommunication("Per-key RGB not supported - no key mapping available".to_string())
        })?;
        let mut usages = Vec::with_capacity(key_colors.len());
        for (key_id, color) in key_colors {
            match mapping.get_key_address(*key_id) {
                Some(address) => usages.push((address.hid_code, *color)),
                None => tracing::warn!("Key {key_id:?} not found in key mapping"),
            }
        }
        self.send_direct_key_colors(&usages)
    }

    async fn set_key_color_direct(&mut self, address: KeyAddress, color: Color) -> Result<()> {
        self.set_key_colors_direct(&[(address, color)]).await
    }

    async fn set_key_colors_direct(&mut self, key_colors: &[(KeyAddress, Color)]) -> Result<()> {
        if !self.uses_new_protocol() {
            return self.inner.set_key_colors_direct(key_colors).await;
        }
        let usages: Vec<(u8, Color)> = key_colors.iter().map(|(a, c)| (a.hid_code, *c)).collect();
        self.send_direct_key_colors(&usages)
    }

    async fn set_key_region(&mut self, start_hid: u8, count: u8, color: Color) -> Result<()> {
        if !self.uses_new_protocol() {
            return self.inner.set_key_region(start_hid, count, color).await;
        }
        let usages: Vec<(u8, Color)> = (0..count).map(|i| (start_hid.wrapping_add(i), color)).collect();
        self.send_direct_key_colors(&usages)
    }

    fn read_actuation_point(&mut self) -> Result<u8> {
        self.inner.read_actuation_point()
    }

    fn set_actuation_point(&mut self, value: u8) -> Result<()> {
        self.set_actuation_point(value)
    }

    fn set_actuation_point_mm(&mut self, mm: f32) -> Result<()> {
        self.set_actuation_point_mm(mm)
    }
});

// Deref allows access to Device trait methods like send_raw/receive_raw
impl Deref for ApexProTkl2023 {
    type Target = GenericKeyboard;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for ApexProTkl2023 {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keyboard_for(product_id: u16) -> ApexProTkl2023 {
        let info = DeviceInfo {
            name: std::borrow::Cow::Borrowed("Test Apex Pro TKL"),
            device_type: DeviceType::Keyboard,
            vendor_id: crate::STEELSERIES_VENDOR_ID,
            product_id,
            interface_number: 3,
            usage_page: 0xFFC0,
            usage: 0x01,
            serial_number: None,
            manufacturer: Some("SteelSeries".to_string()),
            path: "/test/device/path".to_string(),
        };
        ApexProTkl2023::new_wireless_raw(info)
    }

    #[test]
    fn gen3_models_use_the_direct_protocol() {
        for pid in [
            product_ids::APEX_PRO_TKL_2024,
            product_ids::APEX_PRO_TKL_WIRELESS_2024_DONGLE,
            product_ids::APEX_PRO_TKL_WIRELESS_2024,
        ] {
            assert!(
                keyboard_for(pid).uses_new_protocol(),
                "PID {pid:#06x} should use the 643-byte direct protocol"
            );
        }
    }

    #[test]
    fn gen3_packet_ids_match_hardware() {
        // Verified on hardware: the dongle and the same board on cable both answer
        // to the wireless packet ID; the wired-only model uses 0x40.
        assert_eq!(
            keyboard_for(product_ids::APEX_PRO_TKL_WIRELESS_2024_DONGLE).direct_packet_id(),
            Some(APEX_2023_PACKET_ID_DIRECT_WIRELESS)
        );
        assert_eq!(
            keyboard_for(product_ids::APEX_PRO_TKL_WIRELESS_2024).direct_packet_id(),
            Some(APEX_2023_PACKET_ID_DIRECT_WIRELESS)
        );
        assert_eq!(
            keyboard_for(product_ids::APEX_PRO_TKL_2024).direct_packet_id(),
            Some(APEX_2023_PACKET_ID_DIRECT_WIRED)
        );
    }

    #[test]
    fn wired_2023_uses_the_gg_captured_layout() {
        // [EXPERIMENTAL] layout read from a GG USBPcap capture, not yet confirmed on hardware.
        let keyboard = keyboard_for(product_ids::APEX_PRO_TKL_2023);
        assert!(keyboard.uses_new_protocol());
        assert_eq!(keyboard.direct_packet_id(), Some(APEX_2023_PACKET_ID_DIRECT_WIRED));
        assert_eq!(keyboard.direct_keys().len(), 84);
        assert_eq!(keyboard.direct_packet_length(), 645);
        assert!(3 + keyboard.direct_keys().len() * 4 <= WIRED_2023_PACKET_LENGTH);
    }

    #[test]
    fn wired_2023_per_key_frame_lights_only_requested_keys() {
        let keyboard = keyboard_for(product_ids::APEX_PRO_TKL_2023);
        let requested = [(0x1A, Color::RED), (0x29, Color::BLUE)]; // W, Esc
        let frame = keyboard.build_direct_frame(|usage| per_key_color(&requested, &[], usage));

        assert_eq!(frame.len(), WIRED_2023_PACKET_LENGTH);
        assert_eq!(&frame[..3], &[0x00, APEX_2023_PACKET_ID_DIRECT_WIRED, 0x54]);
        for (i, entry) in frame[3..3 + WIRED_2023_KEYS.len() * 4]
            .as_chunks::<4>()
            .0
            .iter()
            .enumerate()
        {
            assert_eq!(entry[0], WIRED_2023_KEYS[i]);
            let expected = match entry[0] {
                0x1A => [255, 0, 0],
                0x29 => [0, 0, 255],
                _ => [0, 0, 0],
            };
            assert_eq!(&entry[1..], &expected, "usage {:#04x}", entry[0]);
        }
        assert!(frame[3 + WIRED_2023_KEYS.len() * 4..].iter().all(|&b| b == 0));
    }

    #[test]
    fn per_key_request_keeps_previous_colours_for_unlisted_keys() {
        let previous = parse_last_frame(&[0x1A, 1, 2, 3, 0x04, 9, 9, 9]);
        let requested = [(0x04, Color::RED)];

        assert_eq!(per_key_color(&requested, &previous, 0x04), Color::RED);
        assert_eq!(per_key_color(&requested, &previous, 0x1A), Color::new(1, 2, 3));
        assert_eq!(per_key_color(&requested, &previous, 0x05), Color::BLACK);
    }

    #[test]
    fn wired_2023_key_table_has_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        assert!(WIRED_2023_KEYS.iter().all(|k| seen.insert(*k)));
    }

    #[test]
    fn profile_slots_cover_exactly_ggs_analog_keys() {
        // `hall_thresholds.keys[].hid` from GG's stored config for this model, sorted by HID.
        let mut gg_analog_keys: Vec<u8> = (0x04..=0x39).filter(|&u| u != 0x29).collect();
        gg_analog_keys.extend([
            0x64, 0x87, 0x88, 0x89, 0x8A, 0x8B, 0xE0, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xF0,
        ]);
        let mut slots = WIRED_2023_PROFILE_SLOTS.to_vec();
        slots.sort_unstable();
        assert_eq!(slots, gg_analog_keys);
    }

    #[test]
    fn profile_slots_match_captured_single_key_edits() {
        let captured = [
            (0x1E, 1),  // 1
            (0x1A, 16), // W
            (0x13, 24), // P
            (0x31, 27), // backslash
            (0x04, 29), // A
            (0x0F, 37), // L
            (0x28, 41), // Enter
            (0xE1, 42), // LShift
            (0x1D, 44), // Z
            (0x38, 53), // slash
            (0xE5, 55), // RShift
            (0xE0, 56), // LCtrl
            (0xE3, 57), // LGui
            (0xE2, 58), // LAlt
            (0x2C, 60), // Space
            (0xE6, 63), // RAlt
            (0xE7, 64), // RGui / Menu
            (0xF0, 65), // Fn
            (0xE4, 66), // RCtrl
            (0x2A, 67), // Backspace
        ];
        for (usage, slot) in captured {
            assert_eq!(profile_slot(usage), Some(slot), "usage {usage:#04x}");
        }
    }

    #[test]
    fn keys_without_analog_switches_have_no_profile_slot() {
        assert_eq!(profile_slot(0x29), None); // Esc
        assert_eq!(profile_slot(0x3A), None); // F1
        assert_eq!(profile_slot(0x4F), None); // Right arrow
    }

    #[test]
    fn original_apex_pro_uses_openrgb_gen1_frame() {
        let keyboard = keyboard_for(product_ids::APEX_PRO);
        assert_eq!(keyboard.direct_packet_id(), Some(APEX_GEN1_PACKET_ID_DIRECT));
        let frame = keyboard.build_direct_frame(|_| Color::RED);
        assert_eq!(frame.len(), APEX_2023_PACKET_LENGTH);
        assert_eq!(&frame[..7], &[0x00, 0x3A, 112, 0x04, 0xFF, 0x00, 0x00]);
    }

    #[test]
    fn gen3_keeps_the_112_key_table() {
        let keyboard = keyboard_for(product_ids::APEX_PRO_TKL_2024);
        assert_eq!(keyboard.direct_keys().len(), 112);
        assert_eq!(keyboard.direct_packet_length(), APEX_2023_PACKET_LENGTH);
    }

    #[test]
    fn key_table_covers_every_led_including_the_logo() {
        assert_eq!(TKL_KEYS.len(), 112);
        assert_eq!(*TKL_KEYS.last().unwrap(), 0xFB, "logo LED must be addressed");
        // The direct packet is a 3-byte header plus 4 bytes per key.
        assert!(3 + TKL_KEYS.len() * 4 <= APEX_2023_PACKET_LENGTH);
    }
}
