//! `[EXPERIMENTAL]` Live key bindings of the wired Apex Pro TKL 2023 (`0x1628`), from GG's decrypted device spec
//! (`base_apex_2022_wired`, `apex_pro_tkl_2022`, 2026-11-10). GG did not send the binding commands (`36`, `B6`,
//! `B7`, `35`) on this board in the 2026-11-10 capture; it rewrote the on-board profile instead. Nothing here is
//! confirmed on hardware.
//!
//! Not implemented: GG follows a mapping write with `00 3C <32-byte bitmask of meta-bound keys>` and the macro data
//! (`00 37`); `ssgg` sends neither, so whether meta-layer bindings take effect without them is unknown.

use std::fmt;

use super::apex_pro_tkl_2023::profile_slot;
use crate::devices::hid_reports::{
    DeviceQueryCommand, HidDeviceType, HidReportBuilder, KeyBinding, KeyMappingCommand, KeyMappingReadCommand,
    MacroReadCommand, PROFILE_CHUNK_REPORT_SIZE,
};
use crate::{Error, Result};

/// The 91 keys of a mapping layer in HID order: GG's `device-mapping-hid-codes`, i.e. the non-zero entries of
/// `device-key-idx-to-hid` (`apex_pro_tkl_2022`) without the OLED buttons `device-ignore-hid-codes`, sorted.
/// Includes `0xF0`, the SteelSeries key.
pub const MAPPING_KEYS: [u8; 91] = [
    0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16,
    0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29,
    0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x3B, 0x3C,
    0x3D, 0x3E, 0x3F, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F, 0x50, 0x51, 0x52,
    0x64, 0x87, 0x88, 0x89, 0x8A, 0x8B, 0xE0, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xF0,
];

/// `[EXPERIMENTAL]` Binding layer of the mapping commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MappingLayer {
    /// Normal key presses.
    Base = 0x00,
    /// Presses while the SteelSeries (meta) key is held.
    Meta = 0x01,
    /// The second actuation point of the analog keys.
    Second = 0x02,
}

impl MappingLayer {
    /// The layer's keys in HID order: [`MAPPING_KEYS`] for base and meta, the 68 analog keys
    /// (`device-analog-hid-codes`) for the second layer.
    pub fn keys(self) -> Vec<u8> {
        MAPPING_KEYS
            .into_iter()
            .filter(|&hid| self != Self::Second || profile_slot(hid).is_some())
            .collect()
    }
}

impl fmt::Display for MappingLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Base => "base",
            Self::Meta => "meta",
            Self::Second => "second",
        })
    }
}

/// `[EXPERIMENTAL]` What a key does, the `function` byte and `key_codes` of GG's `button_mapping`. The spec names
/// every function value but not how most of them use the four key codes: only `keyboard` (code 0 = HID usage, seen
/// in the board's own profile) and `consumer` (code 0 = consumer usage, from GG's OLED-button bindings) are shown
/// with data. Every other function is assumed to carry zero codes, `macro` and `external` one code in byte 0. Any
/// other byte combination is kept as [`KeyAction::Raw`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    /// `0x00`.
    Unbound,
    /// `0x01..=0x08`: mouse button 1-8.
    Mouse(u8),
    /// `0x31`.
    WheelUp,
    /// `0x32`.
    WheelDown,
    /// `0x33`: AC pan left.
    PanLeft,
    /// `0x34`: AC pan right.
    PanRight,
    /// `0x51`: a keyboard HID usage.
    Key(u8),
    /// `0x61`: a consumer-page usage; only 8-bit usages are shown in the spec.
    Consumer(u8),
    /// `0x62`.
    Meta,
    /// `0x30`.
    Cpi,
    /// `0x71`: code 0 assumed to be the macro index.
    Macro(u8),
    /// `0x72`: code 0 kept raw.
    External(u8),
    /// Any other function / key-code combination, bytes as they are.
    Raw(u8, [u8; 4]),
}

