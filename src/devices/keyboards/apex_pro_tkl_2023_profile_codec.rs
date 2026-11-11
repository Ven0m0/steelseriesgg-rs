//! [EXPERIMENTAL] Typed codec of the 12,500-byte on-board profile blob of the wired Apex Pro TKL 2023
//! (PID 0x1628, firmware 1.19.7).
//!
//! The layout is GG's packed device struct (blob schema 3, device schema 8), every offset checked against the
//! five slots read back on 2026-11-10. Regions whose meaning is unknown (`zone_key_subscriptions`, the macro
//! events, the OLED image, the GUID, reserved and padding bytes) are kept as raw bytes, so a decode followed by
//! an encode reproduces a read blob byte for byte. Nothing here talks to a keyboard; the codec has not yet
//! been used for a write replayed from `ssgg` on hardware.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::apex_pro_tkl_2023_profile::{
    ACTUATION_POINTS, PROFILE_CHECKSUM_OFFSET, PROFILE_LEN, actuation_raw, checksum_matches, seal_profile,
};
use super::key_action::KeyAction;
use crate::{Error, Result};

/// Blob schema word at offset 0.
pub const BLOB_SCHEMA: u32 = 3;
/// Device-profile schema word at offset 1080.
pub const DEVICE_SCHEMA: u32 = 8;
/// Analog keys (and `ma_mappings` entries) the profile carries.
pub const ANALOG_KEYS: usize = 70;
/// Entries of one button-mapping table.
pub const MAPPING_ENTRIES: usize = 100;
/// Rapid-tap pairs the profile carries.
pub const RAPID_TAP_PAIRS: usize = 10;
/// Bytes of the profile name field.
pub const NAME_LEN: usize = 24;

/// Offsets of the packed struct, in blob order.
pub mod offsets {
    pub const SCHEMA: usize = 0;
    pub const NAME: usize = 4;
    pub const LOCK_BRIGHTNESS: usize = 28;
    pub const LOCK_COLOR: usize = 29;
    pub const LOCK_HIDS: usize = 32;
    pub const META_BRIGHTNESS: usize = 36;
    pub const META_COLOR: usize = 37;
    pub const META_HIDS_MASK: usize = 40;
    pub const META_DIMMER: usize = 72;
    pub const LIGHTING_RESERVED: usize = 73;
    pub const META_KEY: usize = 76;
    pub const MAPPINGS: usize = 80;
    pub const META_MAPPINGS: usize = 580;
    pub const DEVICE_SCHEMA: usize = 1080;
    pub const SENSITIVITY_H: usize = 1084;
    pub const SENSITIVITY_L: usize = 1154;
    pub const ZONE_LEVELS: usize = 1224;
    pub const ZONE_KEY_SUBSCRIPTIONS: usize = 1236;
    pub const SECOND_SENSITIVITY_H: usize = 1306;
    pub const SECOND_SENSITIVITY_L: usize = 1376;
    pub const RELEASE_MODE: usize = 1446;
    pub const ADAPTIVE_DIST: usize = 1516;
    pub const PROTECTION_DUR: usize = 1586;
    pub const PROTECTION_DIST: usize = 1590;
    pub const MA_MAPPINGS: usize = 1660;
    pub const OLED: usize = 2010;
    pub const RAPID_TAP: usize = 2650;
    pub const RAPID_TAP_ENABLE: usize = 2700;
    pub const PADDING: usize = 2701;
    pub const MACRO_EVENTS: usize = 2704;
    pub const GUID: usize = 12304;
    /// The CRC (see `PROFILE_CHECKSUM_OFFSET`); the bytes behind it are `0xFF`.
    pub const CRC: usize = super::PROFILE_CHECKSUM_OFFSET;
}

use offsets as o;

/// HID usage per mapping-table entry (GG `device-key-idx-to-hid`). The first 70 entries are the analog keys.
pub const DEVICE_KEY_HID: [u8; MAPPING_ENTRIES] = [
    53, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 45, 46, 137, 43, 20, 26, 8, 21, 23, 28, 24, 12, 18, 19, 47, 48, 49, 57,
    4, 22, 7, 9, 10, 11, 13, 14, 15, 51, 52, 50, 40, 225, 100, 29, 27, 6, 25, 5, 17, 16, 54, 55, 56, 135, 229, 224,
    227, 226, 139, 44, 138, 136, 230, 231, 240, 228, 42, 0, 0, 41, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 73,
    74, 75, 76, 77, 78, 82, 80, 81, 79, 0, 245, 246, 247, 241, 254, 255,
];

/// Fixed-size byte array shown as a lower-case hex string in JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hex<const N: usize>(pub [u8; N]);

impl<const N: usize> Serialize for Hex<N> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let text: String = self.0.iter().map(|b| format!("{b:02x}")).collect();
        serializer.serialize_str(&text)
    }
}

impl<'de, const N: usize> Deserialize<'de> for Hex<N> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        if text.len() != N * 2 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(serde::de::Error::custom(format!(
                "expected {} hex digits, got {}",
                N * 2,
                text.len()
            )));
        }
        let mut bytes = [0u8; N];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16)
                .map_err(|_| serde::de::Error::custom(format!("not a hex byte at digit {}", i * 2)))?;
        }
        Ok(Self(bytes))
    }
}

/// A HID usage shown as `0xNN`.
mod hid_text {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(hid: &u8, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&format_args!("0x{hid:02X}"))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u8, D::Error> {
        let text = String::deserialize(deserializer)?;
        let digits = text
            .strip_prefix("0x")
            .or_else(|| text.strip_prefix("0X"))
            .unwrap_or(&text);
        u8::from_str_radix(digits, 16).map_err(|_| serde::de::Error::custom(format!("{text}: not a HID usage")))
    }
}

