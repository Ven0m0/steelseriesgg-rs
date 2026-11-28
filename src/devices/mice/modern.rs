//! Modern-family SteelSeries mice: Aerox 3/5/9, Prime Mini, Prime Wireless, Rival 3 Gen 2, Rival 5.
//!
//! The whole protocol comes from a community fork derived from rivalcfg. Only the wired Rival 3 Gen 2 (`0x1870`)
//! has been tested on hardware (contributor, Linux, 2026-11-16 to 18; status per frame in
//! `.claude/rules/experimental-protocol.md`); every other model is [EXPERIMENTAL].
//! Commands are output reports `00 <opcode> <args>` on the model's control interface: settings of variable length,
//! unpadded, the read-only queries zero padded to 64 bytes.
//! The 2.4 GHz receiver PID of a wireless mouse sets bit 0x40 on every opcode and answers each setting with
//! a 64-byte report that is read and dropped. The save command `11 00` (write to on-board memory) is never
//! sent and not implemented; a wired Rival 3 Gen 2 keeps its DPI stages across a replug without it, not its colour
//! or polling rate.
//!
//! A wired Rival 3 Gen 2 echoes each setting as `<opcode> 00`, in GG's capture (the USBPcap text attached to
//! rivalcfg issue #249, 2025-07-29; the PID comes from the issue, the capture does not decode the descriptors) and
//! to `ssgg`'s own frames (contributor, Linux, 2026-11-17/18); other models [EXPERIMENTAL]. The echo is read after
//! every frame and kept as a [`SettingAck`]; a missing one is not an error, a failed read is. The read-only
//! firmware (`90 00`) and polling-rate (`AB`) queries go only to the models without a wireless mode.

use super::dpi_tables::{TRUEMOVE_AIR, TRUEMOVE_CORE};
use super::{Mouse, PollRateReadback, SettingAck, unsupported};
use crate::devices::hid_reports::{
    CommandCode, HidCommand, HidDeviceType, MOUSE_REPORT_BUFFER_SIZE, MouseBatteryQueryCommand, MouseColorCommand,
    MouseColorLayout, MouseDpiCommand, MouseDpiStagesCommand, MousePollRateCommand, MousePollRateQueryCommand,
    OnboardFirmwareVersionCommand,
};
use crate::devices::keyboards::apex_7_profile::parse_firmware_version;
use crate::devices::keyboards::onboard::hex_of;
use crate::devices::{Device, DeviceInfo, DeviceType, product_ids::*};
use crate::rgb::Color;
use crate::{Error, Result};
use hidapi::HidDevice;
use parking_lot::Mutex;
use std::time::{Duration, Instant};

/// Minimum time between two commands; rivalcfg waits 50 ms and warns that less can hang some mice.
const COMMAND_SPACING: Duration = Duration::from_millis(50);
const READ_TIMEOUT_MS: i32 = 200;
/// How long a wired mouse gets to echo a setting; the echoes in GG's capture came about 1 ms after the frame.
const ECHO_TIMEOUT_MS: i32 = 50;
/// Reports read after a frame before its reply counts as missing. Each read waits up to its timeout, and a read
/// that times out ends the wait.
const REPLY_READS: usize = 8;
/// Most reports dropped from the queue before a wired frame, so a late answer to an earlier frame is not read as
/// this one's.
const STALE_REPORTS_MAX: usize = 64;
/// Size of an answer: a 2.4 GHz receiver's acknowledgement, a wired mouse's echo or query reply.
const REPLY_LEN: usize = 64;

/// Polling rates the protocol offers, in the order of [`MouseSpec::poll_codes`].
const POLL_RATES_HZ: [u16; 4] = [125, 250, 500, 1000];

