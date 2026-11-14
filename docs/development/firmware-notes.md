# GG keyboard firmware notes

Static analysis of the firmware images that ship inside SteelSeries GG. Nothing here was run on a
keyboard, no image was modified or flashed, and no firmware bytes are reproduced (offsets, addresses
and decoded field values only).

Source: `C:\Program Files\SteelSeries\GG\apps\engine\firmware\<id>\`, from GG vcs `70d240b7`
(installed 2026-11-02). First pass 2026-11-10 with `rizin -a arm|avr` (linear sweep only). Second pass
2026-11-14 with Ghidra 12.2 headless (decompiler), which **supersedes the first pass where they differ**:
the first pass used a wrong load base for `0x1628` and wrote "no read path" and "no `0x90`", both
contradicted by the hardware results of 2026-11-10/11 and by the decompiled code below. Every claim is
static evidence only, so it is `[EXPERIMENTAL]` in the terms of `.claude/rules/experimental-protocol.md`.

## What ships

313 `.bin` files. The keyboard families relevant to the direct-mode protocols:

| Device | ID | Image | Size | Architecture |
|--------|----|-------|------|--------------|
| Apex Pro TKL 2023 (`0x1628`) | 272111144 | `firmware-apex-pro-tkl-1.19.7.bin` | `0x17000` | raw ARM, Thumb |
| Apex Pro TKL 2024 (`0x1642`) | 272111170 | `firmware-apex-pro-tkl-2024-1.19.7.bin` | `0x17000` | raw ARM, Thumb |
| Apex Pro TKL 2024 Cyber / Steel (`0x1660`, `0x1662`) | 272111200 / 272111202 | `...-1.20.00.bin` | `0x17000` | raw ARM, Thumb |
| Apex Pro Mini (`0x161E`) | 272111134 | `firmware-apex-pro-mini-1.19.7.bin` | `0x15000` | raw ARM |
| Apex Pro Mini 2024 (`0x1648`) | 272111176 | `firmware-apex-pro-mini-2024-1.19.6.bin` | `0x15000` | raw ARM |
| Apex 9 Mini (`0x1620`) | 272111136 | `firmware-apex-9-mini-1.19.9.bin` | `0x15000` | raw ARM |
| Apex 9 TKL (`0x1634`) | 272111156 | `firmware-apex-9-tkl-1.19.9.bin` | `0x15000` | raw ARM |
| Apex 3 (`0x161A`) | 272111130 | `firmware-apex-3-v0x28.bin` | `0xB800` | raw ARM |
| Apex 3 TKL (`0x1622`) | 272111138 | `firmware-apex-3-tkl-3.0.4.bin` | `0xB400` | raw ARM |
| Apex Pro / 7 / Pro TKL / 7 TKL / Pro 2024 / 7 2024 | 272111120/122/124/128/168/186 | `firmware-apex-*-v4.16.8.bin` | `0x100000` | container, see below |
| Apex 5 / Apex 5 2024 (`0x161C`, `0x1650`) | 272111132 / 272111184 | `firmware-apex-5-*.bin` | `0x100000` | container |
| Apex M750 / M750 TKL (`0x0616`, `0x0617`) | 272107030 / 272107031 | `firmware-apex-m750-1.20.bin` | 44325 | raw ARM |
| Apex M800 (`0x1600`) | 272111104 / 272111107 | `firmware-apex-m800.bin` | `0xA000` | raw ARM |
| Apex M400 / M500 (`0x160C`, `0x1607`) | 272111116 / 272111111 | `firmware-apex-m*-0x015.bin` | `0x5000` | raw ARM |
| Legacy Apex, Apex 350, Apex 300, Apex Raw | 272110080/082/086/088 | `firmware-apex-*.bin` | 9728 to 27648 | raw ARM |

Wireless sets carry three images per device: `-dongle-` (184320), `-nordic-` (307200) and `-stm-`
(51200). The Nordic image is the radio stack; the STM image is the keyboard MCU.

`rz-bin -I` reports `arch N/A, bits 64, LE` for every raw ARM image, because there is no header it
recognises. The first pass also labelled `keyboard-app.bin` and `lighting-app.bin` in the `0x100000` container
directories "genuinely AVR" (`rz-bin` printed `avr/ATmega8`); **that was wrong**. Both start with a Cortex-M vector
table (SP `0x2000c000`, Thumb handlers at `0x0801xxxx`) and decode as ARM; see "Gen 1" below.

## Loading a raw ARM image

Ghidra headless, one instance at a time (a second concurrent instance fails on the shared cache directory):

```
analyzeHeadless <projdir> <name> -import <copy of image> -overwrite -loader BinaryLoader \
    -loader-baseAddr 0x<base> -processor ARM:LE:32:Cortex -preScript Seed.java
