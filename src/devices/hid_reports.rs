//! Structured HID report types for SteelSeries devices.
//!
//! This module provides type-safe HID command construction and validation
//! for SteelSeries keyboards and headsets, replacing primitive byte array building.

use super::key_mapping::{KeyAddress, KeyId, KeyMapping};
use crate::rgb::Color;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;

/// Standard HID report size for SteelSeries keyboards (includes report ID).
pub const KEYBOARD_REPORT_SIZE: usize = 65;

/// Standard HID report size for SteelSeries headsets (no report ID).
pub const HEADSET_REPORT_SIZE: usize = 64;

/// Buffer a Modern-family mouse command is serialized into. The reports themselves are shorter and variable
/// in length (the longest, a Rival 5 logo colour, is 34 bytes), so `serialize` returns the used length.
pub const MOUSE_REPORT_BUFFER_SIZE: usize = 65;

/// Bit set on the first byte of every command sent to the 2.4 GHz receiver PID of a Modern-family mouse.
pub const MOUSE_DONGLE_FLAG: u8 = 0x40;

/// Maximum number of RGB zones supported by keyboards.
pub const MAX_RGB_ZONES: usize = 12;

/// SteelSeries HID command codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum CommandCode {
    /// Apply/Save settings (0x09)
    Apply = 0x09,
    /// RGB zone control (0x21)
    RgbControl = 0x21,
    /// Brightness control (0x22)
    Brightness = 0x22,
    /// Reactive mode (0x25)
    ReactiveMode = 0x25,
    /// Color shift (0x26)
    ColorShift = 0x26,
    /// Per-key RGB control (0x23)
    /// NOTE: This command code is a placeholder. The actual per-key RGB command
    /// for SteelSeries keyboards has not been discovered and may be different.
    PerKeyRgb = 0x23,
    /// [EXPERIMENTAL] Apex Pro TKL 2023 direct-mode lighting frame (0x40), sent as a feature
    /// report by `ApexProTkl2023`. See docs/development/apex-pro-tkl-2023-gg-capture.md.
    Apex2023Direct = 0x40,
    /// [EXPERIMENTAL] Direct-mode lighting frame of the wireless Apex Pro TKL models (0x61), see
    /// [`DirectFrameCommand`]. Not listed as confirmed in `.claude/rules/experimental-protocol.md`.
    Apex2023DirectWireless = 0x61,
    /// [EXPERIMENTAL] Direct-mode lighting frame of the original Apex Pro (0x3A), from OpenRGB, untested.
    ApexGen1Direct = 0x3A,
    /// [EXPERIMENTAL] Apex M750 direct frame, from a community fork, untested. The frame has no command byte
    /// at offset 1; `0x8E` is byte 4 of its header `00 00 00 01 8E 01 03 06 16`. See [`ApexMDirectCommand`].
    ApexMDirect = 0x8E,
    /// [EXPERIMENTAL] Apex M750 enable sequence, from a community fork, untested. The three reports have no
    /// shared leading byte: `0x01` is byte 4 of the first and third, the second carries `03` there (owned
    /// by [`CommandCode::ProfileChunk`]). See [`ApexMEnableCommand`].
    ApexMEnable = 0x01,
    /// [EXPERIMENTAL] Gen 3 init feature report (0x4B) sent once before the first direct frame, see
    /// [`DirectInitCommand`].
    Apex2023Init = 0x4B,
    /// Actuation point control (0x2D) - EXPERIMENTAL
    /// NOTE: This command code is experimental based on hardware research.
    ActuationControl = 0x2D,
    /// [EXPERIMENTAL] Apex Pro TKL 2023 on-board profile chunk (0x03, header `03 01 80`), see
    /// `ProfileChunkCommand`. The legacy Apex family uses the same command code with a different header
    /// (`03 00 | buffer offset u16 | size u16 | payload`), confirmed on hardware on an Apex 7, see
    /// [`OnboardBufferWriteCommand`].
    ProfileChunk = 0x03,
    /// Legacy Apex (Apex 7, Apex Raw, Apex 2014) resource-buffer pull (0x85), see
    /// `OnboardPayloadPullCommand`: answered by the keyboard's ordinary input report, not by a pull-specific
    /// one. Confirmed on hardware on an Apex 7 (`0x1612`).
    OnboardPayloadPull = 0x85,
    /// Legacy Apex resource-buffer push into a profile file (0x05), see [`OnboardPayloadPushCommand`]:
    /// the write side of the transfer, answered by the interrupt report `05 01 …`. Confirmed on hardware on
    /// an Apex 7 (`0x1612`).
    OnboardPayloadPush = 0x05,
    /// Legacy Apex on-board file open (0x88), see [`OnboardFileOpenCommand`]: the frame the
    /// write API sends before it pushes a file, answered by the interrupt report `88 01 …`. Confirmed on
    /// hardware on an Apex 7 (`0x1612`).
    OnboardFileOpen = 0x88,
    /// Legacy Apex profile commit (0xEB), see [`OnboardFileCommitCommand`]: the frame the write
    /// API sends after the last push of the profile region. Confirmed on hardware on an Apex 7 (`0x1612`).
    OnboardFileCommit = 0xEB,
    /// Legacy Apex on-board payload read (0x83), see `OnboardBufferReadCommand`. Confirmed on hardware on an
    /// Apex 7 (`0x1612`).
    OnboardBufferRead = 0x83,
    /// Legacy Apex firmware-version poll (0x90): `90 <source>`, answered on the interrupt
    /// endpoint with the ASCII firmware version. The source byte selects which firmware answers -
    /// [`FirmwareSource::Keyboard`] (`90 00`) the keyboard's, [`FirmwareSource::Led`] (`90 01`) the LED
    /// controller's. See [`OnboardFirmwareVersionCommand`]. Confirmed on hardware on an Apex 7 (`0x1612`).
    OnboardFirmwareVersion = 0x90,
    /// `[EXPERIMENTAL]` Legacy Apex driver mode (0x6C): GG's engine writes `6c 00 01` when it opens a
    /// device (`DriverModeEnable` in its init-sequence descriptor, the fourth frame of its opening
    /// chain, in the read capture and answered by nothing) and `6c 00 00` at session close (engine-side
    /// only). Same report number 0, same command, one byte apart. **Nothing in ssgg sends it.** See
    /// [`OnboardDriverModeCommand`]. GG's chain also carries `f4 00`, answered `1`, whose meaning is
    /// unknown and which `ssgg` does not implement at all.
    OnboardDriverMode = 0x6C,
    /// Legacy Apex profile deploy (0x89): GG's `89 <1-based profile id>`, the
    /// `load_profile` frame seen behind profile-file transfers in the reference capture (slots 0, 1, 2, 4
    /// going out as 1, 2, 3, 5). **This writes to the device** - the keyboard runs the slot named, which is
    /// what a deployed slot answers with: confirmed on hardware on an Apex 7 (`0x1612`). The
    /// `ssgg onboard write` and `ssgg onboard load` commands send it, on request only; a plain read sends
    /// nothing. See [`OnboardProfileDeployCommand`].
    OnboardProfileDeploy = 0x89,
    /// [EXPERIMENTAL] Arctis Nova 7 family sidetone level (0x39, `00 39 <0..=3>`), see
    /// [`HeadsetSettingCommand`]. From community forks, untested on hardware.
    HeadsetSidetone = 0x39,
    /// [EXPERIMENTAL] Arctis Nova 7 family microphone volume (0x37, `00 37 <0..=7>`), see
    /// [`HeadsetSettingCommand`]. From community forks, untested on hardware.
    HeadsetMicVolume = 0x37,
    /// [EXPERIMENTAL] Arctis Nova 7 family inactivity auto-off in minutes (0xA3, `00 a3 <minutes>`), see
    /// [`HeadsetSettingCommand`]. From community forks, untested on hardware.
    HeadsetAutoOff = 0xA3,
    /// [EXPERIMENTAL] Arctis Nova 7 family status query (0xB0, `00 b0`), answered by an input report that
    /// starts with `b0`, see [`HeadsetStatusQueryCommand`]. From community forks, untested on hardware.
    HeadsetStatus = 0xB0,
    /// [EXPERIMENTAL] Modern-family mouse polling rate (0x2B, `00 2b <code>`; 0x6B on a 2.4 GHz receiver PID),
    /// see [`MousePollRateCommand`]. From a community rivalcfg-derived fork, untested on hardware.
    MousePollRate = 0x2B,
    /// [EXPERIMENTAL] Modern-family mouse battery query (0x92, `00 92`; 0xD2 on a 2.4 GHz receiver PID), see
    /// [`MouseBatteryQueryCommand`]. From a community rivalcfg-derived fork, untested on hardware.
    MouseBattery = 0x92,
    /// [EXPERIMENTAL] Modern-family mouse DPI, see [`MouseDpiCommand`]. A tag, never a wire byte: the wire opcode
    /// is 0x2D on every model but the Rival 3 Gen 2 (0x34), and 0x2D already belongs to
    /// [`CommandCode::ActuationControl`]. From a community rivalcfg-derived fork, untested on hardware.
    MouseDpi = 0xF2,
    /// [EXPERIMENTAL] Modern-family mouse LED colour, see [`MouseColorCommand`]. A tag, never a wire byte: the
    /// wire opcode is 0x21 (0x61 on a 2.4 GHz receiver PID), which belong to
    /// [`CommandCode::RgbControl`] and [`CommandCode::Apex2023DirectWireless`]. From a community
    /// rivalcfg-derived fork, untested on hardware.
    MouseColor = 0xF3,
}

impl fmt::Display for CommandCode {
    /// The `_EXPERIMENTAL` suffix marks a code whose protocol is not confirmed on hardware
    /// (`.claude/rules/experimental-protocol.md`); confirmed codes carry the bare label.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandCode::Apply => write!(f, "APPLY"),
            CommandCode::RgbControl => write!(f, "RGB_CTRL"),
            CommandCode::Brightness => write!(f, "BRIGHTNESS"),
            CommandCode::ReactiveMode => write!(f, "REACTIVE"),
            CommandCode::ColorShift => write!(f, "COLOR_SHIFT"),
            CommandCode::PerKeyRgb => write!(f, "PERKEY_RGB_EXPERIMENTAL"),
            CommandCode::Apex2023Direct => write!(f, "APEX2023_DIRECT_EXPERIMENTAL"),
            CommandCode::Apex2023DirectWireless => write!(f, "APEX2023_DIRECT_WIRELESS_EXPERIMENTAL"),
            CommandCode::ApexGen1Direct => write!(f, "APEX_GEN1_DIRECT_EXPERIMENTAL"),
            CommandCode::ApexMDirect => write!(f, "APEX_M_DIRECT_EXPERIMENTAL"),
            CommandCode::ApexMEnable => write!(f, "APEX_M_ENABLE_EXPERIMENTAL"),
            CommandCode::Apex2023Init => write!(f, "APEX2023_INIT_EXPERIMENTAL"),
            CommandCode::ActuationControl => write!(f, "ACTUATION_CTRL_EXPERIMENTAL"),
            CommandCode::ProfileChunk => write!(f, "PROFILE_CHUNK_EXPERIMENTAL"),
            CommandCode::OnboardPayloadPull => write!(f, "ONBOARD_PULL"),
            CommandCode::OnboardPayloadPush => write!(f, "ONBOARD_PUSH"),
            CommandCode::OnboardFileOpen => write!(f, "ONBOARD_FILE_OPEN"),
            CommandCode::OnboardFileCommit => write!(f, "ONBOARD_COMMIT"),
            CommandCode::OnboardBufferRead => write!(f, "ONBOARD_PAYLOAD_READ"),
            CommandCode::OnboardFirmwareVersion => write!(f, "FIRMWARE_VERSION"),
            CommandCode::OnboardDriverMode => write!(f, "DRIVER_MODE_EXPERIMENTAL"),
            CommandCode::OnboardProfileDeploy => write!(f, "LOAD_PROFILE"),
            CommandCode::HeadsetSidetone => write!(f, "HEADSET_SIDETONE_EXPERIMENTAL"),
            CommandCode::HeadsetMicVolume => write!(f, "HEADSET_MIC_VOLUME_EXPERIMENTAL"),
            CommandCode::HeadsetAutoOff => write!(f, "HEADSET_AUTO_OFF_EXPERIMENTAL"),
            CommandCode::HeadsetStatus => write!(f, "HEADSET_STATUS_EXPERIMENTAL"),
            CommandCode::MousePollRate => write!(f, "MOUSE_POLL_RATE_EXPERIMENTAL"),
            CommandCode::MouseBattery => write!(f, "MOUSE_BATTERY_EXPERIMENTAL"),
            CommandCode::MouseDpi => write!(f, "MOUSE_DPI_EXPERIMENTAL"),
            CommandCode::MouseColor => write!(f, "MOUSE_COLOR_EXPERIMENTAL"),
        }
    }
}

impl CommandCode {
    /// Parse a command byte found in a HID report.
    ///
    /// This keeps report-byte handling in one place, including compatibility
    /// with legacy captures that have used `0x2A` for placeholder per-key RGB.
    pub const fn from_report_byte(value: u8) -> Option<Self> {
        match value {
            0x09 => Some(CommandCode::Apply),
            0x21 => Some(CommandCode::RgbControl),
            0x22 => Some(CommandCode::Brightness),
            0x25 => Some(CommandCode::ReactiveMode),
            0x26 => Some(CommandCode::ColorShift),
            0x23 | 0x2A => Some(CommandCode::PerKeyRgb),
            0x40 => Some(CommandCode::Apex2023Direct),
            0x61 => Some(CommandCode::Apex2023DirectWireless),
            0x3A => Some(CommandCode::ApexGen1Direct),
            0x4B => Some(CommandCode::Apex2023Init),
            0x2D => Some(CommandCode::ActuationControl),
            0x03 => Some(CommandCode::ProfileChunk),
            0x85 => Some(CommandCode::OnboardPayloadPull),
            0x05 => Some(CommandCode::OnboardPayloadPush),
            0x88 => Some(CommandCode::OnboardFileOpen),
            0xEB => Some(CommandCode::OnboardFileCommit),
            0x83 => Some(CommandCode::OnboardBufferRead),
            0x90 => Some(CommandCode::OnboardFirmwareVersion),
            0x6C => Some(CommandCode::OnboardDriverMode),
            0x89 => Some(CommandCode::OnboardProfileDeploy),
            0x39 => Some(CommandCode::HeadsetSidetone),
            0x37 => Some(CommandCode::HeadsetMicVolume),
            0xA3 => Some(CommandCode::HeadsetAutoOff),
            0xB0 => Some(CommandCode::HeadsetStatus),
            0x2B => Some(CommandCode::MousePollRate),
            0x92 => Some(CommandCode::MouseBattery),
            _ => None,
        }
    }
}

/// Device type for HID report sizing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HidDeviceType {
    /// Keyboard (65-byte reports with report ID)
    Keyboard,
    /// Headset (64-byte reports without report ID)
    Headset,
    /// Mouse (variable-length reports that start with the report ID, at most [`MOUSE_REPORT_BUFFER_SIZE`])
    Mouse,
}

impl HidDeviceType {
    /// Get the report size for this device type.
    pub fn report_size(self) -> usize {
        match self {
            HidDeviceType::Keyboard => KEYBOARD_REPORT_SIZE,
            HidDeviceType::Headset => HEADSET_REPORT_SIZE,
            HidDeviceType::Mouse => MOUSE_REPORT_BUFFER_SIZE,
        }
    }

    /// Whether this device type includes a report ID byte.
    pub fn includes_report_id(self) -> bool {
        matches!(self, HidDeviceType::Keyboard | HidDeviceType::Mouse)
    }
}

/// A structured HID command that can be serialized to bytes.
pub trait HidCommand {
    /// Get the command code for this command.
    fn command_code(&self) -> CommandCode;

    /// Serialize the command to a byte buffer.
    /// Returns the number of bytes written.
    fn serialize(&self, buffer: &mut [u8], device_type: HidDeviceType) -> Result<usize>;

    /// Validate the command parameters.
    fn validate(&self) -> Result<()>;

    /// Get a human-readable description of the command.
    fn description(&self) -> String;
}

/// RGB zone control command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RgbZoneCommand<'a> {
    /// Zone colors (up to MAX_RGB_ZONES)
    pub colors: Cow<'a, [Color]>,
    /// Zone selector (0xFF for all zones)
    pub zone_selector: u8,
}

impl<'a> RgbZoneCommand<'a> {
    /// Create a new RGB zone command for all zones.
    pub fn new_all_zones(colors: &'a [Color]) -> Self {
        Self {
            colors: Cow::Borrowed(colors),
            zone_selector: 0xFF,
        }
    }

    /// Create a new RGB zone command with a single color for all zones.
    pub fn new_single_color(color: Color, zone_count: usize) -> Self {
        Self {
            colors: Cow::Owned(vec![color; zone_count]),
            zone_selector: 0xFF,
        }
    }

    /// Create a new RGB zone command for a specific zone.
    pub fn new_specific_zone(zone_index: u8, color: Color) -> Self {
        let mut colors = vec![Color::BLACK; MAX_RGB_ZONES];
        if zone_index < MAX_RGB_ZONES as u8 {
            colors[zone_index as usize] = color;
        }

        Self {
            colors: Cow::Owned(colors),
            zone_selector: zone_index,
        }
    }
}

impl<'a> HidCommand for RgbZoneCommand<'a> {
    fn command_code(&self) -> CommandCode {
        CommandCode::RgbControl
    }

    fn serialize(&self, buffer: &mut [u8], device_type: HidDeviceType) -> Result<usize> {
        self.validate()?;

        let report_size = device_type.report_size();
        if buffer.len() < report_size {
            return Err(Error::DeviceCommunication(format!(
                "Buffer too small: {} bytes (expected {})",
                buffer.len(),
                report_size
            )));
        }

        buffer[..report_size].fill(0);

        let mut offset = 0;

        if device_type.includes_report_id() {
            buffer[0] = 0x00;
            offset = 1;
        }

        buffer[offset] = self.command_code() as u8;
        offset += 1;

        buffer[offset] = self.zone_selector;
        offset += 1;

        for color in self.colors.iter() {
            if offset + 2 >= report_size {
                break;
            }
            buffer[offset] = color.r;
            buffer[offset + 1] = color.g;
            buffer[offset + 2] = color.b;
            offset += 3;
        }

        Ok(report_size)
    }

