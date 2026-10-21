//! On-board profile read, write and payload codec for the Apex 7, on its legacy "macho" protocol.
//!
//! The pure half of the read path: frame builders, reply parsing, payload assembly, struct decode,
//! checksum reporting and JSON. It performs no I/O; the hardware loop is
//! [`crate::devices::keyboards::apex_7`] (`Apex7`'s own read methods).
//!
//! Everything here belongs to this one protocol: the reply framing, resource-buffer and window sizes, the
//! file-id base, payload sizes, schema words and the payload struct offsets all come from this platform's
//! own device specification, and from the read and write captures of an Apex 7. Nothing is generic - the Gen3
//! platform (`0x1628`) reads its on-board profile through a different transport
//! ([`crate::devices::keyboards::apex_pro_tkl_2023_profile`]) and shares none of it. The engine's protocol
//! file (`lisp/apex_7.lisp`) covers the whole legacy platform, so its words are evidence for the
//! `0x1614` / `0x1616` / `0x1618` siblings too; those `.lisp` files are the **decrypted** contents of the
//! `*.edevice` device specifications the Engine ships encrypted. This crate enables the path on the board it
//! measured: discovery builds [`Apex7`](crate::devices::keyboards::apex_7::Apex7) for `0x1612` alone
//! ([`discovery::keyboard_class`](crate::devices::discovery::keyboard_class)).
//!
//! Protocol status: reverse-engineered from the Windows engine's device descriptors plus one Windows
//! USBPcap capture of official GG on an Apex 7 (firmware 4.16.8): 120 pulls of 1024 bytes at file offsets
//! 0..11264 - 100 of them on file `0x0201` (profile slot 0) plus one schema probe - each drained by reads
//! at buffer offsets 0 and 512; 241 payload reads, every one answering with the first payload byte
//! `0x0B`, i.e. schema [`SCHEMA_B`].
//!
//! A full profile slot read completes on hardware under Linux, and the read **changes nothing on the
//! keyboard**: GG's opening chain and closing sequence are not sent. The write plan and the deploy frame have
//! the same status on an Apex 7 (`0x1612`), and [`decode`] and [`encode_json`] are verified against clean and
//! modified profiles. The field meanings inside the OLED and macro blocks stay reverse-engineered.
//!
//! # Wire format
//!
//! All transfers run on the keyboard's vendor collections: usage page `0xffc0` on USB interface 1 (where
//! the capture shows every profile transfer running) and usage page `0xffc1` on interface 4 (the
//! collection the Linux `hidraw` node for the profile protocol sits on). Report numbers are embedded in the
//! payload, so in `hidapi` terms the first byte of every buffer is the report ID.
//!
//! * firmware poll (output report ID `0x00`, 64 data bytes on the wire, `SET_REPORT wValue 0x0200 wIndex
//!   1 wLength 64): `90 <source>`, answered on the interrupt endpoint with the ASCII firmware version
//!   (`34 2e 31 …` = `4.16.8`, see [`parse_firmware_version]). The source byte selects which firmware
//!   answers: `90 00` the keyboard's, `90 01` the LED controller's - both read from an Apex 7, both forms
//!   in the capture. The read sends `90 00` in front of the probe for the image's firmware line; GG's own
//!   session polls both.
//! * pull (`HIDIO`, output report ID `0x00`, 64 bytes on the wire, `SET_REPORT wValue 0x0200 wIndex 1`):
//!   `85 | 00 | file_id u16 LE | offset u32 LE | size u16 LE | 00…` (10 frame bytes, 11-byte report buffer)
//!   — copies `size` bytes of the profile file into the device's resource buffer at buffer offset 0.
//!   **The pull has no answer of its own.** The interrupt endpoint streams the keyboard's ordinary input
//!   reports (endpoint `0x82`, report `0x01`, 63 bytes, all-zero = no keys; median gap 14 ms = the poll
//!   interval) and GG consumes exactly one of them per chunk, keeping **no bytes** of it. All 121 pulls in
//!   the capture are followed by an all-zero `0x01` report 1.7-3.7 ms later, and so are all 72 pushes of
//!   the write runs: the interrupt packet is a pacing token, not a status frame. The read answer is what
//!   carries the data ([`PAYLOAD_READ_LEN`] bytes behind the report-ID byte, the offset of
//!   [`READ_REPLY_FRAMING_LEN`]).
//! * payload read (`HIDFEATURE`, feature report ID `0x00`, 642 bytes on the wire, `SET_REPORT wValue
//!   0x0300 wIndex 1`): `83 | 00 | offset u16 LE | size u16 LE | 00…` (6 frame bytes, a
//!   `crate::devices::hid_reports::ONBOARD_FEATURE_REPORT_SIZE` write buffer, report number byte included)
//!   — returns `size` bytes of the resource buffer. GG reads the answer with `GET_REPORT wValue 0x0300
//!   wIndex 1 wLength 642`, and that answer carries no header: the payload starts at byte 1. The 2-byte
//!   probe of file `0x0201` answers `03 | 0b 00` and the first 512-byte read of the same file answers
//!   `03 | 0b 00 43 6f 6e 66 69 67 20 31 …`, i.e. the schema word at payload offset 0 and the profile name
//!   at payload offset 2. Some firmware echoes the request header instead, with the payload from byte 6;
//!   [`parse_read_reply`] accepts both.
//! * GG's closing sequence (output report ID `0x00`, 64 bytes on the wire, `SET_REPORT wValue 0x0200 wIndex
//!   1 wLength 64`): `69 00…`, `89 <slot + 1> 00…`, `41 00 00…`, `6c 00 00…` - OLED off, deploy that slot,
//!   profile not volatile, close driver mode, with the engine's 250 ms sleep behind the deploy frame.
//!   **The read sends none of them.** They matter as a set because `89` **writes** - the keyboard runs the
//!   slot named - so a tool that sends them changes the keyboard's active profile.
//!
//! The read geometry is not tunable: a profile file is read as [`PULLS_PER_FILE`] resource-buffer fills
//! ([`DEVICE_RESOURCE_BUFFER_LEN`] each); every fill is drained with two [`PAYLOAD_READ_LEN`]-byte payload
//! reads at buffer offsets 0 and 512. The profile files are addressed by id: [`PROFILE_FS_BASE`] plus the
//! profile index (`0..=4`).
//!
//! The first two payload bytes are the profile schema: GG probes a file with one 2-byte pull of the file
//! head and one 2-byte payload read. The engine's decoder lists firmware `4.16.3` as selecting
//! [`SCHEMA_B`] and older firmware [`SCHEMA_A`]; no capture has shown a schema `A` payload - both captures
//! this code rests on are the same firmware and answer schema `B` throughout.
//!
//! The payload starts at file offset 0 with the schema word; the engine's payload structs carry one
//! leading constant id byte that is not part of the file, so a payload read lands at its file offset and
//! the assembled image is [`PROFILE_FILE_BYTES`] bytes with no id byte.

use crate::devices::hid_reports::{
    HidCommand, ONBOARD_FEATURE_REPORT_SIZE, ONBOARD_MAX_READ_LEN, ONBOARD_OUTPUT_REPORT_SIZE, ONBOARD_READ_WIRE_BYTES,
    OnboardBufferWriteCommand, OnboardFileCommitCommand, OnboardFileOpenCommand, OnboardPayloadPushCommand,
};
use crate::devices::keyboards::apex_pro_tkl_2023_profile::profile_checksum;
use crate::devices::keyboards::onboard::OnboardProfileCodec;
// Generic payload helpers live in the interface module; re-exported here because this family's decode,
// encode and read path name them through it.
pub use crate::devices::keyboards::onboard::{hex_dump, hex_of, image_diff, payload_diff, schema_of};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Schema the pre-4.16.3 firmware uses.
pub const SCHEMA_A: u16 = 0x000A;
/// Schema used from firmware `4.16.3` (adds the Rapid Tap block).
pub const SCHEMA_B: u16 = 0x000B;
/// Payload length of schema [`SCHEMA_A`] profiles, in bytes.
pub const PAYLOAD_LEN_A: usize = 11_648;
/// Payload length of schema [`SCHEMA_B`] profiles, in bytes.
pub const PAYLOAD_LEN_B: usize = 11_696;
/// Firmware the Windows engine decoder lists as switching the on-board profile schema to [`SCHEMA_B`].
/// Documentation only: the read takes its schema from the word it reads at payload offset 0 and never
/// compares a firmware version with this.
pub const SCHEMA_B_FIRMWARE_VERSION: &str = "4.16.3";
/// Size of the device's on-board resource buffer, in bytes - the fixed fill size of every pull. A fact
/// of the device, not a size this code can request differently.
pub const DEVICE_RESOURCE_BUFFER_LEN: usize = 1024;
/// Size of one payload-read window in bytes: the fixed `size` of every payload read, the window GG's
/// read specifiers use. A protocol fact, not a read length to tune.
pub const PAYLOAD_READ_LEN: usize = 512;
/// Pulls per profile file, the fixed pull count of the read: [`PULLS_PER_FILE`] fills of
/// [`DEVICE_RESOURCE_BUFFER_LEN`] bytes cover both payload lengths. The read loop runs exactly this many
/// pulls; there is no chunk-count option.
pub const PULLS_PER_FILE: usize = 12;
/// File id of profile slot 0; slot `n` is this plus `n`.
pub const PROFILE_FS_BASE: u16 = 0x0201;
/// Highest profile slot the device offers (`0..=4`), a fixed slot count of the platform. GG names the
/// same slots `Config 1` .. `Config 5`, and `ssgg onboard` takes that number: `--profile N` reads slot
/// `N - 1`, the file it pulls as `0x020N`.
pub const MAX_PROFILE_INDEX: u8 = 4;
/// File id of the per-profile lighting config of slot 0; slot `n` is this plus `n`.
///
/// Presumed lighting from the engine's protocol words, **not confirmed on hardware**: no `0x030N` file has
/// ever been read from or written to a keyboard, and nothing in ssgg addresses these ids - every frame the
/// read and the write paths run on hardware is on a [`PROFILE_FS_BASE`] (`0x020N`) profile file.
pub const LIGHTING_FS_BASE: u16 = 0x0301;
/// Macro data block inside a profile payload, in bytes.
pub const MACRO_DATA_LEN: usize = 9600;

/// Command byte of the push (profile write); defined for completeness, nothing here sends it. The
/// read path has no command bytes of its own: they are
/// [`crate::devices::hid_reports::CommandCode`] variants and every frame is built by a typed command in
/// [`crate::devices::hid_reports`] (see [`crate::devices::hid_reports::OnboardPayloadPullCommand`]).
pub const CMD_PUSH_PAYLOAD: u8 = 0x05;
/// Report ID of every request frame.
pub const REQUEST_REPORT_ID: u8 = 0x00;
/// Report ID of a payload-read reply.
pub const READ_REPLY_REPORT_ID: u8 = 0x83;
/// Command byte of a pull, which is what the capture kit recorded in the frames it labels pull
/// replies; the device answers a pull with its ordinary `0x01` input report, not with `0x85`.
pub const PULL_REPLY_REPORT_ID: u8 = 0x85;
/// Report ID of a lighting-config payload-read reply (`03`, the id the capture shows behind the
/// `0x030N` file ids; that those files are the lighting config is a presumption, see [`LIGHTING_FS_BASE`]).
pub const LIGHTING_READ_REPLY_REPORT_ID: u8 = 0x03;
/// Buffer size to ask the transport for when reading a payload read back: [`ONBOARD_READ_WIRE_BYTES`] (642).
/// The read side sends this number to the device as `wLength` verbatim - the capture's `GET_REPORT wValue
/// 0x0300 wIndex 1 wLength 642`; a 643-byte ask never completes. The kernel lands the device's report-ID
/// byte in buffer[0], so the reply arrives as `[report ID][641 bytes]`.
pub const READ_REPORT_LEN: usize = ONBOARD_READ_WIRE_BYTES;
/// Data-stage length of the 64-byte output reports of this protocol (`SET_REPORT wValue 0x0200
/// wIndex 1 wLength 64`): the firmware poll and every pull. The report number the transport prefixes
/// is not part of it.
pub const OUTPUT_REPORT_DATA_LEN: usize = ONBOARD_OUTPUT_REPORT_SIZE - 1;
/// Input-report size GG reads on the interrupt endpoint: its `read-chunk HIDIO 65` is a report-ID slot
/// plus 64 data bytes. The same read serves the pacing packet after a pull and the firmware poll's answer
/// (`0x6E` with the ASCII version), so this is the length the read
/// loop asks hidapi for. A 64-byte ask works but truncates the last byte of a full 65-byte answer.
pub const GG_INPUT_READ_BYTES: usize = OUTPUT_REPORT_DATA_LEN + 1;
/// Header of a header-echoing payload-read reply, report ID byte excluded: `reserved, offset u16, size u16`.
pub const READ_REPLY_HEADER_LEN: usize = 5;
/// Framing bytes between the report-ID byte and the payload of a payload-read reply (`GET_REPORT
/// wValue 0x0300 wIndex 1 wLength 642`): the report-ID byte and nothing else, so the framing length is 0.
/// All 241 replies of the capture answer `03` as their first byte, with the payload behind it. There is no
/// CLI knob for it: a keyboard that anchors its payload elsewhere is a change of this constant.
pub const READ_REPLY_FRAMING_LEN: usize = 0;
/// Settle in front of the profile **deploy** frame (`0x89`), in milliseconds.
///
/// Measured on hardware on an Apex 7 (`0x1612`) - the deploy frame sent straight after the commit (`eb 01`) can
/// do nothing, and waiting **before** it is what makes the keyboard switch slots; how much the device needs
/// is not measured. 250 ms is the engine's own number: its `load-profile-with-pause` sleeps 250 ms
/// **behind** the deploy frame, and its commit path ends with an **input read** that blocks until the
/// keyboard answers (`base_apex_legacy.lisp`).
pub const DEPLOY_SETTLE_MS: u64 = 250;

/// Median gap between two device replies during a GG profile read in the reference capture, in
/// milliseconds: the fixed pacing between our own transfers. (GG's opening-chain frames are 11 ms apart;
/// this read replays none of them.)
pub const GG_GAP_MS: u64 = 5;

/// Firmware version a [`crate::devices::hid_reports::CommandCode::OnboardFirmwareVersion`] answer carries: the printable
/// ASCII run
/// from the first byte, which the capture shows NUL padded (`34 2e 31 …` = `4.16.8`).
/// Returns `None` when the answer starts with non-printable bytes, which is how a keyboard that frames
/// its answer with a header, or does not answer the poll at all, shows up.
#[must_use]
pub fn parse_firmware_version(reply: &[u8]) -> Option<String> {
    let text = reply
        .iter()
        .take_while(|b| **b >= 0x20 && **b < 0x7F)
        .map(|&b| char::from(b))
        .collect::<String>();
    (!text.is_empty()).then_some(text)
}

/// Largest payload a payload-read reply can carry: the 642 bytes the reply delivers, minus the report-ID
/// byte the kernel puts in buffer[0] and minus the 5-byte header a header-echoing reply prepends
/// (`03 01 01 00 01`). Payload-only replies (the capture's shape) start directly behind the report-ID byte,
/// so 636 is the ceiling both shapes agree on — and it equals [`ONBOARD_MAX_READ_LEN`], the cap the read
/// frame builder enforces. A ceiling derived from the wire, not a tunable.
pub const MAX_READ_LEN: usize = READ_REPORT_LEN - 1 - READ_REPLY_HEADER_LEN;

/// The read frame may never ask for more than a reply can carry.
const _: () = assert!(MAX_READ_LEN == ONBOARD_MAX_READ_LEN);

/// File bytes one read covers: [`PULLS_PER_FILE`] pulls of [`DEVICE_RESOURCE_BUFFER_LEN`] bytes, which
/// covers both payload lengths ([`PAYLOAD_LEN_A`] and [`PAYLOAD_LEN_B`]). The read always covers exactly
/// this many bytes: the shorter image would be a partial profile, the longer one would need slots that
/// do not exist.
pub const PROFILE_FILE_BYTES: usize = PULLS_PER_FILE * DEVICE_RESOURCE_BUFFER_LEN;

/// Payload reads per pull: one pull fills [`DEVICE_RESOURCE_BUFFER_LEN`], each read drains
/// [`PAYLOAD_READ_LEN`] of it.
pub const READS_PER_PULL: usize = DEVICE_RESOURCE_BUFFER_LEN / PAYLOAD_READ_LEN;

// The write geometry, from the engine's write API and the Apex 7 write capture, which
// reproduces it frame for frame (descriptor citations live in the write doc, not here):
//   open the profile file (0x88) -> for every window of the profile region, two 512-byte buffer writes
//   (offsets 0 and 512) then one push of the full buffer (0x05) -> commit the slot (0xEB).

/// Bytes the engine's profile file is (`profile_size` in the family): `0xA000`, which is what the write
/// API pushes and what the capture's 40 windows add up to. It is larger than any schema's payload, and the
/// engine pads the difference with zero bytes.
pub const PROFILE_REGION_BYTES: usize = 40_960;
/// Bytes one write frame carries into the resource buffer: the engine's profile chunk size (512), and 512
/// in every write frame of the capture.
pub const WRITE_CHUNK_BYTES: usize = 512;
/// Bytes one push copies: the whole resource buffer (1024), 1024 in every push of the capture.
pub const WRITE_PUSH_BYTES: usize = DEVICE_RESOURCE_BUFFER_LEN;
/// Write windows in the profile region: profile size divided by the resource buffer, 40 in the capture.
pub const WRITE_WINDOW_COUNT: usize = PROFILE_REGION_BYTES / WRITE_PUSH_BYTES;

/// One frame of the on-board profile write, in the order the engine sends them. The shape is the write
/// API's and the byte pattern the Apex 7 write capture shows. Confirmed on hardware on an Apex 7 (`0x1612`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnboardWriteFrame {
    /// `88 | 00 | file id u16 LE` in the 64-byte output report: open the profile file before pushing it.
    FileOpen { fs_id: u16 },
    /// `03 | 00 | buffer offset u16 LE | size u16 LE | payload bytes` in the 643-byte feature report: part
    /// of the payload, placed in the keyboard's resource buffer. No reply of its own.
    BufferWrite { buffer_offset: u16, data: Vec<u8> },
    /// `05 | 00 | file id u16 LE | file offset u32 LE | size u16 LE` in the 64-byte output report: copy the
    /// resource buffer into the profile file. The only write-side frame with a reply of its own.
    Push { fs_id: u16, file_offset: u32, size: u16 },
    /// `eb | 1-based profile id` in the 64-byte output report: the last frame of the write.
    Commit { profile: u8 },
}

/// Which HID report a write frame travels in, the two shapes the capture separates cleanly:
/// `wValue 0x0200 wLength 64` (output) and `wValue 0x0300 wLength 642` (feature).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardWriteReportKind {
    /// 64-byte output report.
    Output,
    /// 643-byte feature report.
    Feature,
}

impl OnboardWriteFrame {
    /// The report this frame travels in.
    #[must_use]
    pub fn report_kind(&self) -> OnboardWriteReportKind {
        match self {
            Self::FileOpen { .. } | Self::Push { .. } | Self::Commit { .. } => OnboardWriteReportKind::Output,
            Self::BufferWrite { .. } => OnboardWriteReportKind::Feature,
        }
    }

    /// Wire bytes of the frame: the data stage the capture shows, i.e. the report buffer without its report
    /// number.
    #[must_use]
    pub fn wire_len(&self) -> usize {
        match self.report_kind() {
            OnboardWriteReportKind::Output => ONBOARD_OUTPUT_REPORT_SIZE - 1,
            OnboardWriteReportKind::Feature => ONBOARD_FEATURE_REPORT_SIZE - 1,
        }
    }

    /// Serialize into the report buffer of this frame's kind: [`ONBOARD_OUTPUT_REPORT_SIZE`] (65) or
    /// [`ONBOARD_FEATURE_REPORT_SIZE`] (643), each `[report number][data]` with report number 0.
    ///
    /// # Errors
    ///
    /// Propagates the typed command's own validation.
    pub fn serialize(&self, buffer: &mut [u8]) -> Result<usize> {
        let device = crate::devices::hid_reports::HidDeviceType::Keyboard;
        match self {
            Self::FileOpen { fs_id } => OnboardFileOpenCommand { fs_id: *fs_id }.serialize(buffer, device),
            Self::BufferWrite { buffer_offset, data } => OnboardBufferWriteCommand {
                buffer_offset: *buffer_offset,
                data,
            }
            .serialize(buffer, device),
            Self::Push {
                fs_id,
                file_offset,
                size,
            } => OnboardPayloadPushCommand {
                fs_id: *fs_id,
                file_offset: *file_offset,
                size: *size,
            }
            .serialize(buffer, device),
            Self::Commit { profile } => OnboardFileCommitCommand { profile: *profile }.serialize(buffer, device),
        }
    }

    /// The line `ssgg onboard write --dry-run` prints for this frame: the frame's own numbers and the wire
    /// length it goes out in.
    #[must_use]
    pub fn line(&self) -> String {
        match self {
            Self::FileOpen { fs_id } => format!("88 00 file {fs_id:04X}      output {:02}", self.wire_len()),
            Self::BufferWrite { buffer_offset, data } => format!(
                "03 00 buffer {:>4} size {:>4}  feature {:02}  head {}",
                buffer_offset,
                data.len(),
                self.wire_len(),
                crate::devices::keyboards::onboard::hex_of(&data[..data.len().min(8)])
            ),
            Self::Push {
                fs_id,
                file_offset,
                size,
            } => format!(
                "05 00 file {fs_id:04X} offset {:>5} size {:>4}  output {:02}",
                file_offset,
                size,
                self.wire_len()
            ),
            Self::Commit { profile } => format!(
                "eb {:02X} (slot {})      output {:02}",
                profile + 1,
                profile,
                self.wire_len()
            ),
        }
    }
}

