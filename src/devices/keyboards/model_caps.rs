//! `[EXPERIMENTAL]` Per-model capability table of the Gen 3 wired Apex boards that share the `0x1628` code path
//! ([`ApexProTkl2023`](super::apex_pro_tkl_2023::ApexProTkl2023)).
//!
//! The numbers and key lists are derived from GG's decrypted device specs (`apex_pro_tkl_2022`,
//! `apex_pro_tkl_2024`, `apex_pro_tkl_2024_cyber`, `apex_pro_tkl_2024_steel`, `apex_pro_mini`, `apex_pro_mini_2024`,
//! `apex_9_tkl`, `apex_9_mini`, all on `base_apex_2022_wired`; read 2026-11-11). Only `0x1628` is confirmed on
//! hardware; every other row is untested. A feature a row does not list is refused rather than sent in another
//! board's shape: the commands in `hid_reports` are sized for a 645-byte feature report, so a 513-byte board gets
//! its direct lighting frame and nothing else until those commands take a report size.

use super::apex_pro_tkl_2023::WIRED_2023_KEYS;
use super::key_action::MAPPING_KEYS;
use crate::devices::hid_reports::PROFILE_CHUNK_REPORT_SIZE;
use crate::devices::product_ids::*;

/// How a model's OLED takes its 640-byte image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OledImage {
    /// No OLED.
    None,
    /// Row-major, 16 bytes per row, MSB left: the layout confirmed on `0x1628`.
    RowMajor,
    /// The spec runs the image through `convert-to-column-packed-byte-format` for the 2024 boards; `ssgg` writes
    /// only the row-major layout, so these are refused.
    ColumnPacked,
}

/// What one board has, by product ID.
#[derive(Debug, Clone, Copy)]
pub struct ModelCaps {
    pub product_id: u16,
    /// `feature-report-size`, report-ID byte included.
    pub feature_report_size: usize,
    /// `device-mapping-hid-codes`: the keys of a base or meta mapping layer, sorted.
    pub mapping_keys: &'static [u8],
    /// `device-analog-hid-codes`: keys with analog (hall) switches, sorted; empty on the Apex 9 boards, which have
    /// no analog actuation.
    pub analog_keys: &'static [u8],
    /// `has-second-actuation-layer`.
    pub has_second_actuation: bool,
    pub oled: OledImage,
    /// `macro-data-size` in bytes.
    pub macro_data_size: usize,
    /// The usages of the prism-sync layout (`hidcode-matrix-layout-nonvariant`), i.e. the keys GG lights.
    pub lighting_keys: &'static [u8],
    /// Whether the live `38 6x` frames of the wired `0x1628` are right for this board: its raw hall-threshold table
    /// is the one `ACTUATION_POINTS` holds. The 2024 boards carry their own tables, so this is `false` for them.
    pub live_analog: bool,
    /// Whether the on-board profile blob is the `0x1628` one (same `device_profile` structs, 12,500 bytes).
    pub profile_read: bool,
}

impl ModelCaps {
    /// Whether the board takes the 645-byte feature reports the typed mapping, macro, display and profile
    /// commands are sized for.
    #[must_use]
    pub const fn wide_reports(&self) -> bool {
        self.feature_report_size == PROFILE_CHUNK_REPORT_SIZE
    }
}

/// `device-analog-hid-codes` of the Pro TKL family (`apex_pro_tkl_2022`): the first 70 `device-key-idx-to-hid`
/// entries without blanks, sorted. The same 68 usages as `WIRED_2023_PROFILE_SLOTS`.
pub(crate) const TKL_ANALOG_KEYS: [u8; 68] = [
    0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16,
    0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x2A,
    0x2B, 0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x64, 0x87, 0x88, 0x89,
    0x8A, 0x8B, 0xE0, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xF0,
];

/// `device-mapping-hid-codes` of `apex_pro_mini` / `apex_9_mini` (`device-key-idx-to-hid` without blanks, sorted;
/// their ignore list is empty). On the Pro Mini it is also `device-analog-hid-codes`.
const MINI_KEYS: [u8; 68] = [
    0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16,
    0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29,
    0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x31, 0x32, 0x33, 0x34, 0x36, 0x37, 0x38, 0x39, 0x64, 0x87, 0x88, 0x89,
    0x8A, 0x8B, 0xE0, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xF0,
];

