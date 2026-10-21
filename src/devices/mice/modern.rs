//! [EXPERIMENTAL] Modern-family SteelSeries mice: Aerox 3/5/9, Prime Mini, Prime Wireless, Rival 3 Gen 2, Rival 5.
//!
//! The whole protocol comes from a community fork derived from rivalcfg and has not been run on hardware.
//! Commands are output reports `00 <opcode> <args>` of variable length on the model's control interface.
//! The 2.4 GHz receiver PID of a wireless mouse sets bit 0x40 on every opcode and answers each setting with
//! a 64-byte report that is read and dropped. The save command `11 00` (write to on-board memory) is never
//! sent and not implemented.

use super::Mouse;
use super::dpi_tables::{TRUEMOVE_AIR, TRUEMOVE_CORE};
use crate::devices::hid_reports::{
    HidCommand, HidDeviceType, MOUSE_REPORT_BUFFER_SIZE, MouseBatteryQueryCommand, MouseColorCommand, MouseColorLayout,
    MouseDpiCommand, MousePollRateCommand,
};
use crate::devices::{Device, DeviceInfo, DeviceType, product_ids::*};
use crate::rgb::Color;
use crate::{Error, Result};
use hidapi::HidDevice;
use parking_lot::Mutex;
use std::time::{Duration, Instant};

/// Minimum time between two commands; rivalcfg waits 50 ms and warns that less can hang some mice.
const COMMAND_SPACING: Duration = Duration::from_millis(50);
const READ_TIMEOUT_MS: i32 = 200;
/// Size of the report a 2.4 GHz receiver answers each setting with.
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

/// DPI range, encoding and command layout of a model.
#[derive(Clone, Copy, Debug)]
pub struct DpiSpec {
    pub opcode: u8,
    pub min: u16,
    pub max: u16,
    pub step: u16,
    pub encoding: DpiEncoding,
    /// Index of the stage the single DPI value is sent as.
    pub first_preset: u8,
    /// The command carries the code twice, once per axis.
    pub per_axis: bool,
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

/// What the mice of one rivalcfg profile share.
#[derive(Clone, Copy, Debug)]
pub struct MouseSpec {
    pub dpi: DpiSpec,
    /// Codes for 125, 250, 500 and 1000 Hz.
    pub poll_codes: [u8; 4],
    pub color_layout: MouseColorLayout,
    pub zone_count: u8,
    pub battery: bool,
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
    },
    poll_codes: [4, 3, 2, 1],
    color_layout: MouseColorLayout::Mask8,
    zone_count: 3,
    battery: false,
};

const SPEC_RIVAL_3_GEN_2: MouseSpec = MouseSpec {
    dpi: DpiSpec {
        opcode: MouseDpiCommand::OPCODE_RIVAL_3_GEN_2,
        per_axis: true,
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
    },
    poll_codes: [4, 3, 2, 1],
    color_layout: MouseColorLayout::Mask8,
    zone_count: 3,
    battery: false,
};

const SPEC_RIVAL_5: MouseSpec = MouseSpec {
    color_layout: MouseColorLayout::Mask16,
    zone_count: 10,
    ..SPEC_AEROX_5
};

const SPEC_AEROX_WIRELESS: MouseSpec = MouseSpec {
    poll_codes: [3, 2, 1, 0],
    color_layout: MouseColorLayout::Indexed,
    battery: true,
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

/// One USB product ID of a Modern-family mouse.
#[derive(Clone, Copy, Debug)]
pub struct MouseModel {
    pub product_id: u16,
    pub name: &'static str,
    /// USB interface that takes the commands.
    pub interface_number: i32,
    /// 2.4 GHz receiver PID of a dual-mode mouse.
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
            .battery
            .then_some(MouseBatteryQueryCommand { dongle: self.dongle })
    }
}

/// Battery percent from a query reply as returned by hidapi (no report ID): byte 1 holds the charge in 5 percent
/// steps plus one in its low seven bits and the charging flag in bit 7. `None` for a reply that is too short
/// or carries a level outside 0 to 100, as when the mouse is asleep.
pub fn decode_battery(reply: &[u8]) -> Option<u8> {
    let steps = i16::from(*reply.get(1)? & 0x7F);
    u8::try_from((steps - 1) * 5).ok().filter(|level| *level <= 100)
}

/// [EXPERIMENTAL] Modern-family mouse.
pub struct ModernMouse {
    info: DeviceInfo,
    model: &'static MouseModel,
    device: Option<Mutex<HidDevice>>,
    last_write: Option<Instant>,
}

impl ModernMouse {
    pub fn new(info: DeviceInfo, device: HidDevice, model: &'static MouseModel) -> Self {
        Self {
            info,
            model,
            device: Some(Mutex::new(device)),
            last_write: None,
        }
    }

    fn device(&self) -> Result<&Mutex<HidDevice>> {
        self.device
            .as_ref()
            .ok_or(Error::DeviceCommunication("Device not connected".to_string()))
    }

    /// Write one command, keeping [`COMMAND_SPACING`] since the previous one. Written with `hid_write` directly:
    /// the shared padded writer would pad the report and drop a repeated one.
    fn send(&mut self, command: &dyn HidCommand) -> Result<()> {
        if let Some(elapsed) = self.last_write.map(|at| at.elapsed())
            && elapsed < COMMAND_SPACING
        {
            std::thread::sleep(COMMAND_SPACING - elapsed);
        }
        let mut report = [0u8; MOUSE_REPORT_BUFFER_SIZE];
        let len = command.serialize(&mut report, HidDeviceType::Mouse)?;
        let written = self.device()?.lock().write(&report[..len]);
        self.last_write = Some(Instant::now());
        written?;
        Ok(())
    }

    /// Send a setting, then read and drop the receiver's acknowledgement like rivalcfg.
    fn send_setting(&mut self, command: &dyn HidCommand) -> Result<()> {
        self.send(command)?;
        if self.model.dongle {
            let mut ack = [0u8; REPLY_LEN];
            self.device()?.lock().read_timeout(&mut ack, READ_TIMEOUT_MS)?;
        }
        Ok(())
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
        let command = self.model.dpi_command(dpi)?;
        self.send_setting(&command)
    }

    fn set_poll_rate(&mut self, hz: u16) -> Result<()> {
        let command = self.model.poll_rate_command(hz)?;
        self.send_setting(&command)
    }

    fn set_color(&mut self, color: Color) -> Result<()> {
        for command in self.model.color_commands(color) {
            self.send_setting(&command)?;
        }
        Ok(())
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
        assert_eq!(dpi(RIVAL_3_GEN_2, 200), [0x00, 0x34, 0x01, 0x01, 0x04, 0x04]);
        assert_eq!(dpi(RIVAL_3_GEN_2, 1600), [0x00, 0x34, 0x01, 0x01, 0x24, 0x24]);
        assert_eq!(dpi(RIVAL_3_GEN_2, 8500), [0x00, 0x34, 0x01, 0x01, 0xC5, 0xC5]);
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
            frames.extend(m.color_commands(Color::WHITE).iter().map(|c| bytes(c)));
            frames.extend(m.battery_query().iter().map(|q| bytes(q)));
            for frame in frames {
                let opcode = frame[1] & !0x40;
                assert!(
                    matches!(opcode, 0x2D | 0x34 | 0x2B | 0x21 | 0x92),
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
        assert!(CommandCode::MouseDpi.to_string().ends_with("_EXPERIMENTAL"));
        assert!(CommandCode::MouseColor.to_string().ends_with("_EXPERIMENTAL"));
    }
}
