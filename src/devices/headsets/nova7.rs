//! Arctis Nova 7 family headsets.
//!
//! [EXPERIMENTAL] The whole protocol comes from community forks and has not been run on hardware. Frames are
//! 64-byte output reports `00 <opcode> <arg>` on the vendor control collection (usage page `0xFFC0`).
//! The save frame `00 09` is never sent: it would persist settings on the headset.

use super::{EqPreset, Headset, HeadsetStatus};
use crate::devices::hid_reports::{
    HEADSET_REPORT_SIZE, HeadsetSettingCommand, HeadsetStatusQueryCommand, HidCommand, HidDeviceType,
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
}

/// Per-PID behaviour of one Nova 7 variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Nova7Model {
    pub product_id: u16,
    pub battery: BatteryScale,
    pub sidetone: bool,
    pub chat_mix: bool,
}

const fn model(product_id: u16, battery: BatteryScale, sidetone_and_chat_mix: bool) -> Nova7Model {
    Nova7Model {
        product_id,
        battery,
        sidetone: sidetone_and_chat_mix,
        chat_mix: sidetone_and_chat_mix,
    }
}

/// Every PID routed to [`Nova7Headset`]. The 7P variants have neither sidetone nor ChatMix.
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
];

/// Look up the Nova 7 behaviour for a product ID; `None` for every other headset.
pub fn nova7_model(product_id: u16) -> Option<&'static Nova7Model> {
    NOVA7_MODELS.iter().find(|m| m.product_id == product_id)
}

/// Status replies are `b0 power battery charge chat_game chat_chat`.
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
    if reply.len() < STATUS_REPLY_LEN {
        return Err(Error::DeviceCommunication(format!(
            "Headset status reply too short: {} bytes (expected at least {STATUS_REPLY_LEN})",
            reply.len()
        )));
    }
    let (power, battery, charge, game, chat) = (reply[1], reply[2], reply[3], reply[4], reply[5]);
    let connected = power == 3 && charge != 0;
    let battery_percent = connected.then(|| match model.battery {
        BatteryScale::Percent => battery.min(100),
        BatteryScale::Steps4 => battery.min(4) * 25,
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

/// [EXPERIMENTAL] Arctis Nova 7 family headset.
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
        self.send_command(&HeadsetSettingCommand { code, value })
    }

    /// Query the status, re-sending the query every 100 ms and skipping unrelated input reports.
    fn query_status(&self) -> Result<HeadsetStatus> {
        let mut report = [0u8; HEADSET_REPORT_SIZE];
        let len = HeadsetStatusQueryCommand.serialize(&mut report, HidDeviceType::Headset)?;
        let device = self.device()?.lock();
        let mut buf = [0u8; 64];
        for _ in 0..STATUS_MAX_QUERIES {
            write_padded_report(&device, &report[..len], HEADSET_REPORT_SIZE, false)?;
            let deadline = Instant::now() + STATUS_POLL_INTERVAL;
            while let Some(left) = deadline.checked_duration_since(Instant::now()) {
                let timeout_ms = i32::try_from(left.as_millis().max(1)).unwrap_or(100);
                let read = device.read_timeout(&mut buf, timeout_ms)?;
                if read > 0 && buf[0] == CommandCode::HeadsetStatus as u8 {
                    return decode_status(self.model, &buf[..read]);
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
        let wire = scale_to_wire(level, HeadsetSettingCommand::MAX_SIDETONE, "Sidetone level")?;
        self.set_wire_value(CommandCode::HeadsetSidetone, wire)
    }

    fn set_mic_volume(&mut self, volume: u8) -> Result<()> {
        let wire = scale_to_wire(volume, HeadsetSettingCommand::MAX_MIC_VOLUME, "Microphone volume")?;
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
    fn table_has_no_duplicate_pids_and_fifteen_models() {
        let pids: HashSet<u16> = NOVA7_MODELS.iter().map(|m| m.product_id).collect();
        assert_eq!(pids.len(), NOVA7_MODELS.len());
        assert_eq!(NOVA7_MODELS.len(), 15);
    }

    #[test]
    fn table_attributes_match_the_brief() {
        let steps4 = [0x2202, 0x2206, 0x220A, 0x223A, 0x227A, 0x22A4];
        let no_extras = [0x220A, 0x2298, 0x22A7];
        for m in NOVA7_MODELS {
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
    fn lookup_covers_the_family_and_nothing_else() {
        for m in NOVA7_MODELS {
            assert_eq!(nova7_model(m.product_id), Some(m));
        }
        assert!(nova7_model(ARCTIS_NOVA_7_RX).is_none());
        assert!(nova7_model(ARCTIS_NOVA_PRO_OMNI).is_none());
        assert!(nova7_model(ARCTIS_NOVA_3).is_none());
    }

    #[test]
    fn sidetone_and_mic_mapping_edges() {
        let side = |v| scale_to_wire(v, HeadsetSettingCommand::MAX_SIDETONE, "s").unwrap();
        assert_eq!(
            [side(0), side(16), side(17), side(50), side(83), side(84), side(100)],
            [0, 0, 1, 2, 2, 3, 3]
        );
        let mic = |v| scale_to_wire(v, HeadsetSettingCommand::MAX_MIC_VOLUME, "m").unwrap();
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
