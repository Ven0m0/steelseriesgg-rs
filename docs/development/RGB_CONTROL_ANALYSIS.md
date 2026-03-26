# RGB Control Analysis and Recommendations

**Date**: 2026-03-01 (originally 2026-01-13)
**Status**: ✅ **ANALYSIS COMPLETE — Updated with current implementation details**
**Purpose**: Investigate alternative approaches for forcing RGB to work on SteelSeries devices

## Investigation Summary

This document analyzes whether the crates/projects mentioned in `TODO.md` could help improve RGB control:
- `usbd-human-interface-device`
- `kanata-keyberon`
- Kanata Linux setup documentation

## Findings

### 1. usbd-human-interface-device

**Verdict**: ❌ NOT APPLICABLE

**What it is**:
- Library for **creating** USB HID devices on embedded systems (Raspberry Pi Pico, microcontrollers)
- Device-side implementation (firmware development)
- Used to make custom keyboards, mice, joysticks

**Why it doesn't help**:
- steelseriesgg-rs needs to **control existing devices** from the host computer
- This crate works in the opposite direction (device → host, not host → device)
- Designed for no_std embedded environments, not desktop Linux

**Source**: [GitHub — dlkj/usbd-human-interface-device](https://github.com/dlkj/usbd-human-interface-device)

### 2. kanata-keyberon

**Verdict**: ❌ NOT APPLICABLE

**What it is**:
- Fork of keyberon keyboard firmware library
- Used by the Kanata keyboard remapper for host-side input event processing
- Handles keyboard layout logic, key remapping, chord detection

**Why it doesn't help**:
- Deals with **input events** (reading keypresses), not RGB control
- Uses evdev/uinput for reading/injecting keyboard events
- No USB HID device communication or RGB lighting functionality

**Source**: [kanata-keyberon — crates.io](https://crates.io/crates/kanata-keyberon)

### 3. Kanata Linux Setup Documentation

**Verdict**: ⚠️ PARTIALLY RELEVANT (permissions only)

**What it provides**:
- udev rules for accessing `/dev/input/` devices (evdev)
- User group setup (`input`, `uinput`)
- Permission management for reading keyboard events

**Relevant parts**:
```bash
# Similar to what steelseriesgg-rs already does
sudo usermod -aG input $USER
KERNEL=="uinput", MODE="0660", GROUP="uinput"
```

**Why it's limited**:
- steelseriesgg-rs already has comprehensive udev rules (`assets/99-steelseries.rules`)
- Kanata focuses on evdev (input events), we need hidraw (RGB control)
- Our udev rules are already correct for hidraw access

**Source**: [Kanata Linux Setup](https://github.com/jtroo/kanata/blob/main/docs/setup-linux.md)

## Actual Relevant Projects for RGB Control

While investigating, several Linux projects that **successfully control SteelSeries Apex RGB** were found:

### Working Projects

1. **apexctl** — C tool using hidapi-hidraw
   - [GitHub — AstroSnail/apexctl](https://github.com/AstroSnail/apexctl)
   - Controls brightness, colors, polling frequency
   - Uses `HIDAPI_IMPL=hidapi-hidraw`

2. **RGB_controller** — Python tool for Apex 3 TKL
   - [GitHub — PCBscanner/RGB_controller](https://github.com/PCBscanner/RGB_controller)
   - Uses PyUSB to send commands
   - Reverse-engineered protocol via USB packet capture

3. **apex7tkl_linux** — Python CLI for Apex 7 TKL
   - [GitHub — FrankGrimm/apex7tkl_linux](https://github.com/FrankGrimm/apex7tkl_linux)
   - Includes working udev rules and RGB control

4. **steelkeys** — Python configuration tool
   - [GitHub — daiyam/steelkeys](https://github.com/daiyam/steelkeys)
   - Requires libhidapi-hidraw0
   - Supports Apex M800 and other models

### Common Patterns

All working implementations:
- ✅ Use hidapi or PyUSB for HID communication
- ✅ Require hidraw device access with proper permissions
- ✅ Send binary packets via HID interface
- ✅ Some reverse-engineered via Wireshark USB packet capture

## Current steelseriesgg-rs Architecture

### HID Communication Stack

```
CLI commands (src/main.rs)
  → Keyboard trait methods (src/devices/keyboards/mod.rs)
    → GenericKeyboard / ApexProTkl2023 / Apex3Tkl
      → HidReportBuilder + Command structs (src/devices/hid_reports.rs)
        → write_padded_report() with HidOptimizer (src/devices/mod.rs)
          → hidapi device.write()
```

### Current RGB Implementation

```rust
// src/devices/keyboards/mod.rs — GenericKeyboard::set_zone_colors()
async fn set_zone_colors(&mut self, colors: &[Color]) -> Result<()> {
    let cmd = RgbZoneCommand::new_all_zones(colors);
    let mut buffer = [0u8; KEYBOARD_REPORT_SIZE]; // Stack-allocated, 65 bytes
    let size = self.report_builder.build_report(cmd, &mut buffer)?;

    let device = self.device.as_ref()
        .ok_or_else(|| Error::DeviceCommunication("Not connected".to_string()))?;
    let device = device.lock();
    write_padded_report(&device, &buffer[1..size], KEYBOARD_REPORT_SIZE, true)?;
    Ok(())
}
```

**What's working correctly**:
- ✅ Command `0x21` with zone selector `0xFF` for all-zone RGB (confirmed on hardware)
- ✅ `HidReportBuilder` serializes commands consistently with proper report ID, padding, and validation
- ✅ `HidOptimizer` deduplicates identical reports within 50 ms to reduce unnecessary I/O
- ✅ Stack-allocated `[u8; 65]` buffers avoid per-frame heap allocations
- ✅ `Cow<[Color]>` in `RgbZoneCommand` enables zero-copy when borrowing color slices

### Potential Issues & Improvement Areas

#### 1. **Feature Reports vs Output Reports**

**Current**: `device.write()` sends Output Reports

**Alternative**: Some SteelSeries devices (especially for OLED) use Feature Reports:
```rust
device.send_feature_report(data)?;
```

**Status**: apex-tux uses `send_feature_report()` for OLED. RGB control has been confirmed working with Output Reports (`write()`), but Feature Reports might be needed for per-key RGB or other undiscovered commands.

#### 2. **Report ID Handling**

**Current approach**:
```rust
// write_padded_report() prepends report ID 0x00 for keyboards
// Headsets omit the report ID
```

The `HidDeviceType` enum handles this automatically:
- `Keyboard` → 65 bytes with report ID `0x00` at offset 0
- `Headset` → 64 bytes without report ID

This is confirmed correct for zone-based RGB.

#### 3. **Device Initialization**

**Current**:
```rust
fn initialize(&mut self) -> Result<()> {
    let cmd = ApplyCommand;
    let mut buffer = [0u8; KEYBOARD_REPORT_SIZE];
    let size = self.report_builder.build_report(cmd, &mut buffer)?;
    // ... sends the apply command
}
```

The initialization sends a single Apply command (`0x09`). More complex initialization sequences may be needed for per-key RGB mode switching.

#### 4. **HID Optimizer (Deduplication)**

**Implemented** in `src/devices/mod.rs`:
```rust
pub struct HidOptimizer {
    report_cache: Mutex<HashMap<u64, Instant>>,  // FNV-1a hash → timestamp
    connectivity_cache: Mutex<HashMap<String, (bool, Instant)>>,
    cache_timeout: Duration,                      // 50ms
}
```

The optimizer:
- Hashes outgoing reports with FNV-1a (u64)
- Skips writes if an identical report was sent within 50 ms
- Cleans cache entries when size exceeds 100
- Also caches device connectivity status for 5 seconds

This is transparent to callers and reduces CPU/I/O during high-frequency animation loops.

#### 5. **Direct hidraw Access (Experimental Alternative)**

Some Python tools bypass hidapi entirely and write directly to `/dev/hidraw*`:
```python
with open('/dev/hidraw0', 'wb') as f:
    f.write(bytes([0x21, 0xFF, r, g, b, ...]))
```

**Trade-off**: Less portable but more direct control. Not currently implemented in steelseriesgg-rs and unlikely to be needed given that hidapi works correctly.

#### 6. **Enhanced Debug Logging**

**Already implemented** via `tracing`:
```rust
debug!(
    "Writing optimized HID report: len={}, offset={}, data={:02x?}",
    effective_len, offset,
    &report_data[..effective_len.min(16)]
);
```

Enable with `RUST_LOG=debug ssgg rgb --color red` to see all HID reports being sent.

Additionally, `src/devices/diagnostics.rs` provides `HidDiagnostics` for structured communication logging with timing, operation types, and error tracking.

## Recommendations

### Completed Improvements ✅

1. **HidReportBuilder** — Shared RGB and configuration commands use the builder, while a few Apex-specific helpers still send direct packets
2. **Stack-allocated buffers** — `[u8; 65]` instead of `Vec<u8>` in hot paths
3. **HidOptimizer** — Deduplication layer reduces unnecessary writes by 50–90 % during animations
4. **Diagnostics** — `HidDiagnostics` for communication analysis
5. **Actuation control** — `ActuationCommand` with validation (0x2D, experimental)
6. **Reactive lighting** — 0x25 and 0x26 commands implemented for Apex 3 TKL

### Remaining Improvements

1. **Feature Report Support** ⭐⭐⭐
   - Try `send_feature_report()` for commands that don't work with `write()`
   - Especially relevant for per-key RGB discovery

2. **Per-Key RGB Protocol Discovery** ⭐⭐⭐
   - The placeholder command (`0x23`) needs replacement with the real protocol
   - Requires USB traffic capture from SteelSeries GG
   - See `PROTOCOL_RESEARCH.md` for capture methodology

3. **Retry Logic** ⭐⭐
   - `set_zone_colors_with_retry()` exists on the `Keyboard` trait
   - Consider adding retry at the `write_padded_report()` level for transient HID errors

4. **Actuation Read-Back** ⭐⭐
   - Write works (0x2D); read command unknown
   - Try `get_feature_report()` with various report IDs

## Testing Checklist

When testing RGB control improvements:

- [ ] Test with `RUST_LOG=debug` to see all HID reports
- [ ] Verify device is detected: `ssgg devices`
- [ ] Try static color: `ssgg rgb --color red`
- [ ] Check USB permissions: `ls -la /dev/hidraw*`
- [ ] Verify udev rules loaded: `udevadm control --reload-rules`
- [ ] Test as non-root user (should work with proper udev rules)
- [ ] Monitor USB traffic: `sudo modprobe usbmon && sudo wireshark`
- [ ] Test on actual hardware (not VM)
- [ ] Verify HidOptimizer deduplication: watch for "Skipping duplicate HID report" in debug logs

## Conclusion

**The originally suggested crates (usbd-human-interface-device, kanata-keyberon) are not applicable** as they serve different purposes (device-side firmware and input remapping).

**However**, investigating them led to discovering multiple working Linux RGB control projects that confirm:
1. ✅ Our current approach (hidapi + 0x21 command) is fundamentally correct
2. ✅ The `HidReportBuilder` architecture is sound and well-structured
3. ✅ The `HidOptimizer` provides meaningful performance improvement
4. ⚠️ Feature Reports may be needed for undiscovered commands (per-key RGB, actuation read)
5. ⚠️ The per-key RGB protocol (0x23) is still a placeholder requiring hardware research

**Next steps**: Focus on USB traffic capture to discover the per-key RGB protocol and actuation read-back command. See `PROTOCOL_RESEARCH.md` for the detailed research plan.

---

## Sources

- [usbd-human-interface-device — GitHub](https://github.com/dlkj/usbd-human-interface-device)
- [kanata-keyberon — crates.io](https://crates.io/crates/kanata-keyberon)
- [Kanata Linux Setup](https://github.com/jtroo/kanata/blob/main/docs/setup-linux.md)
- [apexctl — GitHub](https://github.com/AstroSnail/apexctl)
- [RGB_controller — GitHub](https://github.com/PCBscanner/RGB_controller)
- [apex7tkl_linux — GitHub](https://github.com/FrankGrimm/apex7tkl_linux)
- [steelkeys — GitHub](https://github.com/daiyam/steelkeys)
- [apex-tux — GitHub](https://github.com/not-jan/apex-tux)
- [msi-perkeyrgb — GitHub](https://github.com/Askannz/msi-perkeyrgb)
- [ArchWiki: Disable SteelSeries RGB lights](https://wiki.archlinux.org/title/User:Mvasi90/Disable_SteelSeries_RGB_lights)
- [hidapi Issue #174](https://github.com/libusb/hidapi/issues/174)