    fn validate(&self) -> Result<()> {
        if self.colors.is_empty() {
            return Err(Error::DeviceCommunication(
                "RGB command must have at least one color".to_string(),
            ));
        }

        if self.colors.len() > MAX_RGB_ZONES {
            return Err(Error::DeviceCommunication(format!(
                "Too many RGB zones: {} (max {})",
                self.colors.len(),
                MAX_RGB_ZONES
            )));
        }

        Ok(())
    }

    fn description(&self) -> String {
        if self.zone_selector == 0xFF {
            format!("Set {} zones to RGB colors", self.colors.len())
        } else {
            format!("Set zone {} to RGB color", self.zone_selector)
        }
    }
}

/// Brightness control command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrightnessCommand {
    /// Brightness value (0-100)
    pub brightness: u8,
}

impl BrightnessCommand {
    /// Create a new brightness command.
    pub fn new(brightness: u8) -> Self {
        Self {
            brightness: brightness.min(100), // Auto-clamp
        }
    }
}

impl HidCommand for BrightnessCommand {
    fn command_code(&self) -> CommandCode {
        CommandCode::Brightness
    }

    fn serialize(&self, buffer: &mut [u8], device_type: HidDeviceType) -> Result<usize> {
        self.validate()?;

        let report_size = device_type.report_size();
        if buffer.len() < report_size {
            return Err(Error::DeviceCommunication(format!(
                "Buffer too small: {} bytes (expected {})",
                buffer.len(),
                report_size
            )));
        }

        buffer[..report_size].fill(0);
        let mut offset = 0;

        // Add report ID for keyboards
        if device_type.includes_report_id() {
            buffer[0] = 0x00; // Report ID
            offset = 1;
        }

        // Command code and brightness
        buffer[offset] = self.command_code() as u8;
        buffer[offset + 1] = self.brightness;

        Ok(report_size)
    }

    fn validate(&self) -> Result<()> {
        if self.brightness > 100 {
            return Err(Error::DeviceCommunication(format!(
                "Invalid brightness value: {} (max 100)",
                self.brightness
            )));
        }
        Ok(())
    }

    fn description(&self) -> String {
        format!("Set brightness to {}%", self.brightness)
    }
}

/// [EXPERIMENTAL] Brightness for 8-zone Apex keyboards (Apex 3 TKL): `00 23 <0x00-0x10>`.
///
/// Source: OpenRGB `SteelSeriesApex8ZoneController.cpp` (`SetBrightness`), untested on hardware.
/// On this controller `0x22` (`CommandCode::Brightness`) selects the rainbow-wave mode instead.
/// `CommandCode` cannot hold a second `0x23` variant, so `command_code()` (used only for logging) reports
/// `PerKeyRgb`; the opcode is written from `OPCODE`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Apex8ZoneBrightnessCommand {
    /// Device brightness level (0x00-0x10).
    pub level: u8,
}

impl Apex8ZoneBrightnessCommand {
    /// Maximum device brightness level.
    pub const MAX_LEVEL: u8 = 0x10;
    /// Brightness opcode on 8-zone controllers. Same byte as the `PerKeyRgb` placeholder, unrelated meaning.
    const OPCODE: u8 = 0x23;

    /// Create from a 0-100 percentage, rounded to the nearest device level.
    pub fn from_percent(percent: u8) -> Self {
        let percent = u16::from(percent.min(100));
        Self {
            level: ((percent * u16::from(Self::MAX_LEVEL) + 50) / 100) as u8,
        }
    }
}

impl HidCommand for Apex8ZoneBrightnessCommand {
    fn command_code(&self) -> CommandCode {
        CommandCode::PerKeyRgb
    }

    fn serialize(&self, buffer: &mut [u8], device_type: HidDeviceType) -> Result<usize> {
        self.validate()?;

        let report_size = device_type.report_size();
        if buffer.len() < report_size {
            return Err(Error::DeviceCommunication(format!(
                "Buffer too small: {} bytes (expected {})",
                buffer.len(),
                report_size
            )));
        }

        buffer[..report_size].fill(0);
        let offset = usize::from(device_type.includes_report_id());
        buffer[offset] = Self::OPCODE;
        buffer[offset + 1] = self.level;

        Ok(report_size)
    }

    fn validate(&self) -> Result<()> {
        if self.level > Self::MAX_LEVEL {
            return Err(Error::DeviceCommunication(format!(
                "Invalid 8-zone brightness level: {} (max {})",
                self.level,
                Self::MAX_LEVEL
            )));
        }
        Ok(())
    }

    fn description(&self) -> String {
        format!("Set 8-zone brightness to level {}", self.level)
    }
}

/// [EXPERIMENTAL] Report-ID byte plus the 644 wire bytes of one profile chunk feature report.
pub const PROFILE_CHUNK_REPORT_SIZE: usize = 645;

/// [EXPERIMENTAL] One chunk of the wired Apex Pro TKL 2023 (PID 0x1628) on-board profile upload, a feature
/// report: `00 | 03 01 80 | len u16 LE | offset u16 LE | 00 00 | data | zero padding`.
///
/// Layout from GG USBPcap captures (2026-10-14). Nothing sends it: no profile read-back is known, so a write
/// would replace the board's other settings (docs/development/apex-pro-tkl-2023-gg-capture.md).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileChunkCommand<'a> {
    /// Byte offset of `data` in the 12,500-byte profile blob.
    pub offset: u16,
    /// Chunk payload (GG always sent 500 bytes).
    pub data: &'a [u8],
}

impl ProfileChunkCommand<'_> {
    /// Largest payload seen in a chunk.
    pub const MAX_DATA_LEN: usize = 500;
    const HEADER: [u8; 3] = [CommandCode::ProfileChunk as u8, 0x01, 0x80];
    const DATA_OFFSET: usize = 10;
}

impl HidCommand for ProfileChunkCommand<'_> {
    fn command_code(&self) -> CommandCode {
        CommandCode::ProfileChunk
    }

    fn serialize(&self, buffer: &mut [u8], _device_type: HidDeviceType) -> Result<usize> {
        self.validate()?;
        if buffer.len() < PROFILE_CHUNK_REPORT_SIZE {
            return Err(Error::DeviceCommunication(format!(
                "Buffer too small: {} bytes (expected {})",
                buffer.len(),
                PROFILE_CHUNK_REPORT_SIZE
            )));
        }

        let report = &mut buffer[..PROFILE_CHUNK_REPORT_SIZE];
        report.fill(0);
        report[1..4].copy_from_slice(&Self::HEADER);
        report[4..6].copy_from_slice(&(self.data.len() as u16).to_le_bytes());
        report[6..8].copy_from_slice(&self.offset.to_le_bytes());
        report[Self::DATA_OFFSET..Self::DATA_OFFSET + self.data.len()].copy_from_slice(self.data);
        Ok(PROFILE_CHUNK_REPORT_SIZE)
    }

    fn validate(&self) -> Result<()> {
        if self.data.is_empty() || self.data.len() > Self::MAX_DATA_LEN {
            return Err(Error::DeviceCommunication(format!(
                "Profile chunk must carry 1-{} bytes, got {}",
                Self::MAX_DATA_LEN,
                self.data.len()
            )));
        }
        Ok(())
    }

    fn description(&self) -> String {
        format!("Profile chunk: {} bytes at offset {}", self.data.len(), self.offset)
    }
}

/// [EXPERIMENTAL] Direct-mode lighting frame, a feature report:
/// `00 | packet id | key count | (usage R G B) x count | zero padding`, `report_len` bytes in all.
///
/// Packet id `0x40` (wired Apex Pro TKL 2023, Gen 3 and `[EXPERIMENTAL]` the Apex 9 pair), `0x61` (wireless
/// Gen 3) or `0x3A` (original Apex Pro and `[EXPERIMENTAL]` four more Gen 1 models, from OpenRGB and a
/// community fork). One frame replaces every key. Hardware evidence is thin: a solid-colour and a
/// per-key `0x40` frame were replayed on the wired Apex Pro TKL 2023 (`0x1628`, 2026-10-14 and 2026-10-21);
/// see docs/development/apex-pro-tkl-2023-gg-capture.md.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectFrameCommand<'a> {
    /// Frame packet id: [`CommandCode::Apex2023Direct`], [`CommandCode::Apex2023DirectWireless`] or
    /// [`CommandCode::ApexGen1Direct`].
    pub packet: CommandCode,
    /// Report length including the report-ID byte (643; 645 for the wired 2023 model; 513 for the Apex 9 pair).
    pub report_len: usize,
    /// `(HID usage, colour)` per key, in the order the board expects.
    pub entries: &'a [(u8, Color)],
}

impl DirectFrameCommand<'_> {
    const HEADER_LEN: usize = 3;
    const ENTRY_LEN: usize = 4;
}

impl HidCommand for DirectFrameCommand<'_> {
    fn command_code(&self) -> CommandCode {
        self.packet
    }

    fn serialize(&self, buffer: &mut [u8], _device_type: HidDeviceType) -> Result<usize> {
        self.validate()?;
        check_report_buffer(buffer, self.report_len)?;
        let report = &mut buffer[..self.report_len];
        report.fill(0);
        report[1] = self.packet as u8;
        report[2] = self.entries.len() as u8;
        let (slots, _) = report[Self::HEADER_LEN..].as_chunks_mut::<4>();
        for (slot, (usage, color)) in slots.iter_mut().zip(self.entries) {
            *slot = [*usage, color.r, color.g, color.b];
        }
        Ok(self.report_len)
    }

    fn validate(&self) -> Result<()> {
        if !matches!(
            self.packet,
            CommandCode::Apex2023Direct | CommandCode::Apex2023DirectWireless | CommandCode::ApexGen1Direct
        ) {
            return Err(Error::InvalidConfig(format!(
                "{} is not a direct-mode frame packet id",
                self.packet
            )));
        }
        let needed = Self::HEADER_LEN + self.entries.len() * Self::ENTRY_LEN;
        if self.entries.len() > usize::from(u8::MAX) || needed > self.report_len {
            return Err(Error::InvalidConfig(format!(
                "{} direct-mode entries need {} bytes, the {}-byte report holds fewer",
                self.entries.len(),
                needed,
                self.report_len
            )));
        }
        Ok(())
    }

    fn description(&self) -> String {
        format!(
            "Direct frame {:#04x}: {} keys in {} bytes",
            self.packet as u8,
            self.entries.len(),
            self.report_len
        )
    }
}

/// [EXPERIMENTAL] Gen 3 init feature report `00 4B` + zeros (643 bytes), sent once before the first
/// [`DirectFrameCommand`] on the models that need it. The controller needs a short pause afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectInitCommand;

impl DirectInitCommand {
    /// Report length including the report-ID byte.
    pub const REPORT_SIZE: usize = 643;
}

impl HidCommand for DirectInitCommand {
    fn command_code(&self) -> CommandCode {
        CommandCode::Apex2023Init
    }

    fn serialize(&self, buffer: &mut [u8], _device_type: HidDeviceType) -> Result<usize> {
        check_report_buffer(buffer, Self::REPORT_SIZE)?;
        let report = &mut buffer[..Self::REPORT_SIZE];
        report.fill(0);
        report[1] = CommandCode::Apex2023Init as u8;
        Ok(Self::REPORT_SIZE)
    }

    fn validate(&self) -> Result<()> {
        Ok(())
    }

    fn description(&self) -> String {
        "Direct-mode init (0x4B)".to_string()
    }
}

/// [EXPERIMENTAL] Apex M750 feature report size, report-ID byte included.
pub const APEX_M_REPORT_SIZE: usize = 513;

/// [EXPERIMENTAL] LED slots in an Apex M750 direct frame. Which slot lights which key is unknown.
pub const APEX_M_SLOT_COUNT: usize = 132;

/// One report of the Apex M750 enable sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApexMEnableStep {
    First,
    Second,
    Third,
}

impl ApexMEnableStep {
    /// The sequence in the order it is sent.
    pub const ALL: [Self; 3] = [Self::First, Self::Second, Self::Third];
}

/// [EXPERIMENTAL] One report of the Apex M750 enable sequence that has to precede the first
/// [`ApexMDirectCommand`]: a 513-byte feature report, all zeros but a few bytes (indexes count the report
/// ID as 0): first `[4]=01 [6]=85`, second `[4]=03 [5]=01 [7]=FF`, third `[4]=01 [6]=85 [7]=FF`.
///
/// Layout from a community fork, untested on hardware (issue #276). Nothing here saves or persists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApexMEnableCommand {
    pub step: ApexMEnableStep,
}

impl HidCommand for ApexMEnableCommand {
    fn command_code(&self) -> CommandCode {
        CommandCode::ApexMEnable
    }

    fn serialize(&self, buffer: &mut [u8], _device_type: HidDeviceType) -> Result<usize> {
        check_report_buffer(buffer, APEX_M_REPORT_SIZE)?;
        let report = &mut buffer[..APEX_M_REPORT_SIZE];
        report.fill(0);
        match self.step {
            ApexMEnableStep::First => {
                report[4] = 0x01;
                report[6] = 0x85;
            }
            ApexMEnableStep::Second => {
                report[4] = 0x03;
                report[5] = 0x01;
                report[7] = 0xFF;
            }
            ApexMEnableStep::Third => {
                report[4] = 0x01;
                report[6] = 0x85;
                report[7] = 0xFF;
            }
        }
        Ok(APEX_M_REPORT_SIZE)
    }

    fn validate(&self) -> Result<()> {
        Ok(())
    }

    fn description(&self) -> String {
        format!("Apex M750 enable ({:?})", self.step)
    }
}

/// [EXPERIMENTAL] Apex M750 direct frame, a 513-byte feature report: header `00 00 00 01 8E 01 03 06 16`,
/// then one `R G B` triple per slot from index 9, zero padding after. A slot that maps to no LED is
/// `FF 32 00` in the fork this comes from, but the slot-to-key map is unknown, so callers fill every slot.
///
/// Layout from a community fork, untested on hardware (issue #276).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApexMDirectCommand<'a> {
    /// Exactly [`APEX_M_SLOT_COUNT`] colours.
    pub slots: &'a [Color],
}

impl ApexMDirectCommand<'_> {
    const HEADER: [u8; 9] = [
        0x00,
        0x00,
        0x00,
        0x01,
        CommandCode::ApexMDirect as u8,
        0x01,
        0x03,
        0x06,
        0x16,
    ];
}

impl HidCommand for ApexMDirectCommand<'_> {
    fn command_code(&self) -> CommandCode {
        CommandCode::ApexMDirect
    }

    fn serialize(&self, buffer: &mut [u8], _device_type: HidDeviceType) -> Result<usize> {
        self.validate()?;
        check_report_buffer(buffer, APEX_M_REPORT_SIZE)?;
        let report = &mut buffer[..APEX_M_REPORT_SIZE];
        report.fill(0);
        report[..Self::HEADER.len()].copy_from_slice(&Self::HEADER);
        let (triples, _) = report[Self::HEADER.len()..].as_chunks_mut::<3>();
        for (triple, color) in triples.iter_mut().zip(self.slots) {
            *triple = [color.r, color.g, color.b];
        }
        Ok(APEX_M_REPORT_SIZE)
    }

    fn validate(&self) -> Result<()> {
        if self.slots.len() != APEX_M_SLOT_COUNT {
            return Err(Error::InvalidConfig(format!(
                "Apex M750 frame needs {APEX_M_SLOT_COUNT} slots, got {}",
                self.slots.len()
            )));
        }
        Ok(())
    }

    fn description(&self) -> String {
        format!("Apex M750 direct frame: {} slots", self.slots.len())
    }
}

/// Report buffer of a legacy Apex on-board profile transfer, handed to
/// the transport as `[report number][64 data bytes]` with report number 0 - the
/// `SET_REPORT wValue 0x0200 wIndex 1 wLength 64` transfer the reference capture shows.
///
/// Confirmed on hardware on an Apex 7 (`0x1612`): the firmware poll, the pull, the file open, the push, the
/// commit and the deploy all go out in this buffer and the board answers the ones the capture shows answered.
pub const ONBOARD_OUTPUT_REPORT_SIZE: usize = 65;

/// Report buffer of a legacy Apex on-board payload read: `[report number][642 data bytes]` with report
/// number 0, which is the `SET_REPORT wValue 0x0300 wLength 642` transfer the reference capture shows
/// (the kernel takes the report number out of the data stage, so the buffer is one byte longer than the
/// wire, and the 642 data bytes carry the frame zero padded).
///
/// Confirmed on hardware on an Apex 7 (`0x1612`): the payload read request and both write-side buffer writes
/// go out in this buffer.
pub const ONBOARD_FEATURE_REPORT_SIZE: usize = 643;

/// Wire bytes of a payload read-back: the `GET_REPORT wValue 0x0300 wIndex 1 wLength 642` control read
/// the reference capture shows, which is also the length to ask the transport for. On Linux the read side
/// sends the buffer length verbatim (`HIDIOCGFEATURE` hands the slice length straight to the ioctl), so
/// **ask with 642, never 643**: 642 answers, 643 makes the control transfer never complete
/// (`Connection timed out`). The kernel writes the device's report-ID byte into buffer[0], so the bytes
/// behind it are 641.
///
/// `[EXPERIMENTAL]` Wire length capture-confirmed on Windows, answering length measured on Linux.
pub const ONBOARD_READ_WIRE_BYTES: usize = 642;

/// Bytes a payload read frame may ask for. A reply report arrives as `[report ID][641 bytes]`, and a
/// header-echoing reply puts a further 5-byte header (`03 01 01 00 01`, see `READ_REPLY_HEADER_LEN` in
/// `apex_7_profile`) in front of the payload, so the payload a read can ask for tops out at
/// 642 - 1 - 5 = 636: an ask above it is a size no reply can carry.
pub const ONBOARD_MAX_READ_LEN: usize = ONBOARD_READ_WIRE_BYTES - 1 - 5;

/// Firmware source byte for [`OnboardFirmwareVersionCommand`]: which firmware the `90` poll asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareSource {
    /// `0x00` - the firmware running the keyboard.
    Keyboard,
    /// `0x01` - the LED controller's firmware.
    Led,
}

impl From<FirmwareSource> for u8 {
    fn from(source: FirmwareSource) -> Self {
        match source {
            FirmwareSource::Keyboard => 0x00,
            FirmwareSource::Led => 0x01,
        }
    }
}