/// A [`KeyAction`] shown in its CLI grammar (`key:0x04`, `unbound`, ...).
mod key_action_text {
    use serde::{Deserialize, Deserializer, Serializer};

    use crate::devices::keyboards::key_action::KeyAction;

    pub fn serialize<S: Serializer>(action: &KeyAction, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(action)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<KeyAction, D::Error> {
        let text = String::deserialize(deserializer)?;
        KeyAction::parse(&text).map_err(serde::de::Error::custom)
    }
}

/// One mapping-table entry: the physical key by its HID usage (`hid`, informational, checked on encode so an
/// edit cannot shift a table) and what it does. `hid` 0 marks a slot with no key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    #[serde(with = "hid_text")]
    pub hid: u8,
    #[serde(with = "key_action_text")]
    pub key: KeyAction,
}

/// Actuation or release-pair of one analog key: a distance from GG's 40-step table, or a raw `(h, l)` pair
/// that is not in the table (kept as it is).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Actuation {
    /// Tenths of a millimetre, 1..=40.
    Mm(u8),
    /// Raw `(actuation, reset)` hall pair.
    Raw { h: u8, l: u8 },
}

impl Actuation {
    fn from_pair(h: u8, l: u8) -> Self {
        ACTUATION_POINTS
            .iter()
            .position(|&pair| pair == (h, l))
            .map_or(Self::Raw { h, l }, |i| Self::Mm(i as u8 + 1))
    }

    fn to_pair(self) -> Result<(u8, u8)> {
        match self {
            Self::Mm(tenths) => actuation_raw(tenths),
            Self::Raw { h, l } => Ok((h, l)),
        }
    }
}

impl fmt::Display for Actuation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mm(tenths) => write!(f, "{}.{}", tenths / 10, tenths % 10),
            Self::Raw { h, l } => write!(f, "raw:{h}:{l}"),
        }
    }
}

impl FromStr for Actuation {
    type Err = Error;

    /// `1.8` (millimetres, 0.1 steps) or `raw:<h>:<l>`.
    fn from_str(text: &str) -> Result<Self> {
        let bad = || Error::Profile(format!("{text}: not an actuation (`1.8` mm or `raw:<h>:<l>`)"));
        if let Some(rest) = text.strip_prefix("raw:") {
            let (h, l) = rest.split_once(':').ok_or_else(bad)?;
            return Ok(Self::Raw {
                h: h.parse().map_err(|_| bad())?,
                l: l.parse().map_err(|_| bad())?,
            });
        }
        let (whole, tenth) = text.split_once('.').unwrap_or((text, "0"));
        let tenths = match (whole.parse::<u8>(), tenth.len(), tenth.parse::<u8>()) {
            (Ok(whole), 1, Ok(tenth)) if whole <= 4 => whole * 10 + tenth,
            _ => return Err(bad()),
        };
        actuation_raw(tenths)?;
        Ok(Self::Mm(tenths))
    }
}

impl Serialize for Actuation {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Actuation {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// Lock-key and SteelSeries-key lighting (GG `lighting_profile`, offsets 28-75).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lighting {
    pub lock_keys_brightness: u8,
    pub lock_keys_color: [u8; 3],
    pub lock_keys_phys_hids: [u8; 4],
    pub meta_keys_brightness: u8,
    pub meta_keys_color: [u8; 3],
    pub meta_keys_phys_hids_msk: Hex<32>,
    pub meta_background_dimmer: u8,
    pub reserved: Hex<3>,
}

/// One rapid-tap key pair (GG `rapid_tap_pair`): `hid1 hid2 mode|0x80 0 0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RapidTapPair {
    pub hid1: u8,
    pub hid2: u8,
    /// Low seven bits of the third byte.
    pub mode: u8,
    /// Bit 7 of the third byte.
    pub report_both: bool,
    pub reserved: [u8; 2],
}

/// The whole on-board profile of one slot. See the module documentation for what is typed and what is raw.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gen3Profile {
    pub schema: u32,
    pub name: String,
    pub lighting: Lighting,
    pub meta_key_phys_hid: u32,
    /// Base layer, 100 entries in [`DEVICE_KEY_HID`] order.
    pub button_mappings: Vec<Binding>,
    /// SteelSeries-key layer, 100 entries.
    pub button_meta_mappings: Vec<Binding>,
    pub device_profile_schema: u32,
    /// Primary actuation of the 70 analog keys.
    pub actuation: Vec<Actuation>,
    pub zone_levels: [u32; 3],
    pub zone_key_subscriptions: Hex<ANALOG_KEYS>,
    /// Second actuation point (GG `multi_actuation_v2`) of the 70 analog keys.
    pub second_actuation: Vec<Actuation>,
    pub release_mode: Vec<u8>,
    pub adaptive_dist_10: Vec<u8>,
    pub protection_dur_ms: u32,
    pub protection_dist_10: Vec<u8>,
    /// Second-layer mappings of the 70 analog keys.
    pub ma_mappings: Vec<Binding>,
    pub oled: Hex<640>,
    pub rapid_tap: Vec<RapidTapPair>,
    pub rapid_tap_enable: u8,
    pub padding: Hex<3>,
    pub macro_events: Hex<9600>,
    pub guid: Hex<16>,
}

fn array<const N: usize>(blob: &[u8; PROFILE_LEN], offset: usize) -> [u8; N] {
    let mut out = [0u8; N];
    out.copy_from_slice(&blob[offset..offset + N]);
    out
}

fn le32(blob: &[u8; PROFILE_LEN], offset: usize) -> u32 {
    u32::from_le_bytes(array(blob, offset))
}

fn bindings(blob: &[u8; PROFILE_LEN], offset: usize, count: usize) -> Vec<Binding> {
    (0..count)
        .map(|i| {
            let e = &blob[offset + 5 * i..offset + 5 * i + 5];
            Binding {
                hid: DEVICE_KEY_HID[i],
                key: KeyAction::from_bytes(e[0], [e[1], e[2], e[3], e[4]]),
            }
        })
        .collect()
}

