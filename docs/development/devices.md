# SteelSeries devices: product IDs and keyboard HID protocol

Research reference. Source code wins over this file: product IDs live in `product_ids` (`src/devices/mod.rs`),
command codes and report layouts in `src/devices/hid_reports.rs`, command status in
`.claude/rules/experimental-protocol.md`. The Apex Pro TKL 2023 wired (`0x1628`) wire protocol is in
[apex-pro-tkl-2023-gg-capture.md](apex-pro-tkl-2023-gg-capture.md).

All SteelSeries devices use VID `0x1038`.

---

## Product IDs from GG firmware

PIDs below come from the GG 111.0.0 firmware folder names, which encode `(VID << 16) | PID` (checked
2026-05-26). Only the low 16-bit USB PID is listed. A GG name is not proof that ssgg can drive the device.
Which PIDs ssgg registers, and the known naming problems (`ARCTIS_1` is GG's `arctis_7_2018_tx`,
`ARCTIS_NOVA_PRO` is the wireless TX dongle, `0x12CF` / `0x12E4` / `0x12EA` / `0x12EE` are not in GG
firmware), are commented in `product_ids`.

GG names PID `0x1628` `apex_pro_tkl_2022` regardless of the retail year.

### Keyboards

| PID | GG firmware name |
|-----|-----------------|
| 0x1200 | apex-raw |
| 0x1202 | apex |
| 0x1206 | apex-350 |
| 0x1208 | apex-300 |
| 0x1600 | apex_m800 |
| 0x1607 | apex_m500 |
| 0x160C | apex_m400 |
| 0x160E | apex_100 |
| 0x1610 | apex_pro |
| 0x1612 | apex_7 |
| 0x1614 | apex_pro_tkl |
| 0x1616 | apex_150 |
| 0x1618 | apex_7_tkl |
| 0x161A | apex_3 |
| 0x161C | apex_5 |
| 0x161E | apex_pro_mini |
| 0x1620 | apex_9_mini |
| 0x1622 | apex_3_tkl |
| 0x1624 | apex_pro_mini_wireless_dongle |
| 0x1626 | apex_pro_mini_wireless |
| 0x1628 | apex_pro_tkl_2022 |
| 0x1630 | apex_pro_tkl_wireless_dongle |
| 0x1632 | apex_pro_tkl_wireless |
| 0x1634 | apex_9_tkl |
| 0x1640 | apex_pro_2024 |
| 0x1642 | apex_pro_tkl_2024 |
| 0x1644 | apex_pro_tkl_wireless_2024_dongle |
| 0x1646 | apex_pro_tkl_wireless_2024 |
| 0x1648 | apex-pro-mini-2024 |
| 0x1650 | apex_5_2024 |
| 0x1652 | apex_7_2024 |

### Headsets

