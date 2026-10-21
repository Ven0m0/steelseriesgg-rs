//! [EXPERIMENTAL] On-board profile blob of the wired Apex Pro TKL 2023 (PID 0x1628).
//!
//! Layout and checksum come from GG USBPcap captures (2026-10-14, firmware 1.19.7); see
//! docs/development/apex-pro-tkl-2023-gg-capture.md. Nothing here has been written to a keyboard.
//! There is no known read-back command, so a caller cannot obtain the current blob yet; sending a
//! blob built from anything else would overwrite every other on-board setting.

use super::apex_pro_tkl_2023::{
    PROFILE_ACTUATION_OFFSET, PROFILE_RESET_OFFSET, WIRED_2023_PROFILE_SLOTS, profile_slot,
};
use crate::devices::hid_reports::ProfileChunkCommand;
use crate::{Error, Result};

/// Size of the reassembled profile blob.
pub const PROFILE_LEN: usize = 12_500;
/// Offset of the little-endian CRC over `blob[..PROFILE_CHECKSUM_OFFSET]`.
pub const PROFILE_CHECKSUM_OFFSET: usize = 12_320;

/// Observed `(tenths of a mm, actuation raw, reset raw)` triples. GG wrote both bytes together and the
/// scale is non-linear, so values in between are unknown and must not be interpolated.
///
/// The 1.0mm point is a live read (owner's board, 2026-10-21, GG's UI showed 1.0mm; see
/// `docs/development/apex-pro-tkl-2023-gg-capture.md`), overriding the earlier capture's `(0x15, 0x12)`
/// for that point, which was never confirmed against a labelled UI value the way this one was.
const ACTUATION_POINTS: [(u8, u8, u8); 4] = [(1, 0x05, 0x05), (10, 0x2F, 0x2A), (20, 0x39, 0x34), (30, 0x86, 0x7B)];

/// Profile checksum: the STM32 hardware CRC-32 (poly 0x04C11DB7, init 0xFFFFFFFF, no reflection,
/// no final XOR) over the data read as little-endian 32-bit words, MSB first.
///
/// Matched all 26 captured blobs (8 distinct). `data.len()` must be a multiple of 4.
pub fn profile_checksum(data: &[u8]) -> u32 {
    debug_assert!(data.len().is_multiple_of(4), "CRC input must be whole 32-bit words");
    data.chunks(4).fold(0xFFFF_FFFF, |crc, word| {
        word.iter().rev().fold(crc, |mut crc, &byte| {
            crc ^= u32::from(byte) << 24;
            for _ in 0..8 {
                crc = if crc & 0x8000_0000 != 0 {
                    (crc << 1) ^ 0x04C1_1DB7
                } else {
                    crc << 1
                };
            }
            crc
        })
    })
}

/// Recompute and store the checksum.
pub fn seal_profile(blob: &mut [u8; PROFILE_LEN]) {
    let crc = profile_checksum(&blob[..PROFILE_CHECKSUM_OFFSET]);
    blob[PROFILE_CHECKSUM_OFFSET..PROFILE_CHECKSUM_OFFSET + 4].copy_from_slice(&crc.to_le_bytes());
}

/// Set the actuation and reset point of one analog key (by HID usage), or of every analog key when
/// `usage` is `None`, then reseal the blob. Only the four captured distances are accepted.
pub fn set_actuation(blob: &mut [u8; PROFILE_LEN], usage: Option<u8>, tenths_mm: u8) -> Result<()> {
    let &(_, actuation, reset) = ACTUATION_POINTS
        .iter()
        .find(|(t, _, _)| *t == tenths_mm)
        .ok_or_else(|| {
            Error::Profile(format!(
                "Actuation {}.{} mm has no known raw value; captured points are 0.1, 1.0, 2.0 and 3.0 mm",
                tenths_mm / 10,
                tenths_mm % 10
            ))
        })?;

    let slots: Vec<usize> = match usage {
        Some(usage) => vec![
            profile_slot(usage)
                .ok_or_else(|| Error::Profile(format!("HID usage {usage:#04x} has no analog switch")))?,
        ],
        None => (0..WIRED_2023_PROFILE_SLOTS.len()).collect(),
    };
    for slot in slots {
        blob[PROFILE_ACTUATION_OFFSET + slot] = actuation;
        blob[PROFILE_RESET_OFFSET + slot] = reset;
    }
    seal_profile(blob);
    Ok(())
}

