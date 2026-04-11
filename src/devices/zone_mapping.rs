//! Zone-based RGB mapping and fallback system for SteelSeries keyboards.
//!
//! This module provides enhanced zone-based RGB control that serves as a reliable
//! fallback when per-key RGB is not available or fails.

use crate::devices::product_ids;
use crate::rgb::Color;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Zone position on the keyboard for logical zone mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ZonePosition {
    /// Main keys (letters, numbers, basic punctuation)
    MainKeys,
    /// Function row (F1-F12, Escape)
    FunctionRow,
    /// Left modifier keys (Ctrl, Alt, Win)
    LeftModifiers,
    /// Right modifier keys (Ctrl, Alt, Win, Menu)
    RightModifiers,
    /// Arrow keys cluster
    ArrowKeys,
    /// Navigation cluster (Insert, Delete, Home, End, Page Up/Down)
    NavigationCluster,
    /// Numpad (for full-size keyboards)
    Numpad,
    /// Logo/brand area
    Logo,
    /// Under-glow/rim lighting
    Underglow,
    /// Custom zones (manufacturer-specific)
    Custom(u8),
}

impl fmt::Display for ZonePosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ZonePosition::MainKeys => write!(f, "Main Keys"),
            ZonePosition::FunctionRow => write!(f, "Function Row"),
            ZonePosition::LeftModifiers => write!(f, "Left Modifiers"),
            ZonePosition::RightModifiers => write!(f, "Right Modifiers"),
            ZonePosition::ArrowKeys => write!(f, "Arrow Keys"),
            ZonePosition::NavigationCluster => write!(f, "Navigation"),
            ZonePosition::Numpad => write!(f, "Numpad"),
            ZonePosition::Logo => write!(f, "Logo"),
            ZonePosition::Underglow => write!(f, "Underglow"),
            ZonePosition::Custom(id) => write!(f, "Custom {}", id),
        }
    }
}

/// Information about a specific RGB zone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneInfo {
    /// Zone index (0-based)
    pub index: usize,
    /// Logical position on the keyboard
    pub position: ZonePosition,
    /// Human-readable zone name
    pub name: String,
    /// Whether this zone is critical for basic functionality
    pub critical: bool,
    /// Estimated number of LEDs in this zone (for effect scaling)
    pub led_count: usize,
}

impl ZoneInfo {
    /// Create a new zone info.
    pub fn new(index: usize, position: ZonePosition, name: String, critical: bool, led_count: usize) -> Self {
        Self {
            index,
            position,
            name,
            critical,
            led_count,
        }
    }
}

/// Complete zone mapping for a keyboard model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneMapping {
    /// Product ID this mapping applies to
    pub product_id: u16,
    /// Human-readable keyboard name
    pub name: String,
    /// Total number of RGB zones
    pub zone_count: usize,
    /// Detailed information for each zone
    pub zones: Vec<ZoneInfo>,
    /// Zone index to position lookup
    zone_positions: HashMap<usize, ZonePosition>,
}

impl ZoneMapping {
    /// Create a new zone mapping.
    pub fn new(product_id: u16, name: String) -> Self {
        Self {
            product_id,
            name,
            zone_count: 0,
            zones: Vec::new(),
            zone_positions: HashMap::new(),
        }
    }

    /// Add a zone to the mapping.
    pub fn add_zone(&mut self, position: ZonePosition, name: String, critical: bool, led_count: usize) {
        let index = self.zones.len();
        let zone_info = ZoneInfo::new(index, position, name, critical, led_count);
        self.zone_positions.insert(index, position);
        self.zones.push(zone_info);
        self.zone_count = self.zones.len();
    }

    /// Get zone info by index.
    pub fn get_zone(&self, index: usize) -> Option<&ZoneInfo> {
        self.zones.get(index)
    }

    /// Get zone index by position.
    pub fn find_zone_by_position(&self, position: ZonePosition) -> Option<usize> {
        self.zone_positions
            .iter()
            .find(|(_, &pos)| pos == position)
            .map(|(&index, _)| index)
    }