fn actuations(blob: &[u8; PROFILE_LEN], h: usize, l: usize) -> Vec<Actuation> {
    (0..ANALOG_KEYS)
        .map(|i| Actuation::from_pair(blob[h + i], blob[l + i]))
        .collect()
}

fn check_len<T>(what: &str, items: &[T], expected: usize) -> Result<()> {
    if items.len() != expected {
        return Err(Error::Profile(format!(
            "{what}: {} entries, the profile needs {expected}",
            items.len()
        )));
    }
    Ok(())
}

fn put_bindings(blob: &mut [u8; PROFILE_LEN], offset: usize, what: &str, items: &[Binding]) -> Result<()> {
    for (i, binding) in items.iter().enumerate() {
        if binding.hid != DEVICE_KEY_HID[i] {
            return Err(Error::Profile(format!(
                "{what}[{i}]: key {:#04x}, the table has {:#04x} at this position",
                binding.hid, DEVICE_KEY_HID[i]
            )));
        }
        let (function, codes) = binding.key.to_bytes();
        blob[offset + 5 * i] = function;
        blob[offset + 5 * i + 1..offset + 5 * i + 5].copy_from_slice(&codes);
    }
    Ok(())
}

fn put_actuations(blob: &mut [u8; PROFILE_LEN], h: usize, l: usize, items: &[Actuation]) -> Result<()> {
    for (i, item) in items.iter().enumerate() {
        (blob[h + i], blob[l + i]) = item.to_pair()?;
    }
    Ok(())
}

impl Gen3Profile {
    /// Decode a read blob.
    ///
    /// # Errors
    ///
    /// Returns an error for a blob that is not 12,500 bytes, has another schema, or whose checksum does not
    /// match; use [`Self::decode_forensic`] to look inside a blob with a bad checksum.
    pub fn decode(blob: &[u8]) -> Result<Self> {
        let (profile, crc_ok) = Self::decode_forensic(blob)?;
        if !crc_ok {
            return Err(Error::Profile(format!(
                "checksum at offset {PROFILE_CHECKSUM_OFFSET} does not match the blob"
            )));
        }
        Ok(profile)
    }

    /// Decode a blob and also say whether its checksum matches, for forensics. Length and both schema words
    /// are still enforced: the layout is not known for any other value.
    ///
    /// # Errors
    ///
    /// Returns an error for a blob that is not 12,500 bytes or has another schema.
    pub fn decode_forensic(blob: &[u8]) -> Result<(Self, bool)> {
        let blob: &[u8; PROFILE_LEN] = blob
            .try_into()
            .map_err(|_| Error::Profile(format!("profile blob is {} bytes, expected {PROFILE_LEN}", blob.len())))?;
        let schema = le32(blob, o::SCHEMA);
        if schema != BLOB_SCHEMA {
            return Err(Error::Profile(format!(
                "blob schema {schema}, only {BLOB_SCHEMA} is known"
            )));
        }
        let device_profile_schema = le32(blob, o::DEVICE_SCHEMA);
        if device_profile_schema != DEVICE_SCHEMA {
            return Err(Error::Profile(format!(
                "device profile schema {device_profile_schema}, only {DEVICE_SCHEMA} is known"
            )));
        }

        let name_field = &blob[o::NAME..o::NAME + NAME_LEN];
        let name_len = name_field.iter().position(|&b| b == 0).unwrap_or(NAME_LEN);
        let rapid_tap = (0..RAPID_TAP_PAIRS)
            .map(|i| {
                let e = &blob[o::RAPID_TAP + 5 * i..o::RAPID_TAP + 5 * i + 5];
                RapidTapPair {
                    hid1: e[0],
                    hid2: e[1],
                    mode: e[2] & 0x7F,
                    report_both: e[2] & 0x80 != 0,
                    reserved: [e[3], e[4]],
                }
            })
            .collect();

        let profile = Self {
            schema,
            name: String::from_utf8_lossy(&name_field[..name_len]).into_owned(),
            lighting: Lighting {
                lock_keys_brightness: blob[o::LOCK_BRIGHTNESS],
                lock_keys_color: array(blob, o::LOCK_COLOR),
                lock_keys_phys_hids: array(blob, o::LOCK_HIDS),
                meta_keys_brightness: blob[o::META_BRIGHTNESS],
                meta_keys_color: array(blob, o::META_COLOR),
                meta_keys_phys_hids_msk: Hex(array(blob, o::META_HIDS_MASK)),
                meta_background_dimmer: blob[o::META_DIMMER],
                reserved: Hex(array(blob, o::LIGHTING_RESERVED)),
            },
            meta_key_phys_hid: le32(blob, o::META_KEY),
            button_mappings: bindings(blob, o::MAPPINGS, MAPPING_ENTRIES),
            button_meta_mappings: bindings(blob, o::META_MAPPINGS, MAPPING_ENTRIES),
            device_profile_schema,
            actuation: actuations(blob, o::SENSITIVITY_H, o::SENSITIVITY_L),
            zone_levels: [
                le32(blob, o::ZONE_LEVELS),
                le32(blob, o::ZONE_LEVELS + 4),
                le32(blob, o::ZONE_LEVELS + 8),
            ],
            zone_key_subscriptions: Hex(array(blob, o::ZONE_KEY_SUBSCRIPTIONS)),
            second_actuation: actuations(blob, o::SECOND_SENSITIVITY_H, o::SECOND_SENSITIVITY_L),
            release_mode: blob[o::RELEASE_MODE..o::RELEASE_MODE + ANALOG_KEYS].to_vec(),
            adaptive_dist_10: blob[o::ADAPTIVE_DIST..o::ADAPTIVE_DIST + ANALOG_KEYS].to_vec(),
            protection_dur_ms: le32(blob, o::PROTECTION_DUR),
            protection_dist_10: blob[o::PROTECTION_DIST..o::PROTECTION_DIST + ANALOG_KEYS].to_vec(),
            ma_mappings: bindings(blob, o::MA_MAPPINGS, ANALOG_KEYS),
            oled: Hex(array(blob, o::OLED)),
            rapid_tap,
            rapid_tap_enable: blob[o::RAPID_TAP_ENABLE],
            padding: Hex(array(blob, o::PADDING)),
            macro_events: Hex(array(blob, o::MACRO_EVENTS)),
            guid: Hex(array(blob, o::GUID)),
        };
        Ok((profile, checksum_matches(blob)))
    }

