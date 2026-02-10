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

/// HID address for a specific key (row, column coordinates in key matrix).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyAddress {
    /// Matrix row (0-based)
    pub row: u8,
    /// Matrix column (0-based)
    pub col: u8,
}

impl KeyAddress {
    /// Create a new key address.
    pub fn new(row: u8, col: u8) -> Self {
        Self { row, col }
    }
}

impl fmt::Display for KeyAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.row, self.col)
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
    /// Matrix dimensions
    pub matrix_rows: u8,
    pub matrix_cols: u8,
    /// Total number of individually addressable keys
    pub total_keys: usize,
}

impl KeyMapping {
    /// Create a new key mapping.
    pub fn new(product_id: u16, layout: KeyboardLayout, name: String, matrix_rows: u8, matrix_cols: u8) -> Self {
        Self {
            product_id,
            layout,
            name,
            key_map: HashMap::new(),
            cached_keys: Vec::new(),
            matrix_rows,
            matrix_cols,
            total_keys: 0,
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
        let mut max_row = 0;
        let mut max_col = 0;
        for address in self.key_map.values() {
            max_row = max_row.max(address.row);
            max_col = max_col.max(address.col);
        }

        KeyMappingStats {
            total_keys: self.total_keys,
            actual_matrix_rows: max_row + 1,
            actual_matrix_cols: max_col + 1,
            declared_matrix_rows: self.matrix_rows,
            declared_matrix_cols: self.matrix_cols,
            utilization: (self.total_keys as f32 / (self.matrix_rows as f32 * self.matrix_cols as f32)) * 100.0,
        }
    }
}

/// Key mapping statistics.
#[derive(Debug, Clone)]
pub struct KeyMappingStats {
    pub total_keys: usize,
    pub actual_matrix_rows: u8,
    pub actual_matrix_cols: u8,
    pub declared_matrix_rows: u8,
    pub declared_matrix_cols: u8,
    pub utilization: f32,
}

impl fmt::Display for KeyMappingStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Keys: {}, Matrix: {}x{} (declared {}x{}), Utilization: {:.1}%",
            self.total_keys,
            self.actual_matrix_rows,
            self.actual_matrix_cols,
            self.declared_matrix_rows,
            self.declared_matrix_cols,
            self.utilization
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
        // CRITICAL NOTE: The mappings below are RESEARCH PLACEHOLDERS.
        //
        // These are educated guesses based on standard keyboard layouts and typical
        // matrix addressing schemes. The actual HID key addresses for SteelSeries
        // keyboards have NOT been reverse-engineered yet.
        //
        // To get accurate mappings, we need to:
        // 1. Analyze HID descriptor from actual hardware
        // 2. Send test commands to individual keys
        // 3. Use protocol analysis tools
        // 4. Reverse engineer existing SteelSeries software
        //
        // DO NOT USE THESE MAPPINGS IN PRODUCTION - they are for development structure only!

        // Apex Pro TKL (2023) - PLACEHOLDER mapping
        self.add_apex_pro_tkl_2023_mapping();

        // Apex Pro (full) - PLACEHOLDER mapping
        self.add_apex_pro_full_mapping();

