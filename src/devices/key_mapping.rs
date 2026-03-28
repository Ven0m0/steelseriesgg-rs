//! Key-to-address mapping for SteelSeries keyboards.
//!
//! This module provides accurate key matrix mapping for per-key RGB control,
//! supporting different Apex Pro variants and layouts.

use crate::devices::product_ids;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Physical key identifier for SteelSeries keyboards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyId {
    // Function row
    Escape,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,

    // Number row
    Backtick,
    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,
    Key0,
    Minus,
    Equal,
    Backspace,

    // First letter row
    Tab,
    Q,
    W,
    E,
    R,
    T,
    Y,
    U,
    I,
    O,
    P,
    LeftBracket,
    RightBracket,
    Backslash,

    // Second letter row (home row)
    CapsLock,
    A,
    S,
    D,
    F,
    G,
    H,
    J,
    K,
    L,
    Semicolon,
    Quote,
    Enter,

    // Third letter row
    LeftShift,
    Z,
    X,
    C,
    V,
    B,
    N,
    M,
    Comma,
    Period,
    Slash,
    RightShift,

    // Bottom row
    LeftCtrl,
    LeftWin,
    LeftAlt,
    Space,
    RightAlt,
    RightWin,
    Menu,
    RightCtrl,

    // Arrow cluster
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,

    // Navigation cluster (full-size only)
    Insert,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,

    // Number pad (full-size only)
    NumLock,
    NumSlash,
    NumAsterisk,
    NumMinus,
    Num7,
    Num8,
    Num9,
    NumPlus,
    Num4,
    Num5,
    Num6,
    Num1,
    Num2,
    Num3,
    NumEnter,
    Num0,
    NumPeriod,

    // SteelSeries specific keys
    SteelSeriesKey, // The SteelSeries logo key
    VolumeWheel,    // Volume wheel (if applicable)
}