    /// Encode to the blob GG writes: every field at its offset, the CRC resealed and the bytes behind it
    /// `0xFF`.
    ///
    /// # Errors
    ///
    /// Returns an error when a table has the wrong number of entries, a mapping sits at another key's
    /// position, an actuation is outside GG's table, or the name does not fit 24 bytes of printable ASCII.
    pub fn encode(&self) -> Result<[u8; PROFILE_LEN]> {
        check_len("button_mappings", &self.button_mappings, MAPPING_ENTRIES)?;
        check_len("button_meta_mappings", &self.button_meta_mappings, MAPPING_ENTRIES)?;
        check_len("actuation", &self.actuation, ANALOG_KEYS)?;
        check_len("second_actuation", &self.second_actuation, ANALOG_KEYS)?;
        check_len("release_mode", &self.release_mode, ANALOG_KEYS)?;
        check_len("adaptive_dist_10", &self.adaptive_dist_10, ANALOG_KEYS)?;
        check_len("protection_dist_10", &self.protection_dist_10, ANALOG_KEYS)?;
        check_len("ma_mappings", &self.ma_mappings, ANALOG_KEYS)?;
        check_len("rapid_tap", &self.rapid_tap, RAPID_TAP_PAIRS)?;
        if self.schema != BLOB_SCHEMA || self.device_profile_schema != DEVICE_SCHEMA {
            return Err(Error::Profile(format!(
                "schema {}/{}: only {BLOB_SCHEMA}/{DEVICE_SCHEMA} can be written",
                self.schema, self.device_profile_schema
            )));
        }
        let name = self.name.as_bytes();
        if name.len() > NAME_LEN || name.iter().any(|b| !(0x20..=0x7E).contains(b)) {
            return Err(Error::Profile(format!(
                "profile name must be at most {NAME_LEN} printable ASCII bytes (0x20-0x7E), got {:?}",
                self.name
            )));
        }

        let mut blob = [0u8; PROFILE_LEN];
        blob[o::CRC + 4..].fill(0xFF);
        blob[o::SCHEMA..o::SCHEMA + 4].copy_from_slice(&self.schema.to_le_bytes());
        blob[o::NAME..o::NAME + name.len()].copy_from_slice(name);
        let l = &self.lighting;
        blob[o::LOCK_BRIGHTNESS] = l.lock_keys_brightness;
        blob[o::LOCK_COLOR..o::LOCK_COLOR + 3].copy_from_slice(&l.lock_keys_color);
        blob[o::LOCK_HIDS..o::LOCK_HIDS + 4].copy_from_slice(&l.lock_keys_phys_hids);
        blob[o::META_BRIGHTNESS] = l.meta_keys_brightness;
        blob[o::META_COLOR..o::META_COLOR + 3].copy_from_slice(&l.meta_keys_color);
        blob[o::META_HIDS_MASK..o::META_HIDS_MASK + 32].copy_from_slice(&l.meta_keys_phys_hids_msk.0);
        blob[o::META_DIMMER] = l.meta_background_dimmer;
        blob[o::LIGHTING_RESERVED..o::LIGHTING_RESERVED + 3].copy_from_slice(&l.reserved.0);
        blob[o::META_KEY..o::META_KEY + 4].copy_from_slice(&self.meta_key_phys_hid.to_le_bytes());
        put_bindings(&mut blob, o::MAPPINGS, "button_mappings", &self.button_mappings)?;
        put_bindings(
            &mut blob,
            o::META_MAPPINGS,
            "button_meta_mappings",
            &self.button_meta_mappings,
        )?;
        blob[o::DEVICE_SCHEMA..o::DEVICE_SCHEMA + 4].copy_from_slice(&self.device_profile_schema.to_le_bytes());
        put_actuations(&mut blob, o::SENSITIVITY_H, o::SENSITIVITY_L, &self.actuation)?;
        for (i, level) in self.zone_levels.iter().enumerate() {
            blob[o::ZONE_LEVELS + 4 * i..o::ZONE_LEVELS + 4 * i + 4].copy_from_slice(&level.to_le_bytes());
        }
        blob[o::ZONE_KEY_SUBSCRIPTIONS..o::ZONE_KEY_SUBSCRIPTIONS + ANALOG_KEYS]
            .copy_from_slice(&self.zone_key_subscriptions.0);
        put_actuations(
            &mut blob,
            o::SECOND_SENSITIVITY_H,
            o::SECOND_SENSITIVITY_L,
            &self.second_actuation,
        )?;
        blob[o::RELEASE_MODE..o::RELEASE_MODE + ANALOG_KEYS].copy_from_slice(&self.release_mode);
        blob[o::ADAPTIVE_DIST..o::ADAPTIVE_DIST + ANALOG_KEYS].copy_from_slice(&self.adaptive_dist_10);
        blob[o::PROTECTION_DUR..o::PROTECTION_DUR + 4].copy_from_slice(&self.protection_dur_ms.to_le_bytes());
        blob[o::PROTECTION_DIST..o::PROTECTION_DIST + ANALOG_KEYS].copy_from_slice(&self.protection_dist_10);
        put_bindings(&mut blob, o::MA_MAPPINGS, "ma_mappings", &self.ma_mappings)?;
        blob[o::OLED..o::OLED + 640].copy_from_slice(&self.oled.0);
        for (i, pair) in self.rapid_tap.iter().enumerate() {
            let at = o::RAPID_TAP + 5 * i;
            blob[at] = pair.hid1;
            blob[at + 1] = pair.hid2;
            blob[at + 2] = (pair.mode & 0x7F) | if pair.report_both { 0x80 } else { 0 };
            blob[at + 3..at + 5].copy_from_slice(&pair.reserved);
        }
        blob[o::RAPID_TAP_ENABLE] = self.rapid_tap_enable;
        blob[o::PADDING..o::PADDING + 3].copy_from_slice(&self.padding.0);
        blob[o::MACRO_EVENTS..o::MACRO_EVENTS + 9600].copy_from_slice(&self.macro_events.0);
        blob[o::GUID..o::GUID + 16].copy_from_slice(&self.guid.0);
        seal_profile(&mut blob);
        Ok(blob)
    }

