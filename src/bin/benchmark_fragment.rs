use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyAddress {
    pub row: u8,
    pub col: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerKeyAddressingMode {
    Matrix,
    Logical,
}

#[derive(Debug, Clone)]
pub struct PerKeyRgbCommand {
    pub key_colors: HashMap<KeyAddress, Color>,
    pub addressing_mode: PerKeyAddressingMode,
}

impl PerKeyRgbCommand {
    pub const MAX_KEYS_PER_REPORT: usize = 12;

    pub fn fragment_into_reports_original(&self) -> Vec<PerKeyRgbCommand> {
        let mut fragments = Vec::new();
        let chunk_size = Self::MAX_KEYS_PER_REPORT;
        let keys: Vec<_> = self.key_colors.iter().collect();

        if keys.is_empty() {
            return vec![self.clone()];
        }

        for chunk in keys.chunks(chunk_size) {
            let mut fragment_command = PerKeyRgbCommand {
                key_colors: HashMap::new(),
                addressing_mode: self.addressing_mode,
            };

            for (address, color) in chunk {
                fragment_command.key_colors.insert(**address, **color);
            }

            fragments.push(fragment_command);
        }

        fragments
    }

    pub fn fragment_into_reports_optimized(&self) -> Vec<PerKeyRgbCommand> {
        let mut fragments = Vec::new();

        if self.key_colors.is_empty() {
            return vec![self.clone()];
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
}

fn main() {
    let mut cmd = PerKeyRgbCommand {
        key_colors: HashMap::new(),
        addressing_mode: PerKeyAddressingMode::Matrix,
    };

    // Fill with 104 keys (standard full-size keyboard)
    for i in 0..104 {
        let row = (i / 20) as u8;
        let col = (i % 20) as u8;
        cmd.key_colors
            .insert(KeyAddress { row, col }, Color { r: 255, g: 0, b: 0 });
    }

    let iterations = 100_000;

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = cmd.fragment_into_reports_original();
    }
    let duration_original = start.elapsed();

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = cmd.fragment_into_reports_optimized();
    }
    let duration_optimized = start.elapsed();

    println!("Original implementation: {:?}", duration_original);
    println!("Optimized implementation: {:?}", duration_optimized);

    let speedup = duration_original.as_secs_f64() / duration_optimized.as_secs_f64();
    println!("Speedup: {:.2}x", speedup);
}