impl fmt::Display for FirmwareSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FirmwareSource::Keyboard => write!(f, "keyboard"),
            FirmwareSource::Led => write!(f, "LED"),
        }
    }
}

/// Legacy Apex firmware-version poll: `90 <source>` in a 64-byte output report
/// (`SET_REPORT wValue 0x0200`), answered on the interrupt endpoint with the ASCII version string
/// (`34 2e 31 …` = `4.16.8` in the reference capture).
///
/// The source byte picks which firmware answers: [`FirmwareSource::Keyboard`] (`90 00`) the keyboard's,
/// [`FirmwareSource::Led`] (`90 01`) the LED controller's - both forms are in the GG capture (`90 00`
/// `90 00` answered `00 53 74 …` = `SteelSeries ...`) and both answer from an Apex 7 (`0x1612`), confirmed
/// on hardware. Decode the answer with `parse_firmware_version`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnboardFirmwareVersionCommand {
    /// Which firmware this poll asks for.
    pub source: FirmwareSource,
}

impl OnboardFirmwareVersionCommand {
    /// GG's keyboard-firmware form (`90 00`).
    pub const KEYBOARD: Self = Self {
        source: FirmwareSource::Keyboard,
    };
    /// The LED-controller form (`90 01`).
    pub const LED: Self = Self {
        source: FirmwareSource::Led,
    };
}

impl HidCommand for OnboardFirmwareVersionCommand {
    fn command_code(&self) -> CommandCode {
        CommandCode::OnboardFirmwareVersion
    }

    fn serialize(&self, buffer: &mut [u8], _device_type: HidDeviceType) -> Result<usize> {
        check_report_buffer(buffer, ONBOARD_OUTPUT_REPORT_SIZE)?;
        let report = &mut buffer[..ONBOARD_OUTPUT_REPORT_SIZE];
        report.fill(0);
        report[1] = CommandCode::OnboardFirmwareVersion as u8;
        report[2] = self.source.into();
        Ok(ONBOARD_OUTPUT_REPORT_SIZE)
    }

    fn validate(&self) -> Result<()> {
        Ok(())
    }

    fn description(&self) -> String {
        format!(
            "Onboard firmware version poll: 90 {:02X} ({} firmware)",
            u8::from(self.source),
            self.source
        )
    }
}

/// Which driver mode [`OnboardDriverModeCommand`] asks the keyboard for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverModeState {
    /// `0x01` - engaged. GG's engine writes it as the fourth frame of its opening chain (`6c 00 01`,
    /// `DriverModeEnable` in its init-sequence descriptor), answered by nothing.
    Engaged,
    /// `0x00` - released. GG's engine writes it at session close (`6c 00 00`); engine-side only, not in the
    /// capture.
    Released,
}

impl From<DriverModeState> for u8 {
    fn from(mode: DriverModeState) -> Self {
        match mode {
            DriverModeState::Engaged => 0x01,
            DriverModeState::Released => 0x00,
        }
    }
}

/// [EXPERIMENTAL] Legacy Apex driver mode: `6c 00 <mode>` in a 64-byte output report
/// (`SET_REPORT wValue 0x0200`). GG's engine writes [`DriverModeState::Engaged`] as the fourth frame of its
/// opening chain - `DriverModeEnable` in its init-sequence descriptor, in the read capture and answered by
/// nothing - and [`DriverModeState::Released`] at session close (engine-side, not in the capture).
///
/// **Nothing in ssgg sends this command**, and the on-board profile read works without it. The Apex 3
/// family split: Apex 3 uses command `0x03` in a 33-byte report, Apex 3 TKL uses `0x6C` and opens with
/// driver mode engaged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnboardDriverModeCommand {
    /// Driver mode to set.
    pub mode: DriverModeState,
}

impl HidCommand for OnboardDriverModeCommand {
    fn command_code(&self) -> CommandCode {
        CommandCode::OnboardDriverMode
    }

    fn serialize(&self, buffer: &mut [u8], _device_type: HidDeviceType) -> Result<usize> {
        check_report_buffer(buffer, ONBOARD_OUTPUT_REPORT_SIZE)?;
        let report = &mut buffer[..ONBOARD_OUTPUT_REPORT_SIZE];
        report.fill(0);
        report[1] = CommandCode::OnboardDriverMode as u8;
        report[3] = self.mode.into();
        Ok(ONBOARD_OUTPUT_REPORT_SIZE)
    }

    fn validate(&self) -> Result<()> {
        Ok(())
    }

    fn description(&self) -> String {
        format!("Onboard driver mode: 6c 00 {:02X}", u8::from(self.mode))
    }
}

/// Profile slots the engine accepts: `(range 1 5)` on the wire, i.e. 0-based slots 0..=4 here.
pub const ONBOARD_PROFILE_SLOT_COUNT: u8 = 5;

/// Legacy Apex on-board profile deploy: `89 <1-based profile id>` in a 64-byte output
/// report - GG's `load_profile`. Seen in the capture behind profile-file transfers (rows 83,
/// 85, 111, 121: slots 0, 1, 2, 4 going out as 1, 2, 3, 5).
///
/// **This writes to the device**: it makes the keyboard run the slot it names. `ssgg onboard write` sends
/// it at the end of a write (after the 250 ms pause the descriptors' deploy API puts behind the frame) and
/// `ssgg onboard load` sends it on its own; a read sends nothing. Confirmed on hardware on an Apex 7
/// (`0x1612`): the keyboard runs the slot the frame names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnboardProfileDeployCommand {
    /// Profile slot, 0-based as everywhere in this crate; the wire id is `profile + 1`.
    pub profile: u8,
}

impl HidCommand for OnboardProfileDeployCommand {
    fn command_code(&self) -> CommandCode {
        CommandCode::OnboardProfileDeploy
    }

    fn serialize(&self, buffer: &mut [u8], _device_type: HidDeviceType) -> Result<usize> {
        self.validate()?;
        check_report_buffer(buffer, ONBOARD_OUTPUT_REPORT_SIZE)?;
        let report = &mut buffer[..ONBOARD_OUTPUT_REPORT_SIZE];
        report.fill(0);
        report[1] = CommandCode::OnboardProfileDeploy as u8;
        report[2] = self.profile.saturating_add(1);
        Ok(ONBOARD_OUTPUT_REPORT_SIZE)
    }

    fn validate(&self) -> Result<()> {
        if self.profile >= ONBOARD_PROFILE_SLOT_COUNT {
            return Err(Error::InvalidConfig(format!(
                "profile slot {} out of range (GG accepts 0..{})",
                self.profile,
                ONBOARD_PROFILE_SLOT_COUNT - 1
            )));
        }
        Ok(())
    }

    fn description(&self) -> String {
        format!(
            "Onboard profile deploy: 89 {:02X} (slot {})",
            self.profile.saturating_add(1),
            self.profile
        )
    }
}

/// Copy part of a profile file into the keyboard's resource buffer:
/// `85 | 00 | file id u16 LE | file offset u32 LE | size u16 LE | 00…` inside
/// [`ONBOARD_OUTPUT_REPORT_SIZE`]. **The pull has no reply with content**: GG follows it with one input
/// read and keeps none of the bytes (its read spec's byte specifier for the pull is zero length), and on the
/// wire that packet is the keyboard's ordinary input report - report `0x01`, all-zero on an idle keyboard, in
/// all 121 pulls of the capture. The pull is flow control; the profile bytes come from [`OnboardBufferReadCommand`].
/// Draining that packet is not optional on Linux - see `Apex7::read_pull_ack`.
///
/// Sizes and frame layout from the reference capture (121 pulls of 1024 bytes: file offsets 0, 4, 8, …
/// 44 for the schema probe, 0..11264 for full files). Confirmed on hardware on an Apex 7 (`0x1612`): the
/// pacing packet arrives as described, on a handle kept open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnboardPayloadPullCommand {
    /// Profile file id: the file base plus the profile index, and the same ids at +0x0100 for the
    /// lighting files.
    pub fs_id: u16,
    /// Offset in the profile file.
    pub file_offset: u32,
    /// Bytes to copy into the resource buffer.
    pub size: u16,
}

impl OnboardPayloadPullCommand {
    /// Significant frame bytes behind the report number.
    pub const FRAME_LEN: usize = 10;
    /// Largest pull the capture shows, the size of the keyboard's resource buffer.
    pub const MAX_SIZE: u16 = 1024;
}

impl HidCommand for OnboardPayloadPullCommand {
    fn command_code(&self) -> CommandCode {
        CommandCode::OnboardPayloadPull
    }

    fn serialize(&self, buffer: &mut [u8], _device_type: HidDeviceType) -> Result<usize> {
        self.validate()?;
        check_report_buffer(buffer, ONBOARD_OUTPUT_REPORT_SIZE)?;
        let report = &mut buffer[..ONBOARD_OUTPUT_REPORT_SIZE];
        report.fill(0);
        report[1] = CommandCode::OnboardPayloadPull as u8;
        report[3..5].copy_from_slice(&self.fs_id.to_le_bytes());
        report[5..9].copy_from_slice(&self.file_offset.to_le_bytes());
        report[9..Self::FRAME_LEN + 1].copy_from_slice(&self.size.to_le_bytes());
        Ok(ONBOARD_OUTPUT_REPORT_SIZE)
    }

    fn validate(&self) -> Result<()> {
        if self.size > Self::MAX_SIZE {
            return Err(Error::InvalidConfig(format!(
                "pull of {} bytes exceeds the {}-byte resource buffer",
                self.size,
                Self::MAX_SIZE
            )));
        }
        Ok(())
    }

    fn description(&self) -> String {
        format!(
            "On-board pull: file {:04X} offset {} size {}",
            self.fs_id, self.file_offset, self.size
        )
    }
}

/// Read part of the keyboard's resource buffer: `83 | 00 | buffer offset u16 LE |
/// size u16 LE | 00…` inside [`ONBOARD_FEATURE_REPORT_SIZE`], sent as a feature report on report number
/// 0, and answered by a `GET_REPORT wValue 0x0300 wIndex 1 wLength 642` read-back (see
/// [`ONBOARD_FEATURE_REPORT_SIZE`]).
///
/// The reference capture shows every read answered with the file head (schema word and
/// profile name) and zeros, never the requested window. Confirmed on hardware on an Apex 7 (`0x1612`): a
/// full profile slot is collected this way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnboardBufferReadCommand {
    /// Offset in the resource buffer.
    pub buffer_offset: u16,
    /// Bytes to return.
    pub size: u16,
}

impl OnboardBufferReadCommand {
    /// Significant frame bytes behind the report number.
    pub const FRAME_LEN: usize = 6;
}

impl HidCommand for OnboardBufferReadCommand {
    fn command_code(&self) -> CommandCode {
        CommandCode::OnboardBufferRead
    }

    fn serialize(&self, buffer: &mut [u8], _device_type: HidDeviceType) -> Result<usize> {
        self.validate()?;
        check_report_buffer(buffer, ONBOARD_FEATURE_REPORT_SIZE)?;
        let report = &mut buffer[..ONBOARD_FEATURE_REPORT_SIZE];
        report.fill(0);
        report[1] = CommandCode::OnboardBufferRead as u8;
        report[3..5].copy_from_slice(&self.buffer_offset.to_le_bytes());
        report[5..Self::FRAME_LEN + 1].copy_from_slice(&self.size.to_le_bytes());
        Ok(ONBOARD_FEATURE_REPORT_SIZE)
    }

    fn validate(&self) -> Result<()> {
        if usize::from(self.size) > ONBOARD_MAX_READ_LEN {
            return Err(Error::InvalidConfig(format!(
                "payload read of {} bytes exceeds the {} bytes a {}-byte reply report carries behind its \
                 5-byte header",
                self.size, ONBOARD_MAX_READ_LEN, ONBOARD_READ_WIRE_BYTES
            )));
        }
        Ok(())
    }

    fn description(&self) -> String {
        format!(
            "On-board payload read: buffer offset {} size {}",
            self.buffer_offset, self.size
        )
    }
}

/// Legacy Apex on-board file open: `88 | 00 | file id u16 LE | 00…` inside
/// [`ONBOARD_OUTPUT_REPORT_SIZE`], answered by the interrupt report `88 01 …`.
///
/// The engine's write API opens the profile file it is about to push (an output report
/// followed by one input read), and the Apex 7 write capture shows it as
/// `wValue 0x0200 wIndex 1 wLength 64` answered 305 ms later with `88 01 00…`. The file ids are the same
/// as on the read side: `PROFILE_FS_BASE + slot` for the profile file. Confirmed on hardware on an Apex 7
/// (`0x1612`): the board answers this frame with `88 01 …`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnboardFileOpenCommand {
    /// Profile file id.
    pub fs_id: u16,
}

impl OnboardFileOpenCommand {
    /// Significant frame bytes behind the report number.
    pub const FRAME_LEN: usize = 4;
}

impl HidCommand for OnboardFileOpenCommand {
    fn command_code(&self) -> CommandCode {
        CommandCode::OnboardFileOpen
    }

    fn serialize(&self, buffer: &mut [u8], _device_type: HidDeviceType) -> Result<usize> {
        self.validate()?;
        check_report_buffer(buffer, ONBOARD_OUTPUT_REPORT_SIZE)?;
        let report = &mut buffer[..ONBOARD_OUTPUT_REPORT_SIZE];
        report.fill(0);
        report[1] = CommandCode::OnboardFileOpen as u8;
        report[3..Self::FRAME_LEN + 1].copy_from_slice(&self.fs_id.to_le_bytes());
        Ok(ONBOARD_OUTPUT_REPORT_SIZE)
    }

    fn validate(&self) -> Result<()> {
        if self.fs_id == 0 {
            return Err(Error::InvalidConfig(
                "the on-board file id is zero: the write API opens a profile file id, never 0".to_string(),
            ));
        }
        Ok(())
    }

    fn description(&self) -> String {
        format!("On-board file open: 88 00 {:04X}", self.fs_id)
    }
}

/// Copy payload bytes into the keyboard's resource buffer:
/// `03 | 00 | buffer offset u16 LE | size u16 LE | payload bytes` inside
/// [`ONBOARD_FEATURE_REPORT_SIZE`] (the 642-wire-byte feature report the write capture shows).
///
/// The engine's write API fills one 1024-byte window of the buffer with two of these,
/// 512 bytes each, and the Apex 7 write capture shows exactly that: every window is a write at
/// buffer offset 0 and one at 512, with no reply between them. The frame has no reply of its own; the
/// [`OnboardPayloadPushCommand`] that follows it is answered. Confirmed on hardware on an Apex 7 (`0x1612`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnboardBufferWriteCommand<'a> {
    /// Offset in the resource buffer.
    pub buffer_offset: u16,
    /// Bytes to place there.
    pub data: &'a [u8],
}

impl OnboardBufferWriteCommand<'_> {
    /// Significant frame bytes behind the report number, before the payload.
    pub const FRAME_LEN: usize = 6;
    /// Payload bytes a write frame can carry: the feature report behind the report number and the header.
    pub const MAX_DATA_LEN: usize = ONBOARD_FEATURE_REPORT_SIZE - 1 - Self::FRAME_LEN;
}

impl HidCommand for OnboardBufferWriteCommand<'_> {
    fn command_code(&self) -> CommandCode {
        CommandCode::ProfileChunk
    }

    fn serialize(&self, buffer: &mut [u8], _device_type: HidDeviceType) -> Result<usize> {
        self.validate()?;
        check_report_buffer(buffer, ONBOARD_FEATURE_REPORT_SIZE)?;
        let report = &mut buffer[..ONBOARD_FEATURE_REPORT_SIZE];
        report.fill(0);
        report[1] = CommandCode::ProfileChunk as u8;
        let size = u16::try_from(self.data.len()).map_err(|_| {
            Error::InvalidConfig(format!(
                "a resource-buffer write of {} bytes does not fit the frame's u16 size field",
                self.data.len()
            ))
        })?;
        report[3..5].copy_from_slice(&self.buffer_offset.to_le_bytes());
        report[5..Self::FRAME_LEN + 1].copy_from_slice(&size.to_le_bytes());
        report[Self::FRAME_LEN + 1..Self::FRAME_LEN + 1 + self.data.len()].copy_from_slice(self.data);
        Ok(ONBOARD_FEATURE_REPORT_SIZE)
    }

    fn validate(&self) -> Result<()> {
        if self.data.is_empty() {
            return Err(Error::InvalidConfig(
                "a resource-buffer write carries at least one byte".to_string(),
            ));
        }
        if self.data.len() > Self::MAX_DATA_LEN {
            return Err(Error::InvalidConfig(format!(
                "resource-buffer write of {} bytes exceeds the {} bytes a {}-byte feature report carries \
                 behind its header",
                self.data.len(),
                Self::MAX_DATA_LEN,
                ONBOARD_FEATURE_REPORT_SIZE
            )));
        }
        Ok(())
    }

    fn description(&self) -> String {
        format!(
            "On-board buffer write: {} bytes at buffer offset {}",
            self.data.len(),
            self.buffer_offset
        )
    }
}

/// Copy the resource buffer into a profile file: `05 | 00 | file id u16 LE |
/// file offset u32 LE | size u16 LE | 00…` inside [`ONBOARD_OUTPUT_REPORT_SIZE`].
///
/// The write side of [`OnboardPayloadPullCommand`]'s file access: same header shape, opposite direction. The
/// engine's write API pushes one full resource buffer per window, and the Apex 7 write capture
/// answers every push with the interrupt report `05 01 00…` - the only write-side reply with content in
/// that capture. Confirmed on hardware on an Apex 7 (`0x1612`): every push of a write is answered that way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnboardPayloadPushCommand {
    /// Profile file id: the file base plus the profile index.
    pub fs_id: u16,
    /// Offset in the profile file.
    pub file_offset: u32,
    /// Bytes to copy out of the resource buffer.
    pub size: u16,
}

impl OnboardPayloadPushCommand {
    /// Significant frame bytes behind the report number.
    pub const FRAME_LEN: usize = 10;
    /// The push size the capture shows, the size of the keyboard's resource buffer.
    pub const MAX_SIZE: u16 = 1024;
}

impl HidCommand for OnboardPayloadPushCommand {
    fn command_code(&self) -> CommandCode {
        CommandCode::OnboardPayloadPush
    }