/// Prism-sync layout of the 2024 Pro TKL boards: the `0x1628` layout plus the play/pause key `0xFB` over the
/// numpad. Matrix order (row by row).
pub(crate) const TKL_2024_LIGHTING_KEYS: [u8; 85] = [
    0x29, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x3F, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0xFB, 0x35, 0x1E, 0x1F, 0x20, 0x21,
    0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x2D, 0x2E, 0x2A, 0x49, 0x4A, 0x4B, 0x2B, 0x14, 0x1A, 0x08, 0x15, 0x17, 0x1C,
    0x18, 0x0C, 0x12, 0x13, 0x2F, 0x30, 0x31, 0x4C, 0x4D, 0x4E, 0x39, 0x04, 0x16, 0x07, 0x09, 0x0A, 0x0B, 0x0D, 0x0E,
    0x0F, 0x33, 0x34, 0x28, 0xE1, 0x1D, 0x1B, 0x06, 0x19, 0x05, 0x11, 0x10, 0x36, 0x37, 0x38, 0xE5, 0x52, 0xE0, 0xE3,
    0xE2, 0x2C, 0xE6, 0xE7, 0xF0, 0xE4, 0x50, 0x51, 0x4F,
];

/// Prism-sync layout of the Mini boards (`base_apex_2022_mini_layout_lighting`), matrix order.
pub(crate) const MINI_LIGHTING_KEYS: [u8; 61] = [
    0x29, 0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x2D, 0x2E, 0x2A, 0x2B, 0x14, 0x1A, 0x08, 0x15,
    0x17, 0x1C, 0x18, 0x0C, 0x12, 0x13, 0x2F, 0x30, 0x31, 0x39, 0x04, 0x16, 0x07, 0x09, 0x0A, 0x0B, 0x0D, 0x0E, 0x0F,
    0x33, 0x34, 0x28, 0xE1, 0x1D, 0x1B, 0x06, 0x19, 0x05, 0x11, 0x10, 0x36, 0x37, 0x38, 0xE5, 0xE0, 0xE3, 0xE2, 0x2C,
    0xE6, 0xE7, 0xF0, 0xE4,
];

const fn pro_tkl_2024(product_id: u16) -> ModelCaps {
    ModelCaps {
        product_id,
        feature_report_size: 645,
        mapping_keys: &MAPPING_KEYS,
        analog_keys: &TKL_ANALOG_KEYS,
        has_second_actuation: true,
        oled: OledImage::ColumnPacked,
        macro_data_size: 9600,
        lighting_keys: &TKL_2024_LIGHTING_KEYS,
        live_analog: false,
        profile_read: true,
    }
}

const fn pro_mini(product_id: u16) -> ModelCaps {
    ModelCaps {
        product_id,
        feature_report_size: 513,
        mapping_keys: &MINI_KEYS,
        analog_keys: &MINI_KEYS,
        has_second_actuation: true,
        oled: OledImage::None,
        macro_data_size: 9600,
        lighting_keys: &MINI_LIGHTING_KEYS,
        live_analog: false,
        profile_read: false,
    }
}

/// Every wired Gen 3 board that runs on [`ApexProTkl2023`](super::apex_pro_tkl_2023::ApexProTkl2023) and has a
/// spec behind it. A product ID missing here keeps only what its [`KeyboardModel`](super::models::KeyboardModel)
/// row gives it.
pub const MODEL_CAPS: &[ModelCaps] = &[
    // Wired Apex Pro TKL 2023, confirmed on hardware (see the rules file for what).
    ModelCaps {
        product_id: APEX_PRO_TKL_2023,
        feature_report_size: 645,
        mapping_keys: &MAPPING_KEYS,
        analog_keys: &TKL_ANALOG_KEYS,
        has_second_actuation: true,
        oled: OledImage::RowMajor,
        macro_data_size: 9600,
        lighting_keys: WIRED_2023_KEYS,
        live_analog: true,
        profile_read: true,
    },
    pro_tkl_2024(APEX_PRO_TKL_2024),
    pro_tkl_2024(APEX_PRO_TKL_2024_CYBER),
    pro_tkl_2024(APEX_PRO_TKL_2024_STEEL),
    pro_mini(APEX_PRO_MINI),
    pro_mini(APEX_PRO_MINI_2024),
    // Apex 9 TKL: 104 idx entries (97 keys), 98 firmware analog slots, but no second layer and no analog
    // actuation; the spec only defines the reset-all `38 61` frame and rapid tap.
    ModelCaps {
        product_id: APEX_9_TKL,
        feature_report_size: 513,
        mapping_keys: &MAPPING_KEYS,
        analog_keys: &[],
        has_second_actuation: false,
        oled: OledImage::None,
        macro_data_size: 9600,
        lighting_keys: WIRED_2023_KEYS,
        live_analog: false,
        profile_read: false,
    },
    ModelCaps {
        product_id: APEX_9_MINI,
        feature_report_size: 513,
        mapping_keys: &MINI_KEYS,
        analog_keys: &[],
        has_second_actuation: false,
        oled: OledImage::None,
        macro_data_size: 9600,
        lighting_keys: &MINI_LIGHTING_KEYS,
        live_analog: false,
        profile_read: false,
    },
];