/// The frames `ssgg` sends for one on-board profile write, in wire order: built by [`write_plan`], printed
/// by `ssgg onboard write --dry-run`, sent by a keyboard's write path.
///
/// The geometry is capture-confirmed and confirmed on hardware on an Apex 7 (`0x1612`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnboardWritePlan {
    /// Profile slot, 0-based.
    pub profile: u8,
    /// Profile file the plan writes: `PROFILE_FS_BASE + profile`.
    pub fs_id: u16,
    /// Payload bytes the plan carries, the payload's checksum word included.
    pub payload_len: usize,
    /// Bytes the plan pushes: the payload padded out to the profile region, as the engine does.
    pub region_bytes: usize,
    /// The checksum word the payload carries at its end (its last four bytes, little endian).
    pub checksum: u32,
    /// The frames, in wire order.
    pub frames: Vec<OnboardWriteFrame>,
}

impl OnboardWritePlan {
    /// Count of the frames that push the resource buffer into the file.
    #[must_use]
    pub fn push_count(&self) -> usize {
        self.frames
            .iter()
            .filter(|frame| matches!(frame, OnboardWriteFrame::Push { .. }))
            .count()
    }
}

/// Build one on-board profile write: open the profile file (0x88), fill the keyboard's resource buffer in
/// 512-byte halves of each window (0x03), push each window into the file (0x05), commit the slot (0xEB).
/// The engine pads the payload out to the whole profile region with zeros, so a write covers the region,
/// not only the payload: the capture shows all 40 windows, the last 28 of them all zero.
///
/// The deploy frame (0x89) is not part of the plan; the write path sends it right after the commit, and
/// `ssgg onboard load` sends it on its own.
///
/// Geometry capture-confirmed on Apex 7, and sent on an Apex 7 by `ssgg` (2026-10-20).
///
/// A write pads the payload out to the whole region; a slot is verified from the device by reading it back
/// with the read path and diffing the payload - an on-board write has no read-back frame of its own.
///
/// # Errors
///
/// Returns [`crate::error::Error::InvalidConfig`] for a slot outside the five the engine accepts, an empty
/// payload, a payload longer than the region, a payload of an unknown schema or with a wrong stored checksum,
/// a region that is not a whole number of push windows, or a
/// chunk pair that does not fill a window.
pub fn write_plan(profile: u8, payload: &[u8]) -> Result<OnboardWritePlan> {
    if profile > MAX_PROFILE_INDEX {
        return Err(Error::InvalidConfig(format!(
            "profile slot {profile} out of range: the engine accepts 0..{}",
            MAX_PROFILE_INDEX
        )));
    }
    if payload.is_empty() {
        return Err(Error::InvalidConfig(
            "an on-board profile write carries a payload: encode a profile first".to_string(),
        ));
    }
    // A `--bin` payload is raw file bytes: refuse anything that is not a known-schema payload whose stored
    // checksum matches, before it is framed for the board.
    let decoded = decode(payload)?;
    if !decoded.crc_ok {
        return Err(Error::InvalidConfig(
            "payload checksum word does not match its bytes: refusing to write it".to_string(),
        ));
    }
    if payload.len() > PROFILE_REGION_BYTES {
        return Err(Error::InvalidConfig(format!(
            "payload is {} bytes, the profile region holds {PROFILE_REGION_BYTES}",
            payload.len()
        )));
    }
    if WRITE_WINDOW_COUNT * WRITE_PUSH_BYTES != PROFILE_REGION_BYTES {
        return Err(Error::InvalidConfig(
            "the profile region is not a whole number of push windows".to_string(),
        ));
    }
    if 2 * WRITE_CHUNK_BYTES != WRITE_PUSH_BYTES {
        return Err(Error::InvalidConfig(
            "one window is not exactly two write chunks: the engine's write API pairs a buffer half at \
             offset 0 with one at the chunk size"
                .to_string(),
        ));
    }

    let fs_id = PROFILE_FS_BASE.checked_add(u16::from(profile)).ok_or_else(|| {
        Error::InvalidConfig(format!(
            "profile file id {} + {profile} does not fit u16",
            PROFILE_FS_BASE
        ))
    })?;
    // The schema's own checksum word: a read image is longer than its payload, so the last four bytes of
    // `payload` are not it.
    let checksum = decoded.crc_stored;
    let mut image = payload.to_vec();
    image.resize(PROFILE_REGION_BYTES, 0);

    let mut frames = Vec::with_capacity(2 + WRITE_WINDOW_COUNT * 3);
    frames.push(OnboardWriteFrame::FileOpen { fs_id });
    for window in 0..WRITE_WINDOW_COUNT {
        let base = window * WRITE_PUSH_BYTES;
        let file_offset = u32::try_from(base)
            .map_err(|e| Error::InvalidConfig(format!("profile file offset {base} does not fit u32: {e}")))?;
        for half in 0..2 {
            let start = base + half * WRITE_CHUNK_BYTES;
            let buffer_offset = u16::try_from(half * WRITE_CHUNK_BYTES).map_err(|e| {
                Error::InvalidConfig(format!(
                    "buffer offset {} does not fit u16: {e}",
                    half * WRITE_CHUNK_BYTES
                ))
            })?;
            frames.push(OnboardWriteFrame::BufferWrite {
                buffer_offset,
                data: image[start..start + WRITE_CHUNK_BYTES].to_vec(),
            });
        }
        frames.push(OnboardWriteFrame::Push {
            fs_id,
            file_offset,
            size: u16::try_from(WRITE_PUSH_BYTES)
                .map_err(|e| Error::InvalidConfig(format!("push size {WRITE_PUSH_BYTES} does not fit u16: {e}")))?,
        });
    }
    frames.push(OnboardWriteFrame::Commit { profile });

    Ok(OnboardWritePlan {
        profile,
        fs_id,
        payload_len: payload.len(),
        region_bytes: PROFILE_REGION_BYTES,
        checksum,
        frames,
    })
}

/// Counts from one on-board profile write run, the summary `ssgg onboard write` prints after a finished
/// write. A keyboard's write path fills these from the frames it sent and the answers it read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WriteStats {
    /// Frames put on the wire.
    pub frames_sent: usize,
    /// Payload bytes those frames carried.
    pub bytes_written: usize,
    /// Resource-buffer pushes completed.
    pub pushes: usize,
    /// Write-side replies read back (`88 01 …` and one per push).
    pub acks: usize,
}

/// Tally the frames of a write plan: the counts a finished write of that plan reports, counting the answers
/// the capture shows (`88 01 …` for the open, `05 01 …` for every push, none for the writes and the commit).
#[must_use]
pub fn write_stats(plan: &OnboardWritePlan) -> WriteStats {
    let mut stats = WriteStats::default();
    for frame in &plan.frames {
        stats.frames_sent += 1;
        match frame {
            OnboardWriteFrame::FileOpen { .. } => stats.acks += 1,
            OnboardWriteFrame::BufferWrite { data, .. } => stats.bytes_written += data.len(),
            OnboardWriteFrame::Push { .. } => {
                stats.pushes += 1;
                stats.acks += 1;
            }
            OnboardWriteFrame::Commit { .. } => {}
        }
    }
    stats
}

/// The lines `ssgg onboard write --dry-run` prints: the geometry, every frame in wire order, and the deploy
/// frame the write path sends after it. No keyboard is involved.
#[must_use]
pub fn write_plan_lines(plan: &OnboardWritePlan) -> Vec<String> {
    let mut lines = vec![
        format!(
            "write plan for slot {} (profile file {:04X}): {} payload bytes padded to the {}-byte profile region",
            plan.profile, plan.fs_id, plan.payload_len, plan.region_bytes
        ),
        format!("payload checksum word: {:08X}", plan.checksum),
        format!(
            "{} frames: 1 open, {} windows of 2 writes + 1 push, 1 commit",
            plan.frames.len(),
            plan.push_count()
        ),
    ];
    lines.extend(
        plan.frames
            .iter()
            .enumerate()
            .map(|(n, frame)| format!("  frame {:>3}  {}", n + 1, frame.line())),
    );
    lines.push(format!(
        "then 89 {:02X} (deploy slot {}) after a {DEPLOY_SETTLE_MS} ms settle - the engine sleeps 250 ms behind \
         this frame and its commit path ends with an input read; the settle in front of the frame is the hardware \
         fix of 2026-10-19: sent straight after the commit, the frame can do nothing",
        plan.profile + 1,
        plan.profile
    ));
    lines
}

/// The fixed read must cover both payload lengths, or the assembled image would be a partial profile.
const _: () = assert!(
    PROFILE_FILE_BYTES >= PAYLOAD_LEN_B,
    "the fixed pull loop must cover the longer payload"
);

/// Profile file id of slot `profile`: [`PROFILE_FS_BASE`] plus the slot index.
#[must_use]
pub fn profile_file_id(profile: u8) -> u16 {
    PROFILE_FS_BASE.wrapping_add(u16::from(profile))
}

/// Reject a slot index this read does not support.
///
/// # Errors
///
/// [`Error::InvalidConfig`] when `profile` is above [`MAX_PROFILE_INDEX`].
pub fn validate_profile(profile: u8) -> Result<()> {
    if profile > MAX_PROFILE_INDEX {
        return Err(Error::InvalidConfig(format!(
            "profile slot {profile} is above the highest slot {MAX_PROFILE_INDEX}"
        )));
    }
    Ok(())
}

/// Parsed header of a payload-read reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadReply {
    /// Report ID the reply arrived on.
    pub report_id: u8,
    /// Reserved header byte behind the report ID.
    pub reserved: u8,
    /// True when the reply echoed the request header (see [`parse_read_reply`]).
    pub header: bool,
    /// Buffer offset the keyboard echoes.
    pub offset: u16,
    /// Size the keyboard echoes.
    pub size: u16,
    /// Payload bytes extracted from the reply.
    pub data: Vec<u8>,
    /// True when every payload byte is zero.
    pub all_zero: bool,
    /// True when the payload starts with four `FF` bytes: the reference implementation treats such a page as
    /// garbage and replaces it with zeroes.
    pub all_ff: bool,
}

/// Parse a payload-read reply (`reply` starts with the report ID byte) and extract its payload.
///
/// `requested_size` is the size that was requested; the payload is truncated to it when the report
/// carries more.
///
/// The reference capture shows the keyboard answering a payload read in two shapes, both accepted
/// here. On the interrupt endpoint the reply echoes the request header and the payload starts at
/// byte 6; on the control endpoint (`GET_REPORT 0x0300`, 642 bytes on interface 1) the answer carries
/// no header at all, so the payload starts directly behind the report-ID byte. The reply header of
/// the first shape is `[0x83][reserved][buffer offset u16 LE][size u16 LE]`.
pub fn parse_read_reply(reply: &[u8], requested_size: u16) -> Result<ReadReply> {
    parse_read_reply_at(reply, requested_size, READ_REPLY_FRAMING_LEN)
}

/// [`parse_read_reply`] with an explicit payload anchor: the payload starts at
/// `1 + framing_len` (byte 0 is the report ID). The capture's replies put the payload behind the
/// report-ID byte alone; a keyboard that answers with a header needs `5`, one that answers with 20
/// framing bytes needs `20`.
///
/// # Errors
///
/// Returns [`Error::Profile`] when the reply is shorter than its report-ID and reserved bytes.
pub fn parse_read_reply_at(reply: &[u8], requested_size: u16, framing_len: usize) -> Result<ReadReply> {
    if reply.len() < 2 {
        return Err(Error::Profile(format!(
            "payload-read reply is {} byte(s), shorter than its report-ID and reserved bytes",
            reply.len()
        )));
    }
    let header = reply.len() > READ_REPLY_HEADER_LEN && reply[0] == READ_REPLY_REPORT_ID && reply[1] == 0;
    let (offset, size, payload) = if header {
        (
            u16::from_le_bytes([reply[2], reply[3]]),
            u16::from_le_bytes([reply[4], reply[5]]),
            &reply[1 + READ_REPLY_HEADER_LEN..],
        )
    } else if reply.len() > 1 + framing_len {
        (0, requested_size, &reply[1 + framing_len..])
    } else {
        (0, requested_size, &reply[reply.len().saturating_sub(1)..])
    };
    let limit = usize::from(requested_size).min(MAX_READ_LEN);
    let data = payload.iter().copied().take(limit).collect::<Vec<u8>>();
    let all_zero = data.iter().all(|b| *b == 0);
    let all_ff = data.len() >= 4 && data[..4].iter().all(|b| *b == 0xFF);
    Ok(ReadReply {
        report_id: reply[0],
        reserved: reply[1],
        header,
        offset,
        size,
        data,
        all_zero,
        all_ff,
    })
}

/// One pull and the payload reads drained from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChunkRecord {
    /// Pull index.
    pub index: usize,
    /// File id the pull addressed.
    pub fs_id: u16,
    /// File offset the pull filled the resource buffer from.
    pub file_offset: u32,
    /// Report ID the packet after the pull arrived on (`0x01` on an idle keyboard).
    pub ack_report_id: u8,
    /// First bytes of that packet, hex; content is discarded, GG's read spec takes zero bytes from it.
    pub ack_hex: String,
    /// Payload reads drained from this pull.
    pub reads: Vec<ReadReply>,
}

impl ChunkRecord {
    /// Payload bytes collected by this pull.
    pub fn data_len(&self) -> usize {
        self.reads.iter().map(|r| r.data.len()).sum()
    }

    /// True when every payload read of this pull returned only zeros.
    pub fn all_zero(&self) -> bool {
        !self.reads.is_empty() && self.reads.iter().all(|r| r.all_zero)
    }
}

/// Header disagreement between a request and its reply.
pub fn reply_mismatch(reply: &ReadReply, wanted_offset: u16, wanted_size: u16) -> Option<String> {
    if reply.report_id != READ_REPLY_REPORT_ID {
        return Some(format!(
            "reply arrived on report 0x{:02X}, expected 0x{:02X}",
            reply.report_id, READ_REPLY_REPORT_ID
        ));
    }
    if reply.offset != wanted_offset {
        return Some(format!(
            "reply echoes offset 0x{:04X}, the request asked for 0x{:04X}",
            reply.offset, wanted_offset
        ));
    }
    if reply.size != wanted_size {
        return Some(format!(
            "reply echoes size {}, the request asked for {}",
            reply.size, wanted_size
        ));
    }
    None
}

/// Aggregated statistics of one profile read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadStats {
    /// Pulls collected.
    pub chunks_read: usize,
    /// Payload reads collected.
    pub reads_read: usize,
    /// Data bytes in the assembled image.
    pub data_len: usize,
    /// Reads whose reply header did not echo the request.
    pub header_mismatches: usize,
    /// Reads whose payload was entirely zero.
    pub zero_reads: usize,
    /// Pulls whose payload was entirely zero.
    pub zero_chunks: usize,
    /// Reads that returned fewer bytes than requested.
    pub short_reads: usize,
    /// Reads whose payload started with four `FF` bytes and was stored as zeroes.
    pub garbage_reads: usize,
    /// File bytes covered by the pulls.
    pub covered_len: usize,
}

/// Assembled profile data with its per-pull records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProfileImage {
    /// Product ID of the keyboard the read ran on: the model selector. `None` only for an image assembled
    /// without a device. The three identity fields sit above the profile fields: product ID, model name,
    /// firmware version say *which keyboard*, and only then do slot, payload and stats say *what it holds*.
    pub product_id: Option<u16>,
    /// Model name this crate lists for that product ID ([`crate::devices::device_name_from_product_id`]).
    /// The name is derived from the number, so a reader that trusts the number can re-derive it; a model
    /// outside this crate's registry is labelled by its name.
    pub product: Option<String>,
    /// Firmware version the read's `90 00` poll answered with (`None` when the poll was refused or the
    /// keyboard answered nothing: the version is informational, the read does not depend on it).
    pub firmware: Option<String>,
    /// Profile slot the read ran on.
    pub profile_slot: u8,
    /// Per-pull records, in collection order.
    pub chunks: Vec<ChunkRecord>,
    /// Profile file bytes, reads concatenated in pull order.
    pub data: Vec<u8>,
    /// Aggregated reply statistics.
    pub stats: ReadStats,
    /// The image bytes **behind** the payload, as a block summary, in the read's record. The read fills the
    /// whole profile region ([`PROFILE_FILE_BYTES`]) and only its head is the payload; the rest is recorded
    /// and decoded by nothing. `None` skips the field (an image no longer than the payload).
    /// `[EXPERIMENTAL]`: no byte of this region is understood.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<MacroSummary>,
}

impl ProfileImage {
    /// Assemble collected records into the profile image and tally reply statistics.
    ///
    /// Reads are scattered by their file offset; reads past [`PROFILE_FILE_BYTES`] are kept in the
    /// records and dropped from the assembled data.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidConfig`] when `profile_slot` is out of range.
    pub fn assemble(profile_slot: u8, chunks: Vec<ChunkRecord>) -> Result<Self> {
        validate_profile(profile_slot)?;

        let mut data = vec![0u8; PROFILE_FILE_BYTES];
        let mut reads_read = 0usize;
        let mut header_mismatches = 0usize;
        let mut zero_reads = 0usize;
        let mut zero_chunks = 0usize;
        let mut short_reads = 0usize;
        let mut garbage_reads = 0usize;

        for record in &chunks {
            if record.all_zero() {
                zero_chunks += 1;
            }
            let mut cursor = record.file_offset as usize;
            for reply in &record.reads {
                reads_read += 1;
                if reply_mismatch(
                    reply,
                    (cursor - record.file_offset as usize) as u16,
                    PAYLOAD_READ_LEN as u16,
                )
                .is_some()
                {
                    header_mismatches += 1;
                }
                if reply.all_zero {
                    zero_reads += 1;
                }
                if reply.data.len() < PAYLOAD_READ_LEN {
                    short_reads += 1;
                }
                if reply.data.is_empty() {
                    continue;
                }
                if cursor >= PROFILE_FILE_BYTES {
                    continue;
                }
                let end = cursor + reply.data.len();
                if reply.all_ff {
                    // A page whose first four bytes are FF is garbage from the device's point of view; the
                    // reference implementation replaces it with zeroes and so do we. Only this page: the
                    // next read of the pull is a page of its own and still lands at its own offset.
                    garbage_reads += 1;
                    let page = reply.data.len().min(PROFILE_FILE_BYTES - cursor);
                    data[cursor..cursor + page].fill(0);
                    cursor += page;
                } else if end > PROFILE_FILE_BYTES {
                    let room = PROFILE_FILE_BYTES.saturating_sub(cursor);
                    data[cursor..cursor + room].copy_from_slice(&reply.data[..room]);
                    cursor += room;
                } else {
                    data[cursor..end].copy_from_slice(&reply.data);
                    cursor = end;
                }
            }
        }

        let covered_len = chunks
            .iter()
            .map(|r| r.file_offset as usize + r.data_len())
            .max()
            .unwrap_or(0);

        Ok(Self {
            profile_slot,
            region: extra_region(&data),
            stats: ReadStats {
                chunks_read: chunks.len(),
                reads_read,
                data_len: data.len(),
                header_mismatches,
                zero_reads,
                zero_chunks,
                short_reads,
                garbage_reads,
                covered_len,
            },
            chunks,
            data,
            product_id: None,
            product: None,
            firmware: None,
        })
    }

    /// Stamp a decode report with this image's keyboard identity: product ID, model name, firmware
    /// version. A read's report names the keyboard it ran on; a decode of a file names none, and a
    /// report assembled from bytes leaves these fields absent.
    pub fn stamp_identity(&self, profile: &mut OnboardProfile) {
        profile.product_id = self.product_id;
        profile.product = self.product.clone();
        profile.firmware = self.firmware.clone();
        profile.slot = Some(self.profile_slot);
    }

    /// Record the firmware version the read's `90 00` poll answered with.
    #[must_use]
    pub fn with_firmware(mut self, firmware: Option<String>) -> Self {
        self.firmware = firmware;
        self
    }

    /// Schema word of the assembled profile.
    pub fn schema(&self) -> u16 {
        schema_of(&self.data)
    }

    /// Offset of the first byte that differs from another read of the same profile.
    pub fn diff(&self, other: &Self) -> Option<usize> {
        image_diff(self, other)
    }
}

/// Payload length for a schema word; unknown schemas report the length they cannot be decoded at.
pub fn payload_len_for_schema(schema: u16) -> Result<usize> {
    match schema {
        SCHEMA_A => Ok(PAYLOAD_LEN_A),
        SCHEMA_B => Ok(PAYLOAD_LEN_B),
        0 => Err(Error::Profile(
            "this slot has no saved profile: the schema word is 0x0000 (GG writes 0x000A / 0x000B)".to_string(),
        )),
        other => Err(Error::Profile(format!(
            "profile schema 0x{other:04X} is not one of the supported schemas 0x{SCHEMA_A:04X} / 0x{SCHEMA_B:04X}"
        ))),
    }
}

