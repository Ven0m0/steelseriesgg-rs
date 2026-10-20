# On-board profile JSON editor

A browser page that reads the JSON dumps `ssgg onboard read` and
`ssgg onboard decode` write (by default into a file named from the profile name in the
image: `Config 1` -> `config_1.json`), edits them on a drawn keyboard, and saves
the JSON back. No build step, no dependencies, no network: four plain files
(`index.html`, `style.css`, `app.js`, `main.js`).

`read` writes that report directly: it decodes the image it pulled. The per-pull read
record (`read --report FILE` - chunk headers, framing stats, image hash) is capture telemetry, not a
profile, and this page does not open it.

It is a **file editor only**. It never opens a device node and writes nothing to
a keyboard - the bytes it saves are what `ssgg onboard write` sends. The protocol behind them is
reverse-engineered from GG captures of one Apex 7 (`0x1612`) and from GG's own device specifications. Its read, write and deploy are confirmed on
that board, and decode and encode match GG for clean and modified profiles.

## Run

```sh
xdg-open tools/profile-gui/index.html          # straight from the filesystem
python3 -m http.server -d tools/profile-gui 8080   # then http://127.0.0.1:8080
```

`app.js` and `main.js` are **plain scripts**, not ES modules: a page opened from
the filesystem has origin `null`, and browsers refuse module scripts there, so
`index.html` loads them with `<script src=... defer>` in order and `app.js`
publishes one namespace object (`globalThis.SsggProfileApp`).

Open a dump - the Open button, or drop a file anywhere on the page. **One profile
is open at a time**: a new load replaces the open one (dropping several files loads
the first and says so), so the board, the panels and the Save prompt can never point
at a stale slot. The picker opens in `tools/profile-gui/`, so copy a dump here first or
navigate to where it is. The tracked fixtures under `examples/` are **decodes of real slot
captures**: `sample-identity.json` is the stock slot (macro block empty, profile name
`Config 3`), `sample-remap.json` is that decode with base remaps written in and the profile
name `Remapped`. A third sample that carries packed macro records is not tracked - it holds
the operator's own macros - and the suite's macro checks report `skip` in a checkout without
it; those checks run on the fixture bytes `selftest.mjs` assembles, which include a block the
record grammar **rejects**. A read *record* (the per-pull telemetry with `chunks` and no `oled`
block) is refused by name: this page edits profiles, not logs.
`Save a copy` prompts for nothing: the browser download already asks where to put the file, so a
dialog in front of it is one extra question for one decision. The copy name is derived from the
loaded file - `x.json` downloads as `x-copy.json`, a copy of a copy stays `x-copy.json`, and a
document with no loaded name uses the profile name (`Config 1` -> `config_1-copy.json`). The marker
is the point: the name a save offers is never the name of the file that was loaded, so a save can
not look like an in-place write.
The status line reports what loaded (name, character count, slot, schema, checksum,
macros) and every reason a file was refused.

Two picker traps this page avoids, both of which look exactly like "the JSON
does not load": a `display:none` file input is not labelable (clicking its label
opens nothing), so the input is kept in the layout and only made invisible; and
a picked `File` belongs to the `<input>`, so resetting `input.value` detaches it
- the page reads every file's text **before** resetting.

## How keys are bound

`button_mappings` / `button_meta_mappings` are indexed by **profile mapping
offset**, not by key. GG resolves offset to key with the legacy family's HID
table (`lisp/base_apex_legacy.lisp`, `device-offset-lookup-table`) inverted -
the same inverse its own `onboard-to-config-button-mapping` uses. `app.js`
embeds that table as `HID2OFF`/`OFF2HID`, so:

- each key cap shows its legend; the **sub line prints only for a remapped key** (a factory
  board prints nothing - `self` on 112 caps is noise), and the offset, function byte and
  codes are in the hover tooltip and the Key panel;
- 120 of the 128 offsets resolve to a key; the 8 the table does not map are
  left alone by every bulk edit;
- the 17 mapped codes with no place on an ANSI full-size shape (`0x32`, `0x64`,
  `0x85`, `0x87`-`0x8b`, `0xf0`-`0xf8`) are drawn in a strip under the board,
  so no offset is hidden;
- `Meta` (`0x65`) is drawn dim: it has no offset, so it is labelled and lit but not bindable.