    fn serialize(&self, buffer: &mut [u8], _device_type: HidDeviceType) -> Result<usize> {
        self.validate()?;
        check_report_buffer(buffer, ONBOARD_OUTPUT_REPORT_SIZE)?;
        let report = &mut buffer[..ONBOARD_OUTPUT_REPORT_SIZE];
        report.fill(0);
        report[1] = CommandCode::OnboardPayloadPush as u8;
        report[3..5].copy_from_slice(&self.fs_id.to_le_bytes());
        report[5..9].copy_from_slice(&self.file_offset.to_le_bytes());
        report[9..Self::FRAME_LEN + 1].copy_from_slice(&self.size.to_le_bytes());
        Ok(ONBOARD_OUTPUT_REPORT_SIZE)
    }

    fn validate(&self) -> Result<()> {
        if self.fs_id == 0 {
            return Err(Error::InvalidConfig(
                "the on-board file id is zero: a push copies into a profile file id, never 0".to_string(),
            ));
        }
        if self.size > Self::MAX_SIZE {
            return Err(Error::InvalidConfig(format!(
                "push of {} bytes exceeds the {}-byte resource buffer",
                self.size,
                Self::MAX_SIZE
            )));
        }
        Ok(())
    }

    fn description(&self) -> String {
        format!(
            "On-board push: file {:04X} offset {} size {}",
            self.fs_id, self.file_offset, self.size
        )
    }
}

/// Legacy Apex profile commit after the last push: `eb | 1-based profile id | 00…` inside
/// [`ONBOARD_OUTPUT_REPORT_SIZE`] - an **output** report, the shape the write capture shows
/// (`wValue 0x0200 wLength 64`). The engine's write API reads one input report after the frame, and that
/// report is the keyboard's ordinary idle report `01 00…`, not a commit answer.
///
/// The capture answers it with no write-specific reply. Confirmed on hardware on an Apex 7 (`0x1612`): the
/// board accepts the frame with no write-specific answer, as in the capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnboardFileCommitCommand {
    /// Profile slot, 0-based as everywhere in this crate; the wire byte is `profile + 1`.
    pub profile: u8,
}

impl OnboardFileCommitCommand {
    /// Significant frame bytes behind the report number.
    pub const FRAME_LEN: usize = 2;
}

impl HidCommand for OnboardFileCommitCommand {
    fn command_code(&self) -> CommandCode {
        CommandCode::OnboardFileCommit
    }

    fn serialize(&self, buffer: &mut [u8], _device_type: HidDeviceType) -> Result<usize> {
        self.validate()?;
        check_report_buffer(buffer, ONBOARD_OUTPUT_REPORT_SIZE)?;
        let report = &mut buffer[..ONBOARD_OUTPUT_REPORT_SIZE];
        report.fill(0);
        report[1] = CommandCode::OnboardFileCommit as u8;
        report[2] = self.profile.saturating_add(1);
        Ok(ONBOARD_OUTPUT_REPORT_SIZE)
    }

    fn validate(&self) -> Result<()> {
        if self.profile >= ONBOARD_PROFILE_SLOT_COUNT {
            return Err(Error::InvalidConfig(format!(
                "profile slot {} out of range (GG accepts 0..{})",
                self.profile,
                ONBOARD_PROFILE_SLOT_COUNT - 1
            )));
        }
        Ok(())
    }

    fn description(&self) -> String {
        format!(
            "On-board commit: eb {:02X} (slot {})",
            self.profile.saturating_add(1),
            self.profile
        )
    }
}

/// [EXPERIMENTAL] One-byte setting of the Arctis Nova 7 family: `00 <opcode> <value>` zero padded to
/// [`HEADSET_REPORT_SIZE`], byte 0 being the hidapi report number. `code` selects the setting and fixes the
/// accepted range of `value`: sidetone `0..=3`, microphone volume `0..=7`, auto-off any minute count.
/// Source: community forks, never sent from `ssgg` on hardware. Never followed by the save frame `00 09`,
/// which would persist the setting on the headset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeadsetSettingCommand {
    /// [`CommandCode::HeadsetSidetone`], [`CommandCode::HeadsetMicVolume`] or [`CommandCode::HeadsetAutoOff`].
    pub code: CommandCode,
    /// Wire value in the range `code` allows.
    pub value: u8,
}

impl HeadsetSettingCommand {
    /// Highest wire sidetone level.
    pub const MAX_SIDETONE: u8 = 3;
    /// Highest wire microphone volume.
    pub const MAX_MIC_VOLUME: u8 = 7;
}

impl HidCommand for HeadsetSettingCommand {
    fn command_code(&self) -> CommandCode {
        self.code
    }

    fn serialize(&self, buffer: &mut [u8], _device_type: HidDeviceType) -> Result<usize> {
        self.validate()?;
        check_report_buffer(buffer, HEADSET_REPORT_SIZE)?;
        let report = &mut buffer[..HEADSET_REPORT_SIZE];
        report.fill(0);
        report[1] = self.code as u8;
        report[2] = self.value;
        Ok(HEADSET_REPORT_SIZE)
    }

    fn validate(&self) -> Result<()> {
        let max = match self.code {
            CommandCode::HeadsetSidetone => Self::MAX_SIDETONE,
            CommandCode::HeadsetMicVolume => Self::MAX_MIC_VOLUME,
            CommandCode::HeadsetAutoOff => u8::MAX,
            other => {
                return Err(Error::InvalidConfig(format!(
                    "{other} is not a headset one-byte setting"
                )));
            }
        };
        if self.value > max {
            return Err(Error::InvalidConfig(format!(
                "{} value {} out of range (0..={})",
                self.code, self.value, max
            )));
        }
        Ok(())
    }

    fn description(&self) -> String {
        format!("Headset setting {}: {}", self.code, self.value)
    }
}

/// [EXPERIMENTAL] Arctis Nova 7 family status query: `00 b0` zero padded to [`HEADSET_REPORT_SIZE`]. The
/// headset answers with an input report `b0 <power> <battery> <charge> <chat game> <chat chat> ...`.
/// Source: community forks, never sent from `ssgg` on hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeadsetStatusQueryCommand;

impl HidCommand for HeadsetStatusQueryCommand {
    fn command_code(&self) -> CommandCode {
        CommandCode::HeadsetStatus
    }

    fn serialize(&self, buffer: &mut [u8], _device_type: HidDeviceType) -> Result<usize> {
        check_report_buffer(buffer, HEADSET_REPORT_SIZE)?;
        let report = &mut buffer[..HEADSET_REPORT_SIZE];
        report.fill(0);
        report[1] = CommandCode::HeadsetStatus as u8;
        Ok(HEADSET_REPORT_SIZE)
    }

    fn validate(&self) -> Result<()> {
        Ok(())
    }

    fn description(&self) -> String {
        "Headset status query".to_string()
    }
}

/// First byte of a Modern-family mouse command: `base`, with [`MOUSE_DONGLE_FLAG`] set for a 2.4 GHz receiver PID.
const fn mouse_opcode(base: u8, dongle: bool) -> u8 {
    if dongle { base | MOUSE_DONGLE_FLAG } else { base }
}

/// [EXPERIMENTAL] Modern-family mouse polling rate: `00 <0x2B> <code>`, 0x6B on a 2.4 GHz receiver PID. `code`
/// is 4 to 1 for 125 to 1000 Hz on most models and 3 to 0 on the wireless ones, so the model table picks it.
/// Source: community rivalcfg-derived fork, never sent from `ssgg` on hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MousePollRateCommand {
    pub dongle: bool,
    pub code: u8,
}

impl MousePollRateCommand {
    /// Highest polling-rate code any Modern-family model uses.
    pub const MAX_CODE: u8 = 4;
    const LEN: usize = 3;
}

impl HidCommand for MousePollRateCommand {
    fn command_code(&self) -> CommandCode {
        CommandCode::MousePollRate
    }

    fn serialize(&self, buffer: &mut [u8], _device_type: HidDeviceType) -> Result<usize> {
        self.validate()?;
        check_report_buffer(buffer, Self::LEN)?;
        buffer[..Self::LEN].copy_from_slice(&[
            0x00,
            mouse_opcode(CommandCode::MousePollRate as u8, self.dongle),
            self.code,
        ]);
        Ok(Self::LEN)
    }

    fn validate(&self) -> Result<()> {
        if self.code > Self::MAX_CODE {
            return Err(Error::InvalidConfig(format!(
                "mouse polling-rate code {} out of range (0..={})",
                self.code,
                Self::MAX_CODE
            )));
        }
        Ok(())
    }

    fn description(&self) -> String {
        format!("Mouse polling rate code {}", self.code)
    }
}

/// [EXPERIMENTAL] Modern-family mouse battery query: `00 <0x92>`, 0xD2 on a 2.4 GHz receiver PID. The reply is
/// decoded by `mice::modern::decode_battery`. Source: community rivalcfg-derived fork, never sent from `ssgg`
/// on hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseBatteryQueryCommand {
    pub dongle: bool,
}

impl MouseBatteryQueryCommand {
    const LEN: usize = 2;
}

impl HidCommand for MouseBatteryQueryCommand {
    fn command_code(&self) -> CommandCode {
        CommandCode::MouseBattery
    }

    fn serialize(&self, buffer: &mut [u8], _device_type: HidDeviceType) -> Result<usize> {
        check_report_buffer(buffer, Self::LEN)?;
        buffer[..Self::LEN].copy_from_slice(&[0x00, mouse_opcode(CommandCode::MouseBattery as u8, self.dongle)]);
        Ok(Self::LEN)
    }

    fn validate(&self) -> Result<()> {
        Ok(())
    }

    fn description(&self) -> String {
        "Mouse battery query".to_string()
    }
}

/// [EXPERIMENTAL] Modern-family mouse DPI, one stage: `00 <opcode> 01 <selected> <code>`. `code` is one byte, or
/// two little-endian bytes when `wide`, and is repeated for the Y axis when `per_axis`. The opcode is 0x2D, or
/// 0x34 on the Rival 3 Gen 2; a 2.4 GHz receiver PID sets [`MOUSE_DONGLE_FLAG`] on it. Source: community
/// rivalcfg-derived fork, never sent from `ssgg` on hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseDpiCommand {
    pub opcode: u8,
    pub dongle: bool,
    /// Stage the mouse switches to; the model's first preset index.
    pub selected: u8,
    pub code: u16,
    pub wide: bool,
    pub per_axis: bool,
}

impl MouseDpiCommand {
    /// DPI opcode of every Modern-family model but the Rival 3 Gen 2.
    pub const OPCODE: u8 = 0x2D;
    /// DPI opcode of the Rival 3 Gen 2.
    pub const OPCODE_RIVAL_3_GEN_2: u8 = 0x34;
}

impl HidCommand for MouseDpiCommand {
    fn command_code(&self) -> CommandCode {
        CommandCode::MouseDpi
    }

    fn serialize(&self, buffer: &mut [u8], _device_type: HidDeviceType) -> Result<usize> {
        self.validate()?;
        let code = self.code.to_le_bytes();
        let code = if self.wide { &code[..] } else { &code[..1] };
        let axes = if self.per_axis { 2 } else { 1 };
        let len = 4 + code.len() * axes;
        check_report_buffer(buffer, len)?;
        buffer[..4].copy_from_slice(&[0x00, mouse_opcode(self.opcode, self.dongle), 0x01, self.selected]);
        for axis in 0..axes {
            let start = 4 + axis * code.len();
            buffer[start..start + code.len()].copy_from_slice(code);
        }
        Ok(len)
    }

    fn validate(&self) -> Result<()> {
        if !matches!(self.opcode, Self::OPCODE | Self::OPCODE_RIVAL_3_GEN_2) {
            return Err(Error::InvalidConfig(format!(
                "{:#04x} is not a mouse DPI opcode",
                self.opcode
            )));
        }
        if !self.wide && self.code > u16::from(u8::MAX) {
            return Err(Error::InvalidConfig(format!(
                "DPI code {} does not fit in one byte",
                self.code
            )));
        }
        Ok(())
    }

    fn description(&self) -> String {
        format!("Mouse DPI code {}", self.code)
    }
}

/// Where a Modern-family mouse takes the colour of one LED zone. Every layout is `<0x21> <selector> <RGB>`
/// (0x61 on a 2.4 GHz receiver PID); they differ in the selector and in how far into the frame the RGB triple
/// sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseColorLayout {
    /// Aerox 3, Aerox 5 and Rival 3 Gen 2: one mask byte with bit `zone` set, then three zero bytes per
    /// earlier zone.
    Mask8,
    /// Rival 5: a 16-bit little-endian mask with bit `zone` set, then three zero bytes per earlier zone.
    Mask16,
    /// The wireless Aerox, Prime Wireless and Prime Mini Wireless: `01 <zone>`.
    Indexed,
    /// Prime Mini, one LED: `00`.
    Single,
}

/// [EXPERIMENTAL] Modern-family mouse LED colour for one zone, in the [`MouseColorLayout`] of the model. A
/// 2.4 GHz receiver PID sets [`MOUSE_DONGLE_FLAG`] on the opcode. Source: community rivalcfg-derived fork,
/// never sent from `ssgg` on hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseColorCommand {
    pub layout: MouseColorLayout,
    pub dongle: bool,
    pub zone: u8,
    pub color: Color,
}

impl MouseColorCommand {
    /// Opcode of every colour frame; also [`CommandCode::RgbControl`]'s byte.
    pub const OPCODE: u8 = 0x21;

    fn max_zone(&self) -> u8 {
        match self.layout {
            MouseColorLayout::Mask8 => 7,
            MouseColorLayout::Mask16 => 15,
            MouseColorLayout::Indexed => u8::MAX,
            MouseColorLayout::Single => 0,
        }
    }
}

impl HidCommand for MouseColorCommand {
    fn command_code(&self) -> CommandCode {
        CommandCode::MouseColor
    }

    fn serialize(&self, buffer: &mut [u8], _device_type: HidDeviceType) -> Result<usize> {
        self.validate()?;
        let zone = usize::from(self.zone);
        let mut report = vec![0x00, mouse_opcode(Self::OPCODE, self.dongle)];
        match self.layout {
            MouseColorLayout::Mask8 => report.push(1u8 << self.zone),
            MouseColorLayout::Mask16 => report.extend((1u16 << self.zone).to_le_bytes()),
            MouseColorLayout::Indexed => report.extend([0x01, self.zone]),
            MouseColorLayout::Single => report.push(0x00),
        }
        if matches!(self.layout, MouseColorLayout::Mask8 | MouseColorLayout::Mask16) {
            report.resize(report.len() + 3 * zone, 0x00);
        }
        report.extend([self.color.r, self.color.g, self.color.b]);
        check_report_buffer(buffer, report.len())?;
        buffer[..report.len()].copy_from_slice(&report);
        Ok(report.len())
    }

    fn validate(&self) -> Result<()> {
        if self.zone > self.max_zone() {
            return Err(Error::InvalidConfig(format!(
                "LED zone {} out of range for layout {:?} (0..={})",
                self.zone,
                self.layout,
                self.max_zone()
            )));
        }
        Ok(())
    }

    fn description(&self) -> String {
        format!("Mouse LED zone {} colour {:?}", self.zone, self.color)
    }
}

/// Reject a buffer that cannot hold a report of `report_size` bytes.
fn check_report_buffer(buffer: &[u8], report_size: usize) -> Result<()> {
    if buffer.len() < report_size {
        return Err(Error::DeviceCommunication(format!(
            "Buffer too small: {} bytes (expected {})",
            buffer.len(),
            report_size
        )));
    }
    Ok(())
}

/// Apply/Save command to commit changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyCommand;

impl HidCommand for ApplyCommand {
    fn command_code(&self) -> CommandCode {
        CommandCode::Apply
    }

    fn serialize(&self, buffer: &mut [u8], device_type: HidDeviceType) -> Result<usize> {
        let report_size = device_type.report_size();
        if buffer.len() < report_size {
            return Err(Error::DeviceCommunication(format!(
                "Buffer too small: {} bytes (expected {})",
                buffer.len(),
                report_size
            )));
        }

        buffer[..report_size].fill(0);
        let mut offset = 0;

        // Add report ID for keyboards
        if device_type.includes_report_id() {
            buffer[0] = 0x00; // Report ID
            offset = 1;
        }

        // Command code only
        buffer[offset] = self.command_code() as u8;

        Ok(report_size)
    }

    fn validate(&self) -> Result<()> {
        Ok(()) // Always valid
    }

    fn description(&self) -> String {
        "Apply/save current settings".to_string()
    }
}

/// Actuation point control command for adjustable actuation keyboards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActuationCommand {
    /// Actuation point in 0.1mm increments (e.g. 4 = 0.4mm, 36 = 3.6mm)
    pub actuation_point: u8,
}

impl ActuationCommand {
    /// Create a new actuation command with the specified actuation point.
    /// Value is in 0.1mm increments (e.g. 4 = 0.4mm, 36 = 3.6mm).
    pub fn new(actuation_point: u8) -> Self {
        Self { actuation_point }
    }

    /// Create a new actuation command from millimeters.
    /// Precision is limited to 0.1mm increments.
    pub fn from_mm(mm: f32) -> Self {
        let tenths_of_mm = (mm * 10.0).round() as u8;
        Self {
            actuation_point: tenths_of_mm.min(40), // Clamp to max 4.0mm
        }
    }

    /// Convert to millimeters
    pub fn to_mm(&self) -> f32 {
        self.actuation_point as f32 / 10.0
    }
}

impl HidCommand for ActuationCommand {
    fn command_code(&self) -> CommandCode {
        CommandCode::ActuationControl
    }

    fn serialize(&self, buffer: &mut [u8], device_type: HidDeviceType) -> Result<usize> {
        self.validate()?;

        let report_size = device_type.report_size();
        if buffer.len() < report_size {
            return Err(Error::DeviceCommunication(format!(
                "Buffer too small: {} bytes (expected {})",
                buffer.len(),
                report_size
            )));
        }

        buffer[..report_size].fill(0);
        let mut offset = 0;

        // Add report ID for keyboards
        if device_type.includes_report_id() {
            buffer[0] = 0x00; // Report ID
            offset = 1;
        }

        // Command code
        buffer[offset] = self.command_code() as u8;
        offset += 1;

        // Actuation point value
        buffer[offset] = self.actuation_point;

        Ok(report_size)
    }

