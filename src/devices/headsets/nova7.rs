//! Headsets using one-byte `<id> <opcode> <value>` frames: Arctis Nova 4/5/7, Nova 3, Nova Pro and Arctis 7+.
//!
//! [EXPERIMENTAL] The protocol comes from community forks and GG's decrypted device specs. Status, sidetone, mic
//! volume and auto-off are confirmed on the PIDs in [`HARDWARE_CONFIRMED`] only; every other model is untested.
//! Frames are 64-byte output reports on the vendor control collection (usage page `0xFFC0`),
//! report number 0 (6 on the Nova Pro family). The save frame `<id> 09` is never sent: it would persist
//! settings on the headset.

use super::{EqPreset, Headset, HeadsetStatus};
use crate::devices::hid_reports::{
    HEADSET_REPORT_SIZE, HeadsetBatteryQueryCommand, HeadsetSettingCommand, HeadsetStatusQueryCommand, HidCommand,
    HidDeviceType,
};
use crate::devices::{CommandCode, Device, DeviceInfo, DeviceType, product_ids::*, write_padded_report};
use crate::{Error, Result};
use hidapi::HidDevice;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How the `battery` byte of a status reply is scaled.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BatteryScale {
    /// `0..=100` already.
    Percent,
    /// `0..=4` steps of 25 percent.
    Steps4,
    /// `0..=5` steps of 20 percent (Arctis 7+ family, GG spec `battery_level` range).
    Steps5,
    /// `0..=8` steps of 12.5 percent (Nova Pro Wireless `headset_battery_level`).
    Steps8,
}

/// Which query a model answers for its status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusQuery {
    /// `00 b0`, answered `b0 power [gap] battery charge chat_game chat_chat`.
    B0,
    /// `06 b7`, answered `[06] b7 headset_battery charger_battery charging` (Nova Pro Wireless).
    ProBattery,
    /// No status query in the spec (wired Nova Pro, Nova 3).
    None,
}

/// Per-PID behaviour of one Nova 7 variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Nova7Model {
    pub product_id: u16,
    pub battery: BatteryScale,
    pub sidetone: bool,
    pub chat_mix: bool,
    /// Lowest microphone-volume wire value: 0, or 1 on the Arctis 7+ family.
    pub mic_min: u8,
    /// Highest sidetone wire value: 3, or 10 on the Nova 5 family.
    pub sidetone_max: u8,
    /// Highest microphone-volume wire value: 7, or 15 on the Nova 5 family.
    pub mic_max: u8,
    /// Bytes between the power and battery bytes of a status reply: 0, or 1 on the Nova 5 family.
    pub status_gap: usize,
    /// Report number in byte 0 of every frame: 0, or 6 on the Nova Pro family.
    pub report_id: u8,
    /// The status query the model answers.
    pub status: StatusQuery,
    /// Whether the spec has an `inactivity_timer` (`a3`) for the model.
    pub auto_off: bool,
}

const fn model(product_id: u16, battery: BatteryScale, sidetone_and_chat_mix: bool) -> Nova7Model {
    Nova7Model {
        product_id,
        battery,
        sidetone: sidetone_and_chat_mix,
        chat_mix: sidetone_and_chat_mix,
        mic_min: 0,
        sidetone_max: 3,
        mic_max: 7,
        status_gap: 0,
        report_id: 0,
        status: StatusQuery::B0,
        auto_off: true,
    }
}

/// [EXPERIMENTAL] Nova Pro family and the wired Nova 3: report number 6, sidetone `0..=3`, mic volume
/// `1..=10`, no auto-off, and a `b7` battery query on the wireless TX only, per GG's decrypted device specs
/// (`base_arctis_nova_pro`, `base_arctis_nova_pro_wireless`, `arctis-nova-3`, 2026-10-25). No ChatMix
/// frame is read: the spec's chat mix on these models is software-side. Untested on hardware.
const fn nova_pro_model(product_id: u16, wireless: bool) -> Nova7Model {
    Nova7Model {
        product_id,
        battery: BatteryScale::Steps8,
        sidetone: true,
        chat_mix: false,
        mic_min: 1,
        sidetone_max: 3,
        mic_max: 10,
        status_gap: 0,
        report_id: 0x06,
        status: if wireless {
            StatusQuery::ProBattery
        } else {
            StatusQuery::None
        },
        auto_off: false,
    }
}

