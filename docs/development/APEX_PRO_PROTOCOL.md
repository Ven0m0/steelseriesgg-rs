# SteelSeries Apex Pro HID Protocol Documentation

## Overview

This document details the HID communication protocol for SteelSeries Apex Pro keyboards, focusing on RGB lighting control, actuation point adjustment, and device communication patterns.

**Last Updated:** 2026-03-01
**Target Devices:** Apex Pro series keyboards (primary: Apex Pro TKL 2023)
**Protocol Version:** Based on reverse engineering, hardware testing, and implementation analysis

---

## Product ID Mapping

### Currently Supported Keyboard Variants

| Model | Product ID | Zone Count | Current Status |
|-------|------------|------------|----------------|
| Apex Pro | `0x1610` | 1 (single zone) | Basic support |
| Apex Pro TKL | `0x1614` | 1 (single zone) | Basic support |
| **Apex Pro TKL (2023)** | **`0x1628`** | **9 zones** | **Primary target — enhanced support** |
| Apex 3 | `0x161A` | 10 zones | Basic support |
| Apex 3 TKL | `0x1622` | 9 zones | Enhanced support (dedicated struct) |
| Apex 5 | `0x161C` | 1 (single zone) | Basic support |
| Apex 7 | `0x1612` | 1 (single zone) | Basic support |
| Apex 7 TKL | `0x1616` | 1 (single zone) | Basic support |

> **⚠️ Important:** The 2023 TKL PID is `0x1628`, **not** the commonly referenced `0x1618` (which is the Apex 7 TKL). This was confirmed through direct hardware observation.

### Supported Headset Product IDs

| Model | Product ID |
|-------|------------|
| Arctis 1 / Arctis 7 (2017) | `0x12AD` |
| Arctis 1 Wireless | `0x12B3` |
| Arctis 5 | `0x12AA` |
| Arctis 7 (2019) | `0x12CF` |
| Arctis 9 | `0x12C2` |
| Arctis Pro | `0x1252` |
| Arctis Pro Wireless | `0x1290` |
| Arctis Nova Pro | `0x12E0` |
| Arctis Nova Pro Wireless | `0x12E4` |
| Arctis Nova 5 | `0x12EA` |
| Arctis Nova 3 | `0x12EC` |
| Arctis Nova 1 | `0x12EE` |

### USB Interface Configuration

- **Vendor ID:** `0x1038` (SteelSeries — official USB-IF assignment)
- **Keyboard Control Interface:** Interface 1 (RGB, actuation, configuration)
- **Headset Control Interface:** Interface 3
- **Keyboard Report Size:** 65 bytes (1 byte report ID `0x00` + 64 bytes data)
- **Headset Report Size:** 64 bytes (no report ID)
- **Communication Type:** HID Output Reports via `device.write()`

---

## HID Report Architecture

### Report Structure

All keyboard HID reports follow this format:
```
[Report ID: 0x00] [Command: 1 byte] [Data: 63 bytes]
Total: 65 bytes (zero-padded)
```

Headset reports omit the report ID:
```
[Command: 1 byte] [Data: 63 bytes]
Total: 64 bytes (zero-padded)
```

### HidReportBuilder (Recommended API)

Shared keyboard and headset commands should prefer `HidReportBuilder` from `src/devices/hid_reports.rs`. A few Apex-specific helpers in `src/devices/keyboards/apex.rs` still use direct byte-array construction for commands that do not yet have dedicated command structs.

```rust
use steelseries_gg::devices::hid_reports::{HidReportBuilder, HidDeviceType};

let builder = HidReportBuilder::new(HidDeviceType::Keyboard);
let mut buffer = [0u8; 65];
let size = builder.build_report(command, &mut buffer)?;
device.write(&buffer[..size])?;
```

Each command type implements the `HidCommand` trait:

```rust
pub trait HidCommand {
    fn command_code(&self) -> CommandCode;
    fn serialize(&self, buffer: &mut [u8], device_type: HidDeviceType) -> Result<usize>;
    fn validate(&self) -> Result<()>;
    fn description(&self) -> String;
}
```

### HID Write Optimization

The `write_padded_report()` function in `src/devices/mod.rs` includes a deduplication layer (`HidOptimizer`) that:
- Hashes outgoing reports using FNV-1a (u64)
- Caches sent reports for 50 ms to skip duplicate writes
- Reduces unnecessary I/O during high-frequency animation loops

---

## Known Commands

