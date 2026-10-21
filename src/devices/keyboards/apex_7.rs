//! The Apex 7 keyboard device: standard lighting control through [`GenericKeyboard`], plus the
//! on-board profile read, write and deploy of its legacy "macho" protocol.
//!
//! Frame construction, reply parsing, payload geometry and decoding live in
//! [`crate::devices::keyboards::apex_7_profile`]; every wire fact the transfers depend on is stated
//! there. This module is the transfer machinery: the firmware poll, the pull frames, the pacing read that
//! follows each pull, the payload reads, and the assembly of a
//! [`ProfileImage`](crate::devices::keyboards::apex_7_profile::ProfileImage).
//!
//! Transfers run on the handle this device already owns - the collection the device manager opened, held
//! open for the whole transfer. `usbhid` polls a collection's interrupt endpoint only while a descriptor on
//! that node is open, so the interrupt reports the transfers consume (the `0x90` answer, the packet after
//! each pull) only arrive on a handle that stays open.
//!
//! Read, write and deploy of a profile slot are confirmed on hardware on an Apex 7 (`0x1612`) under Linux
//! (2026-10-20). Decode and encode are verified against clean and modified profiles; the field meanings
//! inside the OLED and macro blocks stay reverse-engineered.

use std::ops::{Deref, DerefMut};
use std::time::{Duration, Instant};

use async_trait::async_trait;

use super::apex_7_profile::{
    Apex7Onboard, ChunkRecord, DEVICE_RESOURCE_BUFFER_LEN, GG_GAP_MS, GG_INPUT_READ_BYTES, MAX_READ_LEN,
    PAYLOAD_READ_LEN, PROFILE_FILE_BYTES, PULLS_PER_FILE, ProfileImage, READ_REPLY_FRAMING_LEN, READ_REPORT_LEN,
    READS_PER_PULL, ReadReply, SCHEMA_B, WriteStats, hex_of, parse_firmware_version, parse_read_reply_at,
    profile_file_id, reply_mismatch, schema_of, validate_profile,
};
use super::onboard::{OnboardProfileCodec, OnboardWriteFrame};
use super::{GenericKeyboard, Keyboard};
use crate::devices::hid_reports::{
    FirmwareSource, HidCommand, ONBOARD_FEATURE_REPORT_SIZE, ONBOARD_OUTPUT_REPORT_SIZE, OnboardBufferReadCommand,
    OnboardBufferWriteCommand, OnboardFileCommitCommand, OnboardFileOpenCommand, OnboardFirmwareVersionCommand,
    OnboardPayloadPullCommand, OnboardPayloadPushCommand, OnboardProfileDeployCommand,
};
use crate::devices::key_mapping::{KeyId, KeyMapping};
use crate::devices::product_ids;
use crate::devices::zone_mapping::{ZoneEffect, ZoneMapping};
use crate::devices::{Device, DeviceType, device_name_from_product_id};
use crate::error::{Error, Result};
use crate::rgb::{Color, PerKeyEffect};

/// Report number to ask for when reading a payload read back: **0** - `GET_REPORT wValue 0x0300 wIndex 1`
/// in the reference capture. The kernel writes the device's own report-ID byte into byte 0, so the reply's
/// leading `03` arrives there whatever we ask for. Confirmed on hardware on an Apex 7 (`0x1612`).
const READ_REPLY_ID: u8 = 0x00;
/// Buffer size to hand the transport when reading a payload read back: [`READ_REPORT_LEN`] + 1 (643). The
/// buffer carries the report-number byte (0) in front of the data, and with report number 0 the kernel drops
/// that byte from the request, so the wire `wLength` is [`READ_REPORT_LEN`] (642), the value the capture shows.
/// It is the `wLength` that must not be 643: a 643-byte `GET_REPORT` never completes.
const READ_REPLY_BUFFER_LEN: usize = READ_REPORT_LEN + 1;
/// Poll acknowledges and status reports the reply collector skips.
const ACK_REPORT_IDS: [u8; 2] = [0x0B, 0x12];
/// Bytes to read for the pacing packet that follows a pull. The pull has no answer of its own: the device
/// keeps streaming its ordinary input reports (report `0x01`, all-zero on an idle keyboard, 1.7-3.7 ms after
/// the pull in the capture) and GG consumes one packet per chunk as pacing, taking no bytes from it.
const PULL_ACK_LEN: usize = GG_INPUT_READ_BYTES;
/// Polls per reply collection round, in milliseconds.
const REPLY_POLL_MS: u64 = 50;
/// Reply budget for one payload read, in milliseconds: the feature-report poll loop runs until this
/// expires, then the input-endpoint fallback starts. A fixed budget, not a caller-set deadline.
const REPLY_BUDGET_MS: u64 = 2_000;
/// Input-report polls allowed per reply before giving up.
const REPLY_INPUT_ATTEMPTS: u32 = 20;
/// Reads allowed per pull before giving up (the device streams input reports, so one answer is expected).
const PULL_ACK_ATTEMPTS: u32 = 4;
/// Bytes to read for the firmware poll's answer: GG's input read size, [`GG_INPUT_READ_BYTES`] (a
/// report-ID slot plus 64 data bytes), which the answer fits (`0x6E` with the ASCII version).
const FIRMWARE_REPLY_LEN: usize = GG_INPUT_READ_BYTES;
/// Reads allowed for the firmware poll's answer. One: the version is informational, so the read must not
/// spend time waiting on a keyboard that answers nothing.
const FIRMWARE_REPLY_ATTEMPTS: u32 = 1;
/// Size of the schema probe pull, in bytes: the 2-byte probe on the profile file of slot 0 the reference
/// capture shows in front of the file read (`[85][00][0201][0][2]`, answered `03 0b 00`).
const PROBE_PULL_LEN: u16 = 2;
/// Largest input report a profile collection can return (Apex 7 profile collections report 1024).
const INPUT_REPORT_MAX_LEN: usize = 1024;
/// Input-report polls allowed while waiting for the file-open answer (0x88): the write capture
/// answers it 305 ms after the frame, and this endpoint carries protocol reports in between, which the wait
/// skips (see [`Apex7::read_protocol_report`]).
const WRITE_OPEN_ACK_ATTEMPTS: u32 = 40;
/// Input-report polls allowed while waiting for a push answer (0x05): 5 ms after the frame in the capture.
const WRITE_PUSH_ACK_ATTEMPTS: u32 = 20;
/// Status byte every write-side answer carries. Confirmed on hardware on an Apex 7 (`0x1612`): every
/// answer a write gets carries it. What it means beyond "accepted" is observed, not proven; a write stops
/// on any other value, because the frames after a refused one build on it.
const WRITE_ACK_STATUS_OK: u8 = 0x01;

