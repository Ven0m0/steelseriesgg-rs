//! Keyboard models: one row per keyboard product ID. A row decides the class discovery builds, the
//! registry name, the raw-hidraw fallback, and the `[EXPERIMENTAL]` direct lighting frame. Headsets and
//! mice keep their own tables (`headsets::nova7`, `mice::modern`).

use super::apex_pro_tkl_2023::{
    APEX_9_FRAME, APEX_9_GEN3_FRAME, GEN1_FRAME, GEN3_FRAME, WIRED_2023_FRAME, WIRELESS_FRAME,
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
/// key table, and whether the `0x4B` init report precedes the first frame.
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
    pub class: KeyboardClass,
    /// Wireless models that hidapi may fail to open; discovery then builds them on raw hidraw
    /// feature reports instead.
    pub raw_fallback: bool,
    pub direct: Option<DirectLighting>,
}

const fn model(
    product_id: u16,
    name: &'static str,
    class: KeyboardClass,
    raw_fallback: bool,
    direct: Option<DirectLighting>,
) -> KeyboardModel {
    KeyboardModel {
        product_id,
        name,
        class,
        raw_fallback,
        direct,
    }
}

const GEN1: Option<DirectLighting> = Some(DirectLighting::Fixed(GEN1_FRAME));
const GEN3: Option<DirectLighting> = Some(DirectLighting::Fixed(GEN3_FRAME));
const WIRELESS: Option<DirectLighting> = Some(DirectLighting::Fixed(WIRELESS_FRAME));
const WIRED_2023: Option<DirectLighting> = Some(DirectLighting::Fixed(WIRED_2023_FRAME));
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
    model(APEX_3, "Apex 3", C::Apex3, false, None),
    model(APEX_5, "Apex 5", C::ApexProTkl2023, false, GEN1),
    model(APEX_PRO_MINI, "Apex Pro Mini", C::Generic, false, None),
    model(APEX_9_MINI, "Apex 9 Mini", C::ApexProTkl2023, false, APEX_9),
    model(APEX_3_TKL, "Apex 3 TKL", C::Apex3Tkl, false, None),
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
    ),
    model(
        APEX_PRO_TKL_2023_WIRELESS_2,
        "Apex Pro TKL (2023) Wireless",
        C::ApexProTkl2023,
        true,
        WIRELESS,
    ),
    model(
        APEX_PRO_TKL_2023_WIRELESS,
        "Apex Pro TKL (2023) Wireless",
        C::ApexProTkl2023,
        true,
        WIRELESS,
    ),
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
    model(APEX_PRO_MINI_2024, "Apex Pro Mini (2024)", C::Generic, false, None),
    model(APEX_5_2024, "Apex 5 (2024)", C::Generic, false, None),
    model(APEX_7_2024, "Apex 7 (2024)", C::Generic, false, None),
    model(APEX_M750, "Apex M750", C::ApexM750, false, None),
];

#[must_use]
pub fn keyboard_model(product_id: u16) -> Option<&'static KeyboardModel> {
    KEYBOARD_MODELS.iter().find(|m| m.product_id == product_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_product_id_has_one_row() {
        let mut seen = std::collections::HashSet::new();
        for m in KEYBOARD_MODELS {
            assert!(seen.insert(m.product_id), "{:#06x} listed twice", m.product_id);
        }
    }

    #[test]
    fn raw_fallback_is_the_four_wireless_apex_pro_tkl_ids() {
        let raw: Vec<u16> = KEYBOARD_MODELS
            .iter()
            .filter(|m| m.raw_fallback)
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