/// Offset of the trailing checksum word and the length of the region it covers.
pub fn checksum_offset(payload_len: usize) -> usize {
    payload_len - 4
}

/// Compute the profile checksum the way the 2023-family profile blob is checksummed: STM32 hardware
/// CRC-32 over the payload prefix read as little-endian 32-bit words. Assumed, not verified on Apex 7.
pub fn compute_checksum(data: &[u8]) -> u32 {
    profile_checksum(data)
}

/// One `key_mapping` entry: a function selector plus its four key codes.
///
/// The binding classification is what makes a stock profile readable: GG's factory defaults bind
/// every key to itself (`0x51` + its own HID code), and GG's own writes leave unused slots as
/// `0x00 00 00 00 00`. Both mean "this key types normally", so they are reported as `self` and
/// `none`; only anything else is a `custom` binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyMapping {
    /// Mapping slot this entry belongs to (table index = GG's mapping offset).
    pub offset: usize,
    /// Key this slot belongs to, from [`OFFSET_TO_HID`] (`null` = the device has no key at that slot).
    pub hid_code: Option<u8>,
    /// Function selector (`0x00` = key deactivated, `0x71` = macro, `0x62` = META, …).
    pub function: u8,
    /// Function selector as a name, for the human-readable summary only.
    pub function_name: String,
    /// `self` = bound to its own key, `none` = all-zero slot, `custom` = a real remap.
    pub binding: &'static str,
    /// Four raw key-code bytes, hex.
    pub key_codes: String,
    /// For a `0x71` macro binding, the binding index this slot carries; `None` for every other
    /// function: `key_codes[2]`, the third code byte. The record it binds is the one at
    /// `index / MACRO_INDEX_STRIDE` ([`MACRO_INDEX_STRIDE`] carries the measurement), so a binding of 7
    /// and a binding of 11 both bind the record at ordinal 1, and an index whose ordinal is past the
    /// record list is the deleted-macro state - GG deletes the macro and leaves the key bound.
    /// `[EXPERIMENTAL]`: the record list has no stride and no length word; a block the walker cannot walk
    /// is refused by [`encode_json`] instead of written back.
    pub macro_index: Option<u8>,
}

/// A lighting colour of the on-board profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Color {
    /// Red byte.
    pub r: u8,
    /// Green byte.
    pub g: u8,
    /// Blue byte.
    pub b: u8,
}

/// Lighting block of the on-board profile (57 bytes at payload offset 27).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LightingProfile {
    /// Six raw OLED illumination option bytes, hex.
    pub oled_illumination_options: String,
    /// Colour of the lock-key cluster.
    pub lock_key_color: Color,
    /// Colour of the META-mapped cluster.
    pub meta_key_color: Color,
    /// Colour of the disabled Win key.
    pub disabled_win_key_color: Color,
    /// 32-byte META mapping mask, hex.
    pub meta_mapped_elements: String,
    /// Three lock-key HID codes, hex.
    pub lock_key_hid_codes: String,
    /// Win key HID code.
    pub win_key_hid_code: u8,
    /// META-key background dimmer.
    pub meta_key_background_dimmer: u8,
    /// Idle-mode dimmer.
    pub idle_mode_dimmer: u8,
    /// Idle timeout in milliseconds.
    pub idle_timeout_ms: u32,
}

/// Rapid Tap block of a schema [`SCHEMA_B`] profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RapidTap {
    /// Rapid Tap enable byte.
    pub enable: u8,
    /// Ten Rapid Tap key pairs.
    pub pairs: Vec<RapidTapPair>,
}

/// One Rapid Tap pair (GG's `onboard_rapid_tap_pair`: five bytes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RapidTapPair {
    /// First HID code.
    pub hid_1: u8,
    /// Second HID code.
    pub hid_2: u8,
    /// Mode without the report-both-keys flag.
    pub mode: u8,
    /// High bit of the mode byte, reported separately.
    pub report_both_keys: bool,
    /// The two reserved bytes of the on-board pair, hex.
    pub reserved: String,
}

/// Summary of a payload block. The macro block is only summarised; the OLED block carries
/// `bytes_hex` as well, because the profile GUI draws it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MacroSummary {
    /// Offset of the block in the payload.
    pub offset: usize,
    /// Block length in bytes.
    pub len: usize,
    /// Non-zero bytes in the block.
    pub nonzero: usize,
    /// 64-bit FNV-1a hash of the block, hex: identity check between reads.
    pub fnv1a_64: String,
    /// First 16 bytes, hex.
    pub head: String,
    /// Every byte of the block, hex, space separated. Written for the blocks the engine treats as
    /// opaque byte arrays and that a write must reproduce verbatim: the OLED bitmap
    /// ([`OLED_LEN`] bytes = a 128x40 1bpp bitmap) and the macro block
    /// ([`MACRO_DATA_LEN`] bytes, the engine's own `(macro-data-size)`). `None` elsewhere, and
    /// `None` skips the field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_hex: Option<String>,
}

/// Marker byte that starts a packed macro record.
pub const MACRO_RECORD_MARKER: u8 = 0x77;
/// Action byte of a key press or release inside a macro record.
pub const MACRO_ACTION_KEY: u8 = 0x02;
/// Action byte of a timed pause inside a macro record.
pub const MACRO_ACTION_DELAY: u8 = 0x04;
/// Bytes of a macro record header: `0x77 | mode | id u16 | flags u16 | interval u16`.
pub const MACRO_HEADER_LEN: usize = 8;
/// Bytes of a macro press action: `0x02 | key | 0x01 0x01 | word u16 | ms u16`.
pub const MACRO_PRESS_LEN: usize = 8;
/// Bytes of a macro release action: `0x02 | key | 0x01 0x00`.
pub const MACRO_RELEASE_LEN: usize = 4;
/// Bytes of a macro delay action: `0x04 | 0x00 | ms u16`.
pub const MACRO_DELAY_LEN: usize = 4;
/// Macro mode GG writes for "play again every interval": the META + Insert binding of profile 1 in
/// the read round is a mode 4 record that presses `4` with `interval` 5000 ms.
pub const MACRO_MODE_REPEAT: u8 = 4;
/// Macro mode of a record that plays once, stepping `interval` ms between its actions.
pub const MACRO_MODE_ONCE: u8 = 1;
/// Records per binding index: a `0x71` mapping row carries `record ordinal * MACRO_INDEX_STRIDE` in its
/// third code byte, so the index a key carries is `7` times the position of the record it binds, and
/// `index / MACRO_INDEX_STRIDE` is the record. `[EXPERIMENTAL]`: every binding a read carried resolved by
/// `index / 7` to a record whose content matched the bound key, and the largest binding seen (117 =
/// 7 * 16 + 5) needs 17 records, the same order of size as the block holds. The encode side writes these
/// bytes back, and an encode verified against clean and modified profiles has been written on an Apex 7.
pub const MACRO_INDEX_STRIDE: usize = 7;

/// One action of a packed macro record. `[EXPERIMENTAL]`: the split below is what makes the 134
/// non-zero bytes of the only read that carries macros parse exactly, and the press `word` (4 in
/// every press seen) is still unexplained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MacroAction {
    /// `press`, `release` or `delay`.
    pub kind: &'static str,
    /// Key code of a press or release, absent on a delay.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<u8>,
    /// `u16` a press carries between its key byte and its duration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word: Option<u16>,
    /// Milliseconds: press duration on a press, pause length on a delay.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ms: Option<u16>,
}

/// One packed record of the macro data block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OnboardMacro {
    /// The number a `0x71` binding carries for this record: `ordinal * MACRO_INDEX_STRIDE`. A key bound
    /// to any index of the record's group (`0..MACRO_INDEX_STRIDE` around `ordinal * 7`) binds it.
    pub index: usize,
    /// Position in the packed list, from 0: `index / MACRO_INDEX_STRIDE`.
    pub ordinal: usize,
    /// Offset of the record inside the payload.
    pub offset: usize,
    /// Mode byte, see [`MACRO_MODE_REPEAT`] and [`MACRO_MODE_ONCE`]; `[EXPERIMENTAL]`, two records seen.
    pub mode: u8,
    /// Mode as a word, for the summary line.
    pub mode_name: &'static str,
    /// Record id (3 and 27 in the only read with macros; read as GG's database id).
    pub id: u16,
    /// The two bytes every record seen carries as `01 01`, unexplained.
    pub flags: u16,
    /// Repeat interval in mode [`MACRO_MODE_REPEAT`], inter-action delay in mode [`MACRO_MODE_ONCE`].
    pub interval_ms: u16,
    /// Bytes the record fills: header plus its actions. The list is packed with no length word, so
    /// this is what the decoder walked. In the reference read 2076 + 20 + 116 = 2212 is where the
    /// block content ends.
    pub record_bytes: usize,
    /// Number of actions parsed.
    pub action_count: usize,
    /// Actions of the record, in order.
    pub actions: Vec<MacroAction>,
    /// Raw record bytes, hex.
    pub raw: String,
}

/// Mode byte as a word.
#[must_use]
pub fn macro_mode_name(mode: u8) -> &'static str {
    match mode {
        MACRO_MODE_REPEAT => "repeat",
        MACRO_MODE_ONCE => "once",
        _ => "unknown",
    }
}

/// Parse the packed macro records out of a macro data block. `[EXPERIMENTAL]`.
///
/// A record is `0x77 | mode u8 | id u16 LE | flags u16 LE | interval u16 LE` followed by actions up
/// to the next record marker or the zero run that pads the block: press
/// `0x02 | key u8 | 0x01 0x01 | word u16 LE | ms u16 LE`, release `0x02 | key u8 | 0x01 0x00`, delay
/// `0x04 | 0x00 | ms u16 LE`.
///
/// Returns the records in list order and a note when the block does not parse cleanly; an all-zero
/// block (the usual case, GG leaves it empty) yields no records and no note.
///
/// TODO: At the moment this only decodes the five most recent macros, but the keyboard evidently has
/// more recorded somewhere.
#[must_use]
pub fn decode_macros(data: &[u8], offset: usize, len: usize) -> (Vec<OnboardMacro>, Option<String>) {
    let start = offset.min(data.len());
    let end = (offset + len).min(data.len());
    let block = &data[start..end];
    let mut records: Vec<OnboardMacro> = Vec::new();
    let mut at = 0usize;
    let mut zeros = 0usize;
    while at < block.len() {
        // A zero run is an empty binding group: GG leaves the records packed at the start of the block
        // and the scan walks on to the next marker, because a binding can name a record past every gap
        // in front of it (`some_rebounds.json` binds indices up to 117 in a block whose first records sit
        // in its first 108 bytes).
        if block[at] == 0 {
            zeros += 1;
            at += 1;
            continue;
        }
        if block[at] != MACRO_RECORD_MARKER {
            return (
                records,
                Some(format!(
                    "record at +{at} starts with 0x{:02X}, not the marker 0x77 ({zeros} zero bytes skipped so far)",
                    block[at]
                )),
            );
        }
        if at + MACRO_HEADER_LEN > block.len() {
            return (records, Some(format!("record header at +{at} runs past the block end")));
        }
        let mode = block[at + 1];
        let id = u16::from_le_bytes([block[at + 2], block[at + 3]]);
        let flags = u16::from_le_bytes([block[at + 4], block[at + 5]]);
        let interval_ms = u16::from_le_bytes([block[at + 6], block[at + 7]]);
        let mut actions: Vec<MacroAction> = Vec::new();
        let mut pos = at + MACRO_HEADER_LEN;
        let broken = loop {
            // Reaching the block end after a whole action is a normal end: the block is zero padded
            // only behind the last record, and a read image can stop right on the record's last byte.
            let Some(&byte) = block.get(pos) else { break None };
            if byte == 0 || byte == MACRO_RECORD_MARKER {
                break None;
            }
            if byte == MACRO_ACTION_KEY {
                let shape = block.get(pos + 2).copied();
                let flag = block.get(pos + 3).copied();
                match (shape, flag) {
                    (None, _) | (_, None) => {
                        break Some(format!(
                            "key action at +{pos} is cut by the block end: {} bytes left, a key action needs at least {MACRO_RELEASE_LEN}",
                            block.len() - pos
                        ));
                    }
                    (Some(0x01), Some(0x01)) => {
                        if pos + MACRO_PRESS_LEN > block.len() {
                            break Some("press action runs past the block end".to_string());
                        }
                        actions.push(MacroAction {
                            kind: "press",
                            key: Some(block[pos + 1]),
                            word: Some(u16::from_le_bytes([block[pos + 4], block[pos + 5]])),
                            ms: Some(u16::from_le_bytes([block[pos + 6], block[pos + 7]])),
                        });
                        pos += MACRO_PRESS_LEN;
                    }
                    (Some(0x01), Some(0x00)) => {
                        actions.push(MacroAction {
                            kind: "release",
                            key: Some(block[pos + 1]),
                            word: None,
                            ms: None,
                        });
                        pos += MACRO_RELEASE_LEN;
                    }
                    (Some(0x01), other) => {
                        break Some(format!(
                            "key action at +{pos} has flag {other:?}, not 0x01 (press) or 0x00 (release)"
                        ));
                    }
                    (other, _) => {
                        break Some(format!(
                            "key action at +{pos} has 0x{:02X} where 0x01 is expected",
                            other.unwrap_or(0)
                        ));
                    }
                }
            } else if byte == MACRO_ACTION_DELAY {
                if pos + MACRO_DELAY_LEN > block.len() {
                    break Some("delay action runs past the block end".to_string());
                }
                actions.push(MacroAction {
                    kind: "delay",
                    key: None,
                    word: None,
                    ms: Some(u16::from_le_bytes([block[pos + 2], block[pos + 3]])),
                });
                pos += MACRO_DELAY_LEN;
            } else {
                break Some(format!("unknown action byte 0x{byte:02X} at +{pos}"));
            }
        };
        let record_bytes = pos - at;
        let ordinal = records.len();
        records.push(OnboardMacro {
            index: ordinal * MACRO_INDEX_STRIDE,
            ordinal,
            offset: start + at,
            mode,
            mode_name: macro_mode_name(mode),
            id,
            flags,
            interval_ms,
            record_bytes,
            action_count: actions.len(),
            actions,
            raw: hex_of(&block[at..pos]),
        });
        if let Some(reason) = broken {
            return (records, Some(reason));
        }
        at = pos;
    }
    (records, None)
}

/// Decoded on-board profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OnboardProfile {
    /// Product ID of the keyboard these bytes were read from, stamped by the read. Absent for a decode of a
    /// file: bytes alone name no keyboard. Together with [`OnboardProfile::product`] and
    /// [`OnboardProfile::firmware`] these are the record's first fields - the keyboard first, its profile
    /// after. `[EXPERIMENTAL]`: one keyboard records an identity today (Apex 7).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_id: Option<u16>,
    /// Model name this crate lists for that product ID; see [`OnboardProfile::product_id`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    /// Firmware version the read's `0x90` poll answered with; see [`OnboardProfile::product_id`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firmware: Option<String>,
    /// Profile slot the read ran on (0-based), stamped by the read. Absent for a decode of a file:
    /// bytes alone say which slot they came from only if a reader already knows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<u8>,
    /// Schema word.
    pub schema: u16,
    /// Schema name (`a`, `b` or `unknown`).
    pub schema_name: String,
    /// Profile name (payload bytes 2..26, NUL-stripped).
    pub name: String,
    /// Raw profile name bytes, hex.
    pub name_hex: String,
    /// Lighting block.
    pub lighting: LightingProfile,
    /// META toggle HID byte.
    pub meta_toggle_hid: u8,
    /// 128 button mappings.
    pub button_mappings: Vec<KeyMapping>,
    /// Two wheel mappings.
    pub wheel_mappings: Vec<KeyMapping>,
    /// 128 button meta mappings.
    pub button_meta_mappings: Vec<KeyMapping>,
    /// Two wheel meta mappings.
    pub wheel_meta_mappings: Vec<KeyMapping>,
    /// OLED pixel block, carried verbatim: [`OLED_LEN`] = 640 bytes, opaque to `decode` and `encode`, so a
    /// write sends back exactly the bytes a read produced. Its form is confirmed on a `0x1612` keyboard: a 128 x
    /// 40 monochrome bitmap, 8 px per byte, 16 bytes per row, bit 7 of a byte the left pixel - the image GG
    /// paints on the panel was decoded to bytes and read back as this block, and writing them back paints that
    /// picture. `tools/profile-gui` does that conversion (draw, Clear / Invert, PNG export); nothing in this
    /// codec rasterises.
    pub oled: MacroSummary,
    /// Rapid Tap block, schema [`SCHEMA_B`] only.
    pub rapid_tap: Option<RapidTap>,
    /// Macro data block summary, carrying the block's bytes verbatim: the engine declares this field
    /// as a `bytearray` and never parses it (`lisp/apex_7.lisp:62, 101`), so a write sends these bytes
    /// back unchanged and [`Self::macros`] is display only.
    pub macro_data: MacroSummary,
    /// Packed macro records of the macro block, in list order, as [`decode_macros`] reads them.
    /// Display only: a write takes [`Self::macro_data`]'s bytes, never this list, when the report carries
    /// them. Empty when the block is empty.
    pub macros: Vec<OnboardMacro>,
    /// Why the macro block did not parse cleanly, `None` when it did. A note says the parsed record list
    /// in [`Self::macros`] is incomplete; it does not stop a write of a report that carries the block's
    /// bytes. `ssgg` has seen real GG blocks whose bytes its grammar rejects.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub macro_note: Option<String>,
    /// 16-byte profile GUID, hex.
    pub guid: String,
    /// Schema [`SCHEMA_A`] padding behind the GUID, hex (empty for schema [`SCHEMA_B`]).
    pub guid_padding: String,
    /// Checksum stored in the payload.
    pub crc_stored: u32,
    /// Checksum computed over the payload prefix.
    pub crc_computed: u32,
    /// True when both checksums match.
    pub crc_ok: bool,
    /// Length of the checksummed prefix.
    pub crc_region_len: usize,
    /// Payload length the decode used for this schema ([`PAYLOAD_LEN_A`] / [`PAYLOAD_LEN_B`]).
    pub payload_len: usize,
    /// Bytes the image carries **behind** the payload. The read fills the whole profile region
    /// ([`PROFILE_REGION_BYTES`]) and only its head is the payload: what sits behind it is recorded here
    /// and decoded by nothing. `0` skips the field, so a payload-only image (a saved `.bin`) says nothing.
    /// `[EXPERIMENTAL]`: no byte of this region is understood.
    #[serde(skip_serializing_if = "is_zero")]
    pub extra_len: usize,
    /// 64-bit FNV-1a of that region, hex, for identity between reads. Absent when there is no region.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_fnv1a_64: Option<String>,
    /// First 16 bytes of that region, hex. Absent when there is no region.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_head: Option<String>,
}

