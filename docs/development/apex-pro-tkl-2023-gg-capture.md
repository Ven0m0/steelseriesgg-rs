# Apex Pro TKL (2023, wired, PID 0x1628): SteelSeries GG wire capture

Status: **[EXPERIMENTAL]**. Everything below was read from USB captures of the official SteelSeries GG
(Windows) talking to a real Apex Pro TKL (VID 0x1038, PID 0x1628, firmware reported as `1.19.7`),
captured with USBPcap 1.5.4.0 on 2026-10-14. The bytes are observed GG traffic. Only a solid-colour frame
(2026-10-14) and per-key frames (2026-10-21) have been replayed from this project (see open question 3), so nothing here is "confirmed"
in the sense of `.claude/rules/experimental-protocol.md`.

## Transport

All host-to-device traffic went to USB interface 1 (`wIndex = 1`) as HID class SET_REPORT control
transfers (`bmRequestType 0x21`, `bRequest 9`). Replies came back on interrupt IN endpoint 0x82.

| Channel | `wValue` | Length | Use |
|---------|----------|--------|-----|
| Feature report, ID 0 | `0x0300` | 644 | per-key lighting, on-board profile upload |
| Output report, ID 0 | `0x0200` | 64 | commands and polls |

The 644 is the on-wire length; it excludes the report-ID byte. For `0x1628` the code builds a
`WIRED_2023_PACKET_LENGTH = 645` buffer (report-ID byte + 644 wire bytes) in `apex_pro_tkl_2023.rs`. `GenericKeyboard::send_feature`
(`keyboards/mod.rs`) sends the frame on the device's own hidapi handle on every platform.
**Needs a hardware test on `0x1628`**: if the keyboard ignores
direct-mode frames sent on the collection discovery opened, the interface-1 control collection
(the one carrying this model's profile traffic on `wIndex 1`) has to be opened as its own device.
The Linux path for `0x1628` is untested.

## Per-key lighting (feature report, command 0x40)

```
wire offset  0      1      2 + 4*i ...        338 .. 643
             0x40   count  [usage][R][G][B]   zero padding
```

- `count` was always `0x54` (84) in 8149 frames.
- `usage` is a USB HID keyboard usage ID. GG sent exactly these 84 usages, in this order:
  `04-27, 29, 2a-30, 33-45, 49-52, e0-e5, e7, f0, 28, 31, e6`
  (no 0x32, 0x46-0x48, 0x53-0x64, 0x87-0x8b, 0xfb).
- GG streams frames continuously (about 19 per second), not only on change. While GG's engine
  runs, a one-shot frame from another program is replaced within a frame or two; with GG closed the
  keyboard keeps the last frame. `ssgg rgb` warns about this on Windows.
- Brightness 0 in GG produced all-zero RGB in these frames (software scaling). A mid-level
  brightness was not distinguishable in this capture.
- The `0x4B` init report used by the code was not seen: GG's startup precedes the capture.
- `ssgg rgb color` and `ssgg rgb perkey` both build this frame as a feature report. Per-key sends all 84
  entries; unlisted keys keep the colour of the last frame `ssgg` saved (`direct-frame-1628.bin` in the config
  directory) and are black only when none is saved, since no read-back of the current colours is known. The old
  `experimental-apex-2023` path (65-byte output report) was removed on 2026-10-15.

## On-board profile (feature report, header `03 01 80`)

GG rewrites the whole profile after every settings change: 25 chunks, 12,500 bytes in total.

```
0x03 0x01 0x80 | len u16 LE (=500) | offset u16 LE (0, 500, .. 12000) | 0x00 0x00 | 500 data bytes
```

Data bytes start at wire offset 9. Blob offsets below are into the reassembled 12,500 bytes.

Per-key settings live in tables of 70 one-byte slots (slots 0..67 used, 68..69 always zero). The
field names come from GG's stored config (see "GG's stored config" below).

| Blob offset | Content | Values seen |
|-------------|---------|-------------|
| 1080 | `08 00 00 00` | `device_profile_schema` = 8 (spec `base_apex_2022_wired`, see "Blob layout from the spec" below) |
| 1084 + slot | Actuation point (`hall_thresholds.h`) | 0.1 mm = 0x05, 1.0 = 0x15, 1.8 = 0x2F, 2.0 = 0x39, 3.0 = 0x86, 4.0 = 0xE0 |
| 1154 + slot | Reset point (`hall_thresholds.l`) | 0.1 mm = 0x05, 1.0 = 0x12, 1.8 = 0x2A, 2.0 = 0x34, 3.0 = 0x7b, 4.0 = 0xDB |
| 1224 | `04 00 00 00` x3, then zeros | `zone_levels` u32 x3 (default 4 4 4), then `zone_key_subscriptions[70]`; what a zone level does is unknown |
| 1446 + slot | Release mode (`release_mode`) | 0 = off, 2 = Rapid Trigger, 3 = Key Guard |
| 1516 + slot | Rapid Trigger sensitivity (`rapid_trigger_sensitivity`) | default 2 |
| 1590 + slot | `protection_dist_10`, all 0x14 (2.0 mm): Key Guard protection distance; `protection_dur_ms` u32 at 1586 (500) | `second_actuation_thresholds` are the `multi_actuation_v2` tables at 1306 / 1376 |
| 2650 | Rapid Tap pairs (`rapid_tap_pairs`), stored as HID usages | default `04 07` (A, D) |
| 2700 | Rapid Tap enable (`rapid_tap_enable`) | 0 = off, 1 = on |

Key W is slot 16 (actuation byte at 1100, reset byte at 1170, mode byte at 1462).

- The actuation encoding is non-linear (a raw sensor scale). The full 0.1-4.0 mm table (40 steps) is GG's:
  the spec's `hall-thresholds-to-actuation-alist` (`apex_pro_tkl_2022`) and the identical `mm -> "l:h"` map in
  GG's UI bundle (`resources\app.asar`, byte offset 11774110), read 2026-10-25; `ACTUATION_POINTS` holds it.
  The board's own OLED-menu sync reports agree for every 0.2 mm step (see "Hardware results, 2026-10-25").
- Earlier versions of this note called offset 1170 a "second threshold". It is the reset point
  (`hall_thresholds.l`) of the same setting. `h` = 47, `l` = 42 is **1.8 mm**, the board's default, not 1.0 mm
  as the 2026-10-21 note below claimed; the original 1.0 mm value `h` = 21, `l` = 18 was right.

### Slot map (slot = index, HID usage in hex)

Slots follow the physical rows, left to right. Evidence: **C** = captured with a single-key edit
(`run2.pcap`, `run3.pcap`), **S** = matched as a set against GG's stored config (values unique to that
set; identity inside the set follows the ISO/JIS layout), **R** = row order, not observed directly.
The same table is `WIRED_2023_PROFILE_SLOTS` in `src/devices/keyboards/apex_pro_tkl_2023.rs`.