impl fmt::Display for KeyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Function row
            KeyId::Escape => write!(f, "ESC"),
            KeyId::F1 => write!(f, "F1"),
            KeyId::F2 => write!(f, "F2"),
            KeyId::F3 => write!(f, "F3"),
            KeyId::F4 => write!(f, "F4"),
            KeyId::F5 => write!(f, "F5"),
            KeyId::F6 => write!(f, "F6"),
            KeyId::F7 => write!(f, "F7"),
            KeyId::F8 => write!(f, "F8"),
            KeyId::F9 => write!(f, "F9"),
            KeyId::F10 => write!(f, "F10"),
            KeyId::F11 => write!(f, "F11"),
            KeyId::F12 => write!(f, "F12"),

            // Number row
            KeyId::Backtick => write!(f, "`"),
            KeyId::Key1 => write!(f, "1"),
            KeyId::Key2 => write!(f, "2"),
            KeyId::Key3 => write!(f, "3"),
            KeyId::Key4 => write!(f, "4"),
            KeyId::Key5 => write!(f, "5"),
            KeyId::Key6 => write!(f, "6"),
            KeyId::Key7 => write!(f, "7"),
            KeyId::Key8 => write!(f, "8"),
            KeyId::Key9 => write!(f, "9"),
            KeyId::Key0 => write!(f, "0"),
            KeyId::Minus => write!(f, "-"),
            KeyId::Equal => write!(f, "="),
            KeyId::Backspace => write!(f, "BKSP"),

            // Letter rows
            KeyId::Tab => write!(f, "TAB"),
            KeyId::Q => write!(f, "Q"),
            KeyId::W => write!(f, "W"),
            KeyId::E => write!(f, "E"),
            KeyId::R => write!(f, "R"),
            KeyId::T => write!(f, "T"),
            KeyId::Y => write!(f, "Y"),
            KeyId::U => write!(f, "U"),
            KeyId::I => write!(f, "I"),
            KeyId::O => write!(f, "O"),
            KeyId::P => write!(f, "P"),
            KeyId::LeftBracket => write!(f, "["),
            KeyId::RightBracket => write!(f, "]"),
            KeyId::Backslash => write!(f, "\\"),

            KeyId::CapsLock => write!(f, "CAPS"),
            KeyId::A => write!(f, "A"),
            KeyId::S => write!(f, "S"),
            KeyId::D => write!(f, "D"),
            KeyId::F => write!(f, "F"),
            KeyId::G => write!(f, "G"),
            KeyId::H => write!(f, "H"),
            KeyId::J => write!(f, "J"),
            KeyId::K => write!(f, "K"),
            KeyId::L => write!(f, "L"),
            KeyId::Semicolon => write!(f, ";"),
            KeyId::Quote => write!(f, "'"),
            KeyId::Enter => write!(f, "ENTER"),

            KeyId::LeftShift => write!(f, "LSHIFT"),
            KeyId::Z => write!(f, "Z"),
            KeyId::X => write!(f, "X"),
            KeyId::C => write!(f, "C"),
            KeyId::V => write!(f, "V"),
            KeyId::B => write!(f, "B"),
            KeyId::N => write!(f, "N"),
            KeyId::M => write!(f, "M"),
            KeyId::Comma => write!(f, ","),
            KeyId::Period => write!(f, "."),
            KeyId::Slash => write!(f, "/"),
            KeyId::RightShift => write!(f, "RSHIFT"),

            // Bottom row
            KeyId::LeftCtrl => write!(f, "LCTRL"),
            KeyId::LeftWin => write!(f, "LWIN"),
            KeyId::LeftAlt => write!(f, "LALT"),
            KeyId::Space => write!(f, "SPACE"),
            KeyId::RightAlt => write!(f, "RALT"),
            KeyId::RightWin => write!(f, "RWIN"),
            KeyId::Menu => write!(f, "MENU"),
            KeyId::RightCtrl => write!(f, "RCTRL"),

            // Arrows
            KeyId::ArrowUp => write!(f, "UP"),
            KeyId::ArrowDown => write!(f, "DOWN"),
            KeyId::ArrowLeft => write!(f, "LEFT"),
            KeyId::ArrowRight => write!(f, "RIGHT"),

            // Navigation
            KeyId::Insert => write!(f, "INS"),
            KeyId::Delete => write!(f, "DEL"),
            KeyId::Home => write!(f, "HOME"),
            KeyId::End => write!(f, "END"),
            KeyId::PageUp => write!(f, "PGUP"),
            KeyId::PageDown => write!(f, "PGDN"),

            // Numpad
            KeyId::NumLock => write!(f, "NUMLK"),
            KeyId::NumSlash => write!(f, "NUM/"),
            KeyId::NumAsterisk => write!(f, "NUM*"),
            KeyId::NumMinus => write!(f, "NUM-"),
            KeyId::Num7 => write!(f, "NUM7"),
            KeyId::Num8 => write!(f, "NUM8"),
            KeyId::Num9 => write!(f, "NUM9"),
            KeyId::NumPlus => write!(f, "NUM+"),
            KeyId::Num4 => write!(f, "NUM4"),
            KeyId::Num5 => write!(f, "NUM5"),
            KeyId::Num6 => write!(f, "NUM6"),
            KeyId::Num1 => write!(f, "NUM1"),
            KeyId::Num2 => write!(f, "NUM2"),
            KeyId::Num3 => write!(f, "NUM3"),
            KeyId::NumEnter => write!(f, "NUMENTER"),
            KeyId::Num0 => write!(f, "NUM0"),
            KeyId::NumPeriod => write!(f, "NUM."),

            // Special
            KeyId::SteelSeriesKey => write!(f, "SS"),
            KeyId::VolumeWheel => write!(f, "VOL"),
        }
    }
}

/// HID address for a specific key (USB HID Usage ID).
///
/// SteelSeries keyboards use standard USB HID keycodes (Usage IDs) for per-key
/// addressing, NOT matrix row/column coordinates. This was discovered through
/// reverse engineering of SteelSeries GG 107.0.0.
///
/// Standard USB HID keycodes:
/// - 4-29: A-Z (4=A, 5=B, ..., 29=Z)
/// - 30-39: 1-0 (30=1, ..., 39=0)
/// - 40: Enter, 41: Escape, 42: Backspace, 43: Tab, 44: Space
/// - 45-55: - = [ ] \ ; ' ` , . /
/// - 56: Caps Lock, 57-68: F1-F12
/// - 69-77: Print/Scroll/Pause/Insert/Home/PgUp/Del/End/PgDn
/// - 78-81: Arrow keys (Right/Left/Down/Up)
/// - 82-97: Numpad keys
/// - 100: Menu, 133: Power
/// - 224-231: Left/Right Ctrl/Shift/Alt/GUI
/// - 240: SteelSeries logo key
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyAddress {
    /// USB HID Usage ID (keycode)
    pub hid_code: u8,
}

impl KeyAddress {
    /// Create a new key address from HID code.
    pub fn new(hid_code: u8) -> Self {
        Self { hid_code }
    }