/// How a DPI value becomes the code on the wire.
#[derive(Clone, Copy, Debug)]
pub enum DpiEncoding {
    /// One byte looked up at `(dpi - min) / step`.
    Table(&'static [u8]),
    /// Two bytes, `dpi / dpi_per_code`.
    Linear { dpi_per_code: u16 },
}

/// DPI range, encoding and command layout of a model. From the community rivalcfg-derived fork and rivalcfg;
/// `[EXPERIMENTAL]` except what a field says was confirmed on the wired Rival 3 Gen 2 `0x1870`.
#[derive(Clone, Copy, Debug)]
pub struct DpiSpec {
    pub opcode: u8,
    /// Lowest DPI that `ssgg mouse dpi` accepts.
    pub min: u16,
    /// Highest DPI that `ssgg mouse dpi` accepts.
    pub max: u16,
    /// DPI step: `ssgg mouse dpi` accepts only `min + n * step`.
    pub step: u16,
    pub encoding: DpiEncoding,
    /// Index of the stage the single DPI value is sent as; 0 on the Rival 3 Gen 2, confirmed on the `0x1870`
    /// (2026-11-17).
    pub first_preset: u8,
    /// The command carries the code twice, once per axis (a frame layout, not a capability; from rivalcfg).
    pub per_axis: bool,
    /// `ssgg mouse dpi` can set several DPI stages for the mouse's DPI button in one command
    /// ([`MouseDpiStagesCommand`]). Set only where that frame is evidenced: the Rival 3 Gen 2, by rivalcfg's test
    /// vectors, the third-party hardware test (macOS) of the two-stage frame in its PR #292, and from `ssgg` with 2
    /// stages (first active) and 4 (second active; contributor, Linux, 2026-11-17/18). 3 and 5 stages and the other
    /// active stages are `[EXPERIMENTAL]`.
    pub can_set_dpi_stages: bool,
}

impl DpiSpec {
    fn code(&self, dpi: u16) -> Result<u16> {
        let off_grid = || {
            Error::InvalidConfig(format!(
                "{dpi} DPI is not supported: {} to {} in steps of {}",
                self.min, self.max, self.step
            ))
        };
        if !(self.min..=self.max).contains(&dpi) || (dpi - self.min).checked_rem(self.step) != Some(0) {
            return Err(off_grid());
        }
        match self.encoding {
            DpiEncoding::Table(table) => (dpi - self.min)
                .checked_div(self.step)
                .and_then(|index| table.get(usize::from(index)))
                .map(|code| u16::from(*code))
                .ok_or_else(off_grid),
            DpiEncoding::Linear { dpi_per_code } => dpi.checked_div(dpi_per_code).ok_or_else(off_grid),
        }
    }
}

/// What the mice of one rivalcfg profile share. From the community rivalcfg-derived fork; `[EXPERIMENTAL]` except
/// what a field says was confirmed on the wired Rival 3 Gen 2 `0x1870`.
#[derive(Clone, Copy, Debug)]
pub struct MouseSpec {
    pub dpi: DpiSpec,
    /// Codes for 125, 250, 500 and 1000 Hz; 125 and 1000 Hz confirmed on the `0x1870` (2026-11-17).
    pub poll_codes: [u8; 4],
    /// How the colour frame addresses the LED zones; the mask layout is confirmed on the `0x1870` (2026-11-16/18).
    pub color_layout: MouseColorLayout,
    /// How many LED zones `ssgg mouse color --zone` can address; the three zones of the `0x1870` are confirmed
    /// (2026-11-18).
    pub zone_count: u8,
    /// The mouse runs on a battery and `ssgg mouse info` can ask for its charge (`92`, `D2` through a receiver).
    /// From the community rivalcfg-derived fork; `[EXPERIMENTAL]`, untested on hardware.
    pub can_report_battery: bool,
    /// `ssgg mouse info` can ask the mouse for its firmware version (`90 00`) and polling rate (`AB`) over the
    /// cable. Set on the models with no wireless mode (Mousework's family F2: Aerox 3, Aerox 5, Prime Mini, Rival 3
    /// Gen 2, Rival 5), whose reply starts with the echoed opcode. Confirmed on a wired Rival 3 Gen 2 `0x1870`
    /// (Linux, 2026-11-17), `[EXPERIMENTAL]` on the others; `false` on models with a wireless mode, whose cabled
    /// answer has no echoed opcode in Mousework's probe.
    pub can_read_firmware_and_poll_rate: bool,
}

const SPEC_AEROX_3: MouseSpec = MouseSpec {
    dpi: DpiSpec {
        opcode: MouseDpiCommand::OPCODE,
        min: 200,
        max: 8500,
        step: 100,
        encoding: DpiEncoding::Table(&TRUEMOVE_CORE),
        first_preset: 1,
        per_axis: false,
        can_set_dpi_stages: false,
    },
    poll_codes: [4, 3, 2, 1],
    color_layout: MouseColorLayout::Mask8,
    zone_count: 3,
    can_report_battery: false,
    can_read_firmware_and_poll_rate: true,
};

/// The preset index is 0-based on this model, unlike the Aerox 3 it shares the sensor table with: rivalcfg PR #292
/// (merged 2026-10-07), tested by its author on a wired `0x1870` (macOS), where `34 01 01 08 08` was ignored and
/// `34 01 00 12 12` applied.
const SPEC_RIVAL_3_GEN_2: MouseSpec = MouseSpec {
    dpi: DpiSpec {
        opcode: MouseDpiCommand::OPCODE_RIVAL_3_GEN_2,
        first_preset: 0,
        per_axis: true,
        can_set_dpi_stages: true,
        ..SPEC_AEROX_3.dpi
    },
    ..SPEC_AEROX_3
};

const SPEC_AEROX_5: MouseSpec = MouseSpec {
    dpi: DpiSpec {
        opcode: MouseDpiCommand::OPCODE,
        min: 100,
        max: 18000,
        step: 100,
        encoding: DpiEncoding::Table(&TRUEMOVE_AIR),
        first_preset: 0,
        per_axis: false,
        can_set_dpi_stages: false,
    },
    poll_codes: [4, 3, 2, 1],
    color_layout: MouseColorLayout::Mask8,
    zone_count: 3,
    can_report_battery: false,
    can_read_firmware_and_poll_rate: true,
};

const SPEC_RIVAL_5: MouseSpec = MouseSpec {
    color_layout: MouseColorLayout::Mask16,
    zone_count: 10,
    ..SPEC_AEROX_5
};

const SPEC_AEROX_WIRELESS: MouseSpec = MouseSpec {
    poll_codes: [3, 2, 1, 0],
    color_layout: MouseColorLayout::Indexed,
    can_report_battery: true,
    can_read_firmware_and_poll_rate: false,
    ..SPEC_AEROX_5
};

const SPEC_PRIME_WIRELESS: MouseSpec = MouseSpec {
    zone_count: 1,
    ..SPEC_AEROX_WIRELESS
};

const SPEC_PRIME_MINI: MouseSpec = MouseSpec {
    dpi: DpiSpec {
        min: 50,
        max: 18000,
        step: 50,
        encoding: DpiEncoding::Linear { dpi_per_code: 50 },
        ..SPEC_AEROX_5.dpi
    },
    color_layout: MouseColorLayout::Single,
    zone_count: 1,
    ..SPEC_AEROX_5
};

/// One USB product ID of a Modern-family mouse. From the community rivalcfg-derived fork; `[EXPERIMENTAL]` except
/// what a field says was confirmed on the wired Rival 3 Gen 2 `0x1870`.
#[derive(Clone, Copy, Debug)]
pub struct MouseModel {
    pub product_id: u16,
    pub name: &'static str,
    /// USB interface that takes the commands; interface 3 confirmed on the wired Rival 3 Gen 2 `0x1870`, the others
    /// from the community fork, `[EXPERIMENTAL]`.
    pub interface_number: i32,
    /// 2.4 GHz receiver PID of a dual-mode mouse: the frames use the receiver opcodes and no echo is read (a
    /// transport fact, not a capability). No receiver PID has been tested.
    pub dongle: bool,
    pub spec: &'static MouseSpec,
}

const fn model(
    product_id: u16,
    name: &'static str,
    interface_number: i32,
    dongle: bool,
    spec: &'static MouseSpec,
) -> MouseModel {
    MouseModel {
        product_id,
        name,
        interface_number,
        dongle,
        spec,
    }
}

/// Every PID routed to [`ModernMouse`]. Interface numbers and specs follow the community fork.
pub const MOUSE_MODELS: &[MouseModel] = &[
    model(AEROX_3, "Aerox 3", 3, false, &SPEC_AEROX_3),
    model(
        AEROX_3_WIRELESS,
        "Aerox 3 Wireless (wired)",
        3,
        false,
        &SPEC_AEROX_WIRELESS,
    ),
    model(
        AEROX_3_WIRELESS_DONGLE,
        "Aerox 3 Wireless (2.4 GHz)",
        3,
        true,
        &SPEC_AEROX_WIRELESS,
    ),
    model(
        AEROX_3_WIRELESS_CS2,
        "Aerox 3 Wireless CS2 Dragon Lore Edition (wired)",
        3,
        false,
        &SPEC_AEROX_WIRELESS,
    ),
    model(
        AEROX_3_WIRELESS_CS2_DONGLE,
        "Aerox 3 Wireless CS2 Dragon Lore Edition (2.4 GHz)",
        3,
        true,
        &SPEC_AEROX_WIRELESS,
    ),
    model(AEROX_5, "Aerox 5", 3, false, &SPEC_AEROX_5),
    model(
        AEROX_5_WIRELESS,
        "Aerox 5 Wireless (wired)",
        3,
        false,
        &SPEC_AEROX_WIRELESS,
    ),
    model(
        AEROX_5_WIRELESS_DONGLE,
        "Aerox 5 Wireless (2.4 GHz)",
        3,
        true,
        &SPEC_AEROX_WIRELESS,
    ),
    model(
        AEROX_5_WIRELESS_DESTINY_2,
        "Aerox 5 Wireless Destiny 2 Edition (wired)",
        3,
        false,
        &SPEC_AEROX_WIRELESS,
    ),
    model(
        AEROX_5_WIRELESS_DESTINY_2_DONGLE,
        "Aerox 5 Wireless Destiny 2 Edition (2.4 GHz)",
        3,
        true,
        &SPEC_AEROX_WIRELESS,
    ),
    model(
        AEROX_5_WIRELESS_DIABLO_IV,
        "Aerox 5 Wireless Diablo IV Edition (wired)",
        3,
        false,
        &SPEC_AEROX_WIRELESS,
    ),
    model(
        AEROX_5_WIRELESS_DIABLO_IV_DONGLE,
        "Aerox 5 Wireless Diablo IV Edition (2.4 GHz)",
        3,
        true,
        &SPEC_AEROX_WIRELESS,
    ),
    model(
        AEROX_9_WIRELESS,
        "Aerox 9 Wireless (wired)",
        3,
        false,
        &SPEC_AEROX_WIRELESS,
    ),
    model(
        AEROX_9_WIRELESS_DONGLE,
        "Aerox 9 Wireless (2.4 GHz)",
        3,
        true,
        &SPEC_AEROX_WIRELESS,
    ),
    model(
        AEROX_9_WIRELESS_WOW,
        "Aerox 9 Wireless WOW Edition (wired)",
        3,
        false,
        &SPEC_AEROX_WIRELESS,
    ),
    model(
        AEROX_9_WIRELESS_WOW_DONGLE,
        "Aerox 9 Wireless WOW Edition (2.4 GHz)",
        3,
        true,
        &SPEC_AEROX_WIRELESS,
    ),
    model(PRIME_MINI, "Prime Mini", 3, false, &SPEC_PRIME_MINI),
    model(
        PRIME_MINI_WIRELESS,
        "Prime Mini Wireless (wired)",
        3,
        false,
        &SPEC_PRIME_WIRELESS,
    ),
    model(
        PRIME_MINI_WIRELESS_DONGLE,
        "Prime Mini Wireless (2.4 GHz)",
        3,
        true,
        &SPEC_PRIME_WIRELESS,
    ),
    model(PRIME_WIRELESS, "Prime Wireless (wired)", 3, false, &SPEC_PRIME_WIRELESS),
    model(
        PRIME_WIRELESS_DONGLE,
        "Prime Wireless (2.4 GHz)",
        3,
        true,
        &SPEC_PRIME_WIRELESS,
    ),
    model(RIVAL_3_GEN_2, "Rival 3 Gen 2", 3, false, &SPEC_RIVAL_3_GEN_2),
    model(RIVAL_5, "Rival 5", 0, false, &SPEC_RIVAL_5),
    model(RIVAL_5_DESTINY, "Rival 5 Destiny Edition", 0, false, &SPEC_RIVAL_5),
];

/// Look up the Modern-family model for a product ID; `None` for every other device.
pub fn model_for(product_id: u16) -> Option<&'static MouseModel> {
    MOUSE_MODELS.iter().find(|m| m.product_id == product_id)
}

