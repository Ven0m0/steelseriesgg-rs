# Apex Pro TKL 2023 — HID Protocol Reference

**Device:** SteelSeries Apex Pro TKL 2023  
**VID:** `0x1038`  **PID:** `0x1628`  
**Last Updated:** 2026-05-27  

---

## USB Interface Layout

The keyboard exposes 5 USB interfaces (MI_00–MI_04):

| Interface | Description | Used for |
|-----------|-------------|----------|
| MI_00 | Standard HID keyboard | Keypress events |
| MI_01 | Vendor-defined HID | **RGB, actuation, config (control interface)** |
| MI_02 | Secondary HID | Additional keycodes |
| MI_03 | HID mouse | Mouse emulation keys |
| MI_04 | USB input device | Media/additional keys |

All configuration commands are sent to **MI_01** (usage page `0xFF00`, vendor-defined).

---

## Report Format

All keyboard feature/output reports are 65 bytes:

```
[0x00] [CMD] [DATA...] [0x00 padding...]
  ^      ^     ^-- up to 63 bytes of command-specific data
  |      +-- command byte
  +-- report ID (always 0x00 for this device)
```

Commands are sent via `hid_send_feature_report()` (Windows: `HidD_SetFeature`) or `hid_write()` depending on the command.

---

## Command Reference

### CONFIRMED commands (tested on hardware)

| Byte | Name | Format | Source |
|------|------|--------|--------|
| `0x09` | Apply/Save | `[0x00 0x09 0x00...]` | Hardware test |
| `0x21` | Zone RGB | `[0x00 0x21 zone R G B R G B...]` | Hardware test |
| `0x22` | Brightness | `[0x00 0x22 brightness 0x00...]` (0–100) | Hardware test |
| `0x24` | OSD Navigation | `[0x00 0x24 position 0x00...]` | Hardware test (opens actuation menu on OLED) |
| `0x25` | Reactive Mode | `[0x00 0x25 0x01/0x00 0x00...]` | Hardware test (Apex 3 TKL; compatible) |
| `0x26` | Color Shift | `[0x00 0x26 R1 G1 B1 R2 G2 B2 speed 0x00...]` | Hardware test |
| `0x2D` | Actuation Write | `[0x00 0x2D value 0x00...]` (1–40, 0.1 mm units) | Hardware test (write only) |

#### `0x21` Zone selector values
- `0xFF` = all zones simultaneously
- `0x00`–`0x08` = zone 0–8 (9-zone layout for Apex Pro TKL 2023)

#### `0x2D` Actuation encoding
- `1` = 0.1 mm, `8` = 0.8 mm (default), `20` = 2.0 mm, `40` = 4.0 mm (max)
- SteelSeries UI limits to 0.4–3.6 mm; firmware accepts 0.1–4.0 mm

---

### CONFIRMED — Apex Pro TKL 2023 per-key RGB (0x40)

Captured via `IOCTL_HID_SET_FEATURE` (code `0x000B0191`) from `SteelSeriesEngine.exe` on 2026-05-27.

**Packet layout — 645 bytes total:**

```
[0x00][0x40][0x54][key_id R G B] × 84 + [0x00 × 306]
  ^     ^     ^    \--- repeated 84 times (336 bytes) ---/
  |     |     |
  |     |     count byte — always 0x54 = 84 (fixed)
  |     command byte — 0x40
  report ID — 0x00
```

- **key_id**: USB HID keyboard Usage ID (e.g. `0x04`=A, `0x28`=Enter, `0xE0`=Left Ctrl)
- **R, G, B**: 0–255 color components
- Keys are listed in physical layout order (not ascending Usage ID order)
- Unused entries after the last key are zero-padded to fill 645 bytes
- Sent via `DeviceIoControl`/`NtDeviceIoControlFile` with `IOCTL_HID_SET_FEATURE`, not `WriteFile`
- Implementation: `Apex2023DirectCommand` in `src/devices/hid_reports.rs` (feature `experimental-apex-2023`)

### EXPERIMENTAL / PLACEHOLDER (not confirmed on hardware)

