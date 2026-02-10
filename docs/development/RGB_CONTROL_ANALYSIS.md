# RGB Control Analysis and Recommendations

**Date**: 2026-01-13
**Status**: ✅ **ANALYSIS COMPLETE**
**Purpose**: Investigate alternative approaches for forcing RGB to work on SteelSeries devices

## Investigation Summary

This document analyzes whether the crates/projects mentioned in todo.md could help improve RGB control:
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

**Source**: [GitHub - dlkj/usbd-human-interface-device](https://github.com/dlkj/usbd-human-interface-device)

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

**Source**: [kanata-keyberon - crates.io](https://crates.io/crates/kanata-keyberon)

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
- steelseriesgg-rs already has comprehensive udev rules (assets/99-steelseries.rules)
- Kanata focuses on evdev (input events), we need hidraw (RGB control)
- Our udev rules are already correct for hidraw access

**Source**: [Kanata Linux Setup](https://github.com/jtroo/kanata/blob/main/docs/setup-linux.md)

## Actual Relevant Projects for RGB Control

While investigating, I found several Linux projects that **successfully control SteelSeries Apex RGB**:

### Working Projects

1. **apexctl** - C tool using hidapi-hidraw
   - [GitHub - AstroSnail/apexctl](https://github.com/AstroSnail/apexctl)
   - Controls brightness, colors, polling frequency
   - Uses `HIDAPI_IMPL=hidapi-hidraw`

2. **RGB_controller** - Python tool for Apex 3 TKL
   - [GitHub - PCBscanner/RGB_controller](https://github.com/PCBscanner/RGB_controller)
   - Uses PyUSB to send commands
   - Reverse-engineered protocol via USB packet capture

3. **apex7tkl_linux** - Python CLI for Apex 7 TKL
   - [GitHub - FrankGrimm/apex7tkl_linux](https://github.com/FrankGrimm/apex7tkl_linux)
   - Includes working udev rules and RGB control

4. **steelkeys** - Python configuration tool
   - [GitHub - daiyam/steelkeys](https://github.com/daiyam/steelkeys)
   - Requires libhidapi-hidraw0
   - Supports Apex M800 and other models

### Common Patterns

All working implementations:
- ✅ Use hidapi or PyUSB for HID communication
- ✅ Require hidraw device access with proper permissions
- ✅ Send binary packets via HID interface
- ✅ Some reverse-engineered via Wireshark USB packet capture

## Current steelseriesgg-rs Implementation

### What We're Doing Right

```rust
// src/devices/keyboards/mod.rs
fn set_zone_colors(&mut self, colors: &[Color]) -> Result<()> {
    let mut data = vec![0x21, 0xFF];  // RGB command

    for color in colors.iter().take(self.zone_count) {
        data.push(color.r);
        data.push(color.g);
        data.push(color.b);
    }

    self.send_report(&data)  // Sends to hidapi
}
```

This looks correct based on similar projects. The 0x21 command is standard for RGB control.

### Potential Issues

#### 1. **Report ID Handling**

**Current approach**:
```rust
write_padded_report(&device, data, 65, true)  // Always uses report ID 0x00
```

**Question**: Should we try different report IDs?
- Some devices use report ID 0x00 (implicit)
- Others require explicit report IDs
- Try: `[0x00, 0x21, 0xFF, ...]` vs `[0x21, 0xFF, ...]`

#### 2. **Feature Reports vs Output Reports**

**Current**: We use `device.write()` which sends Output Reports

**Alternative**: Try `device.send_feature_report()` for Feature Reports

Some SteelSeries devices may require Feature Reports instead:
```rust
// Try adding to GenericKeyboard
fn send_feature_report(&mut self, data: &[u8]) -> Result<()> {
    let device = self.device.as_ref()?;
    let device = device.lock()?;

    // hidapi send_feature_report
    device.send_feature_report(data)?;
    Ok(())
}
```

**Known issue**: [hidapi #174](https://github.com/libusb/hidapi/issues/174) mentions feature report failures on some devices

#### 3. **Device Initialization**

**Current**:
```rust
fn initialize(&mut self) -> Result<()> {
    let init_cmd = [0x09];  // "save" command
    let _ = self.send_report(&init_cmd);
    Ok(())
}
```

**Enhancement**: Try more comprehensive initialization:
```rust
fn initialize(&mut self) -> Result<()> {
    // Try multiple common init sequences
    let _ = self.send_report(&[0x09]);  // Save/commit
    let _ = self.send_report(&[0x28]);  // Alternative commit
    let _ = self.send_report(&[0x0C, 0x00]);  // Device wake/reset

    std::thread::sleep(Duration::from_millis(50));  // Let device settle
    Ok(())
}
```

#### 4. **Interface Selection**

**Current**: Opens first matching interface

**Verify**: Ensure we're using the correct HID interface
- Keyboards often have multiple interfaces (0, 1, 2)
- Interface 1 is usually for control (confirmed in CLAUDE.md)
- Check if DeviceManager correctly filters by interface number

#### 5. **Direct hidraw Access**

**Alternative approach**: Bypass hidapi and use raw `/dev/hidraw*` access

Many working Python tools use direct file I/O:
```python
# Example from working Python tools
with open('/dev/hidraw0', 'wb') as f:
    f.write(bytes([0x21, 0xFF, r, g, b, ...]))
```

Rust equivalent:
```rust
use std::fs::OpenOptions;
use std::io::Write;

fn send_via_hidraw(path: &str, data: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .open(path)?;
    file.write_all(data)?;
    Ok(())
}
```

**Trade-off**: Less portable, but more direct control

## Recommendations

### Immediate Actions (Priority Order)

1. **Add Feature Report Support** ⭐⭐⭐
   ```rust
   // Add method to Device trait and GenericKeyboard
   fn send_feature_report(&mut self, data: &[u8]) -> Result<()>;

   // Try feature reports if output reports fail
   if self.send_report(data).is_err() {
       self.send_feature_report(data)?;
   }
   ```

2. **Enhanced Debug Logging** ⭐⭐⭐
   ```rust
   // Log USB interface numbers, device paths
   debug!("Opening device: vendor={:04x} product={:04x} interface={}",
          info.vendor_id, info.product_id, info.interface_number);

   // Log actual HID report sent (already done)
   debug!("Sending HID report: {:02x?}", data);
   ```

3. **Try Different Report ID Prefixes** ⭐⭐
   ```rust
   // Test both approaches
   send_report(&[0x00, 0x21, 0xFF, ...]);  // Explicit report ID
   send_report(&[0x21, 0xFF, ...]);         // Implicit report ID
   ```

4. **Add Retry Logic** ⭐⭐
   ```rust
   fn send_report_with_retry(&mut self, data: &[u8], attempts: u32) -> Result<()> {
       if attempts == 0 {
           return Err(Error::InvalidConfig("Number of attempts must be at least 1".to_string()));
       }
       for attempt in 0..attempts {
           match self.send_report(data) {
               Ok(_) => return Ok(()),
               Err(e) if attempt < attempts - 1 => {
                   debug!("Attempt {}/{} failed, retrying: {:?}", attempt + 1, attempts, e);
                   std::thread::sleep(Duration::from_millis(50));
               }
               Err(e) => return Err(e),
           }
       }
       unreachable!()
   }
   ```

5. **Test Direct hidraw Access** ⭐ (experimental)
   - Add feature flag `direct-hidraw`
   - Implement fallback to raw `/dev/hidraw*` if hidapi fails
   - Compare results between hidapi and direct access

### Long-term Improvements

1. **USB Packet Capture for Verification**
   - Use Wireshark to capture official SteelSeries Engine traffic
   - Verify our byte sequences match exactly
   - Document any discrepancies

2. **Device-Specific Profiles**
   - Different initialization for different product IDs
   - Per-device command mappings
   - Handle quirks (e.g., Apex 3 vs Apex Pro TKL 2023)

3. **Kernel Driver Investigation**
   - Recent kernel patch: [LKML: steelseries: Add support for Arctis headset lineup](https://lkml.org/lkml/2026/1/11/618)
   - Check if kernel exposes better interfaces
   - Consider using kernel HID API directly

## Testing Checklist

When testing RGB control improvements:

- [ ] Test with `RUST_LOG=debug` to see all HID reports
- [ ] Verify device is detected: `cargo run -- devices`
- [ ] Try static color: `cargo run -- rgb --color red`
- [ ] Check USB permissions: `ls -la /dev/hidraw*`
- [ ] Verify udev rules loaded: `udevadm control --reload-rules`
- [ ] Test as non-root user (should work)
- [ ] Compare hidapi backend: `HIDAPI_IMPL=hidapi-hidraw` vs `hidapi-libusb`
- [ ] Monitor USB traffic: `sudo usbmon` or Wireshark
- [ ] Test on actual hardware (not VM)

## Conclusion

**The originally suggested crates (usbd-human-interface-device, kanata-keyberon) are not applicable** as they serve different purposes (device-side firmware and input remapping).

**However**, investigating them led to discovering multiple working Linux RGB control projects that confirm:
1. ✅ Our current approach (hidapi + 0x21 command) is fundamentally correct
2. ⚠️ We may need to try Feature Reports instead of Output Reports
3. ⚠️ Device initialization may need improvement
4. ⚠️ Interface selection should be verified

**Next steps**: Implement the recommended changes above, starting with Feature Report support and enhanced debugging.

---

## Sources

- [usbd-human-interface-device - GitHub](https://github.com/dlkj/usbd-human-interface-device)
- [kanata-keyberon - crates.io](https://crates.io/crates/kanata-keyberon)
- [Kanata Linux Setup](https://github.com/jtroo/kanata/blob/main/docs/setup-linux.md)
- [apexctl - GitHub](https://github.com/AstroSnail/apexctl)
- [RGB_controller - GitHub](https://github.com/PCBscanner/RGB_controller)
- [apex7tkl_linux - GitHub](https://github.com/FrankGrimm/apex7tkl_linux)
- [steelkeys - GitHub](https://github.com/daiyam/steelkeys)
- [apex-tux - GitHub](https://github.com/not-jan/apex-tux)
- [steelseries-linux - GitHub](https://github.com/MiddleMan5/steelseries-linux)
- [msi-kbd-backlight - GitHub](https://github.com/kyak/msi-kbd-backlight)
- [MSIKLM - GitHub](https://github.com/Gibtnix/MSIKLM)
- [msi-perkeyrgb - GitHub](https://github.com/Askannz/msi-perkeyrgb)
- [ArchWiki: Disable SteelSeries RGB lights](https://wiki.archlinux.org/title/User:Mvasi90/Disable_SteelSeries_RGB_lights)
- [LKML: HID: steelseries kernel patch](https://lkml.org/lkml/2026/1/11/618)
- [hidapi Issue #174](https://github.com/libusb/hidapi/issues/174)
- [OpenRGB Apex Support Issue](https://gitlab.com/CalcProgrammer1/OpenRGB/-/issues/326)
