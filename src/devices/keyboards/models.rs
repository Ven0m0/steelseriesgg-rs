//! Keyboard models: one row per keyboard product ID. A row decides the class discovery builds, the
//! registry name, the raw-hidraw fallback, and the `[EXPERIMENTAL]` direct lighting frame. Headsets and
//! mice keep their own tables (`headsets::nova7`, `mice::modern`).

use super::apex_m750::CONTROL_INTERFACE as APEX_M750_CONTROL_INTERFACE;
use super::apex_pro_tkl_2023::{
    APEX_9_FRAME, APEX_9_GEN3_FRAME, GEN1_FRAME, GEN3_FRAME, MINI_FRAME, WIRED_2023_FRAME, WIRELESS_FRAME,
};
use crate::devices::hid_reports::CommandCode;
use crate::devices::product_ids::*;

/// The keyboard class a product ID is built as by
/// [`DeviceManager::open_keyboard`](crate::devices::DeviceManager::open_keyboard).
///
/// The class is what answers the on-board profile protocol: [`Keyboard::onboard_codec`](super::Keyboard)
/// is a property of the class discovery built, so a keyboard only reads a profile off the board when its
/// product ID is built as the class that speaks the protocol. `Apex7` is the on-board class, and it is
/// built for the board the frames are known from and for nothing else - the `0x1614` / `0x1616` /
/// `0x1618` platform siblings keep lighting only and answer no codec, because nobody has read an answer
/// from them. `0x1614` and `0x1618` are built as `ApexProTkl2023` for their `[EXPERIMENTAL]` direct
/// lighting frame, which answers no codec either.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardClass {
    Apex3,
    Apex3Tkl,
    ApexProTkl2023,
    Apex7,
    ApexM750,
    Generic,
}

/// [EXPERIMENTAL] A direct-mode lighting frame: packet id, report length including the report-ID byte,
/// key table, and whether the `0x4B` init report precedes the first frame (a frame fact, not a capability; the
/// init comes from OpenRGB's Gen 3 `SendInitialization`, 2026-10-25). The frames confirmed on hardware are listed in
/// `.claude/rules/experimental-protocol.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectFrame {
    pub packet_id: CommandCode,
    pub length: usize,
    pub keys: &'static [u8],
    pub init: bool,
}

/// [EXPERIMENTAL] Which direct frame a model sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectLighting {
    Fixed(DirectFrame),
    /// Picked by a firmware probe, as OpenRGB's `SendInitialization` does: `gen3` from firmware 1.19.7,
    /// `legacy` before it or when the probe fails.
    ByFirmware {
        legacy: DirectFrame,
        gen3: DirectFrame,
    },
}

impl DirectLighting {
    #[must_use]
    pub fn frame(self, gen3_firmware: bool) -> DirectFrame {
        match self {
            Self::Fixed(frame) => frame,
            Self::ByFirmware { legacy, gen3 } => {
                if gen3_firmware {
                    gen3
                } else {
                    legacy
                }
            }
        }
    }

    /// `ssgg` sends the `90 00` firmware poll before the first direct frame, to choose between the two frames (a
    /// connection fact, not a capability). From OpenRGB `SendInitialization` (2026-10-25); `[EXPERIMENTAL]`, untested.
    #[must_use]
    pub fn probes_firmware(self) -> bool {
        matches!(self, Self::ByFirmware { .. })
    }
}

/// One keyboard product ID.
#[derive(Debug, Clone, Copy)]
pub struct KeyboardModel {
    pub product_id: u16,
    pub name: &'static str,
    /// The driver class discovery builds for this keyboard; it decides which protocol answers (see
    /// [`KeyboardClass`]).
    pub class: KeyboardClass,
    /// Discovery can open this keyboard through raw hidraw feature reports when hidapi fails to open it. Set on
    /// the wireless models, where hidapi may fail; `[EXPERIMENTAL]`.
    pub can_open_through_raw_hidraw: bool,
    /// The direct-mode lighting frame that `ssgg rgb` streams to this keyboard, or `None` where it uses the older
    /// zone commands. `[EXPERIMENTAL]` except where `.claude/rules/experimental-protocol.md` records a hardware test.
    pub direct: Option<DirectLighting>,
    /// The only USB interface that takes this keyboard's lighting commands; discovery ignores every other
    /// interface. `None` on most boards, which discovery ranks by usage page. Apex M750: 2 (community fork,
    /// `[EXPERIMENTAL]`).
    pub only_control_interface: Option<i32>,
    /// The interface discovery picks when the keyboard shows no vendor-defined usage page: 1 on most boards, 3 on
    /// the Apex Pro TKL 2023 Wireless (`0x1630`, `0x1632`). Both values come from the older per-model table;
    /// `[EXPERIMENTAL]`.
    pub fallback_control_interface: i32,
    /// The keyboard takes one colour for all its zones in one command (zone selector `0xFF`). `false` on the Apex
    /// Pro TKL 2023 Wireless (`0x1630`, `0x1632`), which needs one command per zone, zones counted from 1 (code
    /// comment from an earlier contributor). `[EXPERIMENTAL]` on every board: no hardware test of either form since
    /// the legacy output path was fixed (2026-10-25).
    pub can_set_all_zones_at_once: bool,
    /// How many lighting zones the zone commands and the daemon address: 10 on the
    /// Apex 3, 8 on the Apex 3 TKL (OpenRGB), 9 on the Apex Pro TKL 2023 wired and wireless, 1 elsewhere, where the
    /// count is not captured. `[EXPERIMENTAL]`, from the older per-model table.
    pub zone_count: usize,
    /// `false` where the per-key key table holds only US keys and the layout cannot be queried, so `ssgg` warns
    /// that ISO, UK and JP keys may stay dark: the Mini boards (GG's specs, `[EXPERIMENTAL]`). `true` does not mean
    /// a non-US layout was tested: that is confirmed only on the Apex Pro (2024) `0x1640` ISO-DE (Linux,
    /// 2026-11-02), and the `0x1628` table is US-only too.
    pub can_light_non_us_layouts: bool,
}