| PID | GG firmware name |
|-----|-----------------|
| 0x1240 | siberia_840 |
| 0x1250 | arctis_5 (2017) |
| 0x1252 | arctis_pro |
| 0x1260 | arctis_7_dongle |
| 0x1261 | arctis_7 (2017) |
| 0x1290 | arctis_pro_wireless |
| 0x1292 | arctis_pro_wireless_headset |
| 0x12AA | arctis_5_2018 |
| 0x12AD | arctis_7_2018_tx |
| 0x12AE | arctis_7_2018_rx |
| 0x12B1 | arctis_9x |
| 0x12B3 | arctis_1w_tx |
| 0x12B4 | arctis_1w_rx |
| 0x12B6 | arctis_1x_tx |
| 0x12B7 | arctis_1x_rx |
| 0x12C0 | arctis_9_rx |
| 0x12C2 | arctis_9_tx |
| 0x12CB | arctis_nova_pro |
| 0x12CD | arctis_nova_pro_xbox |
| 0x12D5 | arctis_7p_tx |
| 0x12D6 | arctis_7p_rx |
| 0x12D7 | arctis-7x-tx |
| 0x12D8 | arctis-7x-rx |
| 0x12E0 | arctis_nova_pro_wireless_tx |
| 0x12E2 | arctis_nova_pro_wireless_rx |
| 0x12E5 | arctis_nova_pro_wireless_xbox_tx |
| 0x12E8 | arctis_nova_pro_wireless_xbox_rx |
| 0x12EC | arctis_nova_3 |
| 0x12F0 | arctis_nova_4_rx |
| 0x12F2 | arctis_nova_4_tx |
| 0x12F4 | arctis_nova_4x_rx |
| 0x12F6 | arctis_nova_4x_tx |
| 0x12FA | arctis_nova_pro_v2 |
| 0x2200 | arctis_nova_7_rx |
| 0x2202 | arctis_nova_7_tx |
| 0x2204 | arctis_nova_7x_rx |
| 0x2206 | arctis_nova_7x_tx |
| 0x2208 | arctis_nova_7p_rx |
| 0x220A | arctis_nova_7p_tx |
| 0x220C | arctis_7_plus_rx |
| 0x220E | arctis_7_plus_tx |
| 0x2210 | arctis_7p_plus_rx |
| 0x2212 | arctis_7p_plus_tx |
| 0x2214 | arctis_7x_plus_rx |
| 0x2216 | arctis_7x_plus_tx |
| 0x2230 | arctis_nova_5_rx |
| 0x2232 | arctis_nova_5_tx |
| 0x2238 | arctis_nova_7_diablo_iv_rx |
| 0x2240 | arctis_nova_pro_wireless_v2_rx |
| 0x2244 | arctis_nova_elite_tx |
| 0x2249 | arctis_nova_elite_rx |
| 0x2251 | arctis_nova_5x_rx |
| 0x2253 | arctis_nova_5x_tx |
| 0x2267 | arctis_nova_3_wireless_rx |
| 0x2269 | arctis_nova_3_wireless_tx |
| 0x227C | arctis_nova_7_gen2_rx |
| 0x227E | arctis_nova_7_gen2_tx |
| 0x2288 | arctis_nova_7p_gen2_rx |
| 0x2290 | arctis_nova_pro_omni_tx |
| 0x2296 | arctis_nova_pro_omni_rx |
| 0x2298 | arctis_nova_7p_gen2_tx |
| 0x229C | arctis_nova_7x_gen2_rx |
| 0x229E | arctis_nova_7x_gen2_tx |
| 0x230A | arctis_gamebuds_dongle |
| 0x230C | arctis_gamebuds_case |

### Arctis Nova 7 family protocol [EXPERIMENTAL]

Source: two community forks. Code: `src/devices/headsets/nova7.rs` (per-PID table `NOVA7_MODELS`, confirmed PIDs
in `HARDWARE_CONFIRMED`) and `HeadsetSettingCommand` / `HeadsetStatusQueryCommand` in `src/devices/hid_reports.rs`.
Every command is Experimental on every PID except the one below (see `.claude/rules/experimental-protocol.md`).

Hardware results, Arctis Nova 7 Gen 2 `0x227E`, Linux, 2026-10-25 (owner test, GG not running):

- Detection: control collection is interface 3 (usage page `0xFFC0`), picked automatically; the serial is empty.
- Status `00 b0`: percent battery matched an independent reading; the charge byte flipped to charging on USB
  plug-in; ChatMix read -1.00 / +0.02 / +1.00 with the dial at full game / middle / full chat.
- Sidetone `00 39`: step 3 audible, step 0 silent. After a power cycle the headset falls back to its own saved
  level, because the save frame is never sent. Steps 1-2 untested.
- Mic volume `00 37`: recorded mic level -80 dBFS at step 0 and -39 dBFS at step 7 (alternating takes through
  PipeWire). Sidetone does not follow the mic volume. Middle steps untested.
- Auto-off `00 a3`: `01` turned the headset off after about a minute idle, so the value is minutes. `00`
  (disable) was sent but not observed.

