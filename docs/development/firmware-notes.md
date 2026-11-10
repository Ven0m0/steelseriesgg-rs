# GG keyboard firmware notes

Static analysis of the firmware images that ship inside SteelSeries GG. Nothing here was run on a
keyboard, no image was modified or flashed, and no firmware bytes are reproduced (offsets, addresses
and decoded field values only).

Source: `C:\Program Files\SteelSeries\GG\apps\engine\firmware\<id>\`, from GG vcs `70d240b7`
(installed 2026-11-02). Analysed 2026-11-10 with `rizin -a arm|avr`. Every claim below is static
evidence only, so it is `[EXPERIMENTAL]` in the terms of `.claude/rules/experimental-protocol.md`.

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
recognises. Two Gen 1 companions are genuinely AVR: `keyboard-app.bin` and `lighting-app.bin` in the
`0x100000` container directories are `avr/ATmega8, bits 8`, 118784 bytes (98304 for Apex 5).

## Loading a raw ARM image

```
rizin -a arm -b 16 -m <base> -e scr.color=0 -q -c "pd 47000 @ <base>" -nn <image>
```

Thumb only: the reset vector is a Thumb address (word 1 of the image ends in `1`). Linear sweep is
enough for reconnaissance; `aa` is unreliable on a headerless Cortex-M image and a linear sweep is
what produced the tables below.

### Load base

The image has no address of its own, so the base has to be inferred. The strongest signal is where
the pointer-like 32-bit words in the file point: 955 words fall in `0x08000000..0x08200000`, and a
correct base puts nearly all of them inside the image.

| Candidate base | Window | Inside | Below | Above |
|----------------|--------|--------|-------|-------|
| `0x08000000` | `0x08000000..0x08017000` | 319 | 0 | 636 |
| `0x08004000` | `0x08004000..0x0801b000` | 472 | 5 | 478 |
| `0x08007000` | `0x08007000..0x0801e000` | 857 | 5 | 93 |
| `0x08008000` | `0x08008000..0x0801f000` | 924 | 5 | 26 |
| `0x08009000` | `0x08009000..0x08020000` | 921 | 16 | 18 |

`0x08008000` scores best and `0x08009000` is nearly tied, so the base is **not settled**: treat it as
`0x08008000 +/- 0x1000`. A separate test that looked for literals pointing at the USB string table
found none for any candidate base, so the strings are addressed through a base register rather than
an absolute literal, and that test cannot break the tie.

All addresses in this file are given **as produced with base `0x08008000`**. If the base turns out to
be `0x08009000`, subtract `0x1000` from every address.

The 64-byte header (SP `0x2000a000`, entry `0x0801265d`) looks like a Cortex-M vector table but most
of its entries land mid-instruction, so it is a packer or bootloader artefact, not a usable table.

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

Decoded from the raw bytes at file offset `0x15df0` (VA `0x0801ddf0` with base `0x08008000`):

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

**Consequence for `GET_FEATURE`:** the descriptor declares a feature report, so the USB stack will
route a class `GET_REPORT(Feature)` request to the firmware. Whether the firmware fills it is a
separate question, answered below.

### Command router

Two near-identical functions dispatch an incoming report: `0x0800e158` and `0x0800e1f0`. They take
the payload pointer plus a length and an output buffer, read `payload[0]` as the command class, and
select a table by class:

| Class | Table (base-relative) | Entry count | Note |
|-------|----------------------|-------------|------|
| `0x38` | `0x0801b648` | 9 | actuation and per-key consts (`38 61`, `38 62`, `38 65`, `38 66`, `38 67`) |
| `0x1f` | `0x0801b30c` | 2 | OLED (`1f 81`, `1f 82`) |
| `0x24` | `0x0801b36c` | 2 | |
| `0x73` | `0x0801b324` | 6 | |

Entries are 12 bytes: a 32-bit tag, then two handler pointers (one per dispatcher variant), compared
against the next payload byte and called indirectly (`blx`). A class that is not in the list returns
`0xff`. `HIDCONFIG`-style commands are not routed here.

The exact tag-to-handler mapping of those four tables is **not decoded**: the words at the table
addresses do not line up as `{tag, handler, handler}` for every entry, so the tables were not written
into any protocol claim. What is solid is the shape: class byte, 12-byte stride, indirect call, and
the class list above.

### 12-byte tables found in the image

An independent scan for runs of `{u32 < 0x200, u32 code, u32 code}` found seven tables. They are
consistent with per-command handler tables, and their tag sets match command bytes that GG's device
specs use:

| Table | Entries | Tags |
|-------|---------|------|
| `0x0801a648` | 9 | `60 83 61 62 63 65 64 66 67` |
| `0x0801b1b8` | 13 | `32 33 34 35 36 37 38 39 3a 3b 3c 3d 3e` |
| `0x0801b260` | 6 | `23 24 26 27 1e 40` |
| `0x0801b2b4` | 7 | `71 72 73 74 75 76 77` |
| `0x0801a330` | 3 | `51 52 53` |
| `0x0801a360` | 3 | `11 02 86` |
| `0x08008028` | 2 | vector-table artefact, not a command table |

The tag sets line up with the specs in `docs/development/gg-keyboard-apis.md`: `0x40` is the per-key
direct frame, `0x24` is brightness, `0x23`/`0x26`/`0x27` are effect and colour commands, `0x74` is
layout and `0x75` is `write_region_id`, and `0x60` to `0x67` are the actuation families. Because the
mapping of a table to a class is not proven, these are reported as evidence that the firmware has
handlers for those command bytes, not as a decoded protocol.

## Answers for the three W3 questions

**(a) Does 1.19.7 answer `GET_FEATURE` for `38 61` or any lighting or actuation read?**

No read path was found. The image contains a 644-byte feature report descriptor and a write-side
router with handlers for `38 61` and the other actuation sub-commands, but no code that assembles a
feature-report reply, and no read command among the command tags. The earlier hardware trace
(2026-10-21) agrees from the other side: GG itself never issues a `GET_FEATURE` on this model.

Status: **not found in image**. This is static evidence, not a hardware result; a `GET_FEATURE`
issued by another host remains untested.

**(b) What is the handler for `83 01`?**

The dispatch scan found no `cmp ..., 0x83` anywhere in the image, but `0x83` is present as a tag in
the nine-entry table at `0x0801a648` (handlers `0x08018375` and `0x08018311`, base-relative). So the
byte is routed, which contradicts an earlier note that the firmware has no `0x83` handling at all.
Whether that entry implements the Gen 3 flash read described by `read_onboard_profile_*`
(`00 83 01 <fs-id> <size> <offset>`) is **not established**: the same byte could serve a different
table. Status: handler addresses found, semantics unproven.

**(c) Full command list versus the status table**

| Command byte | In firmware | Evidence |
|--------------|-------------|----------|
| `0x21`, `0x22` (direct write, clear) | not found as a class | the lighting path may be handled outside the router |
| `0x23`, `0x24`, `0x26`, `0x27`, `0x1e` | yes | table `0x0801b260` |
| `0x32` to `0x3e` | yes | table `0x0801b1b8` |
| `0x40` (per-key direct) | yes | table `0x0801b260` |
| `0x38` with `0x60` to `0x67` | yes | class `0x38`, table `0x0801a648` |
| `0x1f` with `0x81`/`0x82` (OLED) | yes | class `0x1f`, table `0x0801b30c` |
| `0x71` to `0x77` | yes | table `0x0801b2b4` |
| `0x83` | yes | table `0x0801a648` |
| `0x90`, `0x65`, `0x66`, `0x67`, `0x41` | not found | no `cmp` with these values; they belong to the Gen 1 firmware |

The absence of `0x90` in the Gen 3 image matches the repo's note that `0x1628` has no firmware-version
command and that GG reads the version from the USB descriptor instead.

## Apex 3 and Apex 3 TKL

`firmware-apex-3-tkl-3.0.4.bin` (`0x1622`, `0xB400`) and `firmware-apex-3-v0x28.bin` (`0x161A`,
`0xB800`) are raw ARM images with the same "no header" signature as 1.19.7. They were not triaged
further; the Apex 3 lighting protocol in `docs/development/gg-keyboard-apis.md` comes from the GG
device specs, not from these images.

## Gen 1: the 1 MB images are flash containers

`firmware-apex-pro-v4.16.8.bin` (1048576 bytes) starts with `a5 a5 01 00`, then a table of 32-bit
fields (total size, then per-record offset and size). Two of the records are byte-identical to the
companion files:

| Record offset | Size | Content |
|---------------|------|---------|
| `0x1000` | `0x1d000` | `keyboard-app.bin` (exact match, full length) |
| `0x1e000` | `0x1d000` | `lighting-app.bin` (exact match, full length) |

So the container is a flat flash layout, not a compressed or encrypted format, and the Gen 1 protocol
code lives in the two AVR `ATmega8` apps. Apex 5 and Apex 5 2024 use the same layout with 98304-byte
apps. The 98304-byte Apex 5 apps are reported as `arch N/A, bits 64`, so that pair is not the same
AVR build as the `0x1d000` pair.

Analysis implication: the Gen 1 `0x3A` lighting handler has to be read with `-a avr -b 8` on the
`lighting-app.bin` record, not with the ARM decoder.

## Method and limits

- `rizin -a arm -b 16 -m 0x08008000 -e scr.color=0 -q -c "pd 47000 @ 0x08008000" -nn <image>`
  produces a linear Thumb sweep; post-process it with a range printer instead of reading the raw
  output, because the console output is box-drawing heavy.
- There are no `movw`/`movt` pairs in 1.19.7, so every constant comes from a literal pool. A literal
  pool value is only an address if the code loads it with `ldr rX, [pc, ...]` and then dereferences
  it; a value that merely looks like a flash address is not evidence of a pointer.
- `rz-bin -s` can not resolve anything in these images; all symbol knowledge here comes from manual
  structure recovery.
- Static code proves what the firmware can do, never what it does on the wire. No claim in this file
  is Confirmed in the sense of `.claude/rules/experimental-protocol.md`.

## Open items

1. Settle the load base (`0x08008000` versus `0x08009000`).
2. Decode the class `0x38` / `0x1f` / `0x24` / `0x73` tables entry by entry.
3. Find the HID class-request handler and confirm what it does with `GET_REPORT(Feature)`.
4. Triage the Apex 3 and Apex 9 images the same way.
5. Locate the per-key `0x40` handler and check whether it is a copy of the lighting path seen on the
   wire.