/// A payload read reply arrives as `[report ID][641 bytes]`, and a header-echoing reply spends
/// [`READ_REPLY_FRAMING_LEN`] of those bytes on its header: that is why a read frame never asks for more
/// than [`MAX_READ_LEN`] (636) - a larger ask could not be carried by any reply.
const _: () = assert!(MAX_READ_LEN + READ_REPLY_FRAMING_LEN < READ_REPLY_BUFFER_LEN);

// Transfers use the packet I/O of the keyboard this device owns: `write_profile_packet` for output frames
// (not `send_report`, whose padded write drops a report byte-identical to one sent in the last 50 ms - see
// [`Apex7::write_output_command`]), `send_feature` for the feature-report read request, and
// `read_profile_feature_packet` / `read_profile_input_packet` for the two reply shapes.
//
// Confirmed on hardware on an Apex 7 (`0x1612`): the firmware poll, the pull, the pacing read, the payload
// read-back, the write-side frames and the deploy all complete. A profile slot is collected without GG's
// opening chain and closing sequence, so this crate sends neither.

/// Apex 7 keyboard device, with the on-board profile read on its own handle.
///
/// Discovery builds this class for product ID `0x1612` and for nothing else (`discovery::open_keyboard`),
/// so the on-board scope is the class itself: [`Keyboard::onboard_codec`] answers it, and
/// [`Keyboard::supports_onboard_profile`](crate::devices::keyboards::Keyboard::supports_onboard_profile)
/// reports that answer for the keyboard the manager opened. No product-ID table is consulted.
pub struct Apex7 {
    inner: GenericKeyboard,
}

impl Apex7 {
    /// Product ID of the Apex 7 the reference capture was taken on.
    pub const PRODUCT_ID: u16 = product_ids::APEX_7;

    /// Create from a generic keyboard.
    pub fn new(keyboard: GenericKeyboard) -> Self {
        Self { inner: keyboard }
    }