/// Layout of the decoded blocks.
///
/// This is the engine's own payload struct, read through: `profile_all_settings_struct` (schema A)
/// and `profile_all_settings_schema_b_struct` (schema B) in `lisp/apex_7.lisp` and
/// `lisp/base_apex_legacy.lisp`, whose byte comments give every offset below. `key_mapping` is
/// `function: uint8` plus `key_codes: bytearray (size 4)` - five bytes per mapping entry, not four;
/// `onboard_rapid_tap_pair` is five bytes. In a read image of the factory profile (slot 0, schema B)
/// the trailing checksum verifies over `payload[0..11692]` (STM32 CRC-32 = `0xAE57B9DC`).
///
/// Schema A: … wheel META (1375) → OLED (1385) → macro (2025) → GUID (11625) → 3 pad → CRC (11644).
/// Schema B: … wheel META (1375) → OLED (1385) → Rapid Tap (2025) → macro (2076) → GUID (11676) →
/// CRC (11692), with no padding word.
///
/// `[EXPERIMENTAL]`: the field meanings inside each block are still reverse-engineered. The block offsets
/// and strides are GG's own struct spec, and the decode matches what GG shows for clean and modified
/// profiles.
/// Offset of the Rapid Tap enable byte (schema [`SCHEMA_B`]).
pub const RAPID_TAP_ENABLE_OFFSET: usize = OLED_OFFSET + OLED_LEN;
/// Offset of the Rapid Tap pair table (schema [`SCHEMA_B`]).
pub const RAPID_TAP_PAIRS_OFFSET: usize = RAPID_TAP_ENABLE_OFFSET + 1;
/// Bytes of the Rapid Tap block: one enable byte plus ten 5-byte pairs.
pub const RAPID_TAP_LEN: usize = 1 + 10 * RAPID_TAP_PAIR_LEN;
/// Stride of one Rapid Tap pair (`onboard_rapid_tap_pair`).
pub const RAPID_TAP_PAIR_LEN: usize = 5;
/// Offset of the OLED pixel block, right behind the wheel META table in both schemas.
pub const OLED_OFFSET: usize = WHEEL_META_MAPPINGS_OFFSET + WHEEL_MAPPING_COUNT * KEY_MAPPING_LEN;
/// Length of the OLED pixel block.
pub const OLED_LEN: usize = 640;
/// Offset of the button mapping table.
pub const BUTTON_MAPPINGS_OFFSET: usize = META_TOGGLE_HID_OFFSET + 1;
/// Offset of the wheel mapping table.
pub const WHEEL_MAPPINGS_OFFSET: usize = BUTTON_MAPPINGS_OFFSET + BUTTON_MAPPING_COUNT * KEY_MAPPING_LEN;
/// Offset of the button meta mapping table.
pub const BUTTON_META_MAPPINGS_OFFSET: usize = WHEEL_MAPPINGS_OFFSET + WHEEL_MAPPING_COUNT * KEY_MAPPING_LEN;
/// Offset of the wheel meta mapping table.
pub const WHEEL_META_MAPPINGS_OFFSET: usize = BUTTON_META_MAPPINGS_OFFSET + BUTTON_MAPPING_COUNT * KEY_MAPPING_LEN;
/// Offset of the lighting block.
pub const LIGHTING_OFFSET: usize = NAME_OFFSET + NAME_LEN + 1;
/// Length of the lighting block.
pub const LIGHTING_LEN: usize = 57;
/// Offset of the profile name.
pub const NAME_OFFSET: usize = 2;
/// Length of the profile name field.
pub const NAME_LEN: usize = 24;
/// Profile name of a payload: the [`NAME_LEN`] bytes at [`NAME_OFFSET`], cut at the first NUL. An
/// empty string when the payload is too short or the name region is blank.
#[must_use]
pub fn profile_name(data: &[u8]) -> String {
    if data.len() <= NAME_OFFSET {
        return String::new();
    }
    let raw = &data[NAME_OFFSET..(NAME_OFFSET + NAME_LEN).min(data.len())];
    let name_end = raw.iter().position(|b| *b == 0).unwrap_or(raw.len());
    String::from_utf8_lossy(&raw[..name_end]).into_owned()
}
/// Offset of the META toggle HID byte.
pub const META_TOGGLE_HID_OFFSET: usize = LIGHTING_OFFSET + LIGHTING_LEN;
/// Length of the GUID field.
pub const GUID_LEN: usize = 16;
/// Bytes between the GUID and the checksum in a schema [`SCHEMA_A`] payload (schema B has none).
pub const GUID_PADDING_LEN: usize = 3;
/// Mapping table entry stride: GG's `key_mapping` (`function` byte + 4 `key_codes` bytes).
pub const KEY_MAPPING_LEN: usize = 5;
/// Buttons in each mapping table, GG's decode loop.
pub const BUTTON_MAPPING_COUNT: usize = 128;
/// Wheels in each mapping table.
pub const WHEEL_MAPPING_COUNT: usize = 2;
/// Offset of the macro block behind the OLED block (schema [`SCHEMA_A`]).
pub const MACRO_OFFSET_A: usize = OLED_OFFSET + OLED_LEN;
/// Offset of the macro block behind Rapid Tap (schema [`SCHEMA_B`]).
pub const MACRO_OFFSET_B: usize = RAPID_TAP_PAIRS_OFFSET + 10 * RAPID_TAP_PAIR_LEN;
/// Offset of the 16-byte profile GUID, behind the macro block (schema [`SCHEMA_A`]).
pub const GUID_OFFSET_A: usize = MACRO_OFFSET_A + MACRO_DATA_LEN;
/// Offset of the 16-byte profile GUID, behind the macro block (schema [`SCHEMA_B`]).
pub const GUID_OFFSET_B: usize = MACRO_OFFSET_B + MACRO_DATA_LEN;

/// Offset of the GUID block for a schema word; any other schema reads it where schema B puts it.
#[must_use]
pub fn guid_offset_for_schema(schema: u16) -> usize {
    if schema == SCHEMA_B {
        GUID_OFFSET_B
    } else {
        GUID_OFFSET_A
    }
}

/// GG's `device-offset-lookup-table`, inverted: mapping slot → HID usage code, `0xFF` = the device
/// has no key in that slot. Slots 120..127 belong to the `0xF1..0xF8` function codes, which are not
/// keys; they carry the wheel and media bindings of the profile.
pub const OFFSET_TO_HID: [u8; BUTTON_MAPPING_COUNT] = [
    41, 53, 43, 57, 225, 224, 255, 255, 58, 30, 20, 4, 100, 227, 72, 97, 59, 31, 26, 22, 29, 226, 74, 87, 60, 32, 8, 7,
    27, 255, 75, 92, 61, 33, 21, 9, 6, 139, 83, 93, 62, 34, 23, 10, 25, 44, 84, 94, 63, 35, 28, 11, 5, 138, 85, 133,
    64, 36, 24, 13, 17, 136, 86, 89, 65, 37, 12, 14, 16, 255, 77, 90, 66, 38, 18, 15, 54, 230, 78, 91, 67, 39, 19, 51,
    55, 231, 95, 99, 68, 45, 47, 52, 56, 240, 96, 88, 69, 46, 48, 50, 135, 228, 255, 255, 70, 137, 49, 40, 229, 80,
    255, 255, 71, 42, 73, 76, 82, 81, 79, 98, 241, 242, 243, 244, 245, 246, 247, 248,
];

/// Inverse of [`OFFSET_TO_HID`]: HID usage code → mapping slot, `0xFF` = not a mapping key. GG's
/// `device-offset-lookup-table` verbatim, with its `-1` entries (which the engine stores as 255)
/// kept as 255 here.
pub const HID_TO_OFFSET: [u8; 256] = [
    255, 255, 255, 255, 11, 52, 36, 27, 26, 35, 43, 51, 66, 59, 67, 75, 68, 60, 74, 82, 10, 34, 19, 42, 58, 44, 18, 28,
    50, 20, 9, 17, 25, 33, 41, 49, 57, 65, 73, 81, 107, 0, 113, 2, 45, 89, 97, 90, 98, 106, 99, 83, 91, 1, 76, 84, 92,
    3, 8, 16, 24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 104, 112, 14, 114, 22, 30, 115, 70, 78, 118, 109, 117, 116, 38,
    46, 54, 62, 23, 95, 63, 71, 79, 31, 39, 47, 86, 94, 15, 119, 87, 12, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    55, 255, 100, 61, 105, 53, 37, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 5, 4, 21,
    13, 101, 108, 77, 85, 255, 255, 255, 255, 255, 255, 255, 255, 93, 120, 121, 122, 123, 124, 125, 126, 127, 255, 255,
    255, 255, 255, 255, 255,
];

/// The key a mapping slot belongs to (`None` when the device has no key in that slot).
#[must_use]
pub fn offset_to_hid(offset: usize) -> Option<u8> {
    OFFSET_TO_HID.get(offset).copied().filter(|&hid| hid != 0xFF)
}

/// The mapping slot a key belongs to (`None` for keys the profile does not bind, e.g. `0x65` Select).
#[must_use]
pub fn hid_to_offset(hid: u8) -> Option<usize> {
    let slot = HID_TO_OFFSET[usize::from(hid)];
    if slot == 0xFF { None } else { Some(usize::from(slot)) }
}

/// GG's Meta cap: HID usage `0x65`, electrically the Application/Select keystroke. The table has no
/// slot under it - no profile key is named Meta - and GG binds it through [`META_KEY_GG_CODE`].
pub const META_KEY_USAGE: u8 = 0x65;
/// The code GG carries the Meta cap on: slot [`META_KEY_SLOT`] is the only slot whose own table code
/// is this, and GG's factory base layer binds it to itself (`51 F0 00 00 00`). A user profile may
/// rebind that slot - `some_rebounds.json` binds it to `0x65`, the Context Menu keystroke - while the
/// profile keeps naming a different key as the META toggle.
pub const META_KEY_GG_CODE: u8 = 0xF0;
/// The slot GG carries the Meta cap on, found by code, not written as a literal.
pub const META_KEY_SLOT: usize = 93;

/// The slot GG carries the Meta cap on: the slot whose table code is [`META_KEY_GG_CODE`].
/// `META_KEY_USAGE` itself has no slot ([`hid_to_offset`] returns `None` for it).
///
/// The Meta cap / slot / code identity is read-side evidence from the Apex 7 capture: reverse-engineered,
/// not a protocol fact proven on hardware.
#[must_use]
pub fn meta_key_offset() -> Option<usize> {
    OFFSET_TO_HID.iter().position(|&code| code == META_KEY_GG_CODE)
}

/// Function selector names, for summaries only; `0x00` means the key is deactivated.
pub fn function_name(function: u8) -> &'static str {
    match function {
        0x00 => "deactivated",
        0x01 => "mouse_left",
        0x02 => "mouse_right",
        0x03 => "mouse_middle",
        0x04 => "mouse_button_4",
        0x05 => "mouse_button_5",
        0x06 => "mouse_button_6",
        0x07 => "mouse_button_7",
        0x08 => "mouse_button_8",
        0x30 => "cpi",
        0x31 => "wheel_up",
        0x32 => "wheel_down",
        0x33 => "ac_pan_left",
        0x34 => "ac_pan_right",
        0x51 => "combo_key",
        0x61 => "media_key",
        0x62 => "meta",
        0x71 => "macro",
        _ => "unknown",
    }
}

/// 64-bit FNV-1a hash, hex. Used for block identity between reads, not as a protocol checksum.
pub fn fnv1a_64(data: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in data {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    format!("{hash:016X}")
}

fn summary_of(data: &[u8], offset: usize, len: usize) -> MacroSummary {
    let end = (offset + len).min(data.len());
    let start = offset.min(data.len());
    let block = &data[start..end];
    MacroSummary {
        offset,
        len,
        nonzero: block.iter().filter(|b| **b != 0).count(),
        fnv1a_64: fnv1a_64(block),
        head: hex_of(&block[..block.len().min(16)]),
        bytes_hex: None,
    }
}

/// Summary plus the full block bytes: the OLED block is a bitmap, not a blob to summarise.
fn summary_of_bytes(data: &[u8], offset: usize, len: usize) -> MacroSummary {
    let mut summary = summary_of(data, offset, len);
    let start = offset.min(data.len());
    let end = (offset + len).min(data.len());
    summary.bytes_hex = Some(hex_of(&data[start..end]));
    summary
}

/// Decode one `key_mapping` entry and classify it against the slot's own key.
fn key_mapping(data: &[u8], at: usize, offset: usize) -> Result<KeyMapping> {
    if at + KEY_MAPPING_LEN > data.len() {
        return Err(Error::Profile(format!(
            "key mapping at {at} needs {} bytes, the payload has {}",
            KEY_MAPPING_LEN,
            data.len()
        )));
    }
    let codes = &data[at + 1..at + KEY_MAPPING_LEN];
    let function = data[at];
    let own = offset_to_hid(offset);
    // GG's two "types normally" encodings: an all-zero slot (what GG itself writes) and the factory
    // default, a combo key whose only code is the slot's own key.
    let binding = if codes.iter().all(|b| *b == 0) && function == 0 {
        "none"
    } else if function == 0x51 && own.is_some_and(|hid| codes[0] == hid) && codes[1..].iter().all(|b| *b == 0) {
        "self"
    } else {
        "custom"
    };
    Ok(KeyMapping {
        offset,
        hid_code: own,
        function,
        function_name: function_name(function).to_string(),
        binding,
        key_codes: hex_of(codes),
        macro_index: (function == 0x71).then_some(codes[2]),
    })
}

fn key_table(data: &[u8], at: usize, count: usize) -> Result<Vec<KeyMapping>> {
    (0..count)
        .map(|i| key_mapping(data, at + i * KEY_MAPPING_LEN, i))
        .collect()
}

fn color(data: &[u8], at: usize) -> Result<Color> {
    if at + 3 > data.len() {
        return Err(Error::Profile(format!(
            "colour at {at} needs 3 bytes, the payload has {}",
            data.len()
        )));
    }
    Ok(Color {
        r: data[at],
        g: data[at + 1],
        b: data[at + 2],
    })
}

/// Decode a profile payload.
///
/// `data` must be a whole payload: [`PAYLOAD_LEN_A`] for schema [`SCHEMA_A`] or [`PAYLOAD_LEN_B`] for
/// schema [`SCHEMA_B`]. A shorter buffer is rejected rather than decoded into partial fields.
pub fn decode(data: &[u8]) -> Result<OnboardProfile> {
    let schema = schema_of(data);
    let expected = payload_len_for_schema(schema)?;
    if data.len() < expected {
        return Err(Error::Profile(format!(
            "payload is {} bytes, schema 0x{schema:04X} needs {expected}",
            data.len()
        )));
    }

    let raw_name = &data[NAME_OFFSET..NAME_OFFSET + NAME_LEN];
    let name_end = raw_name.iter().position(|b| *b == 0).unwrap_or(raw_name.len());
    let name = String::from_utf8_lossy(&raw_name[..name_end]).into_owned();

    let l = LIGHTING_OFFSET;
    let lighting = LightingProfile {
        oled_illumination_options: hex_of(&data[l..l + 6]),
        lock_key_color: color(data, l + 6)?,
        meta_key_color: color(data, l + 9)?,
        disabled_win_key_color: color(data, l + 12)?,
        meta_mapped_elements: hex_of(&data[l + 15..l + 47]),
        lock_key_hid_codes: hex_of(&data[l + 47..l + 50]),
        win_key_hid_code: data[l + 50],
        meta_key_background_dimmer: data[l + 51],
        idle_mode_dimmer: data[l + 52],
        idle_timeout_ms: u32::from_le_bytes([data[l + 53], data[l + 54], data[l + 55], data[l + 56]]),
    };

    let schema_b = schema == SCHEMA_B;
    let rapid_tap = schema_b.then(|| RapidTap {
        enable: data[RAPID_TAP_ENABLE_OFFSET],
        pairs: (0..10)
            .map(|i| {
                let at = RAPID_TAP_PAIRS_OFFSET + i * RAPID_TAP_PAIR_LEN;
                RapidTapPair {
                    hid_1: data[at],
                    hid_2: data[at + 1],
                    mode: data[at + 2] & 0x7f,
                    report_both_keys: data[at + 2] & 0x80 != 0,
                    reserved: hex_of(&data[at + 3..at + RAPID_TAP_PAIR_LEN]),
                }
            })
            .collect(),
    });

    let macro_offset = if schema_b { MACRO_OFFSET_B } else { MACRO_OFFSET_A };
    let guid_offset = guid_offset_for_schema(schema);
    // The checksum word is the last four bytes of the schema's payload, not of the image: a read
    // image is padded out to whole resource buffers behind it.
    let crc_offset = checksum_offset(expected);
    let (macros, macro_note) = decode_macros(data, macro_offset, MACRO_DATA_LEN);
    let computed = compute_checksum(&data[..crc_offset]);
    let stored = u32::from_le_bytes([
        data[crc_offset],
        data[crc_offset + 1],
        data[crc_offset + 2],
        data[crc_offset + 3],
    ]);

    Ok(OnboardProfile {
        // A decode of bytes names no keyboard; the read stamps the identity fields from the device it ran on.
        product_id: None,
        product: None,
        firmware: None,
        slot: None,
        schema,
        schema_name: match schema {
            SCHEMA_A => "a".to_string(),
            SCHEMA_B => "b".to_string(),
            _ => "unknown".to_string(),
        },
        name,
        name_hex: hex_of(&raw_name[..name_end]),
        lighting,
        meta_toggle_hid: data[META_TOGGLE_HID_OFFSET],
        button_mappings: key_table(data, BUTTON_MAPPINGS_OFFSET, BUTTON_MAPPING_COUNT)?,
        wheel_mappings: key_table(data, WHEEL_MAPPINGS_OFFSET, WHEEL_MAPPING_COUNT)?,
        button_meta_mappings: key_table(data, BUTTON_META_MAPPINGS_OFFSET, BUTTON_MAPPING_COUNT)?,
        wheel_meta_mappings: key_table(data, WHEEL_META_MAPPINGS_OFFSET, WHEEL_MAPPING_COUNT)?,
        oled: summary_of_bytes(data, OLED_OFFSET, OLED_LEN),
        rapid_tap,
        macro_data: summary_of_bytes(data, macro_offset, MACRO_DATA_LEN),
        macros,
        macro_note,
        guid: hex_of(&data[guid_offset..guid_offset + GUID_LEN]),
        guid_padding: if schema_b {
            String::new()
        } else {
            hex_of(&data[guid_offset + GUID_LEN..guid_offset + GUID_LEN + GUID_PADDING_LEN])
        },
        crc_stored: stored,
        crc_computed: computed,
        crc_ok: stored == computed,
        crc_region_len: crc_offset,
        payload_len: expected,
        extra_len: data.len().saturating_sub(expected),
        extra_fnv1a_64: (data.len() > expected).then(|| fnv1a_64(&data[expected..])),
        extra_head: (data.len() > expected).then(|| hex_of(&data[expected..data.len().min(expected + 16)])),
    })
}

/// serde helper: `extra_len` is written only when the image is longer than the payload.
fn is_zero(value: &usize) -> bool {
    *value == 0
}

/// Summary of the image bytes **behind** the payload: the region GG's write windows cover
/// ([`PROFILE_FILE_BYTES`] with the payload cut off the front). `None` when the image is no longer than
/// the payload, which is the case for every saved `.bin` file. An unknown schema word also answers `None`:
/// there is no payload length to cut at.
#[must_use]
pub fn extra_region(data: &[u8]) -> Option<MacroSummary> {
    let payload_len = payload_len_for_schema(schema_of(data)).ok()?;
    if data.len() <= payload_len {
        return None;
    }
    Some(summary_of(data, payload_len, data.len() - payload_len))
}

/// One-line human summary of a decoded profile.
pub fn summary_line(profile: &OnboardProfile) -> String {
    format!(
        "schema {}  name {:?}  guid {}  macro {} bytes ({} nonzero, {} record(s), fnv1a {})  crc {}",
        profile.schema_name,
        profile.name,
        profile.guid,
        profile.macro_data.len,
        profile.macro_data.nonzero,
        profile.macros.len(),
        profile.macro_data.fnv1a_64,
        if profile.crc_ok {
            "ok".to_string()
        } else {
            format!(
                "MISMATCH stored {:08X} computed {:08X}",
                profile.crc_stored, profile.crc_computed
            )
        }
    )
}

/// File stem a profile name yields for a dump file: lowercase ASCII alphanumerics kept, every other
/// character `_`, leading and trailing `_` dropped. `None` when nothing survives it - an unsaved slot
/// answers with a zero name, and a name in another alphabet has no ASCII letters.
#[must_use]
pub fn profile_file_stem(name: &str) -> Option<String> {
    let stem: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let stem = stem.trim_matches('_');
    (!stem.is_empty()).then(|| stem.to_string())
}

/// Default file name for a profile dump: the profile name (`Config 1` -> `config_1.json`), or GG's config
/// number when the read carries no name to build one from (`Config 3` answers a zero name, and the
/// fallback is `profile_config_3.json`).
#[must_use]
pub fn profile_dump_name(name: &str, config: u8) -> String {
    profile_file_stem(name).map_or_else(
        || format!("profile_config_{config}.json"),
        |stem| format!("{stem}.json"),
    )
}

/// JSON representation of a decoded profile.
pub fn profile_json(profile: &OnboardProfile) -> Result<String> {
    serde_json::to_string_pretty(profile).map_err(|e| Error::Profile(format!("serialize profile: {e}")))
}

/// JSON representation of a raw profile image with its per-pull records.
pub fn to_json(image: &ProfileImage) -> Result<String> {
    let payload = &image.data;
    let schema = image.schema();
    let decoded = if payload_len_for_schema(schema).is_ok() {
        json!(decode(payload).ok().map(|p| p.schema_name).unwrap_or_default())
    } else {
        json!(null)
    };
    let value = json!({
        // Identity first: product ID, model name, firmware version - which keyboard this came off.
        "product_id": image.product_id,
        "product": image.product,
        "firmware": image.firmware,
        "profile_slot": image.profile_slot,
        "fs_id": format!("0x{:04X}", profile_file_id(image.profile_slot)),
        "schema": format!("0x{schema:04X}"),
        "schema_decoded": decoded,
        "data_len": payload.len(),
        "sha_like_fnv1a_64": fnv1a_64(payload),
        "stats": image.stats,
        "chunks": image.chunks,
        "head_hex": hex_of(&payload[..payload.len().min(64)]),
    });
    serde_json::to_string_pretty(&value).map_err(|e| Error::Profile(format!("serialize profile image: {e}")))
}

/// Parse the space-separated hex a decoder field carries, requiring exactly `want` bytes.
fn parse_hex(text: &str, what: &str, want: usize) -> Result<Vec<u8>> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() != want {
        return Err(Error::Profile(format!(
            "{what} carries {} bytes, {want} expected: {text}",
            words.len()
        )));
    }
    words
        .iter()
        .map(|word| {
            u8::from_str_radix(word, 16)
                .map_err(|e| Error::Profile(format!("{what} has the byte {word:?}, which is not a hex byte: {e}")))
        })
        .collect()
}

#[derive(Deserialize)]
struct HexColor {
    r: u8,
    g: u8,
    b: u8,
}

#[derive(Deserialize)]
struct JsonLighting {
    oled_illumination_options: String,
    lock_key_color: HexColor,
    meta_key_color: HexColor,
    disabled_win_key_color: HexColor,
    meta_mapped_elements: String,
    lock_key_hid_codes: String,
    win_key_hid_code: u8,
    meta_key_background_dimmer: u8,
    idle_mode_dimmer: u8,
    idle_timeout_ms: u32,
}

#[derive(Deserialize)]
struct JsonMapping {
    offset: usize,
    function: u8,
    key_codes: String,
}

#[derive(Deserialize)]
struct JsonBlock {
    offset: usize,
    len: usize,
    #[serde(default)]
    bytes_hex: Option<String>,
}

#[derive(Deserialize)]
struct JsonRapidPair {
    hid_1: u8,
    hid_2: u8,
    mode: u8,
    report_both_keys: bool,
    reserved: String,
}