impl MouseModel {
    pub fn dpi_command(&self, dpi: u16) -> Result<MouseDpiCommand> {
        let dpi_spec = &self.spec.dpi;
        Ok(MouseDpiCommand {
            opcode: dpi_spec.opcode,
            dongle: self.dongle,
            selected: dpi_spec.first_preset,
            code: dpi_spec.code(dpi)?,
            wide: matches!(dpi_spec.encoding, DpiEncoding::Linear { .. }),
            per_axis: dpi_spec.per_axis,
        })
    }

    /// One command for 1 to [`MouseDpiStagesCommand::MAX_STAGES`] DPI stages with stage `active` (0-based) running
    /// (confirmed on a wired Rival 3 Gen 2 for 2 stages with `active` 0 and 4 with `active` 1, the rest
    /// [EXPERIMENTAL]). Refused on a model without [`DpiSpec::can_set_dpi_stages`].
    pub fn dpi_stages_command(&self, stages: &[u16], active: u8) -> Result<MouseDpiStagesCommand> {
        let dpi_spec = &self.spec.dpi;
        if !dpi_spec.can_set_dpi_stages {
            return Err(unsupported("setting DPI stages", self.name));
        }
        let command = MouseDpiStagesCommand {
            opcode: dpi_spec.opcode,
            dongle: self.dongle,
            first_preset: dpi_spec.first_preset,
            active,
            codes: stages.iter().map(|dpi| dpi_spec.code(*dpi)).collect::<Result<_>>()?,
            wide: matches!(dpi_spec.encoding, DpiEncoding::Linear { .. }),
            per_axis: dpi_spec.per_axis,
        };
        command.validate()?;
        Ok(command)
    }

    pub fn poll_rate_command(&self, hz: u16) -> Result<MousePollRateCommand> {
        let index = POLL_RATES_HZ
            .iter()
            .position(|rate| *rate == hz)
            .ok_or_else(|| Error::InvalidConfig(format!("{hz} Hz is not a polling rate: use 125, 250, 500 or 1000")))?;
        Ok(MousePollRateCommand {
            dongle: self.dongle,
            code: self.spec.poll_codes[index],
        })
    }

    /// The colour command of one LED zone (0-based), the frame [`Self::color_commands`] sends for it. Confirmed on a
    /// wired Rival 3 Gen 2 (0 front, 1 middle, 2 rear), [EXPERIMENTAL] on other models.
    pub fn zone_color_command(&self, zone: u8, color: Color) -> Result<MouseColorCommand> {
        let zones = self.spec.zone_count;
        if zone >= zones {
            return Err(Error::InvalidConfig(format!(
                "LED zone index {zone} out of range: the {} has {zones} zone(s), 0..={}",
                self.name,
                zones.saturating_sub(1)
            )));
        }
        Ok(MouseColorCommand {
            layout: self.spec.color_layout,
            dongle: self.dongle,
            zone,
            color,
        })
    }

    /// One colour command per LED zone.
    pub fn color_commands(&self, color: Color) -> Vec<MouseColorCommand> {
        (0..self.spec.zone_count)
            .map(|zone| MouseColorCommand {
                layout: self.spec.color_layout,
                dongle: self.dongle,
                zone,
                color,
            })
            .collect()
    }

    /// `None` for a model without a battery.
    pub fn battery_query(&self) -> Option<MouseBatteryQueryCommand> {
        self.spec
            .can_report_battery
            .then_some(MouseBatteryQueryCommand { dongle: self.dongle })
    }

    /// The cabled PID of a model that answers the `90` and `AB` queries
    /// ([`MouseSpec::can_read_firmware_and_poll_rate`]).
    fn can_read_firmware_and_poll_rate(&self) -> bool {
        !self.dongle && self.spec.can_read_firmware_and_poll_rate
    }

    /// The read-only firmware query, `90 00` zero padded to 64 bytes: GG's own frame to a wired Rival 3 Gen 2, which
    /// [`OnboardFirmwareVersionCommand::KEYBOARD`] sends byte for byte; confirmed on a wired `0x1870` (Linux,
    /// 2026-11-17), [EXPERIMENTAL] on the other wired-only models. `None` on a model with a wireless mode:
    /// Mousework's probe reads its answer over the cable without the echoed opcode, which [`await_reply`] would not
    /// take for the reply.
    pub fn firmware_query(&self) -> Option<OnboardFirmwareVersionCommand> {
        self.can_read_firmware_and_poll_rate()
            .then_some(OnboardFirmwareVersionCommand::KEYBOARD)
    }

    /// The read-only polling-rate query `AB`, from Mousework's probe; confirmed on a wired `0x1870` (Linux,
    /// 2026-11-17), [EXPERIMENTAL] on the other wired-only models. `None` on a model with a wireless mode.
    pub fn poll_rate_query(&self) -> Option<MousePollRateQueryCommand> {
        self.can_read_firmware_and_poll_rate()
            .then_some(MousePollRateQueryCommand)
    }

    /// Polling rate in an `AB` reply, taken as `ab <level>` like every other reply of the Rival 3 Gen 2 in GG's
    /// capture, with the level in this model's polling-rate codes (1, 2, 3, 4 = 1000, 500, 250, 125 Hz on the
    /// wired-only models, the table Mousework gives). A wired `0x1870` answered `ab 01` at 1000 Hz and `ab 04` at
    /// 125 Hz (Linux, 2026-11-17), so the level follows the echoed opcode, not the first byte where Mousework reads
    /// it; levels 2 and 3 and the other models are [EXPERIMENTAL]. `None` for another reply or an unknown level.
    pub fn decode_poll_rate_reply(&self, reply: &[u8]) -> Option<u16> {
        let level = *reply_body(reply, CommandCode::MousePollRateQuery as u8)?.get(1)?;
        let index = self.spec.poll_codes.iter().position(|code| *code == level)?;
        POLL_RATES_HZ.get(index).copied()
    }
}

/// Battery percent from a query reply as returned by hidapi (no report ID): byte 1 holds the charge in 5 percent
/// steps plus one in its low seven bits and the charging flag in bit 7. `None` for a reply that is too short
/// or carries a level outside 0 to 100, as when the mouse is asleep.
pub fn decode_battery(reply: &[u8]) -> Option<u8> {
    let steps = i16::from(*reply.get(1)? & 0x7F);
    u8::try_from((steps - 1) * 5).ok().filter(|level| *level <= 100)
}

/// [EXPERIMENTAL] The part of a wired mouse's input report that answers `opcode`: the report itself when it starts
/// with the opcode, as every answer in GG's capture of a Rival 3 Gen 2 does (`90 31 2e ..` to `90 00`, `21 00` to
/// a colour frame). hidapi hands these reports over without a report number (the mice declare no report IDs, and
/// its Windows backend strips the 0 the OS puts in front), so a report starting `00 <opcode>` answers something
/// else. `None` for a report that answers something else.
pub fn reply_body(report: &[u8], opcode: u8) -> Option<&[u8]> {
    match report {
        [first, ..] if *first == opcode => Some(report),
        _ => None,
    }
}

/// [EXPERIMENTAL] Read reports through `read` until one answers `opcode`, the wire opcode of the frame just sent,
/// and return it from the opcode on ([`reply_body`]). A report that answers something else, such as an echo left
/// from an earlier frame, is skipped. At most `REPLY_READS` (8) reports are read, and a read that times out
/// (returns 0 bytes) ends the wait: `Ok(None)` then, a clear "no reply" and not an error. `read` fills its buffer
/// with one report and returns its length, like `HidDevice::read_timeout`.
pub fn await_reply(mut read: impl FnMut(&mut [u8]) -> Result<usize>, opcode: u8) -> Result<Option<Vec<u8>>> {
    let mut report = [0u8; REPLY_LEN + 1];
    for _ in 0..REPLY_READS {
        let len = read(&mut report)?;
        if len == 0 {
            return Ok(None);
        }
        let received = &report[..len.min(report.len())];
        if let Some(body) = reply_body(received, opcode) {
            return Ok(Some(body.to_vec()));
        }
        tracing::debug!(
            "mouse report skipped while waiting for {opcode:#04x}: {}",
            hex_of(received)
        );
    }
    Ok(None)
}

/// [EXPERIMENTAL] [`await_reply`], except that when no report answers `opcode`, the first report read comes back
/// instead, so a reply laid out without the echoed opcode is still seen: Mousework's probe decodes the `AB` reply as
/// if it carried none. `Ok(None)` only when the first read times out.
pub fn await_reply_or_first(mut read: impl FnMut(&mut [u8]) -> Result<usize>, opcode: u8) -> Result<Option<Vec<u8>>> {
    let mut first = None;
    let reply = await_reply(
        |report| {
            let len = read(report)?;
            if first.is_none() && len > 0 {
                first = Some(report[..len.min(report.len())].to_vec());
            }
            Ok(len)
        },
        opcode,
    )?;
    Ok(reply.or(first))
}