    /// Get all zones with a specific position.
    pub fn get_zones_by_position(&self, position: ZonePosition) -> Vec<usize> {
        self.zone_positions
            .iter()
            .filter(|(_, &pos)| pos == position)
            .map(|(&index, _)| index)
            .collect()
    }

    /// Get critical zones (must work for basic functionality).
    pub fn get_critical_zones(&self) -> Vec<usize> {
        self.zones
            .iter()
            .filter(|zone| zone.critical)
            .map(|zone| zone.index)
            .collect()
    }

    /// Get total LED count estimate.
    pub fn total_led_count(&self) -> usize {
        self.zones.iter().map(|zone| zone.led_count).sum()
    }
}

/// Zone-based RGB effect patterns.
#[derive(Debug, Clone, PartialEq)]
pub enum ZoneEffect {
    /// All zones same color
    Solid(Color),
    /// Alternate between zones
    Alternating(Vec<Color>),
    /// Gradient across zones
    Gradient { start: Color, end: Color },
    /// Wave pattern
    Wave { colors: Vec<Color>, offset: f32 },
    /// Breathing effect per zone
    Breathing { colors: Vec<Color>, phase_offset: f32 },
    /// Reactive (highlight specific zones)
    Reactive {
        base: Color,
        highlight: Color,
        zones: Vec<usize>,
    },
    /// Custom color per zone
    Custom(Vec<Color>),
}

impl ZoneEffect {
    /// Compute colors for all zones at a specific time.
    pub fn compute_colors(&self, zone_count: usize, time_secs: f32) -> Vec<Color> {
        match self {
            ZoneEffect::Solid(color) => vec![*color; zone_count],

            ZoneEffect::Alternating(colors) => {
                if colors.is_empty() {
                    vec![Color::BLACK; zone_count]
                } else {
                    (0..zone_count).map(|i| colors[i % colors.len()]).collect()
                }
            }

            ZoneEffect::Gradient { start, end } => {
                if zone_count <= 1 {
                    vec![*start; zone_count]
                } else {
                    (0..zone_count)
                        .map(|i| {
                            let t = i as f32 / (zone_count - 1) as f32;
                            Color::blend(*start, *end, t)
                        })
                        .collect()
                }
            }

            ZoneEffect::Wave { colors, offset } => {
                if colors.is_empty() {
                    vec![Color::BLACK; zone_count]
                } else {
                    (0..zone_count)
                        .map(|i| {
                            let wave_pos = ((i as f32 / zone_count as f32) + *offset + time_secs * 0.5) % 1.0;
                            let color_index = (wave_pos * colors.len() as f32) as usize % colors.len();
                            colors[color_index]
                        })
                        .collect()
                }
            }

            ZoneEffect::Breathing { colors, phase_offset } => {
                if colors.is_empty() {
                    vec![Color::BLACK; zone_count]
                } else {
                    (0..zone_count)
                        .map(|i| {
                            let phase = time_secs * 2.0 + (i as f32 * *phase_offset);
                            let intensity = (phase.sin() + 1.0) / 2.0; // 0.0 to 1.0
                            let base_color = colors[i % colors.len()];
                            base_color.scale(intensity)
                        })
                        .collect()
                }
            }

            ZoneEffect::Reactive { base, highlight, zones } => {
                let mut result = vec![*base; zone_count];
                for &zone_index in zones {
                    if zone_index < zone_count {
                        result[zone_index] = *highlight;
                    }
                }
                result
            }

            ZoneEffect::Custom(colors) => {
                let mut result = Vec::with_capacity(zone_count);
                for i in 0..zone_count {
                    result.push(colors.get(i).copied().unwrap_or(Color::BLACK));
                }
                result
            }
        }
    }
}

/// Zone-based RGB fallback system.
pub struct ZoneFallback {
    /// Available zone mappings
    mappings: HashMap<u16, ZoneMapping>,
    /// Current effect being rendered
    current_effect: Option<ZoneEffect>,
}