    /// Read one on-board profile slot (slots `0..=4`, `ssgg onboard read --profile N` passes `N - 1`).
    ///
    /// The read polls the device and sleeps for GG's inter-frame pacing, so it blocks the calling thread
    /// for about 2 s per slot. Nothing runs concurrently with it and nothing in it changes the keyboard's
    /// state: GG's opening chain and closing sequence are not sent.
    ///
    /// # Errors
    ///
    /// Returns an error when the slot is out of range, the collection rejects a frame, it answers a schema
    /// this read cannot decode, or it never answers a payload read.
    pub fn read_profile(&self, profile: u8) -> Result<ProfileImage> {
        validate_profile(profile)?;
        let fs_id = profile_file_id(profile);
        let read_len = PAYLOAD_READ_LEN as u16;

        // The geometry this platform's read always has; `ssgg --debug onboard read ...` shows it before
        // the first byte moves.
        tracing::info!(
            "onboard read geometry: profile slot {profile} = GG config {} (file {fs_id:04X}) is {PULLS_PER_FILE} pulls of \
             {DEVICE_RESOURCE_BUFFER_LEN} bytes, {} payload reads of {PAYLOAD_READ_LEN} bytes, {} of the {} \
             payload bytes covered",
            profile + 1,
            PULLS_PER_FILE * READS_PER_PULL,
            PULLS_PER_FILE * READS_PER_PULL * PAYLOAD_READ_LEN,
            PROFILE_FILE_BYTES,
        );

        // GG polls the firmware version in front of a profile session. The version is informational, so a
        // refused or unanswered poll is recorded and the read continues (see [`Self::firmware_version`]).
        let firmware = match self.firmware_version(FirmwareSource::Keyboard) {
            Ok(version) => version,
            Err(e) => {
                tracing::debug!("onboard read: firmware poll failed: {e}");
                None
            }
        };

        // GG's 2-byte probe follows, and the schema word it answers decides whether this read can decode
        // what it is about to pull (see [`Self::probe_schema`]).
        self.probe_schema(fs_id)?;

        let mut chunks: Vec<ChunkRecord> = Vec::with_capacity(PULLS_PER_FILE);
        for chunk in 0..PULLS_PER_FILE {
            let file_offset = (chunk * DEVICE_RESOURCE_BUFFER_LEN) as u32;
            let (ack_report_id, ack_hex) = self.pull_window(fs_id, file_offset, DEVICE_RESOURCE_BUFFER_LEN as u16)?;
            let mut reads: Vec<ReadReply> = Vec::with_capacity(READS_PER_PULL);
            for read in 0..READS_PER_PULL {
                let buffer_offset = (read * PAYLOAD_READ_LEN) as u16;
                reads.push(self.read_window(&format!("chunk {chunk}"), buffer_offset, read_len)?);
            }
            chunks.push(ChunkRecord {
                index: chunk,
                fs_id,
                file_offset,
                ack_report_id,
                ack_hex,
                reads,
            });
        }

        let mut image = ProfileImage::assemble(profile, chunks)?;
        image.firmware = firmware;
        // The bytes are stamped with the keyboard they came off: the layout, the encodable fields and
        // the feature set are all per device, so a profile file has to name its model.
        let info = self.info();
        image.product_id = Some(info.product_id);
        image.product = Some(device_name_from_product_id(info.product_id).to_string());
        Ok(image)
    }

    /// Writes one on-board profile slot with `payload` bytes, then deploys the slot. The write blocks on
    /// the keyboard's answers and on GG's pacing, so it blocks the calling thread for its whole duration;
    /// nothing runs concurrently with it.
    ///
    /// **This changes the keyboard**: it rewrites the profile file of the slot and then makes the keyboard
    /// run that slot. Lighting, audio and the other slots are untouched, and the write sends nothing else.
    ///
    /// Confirmed on hardware on an Apex 7 (`0x1612`): the frames are the write capture's sequence, byte for
    /// byte ([`write_plan`](crate::devices::keyboards::apex_7_profile::write_plan)).
    ///
    /// # Errors
    ///
    /// Returns an error when the payload cannot be framed for the slot, a frame is refused, or a frame the
    /// capture shows answered is not answered here. The write stops at the first such frame, leaving the
    /// slot as the frames already sent left it.
    pub fn write_profile(&self, profile: u8, payload: &[u8]) -> Result<WriteStats> {
        let codec = &Apex7Onboard;
        let plan = codec.write_plan(profile, payload)?;
        for line in codec.write_banner_lines() {
            tracing::info!("onboard write geometry: {line}");
        }
        tracing::info!(
            "onboard write: slot {profile} (file {:04X}) of {} payload bytes padded to {}, {} frames, {} ms \
             between them, on {}",
            plan.fs_id,
            plan.payload_len,
            plan.region_bytes,
            plan.frames.len(),
            GG_GAP_MS,
            self.collection_label()
        );

        let mut stats = WriteStats::default();
        for frame in &plan.frames {
            match frame {
                OnboardWriteFrame::FileOpen { fs_id } => {
                    self.write_output_command(OnboardFileOpenCommand { fs_id: *fs_id })
                        .map_err(|e| self.frame_error("file open", &format!("file {fs_id:04X}"), e))?;
                    stats.frames_sent += 1;
                    stats.acks +=
                        self.await_write_ack(codec.write_ack_report_ids()[0], WRITE_OPEN_ACK_ATTEMPTS, "file open")?;
                }
                OnboardWriteFrame::BufferWrite { buffer_offset, data } => {
                    self.write_feature_command(OnboardBufferWriteCommand {
                        buffer_offset: *buffer_offset,
                        data,
                    })
                    .map_err(|e| {
                        self.frame_error(
                            "buffer write",
                            &format!("buffer offset {buffer_offset} size {}", data.len()),
                            e,
                        )
                    })?;
                    stats.frames_sent += 1;
                    stats.bytes_written += data.len();
                }
                OnboardWriteFrame::Push {
                    fs_id,
                    file_offset,
                    size,
                } => {
                    self.write_output_command(OnboardPayloadPushCommand {
                        fs_id: *fs_id,
                        file_offset: *file_offset,
                        size: *size,
                    })
                    .map_err(|e| {
                        self.frame_error("push", &format!("file {fs_id:04X} offset {file_offset} size {size}"), e)
                    })?;
                    stats.frames_sent += 1;
                    stats.pushes += 1;
                    stats.acks +=
                        self.await_write_ack(codec.write_ack_report_ids()[1], WRITE_PUSH_ACK_ATTEMPTS, "push")?;
                }
                OnboardWriteFrame::Commit { profile } => {
                    // The capture answers the commit with nothing of its own, so the write does not wait.
                    self.write_output_command(OnboardFileCommitCommand { profile: *profile })
                        .map_err(|e| self.frame_error("commit", &format!("slot {profile}"), e))?;
                    stats.frames_sent += 1;
                }
            }
            if GG_GAP_MS > 0 {
                std::thread::sleep(Duration::from_millis(GG_GAP_MS));
            }
        }

        // Give time for the profile to settle before switching, this is the same as GG.
        std::thread::sleep(Duration::from_millis(
            crate::devices::keyboards::apex_7_profile::DEPLOY_SETTLE_MS,
        ));
        self.deploy_frame(profile)?;
        Ok(stats)
    }