/// [EXPERIMENTAL] Firmware version in a `90` reply: the printable ASCII behind the echoed opcode, as in GG's
/// capture of a wired Rival 3 Gen 2 (`90 31 2e 31 2e 36 20 2b 65 35 37 66 66 36 61 31 00 ..` = `1.1.6 +e57ff6a1`).
/// `None` for another reply or one without a version.
pub fn decode_firmware_reply(reply: &[u8]) -> Option<String> {
    parse_firmware_version(reply_body(reply, CommandCode::OnboardFirmwareVersion as u8)?.get(1..)?)
}

/// [EXPERIMENTAL] The acknowledgement in a reply from [`await_reply`]: the echoed opcode and the status byte behind
/// it. A reply too short to carry a status counts as none.
pub fn echo_ack(reply: &[u8]) -> SettingAck {
    match reply {
        [opcode, status, ..] => SettingAck::Echoed {
            opcode: *opcode,
            status: *status,
        },
        _ => SettingAck::NoReply,
    }
}

/// [EXPERIMENTAL] The acknowledgement of a setting frame from what [`await_reply`] read after it: no reply in time is
/// [`SettingAck::NoReply`], a failed read is an error that says the frame went out all the same.
fn echo_outcome(reply: Result<Option<Vec<u8>>>, frame: &str) -> Result<SettingAck> {
    match reply {
        Ok(Some(reply)) => Ok(echo_ack(&reply)),
        Ok(None) => Ok(SettingAck::NoReply),
        Err(err) => Err(Error::DeviceCommunication(format!(
            "{frame} was sent, but reading the mouse's echo failed: {err}"
        ))),
    }
}

/// An echo read that a signal interrupted counts as a timeout (0 bytes): hidapi's Linux backend does not retry its
/// `poll` after `EINTR`, so a signal handler in the process would otherwise turn a late echo into a failed setting.
fn interrupted_as_timeout(read: std::result::Result<usize, hidapi::HidError>) -> Result<usize> {
    match read {
        Err(hidapi::HidError::IoError { error }) if error.kind() == std::io::ErrorKind::Interrupted => Ok(0),
        other => Ok(other?),
    }
}

/// [EXPERIMENTAL] Modern-family mouse.
pub struct ModernMouse {
    info: DeviceInfo,
    model: &'static MouseModel,
    device: Option<Mutex<HidDevice>>,
    last_write: Option<Instant>,
    /// What a wired mouse answered to each frame of the last setting.
    acks: Vec<SettingAck>,
}

impl ModernMouse {
    pub fn new(info: DeviceInfo, device: HidDevice, model: &'static MouseModel) -> Self {
        Self {
            info,
            model,
            device: Some(Mutex::new(device)),
            last_write: None,
            acks: Vec::new(),
        }
    }

    fn device(&self) -> Result<&Mutex<HidDevice>> {
        self.device
            .as_ref()
            .ok_or(Error::DeviceCommunication("Device not connected".to_string()))
    }

    /// Sleep out what is left of [`COMMAND_SPACING`] since the previous write.
    fn pace(&self) {
        if let Some(elapsed) = self.last_write.map(|at| at.elapsed())
            && elapsed < COMMAND_SPACING
        {
            std::thread::sleep(COMMAND_SPACING - elapsed);
        }
    }

    /// Write one command, keeping [`COMMAND_SPACING`] since the previous one, and return its wire opcode, the byte
    /// an answer to it starts with. Written with `hid_write` directly: the shared padded writer would pad the
    /// report and drop a repeated one.
    fn send(&mut self, command: &dyn HidCommand) -> Result<u8> {
        self.pace();
        let mut report = [0u8; MOUSE_REPORT_BUFFER_SIZE];
        let len = command.serialize(&mut report, HidDeviceType::Mouse)?;
        let opcode = report[..len]
            .get(1)
            .copied()
            .ok_or_else(|| Error::DeviceCommunication(format!("{} has no opcode byte", command.description())))?;
        let written = self.device()?.lock().write(&report[..len]);
        self.last_write = Some(Instant::now());
        written?;
        Ok(opcode)
    }

    /// Send a setting, then read its answer: a 2.4 GHz receiver's acknowledgement is read and dropped like
    /// rivalcfg does, a wired mouse's echo is kept in `acks`. A missing echo is not an error; a failed read is.
    fn send_setting(&mut self, command: &dyn HidCommand) -> Result<()> {
        if self.model.dongle {
            self.send(command)?;
            let mut ack = [0u8; REPLY_LEN];
            self.device()?.lock().read_timeout(&mut ack, READ_TIMEOUT_MS)?;
            return Ok(());
        }
        // The queue is emptied after the spacing, right before the write, so an answer to the previous frame that
        // arrives during the spacing is not taken for this frame's echo (the colour frames all echo `21`). A failed
        // drain does not stop the write: a read failure that lasts shows up in the echo read after it.
        self.pace();
        if let Err(err) = self.drop_stale_reports() {
            tracing::debug!("dropping stale mouse reports failed: {err}");
        }
        let opcode = self.send(command)?;
        let ack = self.read_echo(opcode, &command.description())?;
        self.acks.push(ack);
        Ok(())
    }

    /// Drop the reports already queued on this handle.
    fn drop_stale_reports(&self) -> Result<()> {
        let device = self.device()?.lock();
        let mut report = [0u8; REPLY_LEN + 1];
        for _ in 0..STALE_REPORTS_MAX {
            if device.read_timeout(&mut report, 0)? == 0 {
                break;
            }
        }
        Ok(())
    }

    /// The echo of `frame`, the frame just sent ([`echo_outcome`]).
    fn read_echo(&self, opcode: u8, frame: &str) -> Result<SettingAck> {
        let reply = self.device().and_then(|device| {
            let device = device.lock();
            await_reply(
                |report| interrupted_as_timeout(device.read_timeout(report, ECHO_TIMEOUT_MS)),
                opcode,
            )
        });
        echo_outcome(reply, frame)
    }

    /// Send a read-only query and return the reply that answers it ([`await_reply`]), or with `or_first` the first
    /// report read when none does ([`await_reply_or_first`]); `None` when no report came.
    fn query(&mut self, command: &dyn HidCommand, or_first: bool) -> Result<Option<Vec<u8>>> {
        self.pace();
        self.drop_stale_reports()?;
        let opcode = self.send(command)?;
        let device = self.device()?.lock();
        let read = |report: &mut [u8]| -> Result<usize> { Ok(device.read_timeout(report, READ_TIMEOUT_MS)?) };
        if or_first {
            await_reply_or_first(read, opcode)
        } else {
            await_reply(read, opcode)
        }
    }
}

impl Device for ModernMouse {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::Mouse
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
        self.device()?.lock().write(data)?;
        Ok(())
    }

    fn receive_raw(&mut self, buf: &mut [u8]) -> Result<usize> {
        Ok(self.device()?.lock().read_timeout(buf, READ_TIMEOUT_MS)?)
    }
}

impl Mouse for ModernMouse {
    fn set_dpi(&mut self, dpi: u16) -> Result<()> {
        self.acks.clear();
        let command = self.model.dpi_command(dpi)?;
        self.send_setting(&command)
    }

    fn set_dpi_stages(&mut self, stages: &[u16], active: u8) -> Result<()> {
        self.acks.clear();
        let command = self.model.dpi_stages_command(stages, active)?;
        self.send_setting(&command)
    }

    fn set_poll_rate(&mut self, hz: u16) -> Result<()> {
        self.acks.clear();
        let command = self.model.poll_rate_command(hz)?;
        self.send_setting(&command)
    }

    fn set_color(&mut self, color: Color) -> Result<()> {
        self.acks.clear();
        for command in self.model.color_commands(color) {
            self.send_setting(&command)?;
        }
        Ok(())
    }

    fn set_zone_color(&mut self, zone: u8, color: Color) -> Result<()> {
        self.acks.clear();
        let command = self.model.zone_color_command(zone, color)?;
        self.send_setting(&command)
    }