    fn validate(&self) -> Result<()> {
        // Validate range (0.1mm - 4.0mm in 0.1mm increments)
        if self.actuation_point == 0 || self.actuation_point > 40 {
            return Err(Error::DeviceCommunication(format!(
                "Actuation point must be between 1 (0.1mm) and 40 (4.0mm), got {}",
                self.actuation_point
            )));
        }
        Ok(())
    }

    fn description(&self) -> String {
        format!("Set actuation point to {:.1}mm", self.to_mm())
    }
}

/// Per-key RGB command for individual key targeting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerKeyRgbCommand {
    /// Key-to-color mappings
    pub key_colors: HashMap<KeyAddress, Color>,
    /// Addressing mode
    pub addressing_mode: PerKeyAddressingMode,
}

/// Addressing mode for per-key RGB commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerKeyAddressingMode {
    /// HID code addressing: [hid_code] [R] [G] [B] (4 bytes per key)
    /// Uses standard USB HID Usage IDs for key identification.
    /// This is the primary addressing mode for SteelSeries keyboards.
    HidCode,
    /// Legacy matrix row/column addressing (deprecated).
    /// Use HidCode instead - SteelSeries uses HID codes, not matrix coordinates.
    #[deprecated(note = "Use PerKeyAddressingMode::HidCode instead")]
    Matrix,
    /// Legacy logical key ID addressing (deprecated).
    /// Use HidCode instead - SteelSeries uses HID codes.
    #[deprecated(note = "Use PerKeyAddressingMode::HidCode instead")]
    Logical,
}

impl PerKeyRgbCommand {
    /// Maximum keys per report to fit within 64-byte payload limit
    /// Header: Report ID (1) + Cmd (1) + Count (1) = 3 bytes
    /// Key Data: HID code (1) + R (1) + G (1) + B (1) + zone (1) = 5 bytes
    /// Max Keys: (65 - 3) / 5 = 12
    pub const MAX_KEYS_PER_REPORT: usize = 12;

    /// Maximum matrix dimensions
    pub const MAX_ROWS: u8 = 32;
    pub const MAX_COLS: u8 = 32;

    /// Create a new per-key RGB command with HID code addressing.
    pub fn new(_addressing_mode: PerKeyAddressingMode) -> Self {
        Self {
            key_colors: HashMap::new(),
            addressing_mode: PerKeyAddressingMode::HidCode,
        }
    }

    /// Create a new per-key RGB command with batch ID for complex operations.
    /// Note: Batch ID is no longer used in serialization but kept for API compatibility.
    pub fn new_with_batch(_addressing_mode: PerKeyAddressingMode, _batch_id: u32) -> Self {
        Self::new(PerKeyAddressingMode::HidCode)
    }

    /// Check if this command needs fragmentation due to size limits.
    pub fn needs_fragmentation(&self) -> bool {
        self.key_colors.len() > Self::MAX_KEYS_PER_REPORT
    }

    /// Split this command into fragments that fit within report size limits.
    pub fn fragment_into_reports(&self) -> Vec<PerKeyRgbCommand> {
        let mut fragments = Vec::new();

        // Handle empty case
        if self.key_colors.is_empty() {
            return Vec::new();
        }

        let mut current_fragment = PerKeyRgbCommand {
            key_colors: HashMap::with_capacity(Self::MAX_KEYS_PER_REPORT),
            addressing_mode: self.addressing_mode,
        };

        for (address, color) in &self.key_colors {
            current_fragment.key_colors.insert(*address, *color);

            if current_fragment.key_colors.len() == Self::MAX_KEYS_PER_REPORT {
                fragments.push(current_fragment);
                current_fragment = PerKeyRgbCommand {
                    key_colors: HashMap::with_capacity(Self::MAX_KEYS_PER_REPORT),
                    addressing_mode: self.addressing_mode,
                };
            }
        }

        if !current_fragment.key_colors.is_empty() {
            fragments.push(current_fragment);
        }

        fragments
    }

    /// Create a command for a single key.
    pub fn single_key(address: KeyAddress, color: Color) -> Self {
        let mut command = Self::new(PerKeyAddressingMode::HidCode);
        command.set_key_color(address, color);
        command
    }

    /// Create a command from logical key IDs using a key mapping.
    pub fn from_logical_keys(key_colors: HashMap<KeyId, Color>, key_mapping: &KeyMapping) -> Result<Self> {
        let mut command = Self::new(PerKeyAddressingMode::HidCode);

        for (key_id, color) in key_colors {
            if let Some(address) = key_mapping.get_key_address(key_id) {
                command.set_key_color(address, color);
            } else {
                return Err(Error::DeviceCommunication(format!(
                    "Key {:?} not found in mapping",
                    key_id
                )));
            }
        }

        Ok(command)
    }

    /// Set color for a specific key address.
    pub fn set_key_color(&mut self, address: KeyAddress, color: Color) {
        self.key_colors.insert(address, color);
    }

    /// Remove a key from the command.
    pub fn remove_key(&mut self, address: KeyAddress) {
        self.key_colors.remove(&address);
    }

    /// Get all key addresses in this command.
    pub fn get_addresses(&self) -> Vec<KeyAddress> {
        self.key_colors.keys().copied().collect()
    }

    /// Get color for a specific address.
    pub fn get_key_color(&self, address: KeyAddress) -> Option<Color> {
        self.key_colors.get(&address).copied()
    }

    /// Check if this command is empty.
    pub fn is_empty(&self) -> bool {
        self.key_colors.is_empty()
    }

    /// Get the number of keys in this command.
    pub fn key_count(&self) -> usize {
        self.key_colors.len()
    }
}

impl HidCommand for PerKeyRgbCommand {
    fn command_code(&self) -> CommandCode {
        CommandCode::PerKeyRgb
    }

    fn serialize(&self, buffer: &mut [u8], device_type: HidDeviceType) -> Result<usize> {
        self.validate()?;

        let report_size = device_type.report_size();
        if buffer.len() < report_size {
            return Err(Error::DeviceCommunication(format!(
                "Buffer too small: {} bytes (expected {})",
                buffer.len(),
                report_size
            )));
        }

        buffer[..report_size].fill(0);
        let mut offset = 0;

        // Add report ID for keyboards
        if device_type.includes_report_id() {
            buffer[0] = 0x00; // Report ID
            offset = 1;
        }

        // Command code
        buffer[offset] = self.command_code() as u8;
        offset += 1;

        // Key count
        buffer[offset] = self.key_colors.len().min(255) as u8;
        offset += 1;

        match self.addressing_mode {
            PerKeyAddressingMode::HidCode => {
                // HID code addressing: [hid_code] [R] [G] [B]
                for (address, color) in &self.key_colors {
                    if offset + 4 > report_size {
                        break;
                    }

                    buffer[offset] = address.hid_code;
                    buffer[offset + 1] = color.r;
                    buffer[offset + 2] = color.g;
                    buffer[offset + 3] = color.b;
                    offset += 4;
                }
            }
            #[allow(deprecated)]
            PerKeyAddressingMode::Matrix | PerKeyAddressingMode::Logical => {
                // Deprecated modes - treat as HidCode
                for (address, color) in &self.key_colors {
                    if offset + 4 > report_size {
                        break;
                    }

                    buffer[offset] = address.hid_code;
                    buffer[offset + 1] = color.r;
                    buffer[offset + 2] = color.g;
                    buffer[offset + 3] = color.b;
                    offset += 4;
                }
            }
        }

        Ok(report_size)
    }

    fn validate(&self) -> Result<()> {
        if self.key_colors.is_empty() && !self.needs_fragmentation() {
            return Err(Error::DeviceCommunication(
                "Per-key RGB command must have at least one key".to_string(),
            ));
        }

        // Validate HID code bounds (valid range: 0x04-0xF0)
        for address in self.key_colors.keys() {
            if address.hid_code < 0x04 || address.hid_code > 0xF0 {
                return Err(Error::DeviceCommunication(format!(
                    "HID code 0x{:02X} is outside valid range (0x04-0xF0)",
                    address.hid_code
                )));
            }
        }

        // 1 (ID) + 1 (Cmd) + 1 (Count) + N*5
        let required_bytes = 1 + 1 + 1 + (self.key_colors.len() * 5);

        if required_bytes > 65 {
            return Err(Error::DeviceCommunication(format!(
                "Too many keys in command: {} keys require {} bytes (max 65)",
                self.key_colors.len(),
                required_bytes
            )));
        }

        Ok(())
    }

    fn description(&self) -> String {
        format!("Set {} keys via HID code addressing", self.key_colors.len())
    }
}

/// Builder for creating batch per-key RGB operations.
#[derive(Debug)]
pub struct PerKeyRgbBuilder {
    addressing_mode: PerKeyAddressingMode,
    key_colors: HashMap<KeyAddress, Color>,
    key_mapping: Option<KeyMapping>,
}

impl PerKeyRgbBuilder {
    /// Create a new per-key RGB builder.
    pub fn new(addressing_mode: PerKeyAddressingMode) -> Self {
        Self {
            addressing_mode,
            key_colors: HashMap::new(),
            key_mapping: None,
        }
    }

    /// Create a new per-key RGB builder with batch ID.
    pub fn new_with_batch(addressing_mode: PerKeyAddressingMode, _batch_id: u32) -> Self {
        Self::new(addressing_mode)
    }

    /// Create a builder with logical addressing support.
    pub fn with_key_mapping(key_mapping: KeyMapping) -> Self {
        Self {
            addressing_mode: PerKeyAddressingMode::HidCode,
            key_colors: HashMap::new(),
            key_mapping: Some(key_mapping),
        }
    }

    /// Create a builder with logical addressing and batch ID.
    pub fn with_key_mapping_and_batch(key_mapping: KeyMapping, _batch_id: u32) -> Self {
        Self::with_key_mapping(key_mapping)
    }

    /// Add a key by matrix address.
    pub fn add_key_matrix(&mut self, address: KeyAddress, color: Color) -> &mut Self {
        self.key_colors.insert(address, color);
        self
    }

    /// Add a key by logical ID (requires key mapping).
    pub fn add_key_logical(&mut self, key_id: KeyId, color: Color) -> Result<&mut Self> {
        if let Some(ref mapping) = self.key_mapping {
            if let Some(address) = mapping.get_key_address(key_id) {
                self.key_colors.insert(address, color);
                Ok(self)
            } else {
                Err(Error::DeviceCommunication(format!(
                    "Key {:?} not found in mapping",
                    key_id
                )))
            }
        } else {
            Err(Error::DeviceCommunication(
                "No key mapping available for logical addressing".to_string(),
            ))
        }
    }

    /// Add multiple keys with the same color.
    pub fn add_keys_batch(&mut self, addresses: &[KeyAddress], color: Color) -> &mut Self {
        for &address in addresses {
            self.key_colors.insert(address, color);
        }
        self
    }

    /// Add keys from a logical key list (requires key mapping).
    pub fn add_logical_keys(&mut self, key_ids: &[KeyId], color: Color) -> Result<&mut Self> {
        if let Some(ref mapping) = self.key_mapping {
            for &key_id in key_ids {
                if let Some(address) = mapping.get_key_address(key_id) {
                    self.key_colors.insert(address, color);
                } else {
                    return Err(Error::DeviceCommunication(format!(
                        "Key {:?} not found in mapping",
                        key_id
                    )));
                }
            }
            Ok(self)
        } else {
            Err(Error::DeviceCommunication(
                "No key mapping available for logical addressing".to_string(),
            ))
        }
    }

    /// Clear all keys.
    pub fn clear(&mut self) -> &mut Self {
        self.key_colors.clear();
        self
    }

    /// Set a region of keys to the same color.
    pub fn set_region(&mut self, start_hid: u8, count: u8, color: Color) -> &mut Self {
        for hid_code in start_hid..(start_hid + count) {
            self.key_colors.insert(KeyAddress::new(hid_code), color);
        }
        self
    }

    /// Build the final per-key RGB command.
    pub fn build(self) -> PerKeyRgbCommand {
        let mut command = PerKeyRgbCommand::new(self.addressing_mode);
        command.key_colors = self.key_colors;
        command
    }

    /// Get the current number of keys in the builder.
    pub fn key_count(&self) -> usize {
        self.key_colors.len()
    }

    /// Check if the builder is empty.
    pub fn is_empty(&self) -> bool {
        self.key_colors.is_empty()
    }
}
/// Structured HID report builder and validator.
#[derive(Debug)]
pub struct HidReportBuilder {
    device_type: HidDeviceType,
}

impl HidReportBuilder {
    /// Create a new report builder for the specified device type.
    pub fn new(device_type: HidDeviceType) -> Self {
        Self { device_type }
    }

    /// Build a report from a command.
    pub fn build_report<T: HidCommand>(&self, command: T, buffer: &mut [u8]) -> Result<usize> {
        tracing::debug!(
            "Building HID report: {} ({})",
            command.description(),
            command.command_code()
        );

        let size = command.serialize(buffer, self.device_type)?;
        let data = &buffer[..size];
        self.validate_report_size(data, size)?;

        tracing::debug!(
            "Built {} byte HID report: {:02x?}",
            data.len(),
            &data[..data.len().min(32)]
        );
        Ok(size)
    }

    /// Validate a raw HID report against the device's standard report size.
    pub fn validate_report(&self, data: &[u8]) -> Result<()> {
        self.validate_report_size(data, self.device_type.report_size())
    }

    fn validate_report_size(&self, data: &[u8], expected_size: usize) -> Result<()> {
        if data.len() != expected_size {
            return Err(Error::DeviceCommunication(format!(
                "Invalid report size: {} bytes (expected {})",
                data.len(),
                expected_size
            )));
        }

        // Check report ID for keyboards
        if self.device_type.includes_report_id() && !data.is_empty() && data[0] != 0x00 {
            tracing::warn!("Unexpected report ID: 0x{:02x} (expected 0x00)", data[0]);
        }

        // Validate command code
        let cmd_offset = if self.device_type.includes_report_id() { 1 } else { 0 };
        if data.len() > cmd_offset {
            let cmd_byte = data[cmd_offset];
            if CommandCode::from_report_byte(cmd_byte).is_none() {
                tracing::warn!("Unknown command byte: 0x{:02x}", cmd_byte);
            }
        }

        Ok(())
    }

    /// Parse a raw report to extract the command code.
    pub fn parse_command_code(&self, data: &[u8]) -> Option<CommandCode> {
        let cmd_offset = if self.device_type.includes_report_id() { 1 } else { 0 };

        if data.len() <= cmd_offset {
            return None;
        }

        CommandCode::from_report_byte(data[cmd_offset])
    }
}

/// HID error recovery system for handling hot-plug scenarios.
pub mod recovery {
    use super::*;
    use hidapi::{HidDevice, HidError};
    use std::time::{Duration, Instant};
    use thiserror::Error;
    use tracing::{debug, error, warn};

    /// Maximum number of retry attempts before marking error as permanent
    const MAX_RETRY_ATTEMPTS: u8 = 5;

    /// Base delay for exponential backoff (milliseconds)
    const BASE_BACKOFF_MS: u64 = 50;

    /// Maximum backoff delay to prevent excessive waiting
    const MAX_BACKOFF_MS: u64 = 2000;

    /// Circuit breaker threshold - errors per minute to trigger breaker
    const CIRCUIT_BREAKER_THRESHOLD: u32 = 10;

    /// Time window for circuit breaker error counting
    const CIRCUIT_BREAKER_WINDOW: Duration = Duration::from_secs(60);

    /// HID report validation and error recovery errors
    #[derive(Error, Debug, Clone)]
    pub enum HidReportError {
        #[error("Device disconnected during operation")]
        DeviceDisconnected,

        #[error("Invalid report format: {0}")]
        InvalidReport(String),

        #[error("Communication timeout after {0}ms")]
        Timeout(u64),

        #[error("Recovery failed after {0} attempts")]
        RecoveryFailed(u8),

        #[error("Circuit breaker open - too many errors ({0}/min)")]
        CircuitBreakerOpen(u32),

        #[error("Transient error: {0}")]
        Transient(String),

        #[error("Permanent error: {0}")]
        Permanent(String),
    }

    /// Categories of HID errors for appropriate recovery strategies
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum ErrorCategory {
        /// Temporary errors that may resolve with retry
        Transient,
        /// Permanent errors requiring device reset or reconnection
        Permanent,
        /// Device physically disconnected
        Disconnected,
    }

    /// Connection health status for proactive failure detection
    #[derive(Debug, Clone)]
    pub struct ConnectionHealth {
        /// Success rate over recent operations (0.0 - 1.0)
        pub success_rate: f64,
        /// Average response time for recent operations
        pub avg_response_time_ms: f64,
        /// Number of consecutive failures
        pub consecutive_failures: u32,
        /// Last successful operation timestamp
        pub last_success: Option<Instant>,
        /// Total operations attempted
        pub total_operations: u64,
        /// Total successful operations
        pub successful_operations: u64,
    }

    impl Default for ConnectionHealth {
        fn default() -> Self {
            Self {
                success_rate: 1.0,
                avg_response_time_ms: 0.0,
                consecutive_failures: 0,
                last_success: None,
                total_operations: 0,
                successful_operations: 0,
            }
        }
    }

    impl ConnectionHealth {
        /// Check if connection is healthy enough for operations
        pub fn is_healthy(&self) -> bool {
            self.success_rate >= 0.8 && self.consecutive_failures < 3
        }

        /// Check if connection requires immediate attention
        pub fn needs_recovery(&self) -> bool {
            self.consecutive_failures >= 5 || self.success_rate < 0.5
        }

        /// Update health metrics after an operation
        pub fn record_operation(&mut self, success: bool, response_time: Duration) {
            self.total_operations += 1;

            if success {
                self.successful_operations += 1;
                self.consecutive_failures = 0;
                self.last_success = Some(Instant::now());
            } else {
                self.consecutive_failures += 1;
            }

            // Update success rate (sliding window approach)
            self.success_rate = self.successful_operations as f64 / self.total_operations as f64;

            // Update average response time (exponential moving average)
            let response_ms = response_time.as_millis() as f64;
            if self.avg_response_time_ms == 0.0 {
                self.avg_response_time_ms = response_ms;
            } else {
                // Use alpha = 0.1 for smoother averaging
                self.avg_response_time_ms = 0.9 * self.avg_response_time_ms + 0.1 * response_ms;
            }
        }
    }

