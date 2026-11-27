//! `[EXPERIMENTAL]` Per-model capability table of the Gen 3 wired Apex boards that share the `0x1628` code path
//! ([`ApexProTkl2023`](super::apex_pro_tkl_2023::ApexProTkl2023)).
//!
//! The numbers and key lists are derived from GG's decrypted device specs (`apex_pro_tkl_2022`,
//! `apex_pro_tkl_2024`, `apex_pro_tkl_2024_cyber`, `apex_pro_tkl_2024_steel`, `apex_pro_mini`, `apex_pro_mini_2024`,
//! `apex_9_tkl`, `apex_9_mini`, all on `base_apex_2022_wired`; read 2026-11-11). Only `0x1628` is confirmed on
//! hardware; every other row is untested. A feature a row does not list is refused rather than sent in another
//! board's shape: the commands in `hid_reports` are sized for a 645-byte feature report, so a 513-byte board gets
//! only its direct lighting frame until those commands take a report size. The `00 41` release is allowed only
//! on the hardware-confirmed `0x1628`.

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
    /// `device-analog-hid-codes`: keys of the live per-key `38 61` table, sorted; empty on the Apex 9 boards, which
    /// have no such table (their per-key sensitivity lives in the profile; the spec's num-analog-keys equals the
    /// mapping-key count).
    pub analog_keys: &'static [u8],
    /// `ssgg actuation` can set a second actuation point per key (the spec's `has-second-actuation-layer`). From
    /// GG's device specs; `[EXPERIMENTAL]` on every board (layer `02` untested).
    pub can_use_second_actuation_layer: bool,
    pub oled: OledImage,
    /// `macro-data-size` in bytes.
    pub macro_data_size: usize,
    /// The usages of the spec's prism-sync layout (`hidcode-matrix-layout-nonvariant`), i.e. the keys GG lights.
    /// Not necessarily the frame `ssgg` sends: the 2024 TKLs and the Apex 9 TKL send the 112-key `TKL_KEYS` frame.
    /// US layout only on the Mini and `0x1628` tables.
    pub spec_lighting_keys: &'static [u8],
    /// `ssgg actuation` can change the actuation point of the keys at once, without writing a profile. Needs the
    /// `0x1628` hall-threshold table (`ACTUATION_POINTS`): confirmed on the Apex Pro TKL 2023 `0x1628` (layer 0,
    /// Windows, 2026-10-25). `false` on the 2024 boards, which have their own tables.
    pub can_set_actuation_live: bool,
    /// `ssgg` can read the profiles stored in the keyboard (`83 01`, 12,500 bytes per slot). Confirmed on the Apex
    /// Pro TKL 2023 `0x1628` (Windows, 2026-11-10); `[EXPERIMENTAL]` on the 2024 TKLs, whose profile layout matches
    /// only on matching firmware, so their read is allowed and not interpreted.
    pub can_read_profile: bool,
    /// `ssgg` can hand the lighting back to the keyboard: after `ssgg rgb release`, or when the daemon stops, the
    /// keyboard shows its own stored lighting again instead of the last colours ssgg sent (`00 41`). Confirmed on the
    /// Apex Pro TKL 2023 `0x1628` (Windows, 2026-11-10); `false` where the command is not tested.
    pub can_restore_onboard_lighting: bool,
    /// `ssgg keymap set` can change what a key does (`36`). Confirmed on the Apex Pro TKL 2023 `0x1628` (Windows,
    /// 2026-11-11); `false` elsewhere: the other boards may read their mappings, but an untested write stays off.
    pub can_write_key_mappings: bool,
    /// `ssgg` can write a whole profile to the keyboard's memory (`gen3-write`, `gen3-restore`). Built and gated for
    /// the Apex Pro TKL 2023 `0x1628` only, from a GG capture (2026-11-10); `[EXPERIMENTAL]`, not yet sent from ssgg
    /// on hardware.
    pub can_write_profile: bool,
    /// `ssgg` can switch the keyboard to another stored profile (`B2`, `34`). The Apex Pro TKL 2023 `0x1628` only,
    /// from a GG capture (2026-11-10); `[EXPERIMENTAL]`, not yet sent from ssgg on hardware.
    pub can_switch_profile: bool,
    /// `ssgg device meta-key` can choose the key that opens the keyboard's second key layer (`35`). The Apex Pro TKL
    /// 2023 `0x1628` only, from GG's device spec; `[EXPERIMENTAL]`, untested.
    pub can_set_meta_key: bool,
    /// `ssgg device region` can change the keyboard's region setting, which writes flash (`75`). The Apex Pro TKL
    /// 2023 `0x1628` only, from GG's device spec; `[EXPERIMENTAL]`, untested.
    pub can_write_region: bool,
    /// The `ssgg onboard gen3-*` and `ssgg keymap` commands look for this keyboard. Only the Apex Pro TKL 2023
    /// `0x1628`, where they were built and tested; the other boards' reads are in the library only.
    pub can_run_onboard_commands: bool,
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
        can_use_second_actuation_layer: true,
        oled: OledImage::ColumnPacked,
        macro_data_size: 9600,
        spec_lighting_keys: &TKL_2024_LIGHTING_KEYS,
        can_set_actuation_live: false,
        can_read_profile: true,
        can_restore_onboard_lighting: false,
        can_write_key_mappings: false,
        can_write_profile: false,
        can_switch_profile: false,
        can_set_meta_key: false,
        can_write_region: false,
        can_run_onboard_commands: false,
    }
}