#[derive(Deserialize)]
struct JsonRapidTap {
    enable: u8,
    pairs: Vec<JsonRapidPair>,
}

#[derive(Deserialize)]
struct JsonMacro {
    offset: usize,
    record_bytes: usize,
    raw: String,
}

/// The shape of a decode report, read back for encoding. Every field the encoder does not need
/// (`crc_ok`, `binding`, `function_name`, `keystrokes`, hashes) is ignored.
#[derive(Deserialize)]
struct JsonProfile {
    schema: u16,
    name_hex: String,
    lighting: JsonLighting,
    meta_toggle_hid: u8,
    button_mappings: Vec<JsonMapping>,
    wheel_mappings: Vec<JsonMapping>,
    button_meta_mappings: Vec<JsonMapping>,
    wheel_meta_mappings: Vec<JsonMapping>,
    oled: JsonBlock,
    #[serde(default)]
    rapid_tap: Option<JsonRapidTap>,
    macro_data: JsonBlock,
    #[serde(default)]
    macros: Vec<JsonMacro>,
    /// The encoder writes the records it was given, so a block the decoder could not walk is refused.
    #[serde(default)]
    macro_note: Option<String>,
    guid: String,
    guid_padding: String,
}

fn write_color(data: &mut [u8], at: usize, color: &HexColor) {
    data[at] = color.r;
    data[at + 1] = color.g;
    data[at + 2] = color.b;
}

fn write_mapping_table(data: &mut [u8], at: usize, entries: &[JsonMapping], count: usize, what: &str) -> Result<()> {
    if entries.len() != count {
        return Err(Error::Profile(format!(
            "{what} has {} entries, {count} expected",
            entries.len()
        )));
    }
    for (index, entry) in entries.iter().enumerate() {
        if entry.offset != index {
            return Err(Error::Profile(format!(
                "{what} slot {index} carries offset {}, so the table is not in payload order",
                entry.offset
            )));
        }
        let at = at + index * KEY_MAPPING_LEN;
        let codes = parse_hex(
            &entry.key_codes,
            &format!("{what} slot {index} codes"),
            KEY_MAPPING_LEN - 1,
        )?;
        data[at] = entry.function;
        data[at + 1..at + KEY_MAPPING_LEN].copy_from_slice(&codes);
    }
    Ok(())
}

/// Write the block a summary describes, validating that the summary still points where this schema
/// puts it: a JSON from an older decoder, or from the other schema, is refused instead of writing
/// bytes into the wrong place.
fn write_block(
    data: &mut [u8],
    summary: &JsonBlock,
    offset: usize,
    len: usize,
    bytes_hex: &str,
    what: &str,
) -> Result<()> {
    if summary.offset != offset || summary.len != len {
        return Err(Error::Profile(format!(
            "{what} is described at offset {} as {len} bytes, this schema puts it at offset {offset} as {len} bytes",
            summary.offset
        )));
    }
    let bytes = parse_hex(bytes_hex, what, len)?;
    data[offset..offset + len].copy_from_slice(&bytes);
    Ok(())
}

/// Encode a decode report back into profile payload bytes: the inverse of [`decode`].
///
/// The payload is rebuilt from the schema word, the name bytes, the lighting block, the META toggle
/// key, the four mapping tables, the OLED pixels, the Rapid Tap block, the packed macro records and
/// the GUID; the last four bytes are the checksum this function computes, never the `crc_stored` of
/// the report. Everything the report does not carry (the zero pad of the macro block, the padding
/// behind the name) is written as the zero bytes the reference reads have there.
///
/// Verified against clean and modified profile files, and an encode of a decoded read has been written on
/// an Apex 7 (`0x1612`) and read back. The layout this inverts is reverse-engineered: field meanings inside
/// the OLED and macro blocks are not proven.
///
/// # Errors
///
/// Returns [`Error::Profile`] when the JSON is not a decode report, when a field is the wrong length,
/// when a block sits where this schema does not put it, when a macro block carries a decode note (the
/// records the decoder could not walk cannot be written back), when Rapid Tap does not match the
/// schema, or when the tables are not in payload order.
pub fn encode_json(text: &str) -> Result<Vec<u8>> {
    let profile: JsonProfile = serde_json::from_str(text).map_err(|e| Error::Profile(format!("profile JSON: {e}")))?;
    let len = payload_len_for_schema(profile.schema)?;
    let schema_b = profile.schema == SCHEMA_B;
    let macro_offset = if schema_b { MACRO_OFFSET_B } else { MACRO_OFFSET_A };
    let guid_offset = guid_offset_for_schema(profile.schema);
    let mut data = vec![0u8; len];
    data[..2].copy_from_slice(&profile.schema.to_le_bytes());

    let name = parse_hex(
        &profile.name_hex,
        "profile name",
        profile.name_hex.split_whitespace().count(),
    )?;
    if name.len() > NAME_LEN {
        return Err(Error::Profile(format!(
            "profile name is {} bytes, the field holds {NAME_LEN}",
            name.len()
        )));
    }
    data[NAME_OFFSET..NAME_OFFSET + name.len()].copy_from_slice(&name);

    let l = LIGHTING_OFFSET;
    let lighting = &profile.lighting;
    data[l..l + 6].copy_from_slice(&parse_hex(
        &lighting.oled_illumination_options,
        "OLED illumination options",
        6,
    )?);
    write_color(&mut data, l + 6, &lighting.lock_key_color);
    write_color(&mut data, l + 9, &lighting.meta_key_color);
    write_color(&mut data, l + 12, &lighting.disabled_win_key_color);
    data[l + 15..l + 47].copy_from_slice(&parse_hex(&lighting.meta_mapped_elements, "META mapping mask", 32)?);
    data[l + 47..l + 50].copy_from_slice(&parse_hex(&lighting.lock_key_hid_codes, "lock-key HID codes", 3)?);
    data[l + 50] = lighting.win_key_hid_code;
    data[l + 51] = lighting.meta_key_background_dimmer;
    data[l + 52] = lighting.idle_mode_dimmer;
    data[l + 53..l + 57].copy_from_slice(&lighting.idle_timeout_ms.to_le_bytes());
    data[META_TOGGLE_HID_OFFSET] = profile.meta_toggle_hid;

    write_mapping_table(
        &mut data,
        BUTTON_MAPPINGS_OFFSET,
        &profile.button_mappings,
        BUTTON_MAPPING_COUNT,
        "button mapping",
    )?;
    write_mapping_table(
        &mut data,
        WHEEL_MAPPINGS_OFFSET,
        &profile.wheel_mappings,
        WHEEL_MAPPING_COUNT,
        "wheel mapping",
    )?;
    write_mapping_table(
        &mut data,
        BUTTON_META_MAPPINGS_OFFSET,
        &profile.button_meta_mappings,
        BUTTON_MAPPING_COUNT,
        "button META mapping",
    )?;
    write_mapping_table(
        &mut data,
        WHEEL_META_MAPPINGS_OFFSET,
        &profile.wheel_meta_mappings,
        WHEEL_MAPPING_COUNT,
        "wheel META mapping",
    )?;

    write_block(
        &mut data,
        &profile.oled,
        OLED_OFFSET,
        OLED_LEN,
        profile.oled.bytes_hex.as_deref().ok_or_else(|| {
            Error::Profile("OLED block carries no bytes_hex, so its pixels cannot be written".to_string())
        })?,
        "OLED block",
    )?;

    match (&profile.rapid_tap, schema_b) {
        (Some(rapid_tap), true) => {
            data[RAPID_TAP_ENABLE_OFFSET] = rapid_tap.enable;
            if rapid_tap.pairs.len() != 10 {
                return Err(Error::Profile(format!(
                    "Rapid Tap has {} pairs, 10 expected",
                    rapid_tap.pairs.len()
                )));
            }
            for (index, pair) in rapid_tap.pairs.iter().enumerate() {
                let at = RAPID_TAP_PAIRS_OFFSET + index * RAPID_TAP_PAIR_LEN;
                data[at] = pair.hid_1;
                data[at + 1] = pair.hid_2;
                data[at + 2] = pair.mode | if pair.report_both_keys { 0x80 } else { 0 };
                data[at + 3..at + RAPID_TAP_PAIR_LEN].copy_from_slice(&parse_hex(
                    &pair.reserved,
                    &format!("Rapid Tap pair {index} reserved bytes"),
                    2,
                )?);
            }
        }
        (None, true) => {
            return Err(Error::Profile(
                "schema b carries a Rapid Tap block and the JSON has none".to_string(),
            ));
        }
        (Some(_), false) => {
            return Err(Error::Profile(
                "the JSON carries Rapid Tap and schema a has no Rapid Tap block".to_string(),
            ));
        }
        (None, false) => {}
    }

    if profile.macro_data.offset != macro_offset || profile.macro_data.len != MACRO_DATA_LEN {
        return Err(Error::Profile(format!(
            "macro block is described at offset {} as {} bytes, this schema puts it at offset {macro_offset} as {MACRO_DATA_LEN} bytes",
            profile.macro_data.offset, profile.macro_data.len
        )));
    }
    // The engine's own field is an opaque 9,600-byte bytearray, so a report that carries the block's
    // bytes writes them back verbatim and its parsed record list is not consulted. `bytes_hex` is absent
    // only in JSON a person wrote by hand, and then the records are the only source of bytes there is.
    if let Some(hex) = profile.macro_data.bytes_hex.as_deref() {
        let raw = parse_hex(hex, "macro block", hex.split_whitespace().count())?;
        if raw.len() != MACRO_DATA_LEN {
            return Err(Error::Profile(format!(
                "macro block carries {} bytes, the block is {MACRO_DATA_LEN}",
                raw.len()
            )));
        }
        data[macro_offset..macro_offset + MACRO_DATA_LEN].copy_from_slice(&raw);
    } else {
        encode_macro_records(&profile, macro_offset, &mut data)?;
    }

    data[guid_offset..guid_offset + GUID_LEN].copy_from_slice(&parse_hex(&profile.guid, "profile GUID", GUID_LEN)?);
    if schema_b {
        if !profile.guid_padding.is_empty() {
            return Err(Error::Profile(
                "schema b has no padding behind the GUID and the JSON carries some".to_string(),
            ));
        }
    } else {
        data[guid_offset + GUID_LEN..guid_offset + GUID_LEN + GUID_PADDING_LEN].copy_from_slice(&parse_hex(
            &profile.guid_padding,
            "GUID padding",
            GUID_PADDING_LEN,
        )?);
    }

    let crc_offset = checksum_offset(len);
    let computed = compute_checksum(&data[..crc_offset]);
    data[crc_offset..crc_offset + 4].copy_from_slice(&computed.to_le_bytes());
    Ok(data)
}

/// Pack the parsed macro records into the block: the source of bytes for JSON that carries no
/// `macro_data.bytes_hex`. A block that did not parse has bytes this grammar cannot represent, so the
/// write is refused rather than sent with invented bytes.
fn encode_macro_records(profile: &JsonProfile, macro_offset: usize, data: &mut [u8]) -> Result<()> {
    if let Some(note) = &profile.macro_note {
        return Err(Error::Profile(format!(
            "the macro block did not parse ({note}) and the JSON carries no macro_data.bytes_hex, so its \
             bytes cannot be written back - read the profile with `ssgg onboard read` and write that report"
        )));
    }
    let mut written = 0usize;
    for record in &profile.macros {
        let raw = parse_hex(
            &record.raw,
            &format!("macro record at {}", record.offset),
            record.record_bytes,
        )?;
        let at = record.offset;
        if at < macro_offset || at + raw.len() > macro_offset + MACRO_DATA_LEN {
            return Err(Error::Profile(format!(
                "macro record at {at} ({} bytes) is outside the macro block ({macro_offset}..)",
                raw.len()
            )));
        }
        if at < macro_offset + written {
            return Err(Error::Profile(format!(
                "macro record at {at} overlaps the record the list packed before it, so the list is not in payload order"
            )));
        }
        data[at..at + raw.len()].copy_from_slice(&raw);
        written = at + raw.len() - macro_offset;
    }
    Ok(())
}

/// The Apex 7 on-board profile codec: what `ssgg onboard` does with the bytes of this payload layout, with
/// no device involved. `apex_7.rs` is what moves them on a keyboard.
///
/// Registered in [`onboard::codecs`](crate::devices::keyboards::onboard::codecs) and handed out by
/// [`Apex7::onboard_codec`](crate::devices::keyboards::apex_7::Apex7). The report shape this codec carries is
/// this layout as data; `onboard.rs` says how the next keyboard plugs in.
///
/// The layout is reverse-engineered from GG captures of one `0x1612`. It is read **and** written on that
/// board, and its decode and encode are verified against clean and modified profiles. The text builders
/// below are its words for a read and a decode.
pub struct Apex7Onboard;

impl OnboardProfileCodec for Apex7Onboard {
    fn family_name(&self) -> &'static str {
        "Apex 7"
    }

    /// The engine's write sequence, framed for this family's geometry (see [`write_plan`]).
    fn write_plan(&self, profile: u8, payload: &[u8]) -> Result<OnboardWritePlan> {
        write_plan(profile, payload)
    }

    /// The write geometry this codec frames, with the source of every number in it.
    fn write_banner_lines(&self) -> Vec<String> {
        vec![
            "Apex 7 write geometry (the engine's write API, reproduced frame for frame by the \
             Apex 7 write capture):"
                .to_string(),
            format!(
                "  profile file {:04X} + slot, region {} bytes = {} windows of {} bytes",
                PROFILE_FS_BASE, PROFILE_REGION_BYTES, WRITE_WINDOW_COUNT, WRITE_PUSH_BYTES
            ),
            format!(
                "  one window = 2 writes of {WRITE_CHUNK_BYTES} bytes into the resource buffer (offsets 0 and \
                 {WRITE_CHUNK_BYTES}, 642-wire-byte feature report) + 1 push of the full buffer \
                 (64-wire-byte output report)"
            ),
            "  88 00 <file id> opens the file before the windows; eb <slot + 1> commits it after the last \
             push"
                .to_string(),
            "  answers: 88 01 … after the open, 05 01 … after every push; the writes and the commit are not \
             answered"
                .to_string(),
            format!(
                "  89 <slot + 1> deploys the slot after a {DEPLOY_SETTLE_MS} ms settle (the engine sleeps 250 ms \
                 behind it and its commit path ends with an input read); no 6c driver-mode frame is sent"
            ),
        ]
    }

    fn config_count(&self) -> u8 {
        MAX_PROFILE_INDEX + 1
    }

    /// GG's config number to the slot behind it; the family owns the numbering because the slots it
    /// numbers are the family's.
    fn slot_for_config(&self, config: u8) -> Result<u8> {
        let Some(slot) = config.checked_sub(1) else {
            return Err(Error::InvalidConfig(format!(
                "config {config} is not a config number: use 1-{} (GG's Config 1 .. Config {})",
                self.config_count(),
                self.config_count()
            )));
        };
        validate_profile(slot)?;
        Ok(slot)
    }

    fn knows_schema(&self, schema: u16) -> bool {
        matches!(schema, SCHEMA_A | SCHEMA_B)
    }

    fn mapping_len(&self) -> usize {
        KEY_MAPPING_LEN
    }

    fn decode(&self, data: &[u8]) -> Result<OnboardProfile> {
        decode(data)
    }

    fn encode_json(&self, text: &str) -> Result<Vec<u8>> {
        encode_json(text)
    }

    fn profile_json(&self, profile: &OnboardProfile) -> Result<String> {
        profile_json(profile)
    }

    fn image_json(&self, image: &ProfileImage) -> Result<String> {
        to_json(image)
    }

    fn dump_name(&self, data: &[u8], config: u8) -> String {
        profile_dump_name(&profile_name(data), config)
    }

    fn summary_line(&self, profile: &OnboardProfile) -> String {
        summary_line(profile)
    }

    fn hex(&self, data: &[u8], from: usize, to: usize) -> String {
        hex_dump(data, from, to)
    }

    fn diff(&self, a: &ProfileImage, b: &ProfileImage) -> Option<usize> {
        image_diff(a, b)
    }

    fn image_lines(&self, image: &ProfileImage, label: &str) -> Vec<String> {
        image_lines(image, label)
    }

    fn profile_lines(&self, profile: &OnboardProfile) -> Vec<String> {
        profile_lines(profile)
    }

    fn check_lines(&self, encoded: &[u8], reference: &[u8], path: &str) -> Vec<String> {
        check_lines(encoded, reference, path)
    }
}

/// One read image as the lines the CLI prints: the bytes it collected, the firmware poll answer, the
/// transfer record, then the decoded profile (or the decode's own failure).
fn image_lines(image: &ProfileImage, label: &str) -> Vec<String> {
    let mut lines = vec![
        format!("read {} profile bytes from {label}", image.data.len()),
        match (image.product_id, image.product.as_deref()) {
            (Some(product_id), Some(product)) => format!("device:  {product} (product {product_id:04X})"),
            _ => "device:  not recorded".to_string(),
        },
        match image.firmware.as_deref() {
            Some(firmware) => format!("firmware:  {firmware} (answered the 0x90 firmware poll)"),
            None => "firmware:  no answer to the 0x90 firmware poll".to_string(),
        },
        match image.region.as_ref() {
            Some(region) => format!(
                "region:  {} bytes behind the payload at {} ({} non-zero, fnv1a {}, head {}): recorded, decoded by nothing",
                region.len, region.offset, region.nonzero, region.fnv1a_64, region.head
            ),
            None => "region:  none behind the payload".to_string(),
        },
        format!(
            "transfers: {} payload reads over {} pulls; {} bytes covered, {} short read(s)",
            image.stats.reads_read, image.stats.chunks_read, image.stats.covered_len, image.stats.short_reads
        ),
        format!(
            "replies: {} header mismatch(es), {} all-zero read(s), {} all-zero pull(s), {} garbage page(s)",
            image.stats.header_mismatches, image.stats.zero_reads, image.stats.zero_chunks, image.stats.garbage_reads
        ),
    ];
    match decode(&image.data) {
        Ok(profile) => lines.extend(profile_lines(&profile)),
        Err(e) => lines.push(format!("decode: {e}")),
    }
    lines.push(String::new());
    lines
}

/// A decoded profile as the lines the CLI prints. The `[EXPERIMENTAL]` labels are on the blocks whose
/// field meanings are read out of GG's struct and not off a keyboard.
fn profile_lines(profile: &OnboardProfile) -> Vec<String> {
    fn color(c: &Color) -> String {
        format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
    }

    let mut lines = vec![
        match (profile.product_id, profile.product.as_deref()) {
            (Some(product_id), Some(product)) => format!("device:  {product} (product {product_id:04X})"),
            _ => "device:  not recorded (decoded from a file, which names no keyboard)".to_string(),
        },
        format!("schema: {} (0x{:04X})", profile.schema_name, profile.schema),
        format!(
            "checksum: stored {:08X}, computed {:08X} over the {}-byte payload (offsets 0..{}), match: {}",
            profile.crc_stored, profile.crc_computed, profile.crc_region_len, profile.crc_region_len, profile.crc_ok
        ),
        format!("name: {:?} (raw bytes {})", profile.name, profile.name_hex),
        format!(
            "guid: {:?} ({} padding after the GUID{})",
            profile.guid,
            profile.guid_padding,
            if profile.guid_padding.is_empty() {
                ": schema B has none"
            } else {
                ""
            }
        ),
        format!("META toggle HID code: 0x{:02X}", profile.meta_toggle_hid),
        format!(
            "mappings: {} buttons, {} wheels, {} button META, {} wheel META, {} bytes per entry",
            profile.button_mappings.len(),
            profile.wheel_mappings.len(),
            profile.button_meta_mappings.len(),
            profile.wheel_meta_mappings.len(),
            KEY_MAPPING_LEN
        ),
        format!(
            "bindings: {} custom on the base layer, {} on the META layer (GG's factory default - a key bound to itself - and empty slots are not counted)",
            profile.button_mappings.iter().filter(|m| m.binding == "custom").count(),
            profile
                .button_meta_mappings
                .iter()
                .filter(|m| m.binding == "custom")
                .count()
        ),
    ];

    let l = &profile.lighting;
    lines.push(format!(
        "lighting [EXPERIMENTAL layout]: lock {} meta {} disabled win {}",
        color(&l.lock_key_color),
        color(&l.meta_key_color),
        color(&l.disabled_win_key_color)
    ));
    lines.push(format!(
        "  dimmers: META {}, idle mode {}; idle timeout {} ms; win HID 0x{:02X}; lock HID codes {}",
        l.meta_key_background_dimmer, l.idle_mode_dimmer, l.idle_timeout_ms, l.win_key_hid_code, l.lock_key_hid_codes
    ));
    lines.push(format!("  OLED illumination options {}", l.oled_illumination_options));
    lines.push(format!("  META mapping mask {}", l.meta_mapped_elements));

    lines.push(match &profile.rapid_tap {
        Some(rt) => format!(
            "rapid tap [EXPERIMENTAL]: enable 0x{:02X}, {} key pairs",
            rt.enable,
            rt.pairs.len()
        ),
        None => "rapid tap: absent (schema A has no rapid-tap block)".to_string(),
    });

    for (label, block) in [("OLED", &profile.oled), ("macro", &profile.macro_data)] {
        lines.push(format!(
            "{label}: {} bytes at offset {}, {} non-zero, FNV-1a {}, head {}",
            block.len, block.offset, block.nonzero, block.fnv1a_64, block.head
        ));
    }

    lines.extend(macro_lines(profile));
    lines
}

