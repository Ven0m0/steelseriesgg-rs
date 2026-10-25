//! Apex Pro TKL 2023 specific implementations.

use super::apex_pro_tkl_2023_profile::{PROFILE_LEN, actuation_raw, checksum_matches};
use super::models::{DirectFrame, keyboard_model};
use super::{GenericKeyboard, Keyboard};
use crate::Result;
use crate::devices::hid_reports::{
    ActuationCommand, AnalogKeyCommand, Apex2022DisplayCommand, CommandCode, DirectFrameCommand, DirectInitCommand,
    Gen3ProfileReadCommand, HallLayer, HidCommand, HidDeviceType, HidReportBuilder, KEYBOARD_REPORT_SIZE,
    OnboardFirmwareVersionCommand, PROFILE_CHUNK_REPORT_SIZE, RapidTapPair,
};
use crate::devices::key_mapping::{KeyAddress, KeyId, KeyMapping};
use crate::devices::product_ids;
use crate::devices::zone_mapping::{ZoneEffect, ZoneMapping};
use crate::devices::{Device, DeviceInfo, DeviceType};
use crate::rgb::{Color, PerKeyEffect};
use async_trait::async_trait;
use std::ops::{Deref, DerefMut};

/// Apex 2023 new protocol report length (from OpenRGB reverse engineering). The packet ids live in
/// [`CommandCode`]; the original Apex Pro (PID 0x1610) uses OpenRGB's Gen 1 id `0x3A` with the same
/// 643-byte layout and key table, no init. Untested here (issue #283).
const APEX_2023_PACKET_LENGTH: usize = DirectInitCommand::REPORT_SIZE;

/// [EXPERIMENTAL] Report length of the Apex 9 TKL / Apex 9 Mini direct frame (OpenRGB's
/// `direct_packet_length_map`, via a community fork). Holds `(513 - 3) / 4 = 127` entries.
const APEX_9_PACKET_LENGTH: usize = 513;

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

/// [EXPERIMENTAL] OpenRGB's Gen 1 frame (`0x3A`, no init): original Apex Pro, and from a community fork's
/// table the Apex 5, Apex 7 TKL, Apex Pro TKL (original, pre-1.19.7 firmware) and Apex Pro (2024).
pub(crate) const GEN1_FRAME: DirectFrame = DirectFrame {
    packet_id: CommandCode::ApexGen1Direct,
    length: APEX_2023_PACKET_LENGTH,
    keys: TKL_KEYS,
    init: false,
};
/// [EXPERIMENTAL] Gen 3 frame (`0x40` after the `0x4B` init), wired Gen 3 model `0x1642`. No test record in
/// this repo; see the note on [`ApexProTkl2023::direct_packet_id`].
pub(crate) const GEN3_FRAME: DirectFrame = DirectFrame {
    packet_id: CommandCode::Apex2023Direct,
    length: APEX_2023_PACKET_LENGTH,
    keys: TKL_KEYS,
    init: true,
};
/// [EXPERIMENTAL] Wireless Apex Pro TKL frame (`0x61` after the `0x4B` init).
pub(crate) const WIRELESS_FRAME: DirectFrame = DirectFrame {
    packet_id: CommandCode::Apex2023DirectWireless,
    ..GEN3_FRAME
};
/// [EXPERIMENTAL] Apex 9 TKL / Apex 9 Mini frame on pre-1.19.7 firmware: `0x40` in 513 bytes, no init.
pub(crate) const APEX_9_FRAME: DirectFrame = DirectFrame {
    packet_id: CommandCode::Apex2023Direct,
    length: APEX_9_PACKET_LENGTH,
    keys: TKL_KEYS,
    init: false,
};
/// [EXPERIMENTAL] Apex 9 frame on 1.19.7+ firmware: the same frame after the `0x4B` init.
pub(crate) const APEX_9_GEN3_FRAME: DirectFrame = DirectFrame {
    init: true,
    ..APEX_9_FRAME
};
/// [EXPERIMENTAL] Wired Apex Pro TKL 2023 (`0x1628`): GG's key table and frame size, and no `0x4B` init,
/// which the GG capture never showed.
pub(crate) const WIRED_2023_FRAME: DirectFrame = DirectFrame {
    packet_id: CommandCode::Apex2023Direct,
    length: WIRED_2023_PACKET_LENGTH,
    keys: WIRED_2023_KEYS,
    init: false,
};

/// [EXPERIMENTAL] Slot order of the per-key tables in GG's 12,500-byte profile blob for the wired
/// Apex Pro TKL 2023 (PID 0x1628): index = slot, value = HID usage. Physical rows, left to right.
///
/// Source: GG USBPcap captures and GG's engine database (2026-10-14, firmware 1.19.7).
/// - Captured with a single-key edit: slots 1, 16, 24, 27, 29, 37, 41, 42, 44, 53, 55, 56, 57, 58, 60, 63-67.
/// - Matched as a set against GG's stored config: ISO/JIS keys (13, 40, 43, 54, 59, 61, 62; identity within
///   the set follows the JIS/ISO layout) and Tab/Caps (14, 28).
/// - Remaining slots follow the row order; slot 0 (grave) is the only key left for it.
/// - GG's decrypted device spec (`apex_pro_tkl_2022` `device-key-idx-to-hid`, first 70 entries minus its two
///   blanks) lists the same 68 usages in the same order, an independent check of every slot (2026-10-25).
///
/// The checksum at offset 12320 is solved (`apex_pro_tkl_2023_profile`), but writing the blob stays blocked:
/// the spec's read-back (`read_gen3_profile`) is untested on hardware.
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