impl ZoneFallback {
    /// Create a new zone fallback system.
    pub fn new() -> Self {
        let mut fallback = Self {
            mappings: HashMap::new(),
            current_effect: None,
        };
        fallback.initialize_known_mappings();
        fallback
    }

    /// Initialize mappings for known keyboard models.
    fn initialize_known_mappings(&mut self) {
        // Apex Pro TKL 2023 - Enhanced 9-zone mapping
        let mut apex_tkl_2023 = ZoneMapping::new(product_ids::APEX_PRO_TKL_2023, "Apex Pro TKL 2023".to_string());
        apex_tkl_2023.add_zone(ZonePosition::FunctionRow, "Function Keys".to_string(), false, 13);
        apex_tkl_2023.add_zone(ZonePosition::MainKeys, "Main Keys Left".to_string(), true, 20);
        apex_tkl_2023.add_zone(ZonePosition::MainKeys, "Main Keys Center".to_string(), true, 25);
        apex_tkl_2023.add_zone(ZonePosition::MainKeys, "Main Keys Right".to_string(), true, 20);
        apex_tkl_2023.add_zone(ZonePosition::LeftModifiers, "Left Modifiers".to_string(), true, 4);
        apex_tkl_2023.add_zone(ZonePosition::RightModifiers, "Right Modifiers".to_string(), true, 4);
        apex_tkl_2023.add_zone(
            ZonePosition::NavigationCluster,
            "Navigation Block".to_string(),
            false,
            6,
        );
        apex_tkl_2023.add_zone(ZonePosition::ArrowKeys, "Arrow Keys".to_string(), false, 4);
        apex_tkl_2023.add_zone(ZonePosition::Logo, "SteelSeries Logo".to_string(), false, 1);
        let mut apex_tkl_2023_wireless = apex_tkl_2023.clone();
        apex_tkl_2023_wireless.product_id = product_ids::APEX_PRO_TKL_2023_WIRELESS;
        self.mappings.insert(product_ids::APEX_PRO_TKL_2023, apex_tkl_2023);
        self.mappings
            .insert(product_ids::APEX_PRO_TKL_2023_WIRELESS, apex_tkl_2023_wireless);

        // Apex 3 - Enhanced 10-zone mapping
        let mut apex_3 = ZoneMapping::new(product_ids::APEX_3, "Apex 3".to_string());
        apex_3.add_zone(ZonePosition::FunctionRow, "Function Keys".to_string(), false, 13);
        apex_3.add_zone(ZonePosition::MainKeys, "Main Keys Left".to_string(), true, 15);
        apex_3.add_zone(ZonePosition::MainKeys, "Main Keys Center".to_string(), true, 20);
        apex_3.add_zone(ZonePosition::MainKeys, "Main Keys Right".to_string(), true, 15);
        apex_3.add_zone(ZonePosition::LeftModifiers, "Left Modifiers".to_string(), true, 3);
        apex_3.add_zone(ZonePosition::RightModifiers, "Right Modifiers".to_string(), true, 3);
        apex_3.add_zone(
            ZonePosition::NavigationCluster,
            "Navigation Block".to_string(),
            false,
            6,
        );
        apex_3.add_zone(ZonePosition::ArrowKeys, "Arrow Keys".to_string(), false, 4);
        apex_3.add_zone(ZonePosition::Numpad, "Numpad".to_string(), false, 17);
        apex_3.add_zone(ZonePosition::Underglow, "Underglow".to_string(), false, 10);
        self.mappings.insert(product_ids::APEX_3, apex_3);

        // Apex 3 TKL - Enhanced 9-zone mapping
        let mut apex_3_tkl = ZoneMapping::new(product_ids::APEX_3_TKL, "Apex 3 TKL".to_string());
        apex_3_tkl.add_zone(ZonePosition::FunctionRow, "Function Keys".to_string(), false, 13);
        apex_3_tkl.add_zone(ZonePosition::MainKeys, "Main Keys Left".to_string(), true, 18);
        apex_3_tkl.add_zone(ZonePosition::MainKeys, "Main Keys Center".to_string(), true, 22);
        apex_3_tkl.add_zone(ZonePosition::MainKeys, "Main Keys Right".to_string(), true, 18);
        apex_3_tkl.add_zone(ZonePosition::LeftModifiers, "Left Modifiers".to_string(), true, 3);
        apex_3_tkl.add_zone(ZonePosition::RightModifiers, "Right Modifiers".to_string(), true, 3);
        apex_3_tkl.add_zone(
            ZonePosition::NavigationCluster,
            "Navigation Block".to_string(),
            false,
            6,
        );
        apex_3_tkl.add_zone(ZonePosition::ArrowKeys, "Arrow Keys".to_string(), false, 4);
        apex_3_tkl.add_zone(ZonePosition::Underglow, "Underglow".to_string(), false, 8);
        self.mappings.insert(product_ids::APEX_3_TKL, apex_3_tkl);

        // Generic single-zone fallback for other keyboards
        let mut generic = ZoneMapping::new(0, "Generic Single Zone".to_string());
        generic.add_zone(ZonePosition::MainKeys, "All Keys".to_string(), true, 50);
        // Add mappings for single-zone keyboards
        for &product_id in &[
            product_ids::APEX_PRO,
            product_ids::APEX_PRO_TKL,
            product_ids::APEX_5,
            product_ids::APEX_7,
            product_ids::APEX_7_TKL,
        ] {
            let mut mapping = generic.clone();
            mapping.product_id = product_id;
            mapping.name = format!("Single Zone (0x{:04x})", product_id);
            self.mappings.insert(product_id, mapping);
        }
    }