### Confirmed & Implemented

| Code | Name | Rust Type | Status |
|------|------|-----------|--------|
| `0x09` | Apply / Save | `ApplyCommand` | ✅ Working |
| `0x21` | RGB Zone Control | `RgbZoneCommand` | ✅ Working |
| `0x22` | Brightness | `BrightnessCommand` | ✅ Working |
| `0x23` | Per-Key RGB | `PerKeyRgbCommand` | ⚠️ Placeholder — protocol unknown |
| `0x25` | Reactive Mode | `CommandCode::ReactiveMode` | ✅ Implemented (Apex 3 TKL) |
| `0x26` | Color Shift | `CommandCode::ColorShift` | ✅ Implemented (Apex 3 TKL) |
| `0x2D` | Actuation Control | `ActuationCommand` | ✅ Experimental — write works |

### Command Details

#### Apply / Save Settings (`0x09`)
**Purpose:** Commit current settings to device.

```
[0x00] [0x09] [padding × 63]
```

- Parameterless command
- Must be sent after configuration commands (RGB, brightness, actuation)
- Used during `GenericKeyboard::apply()` and `GenericKeyboard::initialize()`

#### RGB Zone Control (`0x21`)
**Purpose:** Set RGB colors for keyboard zones.

```
[0x00] [0x21] [zone_selector] [R1] [G1] [B1] [R2] [G2] [B2] ... [padding]
```

- **Zone selector:** `0xFF` = all zones; specific index = single zone
- **Color data:** 3 bytes per zone (R, G, B), up to `MAX_RGB_ZONES` (12)
- **Implementation:** `RgbZoneCommand` struct with `Cow<[Color]>` for zero-copy borrowed slices

#### Brightness Control (`0x22`)
**Purpose:** Set global keyboard brightness.

```
[0x00] [0x22] [brightness] [padding × 62]
```

- **Range:** 0–100 (percentage), auto-clamped by `BrightnessCommand::new()`
- **Implementation:** `BrightnessCommand` struct

#### Per-Key RGB (`0x23`) — HID Code Based
**Purpose:** Individual key RGB control.

```
[0x00] [0x23] [hid_code] [R] [G] [B] [padding]
```

> ⚠️ **UNVERIFIED** — the packet format above is speculative. The command byte `0x23` and the field layout have **not been confirmed via USB capture**. The device may silently ignore this report or require a different structure.

> **Key-addressing breakthrough (2026-03-27):** Reverse engineering of SteelSeriesGG107.0.0Setup.exe confirmed that SteelSeries uses **USB HID keycodes** (not row/col matrix addresses) for per-key identification. The `hidCode` values are standard USB HID Usage IDs (0x04-0x64 for standard keys). The addressing *scheme* is verified; the *packet format* is not.

**Key Findings:**
- **Key Addressing:** Uses USB HID Usage IDs (e.g., 0x04 = A, 0x05 = B, 0x28 = Enter)
- **Complete TKL Keycode List:** 4-69, 73-82, 100, 133, 135-139, 224-231, 240 (87 keys)
- **Source:** Extracted from `apex_7+pro.migration` configuration file
- **Implementation:** `KeyAddress` now uses `hid_code: u8` instead of `(row, col)`