const fn pro_mini(product_id: u16) -> ModelCaps {
    ModelCaps {
        product_id,
        feature_report_size: 513,
        mapping_keys: &MINI_KEYS,
        analog_keys: &MINI_KEYS,
        can_use_second_actuation_layer: true,
        oled: OledImage::None,
        macro_data_size: 9600,
        spec_lighting_keys: &MINI_LIGHTING_KEYS,
        can_set_actuation_live: false,
        can_read_profile: false,
        can_restore_onboard_lighting: false,
        can_write_key_mappings: false,
        can_write_profile: false,
        can_switch_profile: false,
        can_set_meta_key: false,
        can_write_region: false,
        can_run_onboard_commands: false,
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
        can_use_second_actuation_layer: true,
        oled: OledImage::RowMajor,
        macro_data_size: 9600,
        spec_lighting_keys: WIRED_2023_KEYS,
        can_set_actuation_live: true,
        can_read_profile: true,
        can_restore_onboard_lighting: true,
        can_write_key_mappings: true,
        can_write_profile: true,
        can_switch_profile: true,
        can_set_meta_key: true,
        can_write_region: true,
        can_run_onboard_commands: true,
    },
    pro_tkl_2024(APEX_PRO_TKL_2024),
    pro_tkl_2024(APEX_PRO_TKL_2024_CYBER),
    pro_tkl_2024(APEX_PRO_TKL_2024_STEEL),
    pro_mini(APEX_PRO_MINI),
    pro_mini(APEX_PRO_MINI_2024),
    // Apex 9 TKL: 104 idx entries (97 keys), 98 firmware analog slots, but no second layer and no live per-key
    // `38 61` table; the spec only defines the reset-all `38 61` frame and rapid tap.
    ModelCaps {
        product_id: APEX_9_TKL,
        feature_report_size: 513,
        mapping_keys: &MAPPING_KEYS,
        analog_keys: &[],
        can_use_second_actuation_layer: false,
        oled: OledImage::None,
        macro_data_size: 9600,
        spec_lighting_keys: WIRED_2023_KEYS,
        can_set_actuation_live: false,
        can_read_profile: false,
        can_restore_onboard_lighting: false,
        can_write_key_mappings: false,
        can_write_profile: false,
        can_switch_profile: false,
        can_set_meta_key: false,
        can_write_region: false,
        can_run_onboard_commands: false,
    },
    ModelCaps {
        product_id: APEX_9_MINI,
        feature_report_size: 513,
        mapping_keys: &MINI_KEYS,
        analog_keys: &[],
        can_use_second_actuation_layer: false,
        oled: OledImage::None,
        macro_data_size: 9600,
        spec_lighting_keys: &MINI_LIGHTING_KEYS,
        can_set_actuation_live: false,
        can_read_profile: false,
        can_restore_onboard_lighting: false,
        can_write_key_mappings: false,
        can_write_profile: false,
        can_switch_profile: false,
        can_set_meta_key: false,
        can_write_region: false,
        can_run_onboard_commands: false,
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

    /// The capabilities that only the wired Apex Pro TKL 2023 has today. When a test on another board confirms one,
    /// set it in that board's row and update this list.
    #[test]
    fn the_hardware_confirmed_capabilities_are_set_on_the_0x1628_only() {
        for caps in MODEL_CAPS {
            let wired_2023 = caps.product_id == APEX_PRO_TKL_2023;
            for (name, value) in [
                ("can_set_actuation_live", caps.can_set_actuation_live),
                ("can_restore_onboard_lighting", caps.can_restore_onboard_lighting),
                ("can_write_key_mappings", caps.can_write_key_mappings),
                ("can_write_profile", caps.can_write_profile),
                ("can_switch_profile", caps.can_switch_profile),
                ("can_set_meta_key", caps.can_set_meta_key),
                ("can_write_region", caps.can_write_region),
                ("can_run_onboard_commands", caps.can_run_onboard_commands),
            ] {
                assert_eq!(value, wired_2023, "{name} on {:#06x}", caps.product_id);
            }
        }
    }

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
            assert_eq!(row.can_use_second_actuation_layer, second, "{pid:#06x}");
            assert_eq!(row.oled, oled, "{pid:#06x}");
            assert_eq!(row.spec_lighting_keys.len(), lighting, "{pid:#06x}");
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
            let mut lighting = row.spec_lighting_keys.to_vec();
            lighting.sort_unstable();
            lighting.dedup();
            assert_eq!(
                lighting.len(),
                row.spec_lighting_keys.len(),
                "{:#06x} lighting",
                row.product_id
            );
        }
    }

    #[test]
    fn the_apex_9_boards_have_no_live_analog_table_and_no_second_layer() {
        for pid in [APEX_9_TKL, APEX_9_MINI] {
            assert!(caps(pid).analog_keys.is_empty());
            assert!(!caps(pid).can_use_second_actuation_layer);
            assert!(!caps(pid).can_set_actuation_live);
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
    fn only_the_0x1628_has_confirmed_mapping_writes_and_clear_direct() {
        for row in MODEL_CAPS {
            assert_eq!(
                row.can_write_key_mappings,
                row.product_id == APEX_PRO_TKL_2023,
                "{:#06x}",
                row.product_id
            );
            assert_eq!(
                row.can_restore_onboard_lighting,
                row.product_id == APEX_PRO_TKL_2023,
                "{:#06x}",
                row.product_id
            );
        }
    }

    #[test]
    fn only_the_0x1628_has_the_live_analog_table_and_only_the_645_boards_read_profiles() {
        for row in MODEL_CAPS {
            assert_eq!(
                row.can_set_actuation_live,
                row.product_id == APEX_PRO_TKL_2023,
                "{:#06x}",
                row.product_id
            );
            assert_eq!(row.can_read_profile, row.wide_reports(), "{:#06x}", row.product_id);
        }
    }
}
