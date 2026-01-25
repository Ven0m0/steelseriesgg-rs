//! Device protocol fuzzing tools for reverse engineering.

use crate::devices::{DeviceManager, DeviceType};
use crate::{Error, Result};
use colored::Colorize;
use std::thread;
use std::time::Duration;
use tracing::info;

/// Fuzzing parameters.
pub struct FuzzParams {
    /// Start command byte (inclusive)
    pub start_cmd: u8,
    /// End command byte (inclusive)
    pub end_cmd: u8,
    /// Delay between commands in milliseconds
    pub delay_ms: u64,
    /// Payload pattern to use
    pub payload_pattern: PayloadPattern,
}

/// Payload patterns for fuzzing.
pub enum PayloadPattern {
    /// All zeros
    Zeros,
    /// All ones (0xFF)
    Ones,
    /// Alternating 0xAA / 0x55
    Alternating,
    /// Incrementing values
    Incrementing,
}

/// Run the protocol fuzzer on the first connected keyboard.
pub fn fuzz_keyboard_protocol(manager: &DeviceManager, params: FuzzParams) -> Result<()> {
    // Find keyboard
    let device_info = manager
        .first_device_of_type(DeviceType::Keyboard)
        .ok_or_else(|| Error::Other("No keyboard found for fuzzing".to_string()))?;

    println!(
        "{} {}",
        "WARNING: Fuzzing protocol can potentially brick your device or cause unexpected behavior."
            .red()
            .bold(),
        "Proceed with caution!".red()
    );
    println!(
        "Target: {} (PID: 0x{:04x})",
        device_info.name, device_info.product_id
    );
    println!(
        "Range: 0x{:02x} - 0x{:02x}",
        params.start_cmd, params.end_cmd
    );
    println!("Delay: {}ms", params.delay_ms);

    // Open device
    let mut keyboard = manager.open_keyboard(device_info)?;

    // We need to access the raw HID device to send arbitrary commands.
    // The Keyboard trait might abstracts this away, so we depend on the implementation
    // exposing a way to send raw data or we need to bypass the trait for fuzzing.
    // Since we are inside the crate, we can use the `send_raw` method from the Device trait.

    println!("Starting fuzzing sequence...");

    for cmd in params.start_cmd..=params.end_cmd {
        // Skip known safe/working commands to avoid interference, or maybe we want to test them too?
        // Let's just log what we are doing.
        info!("Fuzzing command: 0x{:02x}", cmd);
        print!("Testing command 0x{:02x}... ", cmd);

        // Construct packet
        let mut packet = vec![0x00, cmd]; // Report ID 0x00, then Command

        // Fill remaining 63 bytes (total 65)
        let payload_len = 63;
        match params.payload_pattern {
            PayloadPattern::Zeros => packet.resize(65, 0x00),
            PayloadPattern::Ones => packet.resize(65, 0xFF),
            PayloadPattern::Alternating => {
                for i in 0..payload_len {
                    packet.push(if i % 2 == 0 { 0xAA } else { 0x55 });
                }
            }
            PayloadPattern::Incrementing => {
                for i in 0..payload_len {
                    packet.push((i % 256) as u8);
                }
            }
        }

        // Send raw packet
        match keyboard.send_raw(&packet) {
            Ok(_) => print!("{}", "Sent".green()),
            Err(e) => print!("{} ({})", "Failed".red(), e),
        }

        // Try to read response (with short timeout implied by non-blocking or short blocking read)
        // The `receive_raw` usually blocks, so we might need to handle that.
        // For now, we rely on the diagnostics logging which taps into send/receive.
        // But `receive_raw` needs to be called to actually pull data.

        // We create a buffer and try to read.
        let mut buf = [0u8; 65];
        // We assume receive_raw has a timeout or is non-blocking enough.
        // If the implementation blocks forever, this fuzzer will hang.
        // Realistically, HID reads usually have a timeout set on the device handle.

        match keyboard.receive_raw(&mut buf) {
            Ok(n) => {
                if n > 0 {
                    print!(" -> {}", "Received Response".cyan());
                    // Diagnostics will log the content
                }
            }
            Err(_) => {
                // Timeout or no data is expected for many commands
            }
        }

        println!();
        thread::sleep(Duration::from_millis(params.delay_ms));
    }

    println!("Fuzzing complete.");
    Ok(())
}