    /// Makes the keyboard run one on-board profile slot: GG's profile deploy frame (0x89).
    /// **This changes what the keyboard is running right now.** The capture shows GG sending the frame with
    /// no answer of its own, so this sends the frame and reads nothing back; the frame is confirmed on
    /// hardware - a deployed slot is the slot the keyboard runs. It runs as part of [`Self::write_profile`]
    /// and as `ssgg onboard load`.
    ///
    /// # Errors
    ///
    /// Returns an error when the slot is out of range or the device refuses the frame.
    pub fn deploy_profile(&self, profile: u8) -> Result<()> {
        validate_profile(profile)?;
        self.deploy_frame(profile)
    }
    /// Sends the profile deploy frame (0x89) and reads nothing back
    fn deploy_frame(&self, profile: u8) -> Result<()> {
        self.write_output_command(OnboardProfileDeployCommand { profile })
            .map_err(|e| self.frame_error("deploy", &format!("slot {profile}"), e))
    }

    /// A frame refusal, named with the frame and the collection it went out on.
    fn frame_error(&self, kind: &str, what: &str, e: Error) -> Error {
        Error::DeviceCommunication(format!(
            "onboard write frame {kind} ({what}) on {} rejected: {e}",
            self.collection_label()
        ))
    }

    /// Waits the answer a write-side frame gets: the capture answers the file open with `88 01 …` (305 ms
    /// after the frame) and every push with `05 01 …` (5 ms after it), so this is
    /// [`Apex7::read_protocol_report`] filtered by report number.
    /// Confirmed on hardware on an Apex 7 (`0x1612`): the status byte is `01` in every answer a write gets,
    /// and any other value, and any frame left unanswered, stops the write - the frames after it build on
    /// the one that was refused.
    fn await_write_ack(&self, expected_id: u8, attempts: u32, label: &str) -> Result<usize> {
        let path = self.collection_label();
        let answer = self
            .read_protocol_report(PULL_ACK_LEN, attempts, |report| report.first() == Some(&expected_id))
            .ok_or_else(|| {
                Error::DeviceCommunication(format!(
                    "onboard write on {path}: {label} answered nothing in {} ms; the capture answers every \
                     frame of this kind, so the write stops here",
                    u64::from(attempts) * REPLY_POLL_MS
                ))
            })?;
        let status = answer.get(1).copied().unwrap_or(0);
        if status != WRITE_ACK_STATUS_OK {
            return Err(Error::DeviceCommunication(format!(
                "onboard write on {path}: {label} answered status {status:#04x}; the capture only \
                 ever shows {WRITE_ACK_STATUS_OK:#04x}, so the write stops here"
            )));
        }
        Ok(1)
    }

    /// Runs GG's 2-byte probe in front of the file read and checks the schema word it answers: a pull of
    /// [`PROBE_PULL_LEN`] bytes at file offset 0 - the shape the capture shows for the profile file of slot
    /// 0 - followed by one payload read at buffer offset 0, whose answer starts with the schema word
    /// (`03 0b 00 …`, i.e. [`SCHEMA_B`]).
    ///
    /// The read decodes [`SCHEMA_B`] only, so any other answer stops the read here: nothing has been
    /// written yet, and a schema this decoder cannot lay out would surface later as a profile whose
    /// checksum never matches. Confirmed on hardware on an Apex 7 (`0x1612`): the schema word answers
    /// [`SCHEMA_B`].
    fn probe_schema(&self, fs_id: u16) -> Result<u16> {
        self.pull_window(fs_id, 0, PROBE_PULL_LEN)?;
        let reply = self.read_window("schema probe", 0, PAYLOAD_READ_LEN as u16)?;
        let schema = schema_of(&reply.data);
        if schema != SCHEMA_B {
            return Err(Error::Profile(format!(
                "device answers schema {schema:#06x} at payload offset 0; this read is implemented for \
                 {SCHEMA_B:#06x} (schema B) and measured on hardware with that schema only"
            )));
        }
        Ok(schema)
    }

    /// Fills the device's resource buffer with one pull frame and drains the pacing packet that follows it.
    ///
    /// Returns the pacing packet's report ID and hex text for the chunk record (`(0, "")` when nothing
    /// arrived, which is logged, not an error - see [`Self::read_pull_ack`]).
    fn pull_window(&self, fs_id: u16, file_offset: u32, size: u16) -> Result<(u8, String)> {
        let path = self.collection_label();
        self.write_output_command(OnboardPayloadPullCommand {
            fs_id,
            file_offset,
            size,
        })
        .map_err(|e| {
            Error::DeviceCommunication(format!(
                "onboard pull frame (file {fs_id:04X} offset {file_offset} size {size}) on {path} rejected: {e}"
            ))
        })?;
        match self.read_pull_ack() {
            Some(report) if !report.is_empty() => Ok((report[0], hex_of(&report))),
            _ => {
                tracing::debug!(
                    "no input report arrived after the pull for file {fs_id:04X} offset {file_offset} on {path}"
                );
                Ok((0, String::new()))
            }
        }
    }