| Slots | Keys |
|-------|------|
| 0-13 | `` ` `` 35 (R, the only key left for the slot), 1 1E (C), 2 1F, 3 20, 4 21, 5 22, 6 23, 7 24, 8 25, 9 26, 0 27, - 2D, = 2E (R), Yen 89 (S) |
| 14-27 | Tab 2B (S), Q 14, W 1A (C), E 08, R 15, T 17, Y 1C, U 18, I 0C, O 12 (R), P 13 (C), [ 2F, ] 30 (R), \ 31 (C) |
| 28-41 | Caps 39 (S), A 04 (C), S 16, D 07, F 09, G 0A, H 0B, J 0D, K 0E (R), L 0F (C), ; 33, ' 34 (R), ISO-# 32 (S), Enter 28 (C) |
| 42-55 | LShift E1 (C), ISO-\ 64 (S), Z 1D (C), X 1B, C 06, V 19, B 05, N 11, M 10, , 36, . 37 (R), / 38 (C), Ro 87 (S), RShift E5 (C) |
| 56-67 | LCtrl E0, LGui E3, LAlt E2 (C), Muhenkan 8B (S), Space 2C (C), Henkan 8A, Kana 88 (S), RAlt E6, RGui/Menu E7, Fn F0, RCtrl E4, Backspace 2A (C) |

Esc, the F-row and the navigation cluster have no slot (not analog in GG's config).

### GG's stored config

GG keeps the per-key settings as JSON in its engine database,
`C:\ProgramData\SteelSeries\GG\apps\engine\db\database.db`, table `configurations`, column `settings`, active
row of `device_id` 242 (`devices.name` = `apex_pro_tkl_2022`, product 0x1628). Keys:
`hall_thresholds.keys[{hid,l,h}]` (68 entries, sorted by HID), `second_actuation_thresholds`,
`release_mode`, `rapid_trigger_sensitivity`, `rapid_tap_enable`, `rapid_tap_pairs`. Open it read-only
with GG stopped (Python: `sqlite3.connect("file:<path>?mode=ro", uri=True)`). The list order is by HID,
not by slot, so it does not give the slot map by itself; keys with unique values in it (ISO/JIS keys
default to `h` 47 / `l` 42; Key Guard keys) anchor slots.
### Checksum (solved 2026-10-15)

The little-endian u32 at blob offset 12320..12323 is the STM32 hardware CRC-32 over blob bytes
`0..12320`: polynomial `0x04C11DB7`, init `0xFFFFFFFF`, no input/output reflection, no final XOR, with the
data read as little-endian 32-bit words and each word fed MSB first (each 4-byte group byte-reversed
before a plain CRC-32/MPEG-2). It matches all 26 captured blobs (8 distinct). Plain byte-order
CRC-32/MPEG-2 does not match, which is why the earlier search missed it.

How it was found: the checksum deltas were GF(2)-linear in the data deltas (offset 1462 values 00/01/02),
and a reveng-style GCD over diff pairs gave exactly `0x104C11DB7` once the 4-byte word swap was applied.
Caveat: only offsets 1100, 1113, 1170, 1183, 1462 and 2700 varied in the captures.

Code: `src/devices/keyboards/apex_pro_tkl_2023_profile.rs` (`profile_checksum`, `seal_profile`,
`set_actuation`, `profile_chunk_reports`). The read-back (`ssgg onboard gen3-read`, `83 01`) is confirmed on
hardware since 2026-11-10 (all five slots, CRC matching), which lifts the "never write" rule once a read, decode
and re-encode round trip is byte-identical; the write stays behind `--yes` (see "GG actions capture, 2026-11-10").

## Output reports (64 bytes)

| Bytes (trailing zeros trimmed) | Seen | Reply on EP 0x82 |
|--------------------------------|------|------------------|
| `02 01 80`, `b1`, `b2`, `34`, `90` | repeating poll every few seconds | `90` returns `90 00 "1.19.7"` (firmware version) |
| `38 66 01` / `38 66 00` | Rapid Tap on / off in GG | `38 00 66` |
| `38 67 0a 00 04 07 <n> ...` | 4 times while cycling a 4-state option; `<n>` was 1, 2, 3, 0 | not captured |

Meaning of `b1`, `b2`, `34` and the `38 67` payload is unknown.

## Live HID trace and engine memory read (2026-10-21)

Method, on the owner's machine (Windows, real `0x1628`, GG running): Frida attached read-only to
`SteelSeriesEngine.exe`, hooking `ReadFile`/`WriteFile`/`DeviceIoControl` on kernel32 (no Ghidra needed, no
target modification). First two attach attempts coincided with the engine restarting on its own (no crash
report in the Windows Application event log either time); a third attach, hooking only small-buffer
I/O, ran stably for the whole session. Separately, the engine's own heap was read (`ReadProcessMemory`).
Over about one minute, including the owner opening the keyboard's actuation settings page in GG:

- **No `GET_FEATURE` control transfer ever happened.** Only `SET_FEATURE`, every time 645 bytes, the known
  `0x40` per-key lighting frame (GG's own ~19 fps stream). This checks every feature-report transaction
  the process made to any HID device for the whole trace, not a sample.
- **Output-report polling is exactly the already-listed set**: `02 01 80`, `b1`, `b2`, `34`, `90`, cycling
  every few seconds. Opening the actuation page triggered none of these, and no new command.
- **Conclusion: this firmware/GG version has no actuation (or lighting) read-back at all.** GG's actuation
  UI is backed entirely by its own cached config, not a hardware query. This matches what `ssgg actuation`
  already showed indirectly (`0x2D` has no effect on `0x1628`, confirmed by the owner 2026-10-21) and
  explains it: there is nothing on the wire to read, not a missing `ssgg` command.
- The engine's heap held the exact live JSON settings object GG keeps for this device (device id 242,
  profile name `Config 5`), decoded whole via `ReadProcessMemory` + JSON parsing of the heap bytes
  (no decryption of anything was needed or attempted; this is the engine's live, already-decoded state).
  Confirmed field shapes, matching `src/devices/keyboards/apex_pro_tkl_2023_profile.rs` and the slot map
  above: `hall_thresholds.keys[{h,hid,l}]` (all 68 keys), `release_mode.keys[{hid,mode}]`,
  `rapid_trigger_sensitivity.keys[{hid,sensitivity}]`, `rapid_tap_pairs.pairs[{hid_1,hid_2,mode,report_both_keys}]`,
  `second_actuation_thresholds.keys[{h,hid,l}]`.
  Live values: every key at `h=47,l=42`, `release_mode` off on every key, `rapid_trigger_sensitivity=2`
  uniformly, `rapid_tap_enable=0`, `second_actuation_thresholds` all zero.
- **Actuation calibration conflict, resolved in favour of the live read.** The owner confirmed GG's UI
  showed **1.0mm** at the time of this read; the board itself read `h=47(0x2F), l=42(0x2A)` for every key.
  The code's 1.0mm point used to be `h=0x15(21), l=0x12(18)`, from an earlier capture that was never checked
  against a labelled UI value. `ACTUATION_POINTS` now uses the live `(0x2F, 0x2A)` pair for 1.0mm (owner's
  call, 2026-10-21: the live-captured, UI-labelled value outranks the unlabelled one). The `apex-pro-tkl-2024-cyber`
  device file's `hall-thresholds-to-actuation-alist` (recovered from the same heap read, included via
  `(include "apex_pro_tkl_2022")`) is a different table entirely: raw hall value &rarr; a 1-40 "actuation
  level" index for the Prism reactive-lighting effect, not mm depth; unrelated to this point.
- Base device file confirmed by name: a resident `apex-pro-tkl-2024-cyber`/`-steel` device definition
  contains `(include "apex_pro_tkl_2022")`, matching the code's assumption about which `.edevice` family
  `0x1628` belongs to. The `apex_pro_tkl_2022` file's own `api`/read-logic body was not found resident in
  the heap during this session (the engine appears not to keep it after initial load), so this trace could
  not confirm or rule out a read-back command by reading the spec's source; the live-traffic result above
  (no `GET_FEATURE`, no new poll on UI actuation view) is the direct evidence instead.
- `.edevice` decryption was not pursued: GG decrypts these into memory at its own startup, so reading the
  engine's live, already-decoded state (as above) answered the open questions here without needing to break
  the encryption.

## Engine to HID transport (2026-11-10)

Static disassembly only, no process attached: engine `SteelSeriesEngine.exe` (Go, sha256
`F1C32B85...C582`), HID shim `SSEdevice.dll` (sha256 `D56EC159...F8C0`), both from GG vcs
`70d240b7`, installed 2026-11-02. The live-trace attempt at the end is separate and produced nothing.

`HIDDLL.dll` (2016 build, sha256 `EDF32A6A...DA25`) is not on the keyboard path. `SSEdevice.dll`
imports no `HT_*` symbol from it; it opens the device with `CreateFileW` and runs its own overlapped
I/O. `DeviceLib_SendCommand` (export, `0x180004f10`) is what the Go side reaches through cgo
(`_Cfunc_DeviceLib_SendCommand`).

### Transport name to chunk type

The transport names that appear in the device specs are registered as golisp constants in
`device/support.InitHIDCommandChunkTypeConstants` (VA `0x140492c20`) and
`InitSSECommandChunkTypeConstants` (next function, size `0x12e0`). Each entry is
`MakeSymbol(name)` -> `newobject(int64)` -> `mov qword [rax], imm` -> define, and an absent `imm`
store means the constant is `0`. The HID table:

| Value | Name | Value | Name |
|-------|------|-------|------|
| `0x00` | `HIDIO` | `0x07` | `HIDFEATUREREADONLY` |
| `0x01` | `HIDFEATURE` | `0x08` | `CNXT_OWB_WRITE` |
| `0x02` | `HIDNOW` | `0x09` | `CNXT_OWB_READ` |
| `0x03` | `HIDREADONLY` | `0x0a` | `SSFWSTATUS` |
| `0x04` | `HIDSLEEP` | `0x0b` | `HIDIONOFAIL` |
| `0x05` | `HIDCONFIG` | `0x0c` | `SS_OWB_WRITE` |
| `0x06` | (not registered) | `0x0d` | (not registered) |
| `0x0e` | `HIDFEATUREOUTINPUTIN` | `0x0f` | (not registered) |
| `0x10` | `HIDOUTPUTFEATUREIN` | | |

`SSFWFIZZ` (`0x1f`), `SSFW` (`0x03`), `SSFWLEGACY` (`0x0d`), `SSFWGENERIC` (`0x16`),
`HOLTEKFW` (`0x15`) and the rest of the firmware names come from the second table and never touch the
HID dispatch below.

### Chunk type to Win32 call

`DeviceLib::HandleHIDCmd` (`0x180008e00`) takes the 32-bit chunk type, clears bit 31 (`btr`),
rejects anything above `0x10`, and jumps through the table at `0x180009188`. Types that share the
second-table entry `0x180008eb6` pick an operation kind from `0x1800091cc`; types with their own
case use a dedicated writer (one-wire, `SSFWSTATUS`, `HIDCONFIG`).

Bit 31 of the chunk type selects the transport object's write method (`[vtbl+0x28]`) instead of its
read method (`[vtbl+0x20]`). `ApiChunk.Serialize` (`0x14048c0c0`) is where the engine sets that bit,
from the byte at `+0x18` of the chunk. The read method is the function at `0x180056000` with its kind
table at `0x1800563a4`; the write method is `0x1800563f0` with its kind table at `0x180056840`. Both
end in the same four calls, each guarded on a report length cached in the device object:

| Type | Transport | Kind | Read | Write |
|------|-----------|------|------|-------|
| `0x00` | `HIDIO` | 0 | `ReadFile`, size == input length | `WriteFile`, size == output length |
| `0x01` | `HIDFEATURE` | 1 | `DeviceIoControl 0xB0192`, size <= feature length | `0xB0191`, size == feature length |
| `0x02` | `HIDNOW` | 2 | `HidD_GetInputReport`, size <= input length | `WriteFile` |
| `0x03` | `HIDREADONLY` | 3 | `ReadFile` | rejected, `0xbeefbeef` |
| `0x04` | `HIDSLEEP` | 0 | as `HIDIO` | as `HIDIO` |
| `0x05` | `HIDCONFIG` | - | `[vtbl+0x38]`, payload 12 or 16 bytes only | same |
| `0x07` | `HIDFEATUREREADONLY` | 4 | `DeviceIoControl 0xB0192` | rejected, `0xbeefbeef` |
| `0x08` | `CNXT_OWB_WRITE` | - | `DeviceLib::WriteOneWirePacket` | - |
| `0x09` | `CNXT_OWB_READ` | - | case `0x1800090e0` | - |
| `0x0a` | `SSFWSTATUS` | - | case `0x180009148` | - |
| `0x0b` | `HIDIONOFAIL` | 5 | rejected | `WriteFile`, failures swallowed |
| `0x0c` | `SS_OWB_WRITE` | - | case `0x180009075` | - |
| `0x0d` | (not registered) | 6 | returns success, no I/O | same |
| `0x0e` | `HIDFEATUREOUTINPUTIN` | 7 | `ReadFile` | `DeviceIoControl 0xB0191` |
| `0x0f` | (not registered) | 8 | `DeviceIoControl 0xB0192` | `0xB0191` |
| `0x10` | `HIDOUTPUTFEATUREIN` | 9 | `DeviceIoControl 0xB0192` | `WriteFile` |

`0xB0191` is `IOCTL_HID_SET_FEATURE` and `0xB0192` is `IOCTL_HID_GET_FEATURE`. Failure codes seen on
the paths: `0xbad512e` (size mismatch), `0xfeadfa11` (I/O failed), `0xbeefbeef` (device not found, or
a transport that has no such direction). Every call is overlapped: `CreateEventW`, the call, then on
`ERROR_IO_PENDING` (997) `WaitForSingleObject` with the object's timeout at `+0x1a8` and
`GetOverlappedResult`.

The `HIDFEATUREOUTINPUTIN` row is the one that explains the mixed specs: it reads with a plain
`ReadFile` and writes with `SET_FEATURE`, which is exactly the shape of the `read_button_mappings`
style APIs that write a request as a feature report and then wait for the answer on the input pipe.

### Report lengths

All guards test against three dwords in the device object, filled directly from `HIDP_CAPS` right
after `HidP_GetCaps` at `0x180053e6e`:

```
0x180053e9c  movzx eax, word [rsp+0x64] ; caps.InputReportByteLength
0x180053ea1  mov   dword [rsi+0x60], eax
0x180053ea4  movzx eax, word [rsp+0x66] ; caps.OutputReportByteLength
0x180053ea9  mov   dword [rsi+0x64], eax
0x180053eac  movzx eax, word [rsp+0x68] ; caps.FeatureReportByteLength
0x180053eb1  mov   dword [rsi+0x68], eax
```

These are the Windows HID caps, so they include the report-ID byte. On `0x1628` the specs define a
65-byte input and output report and a 645-byte feature report, and the corresponding caps are 65 and
645. The guards are exact equality on the write path (`jz`), so the chunk payload must already be
padded to the caps length before the call. The padding site was not identified; the specs' `(size N)`
chunk field is the payload size, not the buffer size.

### What this settles

- `HIDFEATURE` frames leave as `IOCTL_HID_SET_FEATURE`, which is what the earlier capture saw on the
  wire as `SET_REPORT` wValue `0x0300`, 645 bytes on `0x1628`. `HIDIO` frames leave as a plain
  `WriteFile` on the output report, which matches the 64-byte output reports listed above.
- `HIDREADONLY`, `HIDFEATUREREADONLY` and `HIDIONOFAIL` explain the spec vocabulary: a read-only
  transport cannot be written at all, and a `HIDIONOFAIL` write is sent with the result ignored.
- ssgg's own path (`HidReportBuilder` plus `write`/`send_feature_report`) is the same two calls, so
  the transport layer is not where any remaining difference between GG and ssgg lives.
- Two transports are already tied to frames captured from GG on 2026-10-25 (see "Hardware results",
  below): `00 38 61 <68 keys>` arrived as `IOCTL_HID_SET_FEATURE`, 645 bytes (`HIDFEATURE`), and
  `38 66 01` / `38 66 00` arrived on `WriteFile` as output reports (`HIDIO`). The remaining transports
  in the table are static only.

### Live trace attempt, same day

Frida 17 (Python) attached read-only to `SteelSeriesEngine.exe` (PID 14204) and
`SteelSeriesPrism.exe` (PID 14132), hooking `DeviceIoControl` (only `0xB0191`/`0xB0192`) and
`WriteFile`, over 6, 10, 12, 14 and 25 second windows, with three media-key presses (volume up,
next, previous) during two of them. Result: **zero HID calls from either process**, so GG touches the
keyboard only on a real state change. A round of GG UI interaction is still needed to tie each
transport to a captured frame; that is the open part of this section.

### Which process owns the lighting

`SteelSeriesEngine.exe` is the only GG process that can reach the keyboard:

| Process | Binary | HID capability |
|---------|--------|----------------|
| `SteelSeriesEngine.exe` | Go, imports `ssedevice.dll` (43 functions) | owns the device path |
| `SteelSeriesPrism.exe` | Go 1.24.3, `github.com/steelseries/prismSync`, same vcs revision `70d240b7` | imports only `kernel32.dll`, `msvcrt.dll`, `user32.dll`; no `hid.dll`, no `SSEdevice.dll` |
| `SteelSeriesGG.exe` | Electron host | no HID import; drives the engine over its local HTTP API |

Prism therefore computes screen-sync colours and hands them over: the engine carries the receiving side
(`driver.(*RealPrismSyncer)`, `frontend.handleUpdatePrismSync*`, `models/prism_sync`), and the engine's own
symbol table has no per-frame renderer (no animation or re-render loop, only `SetBaseEffect` /
`SetTriggeredEffect` style configuration writes). A living colour effect on the board is the engine writing
direct-mode frames; Prism is the colour source, not the writer.

## Live trace, 2026-11-10 (engine `70d240b7`)

Read-only Frida attach to `SteelSeriesEngine.exe` (PID 14204), hooks on `DeviceIoControl` (only the
two HID codes), `WriteFile` and `ReadFile`, across two action rounds while the owner changed settings
in the GG window. Payloads are read with `readByteArray`; `new Uint8Array(ptr, off, len)` silently
returns length 0 in Frida 17.24.3, which costs a whole round if used.

The engine is silent on HID while nothing changes: idle windows of 6, 10, 12, 14, 25 and 40 s
produced no HID call at all, so every frame below comes from an owner action.

### Frames captured

All keyboard traffic is on one handle (a 65-byte output report device) and every write is exactly 65
bytes, zero-padded after the command bytes:

| Frame | Count | Meaning |
|-------|-------|---------|
| `00 38 66 01 ...` | 1 | Rapid Tap on |
| `00 38 66 00 ...` | 1 | Rapid Tap off |
| `00 02 01 80 ...` | 8 | sequence head |
| `00 b1 00 ...` | 8 | profile chunk write, zero payload |
| `00 b2 00 ...` | 8 | `load_profile` |
| `00 34 00 ...` | 8 | `set_profile_volatile` |
| `00 90 00 00 ...` | 8 | firmware-version poll |

The last five repeat as a group about every 2.5 s for the whole action window, and each `90 00` is
followed by a read of GG's own 81-byte device JSON (`{"firmware_version": "1.19.7", "device_name":
"apex_pro_tkl_2022"}`). That group is a refresh loop, not a user frame: it fires whether or not
anything changed, and the profile bytes in `b1` are all zero.

### What was not seen

No `IOCTL_HID_SET_FEATURE` and no `IOCTL_HID_GET_FEATURE` at all, therefore no 644-byte or 645-byte
feature report in either round: no `00 38 61` (actuation), no `00 40` (per-key), no `00 24`
(brightness), no `00 1f 81`/`00 1f 82` (OLED), no `00 35`, no `00 f2`, no profile `00 03` chunk with
data, and no `00 41` clear. The only user-driven frames in two full rounds of nine UI actions were the
two Rapid Tap toggles.

Two readings fit, and this trace does not separate them: either the UI edits were no-ops relative to
the board's current state (GG sends a value only when it changes), or this build applies lighting and
actuation through the on-board profile rather than direct feature frames. The 2026-10-25 capture, from
engine build `c1b8e182`, did contain a 645-byte `00 38 61` sent as a feature report, so the second
reading would mean behaviour changed between builds.

One correction to the instrumentation plan: `DeviceIoControl` and `WriteFile` sit below the transport
layer. `SSEdevice.dll` exports `DeviceLib_SendCommand(unsigned int deviceID, unsigned int cmdType,
unsigned char *data, unsigned int len)` at `0x180004f10`, and `cmdType` is the chunk type from the
transport table above, so hooking that one function shows every logical command with its transport
(HIDIO, HIDFEATURE, ...) before any Win32 call. That is the hook to use for the next round.

## Open questions

1. ~~Checksum at blob offset 12320.~~ Solved 2026-10-15 (see "Checksum").
2. ~~Full key-to-offset map.~~ Decoded 2026-10-14 (see "Slot map"); 20 slots captured, 9 set-matched,
   the rest by row order. The tables at 1080, 1224 and 1590 and the second-actuation thresholds were named
   from the spec's packed structs on 2026-11-10 (see "Blob layout from the spec").
3. ~~Whether replaying the 0x40 frames works from this project.~~ Tested once on 2026-10-14 with GG
   stopped: `ssgg rgb color FF0000` put 644 wire bytes on interface 1 (checked with USBPcap) and the
   keyboard turned solid red (user-confirmed). Per-key tested 2026-10-21: `ssgg rgb perkey set-key W red`
   lit W red and turned every other key off (by design: a per-key frame replaces all 84 keys and unlisted
   ones send black; `ssgg` now merges onto the last frame it sent, see `apex_pro_tkl_2023.rs`). Other
   colours and Linux hidraw were still not tested. The 0x4B init is skipped for this model and was not
   needed in either test.
4. The init sequence at GG startup (needs a capture that begins before GG launches).
5. ~~What `38 66` toggles globally and what `38 67` configures.~~ Per GG's decrypted device spec
   (`apex_pro_tkl_2022`, 2026-10-24): `38 66 <0|1>` is `rapid_tap_enable`, `38 67` is `rapid_tap_pairs`
   (`count {index, hid_1, hid_2, mode | 0x80 report-both, 0, 0}`). Spec-derived, not confirmed on hardware;
   `ssgg actuation rapid-tap` / `rapid-tap-pairs` send them.
6. ~~Whether an actuation/profile read-back command exists.~~ **Yes (2026-11-10):** `ssgg onboard gen3-read` read
   all five slots back on the owner's board (fw 1.19.7), every chunk answered `00 83 00` + 500 bytes, CRC matching;
   GG itself issues the same 125 `83 01 8x` reads at startup (capture below). History: **Reopened 2026-10-24:** GG's decrypted device spec
   (`base_apex_2022_wired` `read_onboard_profile_N`, `apex_pro_tkl_2022` `hall_thresholds`) defines both a chunked
   profile read (`00 83 01 <fs-id> <size> <offset>`) and live actuation writes (`00 38 61 ...`). Spec-derived,
   untested on hardware; the trace below only shows GG did not use them during that minute. Original note:
   ~~No.~~ Live HID trace, 2026-10-21 (see "Live HID
   trace and engine memory read"): zero `GET_FEATURE` transfers over ~1 minute including the owner viewing
   GG's actuation page; GG's UI reads only its own cached config. Explains `ssgg actuation` having no effect
   on `0x1628` (confirmed by the owner, 2026-10-21): there is no wire command to send.
7. ~~Whether `ACTUATION_POINTS`' `(10, 21, 18)` (1.0mm) is correct.~~ Yes (2026-10-25): GG's table and the
   board's own sync report both give `(21, 18)` for 1.0 mm. The 2026-10-21 switch to `(47, 42)` was wrong:
   that pair is 1.8 mm, the default, so `ssgg actuation set 1.0` resent the board's current value.

## Hardware results, 2026-10-25

Wired `0x1628`, firmware 1.19.7, Windows, owner's board.

- **GG's wire frames** (Frida, read-only, hooks on `DeviceIoControl` `IOCTL_HID_SET_FEATURE` and `WriteFile`
  in `SteelSeriesEngine.exe`): setting all keys to 0.1 mm in GG sent `00 38 61 44 00 04 05 05 05 05 05 06 ...`
  (feature, 645 bytes), 4.0 mm sent the same with `e0 db`; both byte-identical to `ssgg actuation set --dry-run`.
  Rapid Trigger on/off sent `00 38 62 44 04 02 ...` / `... 04 00 ...`, Rapid Tap `38 66 01` / `38 66 00`
  (output). After the 0.1 mm frame GG also sent `02 01 80` and profile chunks `00 03 01 80 ...` (an on-board
  profile save; the hook kept only the first chunk).
- **Board sync reports** (USBPcap, EP `0x85`, 64 bytes): changing actuation on the OLED menu sends
  `0f <h> <l> <32-byte HID bitmask>` per step, e.g. `0f 05 05` 0.1 mm, `0f 15 12` 1.0, `0f 39 34` 2.0,
  `0f e0 db` 4.0, matching GG's table. Release-mode changes send `0d <mode> <bitmask>`.
- **ssgg's frame takes effect**: with GG stopped and the board at 4.0 mm (set on the OLED),
  `ssgg actuation set 0.1` (on the wire: `SET_REPORT` wValue `0x0300`, wIndex 1, 644 bytes) made every key
  hair-trigger (owner, by feel). The board sends no answer to `38 61`, and the OLED does not refresh after a
  host-side change, so its displayed value cannot be used to check one.

## GG actions capture, 2026-11-10 (engine `70d240b7`, firmware 1.19.7)

USBPcap of GG (`scripts/capture-device.ps1`, 20 guided steps, keyboard `0x1628`), filtered to `SET_REPORT`
control transfers (feature `wValue 0x0300` and output `0x0200`); the `0x40` lighting stream (about 19 fps,
count byte `54`) is omitted from every list. Pcap and per-step CSV live outside the repo. The macro steps were
skipped: GG offered no macro editor for this board in the UI the owner used. "Profile rewrite" below means the
sequence under "On-board profile write chain".

### Startup (GG launched with the board attached)

`90 00`, `F5`, `90 00`, `F2`, `F5` (output reports, each answered on the input endpoint), then 125 x
`83 01 <80..84> F4 01 <offset u32 LE>` feature reads (25 per slot, slot order 0-4, 13 ms apart: GG reads every
slot once), then `1F 82`, and three times `B2 00` + `34 00`, then `90 00` x3.

### Per action

| GG action | Frames (report id omitted) |
|-----------|-----------------------------|
| Rebind Caps Lock to Esc; undo | profile rewrite only (slot 0). No `36`, no `B6` |
| Actuation of W to 2.0 mm; back | profile rewrite, then feature `38 61 44 00 <(hid h l) x 68>` (layer 00, 68 keys), then a second profile rewrite; the undo sent `38 61` then the rewrite |
| Rapid Trigger on W on; off | feature `38 62 44 <(hid mode) x 68>` (W = `1A 02`, then `1A 00`), then profile rewrite |
| Rapid Tap pair A + D added; removed | output `38 66 01` then profile rewrite; `38 66 00` then profile rewrite. No `38 67`: the pair lives in the blob (offset 2650) |
| Lock-key colour to blue; back to white | no command at all: Caps Lock (`39`) changed from the frame colour to `00 5E FF` inside the `0x40` stream while lit, then to `FF FF FF`. The colour is host-rendered; the blob's `lock_keys_color` (offset 29) is written with the next profile rewrite |
| OLED image set; set back | output `1F 82`, feature `38 83 00 <640 bytes>`, then profile rewrite (the image also sits at blob offset 2010). The second image was all `FF`, sent twice |
| Active on-board profile to slot 1; back to slot 0 | `1F 82`, `B2 01`, `34 00`, `90 00`; then `1F 82`, `B2 00`, `34 00`, `90 00` |
| Rename slot 3 ("Config 4" to "Config 4x"); back | `1F 82`, `B2 03`, `34 00`, `90 00` (selecting the profile activates it), then profile rewrite of slot 3 (`02 01 83`, chunks `03 01 83`, `B1 03`, `B2 03`, `34 00`, `90 00`, `B2 03`, `34 00`, `90 00`), twice (an intermediate name was written first) |

### On-board profile write chain (as GG sends it)

1. output `02 01 <0x80 + slot>` (erase / open the slot's file), answered on the input endpoint
2. 25 x feature `03 01 <0x80 + slot> F4 01 <offset u32 LE> <500 data bytes>`, offsets 0, 500, .. 12000; bytes after
   the CRC (12324..12499) are `FF`
3. output `B1 <slot>` (commit), answered
4. output `B2 <slot>` (load / deploy), answered; output `34 00` (`set_profile_volatile` 0); output `90 00`

Chunk 0 of slot 0 starts `03 01 80 F4 01 00 00 00 00 | 03 00 00 00 "main" ...`: the blob's `schema` u32 (3) and
24-byte name are the first data bytes. Status of every frame: `.claude/rules/experimental-protocol.md`.

### Blob layout from the spec

Field offsets computed from the packed structs of `base_apex_2022_wired` / `apex_pro_tkl_2022` (device schema 8)
land on every offset measured above, and the slot-0 read-back of 2026-11-10 decodes accordingly (name `main`,
`device_profile_schema` 8, hall thresholds 21/18 as in GG's DB, meta key `F0`, lock keys `53 39 47 E3`):

| Offset | Field | Notes |
|--------|-------|-------|
| 0 | `schema` u32 | 3 |
| 4 | `name[24]` | NUL padded |
| 28 | `lighting_profile` (48 B) | `lock_keys_brightness` u8 @28, `lock_keys_color` rgb @29, `lock_keys_phys_hids[4]` @32, `meta_keys_brightness` @36, `meta_keys_color` rgb @37, `meta_keys_phys_hids_msk[32]` @40, `meta_background_dimmer` @72, reserved[3] @73 |
| 76 | `meta_key_phys_hid` u32 | `F0` |
| 80 | `button_mappings` 100 x {`function` u8, `key_codes[4]`} | indexed by `device-key-idx-to-hid` (first 70 are the analog keys); base layer, `51 <hid>` = identity |
| 580 | `button_meta_mappings` 100 x 5 | meta (SteelSeries-key) layer |
| 1080 | `device_profile_schema` u32 | 8 |
| 1084 / 1154 | `sensitivity_h[70]` / `sensitivity_l[70]` | actuation / reset |
| 1224 | `zone_levels` u32 x3 | default 4 4 4 |
| 1236 | `zone_key_subscriptions[70]` | |
| 1306 / 1376 | `multi_actuation_v2.sensitivity_h/l[70]` | second actuation thresholds |
| 1446 | `release_mode[70]` | 0 off, 2 rapid trigger, 3 key guard, 4 unknown |
| 1516 | `adaptive_dist_10[70]` | rapid-trigger sensitivity, 0.1 mm |
| 1586 | `protection_dur_ms` u32 | 500 |
| 1590 | `protection_dist_10[70]` | 0x14 = 2.0 mm |
| 1660 | `ma_mappings` 70 x 5 | second-actuation layer bindings |
| 2010 | `oled[640]` | on-board OLED image, row-major |
| 2650 / 2700 | `rapid_tap` 10 x {hid1, hid2, mode + report_both bit 7, 0, 0} / `rapid_tap_enable` | |
| 2701 | padding[3] | |
| 2704 | `macro_events[9600]` | 4-byte events, format not decoded |
| 12304 | `guid[16]` | GG's config guid |
| 12320 | crc u32 LE | see "Checksum" |
| 12324 | `FF` x176 | pad to 25 x 500 |

Mapping `function` byte: `00` unbound, `01..08` mouse buttons, `31`/`32` wheel, `33`/`34` AC pan, `51` keyboard,
`61` consumer, `62` meta, `30` CPI, `71` macro, `72` external; `key_codes[4]` holds the argument.