**HID Code Reference (Standard USB HID Usage IDs):**
| HID Code | Key | HID Code | Key | HID Code | Key |
|----------|-----|----------|-----|----------|-----|
| 0x04 | A | 0x10 | M | 0x28 | Enter |
| 0x05 | B | 0x11 | N | 0x29 | Escape |
| 0x06 | C | 0x12 | O | 0x2A | Backspace |
| 0x07 | D | 0x13 | P | 0x2C | Space |
| 0x08 | E | 0x14 | Q | 0x2D | - (minus) |
| 0x09 | F | 0x15 | R | 0x2E | = (equal) |
| 0x0A | G | 0x16 | S | 0x2F | [ |
| 0x0B | H | 0x17 | T | 0x30 | ] |
| 0x0C | I | 0x18 | U | 0x31 | \ |
| 0x0D | J | 0x19 | V | 0x33 | ; |
| 0x0E | K | 0x1A | W | 0x34 | ' |
| 0x0F | L | 0x1B | X | 0x35 | ` |
| ... | ... | 0x1C | Y | 0x36 | , |
| 0x1D | Z | 0x37 | . | 0x38 | / |
| 0x39 | Caps Lock | 0x3A-0x45 | F1-F12 | 0x47 | Scroll Lock |
| 0x4A-0x4D | Home, PgUp, Del, End | 0x50-0x53 | Arrow keys | 0x58-0x61 | F13-F24 |
| 0x62-0x64 | Keypad 00, 000, . | 0x65-0x68 | Menu, Power, etc. | 0xE0-0xE7 | Modifiers |
| 0x87 | Menu | 0x85-0x8B | Media keys | 0xF0 | SteelSeries key |

> **⚠️ Note:** The exact command format for `0x23` (single key vs batch, byte ordering) still requires USB capture verification. The HID code discovery confirms the addressing method but not the complete packet structure.

- **Implementation:** `PerKeyRgbCommand`, `PerKeyRgbBuilder`, `KeyAddress::hid_code()`
- **Status:** HID codes verified; packet format needs USB capture validation

#### Reactive Mode (`0x25`)
**Purpose:** Enable or disable reactive lighting (key-press triggers).

```
[0x00] [0x25] [0x01=enable / 0x00=disable] [padding]
```

- **Implementation:** `Apex3Tkl::set_reactive_mode()` (direct byte construction)
- **Status:** Confirmed working on Apex 3 TKL; untested on Apex Pro models

#### Color Shift (`0x26`)
**Purpose:** Transition between two colors at a configurable speed.

```
[0x00] [0x26] [R1] [G1] [B1] [R2] [G2] [B2] [speed] [padding]
```

- **Speed:** 0–100 (clamped)
- **Implementation:** `Apex3Tkl::set_color_shift()`

#### Actuation Point Control (`0x2D`) — Experimental
**Purpose:** Set the actuation distance for OmniPoint Hall Effect switches.

```
[0x00] [0x2D] [value] [padding × 62]
```

- **Value:** 0.1 mm increments (e.g., `4` = 0.4 mm, `36` = 3.6 mm)
- **Valid range:** 1–40 (0.1 mm – 4.0 mm)
- **Implementation:** `ActuationCommand` struct with `new()`, `from_mm()`, `to_mm()` helpers
- **Status:** Write works; **read is not implemented** (query command unknown)
- **Device:** Only meaningful on Apex Pro models (OmniPoint switches)

#### OSD Navigation (`0x24`) — Confirmed on hardware
**Purpose:** Navigate the keyboard's on-screen display / actuation menu.

```
[0x00] [0x24] [menu_position] [padding]
```

- **Status:** Confirmed to bring up the actuation menu on the Apex Pro TKL 2023 OLED
- **Implementation:** `Apex3Tkl::CMD_OSD_NAV` constant; no dedicated command struct yet

### Commands Under Investigation (0x20, 0x25–0x2F)

These commands were investigated with targeted helper binaries, USB capture, and manual hardware observation using value patterns such as `0x00, 0x04, 0x08, 0x14, 0x24, 0x30`. Functions are still not identified through observable behavior. See `PROTOCOL_RESEARCH.md` for the current testing workflow.

---

## Zone Mapping

### Apex Pro TKL (2023) — 9 Zone Layout

The zone mapping system is implemented in `src/devices/zone_mapping.rs` using the `ZoneMapping` struct with logical `ZonePosition` identifiers:

| Zone Index | Position | Physical Area | Critical |
|------------|----------|---------------|----------|
| 0 | `MainKeys` (left) | Left side keys | Yes |
| 1 | `MainKeys` (left-center) | Left-center area | Yes |
| 2 | `MainKeys` (center) | Center area | Yes |
| 3 | `MainKeys` (right-center) | Right-center area | Yes |
| 4 | `MainKeys` (right) | Right side keys | Yes |
| 5 | `FunctionRow` | F1–F12 keys | No |
| 6 | `Custom(0)` (NumberRow) | Number row (1–0) | No |
| 7 | `Custom(1)` (WASD) | WASD cluster | No |
| 8 | `ArrowKeys` | Arrow key cluster | No |

### Zone Fallback System

When per-key RGB is unavailable, the `ZoneFallback` system maps individual `KeyId` values to their nearest zone using `ZoneMapping`. This allows `simulate_per_key_with_zones()` to provide an approximation of per-key effects.

---

## Identified Gaps for Per-Key Control

### Current Limitations

1. **Zone-Only RGB:** Per-key control uses a placeholder command (`0x23`); packet format needs USB capture verification
2. **No Read Commands:** Cannot query current RGB state, actuation settings, or device mode
3. ~~**No Per-Key Addressing:** Physical key → HID matrix address mapping is placeholder data~~ **RESOLVED (2026-03-27):** HID codes discovered and implemented
4. **Protocol Variations:** Differences between Apex Pro variants not fully documented
5. **Actuation Read:** Write works (`0x2D`) but there is no known read-back command

### Missing Protocol Elements

#### Per-Key RGB
- Actual command byte(s) for individual key addressing
- Key matrix address format (row/col, sequential, or other)
- Batch per-key update mechanism and maximum payload

#### Device Query Commands
- Read current RGB zone colors
- Read current actuation point settings
- Query device capabilities / firmware version
- Device mode queries (software vs. hardware control)

#### Advanced Features
- Audio-reactive lighting protocol
- Profile switching commands
- Rapid Trigger enable/disable and sensitivity
- Macro programming interface (if supported via HID)

---

## Research Findings

### Working Commands (Confirmed on Hardware)

✅ **RGB Zone Control (0x21, 0xFF)** — Zone-based lighting
✅ **Brightness Control (0x22)** — Global brightness 0–100 %
✅ **Apply Settings (0x09)** — Commit changes to device
✅ **Reactive Mode (0x25)** — Enable/disable reactive lighting
✅ **Color Shift (0x26)** — Two-color transition effect
✅ **OSD Navigation (0x24)** — Opens actuation menu on keyboard OLED
✅ **Actuation Write (0x2D)** — Set actuation point (experimental)

### Known Issues

❌ **Random Key Targeting** — RGB commands sometimes affect unintended keys
❌ **Inconsistent Zone Response** — Some zones respond intermittently
❌ **Command Reliability** — Commands sometimes fail without error indication

### Hypothesis for Issues

1. **Missing Command Headers:** Per-key commands may require different header bytes
2. **Timing Issues:** Device may need delays between commands (current optimizer uses 50 ms cache)
3. **Checksum Requirements:** Commands may require validation bytes
4. **Mode Switching:** Device may need to be put in specific mode for per-key control
5. **Interface Selection:** May need different USB interface for per-key operations

---

## Implementation Status

### Current State (Zone-Based + Actuation)
```rust
// Working: Zone-based RGB control (async trait)
keyboard.set_zone_colors(&[Color::RED, Color::GREEN, Color::BLUE]).await?; // ✅
keyboard.set_color(Color::CYAN).await?;                                    // ✅