/// [EXPERIMENTAL] One live analog-key setting for every analog key of the wired board (`0x1628`), sent as
/// an [`AnalogKeyCommand`]. Spec-derived (GG `apex_pro_tkl_2022`, 2026-10-24), untested on hardware; whether
/// a setting survives a power cycle is unknown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalogSetting {
    /// Actuation of one layer at a distance in tenths of a mm (1..=40), or `None` for all-zero thresholds
    /// (the second layer read all zero on the owner's board, which GG showed as no second actuation).
    Actuation { layer: HallLayer, tenths_mm: Option<u8> },
    /// Per-key thresholds of one layer as `(hid, h, l)`, exactly the 68 analog keys; see [`key_actuation_setting`].
    Thresholds { layer: HallLayer, keys: Vec<(u8, u8, u8)> },
    /// Release mode (0, 2, 3 or 4) of every analog key.
    ReleaseMode(u8),
    /// Rapid-trigger sensitivity of every analog key, in the spec's raw unit.
    RapidTriggerSensitivity(u8),
    /// Rapid tap on or off.
    RapidTap(bool),
    /// The full rapid-tap pair list; it replaces whatever pairs the board held.
    RapidTapPairs(Vec<RapidTapPair>),
}

/// The analog keys in GG's frame order: `device-analog-hid-codes`, the profile slots sorted by HID usage.
fn analog_keys() -> [u8; 68] {
    let mut keys = WIRED_2023_PROFILE_SLOTS;
    keys.sort_unstable();
    keys
}

/// [EXPERIMENTAL] The report an [`AnalogSetting`] puts on the wire for the wired board, report-ID byte first,
/// and whether it is a feature report.
///
/// # Errors
///
/// Returns an error for a distance outside 0.1-4.0 mm, a bad mode, or a rapid-tap key without an analog switch.
pub fn analog_report(setting: &AnalogSetting) -> Result<(Vec<u8>, bool)> {
    let keys = analog_keys();
    let frame: Option<HallFrame>;
    let pairs: Vec<(u8, u8)>;
    let command = match setting {
        AnalogSetting::Actuation { .. } | AnalogSetting::Thresholds { .. } => {
            frame = hall_frame(setting)?;
            let (layer, triples) = frame.as_ref().ok_or_else(|| {
                crate::Error::InvalidConfig("an actuation setting without a threshold frame".to_string())
            })?;
            AnalogKeyCommand::HallThresholds {
                layer: *layer,
                keys: triples,
            }
        }
        AnalogSetting::ReleaseMode(value) | AnalogSetting::RapidTriggerSensitivity(value) => {
            pairs = keys.iter().map(|&hid| (hid, *value)).collect();
            if matches!(setting, AnalogSetting::ReleaseMode(_)) {
                AnalogKeyCommand::ReleaseMode(&pairs)
            } else {
                AnalogKeyCommand::RapidTriggerSensitivity(&pairs)
            }
        }
        AnalogSetting::RapidTap(on) => AnalogKeyCommand::RapidTapEnable(*on),
        AnalogSetting::RapidTapPairs(list) => {
            if let Some(hid) = list
                .iter()
                .flat_map(|p| [p.hid_1, p.hid_2])
                .find(|hid| profile_slot(*hid).is_none())
            {
                return Err(crate::Error::InvalidConfig(format!(
                    "HID usage {hid:#04x} has no analog switch on this board"
                )));
            }
            AnalogKeyCommand::RapidTapPairs(list)
        }
    };
    let mut buf = vec![0u8; command.report_len()];
    let size = HidReportBuilder::new(HidDeviceType::Keyboard).build_report(command, &mut buf)?;
    buf.truncate(size);
    Ok((buf, command.is_feature()))
}

/// One layer's `(hid, h, l)` threshold entries.
type HallFrame = (HallLayer, Vec<(u8, u8, u8)>);

/// The full `(hid, h, l)` frame an actuation setting sends, in GG's key order; `None` for other settings.
fn hall_frame(setting: &AnalogSetting) -> Result<Option<HallFrame>> {
    match setting {
        AnalogSetting::Actuation { layer, tenths_mm } => {
            let (h, l) = tenths_mm.map_or(Ok((0, 0)), actuation_raw)?;
            Ok(Some((*layer, analog_keys().iter().map(|&hid| (hid, h, l)).collect())))
        }
        AnalogSetting::Thresholds { layer, keys } => {
            let mut hids: Vec<u8> = keys.iter().map(|k| k.0).collect();
            hids.sort_unstable();
            if hids != analog_keys() {
                return Err(crate::Error::InvalidConfig(
                    "a threshold frame must list each of the 68 analog keys exactly once".to_string(),
                ));
            }
            let mut sorted = keys.clone();
            sorted.sort_unstable_by_key(|k| k.0);
            Ok(Some((*layer, sorted)))
        }
        _ => Ok(None),
    }
}

/// File holding the last threshold frame ssgg sent for one layer, as `[hid][h][l]` entries: the base a
/// per-key change is merged onto, since a frame must carry all 68 keys.
/// shortcut: only knows what ssgg sent; GG or on-board changes since then are invisible, upgrade to the
/// `read_gen3_profile` tables once that read is confirmed on hardware.
fn actuation_frame_path(layer: HallLayer) -> Option<std::path::PathBuf> {
    crate::config::Config::config_dir().map(|dir| {
        dir.join(format!(
            "actuation-{:04x}-layer{:02x}.bin",
            product_ids::APEX_PRO_TKL_2023,
            layer as u8
        ))
    })
}

/// Remember a threshold frame that reached the board. Best effort, like `save_last_frame`: a failed save
/// only costs the next per-key change its merge base, so it is logged and not propagated.
fn save_actuation_frame(layer: HallLayer, frame: &[(u8, u8, u8)]) {
    let Some(path) = actuation_frame_path(layer) else {
        return;
    };
    let bytes: Vec<u8> = frame.iter().flat_map(|&(hid, h, l)| [hid, h, l]).collect();
    let saved = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .map_err(crate::Error::from)
        .and_then(|()| crate::fs_utils::secure_write(&path, &bytes));
    if let Err(e) = saved {
        tracing::warn!("Could not save the actuation frame to {}: {e}", path.display());
    }
}

/// Apply per-key changes `(hid, tenths of a mm)` to a saved frame, keeping every other key as it was.
fn merge_actuation(base: &[(u8, u8, u8)], layer: HallLayer, changes: &[(u8, u8)]) -> Result<AnalogSetting> {
    let mut keys = base.to_vec();
    for &(hid, tenths_mm) in changes {
        let (h, l) = actuation_raw(tenths_mm)?;
        let entry = keys.iter_mut().find(|k| k.0 == hid).ok_or_else(|| {
            crate::Error::InvalidConfig(format!("HID usage {hid:#04x} has no analog switch on this board"))
        })?;
        *entry = (hid, h, l);
    }
    let setting = AnalogSetting::Thresholds { layer, keys };
    hall_frame(&setting)?;
    Ok(setting)
}