/// Split a blob into the 25 chunk commands GG sends (500 bytes each); serialize them with `HidReportBuilder`.
pub fn profile_chunks(blob: &[u8; PROFILE_LEN]) -> impl Iterator<Item = ProfileChunkCommand<'_>> {
    blob.chunks(ProfileChunkCommand::MAX_DATA_LEN)
        .enumerate()
        .map(|(i, data)| ProfileChunkCommand {
            offset: (i * ProfileChunkCommand::MAX_DATA_LEN) as u16,
            data,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_matches_the_stm32_crc_unit() {
        // ST's reference value for the CRC peripheral fed the single word 0x12345678.
        assert_eq!(profile_checksum(&0x1234_5678u32.to_le_bytes()), 0xDF8A_8A2B);
    }

    #[test]
    fn set_actuation_patches_both_tables_and_reseals() {
        let mut blob = Box::new([0u8; PROFILE_LEN]);
        set_actuation(&mut blob, Some(0x1A), 20).unwrap(); // W = slot 16

        assert_eq!(blob[PROFILE_ACTUATION_OFFSET + 16], 0x39);
        assert_eq!(blob[PROFILE_RESET_OFFSET + 16], 0x34);
        assert_eq!(blob[PROFILE_ACTUATION_OFFSET + 15], 0);
        let stored = u32::from_le_bytes(
            blob[PROFILE_CHECKSUM_OFFSET..PROFILE_CHECKSUM_OFFSET + 4]
                .try_into()
                .unwrap(),
        );
        assert_eq!(stored, profile_checksum(&blob[..PROFILE_CHECKSUM_OFFSET]));
    }

    #[test]
    fn set_actuation_rejects_unknown_values_and_keys() {
        let mut blob = Box::new([0u8; PROFILE_LEN]);
        assert!(set_actuation(&mut blob, None, 15).is_err());
        assert!(set_actuation(&mut blob, Some(0x29), 10).is_err()); // Esc has no slot
        assert!(blob.iter().all(|&b| b == 0), "a rejected edit must not touch the blob");

        set_actuation(&mut blob, None, 1).unwrap();
        assert!(
            blob[PROFILE_ACTUATION_OFFSET..PROFILE_ACTUATION_OFFSET + 68]
                .iter()
                .all(|&b| b == 0x05)
        );
        assert_eq!(blob[PROFILE_ACTUATION_OFFSET + 68], 0, "slots 68-69 stay unused");
    }

    #[test]
    fn chunks_follow_the_captured_header() {
        use crate::devices::hid_reports::{HidDeviceType, HidReportBuilder, PROFILE_CHUNK_REPORT_SIZE};

        let mut blob = Box::new([0u8; PROFILE_LEN]);
        blob[500] = 0xAB;
        blob[PROFILE_LEN - 1] = 0xCD;
        let builder = HidReportBuilder::new(HidDeviceType::Keyboard);
        let reports: Vec<[u8; PROFILE_CHUNK_REPORT_SIZE]> = profile_chunks(&blob)
            .map(|chunk| {
                let mut report = [0u8; PROFILE_CHUNK_REPORT_SIZE];
                assert_eq!(
                    builder.build_report(chunk, &mut report).unwrap(),
                    PROFILE_CHUNK_REPORT_SIZE
                );
                report
            })
            .collect();

        assert_eq!(reports.len(), 25);
        assert_eq!(
            &reports[1][..10],
            &[0x00, 0x03, 0x01, 0x80, 0xF4, 0x01, 0xF4, 0x01, 0x00, 0x00]
        );
        assert_eq!(reports[1][10], 0xAB);
        assert_eq!(&reports[24][6..8], &12_000u16.to_le_bytes());
        assert_eq!(reports[24][10 + 499], 0xCD);
        assert!(reports[24][10 + 500..].iter().all(|&b| b == 0));
    }
}
