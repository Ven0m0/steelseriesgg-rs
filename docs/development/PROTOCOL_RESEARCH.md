# Protocol Research Findings — Apex Pro TKL 2023

**Last Updated**: 2026-03-01 (originally 2026-01-15)
**Status**: Active Testing Phase — Targeted Command Discovery

---

## Executive Summary

This document compiles reverse engineering resources and protocol discoveries for implementing full SteelSeries Apex Pro TKL 2023 support in steelseriesgg-rs. Key findings include:

- ✅ **WINE/Lutris compatibility** — SteelSeries GG can run on Linux via Lutris
- ✅ **Existing protocol implementations** — Multiple open-source projects with working Rust/Python code
- ✅ **RGB protocol documentation** — Detailed packet structures from MSI keyboard research
- ✅ **Systematic command testing** — Automated bulk testing system implemented (0x20–0x2F range)
- ✅ **Confirmed RGB commands** — 0x21 (zone colors), 0x22 (brightness), 0x23 (per-key placeholder)
- ✅ **Reactive & color shift** — 0x25 (reactive mode), 0x26 (color shift effect)
- ✅ **OSD navigation discovered** — 0x24 brings up actuation menu on keyboard OLED
- ✅ **Actuation control** — 0x2D write works (experimental); read command unknown
- 🔄 **Per-key RGB protocol** — Command `0x23` is a placeholder; actual protocol undiscovered
- ❌ **Rapid Trigger protocol** — Not yet tested (planned for 0x30–0x3F range)

---

## 1. Confirmed HID Protocol Commands (Apex Pro TKL 2023)

### Testing Methodology

**Device**: Apex Pro TKL 2023 (VID:`0x1038`, PID:`0x1628`)
**Approach**: Targeted HID command testing, USB capture comparison, and manual hardware verification
**Current Tools**:
- `cargo run --bin discover_actuation -- --pid 0x1628 --force` for actuation read-back discovery
- `cargo run --bin verify_key_mapping -- --product-id 0x1628 --fuzz` for exploratory per-key command probing
- `cargo run --bin verify_key_mapping -- --product-id 0x1628 --manual --start-row <ROW> --start-col <COL>` for focused matrix tests

### Bulk Testing Results (2026-01-15)

**Test Execution**:
- Command range: 0x20–0x2F (16 commands)
- Value patterns: 0x00, 0x04, 0x08, 0x14, 0x24, 0x30
- Total tests: 96 (16 commands × 6 values)
- Save command: 0x09 sent after each test
- Duration: ~3.4 minutes (2-second delays)
- Results: `test-results-1768466614.json`

### Confirmed Commands

| Command | Function | Rust Type / Impl | Status | Value Range | Notes |
|---------|----------|-------------------|--------|-------------|-------|
| `0x09` | Save/apply | `ApplyCommand` | ✅ Confirmed | N/A (parameterless) | Must follow config commands |
| `0x21` | RGB zone colors | `RgbZoneCommand` | ✅ Confirmed | 3 bytes per zone (RGB) | Zone selector byte precedes color data |
| `0x22` | RGB brightness | `BrightnessCommand` | ✅ Confirmed | 0x00–0x64 (0–100) | Global brightness control |
| `0x23` | Per-key RGB | `PerKeyRgbCommand` | ⚠️ Placeholder | Unknown | Actual protocol not discovered |
| `0x24` | OSD navigation | `Apex3Tkl::CMD_OSD_NAV` | ✅ Confirmed | Menu position | Brings up actuation menu |
| `0x25` | Reactive mode | `Apex3Tkl::set_reactive_mode()` | ✅ Implemented | 0x00=off, 0x01=on | Confirmed on Apex 3 TKL |
| `0x26` | Color shift | `Apex3Tkl::set_color_shift()` | ✅ Implemented | R G B R G B speed | Two-color transition |
| `0x2D` | Actuation control | `ActuationCommand` | ✅ Experimental | 1–40 (0.1 mm units) | Write works; read unknown |

### Commands Under Investigation (0x20, 0x27–0x2C, 0x2E–0x2F)