/// [EXPERIMENTAL] Nova 5 / Nova 3 Wireless family: the Nova 7 frames with sidetone `0..=10`, mic volume
/// `0..=15`, and an unused byte before the percent battery in the status reply, per GG's decrypted device
/// specs (`base_arctis_nova_5_tx`, `base_arctis_nova_3_wireless_tx`, 2026-10-25). Untested on hardware.
const fn nova5_model(product_id: u16) -> Nova7Model {
    Nova7Model {
        product_id,
        battery: BatteryScale::Percent,
        sidetone: true,
        chat_mix: true,
        mic_min: 0,
        sidetone_max: 10,
        mic_max: 15,
        status_gap: 1,
        report_id: 0,
        status: StatusQuery::B0,
        auto_off: true,
    }
}

/// [EXPERIMENTAL] Arctis 7+ family: the Nova 7 frames with a 0-5 battery and mic volume 1-7, per GG's
/// decrypted device specs (`base_arctis_7_plus_tx`, 2026-10-25). Untested on hardware.
const fn plus_model(product_id: u16, sidetone: bool, chat_mix: bool) -> Nova7Model {
    Nova7Model {
        product_id,
        battery: BatteryScale::Steps5,
        sidetone,
        chat_mix,
        mic_min: 1,
        sidetone_max: 3,
        mic_max: 7,
        status_gap: 0,
        report_id: 0,
        status: StatusQuery::B0,
        auto_off: true,
    }
}

/// Every PID routed to [`Nova7Headset`]. The 7P variants have neither sidetone nor ChatMix; the 7X+ has
/// sidetone but no ChatMix.
pub const NOVA7_MODELS: &[Nova7Model] = &[
    model(ARCTIS_NOVA_7_TX, BatteryScale::Steps4, true),
    model(ARCTIS_NOVA_7X_TX, BatteryScale::Steps4, true),
    model(ARCTIS_NOVA_7P_TX, BatteryScale::Steps4, false),
    model(ARCTIS_NOVA_7_DIABLO_IV, BatteryScale::Steps4, true),
    model(ARCTIS_NOVA_7X_V2, BatteryScale::Percent, true),
    model(ARCTIS_NOVA_7_WOW, BatteryScale::Steps4, true),
    model(ARCTIS_NOVA_7_WIRELESS_GEN2, BatteryScale::Percent, true),
    model(ARCTIS_NOVA_7P_GEN2, BatteryScale::Percent, false),
    model(ARCTIS_NOVA_7X_GEN2, BatteryScale::Percent, true),
    model(ARCTIS_NOVA_7_22A1, BatteryScale::Percent, true),
    model(ARCTIS_NOVA_7X_ALT, BatteryScale::Steps4, true),
    model(ARCTIS_NOVA_7X_ALT_V2, BatteryScale::Percent, true),
    model(ARCTIS_NOVA_7P_V2, BatteryScale::Percent, false),
    model(ARCTIS_NOVA_7_DIABLO_IV_V2, BatteryScale::Percent, true),
    model(ARCTIS_NOVA_7X_V2_ALT, BatteryScale::Percent, true),
    plus_model(ARCTIS_7_PLUS, true, true),
    plus_model(ARCTIS_7P_PLUS, false, false),
    plus_model(ARCTIS_7X_PLUS, true, false),
    plus_model(ARCTIS_7_PLUS_DESTINY_2, true, true),
    // GG specs `arctis_nova_4_tx` / `arctis_nova_4x_tx`: byte-identical to the Nova 7 TX.
    model(ARCTIS_NOVA_4, BatteryScale::Steps4, true),
    model(ARCTIS_NOVA_4X, BatteryScale::Steps4, true),
    // GG spec `arctis_nova_7_wow_upgrade_tx` (`base_arctis_nova_7_gen2_tx`): percent battery.
    model(ARCTIS_NOVA_7_WOW_UPGRADE, BatteryScale::Percent, true),
    nova5_model(ARCTIS_NOVA_5_TX),
    nova5_model(ARCTIS_NOVA_5X),
    nova5_model(ARCTIS_NOVA_5X_WHITE),
    nova5_model(ARCTIS_NOVA_3_WIRELESS_TX),
    nova5_model(ARCTIS_NOVA_3X_WIRELESS),
    nova_pro_model(ARCTIS_NOVA_PRO_WIRED, false),
    nova_pro_model(ARCTIS_NOVA_PRO_XBOX, false),
    nova_pro_model(ARCTIS_NOVA_PRO_V2, false),
    nova_pro_model(ARCTIS_NOVA_PRO, true),
    nova_pro_model(ARCTIS_NOVA_PRO_WIRELESS_XBOX, true),
    nova_pro_model(ARCTIS_NOVA_PRO_WIRELESS_XBOX_WHITE, true),
    nova_pro_model(ARCTIS_NOVA_3, false),
];

