# Apex 7 (wired, PID 0x1612): SteelSeries GG wire capture

Status: the on-board profile **read, write and deploy are confirmed on hardware** on an Apex 7 (`0x1612`)
under Linux (2026-10-20): every frame below that `ssgg` sends has been put on the wire and answered by that
keyboard, and the command statuses are listed in `.claude/rules/experimental-protocol.md`. The **payload
contents are understood**: the crate's codec decodes and encodes every block of the table below - header,
name, lighting fields, the META byte, all four mapping tables, Rapid Tap, the GUID and its padding, the tail
checksum - and `encode` is verified frame for frame against captured clean and modified profiles. The OLED
block is read as what it is: a 128x40 monochrome bitmap whose bytes **were read back from an Apex 7 (`0x1612`)
and written back to one**, and a write carries those bytes verbatim. The only part of a payload nobody
interprets is the *inside* of the 9,600-byte macro block: the engine declares it an opaque byte array and a
write sends those bytes back unchanged, and the macro-record grammar in this document is `[EXPERIMENTAL]` and is
used for reporting only. The GUI converts the OLED block between its bytes and a drawn picture (draw, clear,
invert, PNG export). The same
protocol covers the
other Apex keyboards of this legacy platform - the engine's protocol file is written per platform, not per
model - and `discovery::keyboard_class` enables it on the one board that was measured, leaving the rest
untested.

Two USBPcap captures of official SteelSeries GG on the same keyboard, taken on **the same firmware** (the
`90 00` poll answers `4.16.8`): a **read capture** (GG running a profile read session) and a **write capture**
(2026-10-20, 677,636 bytes). Nothing differs between them except direction - every payload in both is schema
`B`. Frame counts quoted below are from those captures. The write frames are extracted by
`scripts/gg-write-frames.py`; its manifest (`tests/fixtures/onboard-write-apex7.manifest.txt`, 122 lines) is
in git history, not in the current tree.

## Transport

All profile traffic runs on the keyboard's vendor collections on USB interface 1 (`wIndex = 1`): usage page
`0xffc0` on interface 1, where every capture transfer sits, and usage page `0xffc1` on interface 4 — the
collection the Linux `hidraw` node for the profile protocol sits on. Report numbers are embedded in the
payload, so in `hidapi` terms byte 0 of every buffer is the report number, and it is `0x00` for every frame
of this protocol.

| Channel | `wValue` | Wire length | Use |
|---------|----------|-------------|-----|
| Output report, report 0 | `0x0200` | 64 | firmware poll, pull, payload push, file open, commit, deploy, driver mode |
| Feature write, report 0 | `0x0300` | 642 | payload read request (`83`), resource-buffer write (`03 00`) |
| Feature read (`GET_REPORT`) | `0x0300` | 642 | the answer to a payload read |
| Interrupt IN endpoint `0x82` | — | 65 read size | input reports: pacing packet after a pull, firmware answer, write answers |

Report-buffer sizes are the constants in `src/devices/hid_reports.rs`: `ONBOARD_OUTPUT_REPORT_SIZE` (65, i.e.
64 wire bytes), `ONBOARD_FEATURE_REPORT_SIZE` (643, 642 wire bytes), `ONBOARD_READ_WIRE_BYTES` (642) and
`ONBOARD_MAX_READ_LEN` (636, the ceiling a payload read may ask for). A 643-byte `GET_REPORT` ask never
completes: the capture's `wLength` is 642 and the read forwards it verbatim.

The interrupt endpoint streams the keyboard's ordinary input reports (report `0x01`, 63 bytes, all-zero on an
idle keyboard; median gap 14 ms = the key polling interval). GG reads that endpoint with a 65-byte read
(`GG_INPUT_READ_BYTES`: a report-ID slot plus 64 data bytes) and that same read serves the pacing packet after
a pull, the firmware poll's answer and the write answers.

Transfers run on the handle the device manager opened for the collection and keep open for the whole transfer:
`usbhid` polls a collection's interrupt endpoint only while a descriptor on that node is open, so the pacing
packets and answers arrive only on a handle that stays open (`apex_7.rs`, module header).

## Firmware poll (output report, `0x90`)

`90 00` asks the keyboard's firmware, `90 01` the LED controller's firmware. The answer lands 1-3 ms later on
the interrupt endpoint as `[report ID 0x6E][ASCII version]`, NUL padded (`34 2e 31 …` = `4.16.8`). GG polls
**both**, and they are the first two frames of its opening chain (below). `ssgg` sends `90 00` in front of the
schema probe for the firmware line of a read image; an unanswered poll is not an error
(`Apex7::firmware_version`).