    fn battery_level(&mut self) -> Result<Option<u8>> {
        let Some(query) = self.model.battery_query() else {
            return Ok(None);
        };
        self.send(&query)?;
        let mut reply = [0u8; REPLY_LEN];
        let len = self.device()?.lock().read_timeout(&mut reply, READ_TIMEOUT_MS)?;
        Ok(decode_battery(&reply[..len]))
    }

    fn firmware_version(&mut self) -> Result<Option<String>> {
        let query = self
            .model
            .firmware_query()
            .ok_or_else(|| unsupported("reading the firmware version", self.model.name))?;
        let reply = self.query(&query, false)?;
        let version = reply.as_deref().and_then(decode_firmware_reply);
        if let (Some(reply), None) = (&reply, &version) {
            tracing::debug!("the firmware reply carries no version: {}", hex_of(reply));
        }
        Ok(version)
    }

    fn poll_rate_readback(&mut self) -> Result<Option<PollRateReadback>> {
        let query = self
            .model
            .poll_rate_query()
            .ok_or_else(|| unsupported("reading the polling rate back", self.model.name))?;
        Ok(self.query(&query, true)?.map(|reply| PollRateReadback {
            hz: self.model.decode_poll_rate_reply(&reply),
            reply,
        }))
    }

    fn last_acknowledgements(&self) -> &[SettingAck] {
        &self.acks
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::{CommandCode, device_name_from_product_id, device_type_from_product_id, is_detect_only};
    use std::collections::HashSet;

    fn bytes(command: &dyn HidCommand) -> Vec<u8> {
        let mut buffer = [0xAAu8; MOUSE_REPORT_BUFFER_SIZE];
        let len = command.serialize(&mut buffer, HidDeviceType::Mouse).unwrap();
        buffer[..len].to_vec()
    }

    fn pid(product_id: u16) -> &'static MouseModel {
        model_for(product_id).unwrap()
    }

    fn dpi(product_id: u16, value: u16) -> Vec<u8> {
        bytes(&pid(product_id).dpi_command(value).unwrap())
    }

    fn colors(product_id: u16, color: Color) -> Vec<Vec<u8>> {
        pid(product_id).color_commands(color).iter().map(|c| bytes(c)).collect()
    }

    #[test]
    fn table_has_no_duplicate_pids_and_covers_the_scope() {
        let pids: HashSet<u16> = MOUSE_MODELS.iter().map(|m| m.product_id).collect();
        assert_eq!(pids.len(), MOUSE_MODELS.len());
        assert_eq!(MOUSE_MODELS.len(), 24);
        for expected in [
            0x1836, 0x1838, 0x183A, 0x1878, 0x187A, 0x1850, 0x1852, 0x1854, 0x185C, 0x185E, 0x1860, 0x1862, 0x1858,
            0x185A, 0x1874, 0x1876, 0x184D, 0x1848, 0x184A, 0x1840, 0x1842, 0x1870, 0x183C, 0x183E,
        ] {
            assert!(pids.contains(&expected), "{expected:#06x}");
        }
    }

    #[test]
    fn every_pid_is_routed_to_a_named_mouse_that_is_not_detect_only() {
        for m in MOUSE_MODELS {
            assert_eq!(device_type_from_product_id(m.product_id), DeviceType::Mouse);
            assert_eq!(
                device_name_from_product_id(m.product_id),
                m.name,
                "{:#06x}",
                m.product_id
            );
            assert!(!is_detect_only(m.product_id));
        }
        assert!(model_for(0x182E).is_none(), "Prime stays unsupported");
        assert!(model_for(0x182C).is_none(), "Prime+ stays unsupported");
        assert!(model_for(0x1824).is_none(), "Rival 3 stays unsupported");
    }

    #[test]
    fn dpi_tables_cover_their_ranges_with_rising_codes() {
        assert_eq!(TRUEMOVE_CORE.len(), 84);
        assert_eq!(TRUEMOVE_AIR.len(), 180);
        assert!(TRUEMOVE_CORE.windows(2).all(|w| w[0] < w[1]));
        assert!(TRUEMOVE_AIR.windows(2).all(|w| w[0] < w[1]));
        for m in MOUSE_MODELS {
            let d = &m.spec.dpi;
            if let DpiEncoding::Table(table) = d.encoding {
                assert_eq!(table.len(), usize::from((d.max - d.min) / d.step) + 1, "{}", m.name);
            }
        }
    }

    #[test]
    fn dpi_frames_at_min_typical_and_max() {
        assert_eq!(dpi(AEROX_3, 200), [0x00, 0x2D, 0x01, 0x01, 0x04]);
        assert_eq!(dpi(AEROX_3, 800), [0x00, 0x2D, 0x01, 0x01, 0x12]);
        assert_eq!(dpi(AEROX_3, 8500), [0x00, 0x2D, 0x01, 0x01, 0xC5]);
        assert_eq!(dpi(AEROX_5, 100), [0x00, 0x2D, 0x01, 0x00, 0x00]);
        assert_eq!(dpi(AEROX_5, 3200), [0x00, 0x2D, 0x01, 0x00, 0x26]);
        assert_eq!(dpi(AEROX_5, 18000), [0x00, 0x2D, 0x01, 0x00, 0xD6]);
        assert_eq!(dpi(RIVAL_5, 400), [0x00, 0x2D, 0x01, 0x00, 0x04]);
        assert_eq!(dpi(AEROX_9_WIRELESS, 1600), [0x00, 0x2D, 0x01, 0x00, 0x12]);
        assert_eq!(dpi(PRIME_WIRELESS, 18000), [0x00, 0x2D, 0x01, 0x00, 0xD6]);
    }

    #[test]
    fn rival_3_gen_2_uses_its_own_opcode_and_repeats_the_code_per_axis() {
        assert_eq!(dpi(RIVAL_3_GEN_2, 200), [0x00, 0x34, 0x01, 0x00, 0x04, 0x04]);
        assert_eq!(dpi(RIVAL_3_GEN_2, 1600), [0x00, 0x34, 0x01, 0x00, 0x24, 0x24]);
        assert_eq!(dpi(RIVAL_3_GEN_2, 8500), [0x00, 0x34, 0x01, 0x00, 0xC5, 0xC5]);
    }

    /// rivalcfg PR #292: `34 01 00 12 12` is the frame that applied 800 DPI on a wired `0x1870`, where the 1-based
    /// `34 01 01 ..` was ignored. GG's own frame to that mouse (rivalcfg issue #249 capture, frame 27) is
    /// `34 01 00 3e 3e`, 2700 DPI. The Aerox 3 keeps rivalcfg's 1-based preset.
    #[test]
    fn rival_3_gen_2_selects_its_presets_0_based() {
        assert_eq!(dpi(RIVAL_3_GEN_2, 800), [0x00, 0x34, 0x01, 0x00, 0x12, 0x12]);
        assert_eq!(dpi(RIVAL_3_GEN_2, 2700), [0x00, 0x34, 0x01, 0x00, 0x3E, 0x3E]);
        assert_eq!(dpi(AEROX_3, 800), [0x00, 0x2D, 0x01, 0x01, 0x12]);
    }

    fn stages(product_id: u16, values: &[u16], active: u8) -> Vec<u8> {
        bytes(&pid(product_id).dpi_stages_command(values, active).unwrap())
    }

    /// `34 <count> <active, 0-based> x1 y1 .. xn yn`, unpadded, as rivalcfg master's `multidpi_range_choice_xy`
    /// handler builds it for this model (`xyxy`, `first_preset` 0, no `packet_length`); codes from the TrueMove Core
    /// table (400 = 0x08, 800 = 0x12, 1600 = 0x24, 3200 = 0x49, 6400 = 0x94).
    #[test]
    fn rival_3_gen_2_dpi_stage_frames() {
        assert_eq!(stages(RIVAL_3_GEN_2, &[800], 0), [0x00, 0x34, 0x01, 0x00, 0x12, 0x12]);
        assert_eq!(stages(RIVAL_3_GEN_2, &[800], 0), dpi(RIVAL_3_GEN_2, 800));
        // rivalcfg PR #292: this frame made 3200 (the second stage) the running DPI on a wired 0x1870.
        assert_eq!(
            stages(RIVAL_3_GEN_2, &[200, 3200], 1),
            [0x00, 0x34, 0x02, 0x01, 0x04, 0x04, 0x49, 0x49]
        );
        assert_eq!(
            stages(RIVAL_3_GEN_2, &[800, 1600], 0),
            [0x00, 0x34, 0x02, 0x00, 0x12, 0x12, 0x24, 0x24]
        );
        assert_eq!(
            stages(RIVAL_3_GEN_2, &[400, 800, 1600, 3200, 6400], 2),
            [
                0x00, 0x34, 0x05, 0x02, 0x08, 0x08, 0x12, 0x12, 0x24, 0x24, 0x49, 0x49, 0x94, 0x94
            ]
        );
    }