    /// Create a new key address from HID code (alias for clarity).
    pub fn from_hid(hid_code: u8) -> Self {
        Self { hid_code }
    }
}

impl fmt::Display for KeyAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HID 0x{:02X}", self.hid_code)
    }
}

/// Keyboard layout variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyboardLayout {
    /// Full-size keyboard (104 keys)
    FullSize,
    /// Tenkeyless (87 keys)
    TenKeyLess,
    /// Compact layout
    Compact,
}

/// Complete key mapping for a specific keyboard model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMapping {
    /// Product ID this mapping applies to
    pub product_id: u16,
    /// Keyboard layout type
    pub layout: KeyboardLayout,
    /// Human-readable name
    pub name: String,
    /// Key ID to HID address mapping
    pub key_map: HashMap<KeyId, KeyAddress>,
    /// Cached list of keys for stable iteration without allocation
    #[serde(skip)]
    pub cached_keys: Vec<KeyId>,
    /// Total number of individually addressable keys
    pub total_keys: usize,
    /// List of supported HID codes for this keyboard
    pub supported_hid_codes: Vec<u8>,
}

impl KeyMapping {
    /// Create a new key mapping.
    pub fn new(product_id: u16, layout: KeyboardLayout, name: String, supported_hid_codes: Vec<u8>) -> Self {
        Self {
            product_id,
            layout,
            name,
            key_map: HashMap::new(),
            cached_keys: Vec::new(),
            total_keys: 0,
            supported_hid_codes,
        }
    }

    /// Add a key mapping.
    pub fn add_key(&mut self, key_id: KeyId, address: KeyAddress) {
        if self.key_map.insert(key_id, address).is_none() {
            self.cached_keys.push(key_id);
        }
        self.total_keys = self.key_map.len();
    }

    /// Get the address for a specific key.
    pub fn get_key_address(&self, key_id: KeyId) -> Option<KeyAddress> {
        self.key_map.get(&key_id).copied()
    }

    /// Get all keys in this mapping.
    pub fn get_all_keys(&self) -> &[KeyId] {
        &self.cached_keys
    }

    /// Check if a key is supported.
    pub fn supports_key(&self, key_id: KeyId) -> bool {
        self.key_map.contains_key(&key_id)
    }

    /// Get mapping statistics.
    pub fn get_stats(&self) -> KeyMappingStats {
        KeyMappingStats {
            total_keys: self.total_keys,
            supported_hid_codes: self.supported_hid_codes.len(),
            mapped_keys: self.key_map.len(),
            utilization: (self.key_map.len() as f32 / self.supported_hid_codes.len().max(1) as f32) * 100.0,
        }
    }
}

/// Key mapping statistics.
#[derive(Debug, Clone)]
pub struct KeyMappingStats {
    pub total_keys: usize,
    pub supported_hid_codes: usize,
    pub mapped_keys: usize,
    pub utilization: f32,
}

impl fmt::Display for KeyMappingStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Keys: {}/{} mapped ({:.1}% utilization)",
            self.mapped_keys, self.supported_hid_codes, self.utilization
        )
    }
}

/// Key mapping database for all supported keyboards.
pub struct KeyMappingDatabase {
    mappings: HashMap<u16, KeyMapping>,
}

impl KeyMappingDatabase {
    /// Create a new key mapping database.
    pub fn new() -> Self {
        let mut db = Self {
            mappings: HashMap::new(),
        };
        db.initialize_known_mappings();
        db
    }

    /// Initialize mappings for known keyboard models.
    fn initialize_known_mappings(&mut self) {
        // NOTE: These mappings are based on reverse engineering of SteelSeries GG 107.0.0
        // The keycodes are standard USB HID Usage IDs discovered from configuration migration files.
        //
        // Source: apex_7+pro.migration, apex_pro_tkl_2022.migration
        // Complete TKL keycode list: (4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31 32 33 34 35 36 37 38 39 40 41 42 43 44 45 46 47 48 49 50 51 52 53 54 55 56 57 58 59 60 61 62 63 64 65 66 67 68 69 73 74 75 76 77 78 79 80 81 82 100 133 135 136 137 138 139 224 225 226 227 228 229 230 231 240)

        // Apex Pro TKL (2023) - Verified HID codes from SteelSeries GG
        self.add_apex_pro_tkl_2023_mapping();

        // Apex Pro (full) - Verified HID codes from SteelSeries GG
        self.add_apex_pro_full_mapping();

        // Apex Pro TKL (original) - Verified HID codes from SteelSeries GG
        self.add_apex_pro_tkl_mapping();
    }