/// PIDs on which status, sidetone, mic volume and auto-off were sent from `ssgg` on hardware and worked: Arctis
/// Nova 7 Gen 2, Linux, 2026-10-25 (`.claude/rules/experimental-protocol.md`).
pub const HARDWARE_CONFIRMED: &[u16] = &[ARCTIS_NOVA_7_WIRELESS_GEN2];

/// Look up the Nova 7 behaviour for a product ID; `None` for every other headset.
pub fn nova7_model(product_id: u16) -> Option<&'static Nova7Model> {
    NOVA7_MODELS.iter().find(|m| m.product_id == product_id)
}

/// Status replies are `b0 power [gap] battery charge chat_game chat_chat`.
const STATUS_REPLY_LEN: usize = 6;
const STATUS_POLL_INTERVAL: Duration = Duration::from_millis(100);
const STATUS_MAX_QUERIES: usize = 10;

fn unsupported(what: &str, info: &DeviceInfo) -> Error {
    Error::DeviceCommunication(format!(
        "{what} is not supported by {} [EXPERIMENTAL protocol has no such command]",
        info.name
    ))
}

/// Map a 0-100 UI level onto `0..=max`, rounding to nearest.
fn scale_to_wire(value: u8, max: u8, what: &str) -> Result<u8> {
    if value > 100 {
        return Err(Error::InvalidConfig(format!("{what} {value} out of range (0-100)")));
    }
    let scaled = (u16::from(value) * u16::from(max) + 50) / 100;
    Ok(u8::try_from(scaled).unwrap_or(max))
}

/// Decode a status reply as returned by hidapi (no report ID).
pub fn decode_status(model: &Nova7Model, reply: &[u8]) -> Result<HeadsetStatus> {
    let need = STATUS_REPLY_LEN + model.status_gap;
    if reply.len() < need {
        return Err(Error::DeviceCommunication(format!(
            "Headset status reply too short: {} bytes (expected at least {need})",
            reply.len()
        )));
    }
    let body = &reply[model.status_gap..];
    let (power, battery, charge, game, chat) = (reply[1], body[2], body[3], body[4], body[5]);
    let connected = power == 3 && charge != 0;
    let battery_percent = connected.then(|| match model.battery {
        BatteryScale::Percent => battery.min(100),
        BatteryScale::Steps4 => battery.min(4) * 25,
        BatteryScale::Steps5 => battery.min(5) * 20,
        BatteryScale::Steps8 => steps8_percent(battery),
    });
    let chat_mix = model
        .chat_mix
        .then(|| ((f32::from(chat.min(100)) - f32::from(game.min(100))) / 100.0).clamp(-1.0, 1.0));
    Ok(HeadsetStatus {
        connected: Some(connected),
        charging: Some(matches!(charge, 1 | 2)),
        battery_percent,
        chat_mix,
    })
}

fn steps8_percent(level: u8) -> u8 {
    // At most 8 * 100 / 8 = 100, so the cast cannot truncate.
    (u16::from(level.min(8)) * 100 / 8) as u8
}