## The session framing GG adds (and `ssgg` does not send)

GG opens a profile session with a **six-frame chain** of 64-byte output reports, 11 ms apart, and closes it with
a four-frame sequence:

| Order | Frame | Answer |
|-------|-------|--------|
| 1 | `90 00 00 …` | `[0x6E][ASCII]` - the **keyboard** firmware (above) |
| 2 | `90 01 00 …` | `[0x6E][ASCII]` - the **LED controller** firmware (above) |
| 3 | `f4 00 00 …` | `00 01 00 …` - the answer is `1`; **meaning unknown** (see below) |
| 4 | `6c 00 01 00 …` | none — **driver mode enable**: `DriverModeEnable` in the engine's init-sequence descriptor, frame in the read capture |
| 5 | `90 00 00 …` | `[0x6E][ASCII]` |
| 6 | `90 01 00 …` | `[0x6E][ASCII]` |
| - | `69 00 …` | closing sequence: OLED off |
| - | `89 <slot + 1> 00 …` | closing sequence, after a 250 ms sleep: deploy that slot |
| - | `41 00 00 …` | closing sequence: profile not volatile |
| - | `6c 00 00 …` | driver mode released at session close - **engine-side only, not in the capture** (GG was killed in it) |

Frame 4 is the engine's **driver mode enable**: the init-sequence descriptor names `6c 00 01` as
`DriverModeEnable`, the read capture carries that frame as 64 zero-padded bytes, and nothing answers it. The same
opcode closes the mode with `6c 00 00` at session close - engine-side, not in the capture.

GG drains one input report after every chain frame, and the answers come one at a time: a chain run that
drained only the first answer collected the `90 00` and `90 01` answers and nothing from frame 3 on, with all
six frames on the wire (Apex 7, 2026-10-18). Why an undrained answer silences the later frames is not measured;
it is the reason the pull loop drains one input report per pull.

**`ssgg` sends none of the chain.** A full slot read completes on hardware without it (measured 2026-10-19), so
the read changes nothing on the keyboard. `0xF4` is the one frame of the chain that answers and has no meaning:
the answer is `1`, nothing on the engine side names the command, and `ssgg` implements nothing for it - a
previous version of this code called it `EngineStatus`, which is a name this project invented, not a fact. An
earlier run that sent only the `90 00` poll got scrambled payload replies; the reading that the missing piece
was the **undrained answer** and not the frames is inference, not measurement.

The closing set matters as a group because `0x89` **writes** - the keyboard runs the slot it names - so a tool
that replays it changes the keyboard's active profile. `69 00` (Gamesense OLED off) and `41 00 00` (profile not
volatile) are named from the engine's own words; the capture has three `89` and three `41` - one pair behind
each profile file - and two `69`, while `6c 00 00` is engine-side only. `ssgg` sends none of them; its only
closing frame is `0x89`, on an explicit write (`apex_7::deploy_frame`, with the engine's 250 ms pause in front
of it).

Driver mode (`0x6C`, `OnboardDriverModeCommand`) is `[EXPERIMENTAL]`: the opening frame `6c 00 01` is
`DriverModeEnable` in the engine's init-sequence descriptor and is in the read capture, answered by nothing; the
closing frame `6c 00 00` is the engine's session-close write (engine-side, not in the capture). Nothing in `ssgg`
sends either, and the profile read works without them. The Apex 3 family splits: Apex 3 uses command `0x03` in a
33-byte report, Apex 3 TKL uses `0x6C` and opens with driver mode engaged.

## Profile read: pull (`0x85`) and payload read (`0x83`)

The read is two frames deep: a pull copies a slice of a profile **file** into the keyboard's 1024-byte
resource buffer, and payload reads drain that buffer.

```
pull (output, 64)          85 00 | file id u16 LE | file offset u32 LE | size u16 LE | 00 …
payload read (feature, 642) 83 00 | buffer offset u16 LE | size u16 LE | 00 …
```