    /// Field-level differences from `self` (the stored profile) to `new`, one line per changed field, mapping
    /// entry or actuation entry, and one line per raw region with the offsets (inside the region) that differ.
    /// Empty when the two are equal.
    #[must_use]
    pub fn diff(&self, new: &Self) -> Vec<String> {
        let mut out = Vec::new();
        fn scalar<T: PartialEq + fmt::Debug>(out: &mut Vec<String>, name: &str, old: &T, new: &T) {
            if old != new {
                out.push(format!("{name}: {old:?} -> {new:?}"));
            }
        }
        fn each<T: PartialEq>(
            out: &mut Vec<String>,
            name: &str,
            old: &[T],
            new: &[T],
            label: impl Fn(usize, &T) -> String,
            show: impl Fn(&T) -> String,
        ) {
            if old.len() != new.len() {
                out.push(format!("{name}: {} entries -> {} entries", old.len(), new.len()));
                return;
            }
            for (i, (a, b)) in old.iter().zip(new).enumerate() {
                if a != b {
                    out.push(format!("{name}[{}]: {} -> {}", label(i, a), show(a), show(b)));
                }
            }
        }
        fn raw(out: &mut Vec<String>, name: &str, old: &[u8], new: &[u8]) {
            let differing: Vec<usize> = (0..old.len().min(new.len())).filter(|&i| old[i] != new[i]).collect();
            let Some(&first) = differing.first() else {
                return;
            };
            let (mut runs, mut start, mut prev) = (Vec::new(), first, first);
            for &at in &differing[1..] {
                if at != prev + 1 {
                    runs.push((start, prev));
                    start = at;
                }
                prev = at;
            }
            runs.push((start, prev));
            let shown: Vec<String> = runs
                .iter()
                .take(6)
                .map(|&(a, b)| {
                    if a == b {
                        format!("+{a}")
                    } else {
                        format!("+{a}..=+{b}")
                    }
                })
                .collect();
            let more = if runs.len() > 6 { ", ..." } else { "" };
            out.push(format!(
                "{name}: {} bytes differ at offsets {}{more}",
                differing.len(),
                shown.join(", ")
            ));
        }
        let binding_label = |_: usize, b: &Binding| format!("{:#04x}", b.hid);
        let binding_show = |b: &Binding| b.key.to_string();
        let index_label = |i: usize, _: &Actuation| i.to_string();
        let byte_label = |i: usize, _: &u8| i.to_string();
        let byte_show = |b: &u8| b.to_string();

        scalar(&mut out, "name", &self.name, &new.name);
        scalar(
            &mut out,
            "lock_keys_brightness",
            &self.lighting.lock_keys_brightness,
            &new.lighting.lock_keys_brightness,
        );
        scalar(
            &mut out,
            "lock_keys_color",
            &self.lighting.lock_keys_color,
            &new.lighting.lock_keys_color,
        );
        scalar(
            &mut out,
            "lock_keys_phys_hids",
            &self.lighting.lock_keys_phys_hids,
            &new.lighting.lock_keys_phys_hids,
        );
        scalar(
            &mut out,
            "meta_keys_brightness",
            &self.lighting.meta_keys_brightness,
            &new.lighting.meta_keys_brightness,
        );
        scalar(
            &mut out,
            "meta_keys_color",
            &self.lighting.meta_keys_color,
            &new.lighting.meta_keys_color,
        );
        raw(
            &mut out,
            "meta_keys_phys_hids_msk",
            &self.lighting.meta_keys_phys_hids_msk.0,
            &new.lighting.meta_keys_phys_hids_msk.0,
        );
        scalar(
            &mut out,
            "meta_background_dimmer",
            &self.lighting.meta_background_dimmer,
            &new.lighting.meta_background_dimmer,
        );
        raw(
            &mut out,
            "lighting.reserved",
            &self.lighting.reserved.0,
            &new.lighting.reserved.0,
        );
        scalar(
            &mut out,
            "meta_key_phys_hid",
            &self.meta_key_phys_hid,
            &new.meta_key_phys_hid,
        );
        each(
            &mut out,
            "button_mappings",
            &self.button_mappings,
            &new.button_mappings,
            binding_label,
            binding_show,
        );
        each(
            &mut out,
            "button_meta_mappings",
            &self.button_meta_mappings,
            &new.button_meta_mappings,
            binding_label,
            binding_show,
        );
        each(
            &mut out,
            "actuation",
            &self.actuation,
            &new.actuation,
            index_label,
            Actuation::to_string,
        );
        scalar(&mut out, "zone_levels", &self.zone_levels, &new.zone_levels);
        raw(
            &mut out,
            "zone_key_subscriptions",
            &self.zone_key_subscriptions.0,
            &new.zone_key_subscriptions.0,
        );
        each(
            &mut out,
            "second_actuation",
            &self.second_actuation,
            &new.second_actuation,
            index_label,
            Actuation::to_string,
        );
        each(
            &mut out,
            "release_mode",
            &self.release_mode,
            &new.release_mode,
            byte_label,
            byte_show,
        );
        each(
            &mut out,
            "adaptive_dist_10",
            &self.adaptive_dist_10,
            &new.adaptive_dist_10,
            byte_label,
            byte_show,
        );
        scalar(
            &mut out,
            "protection_dur_ms",
            &self.protection_dur_ms,
            &new.protection_dur_ms,
        );
        each(
            &mut out,
            "protection_dist_10",
            &self.protection_dist_10,
            &new.protection_dist_10,
            byte_label,
            byte_show,
        );
        each(
            &mut out,
            "ma_mappings",
            &self.ma_mappings,
            &new.ma_mappings,
            binding_label,
            binding_show,
        );
        raw(&mut out, "oled", &self.oled.0, &new.oled.0);
        each(
            &mut out,
            "rapid_tap",
            &self.rapid_tap,
            &new.rapid_tap,
            |i, _| i.to_string(),
            |p| format!("{p:?}"),
        );
        scalar(
            &mut out,
            "rapid_tap_enable",
            &self.rapid_tap_enable,
            &new.rapid_tap_enable,
        );
        raw(&mut out, "padding", &self.padding.0, &new.padding.0);
        raw(&mut out, "macro_events", &self.macro_events.0, &new.macro_events.0);
        raw(&mut out, "guid", &self.guid.0, &new.guid.0);
        out
    }