    /// Add Apex Pro TKL 2023 key mapping with verified HID codes.
    ///
    /// HID codes discovered from SteelSeries GG 107.0.0 configuration migration files.
    /// Complete TKL keycode list: 87 keys total
    fn add_apex_pro_tkl_2023_mapping(&mut self) {
        // Complete TKL HID code list from apex_7+pro.migration
        let tkl_hid_codes = vec![
            4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
            32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58,
            59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 100, 133, 135, 136,
            137, 138, 139, 224, 225, 226, 227, 228, 229, 230, 231, 240,
        ];

        let mut mapping = KeyMapping::new(
            product_ids::APEX_PRO_TKL_2023,
            KeyboardLayout::TenKeyLess,
            "Apex Pro TKL (2023)".to_string(),
            tkl_hid_codes,
        );

        // Function row (F1-F12 = 58-69, Escape = 41)
        mapping.add_key(KeyId::Escape, KeyAddress::new(41));
        mapping.add_key(KeyId::F1, KeyAddress::new(58));
        mapping.add_key(KeyId::F2, KeyAddress::new(59));
        mapping.add_key(KeyId::F3, KeyAddress::new(60));
        mapping.add_key(KeyId::F4, KeyAddress::new(61));
        mapping.add_key(KeyId::F5, KeyAddress::new(62));
        mapping.add_key(KeyId::F6, KeyAddress::new(63));
        mapping.add_key(KeyId::F7, KeyAddress::new(64));
        mapping.add_key(KeyId::F8, KeyAddress::new(65));
        mapping.add_key(KeyId::F9, KeyAddress::new(66));
        mapping.add_key(KeyId::F10, KeyAddress::new(67));
        mapping.add_key(KeyId::F11, KeyAddress::new(68));
        mapping.add_key(KeyId::F12, KeyAddress::new(69));

        // Number row (1-0 = 30-39, Backtick = 53, Minus = 45, Equal = 46, Backspace = 42)
        mapping.add_key(KeyId::Backtick, KeyAddress::new(53));
        mapping.add_key(KeyId::Key1, KeyAddress::new(30));
        mapping.add_key(KeyId::Key2, KeyAddress::new(31));
        mapping.add_key(KeyId::Key3, KeyAddress::new(32));
        mapping.add_key(KeyId::Key4, KeyAddress::new(33));
        mapping.add_key(KeyId::Key5, KeyAddress::new(34));
        mapping.add_key(KeyId::Key6, KeyAddress::new(35));
        mapping.add_key(KeyId::Key7, KeyAddress::new(36));
        mapping.add_key(KeyId::Key8, KeyAddress::new(37));
        mapping.add_key(KeyId::Key9, KeyAddress::new(38));
        mapping.add_key(KeyId::Key0, KeyAddress::new(39));
        mapping.add_key(KeyId::Minus, KeyAddress::new(45));
        mapping.add_key(KeyId::Equal, KeyAddress::new(46));
        mapping.add_key(KeyId::Backspace, KeyAddress::new(42));

        // QWERTY row (Tab = 43, Q-P = 20-25, Brackets = 47-48, Backslash = 49)
        mapping.add_key(KeyId::Tab, KeyAddress::new(43));
        mapping.add_key(KeyId::Q, KeyAddress::new(20));
        mapping.add_key(KeyId::W, KeyAddress::new(26));
        mapping.add_key(KeyId::E, KeyAddress::new(8));
        mapping.add_key(KeyId::R, KeyAddress::new(21));
        mapping.add_key(KeyId::T, KeyAddress::new(23));
        mapping.add_key(KeyId::Y, KeyAddress::new(28));
        mapping.add_key(KeyId::U, KeyAddress::new(24));
        mapping.add_key(KeyId::I, KeyAddress::new(12));
        mapping.add_key(KeyId::O, KeyAddress::new(18));
        mapping.add_key(KeyId::P, KeyAddress::new(19));
        mapping.add_key(KeyId::LeftBracket, KeyAddress::new(47));
        mapping.add_key(KeyId::RightBracket, KeyAddress::new(48));
        mapping.add_key(KeyId::Backslash, KeyAddress::new(49));

        // Home row (CapsLock = 57, A-L = 4-15, Semicolon = 51, Quote = 52, Enter = 40)
        mapping.add_key(KeyId::CapsLock, KeyAddress::new(57));
        mapping.add_key(KeyId::A, KeyAddress::new(4));
        mapping.add_key(KeyId::S, KeyAddress::new(22));
        mapping.add_key(KeyId::D, KeyAddress::new(7));
        mapping.add_key(KeyId::F, KeyAddress::new(9));
        mapping.add_key(KeyId::G, KeyAddress::new(10));
        mapping.add_key(KeyId::H, KeyAddress::new(11));
        mapping.add_key(KeyId::J, KeyAddress::new(13));
        mapping.add_key(KeyId::K, KeyAddress::new(14));
        mapping.add_key(KeyId::L, KeyAddress::new(15));
        mapping.add_key(KeyId::Semicolon, KeyAddress::new(51));
        mapping.add_key(KeyId::Quote, KeyAddress::new(52));
        mapping.add_key(KeyId::Enter, KeyAddress::new(40));

        // Bottom letter row (LeftShift = 225, Z-M = 29-16, Comma = 54, Period = 55, Slash = 56, RightShift = 229)
        mapping.add_key(KeyId::LeftShift, KeyAddress::new(225));
        mapping.add_key(KeyId::Z, KeyAddress::new(29));
        mapping.add_key(KeyId::X, KeyAddress::new(27));
        mapping.add_key(KeyId::C, KeyAddress::new(6));
        mapping.add_key(KeyId::V, KeyAddress::new(25));
        mapping.add_key(KeyId::B, KeyAddress::new(5));
        mapping.add_key(KeyId::N, KeyAddress::new(17));
        mapping.add_key(KeyId::M, KeyAddress::new(16));
        mapping.add_key(KeyId::Comma, KeyAddress::new(54));
        mapping.add_key(KeyId::Period, KeyAddress::new(55));
        mapping.add_key(KeyId::Slash, KeyAddress::new(56));
        mapping.add_key(KeyId::RightShift, KeyAddress::new(229));

        // Bottom row (LeftCtrl = 224, LeftWin = 227, LeftAlt = 226, Space = 44, RightAlt = 230, RightWin = 231, Menu = 101, RightCtrl = 228)
        mapping.add_key(KeyId::LeftCtrl, KeyAddress::new(224));
        mapping.add_key(KeyId::LeftWin, KeyAddress::new(227));
        mapping.add_key(KeyId::LeftAlt, KeyAddress::new(226));
        mapping.add_key(KeyId::Space, KeyAddress::new(44));
        mapping.add_key(KeyId::RightAlt, KeyAddress::new(230));
        mapping.add_key(KeyId::RightWin, KeyAddress::new(231));
        mapping.add_key(KeyId::Menu, KeyAddress::new(101));
        mapping.add_key(KeyId::RightCtrl, KeyAddress::new(228));

        // Navigation cluster (Insert = 73, Home = 74, PageUp = 75, Delete = 76, End = 77, PageDown = 78)
        mapping.add_key(KeyId::Insert, KeyAddress::new(73));
        mapping.add_key(KeyId::Home, KeyAddress::new(74));
        mapping.add_key(KeyId::PageUp, KeyAddress::new(75));
        mapping.add_key(KeyId::Delete, KeyAddress::new(76));
        mapping.add_key(KeyId::End, KeyAddress::new(77));
        mapping.add_key(KeyId::PageDown, KeyAddress::new(78));

        // Arrow cluster (Right = 79, Left = 80, Down = 81, Up = 82)
        mapping.add_key(KeyId::ArrowRight, KeyAddress::new(79));
        mapping.add_key(KeyId::ArrowLeft, KeyAddress::new(80));
        mapping.add_key(KeyId::ArrowDown, KeyAddress::new(81));
        mapping.add_key(KeyId::ArrowUp, KeyAddress::new(82));

        // SteelSeries key (HID 240 - discovered from migration files)
        mapping.add_key(KeyId::SteelSeriesKey, KeyAddress::new(240));

        let mut wireless_mapping = mapping.clone();
        wireless_mapping.product_id = product_ids::APEX_PRO_TKL_2023_WIRELESS;
        self.mappings.insert(product_ids::APEX_PRO_TKL_2023, mapping);
        self.mappings.insert(product_ids::APEX_PRO_TKL_2023_WIRELESS, wireless_mapping);
    }