- **Files are addressed by id.** Profile slot `n` is `0x0201 + n` (`PROFILE_FS_BASE`); GG's `Config 1` ..
  `Config 5` are slots `0..4`, which is what `ssgg onboard --profile N` takes: `N` is GG's config number, the
  slot behind it is `N - 1`, the file is `0x020N`. Behind that, GG addresses a second family of file ids,
  `0x0301 + n` (`LIGHTING_FS_BASE`), whose payload-read replies carry report ID `0x03`
  (`LIGHTING_READ_REPLY_REPORT_ID`): **presumed** to be the per-profile lighting config from the engine's
  naming, and **never confirmed on hardware** - no `0x030N` file has been read from or written to a keyboard,
  and nothing in `ssgg` addresses those ids. Every frame this project has run on hardware is on a `0x020N`
  profile file.
- **A pull has no answer of its own.** All 121 pulls of the capture are followed 1.7-3.7 ms later by an
  all-zero report `0x01` on the interrupt endpoint, and GG keeps no bytes of it: the interrupt packet is a
  pacing token, not a status frame.
- **The payload read answer carries no header**: `GET_REPORT wValue 0x0300 wIndex 1 wLength 642`, with the
  payload starting at byte 1 (the kernel lands the device's report-ID byte in buffer[0]). Some firmware echoes
  the request header (`03 01 01 00 01`) and puts the payload at byte 6; `parse_read_reply` accepts both, which
  is why a read frame never asks for more than 636 bytes.
- **GG probes a file before reading it**: one 2-byte pull of the file head plus one 2-byte payload read
  answers `03 | 0b 00`. The first two payload bytes are the schema word. The first 512-byte read of the same
  file answered `03 | 0b 00 43 6f 6e 66 69 67 20 31 …` — schema word at offset 0, profile name at offset 2.

Read geometry (a fact of the device, not a tunable — `apex_7_profile.rs`):

| Number | Value |
|--------|-------|
| Resource buffer per pull | 1024 bytes (`DEVICE_RESOURCE_BUFFER_LEN`) |
| Pulls per profile file | 12 (`PULLS_PER_FILE`), at file offsets 0, 1024, … 11264 |
| Payload reads per pull | 2 of 512 bytes, at buffer offsets 0 and 512 |
| File bytes one read covers | 12,288 (`PROFILE_FILE_BYTES`) |
| Pacing between our frames | 5 ms (`GG_GAP_MS`); GG's own opening-chain frames are 11 ms apart |

Capture counts: 120 pulls of 1024 bytes (100 of them on file `0x0201`) plus the 2-byte probe, 241 payload
reads, and **every** payload answer started with the first payload byte `0x0B`, i.e. schema `SCHEMA_B`.

In `ssgg`: `ssgg onboard read --profile N` (add `--bin` for the image bytes, `--report` for the per-pull read
record, `--verify` to read twice and compare, `--hex` to dump). About 2 s per slot. The read sends no closing
sequence and no driver-mode frame, so nothing on the keyboard changes.

## Profile payload layout

The payload is GG's own struct: `profile_all_settings_struct` (schema `0x000A`, 11,648 bytes) and
`profile_all_settings_schema_b_struct` (schema `0x000B`, 11,696 bytes) in the engine's protocol files, whose
byte comments give every offset below. Both captures are schema `B` (the Rapid Tap block added): the engine's
decoder lists firmware `4.16.3` as the switch to schema `B`
(`apex_7_profile::SCHEMA_B_FIRMWARE_VERSION`, documentation only), and every payload seen on the wire - 241
read answers and the write capture's 11,696-byte payloads - starts `0B 00`. The read never compares a firmware
version with anything, it takes the schema from the word it probes. The engine's structs carry one leading
constant id byte that is not part of the file, so the assembled image has no id byte. `decode` (in
`apex_7_profile.rs`) reads every block of this table into named JSON fields, and `encode` writes all of them
back and recomputes the tail checksum, so a report the read path produced round-trips byte for byte; the two
blocks declared as byte arrays are carried through as hex and parsed on the side for display.

Schema A is 11,648 bytes and schema B is 11,696; they share every offset below except the rows marked.

| Offset | Length | Content |
|----------------|--------|---------|
| 0 | 2 | schema word (`0x000A` / `0x000B`) |
| 2 | 24 | profile name (ASCII String, NUL-terminated), then one pad byte |
| 27 | 57 | lighting block (fields below) |
| 84 | 1 | META toggle key code (see below) |
| 85 | 640 | 128 button mappings (5 bytes each) |
| 725 | 10 | 2 wheel mappings |
| 735 | 640 | 128 button META mappings |
| 1375 | 10 | 2 wheel META mappings |
| 1385 | 640 | OLED block (opaque `bytearray`, carried byte-for-byte - see below) |
| 2025 (B only) | 1 | Rapid Tap enable byte |
| 2026 (B only) | 50 | ten 5-byte Rapid Tap pairs (`onboard_rapid_tap_pair`) |
| 2025 (A) / 2076 (B) | 9600 | macro block (the engine's `macro-data-size`, shipped as a `bytearray`) |
| 11625 (A) / 11676 (B) | 16 | profile GUID |
| 11641 (A only) | 3 | pad (schema B has none) |
| 11644 (A) / 11692 (B) | 4 | checksum, u32 LE (see below) |

**The OLED block (1385, 640 bytes) is encoded as-is.** `decode` copies it whole into `oled.bytes_hex` and
`encode` writes those bytes back untouched, so a write sends exactly the bytes a read produced; the engine ships
the field as a `bytearray` and parses nothing inside it. The conversion between those bytes and a picture exists
only in `tools/profile-gui`: it rasterises 128×40 monochrome pixels (8 px per byte, 16 bytes per row, bit 7 of
each byte the left pixel) for the drawn screen, the Clear / Invert controls and the PNG export, and writes a
bitmap back into the same JSON field. **That geometry and bit order are confirmed on an Apex 7 (`0x1612`)**: the
operator took the image GG paints on the panel, decoded it into bytes, and read those bytes back from the
keyboard as the block; writing them back paints that picture. Nothing in the Rust codec rasterises.

Lighting block (offsets inside payload):

| Offset | Length | Field |
|--------|--------|-------|
| 27 | 6 | OLED illumination options |
| 33 / 36 / 39 | 3 each | lock-key cluster colour, META-mapped cluster colour, disabled Win key colour |
| 42 | 32 | META mapping mask |
| 74 | 3 | lock-key HID codes |
| 77, 78, 79 | 1 each | Win key HID code, META-key background dimmer, idle-mode dimmer |
| 80 | 4 | idle timeout, ms (u32 LE) |

The META byte at 84 is GG's **code** for the key that switches META mode, not always that key's own usage:
`0xF0` is GG's code for the Meta cap (usage `0x65`, which has no slot under its own usage and binds through
slot 93), any other value names a key by its own usage, and `0x00` is no key. A Rapid Tap pair is
`hid_1`, `hid_2`, a mode byte whose high bit means "report both keys", and 2 reserved bytes.

Mapping tables use GG's `key_mapping`: a function byte plus **four** key codes (5 bytes per entry, not four).
Function selectors seen in GG's config: `0x00` deactivated, `0x01`-`0x08` mouse buttons, `0x30` CPI, `0x31`/`0x32`
mouse wheel, `0x33`/`0x34` pan, `0x51` combo key (GG's factory layer binds every key to itself: `0x51` + its own HID
code), `0x61` media key, `0x62` META, `0x71` macro; anything else is `unknown`. Mapping **slot** to key is GG's
`device-offset-lookup-table`, carried in `apex_7_profile.rs` as `OFFSET_TO_HID` / `HID_TO_OFFSET`; slots
`120..127` are not keys but the `0xF1..0xF8` function codes carrying the wheel and media bindings.

The macro block is carried through by decode and encode as one 9,600-byte byte array; the record split below is
the `[EXPERIMENTAL]` grammar this project derived from it, used when reporting a profile - a write never needs
it, it sends the block's own bytes back. Records start with marker `0x77`:

```
header  0x77 | mode u8 | id u16 LE | flags u16 LE | interval u16 LE    (8 bytes)
press   0x02 | key | 0x01 0x01 | word u16 LE | ms u16 LE               (8 bytes)
release 0x02 | key | 0x01 0x00                                         (4 bytes)
delay   0x04 | 0x00 | ms u16 LE                                        (4 bytes)
```

Modes seen: `1` = play once, stepping `interval` ms between actions; `4` = play again every `interval`
(a mode-4 record with `interval` 5000 presses `4`, which is the META + Insert binding of profile 1). A `0x71`
mapping row carries `record ordinal × 7` in its third code byte, so `index / 7` names the record
(`MACRO_INDEX_STRIDE`, `[EXPERIMENTAL]`: every binding a read carried resolved that way, and the press `word`
(4 in every press seen) is unexplained).

**Checksum**: the little-endian u32 at the payload tail is the STM32 hardware CRC-32 (poly `0x04C11DB7`, init
`0xFFFFFFFF`, no input/output reflection, no final XOR) over `payload[0..len-4]` read as little-endian 32-bit
words — the same rule as the 2023-family profile blob, and the same function
(`apex_pro_tkl_2023_profile::profile_checksum`, re-exported as `apex_7_profile::compute_checksum`). It verifies
on a factory read: slot 0, schema B → `0xAE57B9DC` over `payload[0..11692]`. `decode`
reports stored and computed words; `encode` recomputes and ignores the stored word.
The 64-bit FNV-1a digests in the read report are identity hashes for comparing reads
(`apex_7_profile::fnv1a_64`), not protocol checksums.

## Macro decode: partial, display-only

`decode` walks the macro block into a record list (`apex_7_profile::decode_macros`), and `tools/profile-gui`
displays it: the Macro panel lists every record the walk produced (mode, id, interval, flags, its keystrokes, the
keys bound to it, its raw bytes), a key bound with `0x71` shows the same record in its own panel, and the binding
select offers the decoded records as bind targets. At the moment, only five most recently written to keyboard
macros are read here.

What is **not** supported, and the limits that go with it:

- **No macro editing.** Nothing in `ssgg` or the GUI composes, inserts, deletes, reorders, trims or renames a
  record. The page can rebind a key to a record - it writes `ordinal * 7` into the third code byte of the
  binding - and it cannot touch the record's actions.
- **The record list is never encoded.** A write takes `macro_data.bytes_hex`, the block as the read recorded it,
  and sends it verbatim; the decoded list in `macros` is not serialised, so a list that disagrees with the bytes
  has no effect on what the keyboard gets.
- **The walk is best-effort.** The list has no length word and no stride, so the walker stops at the first byte
  that is neither the zero pad nor a `0x77` record start, and returns the records up to that point with a note in
  `macro_note`. `ssgg` has seen real GG blocks whose bytes its grammar rejects. A report carrying such a note and
  no `bytes_hex` is refused by the write path (`OnboardWriteError::MacroRecordNote`).
- **Bytes inside a record are unconfirmed.** `flags` is `01 01` in every record seen and is unexplained; the only
  modes measured are `1` and `4`; `id` is read as GG's database id. The binding stride 7 is `[EXPERIMENTAL]` -
  every binding measured resolved by `index / 7`, and the largest binding seen (117 = 7 × 16 + 5) needs 17
  records, the same order of size as the 9,600-byte block.
- **Macro names are not in the payload.** The name and the macro as GG displays it live in GG's own database
  (`db\database.db`); the payload carries packed records only, which is why the panel labels a record by ordinal,
  mode and action count and not by name.

## Profile write: open (`0x88`), buffer write (`03 00`), push (`0x05`), commit (`0xEB`)

The engine's write API fills the whole **profile region**, which is larger than any schema's payload:
`profile_size` = `0xA000` = 40,960 bytes (`PROFILE_REGION_BYTES`). Frames (`OnboardWriteFrame`, plan built by
`write_plan`, 122 frames for one slot):

| Frame | Report | Layout | Answer |
|-------|--------|--------|----------|
| open | output 64 | `88 00 \| file id u16 LE` | `88 01 …`, 305 ms after the frame |
| buffer write ×2 | feature 642 | `03 00 \| buffer offset u16 LE \| size u16 LE \| data` (offsets 0 and 512, 512 bytes each, data behind the 6-byte header) | none |
| push | output 64 | `05 00 \| file id u16 LE \| file offset u32 LE \| size u16 LE` (1024 = the whole buffer) | `05 01 …`, 5 ms after the frame |
| commit | output 64 | `eb \| 1-based slot` | none |

Pattern: one open, then **40 windows** of the region — two buffer writes (offsets 0 and 512) and one push of
the full 1024-byte buffer per window at file offsets 0, 1024, … 39936 — then the commit. Frames go out 5 ms
apart. A write pads the payload out to the region with zeros, so the frames past the payload end (the payload
is 11,696 bytes at most) push zeros: the file's own size is never read from the board.

- Every answer a write gets carries status byte `0x01` in the capture, in 202 answers out of 202. `ssgg`
  treats any other value, and any frame the capture shows answered going unanswered, as a refusal and stops —
  the frames after a refused one build on it. What `0x01` means beyond "accepted" is observed, not proven.
- After the commit `ssgg` pauses 250 ms (`DEPLOY_SETTLE_MS`) and sends the deploy frame `89 <slot + 1>`.
  Measured on hardware: the deploy sent straight after the commit **can do nothing**; waiting **before** it is
  what makes the keyboard switch slots, and how much the device needs is not measured. 250 ms is the engine's
  own number — its `load-profile-with-pause` sleeps 250 ms *behind* the deploy frame, and its commit path ends
  with an input read that blocks until the keyboard answers.
- The write capture (2026-10-20) reproduces this geometry frame for frame; its write runs contain 72 pushes,
  every one answered, and every push is followed by an all-zero pacing packet, exactly as the pulls are.

In `ssgg`: `ssgg onboard write --profile N <profile.json>` (or `--bin <image.bin>`) prints the family's
geometry banner and the 122-frame plan, then runs it on the keyboard's own handle — `--dry-run` prints the same
plan with no keyboard opened and nothing sent. `ssgg onboard load --profile N` sends the deploy frame alone.
The payload can come from a JSON (`onboard decode` report → `encode_json`) or from bytes an earlier
`onboard read --bin` wrote.

**Verification**: an on-board write has no read-back frame. A write is checked from the device by reading the
slot back with the read path and diffing the payload (`apex_7_profile::image_diff` / `payload_diff`), and
`ssgg` prints "read the slot back before trusting it" whenever the frames it sent do not match its plan.

## Where GG's own words are

- **GG's device specifications ship encrypted** (`apps\engine\deviceSpecifications\*.edevice`). The `.lisp`
  files cited below - `apex_7.lisp`, `base_apex_legacy.lisp` - are the **decrypted** contents of those
  `*.edevice` files from the SteelSeries Engine distributable: plain lisp, one file per keyboard family,
  giving its protocol, tables, sizes and init sequence. Everything this document attributes to "the engine's
  own words" is that plaintext.
- The engine's protocol files `apex_7.lisp` and `base_apex_legacy.lisp` (GG's device protocol definitions for
  the legacy platform): the payload structs and their byte comments, `key_mapping`, `onboard_rapid_tap_pair`,
  `device-offset-lookup-table`, `profile_size` = `0xA000`, `macro-data-size` = 9600, the profile slot range
  `(range 1 5)`, and `load-profile-with-pause`. The **init-sequence descriptor** in those files is what names
  the chain's frames, `DriverModeEnable` among them.
  The code cites these files by line next to the constants they give (`apex_7_profile.rs`).