    /// Get zone mapping for a product ID.
    pub fn get_mapping(&self, product_id: u16) -> Option<&ZoneMapping> {
        self.mappings.get(&product_id)
    }

    /// Simulate per-key effect using zone-based approximation.
    pub fn simulate_per_key_effect(
        &self,
        product_id: u16,
        per_key_colors: &[(crate::devices::KeyId, Color)],
    ) -> Option<Vec<Color>> {
        let mapping = self.get_mapping(product_id)?;

        if per_key_colors.is_empty() {
            return Some(vec![Color::BLACK; mapping.zone_count]);
        }

        // Group colors by logical zones
        let mut zone_colors = vec![Color::BLACK; mapping.zone_count];
        let mut zone_led_counts = vec![0usize; mapping.zone_count];

        for (key_id, color) in per_key_colors {
            // Map key to approximate zone
            let zone_index = self.map_key_to_zone(*key_id, mapping);
            if let Some(zone_idx) = zone_index {
                if zone_idx < mapping.zone_count {
                    // Blend colors if multiple keys affect the same zone
                    if zone_led_counts[zone_idx] == 0 {
                        zone_colors[zone_idx] = *color;
                    } else {
                        // Average the colors
                        let existing = zone_colors[zone_idx];
                        zone_colors[zone_idx] = Color::blend(existing, *color, 0.5);
                    }
                    zone_led_counts[zone_idx] += 1;
                }
            }
        }

        Some(zone_colors)
    }