const fn model(
    product_id: u16,
    name: &'static str,
    class: KeyboardClass,
    can_open_through_raw_hidraw: bool,
    direct: Option<DirectLighting>,
) -> KeyboardModel {
    KeyboardModel {
        product_id,
        name,
        class,
        can_open_through_raw_hidraw,
        direct,
        only_control_interface: None,
        fallback_control_interface: 1,
        can_set_all_zones_at_once: true,
        can_light_non_us_layouts: true,
        zone_count: 1,
    }
}

impl KeyboardModel {
    const fn only_control_interface(mut self, interface: i32) -> Self {
        self.only_control_interface = Some(interface);
        self
    }

    const fn fallback_control_interface(mut self, interface: i32) -> Self {
        self.fallback_control_interface = interface;
        self
    }

    const fn zones_one_by_one(mut self) -> Self {
        self.can_set_all_zones_at_once = false;
        self
    }

    const fn zones(mut self, count: usize) -> Self {
        self.zone_count = count;
        self
    }

    const fn us_layout_lighting_only(mut self) -> Self {
        self.can_light_non_us_layouts = false;
        self
    }
}

const GEN1: Option<DirectLighting> = Some(DirectLighting::Fixed(GEN1_FRAME));
const GEN3: Option<DirectLighting> = Some(DirectLighting::Fixed(GEN3_FRAME));
const WIRELESS: Option<DirectLighting> = Some(DirectLighting::Fixed(WIRELESS_FRAME));
const WIRED_2023: Option<DirectLighting> = Some(DirectLighting::Fixed(WIRED_2023_FRAME));
const MINI: Option<DirectLighting> = Some(DirectLighting::Fixed(MINI_FRAME));
const GEN1_OR_GEN3: Option<DirectLighting> = Some(DirectLighting::ByFirmware {
    legacy: GEN1_FRAME,
    gen3: GEN3_FRAME,
});
const APEX_9: Option<DirectLighting> = Some(DirectLighting::ByFirmware {
    legacy: APEX_9_FRAME,
    gen3: APEX_9_GEN3_FRAME,
});

use KeyboardClass as C;