    /// The decode report `ssgg onboard gen3-decode --json` prints.
    ///
    /// # Errors
    ///
    /// Propagates a serde failure.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|e| Error::Profile(format!("profile JSON: {e}")))
    }

    /// Read a decode report back.
    ///
    /// # Errors
    ///
    /// Returns an error for text that is not a decode report.
    pub fn from_json(text: &str) -> Result<Self> {
        serde_json::from_str(text).map_err(|e| Error::Profile(format!("profile JSON: {e}")))
    }
}

/// The write guard on a blob read from the keyboard: it must decode with a matching checksum and re-encode
/// to the very same bytes, so that the codec provably loses nothing of the slot it is about to replace.
///
/// # Errors
///
/// Returns an error when the blob does not decode or re-encodes to different bytes (the first differing
/// offset is named).
pub fn check_lossless(read: &[u8]) -> Result<Gen3Profile> {
    let profile = Gen3Profile::decode(read)?;
    let encoded = profile.encode()?;
    if let Some(at) = encoded.iter().zip(read).position(|(a, b)| a != b) {
        return Err(Error::Profile(format!(
            "re-encoding the read blob differs from it at offset {at} ({:#04x} vs {:#04x}): \
             the codec would lose data",
            encoded[at], read[at]
        )));
    }
    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic filler so every byte of the covered range is distinguishable.
    fn synthetic() -> Box<[u8; PROFILE_LEN]> {
        let mut blob = Box::new([0xFFu8; PROFILE_LEN]);
        let mut state = 0x1234_5678u32;
        for byte in &mut blob[..PROFILE_CHECKSUM_OFFSET] {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *byte = (state >> 24) as u8;
        }
        blob[o::SCHEMA..o::SCHEMA + 4].copy_from_slice(&BLOB_SCHEMA.to_le_bytes());
        blob[o::DEVICE_SCHEMA..o::DEVICE_SCHEMA + 4].copy_from_slice(&DEVICE_SCHEMA.to_le_bytes());
        blob[o::NAME..o::NAME + NAME_LEN].fill(0);
        blob[o::NAME..o::NAME + 4].copy_from_slice(b"main");
        seal_profile(&mut blob);
        blob
    }

    #[test]
    fn synthetic_blob_round_trips_byte_for_byte() {
        let blob = synthetic();
        let profile = Gen3Profile::decode(&blob[..]).unwrap();
        assert_eq!(profile.name, "main");
        assert_eq!(profile.encode().unwrap()[..], blob[..]);
        check_lossless(&blob[..]).unwrap();

        let again = Gen3Profile::from_json(&profile.to_json().unwrap()).unwrap();
        assert_eq!(again, profile);
        assert_eq!(again.encode().unwrap()[..], blob[..]);
    }

    #[test]
    fn zero_usage_keyboard_bindings_survive_profile_json() {
        let mut profile = Gen3Profile::decode(&synthetic()[..]).unwrap();
        profile.button_mappings[0].key = KeyAction::Key(0);
        profile.button_mappings[1].key = KeyAction::Unbound;
        let blob = profile.encode().unwrap();
        let decoded = Gen3Profile::decode(&blob[..]).unwrap();
        let json = decoded.to_json().unwrap();
        let mut again = Gen3Profile::from_json(&json).unwrap();
        assert_eq!(again.button_mappings[0].key.to_bytes(), (0x51, [0; 4]));
        assert_eq!(again.button_mappings[1].key.to_bytes(), (0x00, [0; 4]));
        assert_eq!(again.encode().unwrap()[..], blob[..]);

        again.name = "renamed".into();
        profile.name = "renamed".into();
        assert_eq!(again.encode().unwrap(), profile.encode().unwrap());
    }

    #[test]
    fn every_field_sits_at_its_documented_offset() {
        let mut blob = Box::new([0u8; PROFILE_LEN]);
        blob[o::SCHEMA] = 3;
        blob[o::DEVICE_SCHEMA] = 8;
        let marks: [(usize, u8); 31] = [
            (o::NAME, b'A'),
            (o::LOCK_BRIGHTNESS, 0x11),
            (o::LOCK_COLOR, 0x12),
            (o::LOCK_HIDS, 0x13),
            (o::META_BRIGHTNESS, 0x14),
            (o::META_COLOR, 0x15),
            (o::META_HIDS_MASK, 0x16),
            (o::META_DIMMER, 0x17),
            (o::LIGHTING_RESERVED, 0x18),
            (o::META_KEY, 0xF0),
            (o::MAPPINGS, 0x51),
            (o::META_MAPPINGS, 0x61),
            (o::SENSITIVITY_H, 0x21),
            (o::SENSITIVITY_L, 0x22),
            (o::ZONE_LEVELS, 0x23),
            (o::ZONE_KEY_SUBSCRIPTIONS, 0x24),
            (o::SECOND_SENSITIVITY_H, 0x25),
            (o::SECOND_SENSITIVITY_L, 0x26),
            (o::RELEASE_MODE, 0x27),
            (o::ADAPTIVE_DIST, 0x28),
            (o::PROTECTION_DUR, 0x29),
            (o::PROTECTION_DIST, 0x2A),
            (o::MA_MAPPINGS, 0x72),
            (o::OLED, 0x2B),
            (o::RAPID_TAP, 0x2C),
            (o::RAPID_TAP_ENABLE, 0x2D),
            (o::PADDING, 0x2E),
            (o::MACRO_EVENTS, 0x2F),
            (o::GUID, 0x30),
            (o::RAPID_TAP + 2, 0x84),
            (o::MA_MAPPINGS + 5 * 69 + 4, 0x31),
        ];
        for (offset, mark) in marks {
            blob[offset] = mark;
        }
        let (p, _) = Gen3Profile::decode_forensic(&blob[..]).unwrap();

        assert_eq!(p.name, "A");
        assert_eq!(p.lighting.lock_keys_brightness, 0x11);
        assert_eq!(p.lighting.lock_keys_color[0], 0x12);
        assert_eq!(p.lighting.lock_keys_phys_hids[0], 0x13);
        assert_eq!(p.lighting.meta_keys_brightness, 0x14);
        assert_eq!(p.lighting.meta_keys_color[0], 0x15);
        assert_eq!(p.lighting.meta_keys_phys_hids_msk.0[0], 0x16);
        assert_eq!(p.lighting.meta_background_dimmer, 0x17);
        assert_eq!(p.lighting.reserved.0[0], 0x18);
        assert_eq!(p.meta_key_phys_hid, 0xF0);
        assert_eq!(p.button_mappings[0].key, KeyAction::Key(0));
        assert_eq!(p.button_mappings[0].key.to_bytes().0, 0x51);
        assert_eq!(p.button_meta_mappings[0].key.to_bytes().0, 0x61);
        assert_eq!(p.actuation[0], Actuation::Raw { h: 0x21, l: 0x22 });
        assert_eq!(p.zone_levels[0], 0x23);
        assert_eq!(p.zone_key_subscriptions.0[0], 0x24);
        assert_eq!(p.second_actuation[0], Actuation::Raw { h: 0x25, l: 0x26 });
        assert_eq!(p.release_mode[0], 0x27);
        assert_eq!(p.adaptive_dist_10[0], 0x28);
        assert_eq!(p.protection_dur_ms, 0x29);
        assert_eq!(p.protection_dist_10[0], 0x2A);
        assert_eq!(p.ma_mappings[0].key.to_bytes().0, 0x72);
        assert_eq!(p.ma_mappings[69].key.to_bytes().1[3], 0x31);
        assert_eq!(p.oled.0[0], 0x2B);
        assert_eq!(p.rapid_tap[0].hid1, 0x2C);
        assert_eq!((p.rapid_tap[0].mode, p.rapid_tap[0].report_both), (4, true));
        assert_eq!(p.rapid_tap_enable, 0x2D);
        assert_eq!(p.padding.0[0], 0x2E);
        assert_eq!(p.macro_events.0[0], 0x2F);
        assert_eq!(p.guid.0[0], 0x30);

        // The encode side puts the same marks at the same offsets; the CRC is resealed behind them.
        let encoded = p.encode().unwrap();
        for (offset, mark) in marks {
            assert_eq!(encoded[offset], mark, "offset {offset}");
        }
        assert_eq!(o::CRC, 12_320);
        assert!(checksum_matches(&encoded));
        assert!(encoded[o::CRC + 4..].iter().all(|&b| b == 0xFF));
    }

    #[test]
    fn decode_rejects_bad_length_schema_and_crc() {
        let blob = synthetic();
        assert!(Gen3Profile::decode(&blob[..PROFILE_LEN - 1]).is_err());

        let mut bad_schema = blob.clone();
        bad_schema[o::SCHEMA] = 4;
        seal_profile(&mut bad_schema);
        assert!(Gen3Profile::decode_forensic(&bad_schema[..]).is_err());

        let mut bad_device = blob.clone();
        bad_device[o::DEVICE_SCHEMA] = 9;
        seal_profile(&mut bad_device);
        assert!(Gen3Profile::decode(&bad_device[..]).is_err());

        let mut bad_crc = blob.clone();
        bad_crc[o::OLED] ^= 0xFF;
        let err = Gen3Profile::decode(&bad_crc[..]).unwrap_err().to_string();
        assert!(err.contains("checksum"), "{err}");
        let (profile, crc_ok) = Gen3Profile::decode_forensic(&bad_crc[..]).unwrap();
        assert!(!crc_ok);
        assert_eq!(profile.oled.0[0], bad_crc[o::OLED]);
    }

    #[test]
    fn lossless_guard_catches_what_the_codec_cannot_keep() {
        let mut blob = synthetic();
        // A name byte behind the first NUL is not representable: the re-encode zeroes it.
        blob[o::NAME + 10] = b'x';
        seal_profile(&mut blob);
        assert!(Gen3Profile::decode(&blob[..]).is_ok());
        let err = check_lossless(&blob[..]).unwrap_err().to_string();
        assert!(err.contains(&format!("offset {}", o::NAME + 10)), "{err}");

        // So is a tail that is not 0xFF.
        let mut tail = synthetic();
        tail[PROFILE_LEN - 1] = 0;
        assert!(check_lossless(&tail[..]).is_err());
    }

    #[test]
    fn edits_are_validated_on_encode() {
        let blob = synthetic();
        let mut profile = Gen3Profile::decode(&blob[..]).unwrap();

        profile.actuation[3] = "1.8".parse().unwrap();
        profile.second_actuation[3] = Actuation::Mm(40);
        let encoded = profile.encode().unwrap();
        assert_eq!((encoded[o::SENSITIVITY_H + 3], encoded[o::SENSITIVITY_L + 3]), (47, 42));
        assert_eq!(
            (
                encoded[o::SECOND_SENSITIVITY_H + 3],
                encoded[o::SECOND_SENSITIVITY_L + 3]
            ),
            (224, 219)
        );
        assert_eq!(
            Gen3Profile::decode(&encoded[..]).unwrap().actuation[3],
            Actuation::Mm(18)
        );

        let mut shifted = profile.clone();
        shifted.button_mappings.swap(0, 1);
        assert!(shifted.encode().is_err(), "a mapping moved to another key's slot");

        let mut short = profile.clone();
        short.release_mode.pop();
        assert!(short.encode().is_err());

        let mut long_name = profile.clone();
        long_name.name = "x".repeat(NAME_LEN + 1);
        assert!(long_name.encode().is_err());

        assert!("4.1".parse::<Actuation>().is_err());
        assert!("0.0".parse::<Actuation>().is_err());
        assert!("1.85".parse::<Actuation>().is_err());
        assert_eq!("2".parse::<Actuation>().unwrap(), Actuation::Mm(20));
        assert_eq!("raw:5:4".parse::<Actuation>().unwrap(), Actuation::Raw { h: 5, l: 4 });
        // A pair that is in GG's table comes back as millimetres, one that is not stays raw.
        assert_eq!(Actuation::from_pair(21, 18), Actuation::Mm(10));
        assert_eq!(Actuation::from_pair(21, 17), Actuation::Raw { h: 21, l: 17 });
    }

    #[test]
    fn json_hex_and_bindings_are_strict() {
        let profile = Gen3Profile::decode(&synthetic()[..]).unwrap();
        let mut json: serde_json::Value = serde_json::from_str(&profile.to_json().unwrap()).unwrap();
        assert_eq!(json["button_mappings"][0]["hid"], "0x35");
        assert_eq!(json["guid"].as_str().unwrap().len(), 32);

        json["guid"] = "abcd".into();
        assert!(Gen3Profile::from_json(&json.to_string()).is_err());
    }

    #[test]
    fn names_must_be_printable_ascii_within_24_bytes() {
        let mut profile = Gen3Profile::decode(&synthetic()[..]).unwrap();
        for ok in ["", "main", " ~", &"x".repeat(24)] {
            profile.name = ok.to_string();
            assert!(profile.encode().is_ok(), "{ok:?}");
        }
        for bad in ["tab\there", "nul\0", "caf\u{e9}", "\u{7f}", "line\n", &"x".repeat(25)] {
            profile.name = bad.to_string();
            assert!(profile.encode().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn diff_is_empty_for_equal_profiles_and_names_changed_entries() {
        let old = Gen3Profile::decode(&synthetic()[..]).unwrap();
        assert!(old.diff(&old).is_empty());
        let mut new = old.clone();
        new.button_mappings[3].key = KeyAction::Key(0x04);
        new.actuation[0] = Actuation::Raw { h: 1, l: 2 };
        new.macro_events.0[0] ^= 1;
        new.macro_events.0[9] ^= 1;
        let diff = old.diff(&new).join("\n");
        assert!(
            diff.contains(&format!("button_mappings[{:#04x}]:", DEVICE_KEY_HID[3])),
            "{diff}"
        );
        assert!(diff.contains("actuation[0]:") && diff.contains("raw:1:2"), "{diff}");
        assert!(
            diff.contains("macro_events: 2 bytes differ at offsets +0, +9"),
            "{diff}"
        );
    }

    /// Round-trips every `*.bin` in `SSGG_GEN3_PROFILE_DIR` (blobs read back from a keyboard) byte for byte.
    /// Ignored because it needs saved read-backs: `SSGG_GEN3_PROFILE_DIR=<dir> cargo test -- --ignored`.
    #[test]
    #[ignore = "needs SSGG_GEN3_PROFILE_DIR with blobs read from a keyboard"]
    fn real_blobs_round_trip_byte_for_byte() {
        let dir =
            std::env::var("SSGG_GEN3_PROFILE_DIR").expect("set SSGG_GEN3_PROFILE_DIR to a directory of .bin blobs");
        let mut files: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "bin"))
            .collect();
        let repo_blob = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("apex-pro-tkl-2023-slot0.bin");
        if repo_blob.is_file() {
            files.push(repo_blob);
        }
        assert!(!files.is_empty(), "no .bin files in {dir}");
        for path in files {
            let bytes = std::fs::read(&path).unwrap();
            if bytes.len() != PROFILE_LEN {
                // The capture directory also holds other dumps (a 9,600-byte macro read).
                println!("{}: {} bytes, not a profile blob, skipped", path.display(), bytes.len());
                continue;
            }
            let profile = check_lossless(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            println!("{}: {} ok, name {:?}", path.display(), bytes.len(), profile.name);
        }
    }
}