    /// Map a logical key to a zone index.
    fn map_key_to_zone(&self, key_id: crate::devices::KeyId, mapping: &ZoneMapping) -> Option<usize> {
        use crate::devices::KeyId;

        // Map keys to logical zones based on keyboard layout
        let position = match key_id {
            // Function row
            KeyId::Escape
            | KeyId::F1
            | KeyId::F2
            | KeyId::F3
            | KeyId::F4
            | KeyId::F5
            | KeyId::F6
            | KeyId::F7
            | KeyId::F8
            | KeyId::F9
            | KeyId::F10
            | KeyId::F11
            | KeyId::F12 => ZonePosition::FunctionRow,

            // Main letter/number keys
            KeyId::A | KeyId::Q | KeyId::Z | KeyId::Key1 | KeyId::Tab | KeyId::CapsLock | KeyId::LeftShift => {
                ZonePosition::MainKeys
            } // Left side

            KeyId::S
            | KeyId::W
            | KeyId::X
            | KeyId::Key2
            | KeyId::Key3
            | KeyId::D
            | KeyId::E
            | KeyId::C
            | KeyId::R
            | KeyId::F
            | KeyId::V
            | KeyId::T
            | KeyId::G
            | KeyId::B
            | KeyId::Key4
            | KeyId::Key5
            | KeyId::Key6 => ZonePosition::MainKeys, // Center (reuse MainKeys)

            KeyId::H
            | KeyId::Y
            | KeyId::N
            | KeyId::U
            | KeyId::J
            | KeyId::M
            | KeyId::I
            | KeyId::K
            | KeyId::O
            | KeyId::L
            | KeyId::P
            | KeyId::Key7
            | KeyId::Key8
            | KeyId::Key9
            | KeyId::Key0
            | KeyId::Semicolon
            | KeyId::Quote
            | KeyId::Enter
            | KeyId::LeftBracket
            | KeyId::RightBracket
            | KeyId::Backslash
            | KeyId::Comma
            | KeyId::Period
            | KeyId::Slash
            | KeyId::RightShift
            | KeyId::Minus
            | KeyId::Equal
            | KeyId::Backspace
            | KeyId::Backtick => ZonePosition::MainKeys, // Right side (reuse MainKeys)

            // Modifiers
            KeyId::LeftCtrl | KeyId::LeftWin | KeyId::LeftAlt => ZonePosition::LeftModifiers,
            KeyId::RightAlt | KeyId::RightWin | KeyId::Menu | KeyId::RightCtrl => ZonePosition::RightModifiers,

            // Arrow keys
            KeyId::ArrowUp | KeyId::ArrowDown | KeyId::ArrowLeft | KeyId::ArrowRight => ZonePosition::ArrowKeys,

            // Navigation
            KeyId::Insert | KeyId::Delete | KeyId::Home | KeyId::End | KeyId::PageUp | KeyId::PageDown => {
                ZonePosition::NavigationCluster
            }

            // Numpad
            KeyId::NumLock
            | KeyId::NumSlash
            | KeyId::NumAsterisk
            | KeyId::NumMinus
            | KeyId::Num7
            | KeyId::Num8
            | KeyId::Num9
            | KeyId::NumPlus
            | KeyId::Num4
            | KeyId::Num5
            | KeyId::Num6
            | KeyId::Num1
            | KeyId::Num2
            | KeyId::Num3
            | KeyId::NumEnter
            | KeyId::Num0
            | KeyId::NumPeriod => ZonePosition::Numpad,

            // Special keys
            KeyId::Space => ZonePosition::MainKeys, // Space bar typically in main area
            KeyId::SteelSeriesKey => ZonePosition::Logo,
            KeyId::VolumeWheel => ZonePosition::Logo,
        };

        mapping.find_zone_by_position(position)
    }

    /// Get recommended zone effect for a pattern name.
    pub fn pattern_to_zone_effect(&self, pattern: &str) -> Option<ZoneEffect> {
        match pattern.to_lowercase().as_str() {
            "rainbow" => Some(ZoneEffect::Wave {
                colors: vec![
                    Color::RED,
                    Color::ORANGE,
                    Color::YELLOW,
                    Color::GREEN,
                    Color::CYAN,
                    Color::BLUE,
                    Color::PURPLE,
                    Color::MAGENTA,
                ],
                offset: 0.0,
            }),
            "breathing" => Some(ZoneEffect::Breathing {
                colors: vec![Color::CYAN, Color::PURPLE, Color::ORANGE],
                phase_offset: 0.5,
            }),
            "gradient" => Some(ZoneEffect::Gradient {
                start: Color::PURPLE,
                end: Color::CYAN,
            }),
            "solid" => Some(ZoneEffect::Solid(Color::WHITE)),
            "alternating" => Some(ZoneEffect::Alternating(vec![Color::RED, Color::BLUE])),
            _ => None,
        }
    }