// Working: Global operations
keyboard.set_brightness(80).await?; // ✅
keyboard.apply().await?;            // ✅

// Working: Actuation point (Apex Pro TKL 2023 only)
let apex = ApexProTkl2023::new(generic_keyboard);
apex.set_actuation_point(8)?;       // 0.8 mm  ✅
apex.set_actuation_point_mm(1.5)?;  // 1.5 mm  ✅

// Working: Reactive mode (Apex 3 TKL)
apex3.set_reactive_mode(true)?;                                  // ✅
apex3.set_color_shift(Color::RED, Color::BLUE, 50)?;             // ✅
```

### Target State (Per-Key — Not Yet Working)
```rust
// Goal: Per-key RGB control (API exists, protocol unknown)
keyboard.set_key_color(KeyId::Q, Color::RED).await?;             // ⚠️ Placeholder protocol
keyboard.set_key_colors(&[(KeyId::W, Color::BLUE)]).await?;      // ⚠️ Placeholder protocol

// Fallback: Zone-based approximation (working)
keyboard.simulate_per_key_with_zones(&key_colors).await?;        // ✅ Uses ZoneFallback
```

---

## Next Steps for Protocol Discovery

### Priority Research Areas

1. **Per-Key Command Discovery**
   - Capture USB traffic while SteelSeries GG sets per-key colors (via WINE/Lutris)
   - Test command range 0x10–0x1F and 0x28–0x2C for undiscovered commands
   - Compare with MSI per-key protocol (0x0b/0x0e packets from msi-perkeyrgb)

2. **Actuation Point Read-Back**
   - Discover the HID input report or feature report that returns current actuation
   - Test `device.get_feature_report()` with various report IDs
   - Monitor input reports during actuation menu interactions

3. **Rapid Trigger Protocol**
   - Test 0x30–0x3F command range (hypothesized location)
   - Capture USB traffic while toggling Rapid Trigger in SteelSeries GG
   - Document enable/disable and sensitivity parameters

4. **Command Reliability**
   - Investigate timing requirements between commands
   - Test Feature Reports vs. Output Reports (`send_feature_report()` vs. `write()`)
   - Experiment with initialization sequences

### Research Methods

- **Packet Capture:** SteelSeries GG via WINE/Lutris + Wireshark/usbmon
- **Targeted Testing:** `discover_actuation` and `verify_key_mapping` for focused command discovery on live hardware
- **Reverse Engineering:** Analyze apex-tux, apex7tkl_linux, msi-perkeyrgb projects
- **Hardware Testing:** Visual / physical validation on actual devices

---

## References

### Current Implementation Files

| File | Purpose |
|------|---------|
| `src/devices/mod.rs` | Product ID constants, `DeviceType`, `write_padded_report()`, `HidOptimizer` |
| `src/devices/hid_reports.rs` | `HidReportBuilder`, all command structs (`RgbZoneCommand`, `BrightnessCommand`, `ActuationCommand`, etc.) |
| `src/devices/keyboards/mod.rs` | `Keyboard` trait (async), `GenericKeyboard` struct |
| `src/devices/keyboards/apex.rs` | `Apex3Tkl` — reactive mode, color shift, OSD nav |
| `src/devices/keyboards/apex_pro_tkl_2023.rs` | `ApexProTkl2023` — actuation point control |
| `src/devices/discovery.rs` | `DeviceManager` — hidapi enumeration, hot-plug events |
| `src/devices/key_mapping.rs` | `KeyId`, `KeyAddress`, `KeyMapping`, `KeyMappingDatabase` |
| `src/devices/zone_mapping.rs` | `ZoneMapping`, `ZonePosition`, `ZoneFallback`, `ZoneEffect` |
| `src/devices/diagnostics.rs` | `HidDiagnostics` — communication logging and analysis |
| `src/rgb/mod.rs` | `Color`, `Effect`, `PerKeyEffect`, `RgbController`, `PerKeyRgbController` |
| `src/gamesense/server.rs` | `GameSenseServer` — Axum HTTP server on port 27301 |
| `src/device_state.rs` | Device state persistence (write-behind cache) |

### Protocol Constants (from source)
```rust
// Vendor ID (src/lib.rs)
pub const STEELSERIES_VENDOR_ID: u16 = 0x1038;