/// The capability row of a product ID, or `None` when no spec-derived row exists.
#[must_use]
pub fn model_caps(product_id: u16) -> Option<&'static ModelCaps> {
    MODEL_CAPS.iter().find(|caps| caps.product_id == product_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::keyboards::apex_pro_tkl_2023::WIRED_2023_PROFILE_SLOTS;

    fn caps(product_id: u16) -> &'static ModelCaps {
        model_caps(product_id).unwrap_or_else(|| panic!("no row for {product_id:#06x}"))
    }

    fn sorted(keys: &[u8]) -> bool {
        keys.windows(2).all(|pair| pair[0] < pair[1])
    }

    #[test]
    fn the_spec_numbers_of_every_model() {
        // (pid, feature size, mapping keys, analog keys, second layer, OLED, lighting keys)
        let spec = [
            (0x1628, 645, 91, 68, true, OledImage::RowMajor, 84),
            (0x1642, 645, 91, 68, true, OledImage::ColumnPacked, 85),
            (0x1660, 645, 91, 68, true, OledImage::ColumnPacked, 85),
            (0x1662, 645, 91, 68, true, OledImage::ColumnPacked, 85),
            (0x161E, 513, 68, 68, true, OledImage::None, 61),
            (0x1648, 513, 68, 68, true, OledImage::None, 61),
            (0x1634, 513, 91, 0, false, OledImage::None, 84),
            (0x1620, 513, 68, 0, false, OledImage::None, 61),
        ];
        assert_eq!(MODEL_CAPS.len(), spec.len());
        for (pid, size, mapping, analog, second, oled, lighting) in spec {
            let row = caps(pid);
            assert_eq!(row.feature_report_size, size, "{pid:#06x}");
            assert_eq!(row.mapping_keys.len(), mapping, "{pid:#06x}");
            assert_eq!(row.analog_keys.len(), analog, "{pid:#06x}");
            assert_eq!(row.has_second_actuation, second, "{pid:#06x}");
            assert_eq!(row.oled, oled, "{pid:#06x}");
            assert_eq!(row.lighting_keys.len(), lighting, "{pid:#06x}");
            assert_eq!(row.macro_data_size, 9600, "{pid:#06x}");
        }
    }

    #[test]
    fn key_lists_are_sorted_unique_and_analog_is_a_subset_of_mapping() {
        for row in MODEL_CAPS {
            assert!(sorted(row.mapping_keys), "{:#06x} mapping", row.product_id);
            assert!(sorted(row.analog_keys), "{:#06x} analog", row.product_id);
            assert!(
                row.analog_keys.iter().all(|k| row.mapping_keys.contains(k)),
                "{:#06x}",
                row.product_id
            );
            let mut lighting = row.lighting_keys.to_vec();
            lighting.sort_unstable();
            lighting.dedup();
            assert_eq!(
                lighting.len(),
                row.lighting_keys.len(),
                "{:#06x} lighting",
                row.product_id
            );
        }
    }

    #[test]
    fn the_apex_9_boards_have_no_analog_keys_and_no_second_layer() {
        for pid in [APEX_9_TKL, APEX_9_MINI] {
            assert!(caps(pid).analog_keys.is_empty());
            assert!(!caps(pid).has_second_actuation);
            assert!(!caps(pid).live_analog);
        }
    }

    #[test]
    fn the_pro_tkl_family_shares_the_0x1628_key_lists() {
        let mut slots = WIRED_2023_PROFILE_SLOTS;
        slots.sort_unstable();
        assert_eq!(TKL_ANALOG_KEYS, slots);
        for pid in [
            APEX_PRO_TKL_2023,
            APEX_PRO_TKL_2024,
            APEX_PRO_TKL_2024_CYBER,
            APEX_PRO_TKL_2024_STEEL,
        ] {
            assert_eq!(caps(pid).mapping_keys, MAPPING_KEYS);
            assert_eq!(caps(pid).analog_keys, TKL_ANALOG_KEYS);
        }
        assert_eq!(caps(APEX_9_TKL).mapping_keys, MAPPING_KEYS);
    }

    #[test]
    fn the_2024_layout_is_the_0x1628_layout_plus_the_play_pause_key() {
        let mut layout_2024 = TKL_2024_LIGHTING_KEYS.to_vec();
        layout_2024.sort_unstable();
        let mut expected = WIRED_2023_KEYS.to_vec();
        expected.push(0xFB);
        expected.sort_unstable();
        assert_eq!(layout_2024, expected);
    }

    #[test]
    fn the_mini_layout_lights_only_mapped_keys() {
        assert!(MINI_LIGHTING_KEYS.iter().all(|k| MINI_KEYS.contains(k)));
        assert!(MINI_KEYS.contains(&0xF0));
    }

    #[test]
    fn only_the_0x1628_has_the_live_analog_table_and_only_the_645_boards_read_profiles() {
        for row in MODEL_CAPS {
            assert_eq!(
                row.live_analog,
                row.product_id == APEX_PRO_TKL_2023,
                "{:#06x}",
                row.product_id
            );
            assert_eq!(row.profile_read, row.wide_reports(), "{:#06x}", row.product_id);
        }
    }
}