/// Hex byte with an optional `0x` prefix.
fn hex_u8(text: &str) -> Option<u8> {
    let digits = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))
        .unwrap_or(text);
    u8::from_str_radix(digits, 16).ok()
}

impl KeyAction {
    /// The `function` byte and `key_codes` on the wire.
    pub fn to_bytes(self) -> (u8, [u8; 4]) {
        match self {
            Self::Unbound => (0x00, [0; 4]),
            Self::Mouse(button) => (button, [0; 4]),
            Self::WheelUp => (0x31, [0; 4]),
            Self::WheelDown => (0x32, [0; 4]),
            Self::PanLeft => (0x33, [0; 4]),
            Self::PanRight => (0x34, [0; 4]),
            Self::Key(usage) => (0x51, [usage, 0, 0, 0]),
            Self::Consumer(usage) => (0x61, [usage, 0, 0, 0]),
            Self::Meta => (0x62, [0; 4]),
            Self::Cpi => (0x30, [0; 4]),
            Self::Macro(index) => (0x71, [index, 0, 0, 0]),
            Self::External(code) => (0x72, [code, 0, 0, 0]),
            Self::Raw(function, codes) => (function, codes),
        }
    }

    /// The action a `function` byte and `key_codes` encode; anything outside the forms above is [`Self::Raw`].
    pub fn from_bytes(function: u8, codes: [u8; 4]) -> Self {
        match (function, codes) {
            (0x00, [0, 0, 0, 0]) => Self::Unbound,
            (button @ 0x01..=0x08, [0, 0, 0, 0]) => Self::Mouse(button),
            (0x31, [0, 0, 0, 0]) => Self::WheelUp,
            (0x32, [0, 0, 0, 0]) => Self::WheelDown,
            (0x33, [0, 0, 0, 0]) => Self::PanLeft,
            (0x34, [0, 0, 0, 0]) => Self::PanRight,
            (0x51, [usage, 0, 0, 0]) => Self::Key(usage),
            (0x61, [usage, 0, 0, 0]) => Self::Consumer(usage),
            (0x62, [0, 0, 0, 0]) => Self::Meta,
            (0x30, [0, 0, 0, 0]) => Self::Cpi,
            (0x71, [index, 0, 0, 0]) => Self::Macro(index),
            (0x72, [code, 0, 0, 0]) => Self::External(code),
            _ => Self::Raw(function, codes),
        }
    }

    /// Parse the CLI grammar: `key:0x<usage>`, `unbound`, `mouse:<1-8>`, `wheel:up|down`, `pan:left|right`,
    /// `consumer:<hex>`, `meta`, `cpi`, `macro:<index>`, `external:<hex>`, `raw:<function>:<kc0>,<kc1>,<kc2>,<kc3>`
    /// (hex). `key:` takes a `0x` usage here; key names are resolved by the CLI. A `raw:` that spells a named
    /// form comes back as that form.
    ///
    /// # Errors
    ///
    /// Returns an error for text outside the grammar.
    pub fn parse(text: &str) -> Result<Self> {
        let bad = || {
            Error::InvalidConfig(format!(
                "{text}: not a key action (key:<name|0xUSAGE>, unbound, mouse:<1-8>, wheel:up|down, \
                 pan:left|right, consumer:<hex>, meta, cpi, macro:<index>, external:<hex>, \
                 raw:<function>:<kc0>,<kc1>,<kc2>,<kc3>)"
            ))
        };
        let (kind, arg) = text.split_once(':').unwrap_or((text, ""));
        Ok(match (kind, arg) {
            ("unbound", "") => Self::Unbound,
            ("meta", "") => Self::Meta,
            ("cpi", "") => Self::Cpi,
            ("wheel", "up") => Self::WheelUp,
            ("wheel", "down") => Self::WheelDown,
            ("pan", "left") => Self::PanLeft,
            ("pan", "right") => Self::PanRight,
            ("mouse", button) => match button.parse::<u8>() {
                Ok(button @ 1..=8) => Self::Mouse(button),
                _ => return Err(bad()),
            },
            ("key", usage) if usage.starts_with("0x") || usage.starts_with("0X") => {
                Self::Key(hex_u8(usage).ok_or_else(bad)?)
            }
            ("consumer", usage) => Self::Consumer(hex_u8(usage).ok_or_else(bad)?),
            ("macro", index) => Self::Macro(index.parse().map_err(|_| bad())?),
            ("external", code) => Self::External(hex_u8(code).ok_or_else(bad)?),
            ("raw", rest) => {
                let (function, codes) = rest.split_once(':').ok_or_else(bad)?;
                let function = hex_u8(function).ok_or_else(bad)?;
                let codes: Vec<u8> = codes.split(',').map(hex_u8).collect::<Option<_>>().ok_or_else(bad)?;
                Self::from_bytes(function, codes.try_into().map_err(|_| bad())?)
            }
            _ => return Err(bad()),
        })
    }
}