// Product IDs (src/devices/mod.rs::product_ids)
pub const APEX_PRO: u16            = 0x1610;
pub const APEX_PRO_TKL: u16       = 0x1614;
pub const APEX_PRO_TKL_2023: u16  = 0x1628; // NOT 0x1618
pub const APEX_3: u16             = 0x161A;
pub const APEX_3_TKL: u16         = 0x1622;
pub const APEX_5: u16             = 0x161C;
pub const APEX_7: u16             = 0x1612;
pub const APEX_7_TKL: u16         = 0x1616;

// HID Command Codes (src/devices/hid_reports.rs::CommandCode)
pub const CMD_APPLY: u8           = 0x09;
pub const CMD_RGB_ZONE: u8        = 0x21;
pub const CMD_BRIGHTNESS: u8      = 0x22;
pub const CMD_PER_KEY_RGB: u8     = 0x23; // placeholder
pub const CMD_REACTIVE_MODE: u8   = 0x25;
pub const CMD_COLOR_SHIFT: u8     = 0x26;
pub const CMD_ACTUATION: u8       = 0x2D; // experimental

// Report Sizes (src/devices/hid_reports.rs)
pub const KEYBOARD_REPORT_SIZE: usize = 65; // with report ID
pub const HEADSET_REPORT_SIZE: usize  = 64; // without report ID
pub const MAX_RGB_ZONES: usize        = 12;
```

### External Resources
- [GameSense SDK](https://github.com/SteelSeries/gamesense-sdk) — Official high-level API
- [apex-tux](https://github.com/not-jan/apex-tux) — Rust OLED support for Apex keyboards
- [apex7tkl_linux](https://github.com/FrankGrimm/apex7tkl_linux) — Python RGB + OLED for Apex 7 TKL
- [msi-perkeyrgb](https://github.com/Askannz/msi-perkeyrgb) — Detailed per-key RGB protocol docs (MSI/SteelSeries)
- [apexctl](https://github.com/AstroSnail/apexctl) — C tool using hidapi-hidraw
- USB HID 1.11 Specification — Protocol standards

---

*This document will be updated as protocol research progresses and new commands are discovered.*