    /// rivalcfg master's vector for `200,400,800,1600:1800` is `34 04 00 04 04 08 08 12 12 24 29`; `ssgg` sets both
    /// axes alike, so only the last byte (Y of the fourth stage) differs.
    #[test]
    fn rival_3_gen_2_dpi_stages_match_rivalcfgs_test_vector() {
        let frame = stages(RIVAL_3_GEN_2, &[200, 400, 800, 1600], 0);
        let rivalcfg = [0x00, 0x34, 0x04, 0x00, 0x04, 0x04, 0x08, 0x08, 0x12, 0x12, 0x24, 0x29];
        assert_eq!(frame.len(), rivalcfg.len());
        assert_eq!(frame[..11], rivalcfg[..11]);
        assert_eq!(frame[11], 0x24);
    }

    #[test]
    fn dpi_stages_reject_bad_counts_active_stages_and_values() {
        for (values, active) in [
            (&[][..], 0),
            (&[800, 1600, 2400, 3200, 4000, 4800][..], 0),
            (&[800, 1600][..], 2),
            (&[800][..], 1),
            (&[800, 150][..], 0),
            (&[800, 8600][..], 0),
        ] {
            assert!(
                matches!(
                    pid(RIVAL_3_GEN_2).dpi_stages_command(values, active),
                    Err(Error::InvalidConfig(_))
                ),
                "{values:?} {active}"
            );
        }
    }

    #[test]
    fn dpi_stages_are_refused_where_the_multi_stage_frame_is_unverified() {
        for m in MOUSE_MODELS {
            let command = m.dpi_stages_command(&[m.spec.dpi.min], 0);
            assert_eq!(command.is_ok(), m.product_id == RIVAL_3_GEN_2, "{}", m.name);
        }
        assert!(matches!(
            pid(AEROX_3).dpi_stages_command(&[800, 1600], 1),
            Err(Error::DeviceCommunication(_))
        ));
    }

    #[test]
    fn prime_mini_dpi_is_two_little_endian_bytes() {
        assert_eq!(dpi(PRIME_MINI, 50), [0x00, 0x2D, 0x01, 0x00, 0x01, 0x00]);
        assert_eq!(dpi(PRIME_MINI, 400), [0x00, 0x2D, 0x01, 0x00, 0x08, 0x00]);
        assert_eq!(dpi(PRIME_MINI, 11050), [0x00, 0x2D, 0x01, 0x00, 0xDD, 0x00]);
        assert_eq!(dpi(PRIME_MINI, 18000), [0x00, 0x2D, 0x01, 0x00, 0x68, 0x01]);
    }

    #[test]
    fn dongle_pids_set_the_flag_on_every_opcode() {
        assert_eq!(dpi(AEROX_3_WIRELESS_DONGLE, 400), [0x00, 0x6D, 0x01, 0x00, 0x04]);
        assert_eq!(dpi(PRIME_WIRELESS_DONGLE, 18000), [0x00, 0x6D, 0x01, 0x00, 0xD6]);
        assert_eq!(
            bytes(&pid(AEROX_5_WIRELESS_DONGLE).poll_rate_command(1000).unwrap()),
            [0x00, 0x6B, 0x00]
        );
        assert_eq!(
            bytes(&pid(AEROX_9_WIRELESS_DONGLE).battery_query().unwrap()),
            [0x00, 0xD2]
        );
        assert_eq!(
            colors(AEROX_9_WIRELESS_DONGLE, Color::new(0, 0, 0xFF))[2],
            [0x00, 0x61, 0x01, 0x02, 0x00, 0x00, 0xFF]
        );
    }

    #[test]
    fn poll_rate_codes_follow_the_family_quirk() {
        for (product_id, expected) in [
            (AEROX_3, [4u8, 3, 2, 1]),
            (RIVAL_3_GEN_2, [4, 3, 2, 1]),
            (RIVAL_5, [4, 3, 2, 1]),
            (PRIME_MINI, [4, 3, 2, 1]),
            (AEROX_3_WIRELESS, [3, 2, 1, 0]),
            (PRIME_WIRELESS_DONGLE, [3, 2, 1, 0]),
        ] {
            for (hz, code) in [125, 250, 500, 1000].into_iter().zip(expected) {
                let frame = bytes(&pid(product_id).poll_rate_command(hz).unwrap());
                let opcode = if pid(product_id).dongle { 0x6B } else { 0x2B };
                assert_eq!(frame, [0x00, opcode, code], "{product_id:#06x} {hz}");
            }
        }
    }

    #[test]
    fn colour_frames_per_zone_layout() {
        let red = Color::new(0xFF, 0, 0);
        let green = Color::new(0, 0xFF, 0);
        let blue = Color::new(0, 0, 0xFF);
        assert_eq!(
            colors(AEROX_3, red),
            [
                vec![0x00, 0x21, 0x01, 0xFF, 0x00, 0x00],
                vec![0x00, 0x21, 0x02, 0, 0, 0, 0xFF, 0x00, 0x00],
                vec![0x00, 0x21, 0x04, 0, 0, 0, 0, 0, 0, 0xFF, 0x00, 0x00],
            ]
        );
        assert_eq!(
            colors(RIVAL_3_GEN_2, green)[1],
            [0x00, 0x21, 0x02, 0, 0, 0, 0x00, 0xFF, 0x00]
        );
        assert_eq!(colors(AEROX_5, blue)[2][..4], [0x00, 0x21, 0x04, 0]);
        assert_eq!(
            colors(AEROX_9_WIRELESS, red)[2],
            [0x00, 0x21, 0x01, 0x02, 0xFF, 0x00, 0x00]
        );
        assert_eq!(
            colors(AEROX_3_WIRELESS, blue)[1],
            [0x00, 0x21, 0x01, 0x01, 0x00, 0x00, 0xFF]
        );
        assert_eq!(
            colors(PRIME_MINI, Color::new(0xAB, 0xCD, 0xEF)),
            [vec![0x00, 0x21, 0x00, 0xAB, 0xCD, 0xEF]]
        );
        assert_eq!(
            colors(PRIME_WIRELESS_DONGLE, red),
            [vec![0x00, 0x61, 0x01, 0x00, 0xFF, 0x00, 0x00]]
        );
    }

    /// rivalcfg's Rival 3 Gen 2 vectors for `--z1`, `--z2` and `--z3 #ABCDEF` (top, middle and bottom of the strip):
    /// `21 01 ab cd ef`, `21 02 00 00 00 ab cd ef`, `21 04 00 00 00 00 00 00 ab cd ef`.
    #[test]
    fn rival_3_gen_2_zone_colour_frames() {
        let c = Color::new(0xAB, 0xCD, 0xEF);
        let zone = |z| bytes(&pid(RIVAL_3_GEN_2).zone_color_command(z, c).unwrap());
        assert_eq!(zone(0), [0x00, 0x21, 0x01, 0xAB, 0xCD, 0xEF]);
        assert_eq!(zone(1), [0x00, 0x21, 0x02, 0x00, 0x00, 0x00, 0xAB, 0xCD, 0xEF]);
        assert_eq!(
            zone(2),
            [0x00, 0x21, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xAB, 0xCD, 0xEF]
        );
        assert_eq!(colors(RIVAL_3_GEN_2, c), [zone(0), zone(1), zone(2)]);
    }

    #[test]
    fn zone_colour_rejects_a_zone_the_model_does_not_have() {
        for m in MOUSE_MODELS {
            let last = m.spec.zone_count - 1;
            assert!(m.zone_color_command(last, Color::WHITE).is_ok(), "{}", m.name);
            assert!(
                matches!(
                    m.zone_color_command(last + 1, Color::WHITE),
                    Err(Error::InvalidConfig(_))
                ),
                "{}",
                m.name
            );
        }
    }