**Status**: Tested with value patterns; functions not yet identified through observable behavior.

Possible functions include:
- Direct actuation point setting (alternative to 0x2D)
- Per-key actuation configuration
- Rapid Trigger controls
- Additional OSD/menu commands
- Advanced RGB controls (effect speed, direction, etc.)

**Next Steps**:
1. Manual observation review of bulk test execution
2. Annotation of test results JSON with observed effects
3. Focused retesting of promising commands
4. USB capture comparison with SteelSeries GG commands

### HID Communication Pattern

**Report Structure** (Keyboards — 65 bytes):
```
[Report ID: 0x00] [Command: 0xXX] [Data: variable] [Padding: zeros to 65 bytes]
```

**Report Structure** (Headsets — 64 bytes):
```
[Command: 0xXX] [Data: variable] [Padding: zeros to 64 bytes]
```

**Example Commands**:

**Set RGB Color** (Command 0x21):
```rust
[0x00, 0x21, 0xFF, R1, G1, B1, R2, G2, B2, ..., 0x00, 0x00, ...]
       ^^^^  ^^^^  ^^^^^^^^^^^^^^^^^^^^^^^
       CMD   Zone  RGB data (up to 12 zones × 3 bytes)
             sel
```

**Set Brightness** (Command 0x22):
```rust
[0x00, 0x22, brightness, 0x00, 0x00, ...]
       ^^^^  ^^^^^^^^^^
       CMD   0-100 (decimal)
```

**Save Settings** (Command 0x09):
```rust
[0x00, 0x09, 0x00, 0x00, ...]
       ^^^^
       Parameterless save/apply command
```

**OSD Navigation** (Command 0x24):
```rust
[0x00, 0x24, value, 0x00, 0x00, ...]
       ^^^^  ^^^^^
       CMD   Position/menu parameter
```

**Reactive Mode** (Command 0x25):
```rust
[0x00, 0x25, 0x01, 0x00, ...]  // Enable
[0x00, 0x25, 0x00, 0x00, ...]  // Disable
```

**Color Shift** (Command 0x26):
```rust
[0x00, 0x26, R1, G1, B1, R2, G2, B2, speed, 0x00, ...]
       ^^^^  ^^^^^^^^^  ^^^^^^^^^  ^^^^^
       CMD   Color 1    Color 2    Speed (0-100)
```

**Actuation Point** (Command 0x2D):
```rust
[0x00, 0x2D, value, 0x00, 0x00, ...]
       ^^^^  ^^^^^
       CMD   0.1mm increments (1=0.1mm, 36=3.6mm, max 40=4.0mm)
```

### Actuation Point Details

**Encoding**: 0.1 mm increments
- 4 = 0.4 mm (typical minimum for competitive gaming)
- 8 = 0.8 mm (default)
- 20 = 2.0 mm
- 36 = 3.6 mm (maximum per SteelSeries spec)
- Max internal value: 40 = 4.0 mm (clamped by `ActuationCommand`)

**Implementation**:
```rust
// Via ActuationCommand struct
let cmd = ActuationCommand::new(8);     // 0.8mm
let cmd = ActuationCommand::from_mm(1.5); // Converts to 15 (1.5mm)
cmd.to_mm();                             // → 1.5

// Via ApexProTkl2023 wrapper
apex.set_actuation_point(8)?;           // Raw value
apex.set_actuation_point_mm(1.5)?;      // Millimeters
```

**Read-back**: A device-side query command has not been discovered yet. `Keyboard::read_actuation_point()` can return a cached session value after a successful write, but it does not currently read the live value back from the keyboard firmware.

### Testing Tools

**Current Discovery Tooling**:
```bash
# Probe for an actuation read-back command on the Apex Pro TKL 2023
cargo run --bin discover_actuation -- --pid 0x1628 --force

# Fuzz likely per-key command formats against the default device
cargo run --bin verify_key_mapping -- --product-id 0x1628 --fuzz

# Manually test a specific row/column pair with a chosen command byte
cargo run --bin verify_key_mapping -- \
  --product-id 0x1628 --manual --command-byte 0xB0 --start-row 0 --start-col 0
```