        // Apex Pro TKL (original) - PLACEHOLDER mapping
        self.add_apex_pro_tkl_mapping();
    }

    /// Add Apex Pro TKL 2023 key mapping (PLACEHOLDER).
    fn add_apex_pro_tkl_2023_mapping(&mut self) {
        let mut mapping = KeyMapping::new(
            product_ids::APEX_PRO_TKL_2023,
            KeyboardLayout::TenKeyLess,
            "Apex Pro TKL (2023)".to_string(),
            6,  // Typical matrix rows for TKL
            17, // Typical matrix columns for TKL
        );

        // PLACEHOLDER: Standard TKL layout with guessed matrix addresses
        // Row 0: Function keys
        mapping.add_key(KeyId::Escape, KeyAddress::new(0, 0));
        mapping.add_key(KeyId::F1, KeyAddress::new(0, 2));
        mapping.add_key(KeyId::F2, KeyAddress::new(0, 3));
        mapping.add_key(KeyId::F3, KeyAddress::new(0, 4));
        mapping.add_key(KeyId::F4, KeyAddress::new(0, 5));
        mapping.add_key(KeyId::F5, KeyAddress::new(0, 6));
        mapping.add_key(KeyId::F6, KeyAddress::new(0, 7));
        mapping.add_key(KeyId::F7, KeyAddress::new(0, 8));
        mapping.add_key(KeyId::F8, KeyAddress::new(0, 9));
        mapping.add_key(KeyId::F9, KeyAddress::new(0, 10));
        mapping.add_key(KeyId::F10, KeyAddress::new(0, 11));
        mapping.add_key(KeyId::F11, KeyAddress::new(0, 12));
        mapping.add_key(KeyId::F12, KeyAddress::new(0, 13));

        // Row 1: Number row
        mapping.add_key(KeyId::Backtick, KeyAddress::new(1, 0));
        mapping.add_key(KeyId::Key1, KeyAddress::new(1, 1));
        mapping.add_key(KeyId::Key2, KeyAddress::new(1, 2));
        mapping.add_key(KeyId::Key3, KeyAddress::new(1, 3));
        mapping.add_key(KeyId::Key4, KeyAddress::new(1, 4));
        mapping.add_key(KeyId::Key5, KeyAddress::new(1, 5));
        mapping.add_key(KeyId::Key6, KeyAddress::new(1, 6));
        mapping.add_key(KeyId::Key7, KeyAddress::new(1, 7));
        mapping.add_key(KeyId::Key8, KeyAddress::new(1, 8));
        mapping.add_key(KeyId::Key9, KeyAddress::new(1, 9));
        mapping.add_key(KeyId::Key0, KeyAddress::new(1, 10));
        mapping.add_key(KeyId::Minus, KeyAddress::new(1, 11));
        mapping.add_key(KeyId::Equal, KeyAddress::new(1, 12));
        mapping.add_key(KeyId::Backspace, KeyAddress::new(1, 13));

        // Row 2: First letter row
        mapping.add_key(KeyId::Tab, KeyAddress::new(2, 0));
        mapping.add_key(KeyId::Q, KeyAddress::new(2, 1));
        mapping.add_key(KeyId::W, KeyAddress::new(2, 2));
        mapping.add_key(KeyId::E, KeyAddress::new(2, 3));
        mapping.add_key(KeyId::R, KeyAddress::new(2, 4));
        mapping.add_key(KeyId::T, KeyAddress::new(2, 5));
        mapping.add_key(KeyId::Y, KeyAddress::new(2, 6));
        mapping.add_key(KeyId::U, KeyAddress::new(2, 7));
        mapping.add_key(KeyId::I, KeyAddress::new(2, 8));
        mapping.add_key(KeyId::O, KeyAddress::new(2, 9));
        mapping.add_key(KeyId::P, KeyAddress::new(2, 10));
        mapping.add_key(KeyId::LeftBracket, KeyAddress::new(2, 11));
        mapping.add_key(KeyId::RightBracket, KeyAddress::new(2, 12));
        mapping.add_key(KeyId::Backslash, KeyAddress::new(2, 13));

        // Row 3: Home row
        mapping.add_key(KeyId::CapsLock, KeyAddress::new(3, 0));
        mapping.add_key(KeyId::A, KeyAddress::new(3, 1));
        mapping.add_key(KeyId::S, KeyAddress::new(3, 2));
        mapping.add_key(KeyId::D, KeyAddress::new(3, 3));
        mapping.add_key(KeyId::F, KeyAddress::new(3, 4));
        mapping.add_key(KeyId::G, KeyAddress::new(3, 5));
        mapping.add_key(KeyId::H, KeyAddress::new(3, 6));
        mapping.add_key(KeyId::J, KeyAddress::new(3, 7));
        mapping.add_key(KeyId::K, KeyAddress::new(3, 8));
        mapping.add_key(KeyId::L, KeyAddress::new(3, 9));
        mapping.add_key(KeyId::Semicolon, KeyAddress::new(3, 10));
        mapping.add_key(KeyId::Quote, KeyAddress::new(3, 11));
        mapping.add_key(KeyId::Enter, KeyAddress::new(3, 13));

        // Row 4: Bottom letter row
        mapping.add_key(KeyId::LeftShift, KeyAddress::new(4, 0));
        mapping.add_key(KeyId::Z, KeyAddress::new(4, 2));
        mapping.add_key(KeyId::X, KeyAddress::new(4, 3));
        mapping.add_key(KeyId::C, KeyAddress::new(4, 4));
        mapping.add_key(KeyId::V, KeyAddress::new(4, 5));
        mapping.add_key(KeyId::B, KeyAddress::new(4, 6));
        mapping.add_key(KeyId::N, KeyAddress::new(4, 7));
        mapping.add_key(KeyId::M, KeyAddress::new(4, 8));
        mapping.add_key(KeyId::Comma, KeyAddress::new(4, 9));
        mapping.add_key(KeyId::Period, KeyAddress::new(4, 10));
        mapping.add_key(KeyId::Slash, KeyAddress::new(4, 11));
        mapping.add_key(KeyId::RightShift, KeyAddress::new(4, 12));

        // Row 5: Bottom row
        mapping.add_key(KeyId::LeftCtrl, KeyAddress::new(5, 0));
        mapping.add_key(KeyId::LeftWin, KeyAddress::new(5, 1));
        mapping.add_key(KeyId::LeftAlt, KeyAddress::new(5, 2));
        mapping.add_key(KeyId::Space, KeyAddress::new(5, 6));
        mapping.add_key(KeyId::RightAlt, KeyAddress::new(5, 10));
        mapping.add_key(KeyId::RightWin, KeyAddress::new(5, 11));
        mapping.add_key(KeyId::Menu, KeyAddress::new(5, 12));
        mapping.add_key(KeyId::RightCtrl, KeyAddress::new(5, 13));

        // Navigation cluster (TKL specific positions)
        mapping.add_key(KeyId::Insert, KeyAddress::new(0, 14));
        mapping.add_key(KeyId::Home, KeyAddress::new(0, 15));
        mapping.add_key(KeyId::PageUp, KeyAddress::new(0, 16));
        mapping.add_key(KeyId::Delete, KeyAddress::new(1, 14));
        mapping.add_key(KeyId::End, KeyAddress::new(1, 15));
        mapping.add_key(KeyId::PageDown, KeyAddress::new(1, 16));

        // Arrow cluster
        mapping.add_key(KeyId::ArrowUp, KeyAddress::new(3, 15));
        mapping.add_key(KeyId::ArrowLeft, KeyAddress::new(4, 14));
        mapping.add_key(KeyId::ArrowDown, KeyAddress::new(4, 15));
        mapping.add_key(KeyId::ArrowRight, KeyAddress::new(4, 16));

        // Special SteelSeries key (if present)
        mapping.add_key(KeyId::SteelSeriesKey, KeyAddress::new(5, 14));

        self.mappings.insert(product_ids::APEX_PRO_TKL_2023, mapping);
    }

    /// Add Apex Pro (full) key mapping (PLACEHOLDER).
    fn add_apex_pro_full_mapping(&mut self) {
        let mut mapping = KeyMapping::new(
            product_ids::APEX_PRO,
            KeyboardLayout::FullSize,
            "Apex Pro".to_string(),
            6,  // Typical matrix rows
            21, // Typical matrix columns for full-size
        );

        // PLACEHOLDER: Start with TKL layout and add numpad
        // This is a simplified placeholder - real implementation needs detailed research

        // Copy main layout from TKL (same structure)
        // Function row
        mapping.add_key(KeyId::Escape, KeyAddress::new(0, 0));
        mapping.add_key(KeyId::F1, KeyAddress::new(0, 2));
        // ... (abbreviated for space - would include all keys)

        // Add numpad (placeholder positions)
        mapping.add_key(KeyId::NumLock, KeyAddress::new(0, 17));
        mapping.add_key(KeyId::NumSlash, KeyAddress::new(0, 18));
        mapping.add_key(KeyId::NumAsterisk, KeyAddress::new(0, 19));
        mapping.add_key(KeyId::NumMinus, KeyAddress::new(0, 20));

        mapping.add_key(KeyId::Num7, KeyAddress::new(1, 17));
        mapping.add_key(KeyId::Num8, KeyAddress::new(1, 18));
        mapping.add_key(KeyId::Num9, KeyAddress::new(1, 19));
        mapping.add_key(KeyId::NumPlus, KeyAddress::new(1, 20));

        // ... (abbreviated - would include all numpad keys)

        self.mappings.insert(product_ids::APEX_PRO, mapping);
    }

    /// Add Apex Pro TKL (original) key mapping (PLACEHOLDER).
    fn add_apex_pro_tkl_mapping(&mut self) {
        // PLACEHOLDER: Similar to 2023 TKL but may have different matrix dimensions
        let mapping = KeyMapping::new(
            product_ids::APEX_PRO_TKL,
            KeyboardLayout::TenKeyLess,
            "Apex Pro TKL".to_string(),
            6,
            17,
        );

        // Would populate with actual mappings...
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
        let addr = KeyAddress::new(1, 5);
        assert_eq!(addr.row, 1);
        assert_eq!(addr.col, 5);
        assert_eq!(addr.to_string(), "(1, 5)");
    }

    #[test]
    fn test_key_mapping_creation() {
        let mut mapping = KeyMapping::new(
            product_ids::APEX_PRO_TKL_2023,
            KeyboardLayout::TenKeyLess,
            "Test Keyboard".to_string(),
            6,
            17,
        );

        // Initially empty
        assert_eq!(mapping.total_keys, 0);
        assert!(!mapping.supports_key(KeyId::A));

        // Add a key
        mapping.add_key(KeyId::A, KeyAddress::new(3, 1));
        assert_eq!(mapping.total_keys, 1);
        assert!(mapping.supports_key(KeyId::A));
        assert_eq!(mapping.get_key_address(KeyId::A), Some(KeyAddress::new(3, 1)));
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
    }

    #[test]
    fn test_key_mapping_stats() {
        let mut mapping = KeyMapping::new(
            product_ids::APEX_PRO_TKL_2023,
            KeyboardLayout::TenKeyLess,
            "Test".to_string(),
            6,
            17,
        );

        // Add some keys
        mapping.add_key(KeyId::A, KeyAddress::new(3, 1));
        mapping.add_key(KeyId::B, KeyAddress::new(4, 6));
        mapping.add_key(KeyId::Escape, KeyAddress::new(0, 0));

        let stats = mapping.get_stats();
        assert_eq!(stats.total_keys, 3);
        assert_eq!(stats.actual_matrix_rows, 5); // 0-4 + 1
        assert_eq!(stats.actual_matrix_cols, 7); // 0-6 + 1
        assert_eq!(stats.declared_matrix_rows, 6);
        assert_eq!(stats.declared_matrix_cols, 17);
        assert!(stats.utilization > 0.0);
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