/// The macro block as GG shows it: the records the block packs, and what each `0x71` binding points at.
/// The record index is the third code byte of the binding, so an index past the record list is a macro
/// that was deleted with its binding left behind on the keyboard.
fn macro_lines(profile: &OnboardProfile) -> Vec<String> {
    fn actions_line(actions: &[MacroAction]) -> String {
        actions
            .iter()
            .map(|action| match action.kind {
                "press" => format!(
                    "press {} for {} ms",
                    action
                        .key
                        .map_or_else(|| "key ?".to_string(), |key| format!("key 0x{key:02X}")),
                    action.ms.unwrap_or(0)
                ),
                "release" => format!(
                    "release {}",
                    action
                        .key
                        .map_or_else(|| "key ?".to_string(), |key| format!("key 0x{key:02X}"))
                ),
                _ => format!("delay {} ms", action.ms.unwrap_or(0)),
            })
            .collect::<Vec<_>>()
            .join(" > ")
    }

    if profile.macros.is_empty() {
        return match &profile.macro_note {
            Some(note) => vec![format!("  macro records [EXPERIMENTAL]: {note}")],
            None => vec![format!(
                "  macro records: the block is empty ({} non-zero bytes in {} at {})",
                profile.macro_data.nonzero, profile.macro_data.len, profile.macro_data.offset
            )],
        };
    }

    let content_end = profile
        .macros
        .iter()
        .map(|rec| rec.offset + rec.record_bytes)
        .max()
        .unwrap_or(profile.macro_data.offset);
    let mut lines = vec![format!(
        "  macro records [EXPERIMENTAL]: {} record(s) in the block from {} - a binding carries its record's ordinal times {}, so the block is walked to its end for the records a binding can name (last record ends at {})",
        profile.macros.len(),
        profile.macro_data.offset,
        MACRO_INDEX_STRIDE,
        content_end
    )];
    for rec in &profile.macros {
        lines.push(format!(
            "    record {} (ordinal {}, bound by indices {}..{}): {} (mode 0x{:02X}), id {}, interval {} ms, {} action(s), {} bytes at {}",
            rec.index,
            rec.ordinal,
            rec.index,
            rec.index + MACRO_INDEX_STRIDE - 1,
            rec.mode_name,
            rec.mode,
            rec.id,
            rec.interval_ms,
            rec.action_count,
            rec.record_bytes,
            rec.offset
        ));
        lines.push(format!("      keys: {}", actions_line(&rec.actions)));
        lines.push(format!("      raw:  {}", rec.raw));
    }
    for (offset, entry) in profile.button_meta_mappings.iter().enumerate() {
        let Some(index) = entry.macro_index else { continue };
        let ordinal = usize::from(index) / MACRO_INDEX_STRIDE;
        match profile.macros.iter().find(|rec| rec.ordinal == ordinal) {
            Some(rec) => lines.push(format!(
                "    bound at META mapping {}: index {} -> record {} (ordinal {}) - {} every {} ms: {}",
                offset,
                index,
                rec.index,
                rec.ordinal,
                rec.mode_name,
                rec.interval_ms,
                actions_line(&rec.actions)
            )),
            None => lines.push(format!(
                "    bound at META mapping {}: index {} -> ordinal {}, past the {} record(s) the block holds - deleted macro, the record is gone and the binding is left behind",
                offset,
                index,
                ordinal,
                profile.macros.len()
            )),
        }
    }
    lines
}

