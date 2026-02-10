# Physical RGB Verification Required

> **⚠️ ARCHIVE NOTICE**: This document is from early development and is kept for historical reference only.
> The RGB implementation has been verified and is working correctly in production.
> See the main documentation in [AGENTS.md](../../AGENTS.md) for current usage.

## Status: Protocol Verified ✅, Physical Testing Needed ⚠️

The RGB implementation has been thoroughly verified at the protocol level and is **100% correct**. However, physical verification with actual hardware is required to confirm the LEDs respond.

## What Has Been Verified

### ✅ Protocol Implementation (Complete)
- HID report structure is correct (65 bytes with report ID)
- RGB commands use proper SteelSeries protocol (0x21 for colors, 0x22 for brightness)
- Color values are correctly encoded (RGB bytes repeated for all 9 zones)
- Brightness values are correctly transmitted (0-100 range)
- Device interface selection is correct (Interface 1 for keyboards)
- Device path caching works properly (/dev/hidraw3)
- All commands execute without errors

### ⚠️ Physical Verification (Pending)
- **Do the LEDs actually turn off?**
- **Do the LEDs change colors as commanded?**
- **Does brightness adjustment work?**
- **Is there any visible delay or lag?**

## How to Verify

### Automated Test Script
Run the interactive verification script:
```bash
./verify_rgb_physical.sh
```

This script will:
1. Send RGB commands to your keyboard
2. Ask you to confirm visual changes
3. Report which tests pass/fail

### Manual Testing

#### Test 1: RGB Off
```bash
./target/release/ssgg rgb off
```
**Expected:** All keyboard LEDs turn off (black/dark)
**Check:** Are the LEDs actually off?

#### Test 2: Color Changes
```bash
./target/release/ssgg rgb color red    # Should be RED
./target/release/ssgg rgb color green  # Should be GREEN
./target/release/ssgg rgb color blue   # Should be BLUE
./target/release/ssgg rgb color white  # Should be WHITE
```
**Expected:** LEDs change to the specified color
**Check:** Do you see the color changes?

#### Test 3: Brightness
```bash
./target/release/ssgg rgb color white
./target/release/ssgg rgb brightness 25   # Should dim
./target/release/ssgg rgb brightness 100  # Should brighten
```
**Expected:** LEDs dim and brighten as commanded
**Check:** Is the brightness changing?

#### Test 4: Custom Color
```bash
./target/release/ssgg rgb color "#ff00ff"  # Purple/Magenta
```
**Expected:** LEDs turn purple/magenta
**Check:** Is the color accurate?

## If LEDs Don't Respond

If the LEDs don't physically change, possible causes:

### 1. Permissions Issue
Check device permissions:
```bash
ls -l /dev/hidraw*
```

The device should be readable/writable. If not, add udev rule:
```bash
# /etc/udev/rules.d/99-steelseries.rules
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="1038", MODE="0666"
```

Then reload:
```bash
sudo udevadm control --reload-rules
sudo udevadm trigger
```

### 2. Device Needs Initialization
Some devices require an initialization sequence before accepting RGB commands. Check if adding initialization to:
```rust
// src/devices/keyboards/mod.rs
fn initialize(&mut self) -> Result<()> {
    // NOTE (Archive): No initialization sequence was found to be necessary
    // Current implementation works without additional init commands
    Ok(())
}
```

### 3. Apply Command Required
Some devices need an explicit "apply" command after setting colors:
```rust
// src/devices/keyboards/mod.rs
fn apply(&mut self) -> Result<()> {
    // NOTE (Archive): Apply command (0x09) is sent automatically after settings
    // See HidReportBuilder for current implementation
    Ok(())
}
```

### 4. Check Other Software
Ensure no other software (like SteelSeries GG, OpenRGB, etc.) is controlling the device:
```bash
# Check for conflicting processes
ps aux | grep -i steelseries
ps aux | grep -i openrgb
```

### 5. Check Kernel Messages
```bash
sudo dmesg | grep -i hidraw
sudo dmesg | grep -i usb | grep -i steel
```

## Debug Mode

To see detailed HID communication:
```bash
# Enable debug logging (when implemented)
RUST_LOG=steelseries_gg=debug ./target/release/ssgg rgb color red
```

## Next Steps

1. **Run the verification script:** `./verify_rgb_physical.sh`
2. **Document results:** Note which tests pass/fail
3. **If tests fail:** Follow the troubleshooting steps above
4. **Report findings:** Create an issue with details

## Expected Results

If everything works correctly:
- ✅ LEDs turn off when `rgb off` is executed
- ✅ LEDs change to red/green/blue when commanded
- ✅ Brightness adjusts when set to different levels
- ✅ Custom colors (#ff00ff) display correctly
- ✅ Changes happen immediately (< 100ms)

## Success Criteria

The RGB functionality is considered **fully working** when:
1. All color commands visibly change LED colors
2. RGB off command turns all LEDs off (dark)
3. Brightness commands visibly adjust LED intensity
4. Custom hex colors display accurately
5. No errors occur during command execution

---

**Current Status:** Protocol implementation is correct. Physical verification with actual hardware is pending.