The shape is a standard ANSI full-size board (104 keys, key-unit geometry in `LAYOUT_ROWS`):
the main block is exactly 15 units wide in every row from Backspace down, so Bksp, `\`,
Enter, RShift and RCtrl share the right edge; the nav block occupies columns 16-19 under
PrtSc/ScrLk/Pause, the numpad columns 20-24 - a full unit clear of the nav column and the
arrows - with the tall `Ke +` in numpad rows 2-3 and `Ke Enter` in rows 4-5, both on the
`Ke -` column (x 23). The bottom row is drawn in the order the caps sit on the board:
LCtrl, LWin, LAlt, Space, RCtrl, RWin, Meta, RAlt. The function row sits in four groups: F1-F4 above 2-5, F5-F8 from x 6.5 (half way
between 6 and 7) to 10.5, F9-F12 from x 11, so **F9 sits above `-`** and F12 ends on the
15-unit edge under Backspace. The wheel caps sit in the `Ke -` column (x 23) with their
**top on the function-row line** - the same top as PrtScn - and end 0.3 unit above the
`Ke -` row line; that 0.3-unit gap is GG's own wheel column, which `Ke -` shares.

Two more things are drawn on the board and are not 128-table keys. The two half-height caps
are `wheel_mappings[0]` and `[1]` (the META layer shows `wheel_meta_mappings`): they paint
their mapping, and clicking one opens the same panel that binds keys, with that entry as its
target - there is no wheel tab and no wheel table to edit by hand. The rectangle over
the numpad's first three columns (x 20-23) is the on-board OLED - a display, not a binding -
with its top on the same line as PrtScn, drawn 16:5, which is exactly the 128x40 the bitmap
is: its height is computed from the width the board draws, so the ratio holds at any `UNIT`.
See [OLED bitmap](#oled-bitmap).

The board is drawn with `UNIT = 48` in a 25-unit viewBox and scaled by the browser
(`svg.board { width: 100%; height: auto }`, no `min-width`, no `overflow-x` on the
box): it always fits its column - it never scrolls, never crops, on any window. A
wider left column means bigger keys; `selftest.mjs` checks those CSS rules so the
page cannot grow a scroll bar back.

Legends are centred, and a 1-unit cap is `UNIT - GAP` = 42px wide, so 11px mono takes
5 glyphs there and longer labels drop to 10px/8.5px (`labelClass`); the sub line has its own
shrink ladder (`subClass`: 8 characters fit the widest use, the wheel caps). Consequences:
the numpad is drawn as its own block, so its caps read `7`, `Enter`, `/`, not `Ke 7`,
`Ke Ent`, `Ke /` (`LAYOUT_LABELS`; the `Ke *` names stay in `KEY_NAMES` for lists,
tooltips and bindings, where a bare `1` would be ambiguous with `0x1e`), the arrow
keys are centred glyphs (`ARROW_GLYPHS`), and PrtSc/ScrLk/NumLk/Pause fit. The wheel caps are
half height (42 x 18 px), too small for the label-over-sub layout, so they carry **a
direction glyph (`↑`/`↓`) over the mapping text, both inside the cap** (`wheel-glyph`,
`wheel-sub`) and no legend.

A mapping entry is GG's `key_mapping`: **one function byte + four key-code
bytes** (`KEY_MAPPING_LEN = 5`). Rapid Tap pairs, the lock-key codes, the Win
key code and the META toggle byte carry raw HID codes, not offsets; a Rapid Tap
key is marked `[RT n]` on the cap. Function names mirror `function_name` in
`src/devices/keyboards/apex_7_profile.rs` (`0x00` deactivated, `0x51`
combo_key, `0x62` meta, `0x71` macro, ...).

**What counts as a binding** is the part a stock profile gets wrong if you only
look at the function byte. GG's factory default for a key is
`0x51 + <its own key code> + 00 00 00` - a combo key bound to itself - and the
slot GG leaves unused is `00 00 00 00 00`. Neither is a remap. So each entry is
classified:

| `binding` | bytes | on the board |
|-----------|-------|--------------|
| `self` | `0x51` + the slot's own key, rest zero | neutral cap, prints nothing |
| `none` | all zero | neutral cap (one step darker), prints nothing |
| `custom` | anything else | coloured by function, its target printed in the cap, listed in **Custom bindings** |

A factory wired Apex 7 read (`examples/sample-identity.json`)
therefore reads `base: 6 custom, 112 bound to themselves, 10 empty` - the 4 media-cluster
entries (slots 120, 124, 125, 126: function `0x61` with consumer codes) plus slots 29 and 69
(`0x51` pointing at F13/F14), which is what GG ships. The **Custom bindings** panel lists only the real remaps, in slot
order, and clicking a row selects that key.

## The META toggle

The profile binds exactly one META toggle key, and the page splits what the reads show into **two
fields**:

| field | bytes | what it is |
|-------|-------|------------|
| `meta_toggle_hid` | one byte at `META_TOGGLE_HID_OFFSET` | the key GG switches META mode with, named by its code: `0xF0` is GG's code for the **Meta cap** (`0x65`), which binds through slot 93; any other key is named by its own usage (the user profile carries `0x10` = M); `0x00` is no key |
| a `62 05` meta binding | `button_mappings[n]` / `button_meta_mappings[n]` = `0x62` with code 1 = `05` | the meta action **Lock Win** (it locks the Windows key), written on keys GG puts on the Meta layer: a stock slot read carries `62 05 00 00 00` on the LWin slot of both layers - the slot is LWin, which is the key the action names - the user profile on slots 13 and 35 of the META layer. It is **not** the toggle - the byte above is |

Both are shown in the header row (`META toggle` prints the key the byte names, plus the slots carrying a `62 05` meta binding), and the header edits nothing: it has no select, no input and no button.

**Binding it is a meta action like any other.** A key bound to the `Meta` (`0x62`) group gets the Meta
action select, which lists Profile Switch F9 (`0x01`), Macro F10 (`0x02`), Brightness Down F11 (`0x03`),
Brightness Up F12 (`0x04`), **Lock Win** (`0x05`) and **Rapid Tap** (`0x0A`) - the same list on every key
of both layers, with no extra buttons anywhere in the panel. Lock Win is listed as the META toggle action
because picking it is what writes the byte, and the slot a stock read carries it on is LWin; Rapid Tap
toggles the feature the Rapid Tap panel edits (that block only exists in schema `b`). Neither reading is
measured in a write. Picking the `Meta` binding writes GG's factory shape
`62 01 01 00 00`; picking an action writes code 1 and keeps the other code bytes; picking **Meta
Toggle** (the list calls it **Lock Win**) writes `62 05 00 00 00` and moves `meta_toggle_hid` to this
key's code.

**The byte is the binding, and it shows as Meta - Meta Toggle.** The key the byte names is painted in
the meta colour with `META` in its cap on both layers, and its selects open on Binding `Meta` with the
Meta action `Lock Win (0x05)` - GG's own UI shows that key as Meta - Meta Toggle - whatever the entry's
own bytes say (a stock
read's slot 93 is `51 F0 00 00 00` and the byte `0xF0` names that key). The cap of usage `0x65` is named
**Menu** (`KEY_NAMES[0x65]`, the word GG prints on that cap), in the keybind lists, the tooltips and the
bindings table; `META` in a cap is the byte alone. Nothing is locked: a key bound
`62 05` on the META layer (slot 13 in a stock read, slots 13 and 35 in the user profile) opens the
normal binding panel - Binding, Meta action, Undo and factory reset - and the toggle action is offered on
the META layer as on the base layer. It is not offered on a wheel entry: GG switches a layer with a key,
not with a wheel mapping.

**The cap names the action.** A meta-bound key prints its action word in the cap, from `META_CAP_LABELS`:
`Profile` (0x01), `Macro` (0x02), `Bright down` (0x03), `Bright up` (0x04), `Lock Win` (0x05),
`Rapid Tap` (0x0A). The colour says meta, the word says which meta action.

Picking another binding, Undo and factory reset change binding bytes only; `meta_toggle_hid` is a field
of its own that Undo and factory reset never move, and a byte change is listed on its own in the
file-info diff. The field semantics behind these bytes stay reverse-engineered: only the factory shape is
measured in reads.

The **Meta cap** (`0x65`) has no slot under its own usage - the crate's `hid_to_offset(0x65)` is `None`
and the legacy table has `-1` there - and GG carries it on slot 93, the slot whose table code is GG's
`0xF0` (`Fn F0` in the page's usage table, `apex_7_profile::meta_key_offset`). So the cap is an
ordinary key in every panel: its binding select rebinds it (a stock read binds slot 93 to itself,
`51 F0 00 00 00`; the user profile binds it to `0x65`, the Context Menu keystroke), and binding it as
binding it as the toggle action writes `62 05 00 00 00` on slot 93 and sets the byte to `0xF0`.

## Consumer codes (media keys)

A mapping code is a **USB HID usage**, and which page it lives on follows the function byte:

| code | under `0x61` `media_key` (consumer page) | under a keyboard function (`0x51` combo) |
|------|----------------------------------------|----------------------------------------|
| `0xE2` | Media Mute | `LAlt` |
| `0xB5` | Media Next (scan next track) | - |
| `0xB6` | Media Prev (scan previous track) | - |
| `0xCD` | Media Play/Pause | - |
| `0xE9` | Media Vol+ (volume increment) | - |
| `0xEA` | Media Vol- (volume decrement) | - |

`MEDIA_NAMES` holds the consumer-page column and `keyName(code, functionByte)` picks it, so a
media key is named on the board (`Media Play` in its cap, `Media Vol+`/`Media Vol-` on the wheel
caps), in the code selects (which list the six as legal targets, `CONSUMER_TARGETS`) and in the
Custom bindings list; the Key panel prints the full table as a hint under a `0x61` entry. Names
are the standard consumer-page usages (USB HID Usage Tables §10), supplied by the operator for
GG's media cluster - **not read back from hardware**. The Rust decoder prints code bytes as hex.

## Key labels

Labels are sized for the cap they sit in (`.label.sm` shrinks for keys 1.6 units wide and
narrower; the rules are in `style.css` and the checks compare the label against the box):
print-screen is `PrtSc`, scroll lock `ScrLk`, num lock `NumLk`, pause `Pause`; the media
column uses two-letter forms (`Vol+`, `Prev`, `Play`, `Next`, `Stop`, `Eject`); the arrow
cluster is drawn as centred glyphs `U+2190..U+2193` (`↑` `→` `↓` `←`) instead of the words;
and the numpad - physically split from the main block, so there is nothing to disambiguate -
prints `7`, `Enter`, `+`, `*`, `/` with no `Ke` prefix. `Enter` fits the 1x keypad key.

The OLED is a display, so it is drawn on the grid: the rect spans the numpad's first three
columns (x 20-23) at the **top** row, the same row the wheel caps start, and it is bigger than
the wheel caps (4 x 1 keys, 244 x 57 px) so the 128 x 40 picture keeps its aspect. The wheel
caps start at row 0 (their caps align with `Pause`) and drop to row 4, the 5u column is 5 keys
in 4 rows, the arrow keys are 1u each and the arrow cluster is 4 x 2 keys, with no gaps - the
checks assert the geometry (`boardKeyList` and the rect), and nothing on the board is
per-key lighting.

## Cap colours

`FILL_COLORS` is the state palette. It is picked apart in **hue and lightness** so a board
can be read at a glance, and `selftest.mjs` enforces that: every colour (except `no-offset`)
sits at least 35 luminance points above the cap it fills, `no-offset` is the one colour that
recedes into the cap, the two neutral states are 20+ luminance points apart, macro (green)
and media (magenta) are 40 degrees of hue and 20 points of lightness apart, **no two**
colours are close in both hue and lightness, every fill stays dark enough for the light
`#eef3f9` legend to read, and the cap stroke is 40+ points above the cap fill. The legend
swatches in `style.css` must carry the same hex values as `FILL_COLORS` - the checks compare
the two lists, so the palette cannot drift between the board and the legend.