    /// Add Apex Pro (full-size) key mapping with verified HID codes.
    fn add_apex_pro_full_mapping(&mut self) {
        // Complete full-size HID code list from apex_7+pro.migration (104 keys)
        let full_hid_codes = vec![
            4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
            32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58,
            59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85,
            86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 133, 135, 136, 137, 138, 139, 224, 225, 226,
            227, 228, 229, 230, 231, 240,
        ];

        let mut mapping = KeyMapping::new(
            product_ids::APEX_PRO,
            KeyboardLayout::FullSize,
            "Apex Pro".to_string(),
            full_hid_codes,
        );

        // Copy all TKL keys first
        // Function row
        mapping.add_key(KeyId::Escape, KeyAddress::new(41));
        mapping.add_key(KeyId::F1, KeyAddress::new(58));
        mapping.add_key(KeyId::F2, KeyAddress::new(59));
        mapping.add_key(KeyId::F3, KeyAddress::new(60));
        mapping.add_key(KeyId::F4, KeyAddress::new(61));
        mapping.add_key(KeyId::F5, KeyAddress::new(62));
        mapping.add_key(KeyId::F6, KeyAddress::new(63));
        mapping.add_key(KeyId::F7, KeyAddress::new(64));
        mapping.add_key(KeyId::F8, KeyAddress::new(65));
        mapping.add_key(KeyId::F9, KeyAddress::new(66));
        mapping.add_key(KeyId::F10, KeyAddress::new(67));
        mapping.add_key(KeyId::F11, KeyAddress::new(68));
        mapping.add_key(KeyId::F12, KeyAddress::new(69));

        // Number row
        mapping.add_key(KeyId::Backtick, KeyAddress::new(53));
        mapping.add_key(KeyId::Key1, KeyAddress::new(30));
        mapping.add_key(KeyId::Key2, KeyAddress::new(31));
        mapping.add_key(KeyId::Key3, KeyAddress::new(32));
        mapping.add_key(KeyId::Key4, KeyAddress::new(33));
        mapping.add_key(KeyId::Key5, KeyAddress::new(34));
        mapping.add_key(KeyId::Key6, KeyAddress::new(35));
        mapping.add_key(KeyId::Key7, KeyAddress::new(36));
        mapping.add_key(KeyId::Key8, KeyAddress::new(37));
        mapping.add_key(KeyId::Key9, KeyAddress::new(38));
        mapping.add_key(KeyId::Key0, KeyAddress::new(39));
        mapping.add_key(KeyId::Minus, KeyAddress::new(45));
        mapping.add_key(KeyId::Equal, KeyAddress::new(46));
        mapping.add_key(KeyId::Backspace, KeyAddress::new(42));

        // QWERTY row
        mapping.add_key(KeyId::Tab, KeyAddress::new(43));
        mapping.add_key(KeyId::Q, KeyAddress::new(20));
        mapping.add_key(KeyId::W, KeyAddress::new(26));
        mapping.add_key(KeyId::E, KeyAddress::new(8));
        mapping.add_key(KeyId::R, KeyAddress::new(21));
        mapping.add_key(KeyId::T, KeyAddress::new(23));
        mapping.add_key(KeyId::Y, KeyAddress::new(28));
        mapping.add_key(KeyId::U, KeyAddress::new(24));
        mapping.add_key(KeyId::I, KeyAddress::new(12));
        mapping.add_key(KeyId::O, KeyAddress::new(18));
        mapping.add_key(KeyId::P, KeyAddress::new(19));
        mapping.add_key(KeyId::LeftBracket, KeyAddress::new(47));
        mapping.add_key(KeyId::RightBracket, KeyAddress::new(48));
        mapping.add_key(KeyId::Backslash, KeyAddress::new(49));

        // Home row
        mapping.add_key(KeyId::CapsLock, KeyAddress::new(57));
        mapping.add_key(KeyId::A, KeyAddress::new(4));
        mapping.add_key(KeyId::S, KeyAddress::new(22));
        mapping.add_key(KeyId::D, KeyAddress::new(7));
        mapping.add_key(KeyId::F, KeyAddress::new(9));
        mapping.add_key(KeyId::G, KeyAddress::new(10));
        mapping.add_key(KeyId::H, KeyAddress::new(11));
        mapping.add_key(KeyId::J, KeyAddress::new(13));
        mapping.add_key(KeyId::K, KeyAddress::new(14));
        mapping.add_key(KeyId::L, KeyAddress::new(15));
        mapping.add_key(KeyId::Semicolon, KeyAddress::new(51));
        mapping.add_key(KeyId::Quote, KeyAddress::new(52));
        mapping.add_key(KeyId::Enter, KeyAddress::new(40));

        // Bottom letter row
        mapping.add_key(KeyId::LeftShift, KeyAddress::new(225));
        mapping.add_key(KeyId::Z, KeyAddress::new(29));
        mapping.add_key(KeyId::X, KeyAddress::new(27));
        mapping.add_key(KeyId::C, KeyAddress::new(6));
        mapping.add_key(KeyId::V, KeyAddress::new(25));
        mapping.add_key(KeyId::B, KeyAddress::new(5));
        mapping.add_key(KeyId::N, KeyAddress::new(17));
        mapping.add_key(KeyId::M, KeyAddress::new(16));
        mapping.add_key(KeyId::Comma, KeyAddress::new(54));
        mapping.add_key(KeyId::Period, KeyAddress::new(55));
        mapping.add_key(KeyId::Slash, KeyAddress::new(56));
        mapping.add_key(KeyId::RightShift, KeyAddress::new(229));

        // Bottom row
        mapping.add_key(KeyId::LeftCtrl, KeyAddress::new(224));
        mapping.add_key(KeyId::LeftWin, KeyAddress::new(227));
        mapping.add_key(KeyId::LeftAlt, KeyAddress::new(226));
        mapping.add_key(KeyId::Space, KeyAddress::new(44));
        mapping.add_key(KeyId::RightAlt, KeyAddress::new(230));
        mapping.add_key(KeyId::RightWin, KeyAddress::new(231));
        mapping.add_key(KeyId::Menu, KeyAddress::new(101));
        mapping.add_key(KeyId::RightCtrl, KeyAddress::new(228));

        // Navigation cluster
        mapping.add_key(KeyId::Insert, KeyAddress::new(73));
        mapping.add_key(KeyId::Home, KeyAddress::new(74));
        mapping.add_key(KeyId::PageUp, KeyAddress::new(75));
        mapping.add_key(KeyId::Delete, KeyAddress::new(76));
        mapping.add_key(KeyId::End, KeyAddress::new(77));
        mapping.add_key(KeyId::PageDown, KeyAddress::new(78));

        // Arrow cluster
        mapping.add_key(KeyId::ArrowRight, KeyAddress::new(79));
        mapping.add_key(KeyId::ArrowLeft, KeyAddress::new(80));
        mapping.add_key(KeyId::ArrowDown, KeyAddress::new(81));
        mapping.add_key(KeyId::ArrowUp, KeyAddress::new(82));

        // Numpad (full-size only)
        mapping.add_key(KeyId::NumLock, KeyAddress::new(83));
        mapping.add_key(KeyId::NumSlash, KeyAddress::new(84));
        mapping.add_key(KeyId::NumAsterisk, KeyAddress::new(85));
        mapping.add_key(KeyId::NumMinus, KeyAddress::new(86));
        mapping.add_key(KeyId::Num7, KeyAddress::new(95));
        mapping.add_key(KeyId::Num8, KeyAddress::new(96));
        mapping.add_key(KeyId::Num9, KeyAddress::new(97));
        mapping.add_key(KeyId::NumPlus, KeyAddress::new(87));
        mapping.add_key(KeyId::Num4, KeyAddress::new(92));
        mapping.add_key(KeyId::Num5, KeyAddress::new(93));
        mapping.add_key(KeyId::Num6, KeyAddress::new(94));
        mapping.add_key(KeyId::Num1, KeyAddress::new(89));
        mapping.add_key(KeyId::Num2, KeyAddress::new(90));
        mapping.add_key(KeyId::Num3, KeyAddress::new(91));
        mapping.add_key(KeyId::NumEnter, KeyAddress::new(88));
        mapping.add_key(KeyId::Num0, KeyAddress::new(98));
        mapping.add_key(KeyId::NumPeriod, KeyAddress::new(99));

        // SteelSeries key
        mapping.add_key(KeyId::SteelSeriesKey, KeyAddress::new(240));

        self.mappings.insert(product_ids::APEX_PRO, mapping);
    }