/// [EXPERIMENTAL] A per-key actuation change for the wired board, merged onto the last threshold frame ssgg
/// sent for `layer`. The board has no confirmed read-back, so with no saved frame this refuses rather than
/// guess the other 67 keys: send `ssgg actuation set <mm>` (all keys) first.
///
/// # Errors
///
/// Returns an error when no frame was saved, a key has no analog switch, or a distance is outside 0.1-4.0 mm.
pub fn key_actuation_setting(layer: HallLayer, changes: &[(u8, u8)]) -> Result<AnalogSetting> {
    let base = actuation_frame_path(layer)
        .and_then(|path| std::fs::read(path).ok())
        .map(|bytes| {
            bytes
                .as_chunks::<3>()
                .0
                .iter()
                .map(|e| (e[0], e[1], e[2]))
                .collect::<Vec<_>>()
        })
        .filter(|frame| frame.len() == analog_keys().len())
        .ok_or_else(|| {
            crate::Error::InvalidConfig(format!(
                "no saved {layer:?}-layer actuation frame: set every key once with `ssgg actuation set <mm>{}` \
                 so ssgg knows the other keys' values",
                if layer == HallLayer::Second { " --second" } else { "" }
            ))
        })?;
    merge_actuation(&base, layer, changes)
}

/// [EXPERIMENTAL] What a Gen3 on-board profile read collected: the firmware poll answer, the 3-byte header of
/// every chunk reply (meaning unknown, recorded for the hardware check), and the reassembled profile blob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gen3ProfileRead {
    pub firmware: Option<String>,
    pub headers: Vec<Vec<u8>>,
    pub data: Vec<u8>,
}

impl Gen3ProfileRead {
    /// Whether the blob's stored checksum at offset 12320 matches its contents.
    pub fn checksum_ok(&self) -> bool {
        checksum_matches(&self.data)
    }
}

/// [EXPERIMENTAL] The chunk requests of a slot read, in order: `PROFILE_LEN / 500` = 25 chunks.
///
/// # Errors
///
/// Returns an error for a slot outside 0-4.
pub fn gen3_read_requests(slot: u8) -> Result<Vec<Gen3ProfileReadCommand>> {
    if slot >= Gen3ProfileReadCommand::SLOTS {
        return Err(crate::Error::InvalidConfig(format!(
            "profile slot {slot} out of range 0-{}",
            Gen3ProfileReadCommand::SLOTS - 1
        )));
    }
    let chunk = usize::from(Gen3ProfileReadCommand::CHUNK_LEN);
    Ok((0..PROFILE_LEN.div_ceil(chunk))
        .map(|i| Gen3ProfileReadCommand {
            fs_id: Gen3ProfileReadCommand::FIRST_PROFILE_FS + slot,
            size: Gen3ProfileReadCommand::CHUNK_LEN,
            offset: (i * chunk) as u32,
        })
        .collect())
}

/// Split chunk replies (report-ID byte first, as hidapi returns them) into their headers and the blob.
fn assemble_gen3_replies(replies: &[Vec<u8>]) -> Result<(Vec<Vec<u8>>, Vec<u8>)> {
    let head = Gen3ProfileReadCommand::REPLY_HEADER_LEN;
    let chunk = usize::from(Gen3ProfileReadCommand::CHUNK_LEN);
    let mut headers = Vec::with_capacity(replies.len());
    let mut data = Vec::with_capacity(PROFILE_LEN);
    for (i, reply) in replies.iter().enumerate() {
        let body = reply.get(head..head + chunk).ok_or_else(|| {
            crate::Error::DeviceCommunication(format!(
                "profile read chunk {i}: reply of {} bytes, needs {}",
                reply.len(),
                head + chunk
            ))
        })?;
        headers.push(reply[..head].to_vec());
        data.extend_from_slice(body);
    }
    data.truncate(PROFILE_LEN);
    Ok((headers, data))
}

/// Whether a firmware version (`1.19.7`, `V1.19.7`, ...) is 1.19.7 or newer, OpenRGB's Gen 3 cut-off.
fn is_gen3_firmware(version: &str) -> bool {
    let digits = version.trim_start_matches(|c: char| !c.is_ascii_digit());
    let mut parts = digits.split('.').map(|p| {
        p.chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse::<u32>()
            .ok()
    });
    match (parts.next().flatten(), parts.next().flatten(), parts.next().flatten()) {
        (Some(major), Some(minor), Some(patch)) => (major, minor, patch) >= (1, 19, 7),
        _ => false,
    }
}

/// Firmware version in a Gen3 `90` answer: `[report ID] 90 <err> <ASCII>`; hidapi may or may not keep the
/// report-ID byte, so the answer is located by its command byte. GG treats a version starting with `@`
/// (0x40) as an error answer.
fn parse_gen3_firmware(reply: &[u8]) -> Option<String> {
    let at = reply
        .iter()
        .position(|&b| b == CommandCode::OnboardFirmwareVersion as u8)?;
    super::apex_7_profile::parse_firmware_version(reply.get(at + 2..)?).filter(|v| !v.starts_with('@'))
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
    /// Firmware probe result for the models whose protocol depends on it (`firmware_switches_protocol`):
    /// `None` until probed, then whether the firmware is 1.19.7 or newer.
    gen3_firmware: Option<bool>,
    /// Software brightness (0-100) applied to every direct frame; see `set_direct_brightness`.
    brightness: u8,
}

impl ApexProTkl2023 {
    /// Product ID for Apex Pro TKL 2023.
    pub const PRODUCT_ID: u16 = 0x1628;

    /// Create from a generic keyboard.
    pub fn new(keyboard: GenericKeyboard) -> Self {
        Self {
            inner: keyboard,
            initialized_new_protocol: false,
            gen3_firmware: None,
            brightness: 100,
        }
    }