    /// Circuit breaker state for preventing cascade failures
    #[derive(Debug)]
    struct CircuitBreaker {
        error_count: u32,
        window_start: Instant,
        is_open: bool,
    }

    impl Default for CircuitBreaker {
        fn default() -> Self {
            Self {
                error_count: 0,
                window_start: Instant::now(),
                is_open: false,
            }
        }
    }

    impl CircuitBreaker {
        /// Record an error and check if circuit should open
        fn record_error(&mut self) -> bool {
            let now = Instant::now();

            // Reset window if expired
            if now.duration_since(self.window_start) > CIRCUIT_BREAKER_WINDOW {
                self.error_count = 0;
                self.window_start = now;
                self.is_open = false;
            }

            self.error_count += 1;

            if self.error_count >= CIRCUIT_BREAKER_THRESHOLD {
                self.is_open = true;
                warn!(
                    "Circuit breaker opened: {} errors in {}s window",
                    self.error_count,
                    CIRCUIT_BREAKER_WINDOW.as_secs()
                );
            }

            self.is_open
        }

        /// Check if circuit breaker allows operations
        fn is_open(&self) -> bool {
            // Auto-reset after window expires
            if Instant::now().duration_since(self.window_start) > CIRCUIT_BREAKER_WINDOW {
                return false;
            }
            self.is_open
        }
    }

    /// HID error recovery system with exponential backoff and circuit breaker
    pub struct HidErrorRecovery {
        /// Connection health tracking
        health: ConnectionHealth,
        /// Circuit breaker for cascade failure prevention
        circuit_breaker: CircuitBreaker,
        /// Last recovery attempt timestamp
        last_recovery: Option<Instant>,
        /// Number of recovery attempts for current issue
        recovery_attempts: u8,
    }

    impl Default for HidErrorRecovery {
        fn default() -> Self {
            Self::new()
        }
    }

    impl HidErrorRecovery {
        /// Create a new error recovery system
        pub fn new() -> Self {
            Self {
                health: ConnectionHealth::default(),
                circuit_breaker: CircuitBreaker::default(),
                last_recovery: None,
                recovery_attempts: 0,
            }
        }

        /// Get current connection health status
        pub fn health(&self) -> &ConnectionHealth {
            &self.health
        }

        /// Record a successful operation
        pub fn record_success(&mut self, response_time: Duration) {
            self.health.record_operation(true, response_time);
            self.recovery_attempts = 0; // Reset on success
        }

        /// Record a failed operation and determine error category
        pub fn record_failure(&mut self, error: &HidError, response_time: Duration) -> ErrorCategory {
            self.health.record_operation(false, response_time);

            let category = self.categorize_error(error);

            // Record in circuit breaker only for transient errors
            // (permanent errors and disconnections are handled differently)
            if category == ErrorCategory::Transient {
                self.circuit_breaker.record_error();
            }

            category
        }

        /// Attempt to recover from an error with exponential backoff
        pub async fn recover_from_error(&mut self, device: &HidDevice, error: &HidError) -> Result<()> {
            if self.circuit_breaker.is_open() {
                return Err(Error::DeviceCommunication(
                    HidReportError::CircuitBreakerOpen(CIRCUIT_BREAKER_THRESHOLD).to_string(),
                ));
            }

            if self.recovery_attempts >= MAX_RETRY_ATTEMPTS {
                return Err(Error::DeviceCommunication(
                    HidReportError::RecoveryFailed(self.recovery_attempts).to_string(),
                ));
            }

            let error_category = self.categorize_error(error);

            match error_category {
                ErrorCategory::Disconnected => {
                    debug!("Device disconnected, recovery not possible");
                    return Err(Error::DeviceCommunication(
                        HidReportError::DeviceDisconnected.to_string(),
                    ));
                }

                ErrorCategory::Permanent => {
                    debug!("Permanent error detected, attempting device reset");
                    self.attempt_device_reset(device).await?;
                }

                ErrorCategory::Transient => {
                    debug!("Transient error, using exponential backoff");
                    self.exponential_backoff().await;
                }
            }

            self.recovery_attempts += 1;
            self.last_recovery = Some(Instant::now());

            Ok(())
        }

        /// Categorize HID error for appropriate recovery strategy
        fn categorize_error(&self, error: &HidError) -> ErrorCategory {
            match error {
                HidError::HidApiError { message } => {
                    let msg = message.to_lowercase();
                    if msg.contains("no device") || msg.contains("disconnected") || msg.contains("not found") {
                        ErrorCategory::Disconnected
                    } else if msg.contains("timeout") || msg.contains("busy") || msg.contains("again") {
                        ErrorCategory::Transient
                    } else {
                        ErrorCategory::Permanent
                    }
                }
                HidError::HidApiErrorEmpty => ErrorCategory::Transient,
                HidError::InitializationError => ErrorCategory::Permanent,
                HidError::InvalidZeroSizeData => ErrorCategory::Permanent,
                HidError::IncompleteSendError { .. } => ErrorCategory::Transient,
                HidError::SetBlockingModeError { .. } => ErrorCategory::Permanent,
                HidError::FromWideCharError { .. } => ErrorCategory::Permanent,
                HidError::OpenHidDeviceWithDeviceInfoError { .. } => ErrorCategory::Disconnected,
                HidError::IoError { .. } => ErrorCategory::Transient,
            }
        }

        /// Apply exponential backoff delay
        async fn exponential_backoff(&self) {
            let delay_ms = std::cmp::min(
                BASE_BACKOFF_MS * 2_u64.pow(self.recovery_attempts as u32),
                MAX_BACKOFF_MS,
            );

            debug!(
                "Applying exponential backoff: {}ms (attempt {})",
                delay_ms,
                self.recovery_attempts + 1
            );
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }

        /// Attempt device reset by sending a harmless command
        async fn attempt_device_reset(&self, device: &HidDevice) -> Result<()> {
            debug!("Attempting device reset");

            // Send a minimal harmless report to test device responsiveness
            let reset_report = vec![0u8; 65]; // Empty report should be safe

            match device.write(&reset_report) {
                Ok(_) => {
                    debug!("Device reset successful");
                    Ok(())
                }
                Err(e) => {
                    warn!("Device reset failed: {}", e);
                    Err(Error::DeviceCommunication(format!("Reset failed: {}", e)))
                }
            }
        }

        /// Check if recovery should be attempted based on current state
        pub fn should_attempt_recovery(&self) -> bool {
            if self.circuit_breaker.is_open() {
                return false;
            }

            if self.recovery_attempts >= MAX_RETRY_ATTEMPTS {
                return false;
            }

            // Rate limit recovery attempts
            if let Some(last) = self.last_recovery
                && last.elapsed() < Duration::from_millis(BASE_BACKOFF_MS)
            {
                return false;
            }

            true
        }
    }

    /// HID report validator for ensuring report integrity
    pub struct ReportValidator;

    impl ReportValidator {
        /// Validate HID report before transmission
        pub fn validate_report(report: &[u8]) -> Result<()> {
            // Check minimum size (should be 65 bytes for SteelSeries devices)
            if report.is_empty() {
                return Err(Error::DeviceCommunication(
                    HidReportError::InvalidReport("Report cannot be empty".to_string()).to_string(),
                ));
            }

            // Check maximum size (prevent buffer overflows)
            if report.len() > 65 {
                return Err(Error::DeviceCommunication(
                    HidReportError::InvalidReport(format!("Report too large: {} bytes (max 65)", report.len()))
                        .to_string(),
                ));
            }

            // For SteelSeries devices, we expect exactly 65 bytes
            if report.len() != 65 {
                warn!("Unexpected report size: {} bytes (expected 65)", report.len());
            }

            // Basic validation for RGB commands (if applicable)
            if report.len() >= 2 {
                let command = report[1];
                match command {
                    0x21 => {
                        // RGB color command
                        if report.len() < 29 {
                            return Err(Error::DeviceCommunication(
                                HidReportError::InvalidReport("RGB command too short".to_string()).to_string(),
                            ));
                        }
                    }
                    _ => {
                        // Other commands are generally acceptable
                        debug!("Validating command: 0x{:02x}", command);
                    }
                }
            }

            Ok(())
        }

        /// Validate and potentially correct report format
        pub fn validate_and_correct(report: &mut Vec<u8>) -> Result<()> {
            // Ensure report is exactly 65 bytes for SteelSeries devices
            match report.len() {
                len if len < 65 => {
                    // Pad with zeros
                    report.resize(65, 0);
                    debug!("Padded report from {} to 65 bytes", len);
                }
                len if len > 65 => {
                    // Truncate to 65 bytes
                    report.truncate(65);
                    warn!("Truncated report from {} to 65 bytes", len);
                }
                65 => {
                    // Perfect size, no correction needed
                }
                _ => unreachable!(),
            }

            // Validate the corrected report
            Self::validate_report(report)
        }
    }

    /// High-level function to write HID report with automatic error recovery
    pub async fn write_report_with_recovery(
        device: &HidDevice,
        report: &[u8],
        recovery: &mut HidErrorRecovery,
    ) -> Result<()> {
        // Validate report first
        ReportValidator::validate_report(report)?;

        let start_time = Instant::now();

        loop {
            match device.write(report) {
                Ok(bytes_written) => {
                    let response_time = start_time.elapsed();
                    recovery.record_success(response_time);

                    debug!("HID write successful: {} bytes in {:?}", bytes_written, response_time);
                    return Ok(());
                }

                Err(hid_error) => {
                    let response_time = start_time.elapsed();
                    let error_category = recovery.record_failure(&hid_error, response_time);

                    warn!("HID write failed: {} (category: {:?})", hid_error, error_category);

                    if !recovery.should_attempt_recovery() {
                        error!("Recovery not possible, giving up");
                        return Err(Error::DeviceCommunication(format!("HID write failed: {}", hid_error)));
                    }

                    // Attempt recovery
                    match recovery.recover_from_error(device, &hid_error).await {
                        Ok(()) => {
                            debug!("Recovery successful, retrying write");
                            continue; // Retry the write
                        }
                        Err(recovery_error) => {
                            error!("Recovery failed: {}", recovery_error);
                            return Err(recovery_error);
                        }
                    }
                }
            }
        }
    }
}

// Re-export recovery types for convenience
pub use recovery::{
    ConnectionHealth, ErrorCategory, HidErrorRecovery, HidReportError, ReportValidator, write_report_with_recovery,
};

#[cfg(test)]
mod tests {
    use super::*;
    use recovery::*;

    #[test]
    fn firmware_poll_carries_the_source_byte() {
        for (command, expected) in [
            (OnboardFirmwareVersionCommand::KEYBOARD, 0x00u8),
            (OnboardFirmwareVersionCommand::LED, 0x01),
        ] {
            let mut buffer = [0u8; ONBOARD_OUTPUT_REPORT_SIZE];
            let size = command
                .serialize(&mut buffer, HidDeviceType::Keyboard)
                .expect("firmware poll fits");
            assert_eq!(size, ONBOARD_OUTPUT_REPORT_SIZE);
            assert_eq!(buffer[0], 0x00, "report number 0, as the capture's wValue 0x0200");
            assert_eq!(&buffer[1..3], &[0x90, expected], "source {}", command.source);
            assert_eq!(command.command_code(), CommandCode::OnboardFirmwareVersion);
            assert!(
                buffer[3..].iter().all(|b| *b == 0),
                "the 64-byte data stage is zero padded like the capture's frames"
            );
        }
    }

    #[test]
    fn driver_mode_command_is_the_engine_open_and_close_frames() {
        for (mode, expected) in [(DriverModeState::Engaged, 0x01u8), (DriverModeState::Released, 0x00)] {
            let command = OnboardDriverModeCommand { mode };
            let mut buffer = [0u8; ONBOARD_OUTPUT_REPORT_SIZE];
            assert_eq!(
                command.serialize(&mut buffer, HidDeviceType::Keyboard).unwrap(),
                ONBOARD_OUTPUT_REPORT_SIZE
            );
            assert_eq!(buffer[0], 0x00, "report number 0, as the capture's wValue 0x0200");
            assert_eq!(&buffer[1..4], &[0x6c, 0x00, expected], "mode {mode:?}");
            assert_eq!(command.command_code(), CommandCode::OnboardDriverMode);
            assert!(buffer[4..].iter().all(|b| *b == 0), "zero padded data stage");
        }
    }

    #[test]
    fn load_profile_command_uses_the_1_based_profile_id() {
        let builder = HidReportBuilder::new(HidDeviceType::Keyboard);
        for slot in 0..ONBOARD_PROFILE_SLOT_COUNT {
            let command = OnboardProfileDeployCommand { profile: slot };
            let wire_id = slot.saturating_add(1);
            let mut buffer = [0u8; ONBOARD_OUTPUT_REPORT_SIZE];
            assert_eq!(
                builder.build_report(command, &mut buffer).unwrap(),
                ONBOARD_OUTPUT_REPORT_SIZE
            );
            assert_eq!(buffer[0], 0x00, "report number 0, as the capture's wValue 0x0200");
            assert_eq!(
                &buffer[1..3],
                &[0x89, wire_id],
                "slot {slot} goes out as profile {wire_id}"
            );
            assert!(buffer[3..].iter().all(|b| *b == 0), "zero padded data stage");
        }
        assert!(
            OnboardProfileDeployCommand {
                profile: ONBOARD_PROFILE_SLOT_COUNT
            }
            .validate()
            .is_err(),
            "the engine accepts five slots"
        );
    }

    /// The four write-side frames, byte for byte as the Apex 7 write capture shows them
    /// (GG write capture, device 8: the wire bytes there are the data stage, i.e. this
    /// crate's report buffer without its report number).
    #[test]
    fn the_write_frames_are_the_2026_10_20_capture_shape() {
        // The profile-file open: capture `88 00 01 02`, wValue 0x0200 wLength 64.
        let open = OnboardFileOpenCommand { fs_id: 0x0201 };
        let mut buffer = [0u8; ONBOARD_OUTPUT_REPORT_SIZE];
        assert_eq!(
            open.serialize(&mut buffer, HidDeviceType::Keyboard).unwrap(),
            ONBOARD_OUTPUT_REPORT_SIZE
        );
        assert_eq!(
            &buffer[..OnboardFileOpenCommand::FRAME_LEN + 1],
            &[0x00, 0x88, 0x00, 0x01, 0x02],
            "88 00 | file id 0x0201 little endian"
        );
        assert!(buffer[5..].iter().all(|b| *b == 0), "zero padded data stage");

        // The two buffer fills of one window: capture `03 00 00 …` (buffer 0) and
        // `03 00 00 …` (buffer 512), each 512 bytes, wValue 0x0300 wLength 642.
        let mut head = vec![0u8; 512];
        head[0] = 0x0b;
        head[2..10].copy_from_slice(b"Config 1");
        for (buffer_offset, expect_offset) in [(0u16, [0x00u8, 0x00]), (512, [0x00, 0x02])] {
            let write = OnboardBufferWriteCommand {
                buffer_offset,
                data: &head,
            };
            let mut feature = [0u8; ONBOARD_FEATURE_REPORT_SIZE];
            assert_eq!(
                write.serialize(&mut feature, HidDeviceType::Keyboard).unwrap(),
                ONBOARD_FEATURE_REPORT_SIZE,
                "the write goes in the 643-byte report (642 on the wire) the capture shows"
            );
            assert_eq!(feature[0], 0x00, "report number 0");
            assert_eq!(&feature[1..3], &[0x03, 0x00], "header 03 00, not the 2023 chunk header");
            assert_eq!(&feature[3..5], &expect_offset, "buffer offset {buffer_offset}");
            assert_eq!(&feature[5..7], &[0x00, 0x02], "size 512 little endian");
            assert_eq!(
                &feature[7..7 + head.len()],
                &head[..],
                "payload bytes start behind the header"
            );
            assert!(
                feature[7 + head.len()..].iter().all(|b| *b == 0),
                "the rest of the report is padding"
            );
        }

        // The push of window 0: capture `05 00 01 …`, wValue 0x0200 wLength 64.
        let push = OnboardPayloadPushCommand {
            fs_id: 0x0201,
            file_offset: 0,
            size: 1024,
        };
        let mut buffer = [0u8; ONBOARD_OUTPUT_REPORT_SIZE];
        assert_eq!(
            push.serialize(&mut buffer, HidDeviceType::Keyboard).unwrap(),
            ONBOARD_OUTPUT_REPORT_SIZE
        );
        assert_eq!(
            &buffer[1..OnboardPayloadPushCommand::FRAME_LEN + 2],
            &[0x05, 0x00, 0x01, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00],
            "capture 05 00 | file id u16 | file offset u32 | size u16, all little endian, behind the report number"
        );
        assert!(
            buffer[OnboardPayloadPushCommand::FRAME_LEN + 2..]
                .iter()
                .all(|b| *b == 0),
            "zero padded data stage"
        );

        // The commit: capture `eb 01 00 …`, wValue 0x0200 wLength 64 - an output report, slot 0 going
        // out as 1.
        let commit = OnboardFileCommitCommand { profile: 0 };
        let mut buffer = [0u8; ONBOARD_OUTPUT_REPORT_SIZE];
        assert_eq!(
            commit.serialize(&mut buffer, HidDeviceType::Keyboard).unwrap(),
            ONBOARD_OUTPUT_REPORT_SIZE
        );
        assert_eq!(&buffer[..3], &[0x00, 0xEB, 0x01], "eb | 1-based profile id");
        assert!(buffer[3..].iter().all(|b| *b == 0), "zero padded data stage");
    }

    #[test]
    fn write_frames_are_validated_before_they_reach_a_keyboard() {
        let big = vec![0u8; OnboardBufferWriteCommand::MAX_DATA_LEN + 1];
        assert!(
            OnboardBufferWriteCommand {
                buffer_offset: 0,
                data: &big,
            }
            .validate()
            .is_err(),
            "a write frame carries what one feature report holds"
        );
        assert!(
            OnboardBufferWriteCommand {
                buffer_offset: 0,
                data: &[],
            }
            .validate()
            .is_err(),
            "an empty write is not a frame the engine sends"
        );
        assert!(
            OnboardPayloadPushCommand {
                fs_id: 0,
                file_offset: 0,
                size: 1024,
            }
            .validate()
            .is_err(),
            "a push needs a file id"
        );
        assert!(
            OnboardPayloadPushCommand {
                fs_id: 0x0201,
                file_offset: 0,
                size: 1025,
            }
            .validate()
            .is_err(),
            "a push copies at most one resource buffer"
        );
        assert!(
            OnboardFileOpenCommand { fs_id: 0 }.validate().is_err(),
            "the write API opens a profile file id, never 0"
        );
        assert!(
            OnboardFileCommitCommand {
                profile: ONBOARD_PROFILE_SLOT_COUNT
            }
            .validate()
            .is_err(),
            "the engine commits five slots"
        );
    }