/// Every keyboard product ID ssgg knows. A PID missing here is not a keyboard to discovery.
pub const KEYBOARD_MODELS: &[KeyboardModel] = &[
    model(APEX_PRO, "Apex Pro", C::ApexProTkl2023, false, GEN1),
    model(APEX_7, "Apex 7", C::Apex7, false, None),
    model(APEX_PRO_TKL, "Apex Pro TKL", C::ApexProTkl2023, false, GEN1_OR_GEN3),
    model(APEX_150, "Apex 150", C::Generic, false, None),
    model(APEX_7_TKL, "Apex 7 TKL", C::ApexProTkl2023, false, GEN1),
    model(APEX_3, "Apex 3", C::Apex3, false, None).zones(10),
    model(APEX_5, "Apex 5", C::ApexProTkl2023, false, GEN1),
    model(APEX_PRO_MINI, "Apex Pro Mini", C::ApexProTkl2023, false, MINI).us_layout_lighting_only(),
    model(APEX_9_MINI, "Apex 9 Mini", C::ApexProTkl2023, false, MINI).us_layout_lighting_only(),
    model(APEX_3_TKL, "Apex 3 TKL", C::Apex3Tkl, false, None).zones(8),
    model(
        APEX_PRO_MINI_WIRELESS_DONGLE,
        "Apex Pro Mini Wireless (Dongle)",
        C::Generic,
        false,
        None,
    ),
    model(
        APEX_PRO_MINI_WIRELESS,
        "Apex Pro Mini Wireless",
        C::Generic,
        false,
        None,
    ),
    model(
        APEX_PRO_TKL_2023,
        "Apex Pro TKL (2023)",
        C::ApexProTkl2023,
        false,
        WIRED_2023,
    )
    .zones(9),
    model(
        APEX_PRO_TKL_2023_WIRELESS_2,
        "Apex Pro TKL (2023) Wireless",
        C::ApexProTkl2023,
        true,
        WIRELESS,
    )
    .fallback_control_interface(3)
    .zones_one_by_one()
    .zones(9),
    model(
        APEX_PRO_TKL_2023_WIRELESS,
        "Apex Pro TKL (2023) Wireless",
        C::ApexProTkl2023,
        true,
        WIRELESS,
    )
    .fallback_control_interface(3)
    .zones_one_by_one()
    .zones(9),
    model(APEX_9_TKL, "Apex 9 TKL", C::ApexProTkl2023, false, APEX_9),
    model(APEX_PRO_2024, "Apex Pro (2024)", C::ApexProTkl2023, false, GEN1),
    model(APEX_PRO_TKL_2024, "Apex Pro TKL (2024)", C::ApexProTkl2023, false, GEN3),
    model(
        APEX_PRO_TKL_WIRELESS_2024_DONGLE,
        "Apex Pro TKL Wireless (2024) (Dongle)",
        C::ApexProTkl2023,
        true,
        WIRELESS,
    ),
    model(
        APEX_PRO_TKL_WIRELESS_2024,
        "Apex Pro TKL Wireless (2024)",
        C::ApexProTkl2023,
        true,
        WIRELESS,
    ),
    model(
        APEX_PRO_MINI_2024,
        "Apex Pro Mini (2024)",
        C::ApexProTkl2023,
        false,
        MINI,
    )
    .us_layout_lighting_only(),
    model(
        APEX_PRO_TKL_2024_CYBER,
        "Apex Pro TKL (2024) Cyber",
        C::ApexProTkl2023,
        false,
        GEN3,
    ),
    model(
        APEX_PRO_TKL_2024_STEEL,
        "Apex Pro TKL (2024) Steel",
        C::ApexProTkl2023,
        false,
        GEN3,
    ),
    model(APEX_5_2024, "Apex 5 (2024)", C::ApexProTkl2023, false, GEN1),
    model(APEX_7_2024, "Apex 7 (2024)", C::Generic, false, None),
    model(APEX_M750, "Apex M750", C::ApexM750, false, None).only_control_interface(APEX_M750_CONTROL_INTERFACE),
];

#[must_use]
pub fn keyboard_model(product_id: u16) -> Option<&'static KeyboardModel> {
    KEYBOARD_MODELS.iter().find(|m| m.product_id == product_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Which rows set each property today; a new row that needs one adds it to its builder call and to this list.
    #[test]
    fn the_interface_zone_and_layout_properties_are_set_on_the_expected_rows() {
        let wireless_2023 = [APEX_PRO_TKL_2023_WIRELESS, APEX_PRO_TKL_2023_WIRELESS_2];
        let minis = [APEX_PRO_MINI, APEX_PRO_MINI_2024, APEX_9_MINI];
        for m in KEYBOARD_MODELS {
            let pid = m.product_id;
            assert_eq!(m.only_control_interface, (pid == APEX_M750).then_some(2), "{pid:#06x}");
            let fallback = if wireless_2023.contains(&pid) { 3 } else { 1 };
            assert_eq!(m.fallback_control_interface, fallback, "{pid:#06x}");
            assert_eq!(m.can_set_all_zones_at_once, !wireless_2023.contains(&pid), "{pid:#06x}");
            assert_eq!(m.can_light_non_us_layouts, !minis.contains(&pid), "{pid:#06x}");
            let zones = match pid {
                APEX_3 => 10,
                APEX_3_TKL => 8,
                APEX_PRO_TKL_2023 | APEX_PRO_TKL_2023_WIRELESS | APEX_PRO_TKL_2023_WIRELESS_2 => 9,
                _ => 1,
            };
            assert_eq!(m.zone_count, zones, "{pid:#06x}");
        }
    }

    #[test]
    fn every_product_id_has_one_row() {
        let mut seen = std::collections::HashSet::new();
        for m in KEYBOARD_MODELS {
            assert!(seen.insert(m.product_id), "{:#06x} listed twice", m.product_id);
        }
    }

    #[test]
    fn raw_hidraw_opening_is_the_four_wireless_apex_pro_tkl_ids() {
        let raw: Vec<u16> = KEYBOARD_MODELS
            .iter()
            .filter(|m| m.can_open_through_raw_hidraw)
            .map(|m| m.product_id)
            .collect();
        assert_eq!(raw, [0x1630, 0x1632, 0x1644, 0x1646]);
    }

    #[test]
    fn only_the_direct_class_has_a_direct_frame() {
        for m in KEYBOARD_MODELS {
            if m.direct.is_some() {
                assert_eq!(m.class, KeyboardClass::ApexProTkl2023, "{:#06x}", m.product_id);
            }
        }
    }
}