    /// Set current effect for time-based rendering.
    pub fn set_current_effect(&mut self, effect: ZoneEffect) {
        self.current_effect = Some(effect);
    }

    /// Get current zone colors at a specific time.
    pub fn get_current_colors(&self, product_id: u16, time_secs: f32) -> Option<Vec<Color>> {
        let mapping = self.get_mapping(product_id)?;
        let effect = self.current_effect.as_ref()?;
        Some(effect.compute_colors(mapping.zone_count, time_secs))
    }

    /// Get supported product IDs.
    pub fn get_supported_products(&self) -> Vec<u16> {
        self.mappings.keys().copied().collect()
    }
}

impl Default for ZoneFallback {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zone_mapping_creation() {
        let mut mapping = ZoneMapping::new(0x1628, "Test Keyboard".to_string());
        mapping.add_zone(ZonePosition::MainKeys, "Main".to_string(), true, 50);
        mapping.add_zone(ZonePosition::FunctionRow, "F-Keys".to_string(), false, 12);

        assert_eq!(mapping.zone_count, 2);
        assert_eq!(mapping.total_led_count(), 62);
        assert_eq!(mapping.get_critical_zones(), vec![0]);

        let main_zone = mapping.get_zone(0).unwrap();
        assert_eq!(main_zone.position, ZonePosition::MainKeys);
        assert!(main_zone.critical);
    }

    #[test]
    fn test_zone_effect_solid() {
        let effect = ZoneEffect::Solid(Color::RED);
        let colors = effect.compute_colors(5, 0.0);
        assert_eq!(colors.len(), 5);
        assert!(colors.iter().all(|&c| c == Color::RED));
    }

    #[test]
    fn test_zone_effect_alternating() {
        let effect = ZoneEffect::Alternating(vec![Color::RED, Color::BLUE]);
        let colors = effect.compute_colors(4, 0.0);
        assert_eq!(colors, vec![Color::RED, Color::BLUE, Color::RED, Color::BLUE]);
    }

    #[test]
    fn test_zone_effect_gradient() {
        let effect = ZoneEffect::Gradient {
            start: Color::BLACK,
            end: Color::WHITE,
        };
        let colors = effect.compute_colors(3, 0.0);
        assert_eq!(colors.len(), 3);
        assert_eq!(colors[0], Color::BLACK);
        assert_eq!(colors[2], Color::WHITE);
        // Middle color should be gray
        assert_ne!(colors[1], Color::BLACK);
        assert_ne!(colors[1], Color::WHITE);
    }

    #[test]
    fn test_zone_fallback_mapping() {
        let fallback = ZoneFallback::new();

        // Should have mappings for known keyboards
        assert!(fallback.get_mapping(product_ids::APEX_PRO_TKL_2023).is_some());
        assert!(fallback.get_mapping(product_ids::APEX_3).is_some());
        assert!(fallback.get_mapping(product_ids::APEX_3_TKL).is_some());

        let mapping = fallback.get_mapping(product_ids::APEX_PRO_TKL_2023).unwrap();
        assert_eq!(mapping.zone_count, 9);
        assert!(mapping.total_led_count() > 0);
    }

    #[test]
    fn test_per_key_simulation() {
        let fallback = ZoneFallback::new();
        let per_key_colors = vec![
            (crate::devices::KeyId::A, Color::RED),
            (crate::devices::KeyId::F1, Color::BLUE),
        ];

        let zone_colors = fallback
            .simulate_per_key_effect(product_ids::APEX_PRO_TKL_2023, &per_key_colors)
            .unwrap();

        assert_eq!(zone_colors.len(), 9);
        // Should have some non-black colors where keys were mapped
        assert!(zone_colors.iter().any(|&c| c != Color::BLACK));
    }