- Transport: 64-byte output reports `00 <opcode> <arg>` zero padded (byte 0 is the hidapi report number), on the
  vendor control collection (usage page `0xFFC0`, interface 3 in the forks). `control_score` already ranks that
  usage page first, so no Nova 7 specific selection is needed.
- Commands: sidetone `00 39 <0..=3>`, microphone volume `00 37 <0..=7>`, auto-off `00 a3 <minutes>`,
  status query `00 b0`. The save frame `00 09` persists to the headset and is never sent.
- Status reply (no report ID): `b0 <power> <battery> <charge> <chat game> <chat chat>`. Connected only when
  power is 3 and charge is not 0. Charge 1 or 2 is charging, 3 is not. Battery is 0-100 on percent models and 0-4
  steps (x25) on the 4-step models. ChatMix bytes are 0-100 each.
- No EQ preset or mic mute opcode is known; `ssgg` returns an error for both without sending anything.
- 0x2200, 0x2204 and 0x2208 (the RX dongles) are not in the table and stay on `GenericHeadset`.
- 7P variants (0x220A, 0x2298, 0x22A7) have neither sidetone nor ChatMix. Variants with the 4-step battery:
  0x2202, 0x2206, 0x220A, 0x223A, 0x227A, 0x22A4. Everything else reports percent.
- Arctis Nova Pro Omni (0x2290) has no known protocol and is detect-only.

### Mice: Modern family [EXPERIMENTAL]

Source: a community fork derived from rivalcfg (data licensed WTFPL/GPL, reimplemented here from the facts, with
our own test vectors). Nothing here has been run from `ssgg` on hardware, so every command is Experimental (see
`.claude/rules/experimental-protocol.md`). Code: `src/devices/mice/` (`modern.rs` holds the per-PID table
`MOUSE_MODELS`, `dpi_tables.rs` the sensor code tables) and the `Mouse*Command` types in
`src/devices/hid_reports.rs`. CLI: `ssgg mouse status | dpi <value> | poll-rate <hz> | color <name|hex>`.

Supported (24 PIDs): Aerox 3 `0x1836`; Aerox 3 Wireless `0x1838` (2.4 GHz), `0x183A`, `0x1878`, `0x187A`;
Aerox 5 `0x1850`; Aerox 5 Wireless `0x1852`, `0x1854`, `0x185C`, `0x185E`, `0x1860`, `0x1862`; Aerox 9 Wireless
`0x1858`, `0x185A`, `0x1874`, `0x1876`; Prime Mini `0x184D`; Prime Mini Wireless `0x1848`, `0x184A`; Prime
Wireless `0x1840`, `0x1842`; Rival 3 Gen 2 `0x1870`; Rival 5 `0x183C`, `0x183E`. Every PID in the requested
scope belongs to this family in the fork; none was dropped.

- Transport: output reports `00 <opcode> <args>`, variable length and not padded by `ssgg` (hidapi pads on
  Windows). Commands go to one interface per model: 3 for everything but the Rival 5, which uses 0. A 2.4 GHz
  receiver PID sets bit `0x40` on every opcode and answers each setting with a 64-byte report that is read and
  dropped. Commands are spaced 50 ms apart.
- DPI: `00 2D 01 <preset> <code>`; the Rival 3 Gen 2 uses opcode `34` and sends the code once per axis. Preset
  index is 1 on Aerox 3 and Rival 3 Gen 2, 0 elsewhere. Ranges: 200-8500 (Aerox 3, Rival 3 Gen 2, TrueMove Core
  table), 100-18000 in steps of 100 (all others, TrueMove Air table), Prime Mini 50-18000 in steps of 50 with a
  16-bit little-endian code `dpi / 50`. Off-step and out-of-range values are rejected.
- Polling rate: `00 2B <code>`; 125/250/500/1000 Hz is `04 03 02 01` on wired-only models and Rival 5 / Prime
  Mini, `03 02 01 00` on the wireless models.