`preview.mjs` inlines `style.css` into the exported SVG for the same reason: the preview is
the page's stylesheet, not a copy of it.

## What you can edit

- **profile name, in the bar above the keyboard** (see [The name editor](#the-name-editor)),
- per key: a binding type and its option, on the base or META layer - `Deactivated`, `Mouse Key`
  (one of GG's mouse functions `0x01`-`0x08`, `0x30`-`0x34`), `Keyboard Key` (the four code bytes, as
  `Key 1`..`Key 4`), `Media Key` (the six consumer usages the page names), `Meta` (the action select of
  the `0x62` group - GG's F9-F12 actions, Lock Win, Rapid Tap, and the action that writes the META byte,
  see [The META toggle](#the-meta-toggle)), `Macro` (a record select); a function byte no group owns
  keeps its raw function and code fields visible;
  copy, paste (Shift-click a key), copy base -> META, apply one mapping to all mapped offsets,
- lighting block: the three colours, lock-key codes, META mask, OLED option
  bytes, Win key code, both dimmers, idle timeout,
- Rapid Tap enable byte and the ten pairs,
- the two wheel mappings of the layer being edited, in the key panel (click a wheel cap on the
  board: same binding select, same code selects, `wheel_mappings[n]` as the target),
- `meta_toggle_hid` - the META toggle binding, written by the `Lock Win (0x05)` action of a
  base-layer key's Meta action select (the Meta cap is bound through its slot like any other key).

Nothing in the file-info panel is editable: it has no dropdown and no text field.

`guid`, `guid_padding` and the `crc_*` fields are shown read-only; the `oled` block is dumped in
full (see [OLED bitmap](#oled-bitmap)), and the macro block is **decoded into records**, see
[Macro records](#macro-records). Saving writes the whole document, **including fields this editor
does not know about**; the change panel lists what differs from the copy that
was loaded, and diffing is what marks a key cap as edited. The Rapid Tap pairs print their
`reserved` bytes (the decoder lists 2 hex bytes per pair, the engine's `onboard_rapid_tap_pair`
layout) read-only, so a save writes them back unchanged.

## The record the page opens

The identity of the keyboard a dump came from is **four plain fields written first**:
`product_id`, `product`, `firmware`, `slot`. A read stamps all four; a file decode writes none of them,
so the file panel says `not recorded (decoded from a file, which names no keyboard)` and the
Firmware row says the same. There is no `device` object and no nested block - all SteelSeries
devices answer vendor ID `0x1038`, so the product ID identifies the model by itself, and a model
name is derivable from the number, which is why the page takes the name from the Rust registry
(see [Model names and feature sets](#model-names-and-feature-sets-imported-from-rust)) and prints
it *as the registry's answer*, not as the file's string. `slot` (the profile slot, 0-based) and
`source` (what the read wrote, naming the keyboard and the firmware) sit with them.

Every report carries `payload_len` (the length the decoder used for the file's own schema word:
11,648 for schema A, 11,696 for schema B - the page mirrors both constants and `PROFILE_REGION_BYTES
= 40,960`, mirroring `PAYLOAD_LEN_A` / `PAYLOAD_LEN_B` / `PROFILE_REGION_BYTES` in Rust). The read
fills the whole profile region, so when the image is longer than the payload the report also carries
`extra_len`, `extra_fnv1a_64` and `extra_head`: the region GG's write windows cover behind the
payload. The file panel then prints an **Extra region** row (offset, length, region hash, head bytes)
and the status line reports the split (`payload <n> + extra region <m>`); a payload-only report has
no such fields and shows no row. The bytes of the region are not part of the profile and this page
edits none of them.

## Checks

```sh
node tools/profile-gui/sync-products.mjs           # import the product table from Rust
node tools/profile-gui/sync-products.mjs --check   # fail if products.generated.js is stale
node tools/profile-gui/selftest.mjs    # 197 checks; 13 of them read the profile JSONs in the repo
                                       # root (the operator's captures, not files of the repo) and
                                       # report `skip` in a checkout that has none of them. Covers:
                                       # tables, binding, geometry, round-trip, edits, fake-DOM page
                                       # wiring, index.html script order, the picker path, the
                                       # one-profile rule, the save-a-copy prompt, the wheel caps,
                                       # the OLED rect (raster, bit order, hash fields), the label
                                       # rules, the binding and Meta action selects, the name bar,
                                       # the macro block's bytes and what a report without them says
node tools/profile-gui/preview.mjs [-o FILE] [--source FILE] [profile.json ...]   # -o defaults to board-preview.svg
```

`selftest.mjs` runs `app.js` against a minimal fake DOM (`fake-dom.mjs`), so the
same code the page runs is checked in CI-able plain node. It loads
`examples/sample-identity.json` (a **stock** wired Apex 7 slot, decoder `oled.bytes_hex` intact,
macro block all zero) and `examples/sample-remap.json` (base remaps
written in, meta colour edited); plus `sample-macros.json` in the repo root - a decode carrying
5 packed records and its macro block's 9,600 bytes as `macro_data.bytes_hex` - which is
the operator's file and not in the repo, so those checks skip without it.

The macro checks run on that root file: five records at indices 0, 7, 14, 21, 28 -
i.e. ordinals 0..4 with the measured stride of 7 - modes `once`/`repeat`, 4, 4, 4, 1 and 0 actions,
three `0x71` bindings on the base layer whose third code byte is the index, and the deleted-macro
case is produced in the suite by pointing a binding index past the list. The Rust side keeps its
own fixtures (`SYNTH_MACRO_BLOCK`, and `GG_MACRO_BYTES` - the record runs of that real block, the zero byte,
the `0x03` and a standing-in tail) for the codec tests. Paths are resolved against the script, so
`node tools/profile-gui/selftest.mjs` works from the repo root as well as from this
directory. The canvas the raster needs is faked in `fake-dom.mjs` (no 2D context) and in
the checks that want a bitmap, where a stub canvas returns a fixed data URL; `preview.mjs`
shims a real 24-bit BMP encoder so `board-preview.svg` carries the picture.

## OLED bitmap

The `oled` block is a **128 x 40 monochrome bitmap at 1 bit per pixel**: 8 pixels per byte,
16 bytes per row, 40 rows = 640 bytes = `OLED_LEN`, which is why the decoder dumps it whole
as `bytes_hex` instead of summarising it. **Bit 7 of each byte is its left pixel** - confirmed on an
Apex 7 (`0x1612`): the operator took the image GG paints on that panel, decoded it into bytes, read those bytes
back from the keyboard as the block, and writing them back paints the same picture. There is no bit-order
switch. `oledPixels` rasterises it (a set bit is a lit pixel) and
`oledDataUrl` makes a PNG through a canvas: the board rect draws it, the OLED panel shows it
at 2x with `image-rendering: pixelated`. A page without a 2D canvas keeps its text label and
says why; so does a dump that has no `bytes_hex`.

The conversion is **GUI-only**: the Rust codec treats the block as an opaque byte array and carries it
byte-for-byte (`decode` fills `bytes_hex`, `encode` writes it back untouched, so an on-board write sends exactly
the bytes a read produced). Reading the block as a bitmap - and writing a bitmap back into it - happens here and
nowhere else, so it is a display view, not a claim that the format is understood.

GG's own decoder states the same geometry (`oled_height = 8`, `oled_width = 128`, `bpp = 1`,
reading `pixel[row * 8 + col]` bit `7 - (col % 8)` for 40 rows), measured from the engine code
in `app_code/core.zip`: byte 0 bit 7 is the top-left pixel and byte 16 is the
first pixel of row 2. That is the sentence the panel, the rect tooltip, the exported SVG and
the decoder's JSON all print.

The panel row next to the rect lists the hash the dump records (`oled.fnv1a_64`) and the same
FNV-1a-64 recomputed over `bytes_hex`. In a dump this crate decoded the two are equal: the
decoder hashes the **block bytes** with the engine's **short prime** (43981) and the page
recomputes with that same variant. The published FNV-1a prime (0x100000001b3) gives a value that
agrees only in the low 32 bits, so the page pins the engine variant on both halves of that
difference. If a dump's stored field disagrees with its `bytes_hex` the panel says so in those
words. `Set all black` writes 640 zero bytes and recomputes `len`, `nonzero`, `fnv1a_64` and
`head`; only the JSON changes.

**Upload a BMP** replaces the block in the JSON. `decodeBmp` reads the header, accepts
`BI_RGB` at 1, 4, 8, 24 and 32 bpp (`BI_RGB` top-down too, and 32 bpp `BI_BITFIELDS` with the
plain blue/green/red masks), refuses `BI_RGB`-embedded JPEG/PNG, RLE and any other mask
layout, takes the nearest-neighbour average over a palette entry (1 and 4 bpp read the palette,
8 bpp reads it too, 24/32 bpp read the triple), and samples the picture to 128x40 by
nearest-neighbour - the same rule `python-pptx` documents for a picture stretched to fill a
slot, which is how GG puts a 1920x1080 image on this 128x40 panel. A pixel lights up at 128/255
brightness or more. `Set all black` clears the block, and `Invert every pixel` flips every bit of
`bytes_hex`: lit becomes off and off becomes lit, bit 7 stays the left pixel of its byte, so the
picture is mirrored in colour only and never moved, inverting twice is the identity, and a block with
no `bytes_hex` inverts to every pixel lit. `oled` is one of the four things the board can select: clicking the rect opens this panel in the column next to the board. Both actions
rewrite `bytes_hex` and recompute `nonzero`, `fnv1a_64` and `head` from it, and
`fileChanges` sees one changed path (`/oled/bytes_hex`), so the Changes panel lists it.
Nothing here touches a keyboard: the page opens configs read from an Apex 7 (`0x1612`), has no device access, and
writing to a board is `ssgg onboard write` on the CLI. Encode lives on the CLI side: `ssgg onboard encode <json>` writes a decode report
back into payload bytes (verified byte-identical over the 11,696-byte payload of three factory
reads). The page has no byte view and no encode control, so a
JSON saved here is exactly what the CLI encodes.

`fnv1a_64` is a summary field the decoder computes over the block's own bytes; `bytes_hex` is
the byte dump. The page shows both, recomputes the hash from the bytes, and calls out any dump
where they do not agree.

## Macro records

The JSON carries the block as a summary and as decoded records, and the page shows both.

Only the last five macros **written to keyboard** are currently decoded.
Existing bindings may reference macros outside of this scope, but there's
no GUI way to set new bindings to the macro unless it's known.
Decoding them is display support and partial support: the page can show a record
and bind a key to one (the binding byte it writes is `ordinal * 7`), it cannot compose, edit, reorder, delete or
name a record, and the encoder never serialises the list - a write sends the block's bytes back, so a list that
disagrees with the bytes has no effect on the keyboard.

`macro_data` is the summary: `offset`, `len`, `nonzero`, `fnv1a_64`, `head` - **and the block's own bytes as
`bytes_hex`** (9,600 of them: the engine declares this field an opaque `bytearray` of
`(macro-data-size)` = 9600 and parses nothing inside it, `lisp/apex_7.lisp:62, 101`). A **write sends those
bytes back verbatim**, so the page carries them through untouched: the file panel row names `bytes 9600` with
the block FNV, the Macro panel prints them and says plainly that a write sends them back unchanged. The
parsed list under them is a display view, and a partial one: a real profile's block walks as 5 records
(108 bytes) and stops at the `03` at `+109`, which is no marker we know, so its `macro_note` is printed as the
limit of the list, not of the profile. A report from a read that predates the block dump carries no `bytes_hex`: the
panel and the file panel row then say the write has nothing to send verbatim and name the re-read that fixes
it. There is **no length word** in the payload region behind the block - bytes 2020..2075 of the reference
profile are all zero - so nothing in the dump declares how long the record list is.

`macros[]` is the block **decoded**: one entry per record, in the order the list packs them (`index`
is that order), with `mode` + `mode_name`, `id`, `flags`, `interval_ms`, `record_bytes` (bytes this
record fills: header plus actions), `action_count`, the `actions` list (`kind`, `key` = the key's HID
code, `word`, `ms` - the page names the key through the embedded offset table, so HID `0x55` reads
`Ke *`), and `raw` - the record's own bytes, written verbatim, so a record can be compared with a
capture byte by byte. The page composes the key sequence from `actions` (a record with no actions
says `no keys`); the engine draws a random delay in `[ms, ms+19]`, which the read has never seen.

`Macro records` is a panel under the board. Its head names the span the records fill; per record it
prints the summary line, the interval, the key sequence, the byte and action counts, the action list,
the raw bytes, and **which key binds it** (`Bound by Meta+Ins`, or `Bound by no key`). Per `0x71` mapping the JSON carries `macro_index` (the record index the
third code byte names). The key panel's macro select is short on purpose: an option is
`Record 0 - repeat 5000 ms - 2 action(s) - index 0` (49 characters, no keystrokes; an index the
records do not cover reads `Index 14 - record 2, past the 2 record(s) (a macro GG deleted, binding
left on the key)`), and the whole
record is a **block under the select** - the mode, id, interval and action count, every keystroke
on its own line, the raw bytes, and the `Bound by` line. The bindings table prints the same in its
`reads as` column, and the cap sub line reads `macro 0 (record 0) - 4`.

Three states the panel spells out, because the read is the only place this data has ever been
decoded:

- a dump written before the decode has no `macros` field - the panel says so, quoting `macro_data`
  instead of showing nothing;
- a block that did not pack cleanly sets `macro_note` (the writer reports the bytes it could not
  place) and the panel carries that note;
- an empty block is reported as empty, not as an error.

A `0x71` index past the record list is **not** a bug in the read: the record is deleted, the
binding is left behind, and its index survives - the panel calls it `deleted macro: the record is
gone and the binding is left behind`.

The macro block stays reverse-engineered: one read has ever carried macros, and the only ground truth is
the macro GG shows for that keyboard (META+Insert, repeat every 5000 ms, pressing the `4` key).
`flags` and the mode values other than 1 (once) and 4 (repeat every interval) are open. The decode around it
is verified against clean and modified profiles.

## Model names and feature sets (imported from Rust)

The page shows the **model name** of the keyboard a dump names, and the feature set it can offer
that model. Both come from Rust, not from a table typed into the page:

```sh
node tools/profile-gui/sync-products.mjs           # writes products.generated.js
node tools/profile-gui/sync-products.mjs --check   # exit 1 when Rust moved and the file did not
```

`sync-products.mjs` parses these Rust regions - `pub mod product_ids`,
`device_name_from_product_id` and `device_type_from_product_id` in `src/devices/mod.rs`, the
`keyboard_class` family arms in `src/devices/discovery.rs`, and the
`codecs()` registry in `src/devices/keyboards/onboard.rs` - and writes `products.generated.js`,
which `index.html` loads **before** `app.js`, so the page reads it as `globalThis.SsggProducts`.
Run it after changing any of those tables; `selftest.mjs` runs `--check` as a check, so a stale
generated file fails the suite.

What the page derives from it:

- the model name shown in the file panel: **the registry's name for the product ID wins**, ahead of
  the `device.product` string the file carries (the crate's own rule - trust the number, not the
  name; a file naming an ID the registry does not list is shown by number, and its claimed name is
  used only when the registry is silent);
- the device kind (`keyboard` / `mouse` / `headset` / registry fallback), which decides whether a
  mouse button layout or the line `no keyboard layout: a headset has no key mappings` is printed;
- the on-board family set (the `KeyboardClass::Apex7` arms of `keyboard_class`): a model in it gets
  `payload layout: legacy Apex 750-byte A/B` plus the extra blocks this page decodes (128 mapping
  slots, scroll wheel, OLED, macros); a model outside it is labelled
  `payload layout: not the one this page encodes`, because a profile from it needs that family's own
  layout, encoder and feature set. That is why the product ID is in the JSON at all: the encodings
  and the feature set are per model.

`PRODUCT_NAMES` and `LEGACY_ONBOARD_PIDS` in `app.js` stay as the **fallback** for a page opened in
a checkout where the generator was never run, and the suite checks the fallback against the import
line by line, so the two cannot drift.

## The name editor

The 24-byte name field is edited in a bar **above the keyboard**, not in the file panel: the field,
an indicator, and a **Revert the name** button. The indicator says `as the file - <name_hex>` while
the name matches the file and turns amber with `changed - the file as loaded had "<name>" (<hex>)`
when it does not; the revert button is disabled until then, and writes back the name the file was
loaded with (`setProfileName` regenerates `name_hex` with it, so the two cannot disagree). Typing
updates the indicator and the document and leaves the rest of the page alone - repainting on every
keystroke would pull the caret out of the field - and the full repaint (board marks, Changes list)
comes on the field's `change` event. A name over the field's 24 printable ASCII bytes is refused
with that sentence in the status line and leaves the document alone. Saving writes the name with
the rest of the JSON.

## Limits

- The macro panel **reads** the record list; it edits nothing, and the list covers the known macros. `macro_data` carries the block's bytes
  (`bytes_hex`) and the page writes them back exactly as loaded - it never rebuilds a block from the parsed
  records, and `crc_*` is never recomputed (the encoder in the CLI computes it). A block the list cannot walk
  is normal here: its bytes are what travels.
- `oled` carries the block itself (`bytes_hex`), so the GUI draws it and can replace it in the
  JSON; the bit order is the operator-confirmed one, with no user switch.
- `schema` and `schema_name` are shown, not editable: the layout of the payload
  behind them is the decoder's business. `schema` is a **number** (the capture
  read writes `11`) and `schema_name` is the letter (`"b"`).
- Rapid Tap `mode` is the raw byte; the high bit is reported separately as
  `report_both_keys` (`MASK_MODE_REPORT_BOTH = 0x80`).
- Only schema `a`/`b` onboard dumps are understood. The 2023 model's actuation
  JSON (`ssgg actuation ...`) is a different shape and not handled here.
- The validator asks for `schema` as a number, `name` as a string, exactly
  128 + 2 + 128 + 2 mapping entries, and at most four key-code bytes per entry.
- A dump written by the **pre-fix decoder** (3 key-code bytes, the fourth byte of
  the 5-byte entry cut off) loads: the editor pads it with `00`, which is what GG
  writes there, and the header reports `Dump format: 3 key code(s) per mapping
  entry`. Re-read the slot to get the real bytes.
- `binding`, `offset` and `hid_code` are recomputed from the raw bytes on load and
  after every edit, so they can never disagree with `function` / `key_codes`.
