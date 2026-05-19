use std::time::Instant;
use steelseries_gg::devices::hid_reports::{PerKeyAddressingMode, PerKeyRgbCommand};
use steelseries_gg::devices::key_mapping::KeyAddress;
use steelseries_gg::rgb::Color;

pub fn fragment_into_reports_original(cmd: &PerKeyRgbCommand) -> Vec<PerKeyRgbCommand> {
    let mut fragments = Vec::new();
    let chunk_size = PerKeyRgbCommand::MAX_KEYS_PER_REPORT;
    let keys: Vec<_> = cmd.key_colors.iter().collect();

    if keys.is_empty() {
        return vec![cmd.clone()];
    }

    for chunk in keys.chunks(chunk_size) {
        let mut fragment_command = PerKeyRgbCommand::new(cmd.addressing_mode);

        for (address, color) in chunk {
            fragment_command.key_colors.insert(**address, **color);
        }

        fragments.push(fragment_command);
    }

    fragments
}

fn main() {
    let mut cmd = PerKeyRgbCommand::new(PerKeyAddressingMode::HidCode);

    // Fill with 104 keys (standard full-size keyboard)
    for i in 0..104 {
        cmd.key_colors.insert(KeyAddress::new(i), Color::RED);
    }

    let iterations = 100_000;

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = fragment_into_reports_original(&cmd);
    }
    let duration_original = start.elapsed();

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = cmd.fragment_into_reports();
    }
    let duration_optimized = start.elapsed();

    println!("Original implementation: {:?}", duration_original);
    println!("Optimized implementation: {:?}", duration_optimized);

    let speedup = duration_original.as_secs_f64() / duration_optimized.as_secs_f64();
    println!("Speedup: {:.2}x", speedup);
}
