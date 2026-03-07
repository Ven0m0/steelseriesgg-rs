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

All HID report construction **must** use `HidReportBuilder` from `src/devices/hid_reports.rs`. Direct byte-array construction is discouraged.

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

#### Per-Key RGB (`0x23`) — Placeholder
**Purpose:** Individual key RGB control.

```
[0x00] [0x23] [addressing_mode] [key_data...] [padding]
```

> **⚠️ WARNING:** This command code is a **placeholder**. The actual per-key RGB protocol for SteelSeries keyboards has **not been reverse-engineered**. The current implementation falls back to `simulate_per_key_with_zones()` which approximates per-key effects using zone-based control.

- **Implementation:** `PerKeyRgbCommand`, `PerKeyRgbBuilder`
- **Addressing modes:** `Sequential`, `MatrixRowCol`, `DirectIndex` (defined but unverified)

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

These commands were tested systematically via the bulk testing harness (`src/bin/bulk_test.rs`) with value patterns `0x00, 0x04, 0x08, 0x14, 0x24, 0x30`. Functions not yet identified through observable behavior. See `PROTOCOL_RESEARCH.md` for testing methodology.

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

1. **Zone-Only RGB:** Per-key control uses a placeholder command (`0x23`); real protocol unknown
2. **No Read Commands:** Cannot query current RGB state, actuation settings, or device mode
3. **No Per-Key Addressing:** Physical key → HID matrix address mapping is placeholder data
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
- **Systematic Testing:** `bulk_test` binary for automated command-range sweeps
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