    #[test]
    fn pull_frame_matches_the_capture_layout() {
        let cmd = OnboardPayloadPullCommand {
            fs_id: 0x0201,
            file_offset: 4096,
            size: 1024,
        };
        let mut buffer = [0u8; ONBOARD_OUTPUT_REPORT_SIZE];
        assert_eq!(
            cmd.serialize(&mut buffer, HidDeviceType::Keyboard).unwrap(),
            ONBOARD_OUTPUT_REPORT_SIZE
        );
        assert_eq!(buffer[0], 0x00);
        assert_eq!(
            &buffer[1..OnboardPayloadPullCommand::FRAME_LEN + 1],
            [0x85, 0x00, 0x01, 0x02, 0x00, 0x10, 0x00, 0x00, 0x00, 0x04],
            "the capture's frame bytes, behind the report number"
        );
        assert!(buffer[11..].iter().all(|b| *b == 0));
        assert!(
            OnboardPayloadPullCommand {
                fs_id: 0x0201,
                file_offset: 0,
                size: 2048,
            }
            .validate()
            .is_err(),
            "a pull cannot exceed the resource buffer"
        );
    }

    #[test]
    fn onboard_reports_build_through_the_builder() {
        let builder = HidReportBuilder::new(HidDeviceType::Keyboard);
        let mut chain = [0u8; ONBOARD_OUTPUT_REPORT_SIZE];
        assert_eq!(
            builder
                .build_report(OnboardFirmwareVersionCommand::KEYBOARD, &mut chain)
                .unwrap(),
            ONBOARD_OUTPUT_REPORT_SIZE
        );
        let mut pull = [0u8; ONBOARD_OUTPUT_REPORT_SIZE];
        assert_eq!(
            builder
                .build_report(
                    OnboardPayloadPullCommand {
                        fs_id: 0x0201,
                        file_offset: 0,
                        size: 1024,
                    },
                    &mut pull
                )
                .unwrap(),
            ONBOARD_OUTPUT_REPORT_SIZE
        );
        let mut read = [0u8; ONBOARD_FEATURE_REPORT_SIZE];
        assert_eq!(
            builder
                .build_report(
                    OnboardBufferReadCommand {
                        buffer_offset: 512,
                        size: 512,
                    },
                    &mut read
                )
                .unwrap(),
            ONBOARD_FEATURE_REPORT_SIZE
        );
        for buffer in [&chain[..], &pull[..], &read[..]] {
            assert_eq!(buffer[0], 0x00, "report number 0 is byte 0 of every buffer");
        }
        assert!(
            builder
                .build_report(
                    OnboardPayloadPullCommand {
                        fs_id: 0x0201,
                        file_offset: 0,
                        size: 4096,
                    },
                    &mut pull
                )
                .is_err(),
            "the builder rejects an invalid pull before anything is sent"
        );
    }

    #[test]
    fn read_frame_matches_the_capture_layout() {
        let cmd = OnboardBufferReadCommand {
            buffer_offset: 512,
            size: 512,
        };
        let mut buffer = [0u8; ONBOARD_FEATURE_REPORT_SIZE];
        assert_eq!(
            cmd.serialize(&mut buffer, HidDeviceType::Keyboard).unwrap(),
            ONBOARD_FEATURE_REPORT_SIZE
        );
        assert_eq!(
            &buffer[1..OnboardBufferReadCommand::FRAME_LEN + 1],
            [0x83, 0x00, 0x00, 0x02, 0x00, 0x02]
        );
        assert_eq!(
            OnboardBufferReadCommand {
                buffer_offset: 0,
                size: 2,
            }
            .serialize(&mut buffer, HidDeviceType::Keyboard)
            .unwrap(),
            ONBOARD_FEATURE_REPORT_SIZE
        );
        assert_eq!(
            &buffer[1..OnboardBufferReadCommand::FRAME_LEN + 1],
            [0x83, 0x00, 0x00, 0x00, 0x02, 0x00]
        );
        let mut buffer = [0u8; ONBOARD_FEATURE_REPORT_SIZE];
        assert_eq!(
            cmd.serialize(&mut buffer, HidDeviceType::Keyboard).unwrap(),
            ONBOARD_FEATURE_REPORT_SIZE
        );
        assert_eq!(buffer[0], 0x00, "report number 0, as the capture's wValue 0x0300");
        assert!(buffer[7..].iter().all(|b| *b == 0));
        assert!(
            OnboardBufferReadCommand {
                buffer_offset: 0,
                size: 1024,
            }
            .validate()
            .is_err(),
            "a read cannot ask for more than one reply report carries"
        );
        let mut small = [0u8; 64];
        assert!(
            cmd.serialize(&mut small, HidDeviceType::Keyboard).is_err(),
            "the feature report needs its full buffer"
        );
    }

    #[test]
    fn read_back_length_and_its_ceiling_match_the_reply_the_kernel_hands_us() {
        // Read side: the buffer length goes to the device verbatim (HIDIOCGFEATURE), so the ask equals the
        // capture's GET wLength. Write side: the kernel strips the report number, so the 643-byte buffer of
        // ONBOARD_FEATURE_REPORT_SIZE goes out as wLength 642. The asymmetry is the kernel's, and the
        // measured lengths are the anchor: 642 answers, 643 never completes
        assert_eq!(ONBOARD_READ_WIRE_BYTES, 642, "the capture's GET wLength and our ask");
        assert_eq!(
            ONBOARD_FEATURE_REPORT_SIZE,
            ONBOARD_READ_WIRE_BYTES + 1,
            "the write buffer carries a report-number byte the wire does not"
        );
        assert_eq!(
            ONBOARD_MAX_READ_LEN, 636,
            "642 reply bytes minus the report-ID byte and the 5-byte header"
        );
        assert!(
            OnboardBufferReadCommand {
                buffer_offset: 0,
                size: 636,
            }
            .validate()
            .is_ok(),
            "the largest payload a header-echoing reply can carry is accepted"
        );
        assert!(
            OnboardBufferReadCommand {
                buffer_offset: 0,
                size: 637,
            }
            .validate()
            .is_err(),
            "one byte more would not fit behind the header in the reply the kernel delivers"
        );
    }

    #[test]
    fn test_rgb_zone_command_validation() {
        // Valid command
        let cmd = RgbZoneCommand::new_single_color(Color::RED, 5);
        assert!(cmd.validate().is_ok());

        // Empty colors should fail
        let cmd = RgbZoneCommand {
            colors: vec![].into(),
            zone_selector: 0xFF,
        };
        assert!(cmd.validate().is_err());

        // Too many colors should fail
        let cmd = RgbZoneCommand {
            colors: vec![Color::RED; MAX_RGB_ZONES + 1].into(),
            zone_selector: 0xFF,
        };
        assert!(cmd.validate().is_err());
    }

    #[test]
    fn test_brightness_command_validation() {
        // Valid brightness
        let cmd = BrightnessCommand::new(50);
        assert_eq!(cmd.brightness, 50);
        assert!(cmd.validate().is_ok());

        // Auto-clamping
        let cmd = BrightnessCommand::new(150);
        assert_eq!(cmd.brightness, 100);

        // Manual validation should fail for over 100
        let cmd = BrightnessCommand { brightness: 150 };
        assert!(cmd.validate().is_err());
    }

    #[test]
    fn test_keyboard_report_serialization() {
        let builder = HidReportBuilder::new(HidDeviceType::Keyboard);
        let mut buffer = [0u8; KEYBOARD_REPORT_SIZE];

        // RGB command
        let rgb_cmd = RgbZoneCommand::new_single_color(Color::RED, 1);
        let size = builder.build_report(rgb_cmd, &mut buffer).unwrap();
        assert_eq!(size, KEYBOARD_REPORT_SIZE);
        assert_eq!(buffer[0], 0x00); // Report ID
        assert_eq!(buffer[1], 0x21); // RGB command
        assert_eq!(buffer[2], 0xFF); // Zone selector
        assert_eq!(buffer[3], 255); // Red
        assert_eq!(buffer[4], 0); // Green
        assert_eq!(buffer[5], 0); // Blue

        // Brightness command
        let brightness_cmd = BrightnessCommand::new(75);
        let size = builder.build_report(brightness_cmd, &mut buffer).unwrap();
        assert_eq!(size, KEYBOARD_REPORT_SIZE);
        assert_eq!(buffer[0], 0x00); // Report ID
        assert_eq!(buffer[1], 0x22); // Brightness command
        assert_eq!(buffer[2], 75); // Brightness value

        // Apply command
        let apply_cmd = ApplyCommand;
        let size = builder.build_report(apply_cmd, &mut buffer).unwrap();
        assert_eq!(size, KEYBOARD_REPORT_SIZE);
        assert_eq!(buffer[0], 0x00); // Report ID
        assert_eq!(buffer[1], 0x09); // Apply command
    }

    #[test]
    fn test_headset_report_serialization() {
        let builder = HidReportBuilder::new(HidDeviceType::Headset);
        let mut buffer = [0u8; HEADSET_REPORT_SIZE];

        // RGB command
        let rgb_cmd = RgbZoneCommand::new_single_color(Color::BLUE, 1);
        let size = builder.build_report(rgb_cmd, &mut buffer).unwrap();
        assert_eq!(size, HEADSET_REPORT_SIZE);
        assert_eq!(buffer[0], 0x21); // RGB command (no report ID)
        assert_eq!(buffer[1], 0xFF); // Zone selector
        assert_eq!(buffer[2], 0); // Red
        assert_eq!(buffer[3], 0); // Green
        assert_eq!(buffer[4], 255); // Blue
    }

    #[test]
    fn test_command_code_parsing() {
        let builder = HidReportBuilder::new(HidDeviceType::Keyboard);

        let rgb_data = vec![0x00, 0x21, 0xFF, 255, 0, 0]; // RGB command
        assert_eq!(builder.parse_command_code(&rgb_data), Some(CommandCode::RgbControl));

        let brightness_data = vec![0x00, 0x22, 50]; // Brightness command
        assert_eq!(
            builder.parse_command_code(&brightness_data),
            Some(CommandCode::Brightness)
        );

        let apply_data = vec![0x00, 0x09]; // Apply command
        assert_eq!(builder.parse_command_code(&apply_data), Some(CommandCode::Apply));

        let unknown_data = vec![0x00, 0xFF]; // No command in this crate parses as 0xFF
        assert_eq!(builder.parse_command_code(&unknown_data), None);

        let perkey_data = vec![0x00, 0x2A, 0x00, 0x01]; // Per-key command
        assert_eq!(builder.parse_command_code(&perkey_data), Some(CommandCode::PerKeyRgb));
    }

    #[test]
    fn test_report_validation() {
        let builder = HidReportBuilder::new(HidDeviceType::Keyboard);

        // Valid report
        let valid_data = vec![0u8; KEYBOARD_REPORT_SIZE];
        assert!(builder.validate_report(&valid_data).is_ok());

        // Invalid size
        let invalid_data = vec![0u8; 32];
        assert!(builder.validate_report(&invalid_data).is_err());
    }

    #[test]
    fn test_per_key_rgb_command() {
        // Test single key command
        let addr = KeyAddress::new(0x04); // HID code for 'a' key
        let color = Color::RED;
        let cmd = PerKeyRgbCommand::single_key(addr, color);

        assert_eq!(cmd.key_count(), 1);
        assert_eq!(cmd.get_key_color(addr), Some(color));
        assert!(!cmd.is_empty());
        assert_eq!(cmd.addressing_mode, PerKeyAddressingMode::HidCode);

        // Test validation
        assert!(cmd.validate().is_ok());

        // Test serialization
        let builder = HidReportBuilder::new(HidDeviceType::Keyboard);
        let mut buffer = [0u8; KEYBOARD_REPORT_SIZE];
        let size = builder.build_report(cmd, &mut buffer).unwrap();
        assert_eq!(size, KEYBOARD_REPORT_SIZE);
        assert_eq!(buffer[0], 0x00); // Report ID
        assert_eq!(buffer[1], 0x23); // Per-key RGB command
        assert_eq!(buffer[2], 0x01); // Key count
        assert_eq!(buffer[3], 0x04); // HID code
        assert_eq!(buffer[4], 255); // Red
        assert_eq!(buffer[5], 0); // Green
        assert_eq!(buffer[6], 0); // Blue
    }

    #[test]
    fn test_per_key_rgb_builder() {
        let mut builder = PerKeyRgbBuilder::new(PerKeyAddressingMode::HidCode);

        // Initially empty
        assert!(builder.is_empty());
        assert_eq!(builder.key_count(), 0);

        // Add keys
        builder.add_key_matrix(KeyAddress::new(0x04), Color::RED);
        builder.add_key_matrix(KeyAddress::new(0x05), Color::GREEN);
        builder.add_key_matrix(KeyAddress::new(0x06), Color::BLUE);

        assert_eq!(builder.key_count(), 3);
        assert!(!builder.is_empty());

        // Add batch
        let addresses = vec![KeyAddress::new(0x07), KeyAddress::new(0x08), KeyAddress::new(0x09)];
        builder.add_keys_batch(&addresses, Color::WHITE);
        assert_eq!(builder.key_count(), 6);

        // Set region (start_hid, count, color)
        builder.set_region(0x10, 4, Color::CYAN);
        assert!(builder.key_count() >= 6); // At least original + region keys

        // Build final command
        let cmd = builder.build();
        assert!(cmd.key_count() >= 6);
        assert_eq!(cmd.addressing_mode, PerKeyAddressingMode::HidCode);
        assert!(cmd.validate().is_ok());
    }

    #[test]
    fn test_apex_8zone_brightness_report() {
        assert_eq!(Apex8ZoneBrightnessCommand::from_percent(0).level, 0x00);
        assert_eq!(Apex8ZoneBrightnessCommand::from_percent(50).level, 0x08);
        assert_eq!(Apex8ZoneBrightnessCommand::from_percent(100).level, 0x10);
        assert_eq!(Apex8ZoneBrightnessCommand::from_percent(255).level, 0x10);

        let builder = HidReportBuilder::new(HidDeviceType::Keyboard);
        let mut buffer = [0xAAu8; KEYBOARD_REPORT_SIZE];
        let size = builder
            .build_report(Apex8ZoneBrightnessCommand::from_percent(50), &mut buffer)
            .unwrap();
        assert_eq!(size, KEYBOARD_REPORT_SIZE);
        assert_eq!(&buffer[..3], &[0x00, 0x23, 0x08]);
        assert!(buffer[3..].iter().all(|&b| b == 0));

        assert!(Apex8ZoneBrightnessCommand { level: 0x11 }.validate().is_err());
    }

    #[test]
    fn test_per_key_rgb_validation() {
        // Empty command should fail
        let empty_cmd = PerKeyRgbCommand::new(PerKeyAddressingMode::HidCode);
        assert!(empty_cmd.validate().is_err());

        // Valid command should pass (HID code 0x04 = 'a' key)
        let mut valid_cmd = PerKeyRgbCommand::new(PerKeyAddressingMode::HidCode);
        valid_cmd.set_key_color(KeyAddress::new(0x04), Color::RED);
        assert!(valid_cmd.validate().is_ok());

        // Too many keys should fail
        let mut oversize_cmd = PerKeyRgbCommand::new(PerKeyAddressingMode::HidCode);
        for i in 0..20 {
            // 20 keys * 5 bytes = 100 bytes > 64 byte limit
            oversize_cmd.set_key_color(KeyAddress::new(i + 0x04), Color::RED);
        }
        assert!(oversize_cmd.validate().is_err());
    }

    #[test]
    #[allow(deprecated)]
    fn test_per_key_addressing_modes() {
        // Matrix and Logical are deprecated variants that still exist
        // but are separate from HidCode
        let matrix_mode = PerKeyAddressingMode::Matrix;
        let logical_mode = PerKeyAddressingMode::Logical;
        let hid_mode = PerKeyAddressingMode::HidCode;

        // All modes should be different (deprecated variants are separate)
        assert_ne!(matrix_mode, hid_mode);
        assert_ne!(logical_mode, hid_mode);
        assert_ne!(matrix_mode, logical_mode);

        // Commands created with deprecated modes are converted to HidCode
        // (the deprecated modes are no longer supported)
        let cmd_matrix = PerKeyRgbCommand::new(matrix_mode);
        let cmd_logical = PerKeyRgbCommand::new(logical_mode);

        // The addressing mode is always HidCode regardless of input
        assert_eq!(cmd_matrix.addressing_mode, hid_mode);
        assert_eq!(cmd_logical.addressing_mode, hid_mode);
    }

    #[test]
    fn test_per_key_rgb_manipulation() {
        let mut cmd = PerKeyRgbCommand::new(PerKeyAddressingMode::HidCode);
        let addr1 = KeyAddress::new(0x04); // HID code for 'a'
        let addr2 = KeyAddress::new(0x05); // HID code for 'b'

        // Add keys
        cmd.set_key_color(addr1, Color::RED);
        cmd.set_key_color(addr2, Color::GREEN);
        assert_eq!(cmd.key_count(), 2);

        // Check colors
        assert_eq!(cmd.get_key_color(addr1), Some(Color::RED));
        assert_eq!(cmd.get_key_color(addr2), Some(Color::GREEN));

        // Remove key
        cmd.remove_key(addr1);
        assert_eq!(cmd.key_count(), 1);
        assert_eq!(cmd.get_key_color(addr1), None);
        assert_eq!(cmd.get_key_color(addr2), Some(Color::GREEN));

        // Get addresses
        let addresses = cmd.get_addresses();
        assert_eq!(addresses.len(), 1);
        assert!(addresses.contains(&addr2));
    }

