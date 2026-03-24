import re

with open("src/devices/hid_reports.rs", "r") as f:
    content = f.read()

old_code = """    pub fn fragment_into_reports(&self) -> Vec<PerKeyRgbCommand> {
        let mut fragments = Vec::new();
        let chunk_size = Self::MAX_KEYS_PER_REPORT;
        let keys: Vec<_> = self.key_colors.iter().collect();

        // Handle empty case
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
    }"""

new_code = """    pub fn fragment_into_reports(&self) -> Vec<PerKeyRgbCommand> {
        let mut fragments = Vec::new();

        // Handle empty case
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
    }"""

if old_code in content:
    content = content.replace(old_code, new_code)
    with open("src/devices/hid_reports.rs", "w") as f:
        f.write(content)
    print("Successfully patched src/devices/hid_reports.rs")
else:
    print("Could not find old code in src/devices/hid_reports.rs")