impl fmt::Display for KeyAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unbound => write!(f, "unbound"),
            Self::Mouse(button) => write!(f, "mouse:{button}"),
            Self::WheelUp => write!(f, "wheel:up"),
            Self::WheelDown => write!(f, "wheel:down"),
            Self::PanLeft => write!(f, "pan:left"),
            Self::PanRight => write!(f, "pan:right"),
            Self::Key(usage) => write!(f, "key:0x{usage:02X}"),
            Self::Consumer(usage) => write!(f, "consumer:0x{usage:02X}"),
            Self::Meta => write!(f, "meta"),
            Self::Cpi => write!(f, "cpi"),
            Self::Macro(index) => write!(f, "macro:{index}"),
            Self::External(code) => write!(f, "external:0x{code:02X}"),
            Self::Raw(function, [a, b, c, d]) => write!(f, "raw:{function:02X}:{a:02X},{b:02X},{c:02X},{d:02X}"),
        }
    }
}

/// The reset state of a layer, as the owner's slot-0 profile holds it (2026-11-10): every base key bound to its own
/// usage, every meta and second-layer key unbound.
pub fn default_key_mappings(layer: MappingLayer) -> Vec<KeyBinding> {
    layer
        .keys()
        .into_iter()
        .map(|hid| {
            let action = if layer == MappingLayer::Base {
                KeyAction::Key(hid)
            } else {
                KeyAction::Unbound
            };
            let (function, codes) = action.to_bytes();
            (hid, function, codes)
        })
        .collect()
}

/// `changes` applied onto `base`, every other key kept.
///
/// # Errors
///
/// Returns an error for a key that is not in `base` (not a key of the layer).
pub fn merge_key_mappings(layer: MappingLayer, base: &[KeyBinding], changes: &[KeyBinding]) -> Result<Vec<KeyBinding>> {
    let mut merged = base.to_vec();
    for &change in changes {
        let entry = merged.iter_mut().find(|entry| entry.0 == change.0).ok_or_else(|| {
            Error::InvalidConfig(format!("HID usage {:#04x} is not a key of the {layer} layer", change.0))
        })?;
        *entry = change;
    }
    Ok(merged)
}

fn build_feature(command: impl crate::devices::hid_reports::HidCommand) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; PROFILE_CHUNK_REPORT_SIZE];
    let size = HidReportBuilder::new(HidDeviceType::Keyboard).build_report(command, &mut buf)?;
    buf.truncate(size);
    Ok(buf)
}