    /// Requests one payload window and collects its reply, then paces the way GG's read spec paces
    /// ([`GG_GAP_MS`] between transfers).
    fn read_window(&self, label: &str, buffer_offset: u16, read_len: u16) -> Result<ReadReply> {
        let path = self.collection_label();
        self.write_feature_command(OnboardBufferReadCommand {
            buffer_offset,
            size: read_len,
        })
        .map_err(|e| {
            Error::DeviceCommunication(format!(
                "onboard read request for {label} buffer offset {buffer_offset} on {path} rejected: {e}"
            ))
        })?;
        // If there's no wait there's a race of whether the device prepares the buffer in time, resulting
        // in garbage reads
        std::thread::sleep(Duration::from_millis(1));
        let reply = self.read_reply_report(PAYLOAD_READ_LEN)?;
        let parsed = parse_read_reply_at(&reply, read_len, READ_REPLY_FRAMING_LEN)?;
        if let Some(msg) = reply_mismatch(&parsed, buffer_offset, read_len) {
            tracing::debug!("onboard read: {msg}");
        }
        if GG_GAP_MS > 0 {
            std::thread::sleep(Duration::from_millis(GG_GAP_MS));
        }
        Ok(parsed)
    }

    /// Polls GG's firmware version ([`OnboardFirmwareVersionCommand`]) and decodes its answer. The source
    /// byte selects it: [`FirmwareSource::Keyboard`] (`90 00`) is the firmware running the keyboard,
    /// [`FirmwareSource::Led`] (`90 01`) the LED controller's. Both answer on hardware (Apex 7 `0x1612`).
    ///
    /// The read calls this with [`FirmwareSource::Keyboard`] for the version line of a
    /// [`ProfileImage`](crate::devices::keyboards::apex_7_profile::ProfileImage); a keyboard that accepts
    /// the frame and answers nothing is not an error, `None` is returned.
    ///
    /// # Errors
    ///
    /// Returns an error when the device refuses the frame.
    pub fn firmware_version(&self, source: FirmwareSource) -> Result<Option<String>> {
        let path = self.collection_label();
        let frame = OnboardFirmwareVersionCommand { source };
        self.write_output_command(frame).map_err(|e| {
            Error::DeviceCommunication(format!(
                "onboard firmware poll {} on {path} rejected: {e}",
                frame.description()
            ))
        })?;
        // The answer lands 1-3 ms after the frame; GG's own cadence is one frame per GG_GAP_MS.
        if GG_GAP_MS > 0 {
            std::thread::sleep(Duration::from_millis(GG_GAP_MS));
        }
        let answer =
            self.read_protocol_report(FIRMWARE_REPLY_LEN, FIRMWARE_REPLY_ATTEMPTS, |report| !report.is_empty());
        if answer.is_none() {
            tracing::debug!("onboard firmware poll on {path} answered nothing");
        }
        Ok(answer.as_deref().and_then(parse_firmware_version))
    }