/// Decode a Nova Pro Wireless `b7` battery reply, the slice starting at `b7`. The charging byte's values
/// (`1 2 4 8`) are not explained by the spec, so charging and connection are left unknown.
pub fn decode_pro_battery(reply: &[u8]) -> Result<HeadsetStatus> {
    let level = *reply
        .get(1)
        .ok_or_else(|| Error::DeviceCommunication(format!("Headset battery reply too short: {} bytes", reply.len())))?;
    Ok(HeadsetStatus {
        connected: None,
        charging: None,
        battery_percent: Some(steps8_percent(level)),
        chat_mix: None,
    })
}

/// The part of an input report that starts at `opcode`: hidapi keeps the report number in front of a
/// numbered report (`06 b7 ...`) and drops it for report number 0 (`b0 ...`).
fn reply_body(buf: &[u8], report_id: u8, opcode: u8) -> Option<&[u8]> {
    match buf {
        [first, ..] if *first == opcode => Some(buf),
        [id, op, ..] if report_id != 0 && *id == report_id && *op == opcode => Some(&buf[1..]),
        _ => None,
    }
}

/// [EXPERIMENTAL] Headset using the one-byte `<id> <opcode> <value>` frames (Nova 4/5/7, Nova 3,
/// Nova Pro and Arctis 7+ families).
pub struct Nova7Headset {
    info: DeviceInfo,
    device: Option<Arc<Mutex<HidDevice>>>,
    model: &'static Nova7Model,
}

impl Nova7Headset {
    pub fn new(info: DeviceInfo, device: HidDevice, model: &'static Nova7Model) -> Self {
        Self {
            info,
            device: Some(Arc::new(Mutex::new(device))),
            model,
        }
    }

    fn device(&self) -> Result<&Arc<Mutex<HidDevice>>> {
        self.device
            .as_ref()
            .ok_or(Error::DeviceCommunication("Device not connected".to_string()))
    }

    fn send_command(&self, command: &dyn HidCommand) -> Result<()> {
        let mut report = [0u8; HEADSET_REPORT_SIZE];
        let len = command.serialize(&mut report, HidDeviceType::Headset)?;
        write_padded_report(&self.device()?.lock(), &report[..len], HEADSET_REPORT_SIZE, false)
    }

    fn set_wire_value(&self, code: CommandCode, value: u8) -> Result<()> {
        self.send_command(&HeadsetSettingCommand {
            report_id: self.model.report_id,
            code,
            value,
        })
    }

    /// Query the status, re-sending the query every 100 ms and skipping unrelated input reports.
    fn query_status(&self) -> Result<HeadsetStatus> {
        let mut report = [0u8; HEADSET_REPORT_SIZE];
        let (len, opcode) = match self.model.status {
            StatusQuery::B0 => (
                HeadsetStatusQueryCommand.serialize(&mut report, HidDeviceType::Headset)?,
                CommandCode::HeadsetStatus,
            ),
            StatusQuery::ProBattery => (
                HeadsetBatteryQueryCommand {
                    report_id: self.model.report_id,
                }
                .serialize(&mut report, HidDeviceType::Headset)?,
                CommandCode::HeadsetProBattery,
            ),
            StatusQuery::None => return Err(unsupported("Status", &self.info)),
        };
        let device = self.device()?.lock();
        let mut buf = [0u8; 64];
        for _ in 0..STATUS_MAX_QUERIES {
            write_padded_report(&device, &report[..len], HEADSET_REPORT_SIZE, false)?;
            let deadline = Instant::now() + STATUS_POLL_INTERVAL;
            while let Some(left) = deadline.checked_duration_since(Instant::now()) {
                let timeout_ms = i32::try_from(left.as_millis().max(1)).unwrap_or(100);
                let read = device.read_timeout(&mut buf, timeout_ms)?;
                if let Some(body) = reply_body(&buf[..read], self.model.report_id, opcode as u8) {
                    return match self.model.status {
                        StatusQuery::ProBattery => decode_pro_battery(body),
                        _ => decode_status(self.model, body),
                    };
                }
            }
        }
        Err(Error::DeviceCommunication(format!(
            "{} did not answer the status query",
            self.info.name
        )))
    }
}