/// The `36` feature reports that write a whole layer, report-ID byte first, at most 106 keys each.
///
/// # Errors
///
/// Returns an error unless `entries` lists every key of the layer once, in HID order.
pub fn key_mapping_reports(layer: MappingLayer, entries: &[KeyBinding]) -> Result<Vec<Vec<u8>>> {
    if entries.iter().map(|entry| entry.0).ne(layer.keys()) {
        return Err(Error::InvalidConfig(format!(
            "a {layer}-layer write must carry every key of the layer in HID order"
        )));
    }
    entries
        .chunks(KeyMappingCommand::MAX_KEYS)
        .map(|chunk| {
            build_feature(KeyMappingCommand {
                layer: layer as u8,
                entries: chunk,
            })
        })
        .collect()
}

/// The `B6` feature report asking for every binding of a layer. One report covers a layer of this board (at most
/// 91 keys); a layer over 106 keys would need chunked reads and is refused by the command's validation.
///
/// # Errors
///
/// Returns an error if the request does not fit one report.
pub fn key_mapping_read_request(layer: MappingLayer) -> Result<Vec<u8>> {
    build_feature(KeyMappingReadCommand {
        layer: layer as u8,
        keys: &layer.keys(),
    })
}

/// The bindings in a `B6` reply (report-ID byte first, as hidapi's feature read returns it).
///
/// # Errors
///
/// Returns an error if the reply is short or does not echo `keys` in order.
pub fn parse_key_mapping_reply(reply: &[u8], keys: &[u8]) -> Result<Vec<KeyBinding>> {
    let head = KeyMappingReadCommand::REPLY_HEADER_LEN;
    let body = reply.get(head..head + keys.len() * 6).ok_or_else(|| {
        Error::DeviceCommunication(format!(
            "key-mapping reply of {} bytes, needs {}",
            reply.len(),
            head + keys.len() * 6
        ))
    })?;
    let (entries, _) = body.as_chunks::<6>();
    let bindings: Vec<KeyBinding> = entries.iter().map(|e| (e[0], e[1], [e[2], e[3], e[4], e[5]])).collect();
    if bindings.iter().map(|b| b.0).ne(keys.iter().copied()) {
        return Err(Error::DeviceCommunication(format!(
            "key-mapping reply does not echo the requested keys in order (header {:02x?})",
            &reply[..head]
        )));
    }
    Ok(bindings)
}

/// The macro chunk requests of a full read, in order: 16 chunks of 150 events.
pub fn macro_read_requests() -> Vec<MacroReadCommand> {
    let chunks =
        MacroReadCommand::DATA_LEN / (usize::from(MacroReadCommand::CHUNK_EVENTS) * MacroReadCommand::EVENT_LEN);
    (0..chunks as u16)
        .map(|i| MacroReadCommand {
            offset_events: i * MacroReadCommand::CHUNK_EVENTS,
            len_events: MacroReadCommand::CHUNK_EVENTS,
        })
        .collect()
}

/// The 9600 bytes of macro data behind the 7-byte header of each chunk reply.
///
/// # Errors
///
/// Returns an error if a reply is short or the chunks do not add up to the full data.
pub fn macro_data(replies: &[Vec<u8>]) -> Result<Vec<u8>> {
    let head = MacroReadCommand::REPLY_HEADER_LEN;
    let chunk = usize::from(MacroReadCommand::CHUNK_EVENTS) * MacroReadCommand::EVENT_LEN;
    let mut data = Vec::with_capacity(MacroReadCommand::DATA_LEN);
    for (i, reply) in replies.iter().enumerate() {
        data.extend_from_slice(
            reply.get(head..head + chunk).ok_or_else(|| {
                Error::DeviceCommunication(format!("macro chunk {i}: reply of {} bytes", reply.len()))
            })?,
        );
    }
    if data.len() != MacroReadCommand::DATA_LEN {
        return Err(Error::DeviceCommunication(format!(
            "macro read collected {} bytes, expected {}",
            data.len(),
            MacroReadCommand::DATA_LEN
        )));
    }
    Ok(data)
}