- GG's stored per-device config as JSON: `C:\ProgramData\SteelSeries\GG\apps\engine\db\database.db`, table
  `configurations`, column `settings`. Open it read-only with GG stopped.
- Reference profiles live in `tools/profile-gui/examples/`: `sample-identity.{bin,json}` (decode of a real
  stock slot — name `Config 3`, macro block all zero, 11,696 bytes, trailing checksum word equal to the CRC the
  crate computes) and `sample-remap.{bin,json}` (that decode with base remaps written in, the meta colour
  edited, name `Remapped`).

## Open questions

1. A read collects 12,288 file bytes, however a write pushes 40,960. According to device specs, these should not
   be read, so the purpose of writes is not clear.
2. The *meaning* of the bytes inside the macro block: decode and encode carry the block through intact, the
   engine treats it as an opaque byte array, and a write sends back the bytes a read produced. The macro record
   split above is `[EXPERIMENTAL]` and is used for reporting only.
3. Where macros beyond the last five are stored. The keys function for all macros, so the internal memory for
   them must exist, but at the current path it is not read.
4. A schema `A` payload has never been read on hardware. Both captures are the same firmware and all 241
   answers are schema B, so nothing in the captures discriminates firmware at all; the schema `A` layout comes
   from the engine's struct comments only, and the `4.16.3` threshold is the engine decoder's own number.
5. What the `0x030N` files are. Their ids and the reply report ID `03` behind them are in the capture; that
   they are the per-profile lighting config is GG's naming, not a hardware fact - `ssgg` only ever reads and
   writes `0x020N`, no `0x030N` file has been read on hardware, and nothing decodes one. `ssgg`'s Apex 7
   lighting goes out over the zone commands (`0x21`) from the OpenRGB-era code, not from this capture.
6. What `0xF4` does. GG sends it third in its opening chain and the keyboard answers `1`; no engine-side word
   names it, `ssgg` implements nothing for it, and a read completes without the chain. Open too: what driver mode
   does on the device, and the full bytes of the closing frames - `69 00` (OLED off) and `41 00 00` (profile not
   volatile) are inferred from the engine's names.