analyzeHeadless <projdir> <name> -process <image> -noanalysis -readOnly -postScript <script> <args>
```

`BinaryLoader` finds no entry points, so the pre-script (outside the repo, in the analysis workspace) sets
Thumb mode and creates functions at the vector-table entries, at the handler pointers of every 12-byte
`{tag, ptr, ptr}` table (null pointers allowed) and at every literal-pool word that points at a `push {..., lr}`.
Callbacks that code stores in RAM structures are only reachable that way. Work on copies of the images, never
on GG's own files.

### Load base

**`0x1628` firmware 1.19.7: base `0x08009000`** (the first pass guessed `0x08008000`, which was off by `0x1000`).
The image starts with a Cortex-M vector table (file offset 0: SP `0x2000a000`, reset `0x0801265d`, 46 non-zero
handlers). Evidence for `0x08009000`, none of which holds at `0x08008000`:

- The default handler that fills the unused vector slots (`0x080126a4`) decodes as `b .` (an infinite loop);
  at `0x08008000` it lands mid-instruction.
- The reset handler opens with a data-copy loop and sets SP before any call; the NMI/fault stubs
  (`0x0800e680`) decode as `bx lr` and `b .`.
- Ghidra: 46 of 46 vector entries become function entries (41 of 46 at `0x08008000`); 67% of the image decodes as
  instructions (47% at `0x08008000`).
- String pointers land on string starts (the `90` handler's two version-string pointers).
- The class-router table literals (`0x0801b648` and others) are already in this frame; the first pass had
  listed the same tables `0x1000` lower.

Unless a section names another base (Apex 3 TKL `0x3c00`, Gen 1 apps `0x0800a000` / `0x08008000`), every address in
this file is in the `0x08009000` frame. The first pass printed file offset plus `0x08008000`
for code and table scans, so an address from it is the true address minus `0x1000`.

**Apex 3 TKL 3.0.4 (`0x1622`): base `0x3c00`.** Vector table at file offset 0 (SP `0x20001df0`, reset
`0x3fed`), default handler `0x4090` decodes as `b .` only at this base, reset code is `ldr r0; mov sp, r0;
ldr r0; blx r0`. The image occupies `0x3c00..0xf000`, so a 15 KB bootloader sits below it.

## Apex Pro TKL 2023, firmware 1.19.7

### Identity

USB strings in the image name the model and its recovery interface, and the OLED menu strings cover
the on-board features:

- USB: `SteelSeries Apex Pro TKL 2023`, `SteelSeries HID Config`, `SteelSeries HID Interface`,
  `SteelSeries Apex Pro TKL 2023 Recovery`.
- OLED menu: `ACTUATION`, `RAPID TAP`, `SENSITIVITY`, `RELEASE`, `ILLUMINATION`, `BRIGHTNESS`,
  `REACTIVE`, `BREATHE COLOR`, `BREATHE MULTI`, `FULL COLOR`, `LIGHTS OFF`, `SCREEN SAVER`,
  `FUNCTION`, `DISPLAY ICONS`, `CONFIG KEY ACTION`, `SAVE CONFIG`, `LOAD CONFIG`,
  `DE/ACTIVATE CONFIGS`, `MACRO KEY`, `RECORDING`, `REMOVE MACRO`, `REPRESENT`, `FACTORY RESET`,
  `RECALIBRATE`, `PRESS ALL KEYS`, `CALIBRATION DONE`, `Updating`.

The on-board menu therefore implements the same feature set the direct-mode frames address, which is
the firmware side of the `[EXPERIMENTAL]` command table in
`.claude/rules/experimental-protocol.md`.

### USB HID report descriptor

Decoded from the raw bytes at file offset `0x15df0` (VA `0x0801edf0`):

| Interface | Page / usage | Reports |
|-----------|--------------|---------|
| vendor (`0xFFC0`, usage 1) | report ID 0 | 64-byte input, 64-byte output, **644-byte feature** |
| keyboard boot (`0x01/0x06`) | report ID 0 | 1-byte modifier, 6-key rollover, 3 LED output bits |
| consumer control (`0x0C/0x01`) | report ID 1 | 2 x 16-bit |
| mouse (`0x01/0x02`) | report ID 2 | 8 buttons, 2 x 8-bit wheel/pan, 3-bit buttons |
| vendor (`0xFFC1`, usage 1) | report ID 0 | 64-byte input |

The vendor feature report is `Report Count (644)` = `96 84 02`, `Feature (Data,Var,Abs)` = `B1 02`.
Windows adds the report-ID byte, so the caps are 645 feature, 65 input, 65 output. That matches the
`apex_pro_tkl_2022` spec (`feature-report-size 645`) and the frames captured on the wire in 2026-10.

### Request path and reply path (Q3)

The firmware **does** answer reads, through the same 644-byte buffer that carries feature writes:

1. `FUN_0800ea68` is the USB class-request handler. `SET_REPORT` (`bRequest` 9) on the vendor interface with
   `wLength` up to 644 records whether the report type was Feature (`wValue >> 8 == 3`) and starts the data stage;
   the data-out callback (`0x0800e1d8`) stores length and that flag (`0x080116f8`).
2. The main loop (`0x0800e788`) calls the dispatcher `0x0801170c` every tick in the configured state. It zeroes
   the reply buffer and writes the reply there: a 64-byte buffer sent on endpoint `0x82` (interrupt IN, the
   "input report" answers) when the request was an output report, or the 644-byte buffer at `usb + 0x290 + 0x284`
   when it was a feature report.
3. `GET_REPORT` (`bRequest` 1, device to host) is served from that same buffer with `wLength` capped at 644
   (longer requests stall). Any report type is served from it; a read before any command returns the previous reply.

Reply layout in the buffer: byte 0 echoes the request byte 0, byte 1 is the status, handler output starts at byte 2.
Status `0xff` is written first, so a tag with no handler answers `<tag> ff`. With Windows prepending report ID `0`
this gives the `00 83 00` header the `83 01` read-back showed on hardware 2026-11-10: **Q3 agrees with hardware.**
A feature-report request is therefore answered by "write the query as a feature report, then `GET_REPORT`" (what
`B6` and the other 645-byte reads do), while an output-report request is answered on the interrupt input endpoint
(what `ssgg` uses for `F2`/`F5`, per `src/devices/keyboards/apex_pro_tkl_2023.rs`); which of the two `83 01` and `B7`
take is set by the code, not by this note.

### Dispatcher and tables (Q2)

Dispatch is two levels. `0x0801170c` reads request byte 0, takes `tag = byte0 & 0x7f` and looks it up in a table of 36
12-byte entries `{tag, read_handler, write_handler}` at `0x0801c158`. **Bit 7 of the request byte selects the
read handler** (column at offset +4); a clear bit 7, or a null read handler, uses the write handler (offset +8).
This is why `B6`, `B7`, `F2`, `F5`, `B1`, `B2` and `83` appear in no `cmp` and no table: they are `36`, `37`, `72`,
`75`, `31`, `32` and `03` with bit 7 set. The first pass looked for the literal bytes and concluded they were
absent.

Four top-level tags are class routers (`FUN_0800f158` for the read column, `FUN_0800f1f0` for the write column).
They take the next payload byte as a sub-tag, look it up in a second 12-byte table, and call the same column:

| Class | Table | Entries | Sub-tags |
|-------|-------|---------|----------|
| `0x38` | `0x0801b648` | 9 | `60 83 61 62 63 65 64 66 67` |
| `0x1f` | `0x0801b30c` | 2 | `81 82` |
| `0x73` | `0x0801b324` | 6 | `50 51 52 53 80 11` |
| `0x24` | `0x0801b36c` | 2 | `02 86` |

Top-level table (`R` = bit 7 set, `W` = bit 7 clear, handler entry addresses, `-` = none; meaning column is from
the repo's command notes or the decompiled body, otherwise "not decoded"):

| Tag | R | W | Meaning |
|-----|---|---|---------|
| `01` | - | `0x080116be` | not decoded (`01 <0\|1>`, one flag) |
| `02` | - | `0x0801169a` | `02 01 <fs>`: erase file system slot (ProfileErase) |
| `03` | `0x08011530` | `0x0801166e` | `03 01 <fs> <size u16> <offset u32> <data>` write chunk; `83 01` read, same header |
| `04` | `0x080115f4` | - | `84 01 <fs>`: slot info, 8-byte reply (two u32); not decoded further |
| `10` | `0x080116d4` | - | `90 <0\|1>`: version string (`90 00` = `1.19.7`, `90 01` = a build id string) |
| `1e`, `20`, `23`, `26`, `27` | `0x08018412`, -, `0x08012360`, `0x08012390`, `0x080123e8` | `0x080183e6`, `0x08012350`, `0x08012374`, `0x080123b0`, `0x08012404` | not decoded |
| `1f` | class router | class router | OLED: `1f 81` image write (R `0x080182b6`, W `0x08018260`), `1f 82` (W `0x080182f0`) |
| `24`, `38`, `73` | class router | class router | see class table |
| `30`, `31` | `-`, `0x080106d0` | `0x080107fe`, - | `B1` (31 R) is the profile commit per the repo notes, body not decoded; `30` not decoded |
| `32` | `0x08010534` | `0x080107cc` | `B2 <slot>` loads a slot into the RAM profile; `32 <slot>` **saves the RAM profile to a slot** (calls the profile saver `0x080103ec`) |
| `33`, `39` to `3e` | various | various | not decoded |
| `34` | `0x08010704` | `0x080109e0` | `34 <0\|1>` sets the volatile flag (Q5); `B4` reads it |
| `35` | `0x080105e4` | `0x08010878` | MetaToggle per the repo notes; not decoded here |
| `36` | `0x080105f8` | `0x080108ac` | key mapping: `B6` read, `36` write (Q4) |
| `37` | `0x08010684` | `0x08010988` | macro events: `B7` read, `37` write (Q6) |
| `40` | `0x0800f0a4` | `0x0800f060` | direct per-key frame (Q8) |
| `41` | - | `0x0801306c` | direct release (`00 41`), per the repo notes |
| `71`, `77` | `0x08011f64`, `0x08012058` | `0x08011f14`, `0x08012020` | not decoded; the writes reach on-chip flash |
| `72` | `0x08011fd0` | `0x08011f98` | `F2` layout query; the `72` write reaches flash |
| `74` | `0x0801006c` | `0x0801002c` | not decoded |
| `75` | `0x0801200c` | `0x08011fe4` | `F5` region query; `75` RegionWrite reaches flash, as the repo notes say (`writes flash`) |
| `76` | `0x0800f120` | `0x0800f0dc` | not decoded |

Class `0x38` sub-tags: `61` has both a write (`0x0801772c`) and a read (`0x080177ec`), `62` to `67` and `60` have
read and write pairs (`0x08017958`/`0x080178a8`, `0x08017a24`/`0x080179f0`, `0x08017ac0`/`0x08017a90`,
`0x08017a78`/`0x08017a60`, `0x08012558`/`0x080125e8`, `0x08012570`/`0x080125fe`, `0x080145e0`/`0x08014678`), and
`83` is the on-board OLED image (write `0x08018310`, read `0x08018374`, 640 bytes at RAM-profile offset `0x7da`).

**Read-back of live analog thresholds exists in the firmware.** `B8 61 <count> <layer> (key, 0, 0) x count` fills
each entry's two bytes from the RAM profile (`0x080177ec`); layers 0 and 1 share one table, layer 2 has its own. The
first pass's answer "(a) no read path" was wrong. Whether it works on hardware is untested; only the `83 01`, `B6`,
`B7`, `F2`, `F5` reads are Confirmed.

### Answers to the open questions

**Q1 (load base).** `0x08009000`, see "Load base".

**Q2 (tag to handler map).** The two tables above. Class routers have no direct callers (they are reached only through
the table); the dispatcher is called once, from the main loop.

**Q3 (reply path).** See "Request path and reply path". It agrees with the hardware reply header `00 83 00`.

**Q4 (does the `36` key-mapping write persist?).** Static answer: not by itself, probably yes after a later trigger.
- The `36` handler (`0x080108ac`) writes into the RAM profile structure (`FUN_080102d8()`; layer 0 at `+0x50`, meta
  layer at `+0x244`, second layer at `+0x67c`, 5 bytes per key) and then calls `FUN_0800ffa0(0)`, which only resets
  lighting and display refresh state. It makes no call to the file-system write (`0x080118e8`) or to flash. A count
  byte of `0xff` instead restores the default key-map tables from ROM.
- The profile saver `0x080103ec` serializes the whole RAM profile (`0x3024` = 12,324 bytes, the used part of the 12,500-byte slot blob; the rest is `FF` padding)
  to file-system slot `0x80 | slot`, erase first. It is reached from `32 <slot>` and from the auto-save
  `0x080104cc`, which runs after on-board menu, macro and OLED edits and **does nothing while the volatile flag is
  set** (Q5).
- Consequence: a live `36` binding sits in RAM until any auto-save trigger fires (a menu change, a macro edit, a
  profile switch that saves) or `32 <slot>` is sent; then it is stored with the whole profile. Sending `34 01` first
  would keep it volatile. Untested on hardware; the repo's row still says power-cycle persistence is unknown.
- The only code that reaches the on-chip flash driver (`0x0800992c` unlock) is the settings page writer
  `0x08011e84` (a 336-byte block with CRC at flash offset `0x8800`, 2 KB page erase) and its callers: the `71`, `72`,
  `75` and `77` write handlers and a per-key table writer (`0x08013524`), none of them the key map. The
  file-system layer under `0x080118e8` was not traced to its medium.

**Q5 (`34` flag).** `34 <0|1>` stores `byte == 1` in a RAM flag (`0x20004dd3`, handler `0x080109e0`); `B4` returns
it. The flag is cleared by loading a profile (`0x08010314`, so `B2`) and by saving to a slot (`0x080103ec`, which
then stores the slot number in a RAM variable that is probably the active-slot index, not confirmed), and tested by the auto-save `0x080104cc`. So it is a **volatile-profile
flag: while 1, on-board edits are not auto-saved to the active slot**. GG's `34 00` after `B2` re-enables auto-save
(already the state after a load).

**Q6 (`B7` macro event layout).** The macro region is 2400 events of 4 bytes (limit `0x960`, 9600 bytes) at RAM-profile
offset `0xa90` (2704, the blob offset in `apex-pro-tkl-2023-gg-capture.md`). `B7 <offset u16> <count u16>` returns
the offset and count echoed back (4 bytes, after the echo and status bytes: the 7-byte header with the report ID)
and `count` events; `37` writes them (it first calls `FUN_08010ec8`, apparently a recorder reset).
Decoded from the recorder (`0x08010b84`, `0x08010ee8`, `0x08011208`) and agreeing with the `[EXPERIMENTAL]` grammar
in `apex-7-gg-capture.md`:
- A **delay event** has type byte `0x04`, byte 1 zero and a u16 counter at bytes 2-3; consecutive delays merge by
  incrementing that counter.
- A **record** starts with marker `0x77` (`'w'`); the scan for the end of used space steps `2 + u16 at +2` events
  per record. The Apex 7 notes name that u16 `id`; the firmware treats it as the body length in events, so the
  Apex 7 doc's `id` label is probably the event count (see the engine paragraph below for the hardware evidence). The macro block on the test board is all zero, so there is no hardware
  cross-check.
- **Engine side (2026-11-14, `SteelSeriesEngine.exe`, Ghidra, Go symbols and source paths intact).** The record and
  event layout is decoded in `convert_libsteel.go` (`libsteelMacroMetadataModel` line 246,
  `libsteelTranslateOnboardToMacro` line 331, `libsteelGetMacroDataFromOnboard` line 382). A record is an 8-byte
  header, `77 <playback type> <event count u16 LE> ...`, followed by `count` 4-byte events, so its length is
  `8 + 4 * count`: **the u16 at +2 is the event count, which is exactly what the firmware scan steps over (`2 + count`
  events including the two header events)**. Playback type bytes `1..5` map to internal modes `0..4`; anything else is
  rejected ("unknown onboard playback type"). The header's remaining bytes (+4 u16, +6, +7) are read and passed on; their
  meaning is not decoded. Event types (byte 0):

  | Type | Layout | Meaning |
  |------|--------|---------|
  | `00` | `00 <mask> -- <state>` | mouse button; `mask` must be a single set bit (button index = log2), `state` byte 3 |
  | `01` | `01 -- -- <dir>` | mouse wheel; byte 3 must be `01` or `ff` |
  | `02` | `02 <code> <page> <state>` | key; `code` = HID usage, `page` = usage page (`01` keyboard, `0c` consumer), `state` 1 = down, 0 = up |
  | `04` | `04 00 <ms u16 LE>` | delay; consecutive delays merge (as in the firmware recorder) |
  | `0c` | `0c -- -- <dir>` | AC pan; byte 3 must be `01` or `ff` |

  Any other type is an error ("unknown macro event type"). The engine also recognises a plain key tap as
  `press, delay, release` of one code (3 events) and single-event forms for a lone key, button or wheel step. This agrees with
  the key and delay part of the `[EXPERIMENTAL]` grammar in `apex-7-gg-capture.md`: its `press` row is a key event
  (`02 <code> 01 01`) plus a delay event (`04 00 <ms>`), which explains the press "word" it calls unexplained. It is a
  superset of what `decode_macros` in `src/devices/keyboards/apex_7_profile.rs` handles: mouse button, wheel, pan,
  consumer-page keys and a press without a delay are not parsed there. Evidence for the count reading: the only real
  macro read in this repo (Apex 7, Gen 1) has record ids 3 and 27 and records of 20 and 116 bytes, which is exactly
  `8 + 4 * id`. On the Gen 3 board the macro block is empty, so no stored macro was read there.
  The firmware player was not located.

**Q7 (`6C` driver mode).** Not in the `0x1628` 1.19.7 dispatch tables at either level, so the firmware answers it with
the default `6c ff`; GG's opening `6c 00 01` is answered by nothing useful, consistent with the read capture. The Apex 3
TKL 3.0.4 table does have a `6c` write handler (`0x6c2c`): `6c <a> <b>` stores `a != 0` and `b != 0` as two RAM flags
and posts events `0xf9` and `0xfa`. So `6C` is a platform-dependent frame, not part of the Gen 3 protocol.

**Q8 (`0x40` handler).** Write handler `0x0800f060`: the first byte after the tag is the entry count; `0xff` means a fill
with a 24-bit colour taken from the three bytes at offset 2 from the count byte (`0x08012ae4`); otherwise `count` entries of 4 bytes are applied and the
length must be at least `count * 4 + 1` (`0x08012b10`), else status 2. That matches the repo's confirmed frame
(`00 40 <count> (usage R G B) x count`). The `count == 0xff` fill is not used by `ssgg` and untested. The bit-7 form
(`C0`) takes 5-byte entries (`0x0800f0a4`); not decoded.

### Corrections to the first pass

| First-pass claim | Now |
|------------------|-----|
| (a) no read path in 1.19.7 | Wrong. `GET_REPORT` returns the dispatcher's reply buffer; read handlers exist for `83`, `84`, `90`, `B1`, `B2`, `B4`, `B6`, `B7`, `B8 61`, `F2`, `F5` and more |
| (b) `83` routed, semantics unproven | `83` is `03` with bit 7 set: handler `0x08011530`, `83 01 <fs> <size> <offset>`, matches hardware |
| (c) no `0x90`, `0x65`, `0x66`, `0x67`, `0x41` | Tags `10` (so `90`), `41` and class-`38` sub-tags `65`, `66`, `67` exist. Only tags absent from every table are absent (`6c`, `21`, `22`, `09`) |
| (c) `0x21`, `0x22` not a class | Correct for the Gen 3 tables. The lighting path is `40`/`41`; `21`/`22` are Apex 3 commands |
| Load base `0x08008000` | `0x08009000` |

## Gen 3 siblings (static, 2026-11-14)

Imported the same way (`BinaryLoader`, base `0x08009000`, all 46 vector entries are function entries in every image;
the default-handler rule gives `0x08009000` for each). The dispatcher shape is the `0x1628` one everywhere: a
36-or-38 entry `{tag, read, write}` table plus the class tables `1f`, `24`, `38`, `73`. Only the tables and their
tags were compared, no handler bodies.

| Image | Top table | Tags vs `0x1628` | Class `38` sub-tags |
|-------|-----------|------------------|---------------------|
| Pro TKL 2024 `0x1642`, 1.19.7 | `0x0801c07c` | identical (36) | identical to `0x1628`, incl. `83` (OLED image) |
| Pro TKL 2024 Cyber `0x1660`, 1.20.00 | `0x0801c080` | identical (36) | identical (same image layout as `0x1642`) |
| Apex Pro Mini `0x161e`, 1.19.7 | `0x08017b84` | 38: no `1e`, adds `21 22 25` | `60 61 62 63 64 65 66 67`, no `83` |
| Apex 9 TKL `0x1634`, 1.19.9 | `0x0801768c` | 38: no `1e`, adds `21 22 25` | only `60 66 67`, so no `38 61` |

What this adds to the status rows (all `[EXPERIMENTAL]`, nothing run on a keyboard):
- `0x1642`, `0x1660`: the firmware defines the same read/write command set as `0x1628`, including `36`/`B6`/`B7`,
  `83 01`, `F2`/`F5` and `90`, which supports the library-only read support the rules file lists for them. It does not
  show that the 645-byte frame layouts are the same.
- Apex 9 TKL: the missing `38 61` agrees with the rules file ("no live per-key `38 61` table"). The Apex Pro Mini has
  `38 61` to `67`, so the "Apex 9 boards" wording must not be read as covering the Apex Pro Mini.
- `0x161e`, `0x1634` define `21`, `22` and `25`, which the `0x1628` tables lack; their bodies were not decoded.

## Apex 3 TKL, firmware 3.0.4 (issues #173, #257)

Base `0x3c00`. One 29-entry table at `0x9f00` with the same `{tag, read, write}` shape; the dispatcher is `0x71f4`
(bit 7 selects the read column the same way). Tags: `01 02 03 04 06 08 10 11 13 21 22 23 29 2a 2b 2c 6c 31` to
`3c`. There is no `09` and no `24`.
- `21 <zone mask> (R G B) x 8` (handler `0x6874`): for each set mask bit it sets the zone's enable flag and copies the
  three colour bytes into a per-zone record (stride 12, flag at `+4`, colour at `+5`). This is the frame `ssgg` sends
  (`00 21 FF <RGB x8>`). It needs no other command to arm it.
- `22 <zone mask>` (`0x68cc`) clears the enable flag of those zones.
- `23 <level>` (`0x6914`) stores the level only when it is at most `0x10` and ignores larger values without an error;
  `A3` reads it back. `ssgg` already scales to `0..=16` (`Apex8ZoneBrightnessCommand`).
- **There is no `09` handler in 3.0.4.** `APPLY` is not part of this firmware's command set: the firmware has no handler for it and
  ignores the byte, so omitting it cannot be the cause of #173/#257 (the removal itself stays untested on hardware). Whether the effect
  engine (`0x40d4` and the per-zone records) shows a `21` colour immediately was **not traced**, and nothing static
  here speaks to interface selection or timing, the other candidates for #173/#257.

## Gen 1: the 1 MB images are flash containers

`firmware-apex-pro-v4.16.8.bin` (1048576 bytes) starts with `a5 a5 01 00`, then a table of 32-bit
fields (total size, then per-record offset and size). Two of the records are byte-identical to the
companion files:

| Record offset | Size | Content |
|---------------|------|---------|
| `0x1000` | `0x1d000` | `keyboard-app.bin` (exact match, full length) |
| `0x1e000` | `0x1d000` | `lighting-app.bin` (exact match, full length) |

So the container is a flat flash layout, not a compressed or encrypted format. The two apps are **Cortex-M, not AVR**
(the first pass was misled by `rz-bin`'s default guess for headerless files; 118784 bytes is also far above an
ATmega8's flash). Load bases from the default-handler rule: `keyboard-app.bin` `0x0800a000` (container offset
`0x1000` plus a flash base of `0x08009000`), `lighting-app.bin` `0x08008000` (not consistent with that flash base,
so the lighting app evidently runs on a different part or bank; not resolved). Apex 5 and Apex 5 2024 use the same
layout with 98304-byte apps, which were not imported. Ghidra: all 58 vector entries are function entries in the
`0x1610` keyboard and lighting apps and in the `0x1640` keyboard app.

Gen 1 routing, seen identically on `0x1610` and `0x1640`: the keyboard app has a 64-entry `{tag, read, write}` table
(`0x08022314` on `0x1610`, `0x0802105c` on `0x1640`) in which `3A`, `3B`, `21`, `22` and `23` share one write
stub that, **only while a RAM flag is zero**, calls a forwarding routine and replies with length 0. The lighting app
has its own 22-entry table (`0x0801257c`, identical on both boards) with the real `3A` handler at `0x080090a4`:
`3A <count> (usage, R, G, B) x count`, each usage mapped through a usage-to-slot byte table (slot 6 = no such key,
skipped) into a per-slot record (enable flag, then the RGB bytes). That is the frame `ssgg` builds
(`DirectFrameCommand`, `0x3A`), now seen from the firmware side on both boards.

### `0x1610` against `0x1640` (issue #283)

`lighting-app.bin` of the Apex Pro (`0x1610`, v4.16.8) and the Apex Pro 2024 (`0x1640`, v4.16.8, whose `0x3A` frame is
Confirmed on hardware) differ in exactly **two content bytes and the four-byte trailer** (offsets `0xa39b` and
`0xa55d`, trailer at `0x1cffc`). One of the two bytes is `0xfb`, the logo-key usage the `0x1640` hardware test found
(`0xFB` lit, 0x31 and 0x87-0x8A dark). So the `0x3A` lighting code on `0x1610` is the same code as on `0x1640`;
the two differ only in a small key-table entry. `keyboard-app.bin` differs almost everywhere (a different build), but its routing for the lighting tags is the same
shape (see above). Conclusion: no frame-level reason found for `0x1610` showing nothing; transport (hidraw interface,
report size) is the remaining suspect, as `PLAN.md` P4 already assumes. Not decoded: what the RAM flag that gates
the forwarding stub means (it has many readers in both keyboard apps) and the forwarding routine itself, so a
state-dependent refusal on `0x1610` (for example a mode the board must be in) is **not ruled out**.

## Method and limits

- Ghidra 12.2, JDK 25, `ARM:LE:32:Cortex`, `BinaryLoader`. The Java scripts (seed, decompile-to-file, literal-pool
  reference finder, call-reachability, base statistics) and all decompiled output stay outside the repo in
  `%USERPROFILE%\gg-capture-artifacts\ghidra\`, as do the copies of the images.
- Cross-checks with rizin (`pd` with `-m <base>`) were single-address only.
- There are no `movw`/`movt` pairs in 1.19.7, so every constant comes from a literal pool. A literal pool value is
  only an address if the code loads it with `ldr rX, [pc, ...]` and then dereferences it.
- Ghidra could not follow indirect calls: reachability to the flash driver is by direct calls only, so a handler
  that only sets a flag for the main loop to act on would not show up. The `36` conclusion relies on the auto-save
  callers instead.
- Static code proves what the firmware can do, never what it does on the wire. No claim in this file is Confirmed in
  the sense of `.claude/rules/experimental-protocol.md`.
- x64dbg was installed (winget `x64dbg.x64dbg`, 2026.05.27) and nothing was attached: attaching to
  `SteelSeriesEngine.exe` would change the engine process, which the verification rules forbid.
- `SteelSeriesEngine.exe` (sha256 prefix `f1c32b85`, 16 MB, Go, 18,103 functions after Ghidra's default
  auto-analysis) was imported and analysed; the Go analyzer recovered function names and `file.go:line` source
  paths. Only the macro conversion functions were decompiled (Go decompilation is noisy: slices, bounds-check panics
  and interface values clutter the output). Nothing else in the engine was read, and it was never run or attached to.

## Open items

1. Trace the file-system layer under `0x080118e8` to its medium, and confirm on hardware that a live `36` binding
   survives (or not) a power cycle, with and without `34 01`.
2. Read a real stored macro from hardware and check the decoded event layout against it; decode the header bytes
   +4 to +7 and locate the firmware player.
3. Decode handler bodies (not only the tables) on `0x1642`, `0x1660`, the Apex Pro Mini and the Apex 9 TKL, and the extra
   `21`, `22`, `25` tags the two 513-byte boards define.
4. Decide on a read-back test for `B8 61` on `0x1628`.
5. Trace how the Apex 3 TKL effect engine consumes the `21` zone records.
6. Identify the RAM flag that gates `3A`/`3B`/`21`/`22`/`23` forwarding in the Gen 1 keyboard apps and the forwarding
   routine.