    #[test]
    fn test_pattern_effects() {
        let fallback = ZoneFallback::new();

        assert!(fallback.pattern_to_zone_effect("rainbow").is_some());
        assert!(fallback.pattern_to_zone_effect("breathing").is_some());
        assert!(fallback.pattern_to_zone_effect("gradient").is_some());
        assert!(fallback.pattern_to_zone_effect("unknown").is_none());
    }

    #[test]
    fn test_zone_positions() {
        assert_eq!(ZonePosition::MainKeys.to_string(), "Main Keys");
        assert_eq!(ZonePosition::FunctionRow.to_string(), "Function Row");
        assert_eq!(ZonePosition::Custom(5).to_string(), "Custom 5");
    }

    #[test]
    fn test_zone_fallback_initialization() {
        let fallback = ZoneFallback::new();
        let products = fallback.get_supported_products();
        assert!(products.contains(&product_ids::APEX_PRO_TKL_2023));
        assert!(products.contains(&product_ids::APEX_3));
        assert!(products.contains(&product_ids::APEX_3_TKL));
    }

    #[test]
    fn test_zone_fallback_get_mapping() {
        let fallback = ZoneFallback::new();
        let mapping = fallback.get_mapping(product_ids::APEX_3);
        assert!(mapping.is_some());
        assert_eq!(mapping.unwrap().zone_count, 10);

        let unknown = fallback.get_mapping(0xDEAD);
        assert!(unknown.is_none());
    }

    #[test]
    fn test_per_key_simulation_edge_cases() {
        let fallback = ZoneFallback::new();

        // Empty input
        let empty_colors = vec![];
        let res = fallback.simulate_per_key_effect(product_ids::APEX_PRO_TKL_2023, &empty_colors);
        assert!(res.is_some());
        let colors = res.unwrap();
        assert_eq!(colors.len(), 9);
        assert!(colors.iter().all(|&c| c == Color::BLACK));

        // Unknown product
        let res = fallback.simulate_per_key_effect(0xDEAD, &[(crate::devices::KeyId::A, Color::RED)]);
        assert!(res.is_none());

        // Color blending
        let blending_colors = vec![
            (crate::devices::KeyId::A, Color::RED),
            (crate::devices::KeyId::Q, Color::BLUE),
        ];
        let res = fallback.simulate_per_key_effect(product_ids::APEX_PRO_TKL_2023, &blending_colors);
        assert!(res.is_some());
        let colors = res.unwrap();
        // Both A and Q should map to the same MainKeys zone selected by the current mapping logic.
        // Verify that one zone contains a blended color produced from both inputs.
        let a_zone_idx = map_key_to_zone(product_ids::APEX_PRO_TKL_2023, crate::devices::KeyId::A);
        let q_zone_idx = map_key_to_zone(product_ids::APEX_PRO_TKL_2023, crate::devices::KeyId::Q);
        assert_eq!(
            a_zone_idx, q_zone_idx,
            "A and Q should map to the same zone for blending on Apex Pro TKL 2023"
        );
        let zone_idx = a_zone_idx;
        assert_eq!(
            colors[zone_idx],
            Color::blend(Color::RED, Color::BLUE, 0.5),
            "Expected the shared zone to contain the blended color"
        );
    }

    #[test]
    fn test_zone_fallback_current_colors() {
        let mut fallback = ZoneFallback::new();
        let effect = ZoneEffect::Solid(Color::GREEN);
        fallback.set_current_effect(effect);

        let colors = fallback.get_current_colors(product_ids::APEX_PRO_TKL_2023, 0.0);
        assert!(colors.is_some());
        let colors = colors.unwrap();
        assert_eq!(colors.len(), 9);
        assert!(colors.iter().all(|&c| c == Color::GREEN));
    }

    #[test]
    fn test_pattern_to_zone_effect_all() {
        let fallback = ZoneFallback::new();
        let patterns = ["rainbow", "breathing", "gradient", "solid", "alternating"];
        for pattern in patterns {
            assert!(
                fallback.pattern_to_zone_effect(pattern).is_some(),
                "Pattern {} should be supported",
                pattern
            );
        }
    }
}
