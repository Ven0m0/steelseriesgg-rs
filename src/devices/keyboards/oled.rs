//! Apex Pro TKL 2023 (`0x1628`) 128x40 monochrome OLED frames: file parsing for
//! [`Apex2022DisplayCommand::Oled`](crate::devices::hid_reports::Apex2022DisplayCommand::Oled) and
//! [`OnboardImage`](crate::devices::hid_reports::Apex2022DisplayCommand::OnboardImage). The 640 bytes are the
//! image row-major, 16 bytes per row, MSB = leftmost pixel (the PBM `P4` raster layout): confirmed on hardware
//! 2026-11-10 (firmware 1.19.7) with the test pattern (a one-pixel line across the top), a border-and-diagonal
//! image and the image GG itself had sent.

use crate::{Error, Result};

pub const OLED_WIDTH: usize = 128;
pub const OLED_HEIGHT: usize = 40;
/// One bit per pixel.
pub const OLED_FRAME_LEN: usize = OLED_WIDTH * OLED_HEIGHT / 8;

/// A frame whose first 16 bytes are `0xFF`: a one-pixel line across the top (confirmed 2026-11-10).
pub fn test_pattern() -> [u8; OLED_FRAME_LEN] {
    let mut frame = [0u8; OLED_FRAME_LEN];
    frame[..16].fill(0xFF);
    frame
}

/// Frame from a file's bytes: exactly [`OLED_FRAME_LEN`] bytes are a raw frame and sent as is; anything else
/// must be a 128x40 binary PBM (`P4`), whose raster already has the board's layout.
pub fn frame_from_file(bytes: &[u8]) -> Result<[u8; OLED_FRAME_LEN]> {
    if let Ok(raw) = bytes.try_into() {
        return Ok(raw);
    }
    parse_pbm(bytes)
}

/// Raster of a binary PBM (`P4`) that must be exactly 128x40. Header comments (`#` to end of line) are allowed.
pub fn parse_pbm(bytes: &[u8]) -> Result<[u8; OLED_FRAME_LEN]> {
    let invalid = |msg: String| Error::InvalidConfig(format!("OLED image: {msg}"));
    let rest = bytes.strip_prefix(b"P4").ok_or_else(|| {
        invalid(format!(
            "expected a binary PBM (P4) file or a raw {OLED_FRAME_LEN}-byte frame, got {} bytes",
            bytes.len()
        ))
    })?;
    let mut pos = 0;
    let mut size = [0usize; 2];
    for field in &mut size {
        loop {
            match rest.get(pos) {
                Some(b) if b.is_ascii_whitespace() => pos += 1,
                Some(b'#') => {
                    while rest.get(pos).is_some_and(|&b| b != b'\n') {
                        pos += 1;
                    }
                }
                _ => break,
            }
        }
        let start = pos;
        while rest.get(pos).is_some_and(u8::is_ascii_digit) {
            pos += 1;
        }
        *field = std::str::from_utf8(&rest[start..pos])
            .ok()
            .and_then(|digits| digits.parse().ok())
            .ok_or_else(|| invalid("malformed PBM header".to_string()))?;
    }
    if size != [OLED_WIDTH, OLED_HEIGHT] {
        return Err(invalid(format!(
            "PBM is {}x{}, the OLED is {OLED_WIDTH}x{OLED_HEIGHT}",
            size[0], size[1]
        )));
    }
    // Exactly one whitespace byte separates the header from the raster.
    if !rest.get(pos).is_some_and(u8::is_ascii_whitespace) {
        return Err(invalid("malformed PBM header".to_string()));
    }
    let raster = &rest[pos + 1..];
    raster.try_into().map_err(|_| {
        invalid(format!(
            "PBM raster is {} bytes, a {OLED_WIDTH}x{OLED_HEIGHT} image has {OLED_FRAME_LEN}",
            raster.len()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Raster with pixels (0,0), (9,0) and (127,39) lit.
    fn three_pixels() -> [u8; OLED_FRAME_LEN] {
        let mut raster = [0u8; OLED_FRAME_LEN];
        raster[0] = 0x80; // (0,0)
        raster[1] = 0x40; // (9,0)
        raster[39 * 16 + 15] = 0x01; // (127,39)
        raster
    }

    fn pbm(header: &str, raster: &[u8]) -> Vec<u8> {
        let mut file = header.as_bytes().to_vec();
        file.extend_from_slice(raster);
        file
    }

    #[test]
    fn frame_from_pbm_with_comment() {
        let file = pbm("P4\n# made by hand\n128 40\n", &three_pixels());
        assert_eq!(frame_from_file(&file).unwrap(), three_pixels());
    }

    #[test]
    fn frame_from_raw_640_bytes_is_sent_as_is() {
        let raw = three_pixels();
        assert_eq!(frame_from_file(&raw).unwrap(), raw);
    }

    #[test]
    fn pbm_rejects_wrong_dimensions() {
        let err = parse_pbm(&pbm("P4 128 32\n", &[0u8; 512])).unwrap_err().to_string();
        assert!(err.contains("128x32"), "{err}");
        assert!(parse_pbm(&pbm("P4 40 128\n", &[0u8; OLED_FRAME_LEN])).is_err());
    }

    #[test]
    fn pbm_rejects_bad_header_and_short_raster() {
        assert!(parse_pbm(&pbm("P1 128 40\n", &[0u8; OLED_FRAME_LEN])).is_err());
        assert!(parse_pbm(b"P4 128").is_err());
        assert!(parse_pbm(&pbm("P4 128 40\n", &[0u8; 639])).is_err());
        assert!(frame_from_file(&[0u8; 639]).is_err());
    }

    #[test]
    fn test_pattern_is_16_lit_bytes() {
        let frame = test_pattern();
        assert!(frame[..16].iter().all(|&b| b == 0xFF));
        assert!(frame[16..].iter().all(|&b| b == 0));
    }
}