    /// Add Apex Pro TKL (original) key mapping with verified HID codes.
    fn add_apex_pro_tkl_mapping(&mut self) {
        // Original Apex Pro TKL uses same HID codes as 2023 model
        let tkl_hid_codes = vec![
            4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
            32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58,
            59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 100, 133, 135, 136,
            137, 138, 139, 224, 225, 226, 227, 228, 229, 230, 231, 240,
        ];

        let mapping = KeyMapping::new(
            product_ids::APEX_PRO_TKL,
            KeyboardLayout::TenKeyLess,
            "Apex Pro TKL".to_string(),
            tkl_hid_codes,
        );

        // Uses same keycodes as TKL 2023 - populated in a future update
        // For now, create the mapping structure with supported HID codes
        self.mappings.insert(product_ids::APEX_PRO_TKL, mapping);
    }

    /// Get key mapping for a specific product ID.
    pub fn get_mapping(&self, product_id: u16) -> Option<&KeyMapping> {
        self.mappings.get(&product_id)
    }

    /// Get all supported product IDs.
    pub fn get_supported_products(&self) -> Vec<u16> {
        self.mappings.keys().copied().collect()
    }

    /// Check if a product ID is supported.
    pub fn supports_product(&self, product_id: u16) -> bool {
        self.mappings.contains_key(&product_id)
    }
}