    #[test]
    fn rival_5_colour_frames_use_a_16_bit_mask() {
        let c = Color::new(0xFF, 0x18, 0x00);
        let frames = colors(RIVAL_5, c);
        assert_eq!(frames.len(), 10);
        assert_eq!(frames[0], [0x00, 0x21, 0x01, 0x00, 0xFF, 0x18, 0x00]);
        assert_eq!(frames[1], [0x00, 0x21, 0x02, 0x00, 0, 0, 0, 0xFF, 0x18, 0x00]);
        assert_eq!(frames[7][..4], [0x00, 0x21, 0x80, 0x00]);
        assert_eq!(frames[7].len(), 4 + 21 + 3);
        assert_eq!(frames[8][..4], [0x00, 0x21, 0x00, 0x01]);
        assert_eq!(frames[8].len(), 4 + 24 + 3);
        assert_eq!(frames[9][..4], [0x00, 0x21, 0x00, 0x02]);
        assert_eq!(frames[9].len(), 4 + 27 + 3);
        assert_eq!(frames[9][frames[9].len() - 3..], [0xFF, 0x18, 0x00]);
    }

    #[test]
    fn zone_counts_per_model() {
        for (product_id, zones) in [
            (AEROX_3, 3),
            (AEROX_5, 3),
            (AEROX_9_WIRELESS_DONGLE, 3),
            (RIVAL_3_GEN_2, 3),
            (RIVAL_5_DESTINY, 10),
            (PRIME_MINI, 1),
            (PRIME_MINI_WIRELESS, 1),
            (PRIME_WIRELESS_DONGLE, 1),
        ] {
            assert_eq!(
                pid(product_id).color_commands(Color::BLACK).len(),
                zones,
                "{product_id:#06x}"
            );
        }
    }

    #[test]
    fn battery_query_exists_only_on_wireless_models() {
        assert_eq!(bytes(&pid(AEROX_9_WIRELESS).battery_query().unwrap()), [0x00, 0x92]);
        for m in MOUSE_MODELS {
            let wireless = m.name.contains("Wireless");
            assert_eq!(m.battery_query().is_some(), wireless, "{}", m.name);
        }
    }

    #[test]
    fn battery_reply_decoding() {
        assert_eq!(decode_battery(&[0x92, 21]), Some(100));
        assert_eq!(decode_battery(&[0x92, 1]), Some(0));
        assert_eq!(decode_battery(&[0x92, 0x80 | 11]), Some(50));
        assert_eq!(decode_battery(&[0x92, 0]), None);
        assert_eq!(decode_battery(&[0x92, 22]), None);
        assert_eq!(decode_battery(&[0x92, 0xFF]), None);
        assert_eq!(decode_battery(&[0x92]), None);
        assert_eq!(decode_battery(&[]), None);
    }

    /// GG's capture of a wired Rival 3 Gen 2 (`0x1870`, rivalcfg issue #249), frame 17 on endpoint 0x84: the answer
    /// to `90 00`, the echoed opcode and `1.1.6 +e57ff6a1`, zero padded to 64 bytes.
    fn captured_firmware_reply() -> Vec<u8> {
        let mut reply = vec![
            0x90, 0x31, 0x2E, 0x31, 0x2E, 0x36, 0x20, 0x2B, 0x65, 0x35, 0x37, 0x66, 0x66, 0x36, 0x61, 0x31,
        ];
        reply.resize(REPLY_LEN, 0);
        reply
    }

    /// An echo from the same capture, `<opcode> 00` zero padded to 64 bytes: `34 00`, `2b 00` and `21 00` answer the
    /// DPI, polling-rate and colour frames.
    fn captured_echo(opcode: u8) -> Vec<u8> {
        let mut echo = vec![opcode, 0x00];
        echo.resize(REPLY_LEN, 0);
        echo
    }

    /// Input reports handed out one per read, then timeouts, like `HidDevice::read_timeout`.
    struct Reports {
        queue: std::collections::VecDeque<Vec<u8>>,
        reads: usize,
    }

    impl Reports {
        fn new(reports: Vec<Vec<u8>>) -> Self {
            Self {
                queue: reports.into(),
                reads: 0,
            }
        }

        fn read(&mut self, buffer: &mut [u8]) -> Result<usize> {
            self.reads += 1;
            let Some(report) = self.queue.pop_front() else {
                return Ok(0);
            };
            buffer[..report.len()].copy_from_slice(&report);
            Ok(report.len())
        }
    }

    /// Capture frame 15 is `90 00` zero padded to 64 bytes (`SET_REPORT wValue 0x0200 wIndex 3`); hidapi takes it
    /// with report number 0 in front.
    #[test]
    fn queries_are_ggs_firmware_frame_and_ab_where_the_model_can_read_them() {
        let mut gg = vec![0x00, 0x90];
        gg.resize(MOUSE_REPORT_BUFFER_SIZE, 0);
        assert_eq!(bytes(&pid(RIVAL_3_GEN_2).firmware_query().unwrap()), gg);
        let mut ab = vec![0x00, 0xAB];
        ab.resize(MOUSE_REPORT_BUFFER_SIZE, 0);
        assert_eq!(bytes(&pid(RIVAL_3_GEN_2).poll_rate_query().unwrap()), ab);

        let mut readable = 0;
        for m in MOUSE_MODELS {
            let expected = !m.dongle && !m.name.contains("Wireless");
            readable += usize::from(expected);
            assert_eq!(m.firmware_query().is_some(), expected, "{}", m.name);
            assert_eq!(m.poll_rate_query().is_some(), expected, "{}", m.name);
        }
        assert_eq!(
            readable, 6,
            "Aerox 3, Aerox 5, Prime Mini, Rival 3 Gen 2, Rival 5 and its Destiny Edition"
        );
    }

    #[test]
    fn the_captured_firmware_reply_decodes_to_its_version() {
        let reply = captured_firmware_reply();
        assert_eq!(reply_body(&reply, 0x90), Some(&reply[..]));
        assert_eq!(decode_firmware_reply(&reply).as_deref(), Some("1.1.6 +e57ff6a1"));

        let mut numbered = vec![0x00];
        numbered.extend_from_slice(&reply);
        assert_eq!(reply_body(&numbered, 0x90), None, "hidapi hands over no report number");
        assert_eq!(decode_firmware_reply(&numbered), None);

        assert_eq!(reply_body(&reply, 0x21), None);
        assert_eq!(reply_body(&[], 0x90), None);
        assert_eq!(reply_body(&[0x00], 0x90), None);
        assert_eq!(decode_firmware_reply(&captured_echo(0x21)), None);
        assert_eq!(
            decode_firmware_reply(&captured_echo(0x90)),
            None,
            "an echo without a version"
        );
    }

    #[test]
    fn a_reply_is_found_behind_queued_echoes() {
        let mut reports = Reports::new(vec![
            captured_echo(0x34),
            captured_echo(0x21),
            captured_firmware_reply(),
        ]);
        let reply = await_reply(|buffer| reports.read(buffer), 0x90).unwrap().unwrap();
        assert_eq!(reply, captured_firmware_reply());
        assert_eq!(decode_firmware_reply(&reply).as_deref(), Some("1.1.6 +e57ff6a1"));
        assert_eq!(reports.reads, 3);

        let mut numbered = vec![0x00];
        numbered.extend_from_slice(&captured_firmware_reply());
        let mut reports = Reports::new(vec![captured_echo(0x2B), numbered]);
        assert_eq!(
            await_reply(|buffer| reports.read(buffer), 0x90).unwrap(),
            None,
            "a report starting 00 90 answers something else"
        );
        assert_eq!(reports.reads, 3);
    }

    #[test]
    fn no_reply_is_none_not_an_error() {
        let mut silent = Reports::new(vec![]);
        assert_eq!(await_reply(|buffer| silent.read(buffer), 0x90).unwrap(), None);
        assert_eq!(silent.reads, 1, "a read that times out ends the wait");

        let mut chatter = Reports::new(vec![captured_echo(0x21); 20]);
        assert_eq!(await_reply(|buffer| chatter.read(buffer), 0x90).unwrap(), None);
        assert_eq!(chatter.reads, REPLY_READS, "the reads are bounded");

        let mut late = Reports::new(vec![captured_echo(0x21); REPLY_READS]);
        late.queue.push_back(captured_firmware_reply());
        assert_eq!(await_reply(|buffer| late.read(buffer), 0x90).unwrap(), None);

        let unplugged = |_: &mut [u8]| -> Result<usize> { Err(Error::DeviceCommunication("unplugged".to_string())) };
        assert!(await_reply(unplugged, 0x90).is_err());
    }