/// `(error byte, id)` of a layout or region answer `[00] <F2|F5> <err> <id>`; hidapi may or may not keep the
/// report-ID byte.
pub fn parse_device_query(reply: &[u8], query: DeviceQueryCommand) -> Option<(u8, u8)> {
    let wire = query.wire_byte();
    let at = match reply {
        [0, command, ..] if *command == wire => 1,
        [command, ..] if *command == wire => 0,
        _ => return None,
    };
    Some((*reply.get(at + 1)?, *reply.get(at + 2)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_action() -> Vec<KeyAction> {
        vec![
            KeyAction::Unbound,
            KeyAction::Mouse(1),
            KeyAction::Mouse(8),
            KeyAction::WheelUp,
            KeyAction::WheelDown,
            KeyAction::PanLeft,
            KeyAction::PanRight,
            KeyAction::Key(0x04),
            KeyAction::Key(0xF0),
            KeyAction::Consumer(0xCD),
            KeyAction::Meta,
            KeyAction::Cpi,
            KeyAction::Macro(12),
            KeyAction::External(0x7F),
            KeyAction::Raw(0x99, [1, 2, 3, 4]),
            KeyAction::Raw(0x51, [0x04, 0x02, 0, 0]),
        ]
    }

    #[test]
    fn every_action_round_trips_through_text_and_bytes() {
        for action in every_action() {
            assert_eq!(KeyAction::parse(&action.to_string()).unwrap(), action, "{action}");
            let (function, codes) = action.to_bytes();
            assert_eq!(KeyAction::from_bytes(function, codes), action, "{action}");
        }
    }

    #[test]
    fn spec_function_bytes_and_raw_forms() {
        assert_eq!(
            KeyAction::parse("key:0x04").unwrap().to_bytes(),
            (0x51, [0x04, 0, 0, 0])
        );
        assert_eq!(
            KeyAction::parse("consumer:cd").unwrap().to_bytes(),
            (0x61, [0xCD, 0, 0, 0])
        );
        assert_eq!(KeyAction::parse("mouse:3").unwrap().to_bytes(), (0x03, [0; 4]));
        assert_eq!(KeyAction::parse("cpi").unwrap().to_bytes(), (0x30, [0; 4]));
        assert_eq!(KeyAction::parse("raw:51:04,00,00,00").unwrap(), KeyAction::Key(0x04));
        assert_eq!(
            KeyAction::from_bytes(0x00, [0, 0, 0, 1]),
            KeyAction::Raw(0x00, [0, 0, 0, 1])
        );
        for bad in [
            "key:A",
            "mouse:0",
            "mouse:9",
            "wheel:left",
            "raw:51:04,00",
            "unbound:1",
            "",
            "macro:x",
        ] {
            assert!(KeyAction::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn layers_hold_ggs_91_mapping_and_68_analog_keys() {
        assert_eq!(MappingLayer::Base.keys(), MAPPING_KEYS.to_vec());
        assert_eq!(MappingLayer::Meta.keys().len(), 91);
        let mut analog = crate::devices::keyboards::apex_pro_tkl_2023::WIRED_2023_PROFILE_SLOTS.to_vec();
        analog.sort_unstable();
        assert_eq!(MappingLayer::Second.keys(), analog);
        assert!(MAPPING_KEYS.windows(2).all(|w| w[0] < w[1]));
        for ignored in [245, 246, 247, 241, 254, 255] {
            assert!(!MAPPING_KEYS.contains(&ignored));
        }
    }

    #[test]
    fn reset_write_is_identity_on_base_and_unbound_elsewhere() {
        let base = key_mapping_reports(MappingLayer::Base, &default_key_mappings(MappingLayer::Base)).unwrap();
        assert_eq!(base.len(), 1);
        assert_eq!(base[0].len(), 645);
        assert_eq!(&base[0][..10], &[0x00, 0x36, 0x00, 91, 0x04, 0x51, 0x04, 0, 0, 0]);
        let last = &base[0][4 + 90 * 6..4 + 91 * 6];
        assert_eq!(last, &[0xF0, 0x51, 0xF0, 0, 0, 0]);
        assert!(base[0][4 + 91 * 6..].iter().all(|&b| b == 0));

        let second = key_mapping_reports(MappingLayer::Second, &default_key_mappings(MappingLayer::Second)).unwrap();
        assert_eq!(&second[0][..6], &[0x00, 0x36, 0x02, 68, 0x04, 0x00]);
        assert!(key_mapping_reports(MappingLayer::Second, &default_key_mappings(MappingLayer::Base)).is_err());
    }

    #[test]
    fn set_merges_named_keys_and_refuses_keys_outside_the_layer() {
        let base = default_key_mappings(MappingLayer::Base);
        let merged = merge_key_mappings(MappingLayer::Base, &base, &[(0x39, 0x51, [0xE0, 0, 0, 0])]).unwrap();
        assert_eq!(merged.len(), 91);
        assert_eq!(
            merged.iter().find(|e| e.0 == 0x39),
            Some(&(0x39, 0x51, [0xE0, 0, 0, 0]))
        );
        assert_eq!(merged.iter().zip(&base).filter(|(m, b)| m != b).count(), 1);
        // 0x29 (Escape) has no analog switch, 0x48 is not on the board.
        let second = default_key_mappings(MappingLayer::Second);
        assert!(merge_key_mappings(MappingLayer::Second, &second, &[(0x29, 0, [0; 4])]).is_err());
        assert!(merge_key_mappings(MappingLayer::Base, &base, &[(0x48, 0, [0; 4])]).is_err());
    }

    #[test]
    fn read_request_and_reply_parse() {
        let request = key_mapping_read_request(MappingLayer::Meta).unwrap();
        assert_eq!(&request[..11], &[0x00, 0xB6, 0x01, 91, 0x04, 0, 0, 0, 0, 0, 0x05]);

        let keys = MappingLayer::Second.keys();
        let mut reply = vec![0x00, 0xB6, 0x02, 68, 0x00];
        for &hid in &keys {
            reply.extend([hid, 0x51, hid, 0, 0, 0]);
        }
        reply.resize(645, 0);
        let parsed = parse_key_mapping_reply(&reply, &keys).unwrap();
        assert_eq!(parsed.len(), 68);
        assert_eq!(parsed[0], (0x04, 0x51, [0x04, 0, 0, 0]));
        reply[5] = 0x05;
        assert!(parse_key_mapping_reply(&reply, &keys).is_err());
        assert!(parse_key_mapping_reply(&reply[..100], &keys).is_err());
    }

    #[test]
    fn macro_read_covers_9600_bytes_in_16_chunks() {
        let requests = macro_read_requests();
        assert_eq!(requests.len(), 16);
        assert_eq!(requests[15].offset_events, 2250);
        let replies: Vec<Vec<u8>> = (0..16u8)
            .map(|i| {
                let mut reply = vec![0xEE; 7];
                reply.extend(std::iter::repeat_n(i, 600));
                reply.resize(645, 0);
                reply
            })
            .collect();
        let data = macro_data(&replies).unwrap();
        assert_eq!(data.len(), 9600);
        assert_eq!((data[0], data[599], data[600], data[9599]), (0, 0, 1, 15));
        assert!(macro_data(&replies[..15]).is_err());
    }

    #[test]
    fn device_query_answers_are_found_with_or_without_the_report_id() {
        assert_eq!(
            parse_device_query(&[0xF2, 0, 1], DeviceQueryCommand::Layout),
            Some((0, 1))
        );
        assert_eq!(
            parse_device_query(&[0x00, 0xF5, 0, 3, 0], DeviceQueryCommand::Region),
            Some((0, 3))
        );
        assert_eq!(parse_device_query(&[0xF5, 0, 3], DeviceQueryCommand::Layout), None);
        assert_eq!(parse_device_query(&[0xF2, 1], DeviceQueryCommand::Layout), None);
    }
}