impl Device for Nova7Headset {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::Headset
    }

    fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    fn close(&mut self) -> Result<()> {
        self.device = None;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.device.is_some()
    }

    fn send_raw(&mut self, data: &[u8]) -> Result<()> {
        write_padded_report(&self.device()?.lock(), data, HEADSET_REPORT_SIZE, false)
    }

    fn receive_raw(&mut self, buf: &mut [u8]) -> Result<usize> {
        Ok(self.device()?.lock().read_timeout(buf, 1000)?)
    }
}

impl Headset for Nova7Headset {
    fn battery_level(&mut self) -> Result<Option<u8>> {
        Ok(self.query_status()?.battery_percent)
    }

    fn set_sidetone(&mut self, level: u8) -> Result<()> {
        if !self.model.sidetone {
            return Err(unsupported("Sidetone", &self.info));
        }
        let wire = scale_to_wire(level, self.model.sidetone_max, "Sidetone level")?;
        self.set_wire_value(CommandCode::HeadsetSidetone, wire)
    }

    fn set_mic_volume(&mut self, volume: u8) -> Result<()> {
        let span = self.model.mic_max - self.model.mic_min;
        let wire = self.model.mic_min + scale_to_wire(volume, span, "Microphone volume")?;
        self.set_wire_value(CommandCode::HeadsetMicVolume, wire)
    }

    fn set_mic_mute(&mut self, _muted: bool) -> Result<()> {
        Err(unsupported("Microphone mute", &self.info))
    }

    fn set_eq_preset(&mut self, _preset: EqPreset) -> Result<()> {
        Err(unsupported("EQ presets", &self.info))
    }

    fn chat_mix(&mut self) -> Result<f32> {
        if !self.model.chat_mix {
            return Err(unsupported("ChatMix", &self.info));
        }
        Ok(self.query_status()?.chat_mix.unwrap_or(0.0))
    }

    fn set_auto_off(&mut self, minutes: u8) -> Result<()> {
        if !self.model.auto_off {
            return Err(unsupported("Auto-off", &self.info));
        }
        self.set_wire_value(CommandCode::HeadsetAutoOff, minutes)
    }