    // Tests for HID error recovery system
    #[test]
    fn test_hid_connection_health_tracking() {
        use std::time::Duration;

        let mut health = ConnectionHealth::default();
        assert!(health.is_healthy());
        assert_eq!(health.consecutive_failures, 0);

        // Record successful operations
        health.record_operation(true, Duration::from_millis(10));
        assert!(health.is_healthy());
        assert_eq!(health.success_rate, 1.0);

        // Record some failures
        health.record_operation(false, Duration::from_millis(50));
        health.record_operation(false, Duration::from_millis(50));

        assert_eq!(health.consecutive_failures, 2);
        assert_eq!(health.success_rate, 1.0 / 3.0);
        assert!(!health.is_healthy()); // Not healthy due to low success rate (33%)

        // More failures should trigger unhealthy state
        health.record_operation(false, Duration::from_millis(50));
        health.record_operation(false, Duration::from_millis(50));

        assert_eq!(health.consecutive_failures, 4);
        assert!(!health.is_healthy()); // Now unhealthy
        assert!(health.needs_recovery()); // Needs recovery
    }

    #[test]
    fn test_hid_report_validation() {
        // Valid report
        let valid_report = vec![0u8; 65];
        assert!(ReportValidator::validate_report(&valid_report).is_ok());

        // Empty report
        let empty_report = vec![];
        assert!(ReportValidator::validate_report(&empty_report).is_err());

        // Too large report
        let large_report = vec![0u8; 100];
        assert!(ReportValidator::validate_report(&large_report).is_err());

        // RGB command validation
        let mut rgb_report = vec![0u8; 65];
        rgb_report[1] = 0x21; // RGB command
        assert!(ReportValidator::validate_report(&rgb_report).is_ok());
    }

    #[test]
    fn test_hid_report_correction() {
        // Test padding
        let mut short_report = vec![0u8; 30];
        assert!(ReportValidator::validate_and_correct(&mut short_report).is_ok());
        assert_eq!(short_report.len(), 65);

        // Test truncation
        let mut long_report = vec![0u8; 100];
        assert!(ReportValidator::validate_and_correct(&mut long_report).is_ok());
        assert_eq!(long_report.len(), 65);
    }

    #[test]
    fn test_hid_error_recovery_creation() {
        let recovery = HidErrorRecovery::new();
        assert!(recovery.health().is_healthy());
        assert_eq!(recovery.health().consecutive_failures, 0);
        assert!(recovery.should_attempt_recovery());
    }

    #[test]
    fn test_actuation_command_creation() {
        // Test creation from value
        let cmd = ActuationCommand::new(4);
        assert_eq!(cmd.actuation_point, 4);
        assert_eq!(cmd.to_mm(), 0.4);

        // Test creation from mm
        let cmd = ActuationCommand::from_mm(0.4);
        assert_eq!(cmd.actuation_point, 4);
        assert_eq!(cmd.to_mm(), 0.4);

        // Test creation with rounding
        let cmd = ActuationCommand::from_mm(0.37);
        assert_eq!(cmd.actuation_point, 4); // Rounded to nearest 0.1mm

        // Test clamping
        let cmd = ActuationCommand::from_mm(5.0);
        assert_eq!(cmd.actuation_point, 40); // Clamped to max 4.0mm (40 in 0.1mm units)
    }

    #[test]
    fn test_actuation_command_validation() {
        // Valid values
        let cmd = ActuationCommand::new(1);
        assert!(cmd.validate().is_ok());

        let cmd = ActuationCommand::new(40);
        assert!(cmd.validate().is_ok());

        // Invalid: zero
        let cmd = ActuationCommand::new(0);
        assert!(cmd.validate().is_err());

        // Invalid: too high
        let cmd = ActuationCommand::new(41);
        assert!(cmd.validate().is_err());
    }

    #[test]
    fn test_actuation_command_serialization() {
        let builder = HidReportBuilder::new(HidDeviceType::Keyboard);
        let mut buffer = [0u8; KEYBOARD_REPORT_SIZE];

        // Test valid command serialization
        let cmd = ActuationCommand::new(20);
        let size = builder.build_report(cmd, &mut buffer).unwrap();

        assert_eq!(size, KEYBOARD_REPORT_SIZE);
        assert_eq!(buffer[0], 0x00); // Report ID
        assert_eq!(buffer[1], 0x2D); // Actuation command
        assert_eq!(buffer[2], 20); // Actuation value

        // Test headset serialization
        let builder = HidReportBuilder::new(HidDeviceType::Headset);
        let mut buffer = [0u8; HEADSET_REPORT_SIZE];
        let cmd = ActuationCommand::new(15);
        let size = builder.build_report(cmd, &mut buffer).unwrap();
        assert_eq!(size, HEADSET_REPORT_SIZE);
        assert_eq!(buffer[0], 0x2D); // Actuation command (no report ID)
        assert_eq!(buffer[1], 15); // Actuation value
    }

    #[test]
    fn test_actuation_command_parsing() {
        let builder = HidReportBuilder::new(HidDeviceType::Keyboard);

        // Test parsing actuation command from data
        let actuation_data = vec![0x00, 0x2D, 25, 0, 0]; // Actuation command
        assert_eq!(
            builder.parse_command_code(&actuation_data),
            Some(CommandCode::ActuationControl)
        );
    }

    #[test]
    fn direct_frame_layout_and_padding() {
        let entries = [(0x04, Color::new(1, 2, 3)), (0x05, Color::new(4, 5, 6))];
        let command = DirectFrameCommand {
            packet: CommandCode::ApexGen1Direct,
            report_len: 643,
            entries: &entries,
        };
        let mut buffer = [0xAAu8; 700];
        let size = command.serialize(&mut buffer, HidDeviceType::Keyboard).unwrap();

        assert_eq!(size, 643);
        assert_eq!(&buffer[..11], &[0x00, 0x3A, 2, 0x04, 1, 2, 3, 0x05, 4, 5, 6]);
        assert!(buffer[11..643].iter().all(|&b| b == 0));
        assert_eq!(buffer[643], 0xAA, "bytes past the report are untouched");
    }

    #[test]
    fn direct_frame_rejects_bad_packet_id_and_overflow() {
        let entries = [(0x04, Color::RED); 4];
        let mut buffer = [0u8; 645];
        let wrong_packet = DirectFrameCommand {
            packet: CommandCode::Apply,
            report_len: 643,
            entries: &entries,
        };
        assert!(wrong_packet.serialize(&mut buffer, HidDeviceType::Keyboard).is_err());

        let too_small = DirectFrameCommand {
            packet: CommandCode::Apex2023Direct,
            report_len: 18, // header 3 + 4 entries * 4 = 19 bytes needed
            entries: &entries,
        };
        assert!(too_small.serialize(&mut buffer, HidDeviceType::Keyboard).is_err());

        let short_buffer = DirectFrameCommand {
            packet: CommandCode::Apex2023Direct,
            report_len: 645,
            entries: &entries,
        };
        assert!(
            short_buffer
                .serialize(&mut buffer[..100], HidDeviceType::Keyboard)
                .is_err()
        );
    }

    #[test]
    fn direct_init_is_4b_feature_report() {
        let mut buffer = [0xAAu8; 643];
        let size = DirectInitCommand
            .serialize(&mut buffer, HidDeviceType::Keyboard)
            .unwrap();

        assert_eq!(size, 643);
        assert_eq!(&buffer[..2], &[0x00, 0x4B]);
        assert!(buffer[2..].iter().all(|&b| b == 0));
    }

    #[test]
    fn direct_frame_fits_the_513_byte_apex_9_report() {
        let entries = [(0x04, Color::new(9, 8, 7)); 112];
        let command = DirectFrameCommand {
            packet: CommandCode::Apex2023Direct,
            report_len: 513,
            entries: &entries,
        };
        let mut buffer = [0u8; 513];
        assert_eq!(command.serialize(&mut buffer, HidDeviceType::Keyboard).unwrap(), 513);
        assert_eq!(&buffer[..7], &[0x00, 0x40, 0x70, 0x04, 9, 8, 7]);
        assert!(buffer[3 + 112 * 4..].iter().all(|&b| b == 0));
    }

    fn m750_report(command: &dyn HidCommand) -> Vec<u8> {
        let mut buffer = vec![0xAAu8; APEX_M_REPORT_SIZE + 4];
        let size = command.serialize(&mut buffer, HidDeviceType::Keyboard).unwrap();
        assert_eq!(size, APEX_M_REPORT_SIZE);
        assert!(
            buffer[APEX_M_REPORT_SIZE..].iter().all(|&b| b == 0xAA),
            "bytes past the report are untouched"
        );
        buffer.truncate(APEX_M_REPORT_SIZE);
        buffer
    }

    #[test]
    fn apex_m750_enable_reports_set_only_their_bytes() {
        let expected: [(ApexMEnableStep, &[(usize, u8)]); 3] = [
            (ApexMEnableStep::First, &[(4, 0x01), (6, 0x85)]),
            (ApexMEnableStep::Second, &[(4, 0x03), (5, 0x01), (7, 0xFF)]),
            (ApexMEnableStep::Third, &[(4, 0x01), (6, 0x85), (7, 0xFF)]),
        ];
        assert_eq!(
            ApexMEnableStep::ALL,
            [ApexMEnableStep::First, ApexMEnableStep::Second, ApexMEnableStep::Third]
        );
        for (step, set) in expected {
            let report = m750_report(&ApexMEnableCommand { step });
            for (i, &byte) in report.iter().enumerate() {
                let want = set.iter().find(|(at, _)| *at == i).map_or(0, |(_, value)| *value);
                assert_eq!(byte, want, "{step:?} byte {i}");
            }
        }
    }

    #[test]
    fn apex_m750_direct_frame_header_slots_and_padding() {
        let mut slots = [Color::new(1, 2, 3); APEX_M_SLOT_COUNT];
        slots[APEX_M_SLOT_COUNT - 1] = Color::new(4, 5, 6);
        let report = m750_report(&ApexMDirectCommand { slots: &slots });

        assert_eq!(&report[..9], &[0x00, 0x00, 0x00, 0x01, 0x8E, 0x01, 0x03, 0x06, 0x16]);
        assert_eq!(&report[9..15], &[1, 2, 3, 1, 2, 3]);
        let last = 9 + (APEX_M_SLOT_COUNT - 1) * 3;
        assert_eq!(&report[last..last + 3], &[4, 5, 6]);
        assert!(report[last + 3..].iter().all(|&b| b == 0));
        assert_eq!(report.len() - (last + 3), 513 - 9 - 132 * 3);
    }

    #[test]
    fn apex_m750_direct_frame_rejects_wrong_slot_count_and_short_buffer() {
        let mut buffer = [0u8; APEX_M_REPORT_SIZE];
        let few = [Color::RED; APEX_M_SLOT_COUNT - 1];
        assert!(
            ApexMDirectCommand { slots: &few }
                .serialize(&mut buffer, HidDeviceType::Keyboard)
                .is_err()
        );

        let slots = [Color::RED; APEX_M_SLOT_COUNT];
        let frame = ApexMDirectCommand { slots: &slots };
        assert!(frame.serialize(&mut buffer[..512], HidDeviceType::Keyboard).is_err());
        let enable = ApexMEnableCommand {
            step: ApexMEnableStep::First,
        };
        assert!(enable.serialize(&mut buffer[..512], HidDeviceType::Keyboard).is_err());
    }

    #[test]
    fn direct_packet_ids_are_known_command_codes() {
        for (byte, code) in [
            (0x40, CommandCode::Apex2023Direct),
            (0x61, CommandCode::Apex2023DirectWireless),
            (0x3A, CommandCode::ApexGen1Direct),
            (0x4B, CommandCode::Apex2023Init),
        ] {
            assert_eq!(CommandCode::from_report_byte(byte), Some(code));
            assert_eq!(code as u8, byte);
        }
    }

    #[test]
    fn tag_only_m750_codes_are_not_parsed_from_report_bytes() {
        // Bytes 4 and 5 of the M750 headers, not command bytes at offset 1: parsing them would misread
        // unrelated reports.
        assert_eq!(CommandCode::from_report_byte(0x8E), None);
        assert_eq!(CommandCode::from_report_byte(0x01), None);
    }

    fn headset_report(command: &dyn HidCommand) -> Vec<u8> {
        let mut buffer = vec![0xAAu8; HEADSET_REPORT_SIZE + 4];
        let size = command.serialize(&mut buffer, HidDeviceType::Headset).unwrap();
        assert_eq!(size, HEADSET_REPORT_SIZE);
        assert!(
            buffer[HEADSET_REPORT_SIZE..].iter().all(|&b| b == 0xAA),
            "bytes past the report are untouched"
        );
        buffer.truncate(HEADSET_REPORT_SIZE);
        buffer
    }

    #[test]
    fn headset_setting_frames_are_64_byte_zero_padded() {
        for (code, value, opcode) in [
            (CommandCode::HeadsetSidetone, 0, 0x39),
            (CommandCode::HeadsetSidetone, 3, 0x39),
            (CommandCode::HeadsetMicVolume, 7, 0x37),
            (CommandCode::HeadsetAutoOff, 0, 0xA3),
            (CommandCode::HeadsetAutoOff, 255, 0xA3),
        ] {
            let report = headset_report(&HeadsetSettingCommand { code, value });
            assert_eq!(&report[..3], &[0x00, opcode, value]);
            assert!(report[3..].iter().all(|&b| b == 0));
        }
    }

    #[test]
    fn headset_setting_rejects_out_of_range_and_foreign_codes() {
        let mut buffer = [0u8; HEADSET_REPORT_SIZE];
        for (code, value) in [
            (CommandCode::HeadsetSidetone, 4),
            (CommandCode::HeadsetMicVolume, 8),
            (CommandCode::Apply, 0),
            (CommandCode::HeadsetStatus, 0),
        ] {
            let command = HeadsetSettingCommand { code, value };
            assert!(command.serialize(&mut buffer, HidDeviceType::Headset).is_err());
        }
        let valid = HeadsetSettingCommand {
            code: CommandCode::HeadsetSidetone,
            value: 1,
        };
        assert!(valid.serialize(&mut buffer[..10], HidDeviceType::Headset).is_err());
    }

    #[test]
    fn headset_status_query_is_00_b0_padded() {
        let report = headset_report(&HeadsetStatusQueryCommand);
        assert_eq!(&report[..2], &[0x00, 0xB0]);
        assert!(report[2..].iter().all(|&b| b == 0));
        let mut short = [0u8; 8];
        assert!(
            HeadsetStatusQueryCommand
                .serialize(&mut short, HidDeviceType::Headset)
                .is_err()
        );
    }

    #[test]
    fn headset_opcodes_are_known_command_codes() {
        for (byte, code) in [
            (0x39, CommandCode::HeadsetSidetone),
            (0x37, CommandCode::HeadsetMicVolume),
            (0xA3, CommandCode::HeadsetAutoOff),
            (0xB0, CommandCode::HeadsetStatus),
        ] {
            assert_eq!(CommandCode::from_report_byte(byte), Some(code));
            assert_eq!(code as u8, byte);
            assert!(code.to_string().ends_with("_EXPERIMENTAL"));
        }
    }

    fn mouse_report(command: &dyn HidCommand) -> Vec<u8> {
        let mut buffer = vec![0xAAu8; MOUSE_REPORT_BUFFER_SIZE];
        let size = command.serialize(&mut buffer, HidDeviceType::Mouse).unwrap();
        assert!(
            buffer[size..].iter().all(|&b| b == 0xAA),
            "bytes past the report are untouched"
        );
        buffer.truncate(size);
        buffer
    }

    #[test]
    fn mouse_reports_are_unpadded_and_start_with_the_report_id() {
        assert_eq!(
            mouse_report(&MousePollRateCommand { dongle: false, code: 2 }),
            [0x00, 0x2B, 0x02]
        );
        assert_eq!(mouse_report(&MouseBatteryQueryCommand { dongle: false }), [0x00, 0x92]);
        let dpi = MouseDpiCommand {
            opcode: MouseDpiCommand::OPCODE,
            dongle: false,
            selected: 0,
            code: 0x0168,
            wide: true,
            per_axis: false,
        };
        assert_eq!(mouse_report(&dpi), [0x00, 0x2D, 0x01, 0x00, 0x68, 0x01]);
        let color = MouseColorCommand {
            layout: MouseColorLayout::Mask16,
            dongle: true,
            zone: 9,
            color: Color::new(1, 2, 3),
        };
        let report = mouse_report(&color);
        assert_eq!(report.len(), 34);
        assert_eq!(report[..4], [0x00, 0x61, 0x00, 0x02]);
        assert_eq!(report[31..], [1, 2, 3]);
        assert!(HidDeviceType::Mouse.includes_report_id());
    }

    #[test]
    fn mouse_commands_reject_bad_arguments_and_small_buffers() {
        let mut buffer = [0u8; MOUSE_REPORT_BUFFER_SIZE];
        let poll = MousePollRateCommand { dongle: false, code: 5 };
        assert!(poll.serialize(&mut buffer, HidDeviceType::Mouse).is_err());
        for opcode in [0x11, 0x2B, 0x00] {
            let dpi = MouseDpiCommand {
                opcode,
                dongle: false,
                selected: 0,
                code: 1,
                wide: false,
                per_axis: false,
            };
            assert!(
                dpi.serialize(&mut buffer, HidDeviceType::Mouse).is_err(),
                "{opcode:#04x}"
            );
        }
        let narrow = MouseDpiCommand {
            opcode: MouseDpiCommand::OPCODE,
            dongle: false,
            selected: 0,
            code: 0x100,
            wide: false,
            per_axis: false,
        };
        assert!(narrow.serialize(&mut buffer, HidDeviceType::Mouse).is_err());
        for (layout, zone) in [
            (MouseColorLayout::Mask8, 8),
            (MouseColorLayout::Mask16, 16),
            (MouseColorLayout::Single, 1),
        ] {
            let color = MouseColorCommand {
                layout,
                dongle: false,
                zone,
                color: Color::BLACK,
            };
            assert!(color.serialize(&mut buffer, HidDeviceType::Mouse).is_err());
        }
        assert!(
            MouseBatteryQueryCommand { dongle: false }
                .serialize(&mut buffer[..1], HidDeviceType::Mouse)
                .is_err()
        );
        let last_zone = MouseColorCommand {
            layout: MouseColorLayout::Mask16,
            dongle: false,
            zone: 15,
            color: Color::BLACK,
        };
        assert!(last_zone.serialize(&mut buffer[..20], HidDeviceType::Mouse).is_err());
        assert!(last_zone.serialize(&mut buffer, HidDeviceType::Mouse).is_ok());
    }
}
