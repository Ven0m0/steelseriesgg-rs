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
| 1080 | `08 00 00 00` | meaning unknown |
| 1084 + slot | Actuation point (`hall_thresholds.h`) | 0.1 mm = 0x05, 1.0 = 0x2F, 2.0 = 0x39, 3.0 = 0x86 |
| 1154 + slot | Reset point (`hall_thresholds.l`) | 0.1 mm = 0x05, 1.0 = 0x2A, 2.0 = 0x34, 3.0 = 0x7b |
| 1224 | `04 00 00 00` x3, then zeros | meaning unknown |
| 1446 + slot | Release mode (`release_mode`) | 0 = off, 2 = Rapid Trigger, 3 = Key Guard |
| 1516 + slot | Rapid Trigger sensitivity (`rapid_trigger_sensitivity`) | default 2 |
| 1590 + slot | unknown, all 0x14 | not `second_actuation_thresholds` (all 0 in GG's config) |
| 2650 | Rapid Tap pairs (`rapid_tap_pairs`), stored as HID usages | default `04 07` (A, D) |
| 2700 | Rapid Tap enable (`rapid_tap_enable`) | 0 = off, 1 = on |

Key W is slot 16 (actuation byte at 1100, reset byte at 1170, mode byte at 1462).

- The actuation encoding is non-linear (a raw sensor scale); only these four points are known.
- Earlier versions of this note called offset 1170 a "second threshold". It is the reset point
  (`hall_thresholds.l`) of the same setting. An earlier capture called `h` = 21 (0x15), `l` = 18 (0x12)
  the default; a live read of the owner's board at GG's own labelled 1.0mm (2026-10-21) instead showed
  `h` = 47 (0x2F), `l` = 42 (0x2A), which is now the value in `ACTUATION_POINTS`. The 21/18 claim was
  never checked against a labelled UI value and is likely wrong, but it has not been re-captured to
  confirm what (if anything) it corresponds to.

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
`set_actuation`, `profile_chunk_reports`). Nothing sends a profile: no read-back command is known, so a
write would replace every other on-board setting. Recompute the CRC on a fresh capture with edits past
offset 2700 before the first write.

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

## Open questions

1. ~~Checksum at blob offset 12320.~~ Solved 2026-10-15 (see "Checksum").
2. ~~Full key-to-offset map.~~ Decoded 2026-10-14 (see "Slot map"); 20 slots captured, 9 set-matched,
   the rest by row order. Still unknown: the tables at 1080, 1224 and 1590, and where
   `second_actuation_thresholds` is stored.
3. ~~Whether replaying the 0x40 frames works from this project.~~ Tested once on 2026-10-14 with GG
   stopped: `ssgg rgb color FF0000` put 644 wire bytes on interface 1 (checked with USBPcap) and the
   keyboard turned solid red (user-confirmed). Per-key tested 2026-10-21: `ssgg rgb perkey set-key W red`
   lit W red and turned every other key off (by design: a per-key frame replaces all 84 keys and unlisted
   ones send black; `ssgg` now merges onto the last frame it sent, see `apex_pro_tkl_2023.rs`). Other
   colours and Linux hidraw were still not tested. The 0x4B init is skipped for this model and was not
   needed in either test.
4. The init sequence at GG startup (needs a capture that begins before GG launches).
5. What `38 66` toggles globally and what `38 67` configures.
6. ~~Whether an actuation/profile read-back command exists.~~ No. Live HID trace, 2026-10-21 (see "Live HID
   trace and engine memory read"): zero `GET_FEATURE` transfers over ~1 minute including the owner viewing
   GG's actuation page; GG's UI reads only its own cached config. Explains `ssgg actuation` having no effect
   on `0x1628` (confirmed by the owner, 2026-10-21): there is no wire command to send.
7. ~~Whether `ACTUATION_POINTS`' `(10, 21, 18)` (1.0mm) is correct.~~ No: live read, 2026-10-21, showed
   `(47, 42)` on a board the owner confirmed GG reported as 1.0mm. The table now uses `(10, 47, 42)`; the
   old `(21, 18)` was never checked against a labelled UI value in the first place.