    #[test]
    fn a_failed_echo_read_is_an_error_and_no_echo_is_not() {
        assert_eq!(
            echo_outcome(Ok(Some(captured_echo(0x21))), "colour").unwrap(),
            SettingAck::Echoed {
                opcode: 0x21,
                status: 0
            }
        );
        assert_eq!(echo_outcome(Ok(None), "colour").unwrap(), SettingAck::NoReply);

        let unplugged = echo_outcome(Err(Error::DeviceCommunication("unplugged".to_string())), "colour");
        let message = unplugged.unwrap_err().to_string();
        assert!(message.contains("colour was sent"), "{message}");
        assert!(message.contains("unplugged"), "{message}");
    }

    #[test]
    fn an_interrupted_echo_read_counts_as_a_timeout() {
        let interrupted = hidapi::HidError::IoError {
            error: std::io::Error::from(std::io::ErrorKind::Interrupted),
        };
        assert_eq!(interrupted_as_timeout(Err(interrupted)).unwrap(), 0);
        assert_eq!(interrupted_as_timeout(Ok(64)).unwrap(), 64);
        let disconnected = hidapi::HidError::HidApiError {
            message: "unexpected poll error (device disconnected)".to_string(),
        };
        assert!(interrupted_as_timeout(Err(disconnected)).is_err());
    }

    #[test]
    fn the_captured_echoes_acknowledge_their_settings() {
        for opcode in [0x34, 0x2B, 0x21] {
            let mut reports = Reports::new(vec![captured_echo(opcode)]);
            let reply = await_reply(|buffer| reports.read(buffer), opcode).unwrap().unwrap();
            assert_eq!(echo_ack(&reply), SettingAck::Echoed { opcode, status: 0 });
        }
        let mut reports = Reports::new(vec![captured_echo(0x34), captured_echo(0x21)]);
        let reply = await_reply(|buffer| reports.read(buffer), 0x21).unwrap().unwrap();
        assert_eq!(
            echo_ack(&reply),
            SettingAck::Echoed {
                opcode: 0x21,
                status: 0
            },
            "the echo of an earlier frame is skipped"
        );
        assert_eq!(
            echo_ack(&[0x21, 0x01]),
            SettingAck::Echoed {
                opcode: 0x21,
                status: 1
            }
        );
        assert_eq!(echo_ack(&[0x21]), SettingAck::NoReply);
        assert_eq!(echo_ack(&[]), SettingAck::NoReply);
    }

    /// Mousework's own reading, the level in the first byte as if the reply carried no echo, does not match the
    /// opcode and gives `None`.
    #[test]
    fn poll_rate_replies_decode_with_mouseworks_level_table() {
        let model = pid(RIVAL_3_GEN_2);
        for (level, hz) in [(1, 1000), (2, 500), (3, 250), (4, 125)] {
            let mut reply = vec![0xAB, level];
            reply.resize(REPLY_LEN, 0);
            assert_eq!(model.decode_poll_rate_reply(&reply), Some(hz), "{level}");
            let mut numbered = vec![0x00];
            numbered.extend_from_slice(&reply);
            assert_eq!(model.decode_poll_rate_reply(&numbered), None, "{level}");
        }
        for reply in [&[0xAB, 0x00][..], &[0xAB, 0x05], &[0xAB], &[0x01], &[0x2B, 0x01], &[]] {
            assert_eq!(model.decode_poll_rate_reply(reply), None, "{reply:02x?}");
        }
    }

    /// The `AB` read-back keeps the first report when none starts with `AB`, so a reply laid out as Mousework reads
    /// it (level 1 in the first byte, no echo) is still shown, undecoded.
    #[test]
    fn the_poll_rate_read_back_falls_back_to_the_first_report() {
        let model = pid(RIVAL_3_GEN_2);
        let mut echoed = vec![0xAB, 0x01];
        echoed.resize(REPLY_LEN, 0);
        let mut reports = Reports::new(vec![captured_echo(0x34), echoed.clone()]);
        let reply = await_reply_or_first(|buffer| reports.read(buffer), 0xAB)
            .unwrap()
            .unwrap();
        assert_eq!(reply, echoed, "a report starting with AB is preferred");
        assert_eq!(model.decode_poll_rate_reply(&reply), Some(1000));

        let mut mousework = vec![0x01];
        mousework.resize(REPLY_LEN, 0);
        let mut reports = Reports::new(vec![mousework.clone()]);
        let reply = await_reply_or_first(|buffer| reports.read(buffer), 0xAB)
            .unwrap()
            .unwrap();
        assert_eq!(reply, mousework);
        assert_eq!(reports.reads, 2, "the wait for an AB report ends at the first timeout");
        assert_eq!(model.decode_poll_rate_reply(&reply), None);

        let mut chatter = Reports::new(vec![captured_echo(0x21); 20]);
        assert_eq!(
            await_reply_or_first(|buffer| chatter.read(buffer), 0xAB).unwrap(),
            Some(captured_echo(0x21)),
            "the first report comes back when none starts with AB"
        );
        assert_eq!(chatter.reads, REPLY_READS, "the reads are bounded");

        let mut silent = Reports::new(vec![]);
        assert_eq!(await_reply_or_first(|buffer| silent.read(buffer), 0xAB).unwrap(), None);
        assert_eq!(silent.reads, 1);

        let unplugged = |_: &mut [u8]| -> Result<usize> { Err(Error::DeviceCommunication("unplugged".to_string())) };
        assert!(await_reply_or_first(unplugged, 0xAB).is_err());
    }

    #[test]
    fn dpi_rejects_out_of_range_and_off_step_values() {
        for (product_id, value) in [
            (AEROX_3, 0),
            (AEROX_3, 100),
            (AEROX_3, 250),
            (AEROX_3, 8600),
            (RIVAL_3_GEN_2, 18000),
            (AEROX_5, 0),
            (AEROX_5, 150),
            (AEROX_5, 18100),
            (AEROX_5, u16::MAX),
            (PRIME_MINI, 49),
            (PRIME_MINI, 75),
            (PRIME_MINI, 18050),
        ] {
            assert!(
                matches!(pid(product_id).dpi_command(value), Err(Error::InvalidConfig(_))),
                "{product_id:#06x} {value}"
            );
        }
        assert!(pid(PRIME_MINI).dpi_command(100).is_ok());
    }

    #[test]
    fn poll_rate_rejects_unknown_rates() {
        for hz in [0, 100, 126, 999, 1001, 2000, 8000] {
            assert!(
                matches!(pid(AEROX_5).poll_rate_command(hz), Err(Error::InvalidConfig(_))),
                "{hz}"
            );
        }
    }

    #[test]
    fn no_command_is_the_save_frame() {
        for m in MOUSE_MODELS {
            let mut frames = vec![
                bytes(&m.dpi_command(m.spec.dpi.min).unwrap()),
                bytes(&m.poll_rate_command(1000).unwrap()),
            ];
            let stages = m.dpi_stages_command(&[m.spec.dpi.min, m.spec.dpi.max], 1);
            frames.extend(stages.iter().map(|s| bytes(s)));
            frames.extend(m.color_commands(Color::WHITE).iter().map(|c| bytes(c)));
            frames.extend(m.battery_query().iter().map(|q| bytes(q)));
            frames.extend(m.firmware_query().iter().map(|q| bytes(q)));
            frames.extend(m.poll_rate_query().iter().map(|q| bytes(q)));
            for frame in frames {
                let opcode = frame[1] & !0x40;
                assert!(
                    matches!(opcode, 0x2D | 0x34 | 0x2B | 0x21 | 0x92 | 0x90 | 0xAB),
                    "{}: {opcode:#04x}",
                    m.name
                );
            }
        }
    }

    #[test]
    fn the_mouse_tags_do_not_clash_with_other_command_codes() {
        assert_eq!(CommandCode::from_report_byte(0x2B), Some(CommandCode::MousePollRate));
        assert_eq!(CommandCode::from_report_byte(0x92), Some(CommandCode::MouseBattery));
        assert_eq!(CommandCode::from_report_byte(0x2D), Some(CommandCode::ActuationControl));
        assert_eq!(CommandCode::from_report_byte(0x21), Some(CommandCode::RgbControl));
        assert_eq!(CommandCode::from_report_byte(0xF2), None);
        assert_eq!(CommandCode::from_report_byte(0xF3), None);
        assert_eq!(CommandCode::from_report_byte(0xF6), None);
        assert!(CommandCode::MouseDpi.to_string().ends_with("_EXPERIMENTAL"));
        assert!(CommandCode::MouseColor.to_string().ends_with("_EXPERIMENTAL"));
        assert!(CommandCode::MouseDpiStages.to_string().ends_with("_EXPERIMENTAL"));
    }
}