| Byte | Name | Notes |
|------|------|-------|
| `0x23` | Per-Key RGB | Placeholder for non-2023 keyboards. Exact packet structure unconfirmed. |

---

### UNKNOWN (Rapid Trigger, actuation read-back)

Probed with `discover_actuation` binary on 2026-05-27:
- Scanned all command codes 0x00–0xFF via `hid_send_feature_report` + `get_feature_report`
- **No read-back command found** — device did not respond with actuation data to any GET_FEATURE probe
- Conclusion: Read-back likely requires a specific query+interrupt-read pattern (write query command, then read HID input report), or is done via a separate report mechanism

Rapid Trigger: command byte completely unknown. Hypothesized range 0x2E–0x3F based on command space analysis. Requires live USB capture from official GG software to identify.

---

## Key Addressing

Per-key RGB uses **USB HID Usage IDs** (not row/column matrix):

| HID Code | Key | HID Code | Key |
|----------|-----|----------|-----|
| 0x04–0x1D | A–Z | 0x1E–0x27 | 1–0 |
| 0x28 | Enter | 0x29 | Escape |
| 0x2A | Backspace | 0x2B | Tab |
| 0x2C | Space | 0x3A–0x45 | F1–F12 |
| 0x39 | Caps Lock | 0x47 | Scroll Lock |
| 0x49 | Insert | 0x4A | Home |
| 0x4B | Page Up | 0x4C | Delete |
| 0x4D | End | 0x4E | Page Down |
| 0x50–0x52 | ←↓→ | 0x52 | ↑ |
| 0xE0–0xE7 | Modifiers (Ctrl/Shift/Alt/GUI, L+R) | 0xF0 | SteelSeries fn key |

**TKL keycode list** (from `apex_7+pro.migration`):
```
4–69, 73–82, 100, 133, 135–139, 224–231, 240
```

Source: extracted from `SteelSeriesGG107.0.0Setup.exe` configuration migration files.

---

## What Requires Live USB Capture

The following cannot be implemented without USB capture from official GG software:

1. **Per-key RGB exact packet format** — command `0x23` byte ordering, batch size, commit sequence
2. **Actuation read-back** — query command byte and response parsing
3. **Rapid Trigger enable/disable** — command byte, sensitivity parameter format
4. **Per-key actuation** — individual key actuation setting format

### How to perform capture

1. Open Wireshark, select USBPcap interface for the root hub containing VID_1038/PID_1628
2. Start capture
3. Trigger ONE action in SteelSeries GG (e.g., set a per-key color on the 'A' key)
4. Stop capture
5. Filter: `usb.transfer_type==0x01 && usb.endpoint_address.direction==0`
6. Extract `usb.capdata` field from HID OUT reports
7. The keyboard is on `USB\ROOT_HUB30\4&5375334&0&0` — filter by that hub

---

## Implementation Status

| Feature | Status | Rust location |
|---------|--------|---------------|
| Zone RGB (0x21) | ✅ Working | `src/devices/hid_reports.rs::RgbZoneCommand` |
| Brightness (0x22) | ✅ Working | `src/devices/hid_reports.rs::BrightnessCommand` |
| Apply (0x09) | ✅ Working | `src/devices/hid_reports.rs::ApplyCommand` |
| Actuation write (0x2D) | ✅ Experimental | `src/devices/keyboards/apex_pro_tkl_2023.rs::set_actuation_point` |
| Reactive mode (0x25) | ✅ Working | `src/devices/keyboards/apex.rs::Apex3Tkl` |
| Color shift (0x26) | ✅ Working | `src/devices/keyboards/apex.rs::Apex3Tkl` |
| Per-key RGB (0x40, Apex 2023) | ✅ Confirmed | `src/devices/hid_reports.rs::Apex2023DirectCommand` (feature `experimental-apex-2023`) |
| Per-key RGB (0x23, other) | ⚠️ Placeholder | `src/devices/hid_reports.rs::PerKeyRgbCommand` |
| Actuation read-back | ❌ Unknown protocol | Needs USB capture |
| Rapid Trigger | ❌ Unknown protocol | Needs USB capture |
| Windows HID | ✅ Fixed | `src/devices/keyboards/mod.rs::send_feature` |