impl Default for KeyMappingDatabase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_address_creation() {
        let addr = KeyAddress::new(0x04); // HID code for 'A'
        assert_eq!(addr.hid_code, 0x04);
        assert_eq!(addr.to_string(), "HID 0x04");

        // Test from_hid alias
        let addr2 = KeyAddress::from_hid(0x1E); // HID code for '1'
        assert_eq!(addr2.hid_code, 0x1E);
    }

    #[test]
    fn test_key_mapping_creation() {
        let supported_codes = vec![4, 5, 6, 7, 8]; // A, B, C, D, E
        let mut mapping = KeyMapping::new(
            product_ids::APEX_PRO_TKL_2023,
            KeyboardLayout::TenKeyLess,
            "Test Keyboard".to_string(),
            supported_codes,
        );

        // Initially empty
        assert_eq!(mapping.total_keys, 0);
        assert!(!mapping.supports_key(KeyId::A));

        // Add a key
        mapping.add_key(KeyId::A, KeyAddress::new(4)); // HID 0x04 = A
        assert_eq!(mapping.total_keys, 1);
        assert!(mapping.supports_key(KeyId::A));
        assert_eq!(mapping.get_key_address(KeyId::A), Some(KeyAddress::new(4)));
        assert_eq!(mapping.get_key_address(KeyId::B), None);
    }

    #[test]
    fn test_key_mapping_database() {
        let db = KeyMappingDatabase::new();

        // Should support known products
        assert!(db.supports_product(product_ids::APEX_PRO_TKL_2023));
        assert!(db.supports_product(product_ids::APEX_PRO));
        assert!(!db.supports_product(0xFFFF)); // Unknown product

        // Should have mappings
        let mapping = db.get_mapping(product_ids::APEX_PRO_TKL_2023);
        assert!(mapping.is_some());

        let mapping = mapping.unwrap();
        assert_eq!(mapping.layout, KeyboardLayout::TenKeyLess);
        assert!(mapping.total_keys > 0);

        // Verify specific HID codes are mapped
        let a_key = mapping.get_key_address(KeyId::A);
        assert!(a_key.is_some());
        assert_eq!(a_key.unwrap().hid_code, 4); // HID 0x04 = A
    }

    #[test]
    fn test_key_mapping_stats() {
        let supported_codes = vec![4, 5, 6, 7, 8, 9, 10]; // A-G
        let mut mapping = KeyMapping::new(
            product_ids::APEX_PRO_TKL_2023,
            KeyboardLayout::TenKeyLess,
            "Test".to_string(),
            supported_codes,
        );

        // Add some keys
        mapping.add_key(KeyId::A, KeyAddress::new(4));
        mapping.add_key(KeyId::B, KeyAddress::new(5));
        mapping.add_key(KeyId::C, KeyAddress::new(6));

        let stats = mapping.get_stats();
        assert_eq!(stats.total_keys, 3);
        assert_eq!(stats.supported_hid_codes, 7);
        assert_eq!(stats.mapped_keys, 3);
        assert!(stats.utilization > 0.0);
        assert!(stats.utilization <= 100.0);
    }

    #[test]
    fn test_key_id_display() {
        assert_eq!(KeyId::A.to_string(), "A");
        assert_eq!(KeyId::Escape.to_string(), "ESC");
        assert_eq!(KeyId::LeftShift.to_string(), "LSHIFT");
        assert_eq!(KeyId::Space.to_string(), "SPACE");
        assert_eq!(KeyId::ArrowUp.to_string(), "UP");
        assert_eq!(KeyId::Num1.to_string(), "NUM1");
        assert_eq!(KeyId::SteelSeriesKey.to_string(), "SS");
    }

    #[test]
    fn test_keyboard_layout_types() {
        let full = KeyboardLayout::FullSize;
        let tkl = KeyboardLayout::TenKeyLess;
        let compact = KeyboardLayout::Compact;

        assert_ne!(full, tkl);
        assert_ne!(tkl, compact);
        assert_ne!(full, compact);
    }
}