    fn status(&mut self) -> Result<HeadsetStatus> {
        self.query_status()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    const PCT: Nova7Model = model(0x22A1, BatteryScale::Percent, true);
    const STEPS: Nova7Model = model(0x2202, BatteryScale::Steps4, true);
    const NO_CHAT: Nova7Model = model(0x220A, BatteryScale::Steps4, false);

    #[test]
    fn table_has_no_duplicate_pids_and_thirty_four_models() {
        let pids: HashSet<u16> = NOVA7_MODELS.iter().map(|m| m.product_id).collect();
        assert_eq!(pids.len(), NOVA7_MODELS.len());
        assert_eq!(NOVA7_MODELS.len(), 34);
    }

    // Vectors from `base_arctis_nova_pro_wireless` / `arctis_nova_pro` / `arctis-nova-3`: report id 6,
    // sidetone 0-3, mic 1-10, `b7` battery on the wireless TX only, no inactivity timer.
    #[test]
    fn nova_pro_family_rows_follow_the_spec() {
        for (pid, battery_query) in [
            (0x12CB, false),
            (0x12CD, false),
            (0x12FA, false),
            (0x12EC, false),
            (0x12E0, true),
            (0x12E5, true),
            (0x225D, true),
        ] {
            let m = nova7_model(pid).unwrap();
            assert_eq!(
                (m.report_id, m.mic_min, m.mic_max, m.sidetone_max),
                (6, 1, 10, 3),
                "{pid:#06x}"
            );
            assert!(!m.auto_off && !m.chat_mix, "{pid:#06x}");
            let expected = if battery_query {
                StatusQuery::ProBattery
            } else {
                StatusQuery::None
            };
            assert_eq!(m.status, expected, "{pid:#06x}");
        }
        assert_eq!(decode_pro_battery(&[0xB7, 4, 8, 1]).unwrap().battery_percent, Some(50));
        assert_eq!(decode_pro_battery(&[0xB7, 9]).unwrap().battery_percent, Some(100));
        assert!(decode_pro_battery(&[0xB7]).is_err());
        assert_eq!(reply_body(&[0x06, 0xB7, 4], 6, 0xB7), Some(&[0xB7, 4][..]));
        assert_eq!(reply_body(&[0xB7, 4], 6, 0xB7), Some(&[0xB7, 4][..]));
        assert_eq!(reply_body(&[0xB0, 3], 0, 0xB0), Some(&[0xB0, 3][..]));
        assert_eq!(
            reply_body(&[0x06, 0xB0], 0, 0xB0),
            None,
            "report 0 never carries an id byte"
        );
    }

    // Vectors from the `base_arctis_nova_5_tx` / `base_arctis_nova_3_wireless_tx` structs: sidetone 0-10,
    // mic 0-15, status `b0 power unused battery(0-100) charge game chat`.
    #[test]
    fn nova_5_family_rows_follow_the_spec() {
        for pid in [0x2232, 0x2253, 0x2264, 0x2269, 0x226D] {
            let m = nova7_model(pid).unwrap();
            assert_eq!(
                (m.sidetone_max, m.mic_max, m.status_gap, m.battery),
                (10, 15, 1, BatteryScale::Percent),
                "{pid:#06x}"
            );
        }
        let five = nova5_model(0x2232);
        let s = decode_status(&five, &[0xb0, 3, 0xEE, 87, 3, 100, 0, 0]).unwrap();
        assert_eq!(
            (s.connected, s.battery_percent, s.chat_mix),
            (Some(true), Some(87), Some(-1.0))
        );
        assert!(
            decode_status(&five, &[0xb0, 3, 0, 87, 3, 100]).is_err(),
            "gap makes the reply 7 bytes"
        );
        assert_eq!(scale_to_wire(100, five.sidetone_max, "s").unwrap(), 10);
        assert_eq!(scale_to_wire(100, five.mic_max, "m").unwrap(), 15);
        for m in NOVA7_MODELS {
            assert!(
                m.sidetone_max <= HeadsetSettingCommand::MAX_SIDETONE,
                "{:#06x}",
                m.product_id
            );
            assert!(
                m.mic_max <= HeadsetSettingCommand::MAX_MIC_VOLUME,
                "{:#06x}",
                m.product_id
            );
        }
    }

    #[test]
    fn arctis_7_plus_rows_follow_the_spec() {
        // (pid, sidetone api, chatmix fields in the status reply), from `base_arctis_7_plus_tx`.
        for (pid, sidetone, chat_mix) in [
            (0x220E, true, true),
            (0x2212, false, false),
            (0x2216, true, false),
            (0x2236, true, true),
        ] {
            let m = nova7_model(pid).unwrap();
            assert_eq!(
                (m.sidetone, m.chat_mix, m.mic_min),
                (sidetone, chat_mix, 1),
                "{pid:#06x}"
            );
            assert_eq!(m.battery, BatteryScale::Steps5);
        }
        let plus = plus_model(0x220E, true, true);
        for (level, percent) in [(0, 0), (1, 20), (5, 100), (9, 100)] {
            let s = decode_status(&plus, &[0xb0, 3, level, 3, 0, 0]).unwrap();
            assert_eq!(s.battery_percent, Some(percent));
        }
        let span = plus.mic_max - plus.mic_min;
        let mic = |v| plus.mic_min + scale_to_wire(v, span, "m").unwrap();
        assert_eq!([mic(0), mic(50), mic(100)], [1, 4, 7], "spec range 1-7");
    }

    #[test]
    fn table_attributes_match_the_brief() {
        let steps4 = [0x2202, 0x2206, 0x220A, 0x223A, 0x227A, 0x22A4, 0x12F2, 0x12F6];
        let no_extras = [0x220A, 0x2298, 0x22A7];
        for m in NOVA7_MODELS
            .iter()
            .filter(|m| !matches!(m.battery, BatteryScale::Steps5 | BatteryScale::Steps8))
        {
            assert_eq!(
                m.battery == BatteryScale::Steps4,
                steps4.contains(&m.product_id),
                "{:#06x}",
                m.product_id
            );
            assert_eq!(m.sidetone, !no_extras.contains(&m.product_id), "{:#06x}", m.product_id);
            assert_eq!(m.chat_mix, m.sidetone, "{:#06x}", m.product_id);
        }
    }

    #[test]
    fn hardware_confirmed_pids_are_in_the_table() {
        for pid in HARDWARE_CONFIRMED {
            assert!(nova7_model(*pid).is_some(), "{pid:#06x}");
        }
    }

    #[test]
    fn lookup_covers_the_family_and_nothing_else() {
        for m in NOVA7_MODELS {
            assert_eq!(nova7_model(m.product_id), Some(m));
        }
        assert!(nova7_model(ARCTIS_NOVA_7_RX).is_none());
        assert!(nova7_model(ARCTIS_NOVA_PRO_OMNI).is_none());
        assert!(nova7_model(ARCTIS_PRO).is_none(), "no settings apis in its spec");
        assert!(nova7_model(ARCTIS_7_2019).is_none(), "a bootloader id");
    }

    #[test]
    fn sidetone_and_mic_mapping_edges() {
        let side = |v| scale_to_wire(v, STEPS.sidetone_max, "s").unwrap();
        assert_eq!(
            [side(0), side(16), side(17), side(50), side(83), side(84), side(100)],
            [0, 0, 1, 2, 2, 3, 3]
        );
        let mic = |v| scale_to_wire(v, STEPS.mic_max, "m").unwrap();
        assert_eq!([mic(0), mic(7), mic(8), mic(50), mic(100)], [0, 0, 1, 4, 7]);
        assert!(scale_to_wire(101, 3, "s").is_err());
    }

    #[test]
    fn status_offline_and_charging() {
        let offline = decode_status(&PCT, &[0xb0, 2, 80, 3, 50, 50]).unwrap();
        assert_eq!(offline.connected, Some(false));
        assert_eq!(offline.battery_percent, None);
        let no_charge = decode_status(&PCT, &[0xb0, 3, 80, 0, 50, 50]).unwrap();
        assert_eq!(no_charge.connected, Some(false));

        let charging = decode_status(&PCT, &[0xb0, 3, 80, 1, 50, 50]).unwrap();
        assert_eq!((charging.connected, charging.charging), (Some(true), Some(true)));
        let full = decode_status(&PCT, &[0xb0, 3, 80, 2, 50, 50]).unwrap();
        assert_eq!(full.charging, Some(true));
        let idle = decode_status(&PCT, &[0xb0, 3, 80, 3, 50, 50]).unwrap();
        assert_eq!((idle.connected, idle.charging), (Some(true), Some(false)));
    }

    #[test]
    fn status_battery_scales() {
        let pct = decode_status(&PCT, &[0xb0, 3, 87, 3, 0, 0]).unwrap();
        assert_eq!(pct.battery_percent, Some(87));
        let over = decode_status(&PCT, &[0xb0, 3, 200, 3, 0, 0]).unwrap();
        assert_eq!(over.battery_percent, Some(100));
        for (level, percent) in [(0, 0), (1, 25), (3, 75), (4, 100), (9, 100)] {
            let s = decode_status(&STEPS, &[0xb0, 3, level, 3, 0, 0]).unwrap();
            assert_eq!(s.battery_percent, Some(percent));
        }
    }

    #[test]
    fn status_chat_mix_convention() {
        let mix = |game, chat| decode_status(&PCT, &[0xb0, 3, 50, 3, game, chat]).unwrap().chat_mix;
        assert_eq!(mix(100, 0), Some(-1.0));
        assert_eq!(mix(0, 100), Some(1.0));
        assert_eq!(mix(100, 100), Some(0.0));
        assert_eq!(mix(250, 0), Some(-1.0));
        let none = decode_status(&NO_CHAT, &[0xb0, 3, 50, 3, 10, 90]).unwrap();
        assert_eq!(none.chat_mix, None);
    }

    #[test]
    fn short_status_reply_is_an_error() {
        for len in 0..STATUS_REPLY_LEN {
            let reply = vec![0xb0; len];
            assert!(matches!(
                decode_status(&PCT, &reply),
                Err(Error::DeviceCommunication(msg)) if msg.contains("too short")
            ));
        }
    }
}