- Colour: `00 21 <selector> ... <r g b>` once per LED zone. Aerox 3/5 and Rival 3 Gen 2 (3 zones): selector is a
  mask byte with the zone bit, then 3 zero bytes per earlier zone. Rival 5 (10 zones): 16-bit little-endian mask,
  same padding. Wireless Aerox and Prime Wireless: `01 <zone>`. Prime Mini: `00`. A receiver PID sends `61`.
- Battery: `00 92` (`D2` on a receiver PID) on wireless models; reply byte 1 is bit 7 charging, low bits charge
  in 5 percent steps plus one. Levels outside 0-100 decode as unknown.
- Never implemented: the save command `11 00` (writes to the mouse's on-board memory). `ssgg` sends no
  save, so whether a setting survives a power cycle is unknown.

Skipped on purpose: reactive and rainbow effects, button mapping, multi-stage sensitivity presets, brightness,
sleep and dim timers, firmware queries. Not covered, other command sets: Prime and Prime+ (`0x182E`, `0x182C`,
`0x182A`, `0x1856`), Rival 3 / Rival 3 Wireless, the older Rival (95-710), Sensei (310, TEN, [RAW]), Kana v2 and
Kinzu v2.

---

## Keyboard HID protocol

Two sets of commands have been replayed on hardware from this project:

- the `0x40` lighting frame on the wired Apex Pro TKL 2023 (`0x1628`), one solid colour, 2026-10-14 (details
  in the capture note); it is still `[EXPERIMENTAL]`;
- the legacy Apex on-board profile frames on an Apex 7 (`0x1612`) under Linux, 2026-10-20 - `90` firmware
  poll, `85` pull, `83` payload read, `88` file open, `03 00` resource-buffer write, `05` push, `EB` commit,
  `89` profile deploy. These are **confirmed** on that model.

Every other code below is untested here.

### Control endpoint

Discovery picks the control collection by usage page (`control_score`, `src/devices/discovery.rs`):
`0xFFC0` first, then any other vendor page (`>= 0xFF00`), then a per-PID interface fallback. On `0x1628`
the control collection is interface 1, usage page `0xFFC0`, output report 65 bytes, feature report 645
bytes (measured on Windows 2026-08-18; `STEELSERIES_CONTROL_USAGE_PAGE` in `src/devices/mod.rs`).
Interface 4 is vendor page `0xFFC1`, input only. The full 10-collection list is the
`apex_pro_tkl_2023_table()` test fixture in `discovery.rs`. Other models are unverified. `[EXPERIMENTAL]` The Apex M750 (`0x0616`) exposes many interfaces and only interface 2 is used (fork, untested): `control_score` accepts no other interface for it, preferring its vendor-defined collection.

### Report format

Keyboard output reports are 65 bytes: report ID `0x00`, command byte, data, zero padding
(`KEYBOARD_REPORT_SIZE`). Headset reports are 64 bytes without a report ID. Lighting frames for the
2023/2024 Apex Pro TKL family are feature reports (645 bytes on `0x1628`, 643 on the other models).

### Command codes

| Code | Name in code | Status | Notes |
|------|--------------|--------|-------|
| `0x09` | `Apply` | `[EXPERIMENTAL]` | `[00 09 00 ..]`. Code comments say it makes the 2023 controller fall back to its stored profile, so `ApexProTkl2023::apply()` skips it in direct mode. |
| `0x21` | `RgbControl` | `[EXPERIMENTAL]` | `[00 21 zone R G B ..]`, zone `0xFF` = all. Does nothing on the Apex 3 TKL (issues #173, #257). Not used on `0x1628`: zone colours go through the `0x40` frame (first colour only). |
| `0x22` | `Brightness` | `[EXPERIMENTAL]` | `[00 22 level ..]`, level 0-100. Not sent to the Apex 3 TKL (OpenRGB's 8-zone dialect reads `0x22` as rainbow wave). GG scales RGB in software on `0x1628`. |
| `0x23` | `PerKeyRgb` | `[PLACEHOLDER]` | Real per-key command for non-2023 models is unknown. |
| `0x23` | `Apex8ZoneBrightnessCommand` | `[EXPERIMENTAL]` | Apex 3 TKL brightness `[00 23 level ..]`, level `0x00`-`0x10` scaled from 0-100 (OpenRGB `SteelSeriesApex8ZoneController`). Sent by `rgb brightness` on `0x1622`. |
| `0x25`, `0x26` | `ReactiveMode`, `ColorShift` | `[EXPERIMENTAL]` | Enum values only; the uncalled `Apex3Tkl` helpers (and the `0x24` OSD helper) were removed on 2026-10-15. |
| `0x2D` | `ActuationControl` | `[EXPERIMENTAL]` | `[00 2D value ..]`, value 1-40 = 0.1-4.0 mm (`ActuationCommand`). Sent by `ssgg actuation`. GG does not use it; it writes actuation through the on-board profile blob (capture note). |
| `0x40` | `Apex2023Direct` | `[EXPERIMENTAL]` | Lighting frame `40 54 [usage R G B] x84`, feature report. Used for `rgb color` and `rgb perkey` on `0x1628`. See the capture note. The Apex 9 TKL `0x1634` and Apex 9 Mini `0x1620` take the same frame in a 513-byte report (127 entries fit, 112 are sent, count byte `0x70`) with no `0x4B` init, `[EXPERIMENTAL]`, community fork, untested. |
| `0x3A` | `ApexGen1Direct` | `[EXPERIMENTAL]` | Original Apex Pro `0x1610` direct frame: 643-byte feature report `00 3A N [usage R G B] xN` on interface 1, no init (OpenRGB `SteelSeriesApexController`, Gen 1). Untested here (issue #283). The same frame is routed to Apex 5 `0x161C`, Apex 7 TKL `0x1618`, Apex Pro TKL `0x1614` and Apex Pro (2024) `0x1640` from a community fork's table: unverified, never run on hardware. |
| `0x8E` | `ApexMDirect` | `[EXPERIMENTAL]` | Apex M750 `0x0616` direct frame, 513-byte feature report on interface 2: header `00 00 00 01 8E 01 03 06 16`, then 132 `R G B` slots from index 9. The slot-to-key map is unknown, so `ssgg` fills every slot (solid colour and off only). Community fork, untested (issue #276). |
| `0x01` | `ApexMEnable` | `[EXPERIMENTAL]` | Apex M750 enable sequence, three 513-byte feature reports sent once before the first frame, all zero but (index 0 is the report ID): `[4]=01 [6]=85`; `[4]=03 [5]=01 [7]=FF`; `[4]=01 [6]=85 [7]=FF`. `0x01` is only byte 4 of the first and third; the three share no leading byte. Community fork, untested. |
| `0x4B` | `CommandCode::Apex2023Init` (`DirectInitCommand`) | `[EXPERIMENTAL]` | Init feature report from OpenRGB, sent before direct frames on 2023/2024 models except `0x1628` (GG does not send it). |
| `0x61` | `CommandCode::Apex2023DirectWireless` (`DirectFrameCommand`) | `[EXPERIMENTAL]` | Direct frame ID for the wireless models. A code comment reports it working on Gen 3 wireless (`0x1644`/`0x1646`); no test record in this repo. |
| `0x83` | `OnboardBufferRead` | confirmed (`0x1612`) | `83 \| 00 \| buffer offset u16 \| size u16` feature report, answered by the 642-byte `GET_REPORT` read-back. Reads a profile slot (`ssgg onboard read`). |
| `0x85` | `OnboardPayloadPull` | confirmed (`0x1612`) | `85 \| 00 \| file id u16 \| file offset u32 \| size u16`: file into the resource buffer. Answered only by the keyboard's ordinary input report (flow control). |
| `0x88` | `OnboardFileOpen` | confirmed (`0x1612`) | `88 \| 00 \| file id u16`: open the profile file a write is about to push, answered `88 01 …`. |
| `0x03` (legacy header `03 00`) | `OnboardBufferWriteCommand` | confirmed (`0x1612`) | `03 \| 00 \| buffer offset u16 \| size u16 \| data` in the 642-wire-byte feature report: half of one 1024-byte write window. No reply of its own. |
| `0x05` | `OnboardPayloadPush` | confirmed (`0x1612`) | `05 \| 00 \| file id u16 \| file offset u32 \| size u16`: buffer into the profile file, answered `05 01 …`. |
| `0xEB` | `OnboardFileCommit` | confirmed (`0x1612`) | `eb \| 1-based profile id`: commit the slot after the last push; no write-specific answer. |
| `0x89` | `OnboardProfileDeploy` | confirmed (`0x1612`) | `89 \| 1-based profile id`: GG's `load_profile`; the keyboard runs the slot named. Send it 250 ms after a commit - right after `eb` it can do nothing. |
| `0x90` | `OnboardFirmwareVersion` | confirmed (`0x1612`) | `90 \| source` (`00` keyboard, `01` LED controller), answered with the ASCII firmware version on the interrupt endpoint. |
| `0x6C` | `OnboardDriverMode` | `[EXPERIMENTAL]` | `6c 00 01` on GG's device open, `6c 00 00` on session close. Nothing in `ssgg` sends it. |

No actuation read-back command is known. `src/bin/discover_actuation.rs` probed `0x00`-`0xFF` on
2026-05-27 without a reply. GG may read the profile back at startup (open, `PLAN.md` P2).

### Zones

Zone counts come from `zone_count_for_product_id` (`src/devices/mod.rs`) and layouts from
`src/devices/zone_mapping.rs`. Models without a measured layout return 1 zone. The Apex 3 TKL (`0x1622`) has 8 zones, as
SteelSeries advertises and OpenRGB's `SteelSeriesApex8ZoneController` models (changed from 9 on 2026-10-15;
the zone names in `zone_mapping.rs` are not measured).

OpenRGB's 8-zone dialect, an unverified lead for the Apex 3 TKL: 65-byte report; `0x21` with byte 1 = LED
bitmask (`0xFF` = all 8) and `R G B` x8; `0x22` with `0xFF` = rainbow wave; `0x23` with `0x00`-`0x10` =
brightness.

### Key addressing

Per-key frames use USB HID keyboard usage IDs (page 7), not a row/column matrix. SteelSeries uses `0xF0`
(240) for the FN / SteelSeries key. GG's 84-usage table for `0x1628` is in the capture note and
`WIRED_2023_KEYS`; GG's key coordinates are in [database-schemas.md](database-schemas.md) (`zone_cache`).

---

## Open questions

- Per-key RGB command for keyboards other than the 2023/2024 Apex Pro TKL family.
- Real zone counts for `APEX_PRO`, `APEX_PRO_TKL`, `APEX_5`, `APEX_7`, `APEX_7_TKL` (all return 1).
- Whether the 8-zone `0x21` frame and `0x23` brightness make the Apex 3 TKL show colours (`PLAN.md` P3).
- `key_mapping.rs` maps `Menu` (HID 101) for `0x1628`; GG's `zone_cache` has no such key.
- Whether wireless Arctis models use the same HID protocol through the USB dongle.

## References

- [GameSense SDK](https://github.com/SteelSeries/gamesense-sdk): official high-level API
- [OpenRGB SteelSeriesApex8ZoneController](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/SteelSeriesController/SteelSeriesApex8ZoneController/SteelSeriesApex8ZoneController.h)
- [apex-tux](https://github.com/not-jan/apex-tux): Rust OLED support for Apex keyboards
- [apex7tkl_linux](https://github.com/FrankGrimm/apex7tkl_linux): Python RGB and OLED for Apex 7 TKL
- [apexctl](https://github.com/AstroSnail/apexctl): C tool using hidapi-hidraw
- [msi-perkeyrgb](https://github.com/Askannz/msi-perkeyrgb): per-key protocol notes for SteelSeries keyboards in MSI laptops
- [USB HID 1.11 Specification](https://www.usb.org/sites/default/files/documents/hid1_11.pdf); keyboard usage
  IDs are in the separate HID Usage Tables document (page 7)