**Features**:
- `discover_actuation` safely skips known-dangerous commands and correlates responses with known actuation values
- `verify_key_mapping` supports fuzz, scan, and manual modes for per-key exploration
- Both tools are designed for live hardware observation rather than unattended bulk sweeps
- USB capture (`usbmon` / Wireshark) remains the best way to validate unknown commands against SteelSeries GG traffic

---

## 2. WINE Compatibility for Protocol Capture

### SteelSeries GG via Lutris

**Status**: Functional with caveats

**Installation Method**:
- Use Lutris installer: [SteelSeries GG — Lutris](https://lutris.net/games/steelseries-gg/)
- Requires additional setup from: https://github.com/asheriif/steelseries-linux-lutris

**Known Issues**:
- Wine 7.6 (April 2022) fixed installer crashes
- Device detection requires special configuration (see Lutris instructions)
- Taskbar applet must be properly closed after install for device recognition

**Benefit for Protocol Capture**:
- Allows running official SteelSeries GG software on Linux
- Enables USB traffic capture using Wireshark/usbmon while software communicates with device
- Can test and validate discovered commands against official software behavior

**References**:
- [SteelSeries GG — Lutris](https://lutris.net/games/steelseries-gg/)
- [WineHQ Forums — SteelSeries Engine 3](https://forum.winehq.org/viewtopic.php?t=30879)

---

## 3. Official Resources

### GameSense SDK (High-Level API)

**Repository**: [SteelSeries/gamesense-sdk](https://github.com/SteelSeries/gamesense-sdk)

**What It Provides**:
- HTTP-based API for game integration
- RGB per-key zone control (high-level)
- OLED screen content
- Event-driven lighting effects

**What It Doesn't Provide**:
- Low-level HID protocol details
- Direct actuation point control
- Rapid Trigger settings
- Raw USB command structures

**How to Use**:
```bash
# Find GameSense server port
cat %PROGRAMDATA%/SteelSeries/SteelSeries Engine 3/coreProps.json
# Send HTTP POST events to control RGB
curl -X POST http://127.0.0.1:27301/game_event ...
```

**Implementation Notes**:
- steelseriesgg-rs already implements a GameSense-compatible server in `src/gamesense/server.rs`
- Uses Axum HTTP server on 127.0.0.1:27301 with CORS protection
- This is a higher-level abstraction; doesn't reveal underlying HID protocol

**References**:
- [GameSense Developer Portal](https://steelseries.com/developer)
- [GameSense SDK Documentation](https://github.com/SteelSeries/gamesense-sdk/tree/master/doc/api)

---

## 4. Existing Linux Implementations

### 4.1 apex-tux (Rust — OLED Support)

**Repository**: [not-jan/apex-tux](https://github.com/not-jan/apex-tux)

**Supported Devices**:
- Apex Pro (0x1610)
- Apex Pro TKL (0x1614) ⭐ *Same family as 2023 model*
- Apex 7 (0x1612)
- Apex 7 TKL (0x1618)
- Apex 5 (0x161C)

**Protocol Discoveries**:

#### USB Communication Details
```rust
// From apex-hardware/src/usb.rs
const STEELSERIES_VENDOR_ID: u16 = 0x1038;

// Product IDs
ApexProTKL: 0x1614
Apex7: 0x1612
ApexPro: 0x1610
Apex7TKL: 0x1618
Apex5: 0x161C
```

#### OLED Protocol Pattern
- **Interface**: Interface 1 (multi-interface device)
- **Report Type**: Feature reports (`send_feature_report()`)
- **Data Format**: Raw framebuffer bytes
- **Communication**: Synchronous HID writes

**Implementation Language**: Rust
**Library**: hidapi-rs

**Relevance**: Demonstrates working HID communication patterns for Apex keyboards. Uses same vendor ID and similar product IDs. OLED protocol may share structural patterns with RGB/actuation commands.

**References**:
- [apex-tux GitHub](https://github.com/not-jan/apex-tux)
- [apex-hardware crate](https://github.com/not-jan/apex-tux/tree/master/apex-hardware)

---

### 4.2 apex7tkl_linux (Python — RGB + OLED)

**Repository**: [FrankGrimm/apex7tkl_linux](https://github.com/FrankGrimm/apex7tkl_linux)

**Features Implemented**:
- Individual and region-based RGB lighting control
- Profile/configuration switching
- OLED display manipulation
- Multiple key group definitions (FKEYS, ALPHA, NUMERIC)

**Protocol Approach**:
"Raw and unpolished code" — reverse engineered through trial and error. No formal protocol documentation included, but working Python implementation reveals command structures.

**Key Modules**:
- `device.py` — HID communication layer
- `payload.py` — Command packet construction
- `colors.py` — RGB command generation
- `oled.py` — OLED protocol
- `keys.py` — Key mapping tables

**Implementation Language**: Python
**Library**: hidapi Python bindings

**Relevance**: Working RGB control for Apex 7 TKL. Apex 7 series shares architecture with Apex Pro series, so RGB commands likely have overlap.

**References**:
- [apex7tkl_linux GitHub](https://github.com/FrankGrimm/apex7tkl_linux)

---

### 4.3 msi-perkeyrgb (Python — Comprehensive Protocol Docs)

**Repository**: [Askannz/msi-perkeyrgb](https://github.com/Askannz/msi-perkeyrgb)

**Why It Matters**: MSI gaming laptops use **SteelSeries keyboards**. This project reverse-engineered the complete RGB protocol by capturing traffic between SteelSeries Engine (Windows) and the keyboard using Wireshark.

**Protocol Documentation**:
[msi-kb-effectdoc](https://github.com/Askannz/msi-perkeyrgb/blob/master/documentation/0b_packet_information/msi-kb-effectdoc)

#### RGB Protocol Structure

**Packet Types**:

1. **0x0b Packet** — Effect Definition
   - Size: 533 bytes (0x00–0x20C)
   - Defines animation effects
   - Can be assigned to multiple keys

2. **0x0e Packet** — Key Assignment
   - Maps effects and colors to individual keys
   - Structure: `[Base RGB] [Reactive RGB] [Reactive Duration] [Effect ID] [Mode] 0x00 [Keycode]`

#### 0x0b Effect Packet Layout

| Byte Range | Purpose | Notes |
|------------|---------|-------|
| 0x00–0x01 | Packet header | `[0x0b 0x00]` |
| 0x02–0x81 | Transition blocks | 16 max, 8 bytes each |
| 0x82–0x83 | Filler | `[0x00 0x00]` |
| 0x84–0x89 | Starting color | RGB, 2 bytes each (nibble-swapped) |
| 0x8a–0x8b | Separator | `[0xFF 0x00]` |
| 0x8c–0x95 | Wave mode params | Origin, direction, wavelength |
| 0x96 | Transition count | Max 16 |
| 0x97 | Separator | `[0x00]` |
| 0x98–0x99 | Effect duration | Milliseconds, little-endian |
| 0x9a | Wave direction | 0=outward, 1=inward |
| 0x9b–0x20C | Padding | All zeros |

#### Color Encoding (Nibble Swap)
```python
# Unusual byte ordering
encoded_color = (color & 0x0F) << 4 + (color & 0xF0) >> 4
```

#### 0x0e Key Assignment Structure
```
[Base RGB (3 bytes)]
[Reactive RGB (3 bytes)]
[Reactive Duration (2 bytes)]
[Effect ID (1 byte)]
[Mode (1 byte)]
[0x00]
[Keycode (1 byte)]
```

**Mode Values**:
- `0` — Effect without refresh
- `1` — Inline static color
- `2` — Effect with auto-refresh
- `8` — Reactive (responds to key press)

**Reactive Duration**: 100–1000 ms in vendor software

**Implementation Language**: Python
**Library**: hidapi

**Relevance**: Most detailed RGB protocol documentation found. While specific byte values may differ for Apex Pro TKL 2023, the packet structure and general approach are likely similar.

**References**:
- [msi-perkeyrgb GitHub](https://github.com/Askannz/msi-perkeyrgb)
- [Protocol Documentation](https://github.com/Askannz/msi-perkeyrgb/blob/master/documentation/0b_packet_information/msi-kb-effectdoc)

---

## 5. Product ID Mapping

### Known Apex Keyboard Product IDs

| Model | Product ID | Notes |
|-------|-----------|-------|
| Apex Pro | 0x1610 | Full-size |
| Apex Pro TKL | 0x1614 | Original TKL |
| Apex 7 | 0x1612 | |
| Apex 7 TKL | 0x1618 | |
| Apex 5 | 0x161C | |
| Apex 3 | 0x161A | 10-zone RGB |
| Apex 3 TKL | 0x1622 | 9-zone RGB, dedicated struct |
| **Apex Pro TKL 2023** | **0x1628** | **Primary target device** |

**Vendor ID**: 0x1038 (all SteelSeries devices)

**Interface Numbers**:
- Interface 0: Keyboard HID (standard keypress reporting)
- Interface 1: Control interface (RGB, actuation, OLED)
- Interface 2–3: Additional interfaces (device-specific, headsets use interface 3)

> **Note**: Apex Pro TKL 2023 (0x1628) is NOT documented in existing open-source projects. Protocol must be discovered independently, but can build on patterns from 0x1614 (original Apex Pro TKL).

---

## 6. Actuation & Rapid Trigger Protocol

### Current Status

| Feature | Status | Command | Notes |
|---------|--------|---------|-------|
| Actuation write | ✅ Experimental | `0x2D` | Write confirmed; value = 0.1mm units |
| Actuation read | ❌ Unknown | ? | No query command discovered |
| Rapid Trigger | ❌ Unknown | ? | Not yet tested |

### What We Know (Feature Behavior)

**Actuation Point Adjustment** (OmniPoint Hall Effect):
- Range: 0.1 mm to 4.0 mm (internal limit; SteelSeries spec says 0.4–3.6 mm)
- Can be set globally via `ApexProTkl2023::set_actuation_point()`
- Per-key actuation: not yet implemented (command format unknown)
- Measurement precision: 0.1 mm increments

**Rapid Trigger**:
- Firmware update (2023) added this feature
- Eliminates fixed reset point
- Key releases as soon as upward movement detected
- Response time: 0.54 ms (claimed)
- Protocol: completely unknown — not yet tested

### Discovery Strategy

#### Method 1: WINE + Wireshark Capture
1. Install SteelSeries GG via Lutris
2. Configure device detection (per community guides)
3. Start Wireshark/usbmon capture on Linux
4. Change actuation/Rapid Trigger settings in SteelSeries GG
5. Analyze USB traffic for command patterns

**Tools**:
```bash
# USB capture
sudo modprobe usbmon
sudo wireshark
# Filter: usb.device_address == X && usb.src == "host"

# Or use usbmon directly
sudo cat /sys/kernel/debug/usb/usbmon/1u > capture.log
```

#### Method 2: Systematic HID Command Testing
1. Use `discover_actuation` to narrow candidate read-back commands and verify returned values against known writes
2. Use `verify_key_mapping` in fuzz/manual mode to test likely per-key packet layouts
3. Monitor keyboard behavior (actuation feel, Rapid Trigger responsiveness, LED changes)
4. Document successful commands and cross-reference them with USB captures from SteelSeries GG

**Safety**: Avoid firmware-related command ranges (likely 0xF0+). Stick to configuration commands.

#### Method 3: Firmware Analysis (Advanced)
- Extract firmware binary (if accessible via USB)
- Use tools like Ghidra for reverse engineering
- Look for command parsing routines

**Risk**: High complexity, low success rate without insider knowledge.

---

## 7. Recommended Implementation Plan

### Phase 1: Setup & Validation
1. ✅ Install Lutris + SteelSeries GG (via WINE)
2. ✅ Verify device detection in official software
3. ✅ Set up usbmon/Wireshark for USB capture
4. ✅ Test existing RGB commands from steelseriesgg-rs
5. ✅ Implement actuation write (0x2D)
6. ✅ Implement reactive mode (0x25) and color shift (0x26)

### Phase 2: RGB Protocol Reverse Engineering
1. Capture USB traffic during per-key RGB changes (SteelSeries GG → device)
2. Compare with known protocol structures (msi-perkeyrgb, apex-tux)
3. Test captured commands via steelseriesgg-rs
4. Document working commands in this file
5. Replace `PerKeyRgbCommand` placeholder with verified protocol

### Phase 3: Actuation & Rapid Trigger Discovery
1. Capture USB traffic during actuation adjustments
2. Discover actuation read-back command
3. Test Rapid Trigger enable/disable packets
4. Validate actuation changes (physical testing)
5. Add per-key actuation support if protocol allows

### Phase 4: Integration & Testing
1. Add profile persistence for new features
2. Create CLI commands for actuation and Rapid Trigger
3. Write integration tests
4. Update documentation

---

## 8. Existing Code in steelseriesgg-rs

### Current Architecture

The HID communication stack follows this pattern:

```
CLI (src/main.rs)
  → Keyboard trait (src/devices/keyboards/mod.rs)
    → GenericKeyboard / ApexProTkl2023 / Apex3Tkl
      → HidReportBuilder + Command structs (src/devices/hid_reports.rs)
        → write_padded_report() with HidOptimizer (src/devices/mod.rs)
          → hidapi device.write()
```

### Current RGB Implementation

```rust
// src/devices/keyboards/mod.rs — GenericKeyboard
async fn set_zone_colors(&mut self, colors: &[Color]) -> Result<()> {
    let cmd = RgbZoneCommand::new_all_zones(colors);
    let mut buffer = [0u8; KEYBOARD_REPORT_SIZE];
    let size = self.report_builder.build_report(cmd, &mut buffer)?;
    // ... writes via write_padded_report()
}
```

**Key Design Decisions**:
- Prefer `HidReportBuilder` for shared keyboard/headset commands; a few Apex-specific helper methods still use small direct packets in `src/devices/keyboards/apex.rs`
- Stack-allocated `[u8; 65]` buffers avoid heap allocation in hot paths
- `HidOptimizer` deduplicates identical reports within 50 ms windows (FNV-1a hashing)
- `Cow<[Color]>` in `RgbZoneCommand` allows zero-copy when borrowing color slices

### Actuation Implementation

```rust
// src/devices/keyboards/apex_pro_tkl_2023.rs
pub fn set_actuation_point(&mut self, value: u8) -> Result<()> {
    let command = ActuationCommand::new(value);
    command.validate()?;  // Range check: 1–40

    let report_builder = HidReportBuilder::new(HidDeviceType::Keyboard);
    let mut buffer = [0u8; KEYBOARD_REPORT_SIZE];
    let size = report_builder.build_report(command, &mut buffer)?;

    self.inner.send_raw(&buffer[..size])?;
    self.inner.update_cached_actuation_point(value);
    Ok(())
}
```

### Command Code Map (from `CommandCode` enum)

```rust
pub enum CommandCode {
    Apply         = 0x09, // Save/commit settings
    RgbControl    = 0x21, // Zone-based RGB
    Brightness    = 0x22, // Global brightness
    PerKeyRgb     = 0x23, // Per-key (placeholder)
    ReactiveMode  = 0x25, // Reactive lighting toggle
    ColorShift    = 0x26, // Two-color shift effect
    ActuationControl = 0x2D, // Actuation point (experimental)
}
```

---

## 9. Testing Checklist

### Hardware Testing Requirements
- [ ] Apex Pro TKL 2023 connected via USB
- [ ] Baseline RGB command works (`ssgg rgb --color red`)
- [ ] usbmon/Wireshark capture configured
- [ ] SteelSeries GG installed in WINE/Lutris
- [ ] Device detected by official software

### Capture Workflow
1. Start USB capture
2. Make ONE change in SteelSeries GG (e.g., set actuation to 1.0 mm)
3. Stop capture
4. Filter packets by device address
5. Identify HID output/feature reports from host → device
6. Extract hex data
7. Test command in steelseriesgg-rs

### Validation Tests
- **RGB**: Visual confirmation (color changes)
- **Actuation**: Physical testing (key press feel at different distances)
- **Rapid Trigger**: Rapid keypress test (measure repeat rate)

---

## 10. Safety Considerations

### What to Avoid
- ❌ Commands in `0xF0–0xFF` range (likely firmware/bootloader)
- ❌ Sending random data without validation
- ❌ Overwriting device configuration without backup
- ❌ Testing without ability to restore defaults

### Safety Practices
- ✅ Test on non-critical device if possible
- ✅ Document default settings before changes
- ✅ Start with read-only operations (queries)
- ✅ Verify commands are reversible
- ✅ Keep official software available for recovery
- ✅ Use `ActuationCommand::validate()` to enforce safe ranges

### Recovery Plan
1. If device becomes unresponsive:
   - Unplug USB
   - Wait 10 seconds
   - Reconnect
   - Use SteelSeries GG to restore defaults

2. If settings are corrupted:
   - Use official software to reset to factory defaults
   - Test with official software first before retrying custom commands

---

## 11. Next Actions

### Immediate Next Steps
1. **Sweep remaining command ranges**: 0x30–0x3F (Rapid Trigger hypothesis), 0x10–0x1F
2. **Capture per-key RGB traffic**: SteelSeries GG via WINE → Wireshark
3. **Discover actuation read command**: Test `get_feature_report()` with various report IDs
4. **Clone reference repositories** for code study:
   - `git clone https://github.com/not-jan/apex-tux`
   - `git clone https://github.com/Askannz/msi-perkeyrgb`

### Development Workflow
1. Capture → Analyze → Test → Document → Implement
2. Focus on one feature at a time (Per-Key RGB → Actuation Read → Rapid Trigger)
3. Commit protocol findings to this document incrementally
4. Update steelseriesgg-rs implementation as commands are validated

---

## 12. References & Resources

### Official Resources
- [GameSense Developer Portal](https://steelseries.com/developer)
- [GameSense SDK GitHub](https://github.com/SteelSeries/gamesense-sdk)
- [Apex Pro TKL 2023 Manual](https://support.steelseries.com/hc/en-us/articles/37474028182541-Apex-Pro-TKL-2023-Manual-and-Product-Information-Guide)
- [How to Change Actuation Point](https://support.steelseries.com/hc/en-us/articles/9645931478029)
- [How to Activate Rapid Trigger](https://support.steelseries.com/hc/en-us/articles/18742108024717)

### Community Projects
- [apex-tux (Rust)](https://github.com/not-jan/apex-tux)
- [apex7tkl_linux (Python)](https://github.com/FrankGrimm/apex7tkl_linux)
- [msi-perkeyrgb (Python + Protocol Docs)](https://github.com/Askannz/msi-perkeyrgb)
- [apexctl (Rust)](https://github.com/AstroSnail/apexctl)

### WINE/Lutris
- [SteelSeries GG — Lutris](https://lutris.net/games/steelseries-gg/)
- [SteelSeries Linux Lutris Setup](https://github.com/asheriif/steelseries-linux-lutris)
- [WineHQ — SteelSeries Engine Issues](https://forum.winehq.org/viewtopic.php?t=30879)

### Protocol Analysis Resources
- [MSI Keyboard Effect Protocol](https://github.com/Askannz/msi-perkeyrgb/blob/master/documentation/0b_packet_information/msi-kb-effectdoc)
- [USB HID Protocol Specification](https://www.usb.org/sites/default/files/documents/hid1_11.pdf)
- [Linux HID Introduction](https://docs.kernel.org/hid/hidintro.html)

---

**Document Version**: 2.0
**Contributors**: Research compiled from web sources, hardware testing, and open-source projects
**License**: MIT (same as steelseriesgg-rs project)