/// What `ssgg onboard encode --check` says about encoded bytes against a profile file. A read image is
/// longer than the payload it carries (the pulls fill whole 1024-byte buffers), so the comparison runs
/// over the payload and the bytes behind it are reported as what they are: device buffer content, not
/// profile bytes. In the read of slot 3 those 592 bytes behind the 11,696-byte payload are 80
/// bytes of `FF` and then zeros.
fn check_lines(encoded: &[u8], reference: &[u8], path: &str) -> Vec<String> {
    let common = encoded.len().min(reference.len());
    match payload_diff(&encoded[..common], &reference[..common]) {
        Some(at) => vec![format!(
            "check {path}: first difference at byte {at} ({} bytes encoded, {} bytes in the file)",
            encoded.len(),
            reference.len()
        )],
        None if encoded.len() == reference.len() => {
            vec![format!("check {path}: byte identical, {} bytes", encoded.len())]
        }
        None => {
            let behind = &reference[common..];
            vec![format!(
                "check {path}: byte identical over the {common} profile bytes; the file is a {}-byte read image, carrying {} bytes behind the payload ({} of them non-zero: device buffer content, not profile bytes)",
                reference.len(),
                behind.len(),
                behind.iter().filter(|b| **b != 0).count()
            )]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::hid_reports::{
        CommandCode, HidCommand, HidDeviceType, ONBOARD_FEATURE_REPORT_SIZE, OnboardBufferReadCommand,
        OnboardPayloadPullCommand,
    };

    /// The significant frame bytes a typed command serializes, behind its report-ID byte.
    fn frame_of<C: HidCommand>(command: C, frame_len: usize) -> Vec<u8> {
        let mut buffer = vec![0u8; ONBOARD_FEATURE_REPORT_SIZE];
        let size = command
            .serialize(&mut buffer, HidDeviceType::Keyboard)
            .expect("the read plan sends this frame, so it is serializable");
        assert!(frame_len < size, "{} fits in {size} bytes", command.description());
        buffer[1..frame_len + 1].to_vec()
    }

    /// A macro data block of the measured shape: two packed records, 20 bytes then 116, marker `0x77`, then
    /// the zero pad. The numbers and the keys here are **made up** - no bytes of a real profile
    /// are kept in this file. What the fixture carries is the structure the decoder has to walk: no
    /// stride, no length word, the second record starting at the next `0x77`, a press of 8 bytes
    /// (`02 key 01 01 word u16 ms u16`), a release of 4 (`02 key 01 00`) and a delay of 4
    /// (`04 00 ms u16`), the last action byte equal to the pad.
    const SYNTH_MACRO_BLOCK: [u8; 136] = [
        0x77, 0x04, 0x09, 0x00, 0x01, 0x01, 0x88, 0x13, 0x02, 0x04, 0x01, 0x01, 0x04, 0x00, 0x01, 0x00, 0x02, 0x04,
        0x01, 0x00, 0x77, 0x01, 0x0B, 0x00, 0x01, 0x01, 0x0F, 0x00, 0x02, 0x1E, 0x01, 0x01, 0x04, 0x00, 0x0F, 0x00,
        0x02, 0x1E, 0x01, 0x00, 0x04, 0x00, 0x0F, 0x00, 0x02, 0x1F, 0x01, 0x01, 0x04, 0x00, 0x0F, 0x00, 0x02, 0x1F,
        0x01, 0x00, 0x04, 0x00, 0x0F, 0x00, 0x02, 0x1F, 0x01, 0x01, 0x04, 0x00, 0x0F, 0x00, 0x02, 0x1F, 0x01, 0x00,
        0x04, 0x00, 0x0F, 0x00, 0x02, 0x1F, 0x01, 0x01, 0x04, 0x00, 0x0F, 0x00, 0x02, 0x1F, 0x01, 0x00, 0x04, 0x00,
        0x0F, 0x00, 0x02, 0x1F, 0x01, 0x01, 0x04, 0x00, 0x0F, 0x00, 0x02, 0x1F, 0x01, 0x00, 0x04, 0x00, 0x0F, 0x00,
        0x02, 0x1F, 0x01, 0x01, 0x04, 0x00, 0x0F, 0x00, 0x02, 0x1F, 0x01, 0x00, 0x04, 0x00, 0x0F, 0x00, 0x02, 0x20,
        0x01, 0x01, 0x04, 0x00, 0x0F, 0x00, 0x02, 0x20, 0x01, 0x00,
    ];

    fn synthetic_payload(schema: u16) -> Vec<u8> {
        let len = payload_len_for_schema(schema).unwrap();
        let mut data = vec![0u8; len];
        data[0..2].copy_from_slice(&schema.to_le_bytes());
        data[NAME_OFFSET..NAME_OFFSET + 4].copy_from_slice(b"Pro7");
        data[META_TOGGLE_HID_OFFSET] = 0x62;
        data[LIGHTING_OFFSET + 6] = 0xFF; // lock key colour red
        data[LIGHTING_OFFSET + 53..LIGHTING_OFFSET + 57].copy_from_slice(&1500u32.to_le_bytes());
        // GG's factory default layer: every mapped key bound to itself (`0x51` + its own HID code).
        for offset in 0..BUTTON_MAPPING_COUNT {
            let Some(hid) = offset_to_hid(offset) else {
                continue;
            };
            let at = BUTTON_MAPPINGS_OFFSET + offset * KEY_MAPPING_LEN;
            data[at] = 0x51;
            data[at + 1] = hid;
        }
        data[BUTTON_MAPPINGS_OFFSET + KEY_MAPPING_LEN] = 0x04; // second mapping: left shift
        data[BUTTON_MAPPINGS_OFFSET + KEY_MAPPING_LEN + 1] = 0xE0;
        if schema == SCHEMA_B {
            data[RAPID_TAP_ENABLE_OFFSET] = 1;
            data[RAPID_TAP_PAIRS_OFFSET] = 0x04;
            data[RAPID_TAP_PAIRS_OFFSET + 1] = 0x05;
            data[RAPID_TAP_PAIRS_OFFSET + 2] = 0x83;
        }
        let macro_offset = if schema == SCHEMA_B {
            MACRO_OFFSET_B
        } else {
            MACRO_OFFSET_A
        };
        data[macro_offset..macro_offset + SYNTH_MACRO_BLOCK.len()].copy_from_slice(&SYNTH_MACRO_BLOCK);
        // Four META-layer macro bindings, in the shape the measurement says GG writes:
        // offset 114 carries 0 (record 0, the list holds it), offset 30 carries 7 and offset 31 carries
        // 11 (both resolve to record 1, the group of MACRO_INDEX_STRIDE indices around 7), and offset 22
        // carries 21 - ordinal 3, past the two records the block holds: the deleted-macro state, with no
        // real profile behind it.
        data[BUTTON_META_MAPPINGS_OFFSET + 114 * KEY_MAPPING_LEN] = 0x71;
        data[BUTTON_META_MAPPINGS_OFFSET + 30 * KEY_MAPPING_LEN] = 0x71;
        data[BUTTON_META_MAPPINGS_OFFSET + 30 * KEY_MAPPING_LEN + 3] = 7;
        data[BUTTON_META_MAPPINGS_OFFSET + 31 * KEY_MAPPING_LEN] = 0x71;
        data[BUTTON_META_MAPPINGS_OFFSET + 31 * KEY_MAPPING_LEN + 3] = 11;
        data[BUTTON_META_MAPPINGS_OFFSET + 22 * KEY_MAPPING_LEN] = 0x71;
        data[BUTTON_META_MAPPINGS_OFFSET + 22 * KEY_MAPPING_LEN + 3] = 21;
        data[guid_offset_for_schema(schema)] = 0xAB;
        let crc_offset = checksum_offset(len);
        let computed = compute_checksum(&data[..crc_offset]);
        data[crc_offset..].copy_from_slice(&computed.to_le_bytes());
        data
    }

    /// The encoder is the inverse of the decoder: a decode report written back must give the same
    /// bytes, on both schemas and with a macro block, an OLED block and Rapid Tap pairs behind it.
    #[test]
    fn encode_writes_back_a_decoded_payload_byte_for_byte() {
        for schema in [SCHEMA_A, SCHEMA_B] {
            let data = synthetic_payload(schema);
            let profile = decode(&data).expect("the synthetic payload decodes");
            let json = profile_json(&profile).expect("the decoded profile serializes");
            let encoded = encode_json(&json).expect("the decode report encodes");
            assert_eq!(
                encoded.len(),
                payload_len_for_schema(schema).unwrap(),
                "schema 0x{schema:04X} encodes to its own payload length"
            );
            assert_eq!(encoded, data, "schema 0x{schema:04X} round-trips");
        }
    }

    /// Macro bytes as a keyboard with GG macros carries them (`Some rebounds`): five packed records this
    /// grammar walks (28, 28, 28, 16, 8 bytes), the zero byte that ends the list, then the byte that stops
    /// it - `03`, not a `0x77` marker. The first 109 bytes are the block as the keyboard wrote them; the
    /// eight behind that byte are ours, standing in for the bytes the report does not ship.
    const GG_MACRO_BYTES: [u8; 117] = [
        0x77, 0x01, 0x05, 0x00, 0x01, 0x01, 0x0F, 0x00, 0x02, 0xE0, 0x01, 0x01, 0x02, 0x06, 0x01, 0x01, 0x04, 0x00,
        0x14, 0x00, 0x02, 0x06, 0x01, 0x00, 0x02, 0xE0, 0x01, 0x00, 0x77, 0x01, 0x05, 0x00, 0x01, 0x01, 0x0F, 0x00,
        0x02, 0xE0, 0x01, 0x01, 0x02, 0x19, 0x01, 0x01, 0x04, 0x00, 0x14, 0x00, 0x02, 0x19, 0x01, 0x00, 0x02, 0xE0,
        0x01, 0x00, 0x77, 0x01, 0x05, 0x00, 0x01, 0x01, 0x0F, 0x00, 0x02, 0xE0, 0x01, 0x01, 0x02, 0x1B, 0x01, 0x01,
        0x04, 0x00, 0x14, 0x00, 0x02, 0x1B, 0x01, 0x00, 0x02, 0xE0, 0x01, 0x00, 0x77, 0x04, 0x02, 0x00, 0x01, 0x01,
        0x0F, 0x00, 0x02, 0x25, 0x01, 0x01, 0x02, 0x25, 0x01, 0x00, 0x77, 0x01, 0x01, 0x00, 0x00, 0x01, 0x0F, 0x00,
        0x00, 0x03, 0x01, 0x01, 0x00, 0x00, 0x01, 0x0F, 0x00,
    ];

    /// A real macro block the record grammar rejects: the write is not refused and nothing is invented -
    /// the engine declares this block a `bytearray` (`lisp/apex_7.lisp:62, 101`), so the report ships its
    /// bytes and the write sends them back verbatim.
    #[test]
    fn a_real_macro_block_the_grammar_rejects_writes_back_verbatim() {
        let mut data = synthetic_payload(SCHEMA_B);
        let at = MACRO_OFFSET_B;
        data[at..at + SYNTH_MACRO_BLOCK.len()].fill(0);
        data[at..at + GG_MACRO_BYTES.len()].copy_from_slice(&GG_MACRO_BYTES);
        let crc_offset = checksum_offset(data.len());
        let computed = compute_checksum(&data[..crc_offset]);
        data[crc_offset..].copy_from_slice(&computed.to_le_bytes());

        let profile = decode(&data).expect("the payload decodes");
        let note = profile
            .macro_note
            .as_ref()
            .expect("the list stops at the byte that is not a marker");
        assert!(
            note.contains("0x03") && note.contains("+109"),
            "the note names the byte and where it sits: {note}"
        );
        assert_eq!(
            profile.macros.len(),
            5,
            "the records before that byte are listed for display"
        );
        let bytes_hex = profile
            .macro_data
            .bytes_hex
            .as_ref()
            .expect("the report carries the block's bytes");
        assert_eq!(
            bytes_hex.split_whitespace().count(),
            MACRO_DATA_LEN,
            "the whole 9,600-byte block ships, not the part that parsed"
        );

        let encoded = encode_json(&profile_json(&profile).expect("json")).expect("the write path accepts this report");
        assert_eq!(encoded, data, "the block's bytes go back exactly as they came");
    }

    /// The last four bytes are always the checksum the encoder computes, never the stored word of
    /// the report, so an image read with a wrong checksum encodes to a self-consistent payload.
    #[test]
    fn encode_recomputes_the_checksum_of_the_bytes_it_writes() {
        let mut data = synthetic_payload(SCHEMA_B);
        let crc_offset = checksum_offset(data.len());
        data[crc_offset] ^= 0xFF;
        let profile = decode(&data).expect("a payload with a wrong stored checksum still decodes");
        assert!(!profile.crc_ok, "the stored word disagrees with the bytes");
        let encoded = encode_json(&profile_json(&profile).expect("json")).expect("encodes");
        assert_eq!(
            payload_diff(&data, &encoded),
            Some(crc_offset),
            "only the checksum word changes"
        );
        assert!(decode(&encoded).expect("re-decode").crc_ok);
    }

    /// A write is framed only for a payload whose stored checksum matches its bytes and whose schema is known.
    #[test]
    fn write_plan_refuses_a_payload_with_a_wrong_checksum_or_unknown_schema() {
        let good = synthetic_payload(SCHEMA_B);
        assert!(write_plan(0, &good).is_ok());

        let mut bad_crc = good.clone();
        let crc_offset = checksum_offset(bad_crc.len());
        bad_crc[crc_offset] ^= 0xFF;
        assert!(write_plan(0, &bad_crc).is_err());

        let mut bogus = good;
        bogus[..2].copy_from_slice(&0x0009u16.to_le_bytes());
        assert!(write_plan(0, &bogus).is_err());
    }

    /// The decoder reports the macro block as a summary plus the records it could walk. A block it
    /// could not walk has bytes behind that note, and writing the records alone would silently drop
    /// them, so the encoder refuses the JSON.
    #[test]
    fn encode_refuses_a_macro_block_the_decoder_could_not_walk() {
        let data = synthetic_payload(SCHEMA_B);
        let profile = decode(&data).expect("decodes");
        let mut value: serde_json::Value =
            serde_json::from_str(&profile_json(&profile).expect("json")).expect("valid JSON");
        // Without the block's bytes the parsed records are the only source of them, and a block the
        // decoder walked as far as it could has bytes that list cannot hold.
        value["macro_data"]
            .as_object_mut()
            .expect("macro block")
            .remove("bytes_hex");
        value["macro_note"] = json!("stopped at offset 2100");
        let json = value.to_string();
        let error = encode_json(&json).expect_err("a noted block without bytes is refused");
        let reason = error.to_string();
        assert!(
            reason.contains("did not parse") && reason.contains("no macro_data.bytes_hex"),
            "the reason names the decode note and the missing bytes: {error}"
        );
    }

    /// A table whose slots are not in payload order would write bindings into the wrong keys, so the
    /// encoder checks the `offset` the report carries against the position it writes.
    #[test]
    fn encode_refuses_a_mapping_table_that_is_not_in_payload_order() {
        let data = synthetic_payload(SCHEMA_B);
        let mut value: serde_json::Value =
            serde_json::from_str(&profile_json(&decode(&data).expect("decodes")).expect("json")).expect("valid JSON");
        value["button_mappings"][1]["offset"] = json!(9);
        let error = encode_json(&value.to_string()).expect_err("an out-of-order table is refused");
        assert!(
            error.to_string().contains("not in payload order"),
            "the reason names the order: {error}"
        );
    }

    #[test]
    fn pull_and_read_frames_come_from_the_typed_commands() {
        let pull = frame_of(
            OnboardPayloadPullCommand {
                fs_id: profile_file_id(0),
                file_offset: 0,
                size: DEVICE_RESOURCE_BUFFER_LEN as u16,
            },
            OnboardPayloadPullCommand::FRAME_LEN,
        );
        assert_eq!(pull[0], CommandCode::OnboardPayloadPull as u8);
        assert_eq!(
            u16::from_le_bytes([pull[2], pull[3]]),
            PROFILE_FS_BASE,
            "the file id sits behind the header"
        );
        assert_eq!(pull.len(), OnboardPayloadPullCommand::FRAME_LEN);
        let read = frame_of(
            OnboardBufferReadCommand {
                buffer_offset: 0,
                size: PAYLOAD_READ_LEN as u16,
            },
            OnboardBufferReadCommand::FRAME_LEN,
        );
        assert_eq!(read[0], CommandCode::OnboardBufferRead as u8);
        assert_eq!(read.len(), OnboardBufferReadCommand::FRAME_LEN);
    }

    #[test]
    fn pulls_walk_the_resource_buffer_and_carry_the_slot_file_id() {
        let slot = 3;
        assert_eq!(profile_file_id(slot), 0x0204);
        for n in 0..PULLS_PER_FILE {
            let pull = frame_of(
                OnboardPayloadPullCommand {
                    fs_id: profile_file_id(slot),
                    file_offset: (n * DEVICE_RESOURCE_BUFFER_LEN) as u32,
                    size: DEVICE_RESOURCE_BUFFER_LEN as u16,
                },
                OnboardPayloadPullCommand::FRAME_LEN,
            );
            assert_eq!(
                u16::from_le_bytes([pull[2], pull[3]]),
                0x0204,
                "every pull carries the slot file id"
            );
            assert_eq!(
                u32::from_le_bytes([pull[4], pull[5], pull[6], pull[7]]),
                (n * DEVICE_RESOURCE_BUFFER_LEN) as u32,
                "pull n fills the buffer at file offset n * DEVICE_RESOURCE_BUFFER_LEN"
            );
            assert_eq!(
                u16::from_le_bytes([pull[8], pull[9]]),
                DEVICE_RESOURCE_BUFFER_LEN as u16,
                "pull size"
            );
        }
        for n in 0..READS_PER_PULL {
            let read = frame_of(
                OnboardBufferReadCommand {
                    buffer_offset: (n * PAYLOAD_READ_LEN) as u16,
                    size: PAYLOAD_READ_LEN as u16,
                },
                OnboardBufferReadCommand::FRAME_LEN,
            );
            assert_eq!(
                u16::from_le_bytes([read[2], read[3]]),
                (n * PAYLOAD_READ_LEN) as u16,
                "read n asks for the n-th window of the resource buffer"
            );
            assert_eq!(u16::from_le_bytes([read[4], read[5]]), PAYLOAD_READ_LEN as u16);
        }
        assert_eq!(
            PROFILE_FILE_BYTES,
            PULLS_PER_FILE * DEVICE_RESOURCE_BUFFER_LEN,
            "the fixed read covers 12 x 1024 file bytes"
        );
    }

    #[test]
    fn reply_header_is_echoed_and_payload_starts_at_six() {
        let mut reply = vec![0x83u8, 0x00, 0x00, 0x02, 0x00, 0x02];
        reply.extend((1u8..=64).cycle().take(636));
        let parsed = parse_read_reply(&reply, 512).unwrap();
        assert_eq!(parsed.report_id, 0x83);
        assert_eq!(parsed.offset, 512);
        assert_eq!(parsed.size, 512);
        assert_eq!(parsed.data.len(), 512);
        assert_eq!(parsed.data[0], 1);
        assert!(!parsed.all_zero);
        assert!(!parsed.all_ff);
    }

    #[test]
    fn capture_reply_payload_starts_behind_the_report_id_byte() {
        // The GET_REPORT 0x0300 answer to `83 00 00 …` on file 0x0201 in the capture:
        // report ID 03, then the schema word 0x000B, then the ASCII profile name 2 bytes behind it.
        let mut reply = vec![0x03u8, 0x0b, 0x00];
        reply.extend(b"Config 1");
        reply.resize(READ_REPORT_LEN, 0);
        let parsed = parse_read_reply(&reply, 512).unwrap();
        assert_eq!(parsed.report_id, 0x03, "no header is echoed");
        assert!(!parsed.header);
        assert_eq!(schema_of(&parsed.data), SCHEMA_B);
        assert_eq!(&parsed.data[2..10], b"Config 1");
        assert_eq!(parsed.data.len(), 512);
        assert!(!parsed.all_zero);
    }

    #[test]
    fn a_reply_too_short_for_its_reserved_byte_is_an_error_not_a_panic() {
        assert!(parse_read_reply(&[], 512).is_err());
        assert!(parse_read_reply(&[0x03], 512).is_err());
        assert!(parse_read_reply(&[0x03, 0x00], 512).is_ok());
    }

    #[test]
    fn a_garbage_page_zeroes_only_itself_and_the_next_read_of_the_pull_still_lands() {
        let record = record(0, |read| {
            if read == 0 {
                vec![0xFFu8; PAYLOAD_READ_LEN]
            } else {
                vec![0x5Au8; PAYLOAD_READ_LEN]
            }
        });
        let image = ProfileImage::assemble(0, vec![record]).unwrap();
        assert!(
            image.data[..PAYLOAD_READ_LEN].iter().all(|b| *b == 0),
            "the garbage page is zeroed"
        );
        assert_eq!(
            image.data[PAYLOAD_READ_LEN], 0x5A,
            "the valid page behind it is kept at its own offset"
        );
        assert_eq!(image.stats.garbage_reads, 1);
    }

    #[test]
    fn a_page_starting_with_four_ff_bytes_is_counted_as_garbage() {
        let mut reply = vec![0x83u8, 0x00, 0x00, 0x00, 0x00, 0x04];
        reply.extend([0xFFu8, 0xFF, 0xFF, 0xFF, 0x01, 0x02]);
        let parsed = parse_read_reply(&reply, 6).unwrap();
        assert!(parsed.all_ff);
        assert!(!parsed.all_zero);
    }

    fn reply_report(prefix: [u8; 6], fill: u8, len: usize) -> Vec<u8> {
        let mut reply = prefix.to_vec();
        reply.extend(std::iter::repeat_n(fill, len));
        reply
    }

    #[test]
    fn zero_filled_reply_is_flagged() {
        let reply = reply_report([0x83, 0, 0, 0, 0, 0x02], 0, 636);
        let parsed = parse_read_reply(&reply, 512).unwrap();
        assert!(parsed.all_zero);
        assert_eq!(parsed.data.len(), 512);
    }

    #[test]
    fn reply_mismatch_is_reported() {
        let reply = reply_report([0x83, 0, 0x02, 0x00, 0, 0x02], 0, 636);
        let parsed = parse_read_reply(&reply, 512).unwrap();
        let mismatch = reply_mismatch(&parsed, 0, 512).expect("offset mismatch");
        assert!(mismatch.contains("offset"), "{mismatch}");
        let bad_report = reply_report([0x03, 0, 0, 0, 0, 0x02], 0, 636);
        let parsed = parse_read_reply(&bad_report, 512).unwrap();
        let mismatch = reply_mismatch(&parsed, 0, 512).expect("report mismatch");
        assert!(mismatch.contains("report"), "{mismatch}");
        assert!(reply_mismatch(&parsed, 0, 512).is_some());
    }

    #[test]
    fn short_reply_is_a_short_read() {
        let reply = reply_report([0x83, 0, 0, 0, 0, 0x02], 7, 10);
        let parsed = parse_read_reply(&reply, 512).unwrap();
        assert_eq!(parsed.data.len(), 10, "a truncated report yields a short read");
    }

    fn record(chunk: usize, data: impl Fn(usize) -> Vec<u8>) -> ChunkRecord {
        ChunkRecord {
            index: chunk,
            fs_id: profile_file_id(0),
            file_offset: (chunk * DEVICE_RESOURCE_BUFFER_LEN) as u32,
            ack_report_id: 0x85,
            ack_hex: "85 00".to_string(),
            reads: (0..READS_PER_PULL)
                .map(|read| {
                    let data = data(read);
                    let all_zero = data.iter().all(|b| *b == 0);
                    ReadReply {
                        report_id: 0x83,
                        reserved: 0,
                        header: true,
                        offset: (read * PAYLOAD_READ_LEN) as u16,
                        size: PAYLOAD_READ_LEN as u16,
                        all_ff: data.starts_with(&[0xFF, 0xFF, 0xFF, 0xFF]),
                        all_zero,
                        data,
                    }
                })
                .collect(),
        }
    }

    #[test]
    fn assemble_scatters_reads_by_file_offset() {
        let records = (0..PULLS_PER_FILE)
            .map(|chunk| {
                record(chunk, |read| {
                    vec![(chunk * READS_PER_PULL + read + 1) as u8; PAYLOAD_READ_LEN]
                })
            })
            .collect::<Vec<_>>();
        let image = ProfileImage::assemble(0, records).unwrap();
        assert_eq!(image.data.len(), PROFILE_FILE_BYTES);
        assert_eq!(image.data[0], 1);
        assert_eq!(image.data[PAYLOAD_READ_LEN], 2);
        assert_eq!(image.data[DEVICE_RESOURCE_BUFFER_LEN], 3);
        assert_eq!(
            image.data[PROFILE_FILE_BYTES - 1],
            (PULLS_PER_FILE * READS_PER_PULL) as u8
        );
        assert_eq!(image.stats.chunks_read, PULLS_PER_FILE);
        assert_eq!(image.stats.reads_read, PULLS_PER_FILE * READS_PER_PULL);
        assert_eq!(image.stats.header_mismatches, 0);
        assert_eq!(image.stats.zero_chunks, 0);
        assert_eq!(image.stats.covered_len, PROFILE_FILE_BYTES);
        assert_eq!(image.profile_slot, 0);
    }

    #[test]
    fn assemble_drops_reads_past_the_file_and_clips_the_last_one() {
        let mut records = (0..PULLS_PER_FILE)
            .map(|chunk| record(chunk, |_| vec![0u8; PAYLOAD_READ_LEN]))
            .collect::<Vec<_>>();
        // One pull beyond the file: its reads are recorded and stay out of the assembled bytes.
        records.push(ChunkRecord {
            index: PULLS_PER_FILE,
            fs_id: profile_file_id(0),
            file_offset: PROFILE_FILE_BYTES as u32,
            ack_report_id: 0x85,
            ack_hex: "85 00".to_string(),
            reads: (0..READS_PER_PULL)
                .map(|read| ReadReply {
                    report_id: 0x83,
                    reserved: 0,
                    header: true,
                    offset: (read * PAYLOAD_READ_LEN) as u16,
                    size: PAYLOAD_READ_LEN as u16,
                    data: vec![0xAA_u8; PAYLOAD_READ_LEN],
                    all_zero: false,
                    all_ff: false,
                })
                .collect(),
        });
        let image = ProfileImage::assemble(0, records).unwrap();
        assert_eq!(image.data.len(), PROFILE_FILE_BYTES);
        assert!(
            image.data.iter().all(|b| *b == 0),
            "the extra pull contributes no bytes"
        );
        assert_eq!(image.stats.chunks_read, PULLS_PER_FILE + 1);
        assert_eq!(
            image.stats.reads_read,
            (PULLS_PER_FILE + 1) * READS_PER_PULL,
            "the extra reads are still tallied in the statistics"
        );
    }

    #[test]
    fn only_the_profile_slot_is_selectable() {
        for slot in 0..=MAX_PROFILE_INDEX {
            assert!(validate_profile(slot).is_ok(), "slot {slot} is readable");
        }
        let err = validate_profile(MAX_PROFILE_INDEX + 1).unwrap_err().to_string();
        assert!(err.contains("highest slot"), "{err}");
        assert!(validate_profile(255).is_err());
    }

    #[test]
    fn decode_schema_a_payload() {
        let data = synthetic_payload(SCHEMA_A);
        let profile = decode(&data).unwrap();
        assert_eq!(profile.schema, SCHEMA_A);
        assert_eq!(profile.schema_name, "a");
        assert_eq!(profile.name, "Pro7");
        assert_eq!(profile.meta_toggle_hid, 0x62);
        assert_eq!(profile.lighting.lock_key_color, Color { r: 0xFF, g: 0, b: 0 });
        assert_eq!(profile.lighting.idle_timeout_ms, 1500);
        assert_eq!(profile.rapid_tap, None, "schema A has no Rapid Tap block");
        assert_eq!(profile.button_mappings.len(), 128);
        assert_eq!(profile.button_mappings[1].function, 0x04);
        assert_eq!(profile.button_mappings[1].function_name, "mouse_button_4");
        assert_eq!(profile.macro_data.offset, MACRO_OFFSET_A);
        assert_eq!(profile.macro_data.len, MACRO_DATA_LEN);
        assert_eq!(
            profile.macro_data.nonzero,
            SYNTH_MACRO_BLOCK.iter().filter(|b| **b != 0).count()
        );
        assert!(profile.guid.starts_with("AB"), "{}", profile.guid);
        assert_eq!(profile.guid_padding, "00 00 00");
        assert!(profile.crc_ok, "stored crc matches the computed one");
        assert_eq!(profile.crc_region_len, PAYLOAD_LEN_A - 4);
    }

    #[test]
    fn block_offsets_are_the_engine_struct_stream() {
        // GG's `profile_all_settings_schema_b_struct` byte comments (apex_7.lisp).
        assert_eq!(KEY_MAPPING_LEN, 5, "function byte + four key codes");
        assert_eq!(BUTTON_MAPPINGS_OFFSET, 85);
        assert_eq!(WHEEL_MAPPINGS_OFFSET, 725);
        assert_eq!(BUTTON_META_MAPPINGS_OFFSET, 735);
        assert_eq!(WHEEL_META_MAPPINGS_OFFSET, 1375);
        assert_eq!(
            OLED_OFFSET, 1385,
            "OLED sits behind the wheel META table, not behind the GUID"
        );
        assert_eq!(RAPID_TAP_ENABLE_OFFSET, 2025);
        assert_eq!(RAPID_TAP_PAIRS_OFFSET, 2026);
        assert_eq!(RAPID_TAP_PAIR_LEN, 5, "onboard_rapid_tap_pair has two reserved bytes");
        assert_eq!(MACRO_OFFSET_A, 2025);
        assert_eq!(MACRO_OFFSET_B, 2076);
        assert_eq!(GUID_OFFSET_A, 11_625);
        assert_eq!(GUID_OFFSET_B, 11_676);
        assert_eq!(checksum_offset(PAYLOAD_LEN_A), 11_644);
        assert_eq!(checksum_offset(PAYLOAD_LEN_B), 11_692);
        assert_eq!(
            GUID_OFFSET_A + GUID_LEN + GUID_PADDING_LEN,
            checksum_offset(PAYLOAD_LEN_A),
            "schema A pads three bytes between the GUID and the checksum; schema B does not"
        );
        assert_eq!(GUID_OFFSET_B + GUID_LEN, checksum_offset(PAYLOAD_LEN_B));
    }

    #[test]
    fn factory_defaults_are_not_reported_as_bindings() {
        let profile = decode(&synthetic_payload(SCHEMA_B)).unwrap();
        let kinds: Vec<&str> = profile.button_mappings.iter().map(|m| m.binding).collect();
        assert_eq!(kinds[0], "self", "slot 0 is Esc bound to Esc");
        assert_eq!(
            profile.button_mappings[0].hid_code,
            Some(0x29),
            "slot 0 belongs to Esc per GG's offset table"
        );
        assert_eq!(kinds[1], "custom", "the synthetic remap is the only custom binding");
        let unmapped = [6usize, 7, 29, 69, 102, 103, 110, 111];
        for offset in unmapped {
            assert_eq!(profile.button_mappings[offset].binding, "none");
            assert_eq!(
                profile.button_mappings[offset].hid_code, None,
                "slot {offset} is a slot this keyboard has no key for"
            );
        }
        let customs = kinds.iter().filter(|k| **k == "custom").count();
        assert_eq!(customs, 1, "a stock profile reads as a stock profile");
    }

    #[test]
    fn offset_and_hid_tables_invert_each_other() {
        let mut mapped = 0;
        for offset in 0..BUTTON_MAPPING_COUNT {
            if let Some(hid) = offset_to_hid(offset) {
                mapped += 1;
                assert_eq!(hid_to_offset(hid), Some(offset), "slot {offset} <-> 0x{hid:02X}");
            }
        }
        assert_eq!(mapped, 120, "GG's device-offset-lookup-table maps 120 of the 128 slots");
        assert_eq!(
            hid_to_offset(0x65),
            None,
            "Select has no mapping slot, so it cannot be remapped on-board"
        );
        assert_eq!(hid_to_offset(0xF1), Some(120), "the function-code slots");
    }

    #[test]
    fn checksum_is_the_payload_tail_not_the_image_tail() {
        // A read image is padded with whole resource buffers behind the payload; the checksum word
        // belongs to the payload, so decoding an image must not read the padding as the checksum.
        let mut image = vec![0u8; PROFILE_FILE_BYTES];
        let payload = synthetic_payload(SCHEMA_B);
        image[..payload.len()].copy_from_slice(&payload);
        let profile = decode(&image).expect("an image is a payload with a padded tail");
        assert!(
            profile.crc_ok,
            "stored {:08X} computed {:08X}",
            profile.crc_stored, profile.crc_computed
        );
        assert_eq!(profile.crc_region_len, PAYLOAD_LEN_B - 4);
        assert_eq!(profile.macro_data.len, MACRO_DATA_LEN);
        assert_eq!(
            profile.guid,
            hex_of(&payload[GUID_OFFSET_B..GUID_OFFSET_B + GUID_LEN]),
            "the GUID comes from the payload, not from the padding"
        );
    }

    #[test]
    fn the_region_behind_the_payload_is_recorded_by_a_read_image() {
        // The read fills the whole profile region; the payload is the head of it. The decode report names
        // what sits behind the payload as a block summary, and a payload-only file names nothing.
        let mut image = vec![0u8; PROFILE_FILE_BYTES];
        let payload = synthetic_payload(SCHEMA_B);
        image[..payload.len()].copy_from_slice(&payload);
        image[PAYLOAD_LEN_B] = 0xFE;
        let with_region = decode(&image).expect("an image is a payload with a padded tail");
        assert_eq!(with_region.payload_len, PAYLOAD_LEN_B);
        assert_eq!(with_region.extra_len, PROFILE_FILE_BYTES - PAYLOAD_LEN_B);
        assert_eq!(
            with_region.extra_head.expect("the region has bytes"),
            format!("FE{}", " 00".repeat(15))
        );
        assert_eq!(
            with_region.extra_fnv1a_64.as_deref(),
            Some(fnv1a_64(&image[PAYLOAD_LEN_B..]).as_str())
        );
        let payload_only = decode(&payload).expect("a payload alone decodes");
        assert_eq!(payload_only.extra_len, 0, "a payload-only report records no region");
        assert_eq!(payload_only.extra_fnv1a_64, None);
        let region = extra_region(&image).expect("an image longer than its payload has a region");
        assert_eq!(region.offset, PAYLOAD_LEN_B);
        assert_eq!(region.len, PROFILE_FILE_BYTES - PAYLOAD_LEN_B);
        assert_eq!(
            extra_region(&payload),
            None,
            "a payload-only image has no region to record"
        );
        assert_eq!(
            extra_region(&[0u8; PROFILE_FILE_BYTES]),
            None,
            "an unsaved slot (schema word 0) names no payload length, so no region"
        );
    }

    #[test]
    fn a_read_stamps_the_slot_it_ran_on() {
        let image = ProfileImage::assemble(2, Vec::new()).expect("an empty record set assembles an empty image");
        let mut profile = decode(&synthetic_payload(SCHEMA_B)).unwrap();
        assert_eq!(profile.slot, None, "a decode of bytes names no slot");
        image.stamp_identity(&mut profile);
        assert_eq!(profile.slot, Some(2), "the read's report names the slot it read");
    }

    #[test]
    fn decode_schema_b_payload_carries_rapid_tap() {
        let data = synthetic_payload(SCHEMA_B);
        let profile = decode(&data).unwrap();
        assert_eq!(profile.schema_name, "b");
        let rapid = profile.rapid_tap.expect("schema B has a Rapid Tap block");
        assert_eq!(rapid.enable, 1);
        assert_eq!(rapid.pairs.len(), 10);
        assert_eq!(rapid.pairs[0].hid_1, 0x04);
        assert_eq!(rapid.pairs[0].hid_2, 0x05);
        assert_eq!(rapid.pairs[0].mode, 3);
        assert!(rapid.pairs[0].report_both_keys);
        assert_eq!(profile.macro_data.offset, MACRO_OFFSET_B);
        assert!(profile.guid_padding.is_empty(), "schema B has no GUID padding");
        assert!(profile.crc_ok);
        assert_eq!(profile.crc_region_len, PAYLOAD_LEN_B - 4);
    }

    #[test]
    fn decode_reports_a_crc_disagreement() {
        let mut data = synthetic_payload(SCHEMA_A);
        let crc_offset = data.len() - 4;
        data[crc_offset] ^= 0xFF;
        let profile = decode(&data).unwrap();
        assert!(!profile.crc_ok, "a flipped stored byte must disagree");
        assert_ne!(profile.crc_stored, profile.crc_computed);
        assert!(summary_line(&profile).contains("MISMATCH"));
    }

    #[test]
    fn decode_rejects_unknown_schema_and_short_payloads() {
        let data = synthetic_payload(SCHEMA_A);
        assert!(
            decode(&data[..data.len() - 1]).is_err(),
            "a payload shorter than the schema needs is rejected"
        );
        let mut bogus = vec![0u8; PAYLOAD_LEN_A];
        bogus[0..2].copy_from_slice(&0x0009u16.to_le_bytes());
        assert!(decode(&bogus).is_err(), "an unknown schema is rejected");
        assert!(payload_len_for_schema(0x0009).is_err());
        assert_eq!(schema_of(&[0x0B, 0x00]), SCHEMA_B);
        assert_eq!(schema_of(&[0x01]), 0, "a 1-byte payload has no schema word");
    }

    #[test]
    fn json_output_is_valid() {
        let data = synthetic_payload(SCHEMA_B);
        let profile = decode(&data).unwrap();
        let text = profile_json(&profile).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["schema_name"], "b");
        assert_eq!(value["name"], "Pro7");
        assert_eq!(value["lighting"]["lock_key_color"]["r"], 255);
        assert_eq!(value["button_mappings"].as_array().unwrap().len(), 128);
        assert!(value["rapid_tap"].is_object());

        let image = ProfileImage::assemble(0, {
            let reads_per_pull = READS_PER_PULL;
            (0..PULLS_PER_FILE)
                .map(|chunk| ChunkRecord {
                    index: chunk,
                    fs_id: 0x0201,
                    file_offset: (chunk * DEVICE_RESOURCE_BUFFER_LEN) as u32,
                    ack_report_id: 0x85,
                    ack_hex: "85 00".to_string(),
                    reads: (0..reads_per_pull)
                        .map(|read| {
                            let i = chunk * reads_per_pull + read;
                            let start = i * PAYLOAD_READ_LEN;
                            let data_slice = data
                                .get(start..start + PAYLOAD_READ_LEN)
                                .map(|s| s.to_vec())
                                .unwrap_or_else(|| vec![0u8; PAYLOAD_READ_LEN]);
                            ReadReply {
                                report_id: 0x83,
                                reserved: 0,
                                header: true,
                                offset: (read * PAYLOAD_READ_LEN) as u16,
                                size: PAYLOAD_READ_LEN as u16,
                                all_zero: data_slice.iter().all(|b| *b == 0),
                                all_ff: false,
                                data: data_slice,
                            }
                        })
                        .collect(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap();
        let raw = to_json(&image).unwrap();
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(value["schema_decoded"], "b");
        assert_eq!(value["profile_slot"], 0);
        assert_eq!(value["fs_id"], "0x0201");
        assert_eq!(value["data_len"], PROFILE_FILE_BYTES);
    }

    /// The dump format shared by `onboard read`, `onboard decode` and `tools/profile-gui`: the
    /// top-level keys in report order, and the keys of every nested object the GUI reads. Pinned
    /// because a renamed field silently drops data in the GUI and in any dump a user diffs.
    #[test]
    fn profile_json_keeps_the_documented_report_layout() {
        let data = synthetic_payload(SCHEMA_B);
        let profile = decode(&data).unwrap();
        let text = profile_json(&profile).unwrap();
        let top: Vec<String> = text
            .lines()
            .filter_map(|line| {
                let line = line.strip_prefix("  ")?;
                line.strip_prefix('"')?.split('"').next().map(str::to_string)
            })
            .collect();
        assert_eq!(
            top,
            [
                "schema",
                "schema_name",
                "name",
                "name_hex",
                "lighting",
                "meta_toggle_hid",
                "button_mappings",
                "wheel_mappings",
                "button_meta_mappings",
                "wheel_meta_mappings",
                "oled",
                "rapid_tap",
                "macro_data",
                "macros",
                "guid",
                "guid_padding",
                "crc_stored",
                "crc_computed",
                "crc_ok",
                "crc_region_len",
                // A decode of bytes stamps no identity, no slot and no region: those fields are skipped.
                // The payload length it used is always named.
                "payload_len",
            ]
        );

        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        // serde_json parses objects sorted, so every expectation here is a sorted key list.
        let keys_of = |value: &serde_json::Value| -> Vec<String> {
            let node = value.as_array().map_or(value, |array| &array[0]);
            node.as_object()
                .map(|object| object.keys().cloned().collect())
                .unwrap_or_default()
        };
        assert_eq!(
            keys_of(&value["lighting"]),
            [
                "disabled_win_key_color",
                "idle_mode_dimmer",
                "idle_timeout_ms",
                "lock_key_color",
                "lock_key_hid_codes",
                "meta_key_background_dimmer",
                "meta_key_color",
                "meta_mapped_elements",
                "oled_illumination_options",
                "win_key_hid_code",
            ]
        );
        assert_eq!(keys_of(&value["lighting"]["lock_key_color"]), ["b", "g", "r"]);
        for table in [
            "button_mappings",
            "wheel_mappings",
            "button_meta_mappings",
            "wheel_meta_mappings",
        ] {
            assert_eq!(
                keys_of(&value[table]),
                [
                    "binding",
                    "function",
                    "function_name",
                    "hid_code",
                    "key_codes",
                    "macro_index",
                    "offset"
                ],
                "{table} entries"
            );
        }
        assert_eq!(
            keys_of(&value["macros"]),
            [
                "action_count",
                "actions",
                "flags",
                "id",
                "index",
                "interval_ms",
                "mode",
                "mode_name",
                "offset",
                "ordinal",
                "raw",
                "record_bytes",
            ],
            "the packed macro records the GUI reads"
        );
        assert_eq!(keys_of(&value["macros"][0]["actions"]), ["key", "kind", "ms", "word"]);
        assert_eq!(
            keys_of(&value["oled"]),
            ["bytes_hex", "fnv1a_64", "head", "len", "nonzero", "offset"]
        );
        let oled_bytes = value["oled"]["bytes_hex"].as_str().expect("oled bytes");
        assert_eq!(oled_bytes.split(' ').count(), OLED_LEN);
        assert_eq!(
            value["oled"]["head"].as_str().expect("oled head"),
            oled_bytes.split(' ').take(16).collect::<Vec<_>>().join(" ").as_str()
        );
        assert_eq!(keys_of(&value["rapid_tap"]), ["enable", "pairs"]);
        assert_eq!(
            keys_of(&value["rapid_tap"]["pairs"]),
            ["hid_1", "hid_2", "mode", "report_both_keys", "reserved"]
        );
        assert_eq!(
            keys_of(&value["macro_data"]),
            ["bytes_hex", "fnv1a_64", "head", "len", "nonzero", "offset"]
        );
        let macro_bytes = value["macro_data"]["bytes_hex"].as_str().expect("macro block bytes");
        assert_eq!(
            macro_bytes.split(' ').count(),
            MACRO_DATA_LEN,
            "the engine's opaque bytearray ships whole, so a write can send it back verbatim"
        );
        assert_eq!(value["button_mappings"].as_array().unwrap().len(), 128);
        assert_eq!(value["wheel_mappings"].as_array().unwrap().len(), 2);
        assert_eq!(value["button_meta_mappings"].as_array().unwrap().len(), 128);
        assert_eq!(value["wheel_meta_mappings"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn image_diff_finds_the_first_difference() {
        let record = |first: u8| ChunkRecord {
            index: 0,
            fs_id: profile_file_id(0),
            file_offset: 0,
            ack_report_id: 0x85,
            ack_hex: "85".to_string(),
            reads: (0..(DEVICE_RESOURCE_BUFFER_LEN / PAYLOAD_READ_LEN) * PULLS_PER_FILE)
                .map(|i| ReadReply {
                    report_id: 0x83,
                    reserved: 0,
                    header: true,
                    offset: ((i % 2) * PAYLOAD_READ_LEN) as u16,
                    size: PAYLOAD_READ_LEN as u16,
                    data: vec![if i == 3 { first } else { 0u8 }; PAYLOAD_READ_LEN],
                    all_zero: i != 3,
                    all_ff: false,
                })
                .collect(),
        };
        let a = ProfileImage::assemble(0, vec![record(1)]).unwrap();
        let b = ProfileImage::assemble(0, vec![record(1)]).unwrap();
        let c = ProfileImage::assemble(0, vec![record(2)]).unwrap();
        assert_eq!(image_diff(&a, &b), None);
        assert_eq!(image_diff(&a, &c), Some(3 * PAYLOAD_READ_LEN));
        assert_eq!(a.diff(&c), Some(3 * PAYLOAD_READ_LEN));
    }

    #[test]
    fn dump_names_come_from_the_profile_name() {
        assert_eq!(profile_file_stem("Config 1").as_deref(), Some("config_1"));
        assert_eq!(profile_file_stem("  My Profile 23  ").as_deref(), Some("my_profile_23"));
        assert_eq!(
            profile_file_stem("Config 1 (v2)!").as_deref(),
            Some("config_1__v2"),
            "trailing separators are dropped"
        );
        assert_eq!(profile_file_stem(""), None, "an unsaved slot answers with a zero name");
        assert_eq!(profile_file_stem("Конфиг"), None, "no ASCII to build a name from");
        assert_eq!(
            profile_file_stem("Конфиг 1").as_deref(),
            Some("1"),
            "a digit survives on its own"
        );
        assert_eq!(profile_dump_name("Config 1", 1), "config_1.json");
        assert_eq!(profile_dump_name("", 3), "profile_config_3.json");
    }

    #[test]
    fn hex_dump_is_offsetted() {
        let data = synthetic_payload(SCHEMA_A);
        let dump = hex_dump(&data, 0, 32);
        let lines: Vec<&str> = dump.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("000000  0A 00 50 72 6F"), "{}", lines[0]);
        assert!(lines[1].starts_with("000010  "), "{}", lines[1]);
        assert!(hex_dump(&data, 10, 5).is_empty(), "an inverted range dumps nothing");
    }

    #[test]
    fn a_packed_macro_block_parses_into_its_two_records() {
        let mut data = vec![0u8; MACRO_DATA_LEN];
        data[..SYNTH_MACRO_BLOCK.len()].copy_from_slice(&SYNTH_MACRO_BLOCK);
        let (macros, note) = decode_macros(&data, 0, data.len());
        assert_eq!(note, None, "136 bytes of records then the zero pad must parse clean");
        assert_eq!(macros.len(), 2);
        let repeat = &macros[0];
        assert_eq!(
            (
                repeat.offset,
                repeat.index,
                repeat.ordinal,
                repeat.mode,
                repeat.id,
                repeat.flags
            ),
            (0, 0, 0, 4, 9, 0x0101)
        );
        assert_eq!((repeat.mode_name, repeat.interval_ms), ("repeat", 5000));
        assert_eq!(
            repeat.actions,
            vec![
                MacroAction {
                    kind: "press",
                    key: Some(0x04),
                    word: Some(4),
                    ms: Some(1)
                },
                MacroAction {
                    kind: "release",
                    key: Some(0x04),
                    word: None,
                    ms: None
                },
            ],
            "press `a` for 1 ms and release it, on a repeat record"
        );
        let once = &macros[1];
        assert_eq!(
            (once.index, once.ordinal),
            (MACRO_INDEX_STRIDE, 1),
            "the second record of the list is bound by the indices of its group, 7..13"
        );
        assert_eq!(
            (once.offset, once.index, once.mode, once.id, once.interval_ms),
            (20, MACRO_INDEX_STRIDE, 1, 11, 15),
            "the second record of the list is bound by the indices of its group"
        );
        assert_eq!(once.mode_name, "once");
        assert_eq!(once.actions.len(), 20, "7 key pairs and 6 delays");
        assert_eq!(once.actions.iter().filter(|a| a.kind == "press").count(), 7);
        assert_eq!(once.actions.iter().filter(|a| a.kind == "release").count(), 7);
        assert_eq!(once.actions.iter().filter(|a| a.kind == "delay").count(), 6);
        assert_eq!(
            macros.iter().map(|m| m.raw.split_whitespace().count()).sum::<usize>(),
            SYNTH_MACRO_BLOCK.len(),
            "the two records account for exactly the 136 bytes"
        );
    }

    #[test]
    fn a_meta_layer_macro_binding_names_the_record_it_points_at() {
        let profile = decode(&synthetic_payload(SCHEMA_B)).unwrap();
        assert_eq!(profile.macro_note, None, "the fixture block parses clean");
        let insert = profile
            .button_meta_mappings
            .iter()
            .find(|mapping| mapping.hid_code == Some(0x49))
            .expect("the fixture binds META + Insert");
        assert_eq!((insert.function, insert.function_name.as_str()), (0x71, "macro"));
        assert_eq!(insert.key_codes, "00 00 00 00");
        assert_eq!(insert.macro_index, Some(0));
        let index = usize::from(insert.macro_index.expect("the binding carries its index"));
        let target = profile
            .macros
            .iter()
            .find(|rec| rec.ordinal == index / MACRO_INDEX_STRIDE)
            .expect("the binding resolves to the record its index names");
        assert_eq!((target.mode_name, target.interval_ms), ("repeat", 5000));
        assert_eq!(
            target.actions[0].key,
            Some(0x04),
            "the record names its keys by HID code, not by mapping slot"
        );
        assert_eq!(
            (target.record_bytes, target.action_count),
            (20, 2),
            "the first record fills 20 bytes with two actions"
        );
        assert_eq!(
            profile.macros.iter().map(|rec| rec.record_bytes).sum::<usize>(),
            SYNTH_MACRO_BLOCK.len(),
            "the packed records fill 20 + 116 of the block, with no gap and no length word between them"
        );
        // Two bindings in the group of one record: 7 and 11 both name the record at ordinal 1.
        let group: Vec<(usize, u8, usize)> = profile
            .button_meta_mappings
            .iter()
            .filter(|mapping| mapping.offset == 30 || mapping.offset == 31)
            .map(|mapping| {
                (
                    mapping.offset,
                    mapping.macro_index.expect("both carry an index"),
                    usize::from(mapping.macro_index.expect("both carry an index")) / MACRO_INDEX_STRIDE,
                )
            })
            .collect();
        assert_eq!(
            group,
            vec![(30, 7, 1), (31, 11, 1)],
            "a binding index is 7 times the record position, so 7 and 11 bind the same record"
        );
        assert_eq!(
            profile
                .macros
                .iter()
                .find(|rec| rec.ordinal == 1)
                .expect("record 1 is in the block")
                .mode_name,
            "once",
            "the record 7 and 11 bind is the second record of the block"
        );
        // The binding whose ordinal is past the record list: the record is gone, the binding stays.
        let ghost = profile
            .button_meta_mappings
            .iter()
            .find(|mapping| mapping.offset == 22)
            .expect("the fixture binds a second META key to index 21");
        assert_eq!(
            (ghost.function, ghost.key_codes.as_str(), ghost.macro_index),
            (0x71, "00 00 15 00", Some(21))
        );
        assert!(
            profile.macros.iter().all(|rec| rec.ordinal != 21 / MACRO_INDEX_STRIDE),
            "ordinal 3 is past the list, so the binding points at a deleted macro"
        );

        // Every non-macro slot leaves the field empty.
        assert!(
            profile
                .button_mappings
                .iter()
                .filter(|mapping| mapping.function != 0x71)
                .all(|mapping| mapping.macro_index.is_none())
        );
    }

    #[test]
    fn a_macro_block_is_walked_past_the_gaps_a_binding_can_name() {
        // The shape of `some_rebounds.json`: records packed at the head of the block, a zero gap, then
        // more records - a binding index of 117 needs the record at ordinal 16, so the scan cannot stop
        // at the first gap.
        let mut data = vec![0u8; MACRO_DATA_LEN];
        data[..SYNTH_MACRO_BLOCK.len()].copy_from_slice(&SYNTH_MACRO_BLOCK);
        let deep = 300;
        data[deep..deep + 20].copy_from_slice(&SYNTH_MACRO_BLOCK[..20]);
        let (macros, note) = decode_macros(&data, 0, data.len());
        assert_eq!(note, None, "the gap is an empty slot, not a parse failure");
        assert_eq!(macros.len(), 3, "the two packed records and the one behind the gap");
        assert_eq!(
            macros.iter().map(|rec| rec.offset).collect::<Vec<_>>(),
            vec![0, 20, deep],
            "the scan walks the block in payload order"
        );
        assert_eq!(
            macros.iter().map(|rec| rec.index).collect::<Vec<_>>(),
            vec![0, MACRO_INDEX_STRIDE, 2 * MACRO_INDEX_STRIDE],
            "the binding index is the position in the list times the stride, gaps included"
        );
        assert!(
            macros.iter().all(|rec| rec.index / MACRO_INDEX_STRIDE == rec.ordinal),
            "index / stride is the record, which is how a binding resolves"
        );
    }

    #[test]
    fn a_macro_block_is_reported_when_it_does_not_parse() {
        let mut data = vec![0u8; MACRO_DATA_LEN];
        data[0] = 0x01;
        data[1] = 0xA7;
        let (macros, note) = decode_macros(&data, 0, data.len());
        assert!(macros.is_empty(), "a block without the marker decodes to nothing");
        assert!(note.as_ref().is_some_and(|n| n.contains("not the marker")), "{note:?}");
        // A record, a zero gap, then bytes that are not a record: the record the block holds is kept
        // and the gap is named in the note (the gap is what a binding index can point across).
        let mut data = vec![0u8; MACRO_DATA_LEN];
        data[..20].copy_from_slice(&SYNTH_MACRO_BLOCK[..20]);
        data[40] = 0x01;
        data[41] = 0xA7;
        let (macros, note) = decode_macros(&data, 0, data.len());
        assert_eq!(macros.len(), 1, "the record before the gap is kept");
        assert!(
            note.as_ref()
                .is_some_and(|n| n.contains("not the marker 0x77") && n.contains("20 zero bytes skipped")),
            "{note:?}"
        );
        // A marker with nothing behind it is a header-only record: mode, id and interval read as zeros
        // and its action list ends at once. The scan walks on and the block reports no note.
        let mut data = vec![0u8; MACRO_DATA_LEN];
        data[..20].copy_from_slice(&SYNTH_MACRO_BLOCK[..20]);
        data[40] = MACRO_RECORD_MARKER;
        let (macros, note) = decode_macros(&data, 0, data.len());
        assert_eq!(
            (macros.len(), note),
            (2, None),
            "a lone marker across a gap is the second record, not a parse failure"
        );
        assert_eq!(
            (
                macros[1].index,
                macros[1].ordinal,
                macros[1].record_bytes,
                macros[1].action_count
            ),
            (MACRO_INDEX_STRIDE, 1, MACRO_HEADER_LEN, 0),
            "the header-only record is 8 bytes with no actions"
        );
        let (macros, note) = decode_macros(&[0u8; 64], 0, 64);
        assert!(macros.is_empty() && note.is_none(), "an empty block is the normal case");
        // A record that is cut by the block end is reported, not silently shortened.
        let truncated = &SYNTH_MACRO_BLOCK[..12];
        let (macros, note) = decode_macros(truncated, 0, truncated.len());
        assert_eq!(macros.len(), 1, "the header is already a record");
        assert!(
            note.as_ref().is_some_and(|n| n.contains("press action runs past")),
            "{note:?}"
        );
    }

    /// A profile file names no keyboard, a read names the one it ran on, and the encoder ignores the
    /// identity: the layout, the encodable fields and the feature set are per device, so the JSON has to
    /// carry the product ID, and it comes first - keyboard first, profile after - and moves the encoder
    /// not at all.
    #[test]
    fn a_stamped_device_is_metadata_the_encoder_ignores() {
        let data = synthetic_payload(SCHEMA_B);
        let profile = decode(&data).expect("the synthetic payload decodes");
        let plain = profile_json(&profile).expect("the decode report serializes");
        for key in ["product_id", "product", "firmware"] {
            assert!(
                !plain.contains(&format!("\"{key}\"")),
                "a decode of bytes writes no {key}: bytes name no keyboard"
            );
        }

        let mut stamped = profile.clone();
        stamped.product_id = Some(crate::devices::product_ids::APEX_7);
        stamped.product =
            Some(crate::devices::device_name_from_product_id(crate::devices::product_ids::APEX_7).to_string());
        stamped.firmware = Some("4.16.3".to_string());
        let text = profile_json(&stamped).expect("the stamped report serializes");
        let value: serde_json::Value = serde_json::from_str(&text).expect("the stamped report parses");
        assert_eq!(value["product_id"], serde_json::json!(0x1612));
        assert_eq!(value["product"], serde_json::json!("Apex 7"));
        assert_eq!(value["firmware"], serde_json::json!("4.16.3"));
        let at = |key: &str| text.find(&format!("\"{key}\"")).expect(key);
        assert!(
            at("product_id") < at("product"),
            "product ID before the model name it keys"
        );
        assert!(
            at("product") < at("firmware"),
            "model name before the firmware it belongs to"
        );
        assert!(
            at("firmware") < at("schema"),
            "the identity fields sit above the profile fields: which keyboard, then what it holds"
        );
        assert_eq!(
            encode_json(&text).expect("an identity block does not stop the encoder"),
            data,
            "the identity fields are metadata: the payload bytes are unchanged"
        );
    }

    /// The read's record carries the same identity, and an image assembled without a device says so.
    #[test]
    fn the_read_record_carries_the_device_the_image_names() {
        let image = ProfileImage::assemble(0, Vec::new()).expect("an empty assembly is valid");
        assert!(
            image.product_id.is_none() && image.product.is_none() && image.firmware.is_none(),
            "assembled without a device"
        );
        let record = to_json(&image).expect("the record serializes");
        for key in ["product_id", "product", "firmware"] {
            assert!(
                record.contains(&format!("\"{key}\": null")),
                "the record states openly that no {key} is recorded"
            );
        }
        let at = |key: &str| record.find(&format!("\"{key}\"")).expect(key);
        assert!(
            at("product_id") < at("profile_slot"),
            "the keyboard is named before the slot it was read from"
        );
    }

    /// The write session, checked with nothing outside the repository: every frame of GG's session
    /// matches the plan on index, report kind, wire length and header bytes, and the frames carrying no
    /// payload bytes - the `88 00` open, the 40 `05 00` pushes, the `eb` commit - are byte-identical to
    /// the capture. The 80 buffer writes carry payload bytes, so their content is checked by the hardware
    /// test below.
    ///
    /// The deploy frame's settle: the engine's own pause number; the frame list carries no settle (it is a
    /// wait, not a transfer), and the plan a `--dry-run` prints names it - a plan that prints 122 frames and
    /// sends a frame the keyboard needs a settle for must say so.
    ///
    /// Hardware-measured settle - `89` straight after `eb 01` can do nothing, and waiting a
    /// little longer before it is what lands the switch. The engine sleeps 250 ms behind the deploy frame and
    /// its commit path ends with an input read (`base_apex_legacy.lisp`).
    #[test]
    fn the_deploy_settle_is_the_engine_pause_number_and_the_plan_names_it() {
        assert_eq!(
            DEPLOY_SETTLE_MS, 250,
            "250 ms is load-profile-with-pause's own pause (base_apex_legacy.lisp:2039-2045)"
        );
        let payload = synthetic_payload(SCHEMA_B);
        let plan = write_plan(0, &payload).expect("a payload-length image plans a write");
        assert_eq!(
            plan.frames
                .iter()
                .filter(|frame| frame.line().contains("deploy"))
                .count(),
            0,
            "the settle is a wait in front of the deploy frame, never a frame of the plan"
        );
        let lines = write_plan_lines(&plan);
        let last = lines.last().expect("the plan text ends with the deploy line");
        assert!(
            last.contains(&format!("after a {DEPLOY_SETTLE_MS} ms settle")),
            "the dry-run plan names the settle the send path takes, got: {last}"
        );
    }
}