    /// Collects one payload reply: the feature-report reply the capture shows, then the interrupt
    /// endpoint. Returns `[report ID][payload…]`, the shape `parse_read_reply_at` consumes.
    fn read_reply_report(&self, payload_len: usize) -> Result<Vec<u8>> {
        let path = self.collection_label();
        let stop = Instant::now() + Duration::from_millis(REPLY_BUDGET_MS);
        let mut empty_answers = 0u32;
        let mut last_error: Option<Error> = None;

        while Instant::now() < stop {
            match self.read_profile_feature_packet(READ_REPLY_ID, READ_REPLY_BUFFER_LEN) {
                Ok(report) if reply_carries_payload(&report, payload_len) => return Ok(report),
                Ok(report) => {
                    empty_answers = empty_answers.saturating_add(1);
                    tracing::debug!(
                        "onboard read on {path}: feature reply of {} bytes (report {:#04X}) carries no \
                         payload of {payload_len} bytes",
                        report.len(),
                        report.first().copied().unwrap_or(0),
                    );
                }
                Err(e) => {
                    last_error = Some(Error::DeviceCommunication(format!("payload read on {path}: {e}")));
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(REPLY_POLL_MS));
        }

        let mut input_attempts = 0u32;
        while input_attempts < REPLY_INPUT_ATTEMPTS {
            input_attempts = input_attempts.saturating_add(1);
            match poll_outcome(
                self.read_profile_input_packet(INPUT_REPORT_MAX_LEN, REPLY_POLL_MS),
                &|report: &[u8]| reply_carries_payload(report, payload_len),
            ) {
                PollOutcome::Match(report) => {
                    tracing::debug!(
                        "onboard read: reply arrived on {path} as a {}-byte input report",
                        report.len()
                    );
                    return Ok(report);
                }
                PollOutcome::Skip(report) if !is_ack(&report) => {
                    tracing::debug!(
                        "onboard read on {path}: ignored a {}-byte input report (report {:#04X})",
                        report.len(),
                        report.first().copied().unwrap_or(0),
                    );
                }
                // A window that carried nothing is one of the attempts, not a failure of them.
                PollOutcome::Skip(_) | PollOutcome::Silence => {}
                PollOutcome::Failure(e) => {
                    tracing::debug!("onboard read on {path}: input report read failed: {e}");
                    break;
                }
            }
        }

        let reason = match last_error {
            Some(e) => e.to_string(),
            None => format!("{empty_answers} feature reply answer(s) carried no payload of {payload_len} bytes"),
        };
        Err(Error::DeviceCommunication(format!(
            "no profile payload reply from {path}: {reason}"
        )))
    }

    /// Reads the pacing packet that follows a pull. Its content is not looked at - GG's read specifiers keep
    /// zero bytes of it - and the pull has no answer of its own: the device keeps streaming reports on this
    /// endpoint (report `0x01`, all-zero on an idle keyboard, 1.7-3.7 ms after the pull in the capture). The
    /// drain is load-bearing: skip it and every payload read-back fails.
    fn read_pull_ack(&self) -> Option<Vec<u8>> {
        self.read_protocol_report(PULL_ACK_LEN, PULL_ACK_ATTEMPTS, |report| !report.is_empty())
    }

    /// Identity of the collection the read runs on, for logs and read errors.
    fn collection_label(&self) -> String {
        let info = self.inner.info();
        format!(
            "vid {:04x}:pid {:04x} interface {} ({})",
            info.vendor_id, info.product_id, info.interface_number, info.path
        )
    }

    /// Polls the collection's interrupt endpoint for the next report this transfer is looking for.
    /// `want` names the report to stop on; a report that arrives and does not match is logged and skipped,
    /// which is what the read side needs (the device streams all-zero reports, report `0x01`, and GG's
    /// transfers consume them as pacing) and what lets the write side wait for a specific frame's answer.
    /// `None` when nothing matched within `attempts` polls.
    ///
    /// Keystrokes are not on this endpoint: the profile collection runs at `2.2` (interrupt endpoint `0x82`),
    /// the key reports come from the keyboard collection - so the traffic this sees is protocol traffic.
    fn read_protocol_report(&self, len: usize, attempts: u32, want: impl Fn(&[u8]) -> bool) -> Option<Vec<u8>> {
        let path = self.collection_label();
        for _ in 0..attempts {
            match poll_outcome(self.read_profile_input_packet(len, REPLY_POLL_MS), &want) {
                PollOutcome::Match(report) => return Some(report),
                PollOutcome::Skip(report) if !report.is_empty() => {
                    tracing::debug!(
                        "onboard poll on {path}: skipped report {:#04x} ({} bytes)",
                        report[0],
                        report.len()
                    );
                }
                // Every window is one of the attempts: a window that carried nothing is polled again.
                PollOutcome::Skip(_) | PollOutcome::Silence => {}
                PollOutcome::Failure(e) => {
                    tracing::debug!("onboard input-report poll on {path}: {e}");
                    break;
                }
            }
        }
        None
    }

    /// Serializes a command through the keyboard's [`HidReportBuilder`] and sends it as an output report:
    /// report number 0 plus the 64-byte data stage, i.e. `SET_REPORT wValue 0x0200 wLength 64` as the
    /// reference capture shows (the kernel takes the report number out of the data stage, so the buffer is
    /// one byte longer than the wire).
    ///
    /// This is [`GenericKeyboard::write_profile_packet`], deliberately not [`GenericKeyboard::send_report`]:
    /// `send_report` writes through [`crate::devices::write_padded_report`], whose global
    /// [`crate::devices::HidOptimizer`] skips any report byte-identical to one sent in the last 50 ms, and
    /// profile-protocol transfers repeat identical frames (GG polls `90 00` and `90 01` 5 ms apart).
    fn write_output_command<C: HidCommand>(&self, command: C) -> Result<()> {
        let mut buffer = [0u8; ONBOARD_OUTPUT_REPORT_SIZE];
        let len = self.inner.report_builder.build_report(command, &mut buffer)?;
        if len != buffer.len() {
            return Err(Error::DeviceCommunication(format!(
                "onboard output report built {len} of {} bytes",
                buffer.len()
            )));
        }
        self.inner.write_profile_packet(&buffer)
    }

    /// Serializes a command through the keyboard's [`HidReportBuilder`] and sends it as a feature report on
    /// report number 0 (`SET_REPORT wValue 0x0300 wLength 642` in the capture).
    fn write_feature_command<C: HidCommand>(&self, command: C) -> Result<()> {
        let mut buffer = [0u8; ONBOARD_FEATURE_REPORT_SIZE];
        let len = self.inner.report_builder.build_report(command, &mut buffer)?;
        if len != buffer.len() {
            return Err(Error::DeviceCommunication(format!(
                "onboard feature report built {len} of {} bytes",
                buffer.len()
            )));
        }
        self.inner.send_feature(&buffer, ONBOARD_FEATURE_REPORT_SIZE)
    }
}

// Delegate Device trait.
impl Deref for Apex7 {
    type Target = GenericKeyboard;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Apex7 {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

// Delegate Device trait.
impl Device for Apex7 {
    fn info(&self) -> &crate::devices::DeviceInfo {
        self.inner.info()
    }

    fn device_type(&self) -> DeviceType {
        self.inner.device_type()
    }

    fn initialize(&mut self) -> Result<()> {
        self.inner.initialize()
    }

    fn close(&mut self) -> Result<()> {
        self.inner.close()
    }

    fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    fn send_raw(&mut self, data: &[u8]) -> Result<()> {
        self.inner.send_raw(data)
    }

    fn receive_raw(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.inner.receive_raw(buf)
    }
}

// Delegate Keyboard trait; the on-board profile methods are this device's own.
crate::impl_keyboard_with_delegation!(Apex7, {
    /// This class answers the Apex 7 codec, and discovery builds it only for the Apex 7 collection
    /// (`usage_page 0xFFC0, usage 0x9F`, product ID `0x1612`). That is the whole on-board scope: the
    /// `0x1614` / `0x1616` / `0x1618` boards GG lists on this platform are built as their own classes and
    /// answer no codec, so a file's recorded product ID is checked by the class that reads it, not by a
    /// product-ID table the codec keeps.
    fn onboard_codec(&self) -> Option<&'static dyn OnboardProfileCodec> {
        Some(&Apex7Onboard)
    }

    async fn read_onboard_profile(&self, profile: u8) -> Result<ProfileImage> {
        self.read_profile(profile)
    }

    async fn write_onboard_profile(&self, profile: u8, payload: &[u8]) -> Result<WriteStats> {
        self.write_profile(profile, payload)
    }

    async fn load_onboard_profile(&self, profile: u8) -> Result<()> {
        self.deploy_profile(profile)
    }

    // Lighting is the generic keyboard's. The macro delegates only the query methods, so without these
    // the trait defaults answer: `set_color` errors and the rest silently do nothing.
    async fn apply(&mut self) -> Result<()> {
        self.inner.apply().await
    }

    async fn set_color(&mut self, color: Color) -> Result<()> {
        self.inner.set_color(color).await
    }

    async fn set_zone_colors(&mut self, colors: &[Color]) -> Result<()> {
        self.inner.set_zone_colors(colors).await
    }

    async fn set_brightness(&mut self, brightness: u8) -> Result<()> {
        self.inner.set_brightness(brightness).await
    }

    async fn set_key_color(&mut self, key_id: KeyId, color: Color) -> Result<()> {
        self.inner.set_key_color(key_id, color).await
    }

    async fn set_key_colors(&mut self, key_colors: &[(KeyId, Color)]) -> Result<()> {
        self.inner.set_key_colors(key_colors).await
    }

    async fn set_key_color_direct(&mut self, address: crate::devices::KeyAddress, color: Color) -> Result<()> {
        self.inner.set_key_color_direct(address, color).await
    }

    async fn set_key_colors_direct(&mut self, key_colors: &[(crate::devices::KeyAddress, Color)]) -> Result<()> {
        self.inner.set_key_colors_direct(key_colors).await
    }

    async fn set_key_region(&mut self, start_hid: u8, count: u8, color: Color) -> Result<()> {
        self.inner.set_key_region(start_hid, count, color).await
    }

    fn read_actuation_point(&mut self) -> Result<u8> {
        self.inner.read_actuation_point()
    }

    fn set_actuation_point(&mut self, value: u8) -> Result<()> {
        self.inner.set_actuation_point(value)
    }

    fn set_actuation_point_mm(&mut self, mm: f32) -> Result<()> {
        self.inner.set_actuation_point_mm(mm)
    }
});

/// One poll of the interrupt endpoint, in the terms a poll loop acts on.
///
/// The distinction the loops live by is `Silence` from `Failure`: a window that carried nothing is the normal
/// state of a transfer waiting on this endpoint, so the loop spends another attempt on it; a read failure is
/// not a state to wait out and ends the transfer. Treating a timeout as a failure would burn the whole budget
/// on the first window.
enum PollOutcome {
    /// The report this transfer was looking for.
    Match(Vec<u8>),
    /// The window carried nothing: poll again.
    Silence,
    /// A report arrived and is not the one wanted: skip it and poll again.
    Skip(Vec<u8>),
    /// The read itself failed - the device is not answering on a later attempt either.
    Failure(String),
}

fn poll_outcome(read: Result<Option<Vec<u8>>>, want: &dyn Fn(&[u8]) -> bool) -> PollOutcome {
    match read {
        Ok(Some(report)) if want(&report) => PollOutcome::Match(report),
        Ok(Some(report)) => PollOutcome::Skip(report),
        Ok(None) => PollOutcome::Silence,
        Err(e) => PollOutcome::Failure(e.to_string()),
    }
}

/// A reply carries a payload when it is long enough to hold `payload_len` bytes behind its report-ID byte
/// and its report ID is not a poll acknowledge.
fn reply_carries_payload(report: &[u8], payload_len: usize) -> bool {
    if report.is_empty() {
        return false;
    }
    !ACK_REPORT_IDS.contains(&report[0]) && report.len() > payload_len
}

fn is_ack(report: &[u8]) -> bool {
    report.first().is_some_and(|id| ACK_REPORT_IDS.contains(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::keyboards::apex_7_profile::{
        MAX_PROFILE_INDEX, PROFILE_FS_BASE, profile_file_id, validate_profile,
    };

    #[test]
    fn reply_carries_payload_needs_room_behind_the_report_id() {
        assert!(reply_carries_payload(&[0x03, 0x01, 0x02], 2));
        assert!(!reply_carries_payload(&[0x03, 0x01], 2));
        assert!(!reply_carries_payload(&[0x0B, 0x0a, 0x00, 0x00], 2));
        assert!(!reply_carries_payload(&[0x12, 0x00], 2));
        assert!(!reply_carries_payload(&[], 2));
    }

    #[test]
    fn the_read_geometry_is_the_captured_one() {
        assert_eq!(profile_file_id(0), PROFILE_FS_BASE);
        assert_eq!(
            PULLS_PER_FILE * DEVICE_RESOURCE_BUFFER_LEN,
            12_288,
            "the fixed read covers the payload of the reference capture"
        );
        assert_eq!(
            READS_PER_PULL,
            DEVICE_RESOURCE_BUFFER_LEN / PAYLOAD_READ_LEN,
            "the fixed read drains the buffer in 512-byte windows"
        );
        assert!(validate_profile(MAX_PROFILE_INDEX).is_ok());
        assert!(validate_profile(MAX_PROFILE_INDEX + 1).is_err());
    }

    mod onboard {
        use super::super::*;
        use crate::devices::{DeviceInfo, DeviceType};

        fn device_without_handle(product_id: u16) -> Apex7 {
            let info = DeviceInfo {
                name: std::borrow::Cow::Borrowed("Test Apex 7"),
                device_type: DeviceType::Keyboard,
                vendor_id: crate::STEELSERIES_VENDOR_ID,
                product_id,
                interface_number: 1,
                usage_page: 0xFFC0,
                usage: 0x01,
                serial_number: None,
                manufacturer: Some("SteelSeries".to_string()),
                path: "/test/device/path".to_string(),
            };
            Apex7::new(GenericKeyboard::new_without_device(info))
        }

        #[test]
        fn only_the_apex_7_gets_the_read() {
            let apex_7 = device_without_handle(product_ids::APEX_7);
            assert!(Apex7::supports_onboard_profile(&apex_7));
            // The Apex 7 class answers the codec; every other keyboard class answers none, which is the
            // scope statement in code: discovery builds this class for the captured board's collection alone.
            assert!(Apex7::supports_onboard_profile(&apex_7));
            assert!(matches!(
                apex_7.onboard_codec().map(|codec| codec.family_name()),
                Some(name) if name == "Apex 7"
            ));
        }

        #[test]
        fn a_device_without_a_handle_says_so() {
            let keyboard = device_without_handle(product_ids::APEX_7);
            // report number 0 and a 642-byte ask: the payload read-back shape of the reference capture,
            // which the read side sends to the device verbatim as wLength 642
            match keyboard
                .inner
                .read_profile_feature_packet(0x00, crate::devices::keyboards::apex_7_profile::READ_REPORT_LEN)
            {
                Ok(_) => panic!("a device without a handle must not answer a profile packet"),
                Err(error) => assert!(
                    error.to_string().contains("Device not connected"),
                    "the error is the shared handle-less message: {error}"
                ),
            }
        }

        #[test]
        fn a_read_asks_for_the_slot_it_names() {
            // The device carries no slot state: the slot is an argument, and slot 0 is the file the
            // capture read.
            assert_eq!(
                crate::devices::keyboards::apex_7_profile::profile_file_id(0),
                crate::devices::keyboards::apex_7_profile::PROFILE_FS_BASE
            );
            assert_eq!(crate::devices::keyboards::apex_7_profile::profile_file_id(4), 0x0205);
        }
    }

    /// The distinction every poll of the interrupt endpoint is sorted into: a window that carried nothing
    /// spends one of the loop's attempts, a read failure ends the transfer.
    #[test]
    fn a_poll_window_that_carried_nothing_spends_an_attempt_and_a_read_failure_ends_it() {
        let want = |report: &[u8]| report.first() == Some(&0x05);
        assert!(
            matches!(poll_outcome(Ok(None), &want), PollOutcome::Silence),
            "a timeout is the normal state of a poll, not a failure: the loop polls the endpoint again"
        );
        assert!(
            matches!(poll_outcome(Ok(Some(vec![0x05, 0x01])), &want), PollOutcome::Match(_)),
            "the report the transfer is looking for ends the poll with its answer"
        );
        assert!(
            matches!(poll_outcome(Ok(Some(vec![0x01])), &want), PollOutcome::Skip(_)),
            "a report that is not the wanted one is skipped and the poll goes on"
        );
        let failure = poll_outcome(
            Err(crate::error::Error::DeviceCommunication(
                "input report read: Broken pipe".into(),
            )),
            &want,
        );
        assert!(
            matches!(failure, PollOutcome::Failure(_)),
            "a read failure is not a state to wait out: the transfer stops on it"
        );
    }
}