    /// Create for wireless model without a hidapi device handle.
    /// Uses raw hidraw feature reports only.
    pub fn new_wireless_raw(info: DeviceInfo) -> Self {
        Self {
            inner: GenericKeyboard::new_without_device(info),
            initialized_new_protocol: false,
            gen3_firmware: None,
            brightness: 100,
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

    /// The direct frame of this model's [`KeyboardModel`](super::models::KeyboardModel) row, picked by the
    /// firmware probe on the models that have two; `None` when the model has no direct frame.
    fn model_frame(&self) -> Option<DirectFrame> {
        let lighting = keyboard_model(self.inner.info().product_id)?.direct?;
        Some(lighting.frame(self.gen3_firmware == Some(true)))
    }

    /// The frame to build. Models without a direct frame never send one (every sender checks
    /// `uses_new_protocol`); the Gen 3 fallback only keeps the helpers total.
    fn frame(&self) -> DirectFrame {
        self.model_frame().unwrap_or(GEN3_FRAME)
    }

    /// Whether the 0x4B init report has to precede the first direct frame.
    fn needs_init(&self) -> bool {
        self.frame().init
    }

    /// Probe the firmware once on the models it matters for; a failed or unparsable poll keeps the
    /// pre-probe protocol, as OpenRGB does.
    fn probe_firmware(&mut self) {
        let probes = keyboard_model(self.inner.info().product_id)
            .and_then(|m| m.direct)
            .is_some_and(|d| d.probes_firmware());
        if self.gen3_firmware.is_some() || !probes {
            return;
        }
        let version = self.gen3_firmware_version();
        let gen3 = version.as_deref().is_some_and(is_gen3_firmware);
        tracing::info!("firmware {version:?}: Gen 3 protocol {gen3}");
        self.gen3_firmware = Some(gen3);
    }

    fn direct_keys(&self) -> &'static [u8] {
        self.frame().keys
    }

    fn direct_packet_length(&self) -> usize {
        self.frame().length
    }

    /// Direct-mode packet ID, or `None` if the model has no direct frame.
    ///
    /// Verified on Apex Pro TKL Wireless Gen 3 hardware: the dongle (`0x1644`) and
    /// the same keyboard on USB cable (`0x1646`) both answer to the wireless packet
    /// ID `0x61`, while the wired-only Gen 3 model (`0x1642`) uses `0x40`.
    fn direct_packet_id(&self) -> Option<CommandCode> {
        self.model_frame().map(|f| f.packet_id)
    }

    /// Send the 0x4B initialization feature report required by new protocol.
    fn ensure_new_protocol_init(&mut self) -> Result<()> {
        if self.initialized_new_protocol || !self.needs_init() {
            return Ok(());
        }
        let mut buf = vec![0u8; DirectInitCommand::REPORT_SIZE];
        let size = HidReportBuilder::new(HidDeviceType::Keyboard).build_report(DirectInitCommand, &mut buf)?;
        self.inner.send_feature(&buf[..size], size)?;
        // The controller needs a moment before it will honour direct-mode packets.
        std::thread::sleep(std::time::Duration::from_millis(50));
        self.initialized_new_protocol = true;
        tracing::info!("Sent Apex 2023 new protocol init (0x4B)");
        Ok(())
    }

    /// Send one direct-mode feature report covering every key in `direct_keys()`.
    fn send_direct_frame(&mut self, color_for: impl Fn(u8) -> Color) -> Result<()> {
        self.probe_firmware();
        self.ensure_new_protocol_init()?;
        let entries = self.direct_entries(color_for);
        let buf = self.frame_for(&entries)?;
        self.inner.send_feature(&buf, buf.len())?;
        self.save_last_frame(&entries);
        Ok(())
    }

    /// Remember the requested (unscaled) entries of a frame that reached the board, so repeated
    /// brightness changes do not compound. Best effort: a failed save only costs the next per-key
    /// request its merge base, so it is logged and not propagated.
    fn save_last_frame(&self, entries: &[(u8, Color)]) {
        let Some(path) = last_frame_path(self.inner.info().product_id) else {
            return;
        };
        let bytes: Vec<u8> = entries.iter().flat_map(|(u, c)| [*u, c.r, c.g, c.b]).collect();
        let saved = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .map_err(crate::Error::from)
            .and_then(|()| crate::fs_utils::secure_write(&path, &bytes));
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

    /// One entry per key in `direct_keys()`, in table order.
    fn direct_entries(&self, color_for: impl Fn(u8) -> Color) -> Vec<(u8, Color)> {
        self.direct_keys()
            .iter()
            .map(|&usage| (usage, color_for(usage)))
            .collect()
    }

    /// Frame for `entries` with the software brightness applied.
    fn frame_for(&self, entries: &[(u8, Color)]) -> Result<Vec<u8>> {
        let factor = f32::from(self.brightness) / 100.0;
        let shown: Vec<(u8, Color)> = entries.iter().map(|(u, c)| (*u, c.scale(factor))).collect();
        let command = DirectFrameCommand {
            packet: self.frame().packet_id,
            report_len: self.direct_packet_length(),
            entries: &shown,
        };
        let mut buf = vec![0u8; command.report_len];
        let size = HidReportBuilder::new(HidDeviceType::Keyboard).build_report(command, &mut buf)?;
        buf.truncate(size);
        Ok(buf)
    }

    /// [EXPERIMENTAL] No brightness command is known for the direct models (GG scales colours in
    /// software), so remember the level and resend the last frame at it; later frames are scaled too.
    /// shortcut: the level lives in this process only, so a later `ssgg` run sends full-brightness
    /// colours until it sets brightness again; upgrade by persisting it next to the last frame once
    /// the daemon stops pre-scaling through `RgbController`.
    fn set_direct_brightness(&mut self, level: u8) -> Result<()> {
        self.brightness = level.min(100);
        let previous = self.load_last_frame();
        if previous.is_empty() {
            tracing::warn!("No saved frame to dim; the level applies to the next frame");
            return Ok(());
        }
        self.send_direct_frame(|usage| per_key_color(&[], &previous, usage))
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

    /// On the wired board this sends the spec's live hall thresholds for every analog key (0.1-4.0 mm);
    /// elsewhere the unverified `0x2D` guess, a confirmed no-op on `0x1628`.
    pub fn set_actuation_point(&mut self, value: u8) -> Result<()> {
        if self.is_wired_2023() {
            self.send_analog_setting(&AnalogSetting::Actuation {
                layer: HallLayer::Primary,
                tenths_mm: Some(value),
            })?;
            self.inner.update_cached_actuation_point(value);
            return Ok(());
        }
        let command = ActuationCommand::new(value);
        command.validate()?;
        self.send_actuation_command(command)
    }

    fn require_wired_2023(&self, what: &str) -> Result<()> {
        if self.is_wired_2023() {
            return Ok(());
        }
        Err(crate::Error::DeviceCommunication(format!(
            "{what} is only defined for the wired Apex Pro TKL 2023 (0x1628), not {:#06x}",
            self.inner.info().product_id
        )))
    }

    /// Send a report (report-ID byte first) as a feature report or as an output report, bytes as given.
    /// Not `send_raw`: that path prepends a second report-ID byte and drops repeated reports.
    fn send_report(&mut self, report: &[u8], feature: bool) -> Result<()> {
        if feature {
            self.inner.send_feature(report, report.len())
        } else {
            self.inner.write_profile_packet(report)
        }
    }

    /// [EXPERIMENTAL] Send one live analog-key setting, see [`AnalogSetting`]. Wired board only. Returns the
    /// board's `38 ...` answer if one arrives (rapid tap was captured answering `38 00 66`); what other
    /// answers mean is unknown, so it is only reported.
    pub fn send_analog_setting(&mut self, setting: &AnalogSetting) -> Result<Option<Vec<u8>>> {
        self.require_wired_2023("a live analog-key setting")?;
        let (report, feature) = analog_report(setting)?;
        self.send_report(&report, feature)?;
        if let Some((layer, frame)) = hall_frame(setting)? {
            save_actuation_frame(layer, &frame);
        }
        Ok(self.analog_reply())
    }

    /// First `0x38` input report within about 300 ms. A failed read only loses the diagnostic: the setting
    /// was already sent.
    fn analog_reply(&self) -> Option<Vec<u8>> {
        for _ in 0..3 {
            match self.inner.read_profile_input_packet(KEYBOARD_REPORT_SIZE, 100) {
                Ok(Some(reply)) if reply.first() == Some(&(CommandCode::AnalogKeys as u8)) => return Some(reply),
                Ok(Some(_)) => {}
                Ok(None) => return None,
                Err(e) => {
                    tracing::debug!("No analog-key answer read: {e}");
                    return None;
                }
            }
        }
        None
    }

    /// [EXPERIMENTAL] Send one display command (leave direct mode, OLED write or reset). Wired board only.
    pub fn send_display_command(&mut self, command: Apex2022DisplayCommand<'_>) -> Result<()> {
        self.require_wired_2023("this display command")?;
        let mut buf = vec![0u8; PROFILE_CHUNK_REPORT_SIZE];
        let size = HidReportBuilder::new(HidDeviceType::Keyboard).build_report(command, &mut buf)?;
        self.send_report(&buf[..size], matches!(command, Apex2022DisplayCommand::Oled(_)))
    }

    /// [EXPERIMENTAL] Read one on-board profile slot (0-4) with the spec's `83 01` chunk read. Read only:
    /// nothing here writes to the board. Untested on hardware.
    pub fn read_gen3_profile(&mut self, slot: u8) -> Result<Gen3ProfileRead> {
        self.require_wired_2023("the Gen3 on-board profile read")?;
        let requests = gen3_read_requests(slot)?;
        let gap = std::time::Duration::from_millis(Gen3ProfileReadCommand::COMMAND_GAP_MS);
        let firmware = self.gen3_firmware_version();
        let builder = HidReportBuilder::new(HidDeviceType::Keyboard);
        let mut replies = Vec::with_capacity(requests.len());
        for request in requests {
            std::thread::sleep(gap);
            let mut buf = vec![0u8; PROFILE_CHUNK_REPORT_SIZE];
            let size = builder.build_report(request, &mut buf)?;
            self.inner.send_feature(&buf[..size], size)?;
            std::thread::sleep(gap);
            replies.push(
                self.inner
                    .read_profile_feature_packet(0x00, PROFILE_CHUNK_REPORT_SIZE)?,
            );
        }
        let (headers, data) = assemble_gen3_replies(&replies)?;
        Ok(Gen3ProfileRead {
            firmware,
            headers,
            data,
        })
    }

    /// The `90 00` firmware poll (GG `fw_version`); informational, so a failure is logged and gives `None`.
    fn gen3_firmware_version(&self) -> Option<String> {
        let mut buf = [0u8; KEYBOARD_REPORT_SIZE];
        let poll = HidReportBuilder::new(HidDeviceType::Keyboard)
            .build_report(OnboardFirmwareVersionCommand::KEYBOARD, &mut buf)
            .and_then(|size| self.inner.write_profile_packet(&buf[..size]))
            .and_then(|()| self.inner.read_profile_input_packet(KEYBOARD_REPORT_SIZE, 200));
        match poll {
            Ok(reply) => reply.as_deref().and_then(parse_gen3_firmware),
            Err(e) => {
                tracing::warn!("firmware poll failed: {e}");
                None
            }
        }
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
        // Direct-protocol models get no APPLY (0x09): it reverts direct lighting, GG's 0x1628 capture never sends
        // it, and before the report-ID fix it reached the board shifted (as `00 09`), so the confirmed 0x1628
        // lighting results ran without it. The 0x4B init is sent lazily in ensure_new_protocol_init().
        if self.uses_new_protocol() {
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
        if self.uses_new_protocol() {
            return self.set_direct_brightness(brightness);
        }
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

    fn send_analog_setting(&mut self, setting: &AnalogSetting) -> Result<Option<Vec<u8>>> {
        self.send_analog_setting(setting)
    }

    fn send_display_command(&mut self, command: Apex2022DisplayCommand<'_>) -> Result<()> {
        self.send_display_command(command)
    }

    fn read_gen3_profile(&mut self, slot: u8) -> Result<Gen3ProfileRead> {
        self.read_gen3_profile(slot)
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
            Some(CommandCode::Apex2023DirectWireless)
        );
        assert_eq!(
            keyboard_for(product_ids::APEX_PRO_TKL_WIRELESS_2024).direct_packet_id(),
            Some(CommandCode::Apex2023DirectWireless)
        );
        assert_eq!(
            keyboard_for(product_ids::APEX_PRO_TKL_2024).direct_packet_id(),
            Some(CommandCode::Apex2023Direct)
        );
    }

    #[test]
    fn wired_2023_uses_the_gg_captured_layout() {
        // [EXPERIMENTAL] layout read from a GG USBPcap capture, not yet confirmed on hardware.
        let keyboard = keyboard_for(product_ids::APEX_PRO_TKL_2023);
        assert!(keyboard.uses_new_protocol());
        assert_eq!(keyboard.direct_packet_id(), Some(CommandCode::Apex2023Direct));
        assert_eq!(keyboard.direct_keys().len(), 84);
        assert_eq!(keyboard.direct_packet_length(), 645);
        assert!(3 + keyboard.direct_keys().len() * 4 <= WIRED_2023_PACKET_LENGTH);
    }

    #[test]
    fn wired_2023_per_key_frame_lights_only_requested_keys() {
        let keyboard = keyboard_for(product_ids::APEX_PRO_TKL_2023);
        let requested = [(0x1A, Color::RED), (0x29, Color::BLUE)]; // W, Esc
        let frame = keyboard
            .frame_for(&keyboard.direct_entries(|usage| per_key_color(&requested, &[], usage)))
            .unwrap();

        assert_eq!(frame.len(), WIRED_2023_PACKET_LENGTH);
        assert_eq!(&frame[..3], &[0x00, 0x40, 0x54]);
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
    fn analog_frames_cover_the_68_keys_in_hid_order() {
        let (report, feature) = analog_report(&AnalogSetting::Actuation {
            layer: HallLayer::Primary,
            tenths_mm: Some(20),
        })
        .unwrap();
        assert!(feature);
        assert_eq!(report.len(), 645);
        assert_eq!(&report[..8], &[0x00, 0x38, 0x61, 68, 0x00, 0x04, 0x39, 0x34]);
        let hids: Vec<u8> = report[5..5 + 68 * 3].iter().step_by(3).copied().collect();
        assert!(
            hids.windows(2).all(|w| w[0] < w[1]),
            "sorted, as GG's device-analog-hid-codes"
        );
        assert_eq!(hids.last(), Some(&0xF0));
        assert!(report[5 + 68 * 3..].iter().all(|&b| b == 0));

        let (cleared, _) = analog_report(&AnalogSetting::Actuation {
            layer: HallLayer::Second,
            tenths_mm: None,
        })
        .unwrap();
        assert_eq!(&cleared[..8], &[0x00, 0x38, 0x61, 68, 0x02, 0x04, 0, 0]);

        let (rt, _) = analog_report(&AnalogSetting::RapidTriggerSensitivity(2)).unwrap();
        assert_eq!(&rt[..6], &[0x00, 0x38, 0x65, 68, 0x04, 2]);
        assert_eq!(rt[4 + 68 * 2 - 1], 2);
    }

    #[test]
    fn gen3_firmware_cut_off_is_1_19_7() {
        for (version, gen3) in [
            ("1.19.7", true),
            ("1.19.6", false),
            ("1.20.0", true),
            ("2.0.0", true),
            ("1.9.9", false),
            ("V1.19.7", true),
            ("1.19", false),
            ("", false),
        ] {
            assert_eq!(is_gen3_firmware(version), gen3, "{version:?}");
        }
    }

    #[test]
    fn firmware_picks_the_original_pro_tkl_and_apex_9_protocol() {
        let mut tkl = keyboard_for(product_ids::APEX_PRO_TKL);
        assert_eq!(
            tkl.direct_packet_id(),
            Some(CommandCode::ApexGen1Direct),
            "before or without a probe"
        );
        assert!(!tkl.needs_init());
        tkl.gen3_firmware = Some(true);
        assert_eq!(tkl.direct_packet_id(), Some(CommandCode::Apex2023Direct));
        assert!(tkl.needs_init());
        assert_eq!(tkl.direct_packet_length(), APEX_2023_PACKET_LENGTH);

        let mut apex9 = keyboard_for(product_ids::APEX_9_TKL);
        assert!(!apex9.needs_init());
        apex9.gen3_firmware = Some(true);
        assert!(apex9.needs_init());
        assert_eq!(apex9.direct_packet_length(), APEX_9_PACKET_LENGTH);

        let mut wired = keyboard_for(product_ids::APEX_PRO_TKL_2023);
        wired.probe_firmware();
        assert_eq!(wired.gen3_firmware, None, "0x1628 never probes");
    }

    #[test]
    fn per_key_actuation_changes_only_the_named_keys() {
        let base: Vec<(u8, u8, u8)> = analog_keys().iter().map(|&hid| (hid, 0x2F, 0x2A)).collect();
        let merged = merge_actuation(&base, HallLayer::Primary, &[(0x1A, 20), (0x04, 30)]).unwrap();
        let (report, _) = analog_report(&merged).unwrap();
        let entries: Vec<&[u8]> = report[5..5 + 68 * 3].chunks(3).collect();
        for entry in &entries {
            let expected: &[u8] = match entry[0] {
                0x1A => &[0x1A, 0x39, 0x34],
                0x04 => &[0x04, 0x86, 0x7B],
                hid => &[hid, 0x2F, 0x2A],
            };
            assert_eq!(*entry, expected);
        }
        assert!(
            merge_actuation(&base, HallLayer::Primary, &[(0x29, 20)]).is_err(),
            "Esc is not analog"
        );
        assert!(
            merge_actuation(&base, HallLayer::Primary, &[(0x1A, 41)]).is_err(),
            "4.1 mm is past the travel"
        );
        assert!(
            merge_actuation(&base[..67], HallLayer::Primary, &[(0x1A, 20)]).is_err(),
            "base must be complete"
        );
    }

    #[test]
    fn analog_settings_refuse_unknown_points_and_non_analog_keys() {
        let at = |tenths| AnalogSetting::Actuation {
            layer: HallLayer::Primary,
            tenths_mm: Some(tenths),
        };
        assert!(analog_report(&at(0)).is_err());
        assert!(analog_report(&at(41)).is_err());
        assert!(analog_report(&AnalogSetting::ReleaseMode(1)).is_err());
        let pair = |hid_2| RapidTapPair {
            hid_1: 0x04,
            hid_2,
            mode: 0,
            report_both: false,
        };
        assert!(
            analog_report(&AnalogSetting::RapidTapPairs(vec![pair(0x29)])).is_err(),
            "Esc is not analog"
        );
        assert!(analog_report(&AnalogSetting::RapidTapPairs(vec![pair(0x07)])).is_ok());
    }

    #[test]
    fn non_wired_models_refuse_spec_commands() {
        let mut keyboard = keyboard_for(product_ids::APEX_PRO_TKL_2024);
        assert!(keyboard.send_analog_setting(&AnalogSetting::RapidTap(true)).is_err());
        assert!(keyboard.read_gen3_profile(0).is_err());
        assert!(
            keyboard
                .send_display_command(Apex2022DisplayCommand::ClearDirect)
                .is_err()
        );
    }

    #[test]
    fn gen3_read_requests_walk_the_blob_in_500_byte_chunks() {
        let requests = gen3_read_requests(4).unwrap();
        assert_eq!(requests.len(), 25);
        assert!(requests.iter().all(|r| r.fs_id == 0x84 && r.size == 500));
        assert_eq!(requests[24].offset, 12_000);
        assert!(gen3_read_requests(5).is_err());
    }

    #[test]
    fn gen3_replies_reassemble_behind_the_header() {
        let replies: Vec<Vec<u8>> = (0..25u8)
            .map(|i| {
                let mut reply = vec![0x00, 0x83, 0x01];
                reply.extend(std::iter::repeat_n(i, 500));
                reply.extend([0xEE; 142]);
                reply
            })
            .collect();
        let (headers, data) = assemble_gen3_replies(&replies).unwrap();
        assert_eq!(data.len(), PROFILE_LEN);
        assert_eq!((data[0], data[499], data[500], data[PROFILE_LEN - 1]), (0, 0, 1, 24));
        assert!(headers.iter().all(|h| h == &[0x00, 0x83, 0x01]));

        let short = vec![vec![0u8; 400]];
        assert!(assemble_gen3_replies(&short).is_err());
    }

    #[test]
    fn gen3_firmware_answer_is_found_with_or_without_the_report_id() {
        let mut reply = vec![0x00, 0x90, 0x00];
        reply.extend(b"1.19.7");
        reply.resize(65, 0);
        assert_eq!(parse_gen3_firmware(&reply).as_deref(), Some("1.19.7"));
        assert_eq!(parse_gen3_firmware(&reply[1..]).as_deref(), Some("1.19.7"));
        assert_eq!(
            parse_gen3_firmware(&[0x90, 0x00, b'@', b'1']),
            None,
            "GG's error answer"
        );
        assert_eq!(parse_gen3_firmware(&[0x00; 8]), None);
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
        assert_eq!(keyboard.direct_packet_id(), Some(CommandCode::ApexGen1Direct));
        let frame = keyboard.frame_for(&keyboard.direct_entries(|_| Color::RED)).unwrap();
        assert_eq!(frame.len(), APEX_2023_PACKET_LENGTH);
        assert_eq!(&frame[..7], &[0x00, 0x3A, 112, 0x04, 0xFF, 0x00, 0x00]);
    }

    #[test]
    fn brightness_scales_the_frame_without_compounding() {
        let mut keyboard = keyboard_for(product_ids::APEX_PRO_TKL_2023);
        let entries = keyboard.direct_entries(|_| Color::new(200, 100, 50));

        keyboard.brightness = 50;
        let first = keyboard.frame_for(&entries).unwrap();
        let second = keyboard.frame_for(&entries).unwrap();

        assert_eq!(&first[3..7], &[0x04, 100, 50, 25]);
        assert_eq!(first, second, "scaling must not feed back into the entries");
        assert_eq!(entries[0], (0x04, Color::new(200, 100, 50)));

        keyboard.brightness = 100;
        assert_eq!(&keyboard.frame_for(&entries).unwrap()[3..7], &[0x04, 200, 100, 50]);

        keyboard.brightness = 0;
        assert_eq!(&keyboard.frame_for(&entries).unwrap()[3..7], &[0x04, 0, 0, 0]);
    }

    #[test]
    fn gen3_keeps_the_112_key_table() {
        let keyboard = keyboard_for(product_ids::APEX_PRO_TKL_2024);
        assert_eq!(keyboard.direct_keys().len(), 112);
        assert_eq!(keyboard.direct_packet_length(), APEX_2023_PACKET_LENGTH);
    }

    #[test]
    fn apex_9_pair_uses_the_0x40_frame_in_513_bytes_without_init() {
        for pid in [product_ids::APEX_9_TKL, product_ids::APEX_9_MINI] {
            let keyboard = keyboard_for(pid);
            assert_eq!(
                keyboard.direct_packet_id(),
                Some(CommandCode::Apex2023Direct),
                "{pid:#06x}"
            );
            assert_eq!(keyboard.direct_packet_length(), 513, "{pid:#06x}");
            assert!(!keyboard.needs_init(), "{pid:#06x}");
            assert_eq!(keyboard.direct_keys().len(), 112);

            let frame = keyboard.frame_for(&keyboard.direct_entries(|_| Color::RED)).unwrap();
            assert_eq!(frame.len(), 513);
            assert_eq!(&frame[..7], &[0x00, 0x40, 0x70, 0x04, 0xFF, 0x00, 0x00]);
            assert!(frame[3 + 112 * 4..].iter().all(|&b| b == 0));
        }
        assert!((513 - 3) / 4 >= TKL_KEYS.len());
    }

    #[test]
    fn gen1_fork_models_use_the_0x3a_frame_without_init() {
        for pid in [
            product_ids::APEX_5,
            product_ids::APEX_7_TKL,
            product_ids::APEX_PRO_TKL,
            product_ids::APEX_PRO_2024,
        ] {
            let keyboard = keyboard_for(pid);
            assert_eq!(
                keyboard.direct_packet_id(),
                Some(CommandCode::ApexGen1Direct),
                "{pid:#06x}"
            );
            assert_eq!(keyboard.direct_packet_length(), APEX_2023_PACKET_LENGTH, "{pid:#06x}");
            assert!(!keyboard.needs_init(), "{pid:#06x}");
            assert_eq!(keyboard.direct_keys().len(), 112);
        }
    }

    #[test]
    fn init_is_needed_only_where_it_was_before() {
        assert!(!keyboard_for(product_ids::APEX_PRO_TKL_2023).needs_init());
        assert!(!keyboard_for(product_ids::APEX_PRO).needs_init());
        for pid in [
            product_ids::APEX_PRO_TKL_2024,
            product_ids::APEX_PRO_TKL_WIRELESS_2024_DONGLE,
            product_ids::APEX_PRO_TKL_WIRELESS_2024,
        ] {
            assert!(keyboard_for(pid).needs_init(), "{pid:#06x}");
        }
    }

    #[test]
    fn apex_7_keeps_its_own_class_and_no_direct_frame() {
        assert_eq!(keyboard_for(product_ids::APEX_7).direct_packet_id(), None);
    }

    /// Pins, for every keyboard PID, the class it is built as, its registry name and type, and the
    /// direct frame it sends before a firmware probe and after one that reports Gen 3 firmware:
    /// `(packet id, report length, key count, 0x4B init)`, or `None` without a direct frame.
    #[test]
    fn every_keyboard_pid_keeps_its_class_name_and_direct_frame() {
        use crate::devices::discovery::{KeyboardClass as K, keyboard_class};
        use crate::devices::{device_name_from_product_id, device_type_from_product_id};
        use CommandCode::{Apex2023Direct as D40, Apex2023DirectWireless as D61, ApexGen1Direct as D3A};

        type Frame = Option<(CommandCode, usize, usize, bool)>;
        const GEN1: Frame = Some((D3A, 643, 112, false));
        const GEN3: Frame = Some((D40, 643, 112, true));
        const WIRELESS: Frame = Some((D61, 643, 112, true));
        const APEX_9: Frame = Some((D40, 513, 112, false));
        const APEX_9_GEN3: Frame = Some((D40, 513, 112, true));
        const WIRED_2023: Frame = Some((D40, 645, 84, false));
        let rows: &[(u16, K, &str, Frame, Frame)] = &[
            (0x1610, K::ApexProTkl2023, "Apex Pro", GEN1, GEN1),
            (0x1612, K::Apex7, "Apex 7", None, None),
            (0x1614, K::ApexProTkl2023, "Apex Pro TKL", GEN1, GEN3),
            (0x1616, K::Generic, "Apex 150", None, None),
            (0x1618, K::ApexProTkl2023, "Apex 7 TKL", GEN1, GEN1),
            (0x161A, K::Apex3, "Apex 3", None, None),
            (0x161C, K::ApexProTkl2023, "Apex 5", GEN1, GEN1),
            (0x161E, K::Generic, "Apex Pro Mini", None, None),
            (0x1620, K::ApexProTkl2023, "Apex 9 Mini", APEX_9, APEX_9_GEN3),
            (0x1622, K::Apex3Tkl, "Apex 3 TKL", None, None),
            (0x1624, K::Generic, "Apex Pro Mini Wireless (Dongle)", None, None),
            (0x1626, K::Generic, "Apex Pro Mini Wireless", None, None),
            (0x1628, K::ApexProTkl2023, "Apex Pro TKL (2023)", WIRED_2023, WIRED_2023),
            (
                0x1630,
                K::ApexProTkl2023,
                "Apex Pro TKL (2023) Wireless",
                WIRELESS,
                WIRELESS,
            ),
            (
                0x1632,
                K::ApexProTkl2023,
                "Apex Pro TKL (2023) Wireless",
                WIRELESS,
                WIRELESS,
            ),
            (0x1634, K::ApexProTkl2023, "Apex 9 TKL", APEX_9, APEX_9_GEN3),
            (0x1640, K::ApexProTkl2023, "Apex Pro (2024)", GEN1, GEN1),
            (0x1642, K::ApexProTkl2023, "Apex Pro TKL (2024)", GEN3, GEN3),
            (
                0x1644,
                K::ApexProTkl2023,
                "Apex Pro TKL Wireless (2024) (Dongle)",
                WIRELESS,
                WIRELESS,
            ),
            (
                0x1646,
                K::ApexProTkl2023,
                "Apex Pro TKL Wireless (2024)",
                WIRELESS,
                WIRELESS,
            ),
            (0x1648, K::Generic, "Apex Pro Mini (2024)", None, None),
            (0x1650, K::Generic, "Apex 5 (2024)", None, None),
            (0x1652, K::Generic, "Apex 7 (2024)", None, None),
            (0x0616, K::ApexM750, "Apex M750", None, None),
        ];
        let frame = |keyboard: &ApexProTkl2023| -> Frame {
            keyboard.direct_packet_id().map(|id| {
                (
                    id,
                    keyboard.direct_packet_length(),
                    keyboard.direct_keys().len(),
                    keyboard.needs_init(),
                )
            })
        };
        for &(pid, class, name, before_probe, gen3) in rows {
            assert_eq!(keyboard_class(pid), class, "{pid:#06x} class");
            assert_eq!(device_name_from_product_id(pid), name, "{pid:#06x} name");
            assert_eq!(
                device_type_from_product_id(pid),
                DeviceType::Keyboard,
                "{pid:#06x} type"
            );
            let mut keyboard = keyboard_for(pid);
            assert_eq!(frame(&keyboard), before_probe, "{pid:#06x} frame before a probe");
            keyboard.gen3_firmware = Some(true);
            assert_eq!(frame(&keyboard), gen3, "{pid:#06x} frame on Gen 3 firmware");
        }
    }

    #[test]
    fn key_table_covers_every_led_including_the_logo() {
        assert_eq!(TKL_KEYS.len(), 112);
        assert_eq!(*TKL_KEYS.last().unwrap(), 0xFB, "logo LED must be addressed");
        // The direct packet is a 3-byte header plus 4 bytes per key.
        assert!(3 + TKL_KEYS.len() * 4 <= APEX_2023_PACKET_LENGTH);
    }
}
