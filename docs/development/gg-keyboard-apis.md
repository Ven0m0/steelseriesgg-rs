# GG keyboard API catalogue, per protocol family

Reference for what SteelSeries GG itself sends to each keyboard family. Source: the device specification
files shipped in GG's engine, decrypted on 2026-11-10 (engine `SteelSeriesEngine.exe`, build `70d240b7`,
sha256 `F1C32B859BF8ADF95DDB14F9FCB1DA41FDF42EB3551DC945DBF13107C163C582`, 489 specification files).
`scripts/edevice_index.py` plus `scripts/gg_parity.py` produce the PID side of this document
([gg-parity.md](gg-parity.md)).

The specifications say what GG intends to write. Only a frame replayed from ssgg on real hardware is
**Confirmed** (`.claude/rules/experimental-protocol.md`); every frame below is therefore `[EXPERIMENTAL]`
unless the status column says otherwise. Source code still wins over this file:
`src/devices/hid_reports.rs` holds the command codes ssgg actually sends.

All devices are VID `0x1038`; PIDs below are 16-bit USB PIDs. Every application PID is followed by a
bootloader PID (`app + 1`) that only answers the firmware-update transport.

---

## Reading the specification format

The specifications are a small Lisp DSL. The interesting form is `api`:

```lisp
(api hall_thresholds
  (api-write HID
    (chunk HIDFEATURE
      (size (device-feature-report-size))
      hall_thresholds_payload)))
```

* `api-write` sends, `api-read` receives, `api-write-read` does both in one operation.
* `chunk` names the transport and the payload struct. The payload is usually the struct with the same
  name as the api; when the api body calls a constructor such as `make-rapid-tap-pairs-payload`, that
  constructor builds the bytes instead.
* Every `(constant 0x..)` field in a struct is a fixed byte. Fields without a constant are supplied by
  the caller and appear here as `<name>`.
* Structs marked `(unaligned)` are packed byte sequences with no padding.

### Transports

| Transport | Meaning |
|-----------|---------|
| `HIDIO` | Output report (interrupt OUT / `WriteFile`), report id from the device spec |
| `HIDIONOFAIL` | Output report, send failure tolerated (state pushes during driver mode) |
| `HIDFEATURE` | Feature report (`HID_REPORT_TYPE_FEATURE`, usually a `SET_FEATURE` ioctl) |
| `HIDFEATUREOUTINPUTIN` | Write as feature report, read the answer as an input report |
| `HIDCONFIG` | 12-byte configuration write, used by the legacy macro/region readers |
| `HIDSLEEP` | Delay between two chunks of one api |
| `SSFW`, `SSFWLEGACY`, `SSFWGENERIC`, `SSFWFIZZ` | Firmware-update protocols (see the last section) |
| `SSFWSTATUS` | Firmware-update progress query |
| `HOLTEKFW` | Holtek-based keyboard firmware update (Apex 100/150/M400/M500) |
| `READCONST` | Reads a constant the caller declared, no HID traffic |

### Wireless op encoding

Wireless base files define two helpers instead of literal opcodes:

```lisp
(define write-mask 0x00)
(define (write-op op)
  (binary-or (binary-or write-mask (device-identifier-mask)) op))
(define (non-tunneled-read-op op) ...)
```

`device-identifier-mask` is `0x40` in the dongle (`*_tx`) specifications and `0x00` in the keyboard
(`*_rx`) ones. The same api therefore emits two opcodes: `direct_write` is op `0x21` over a direct USB
connection and `0x61` (`0x40 | 0x21`) through the dongle. This matches ssgg's
`CommandCode::Apex2023DirectWireless = 0x61`; it is not a separate command.

---

## Gen 3 wired: `base_apex_2022_wired`

Feature-report size is set by the including device (`apex_pro_tkl_2022` 645 bytes, wireless base 642).
Report id `0`, input and output reports 65 bytes. The first payload byte after the report id is the
command. Lighting is a stream of fixed-size `direct_write` blocks, one per key, terminated by the length
field in the header.

| api | transport | frame |
|-----|-----------|-------|
| `direct_write` | `HIDFEATURE` | `00 40 <len> [hid r g b] * len` |
| `clear_direct_write` | `HIDIO` | `00 41` |
| `read_layout` | `HIDIO` | `00 F2 ...` |
| `read_region` | `HIDIO` | `00 F5 ...` |
| `write_region_id` | `HIDIO` | `00 75 ...` |
| `fw_version` | `HIDIO` | `00 90 00` -> `00 90 <err> <version string, 62 bytes>` |
| `meta_toggle_hid` | `HIDIO` | `00 35 ...` (wireless uses layer 0, one key: function `0x51`, keycode `0xF0`) |
| `read_button_mappings` | `HIDFEATURE` | mapping layer `0x00` |
| `read_button_second_actuation_mappings` | `HIDFEATURE` | mapping layer `0x02` |
| `read_macro_data` | mixed | `HIDCONFIG` writes plus reads |
| `read_onboard_profile_0`, `write_onboard_profile_0` | mixed | `0x03` chunks, `0xB1` header, CRC32 (`stm32-crc`) over the payload minus the CRC |
| `load_profile` | `HIDIO` | `00 B2 ...` |
| `set_profile_volatile` | `HIDIO` | `00 34 ...` |

`idle_timeout`, `idle_mode_dimmer` and `lock_key_color` are declared but empty in this base; the wired
boards get idle and lock-key handling through the profile blob instead.

### `apex_pro_tkl_2022` (`0x1628`, 645-byte feature reports)

The file of the board ssgg has the most hardware evidence for. It adds the analog-key family on command
byte `0x38` and the OLED pair on `0x1F`.

| api | transport | frame |
|-----|-----------|-------|
| `hall_thresholds` | `HIDFEATURE` | `00 38 61 <count> 00 [hid actuation release] * count` |
| `second_actuation_thresholds` | `HIDFEATURE` | `00 38 61 <count> 02 ...` (layer `02`) |
| `release_mode` | `HIDFEATURE` | `00 38 62 <count> ...` |
| `rapid_trigger_sensitivity` | `HIDFEATURE` | `00 38 65 <count> ...` |
| `rapid_tap_enable` | `HIDIO` | `00 38 66 <enabled>` |
| `rapid_tap_pairs` | `HIDIO` | `00 38 67 <pairs> [index hid1 hid2 mode] * pairs` (10 pairs maximum) |
| `oled_display` | `HIDFEATURE` | `00 38 83 00 <640 bytes>` |
| `oled_direct_write` | `HIDFEATURE` | `00 1F 81 <640 bytes>` |
| `oled_reset_direct_write` | `HIDIO` | `00 1F 82` |
| `fw_update` | `SSFWFIZZ` | pid `0x1628`, bootloader pid `0x1629`, fs id `0x01`, fs file `0x02` |
| onboard profile schema 6 and 8 | flashfs | read and write |

ssgg implements the `0x38 6x` group (`CommandCode::AnalogKeys`), `0x40`, `0x41`, `0x1F 81/82` and the
`0x83` read-only profile path. It does not implement `0xB2`, `0x34`, the profile writes, `0xF2`, `0xF5`,
`0x75`, `0x35`, button mappings or macros.

---

## Gen 3 wireless: `base_apex_2022_wireless`

39 apis, output reports 65 bytes, feature reports 642. Every opcode below is the pre-mask value; add
`0x40` when the frame goes to a dongle PID. The wireless base carries the configuration that the wired
base splits across separate commands: one `lighting_config` struct holds brightness, idle settings,
lock-key and meta colours, battery indicator and charging.

| api | op | notes |
|-----|----|-------|
| `direct_write` | `0x21` | `00 21 <len> [hid r g b] * len` |
| `clear_direct_write` | `0x22` | |
| `hall_thresholds` | `0x2F` layer `0x00` | |
| `second_actuation_thresholds` | `0x2F` layer `0x01` | |
| `release_mode` | `0x36` | |
| `protection_mode` | `0x14` | |
| `rapid_trigger_sensitivity` | `0x37` | |
| `rapid_tap_enable` | `0x17` | |
| `rapid_tap_pairs` | `0x18` | `00 <0x40 or 0x18> <pairs> [index hid1 hid2 mode] * pairs` |
| `lighting_config` | `0x20` | brightness, set_brightness, idle_brightness, set_idle_brightness, idle_timeout, set_idle_timeout, meta_brightness, set_meta_brightness, lock_key_colors, set_lock_key_colors, meta_key_colors, set_meta_key_colors, win_key_color, set_win_key_color, set_battery_indicator, battery_charging, set_battery_charging, meta_follow_global_brightness, set_meta_follow_global_brightness |
| `read_lighting_config` | `0xA0` | |
| `power_saving` | `0x29` | `high_efficiency_style = 1`, `high_efficiency_brightness = 1` |
| `read_power_saving` | `0xA9` | |
| `read_battery_status` | `0x92` | |
| `read_connection_status` | `0xBC` | non-tunneled read |
| `load_profile` | `0x13` | |
| `set_profile_volatile` | `0x28` | |
| `save_global_device_settings` | `0x11` | profile `0x05` |
| `read_layout` | `0xB4` | |
| `read_region` | `0x9C` | non-tunneled read |
| `write_region_id` | `0x1C` | |
| `read_button_mappings` | `0xAA` layer `0x00` | non-tunneled read |
| `read_button_second_actuation_mappings` | `0xAA` layer `0x03` | non-tunneled read |
| `fw_update` | `SSFWSTATUS` | progress and status only |
| onboard profile schemas 14, 16, 17, 19 | flashfs | includes the dongle-side variant |

ssgg implements only `0x61` (the dongle encoding of `0x21`). Nothing checks battery, connection state or
the lighting configuration on a wireless board.

---

## Gen 1: `base_apex_legacy`

41 apis, one shared protocol for Apex 7, Apex 7 TKL, Apex 5, Apex Pro, Apex Pro TKL and the 2024
refresh of those three. Report id `0`, output report 65, feature report 513 for the OLED frame.

| api | transport | frame |
|-----|-----------|-------|
| `fw_version_kb` | `HIDIO` | `00 90 00` |
| `fw_version_led` | `HIDIO` | `00 90 01` |
| `unique_id` | `HIDIO` | `00 FF` |
| `start_driver_mode` | `HIDIO` | `00 6C 00 01` |
| `end_driver_mode` | `HIDIO` | `00 6C 00 00` |
| `oled_display` | `HIDFEATURE` | `00 65 <640 bytes>` |
| `gamesense_oled_display` | `HIDFEATURE` | `00 61` |
| `oled_brightness` | `HIDIO` | `00 66 <level>` |
| `oled_screensaver_timeout` | `HIDIO` | `00 67 <u32>` |
| `brightness` | `HIDIO` | `00 24 <level>` |
| `direct_write` | `HIDFEATURE` | `00 3A <len, at most 125> [hid r g b] * 125` |
| `direct_write_raw` | `HIDFEATURE` | `00 3A <len> <500 bytes>` |
| `reset_all_illumination` | `HIDIO` | `00 3B` |
| `restart_graphics` | `HIDIO` | `00 6A` |
| `disable_gamesense_oled` | `HIDIO` | `00 69` |
| `idle_timeout` | `HIDIO` | `00 6D 00 00 00 <u32>` |
| `idle_mode_dimmer` | `HIDIO` | `00 42` |
| `enter_exit_idle` | `HIDIO` | `00 6E` |
| `load_profile` | `HIDIO` | `00 89 <1..5>` |
| `set_profile_volatile` | `HIDIO` | `00 41` |
| `meta_toggle_hid` | `HIDIO` | `00 60` |
| `layout` | `HIDIO` | `00 74` |
| `rapid_tap_enable` | `HIDIO` | `00 1A` |
| `rapid_tap_pairs` | `HIDIO` | see the specifications, pairs table |
| onboard read, write, deploy | `0x83`, `0x03`, `0x05`, `0x88`, `0xEB`, `0x89` | confirmed on Apex 7 (`0x1612`) on Linux, 2026-10-20 |
| `fw_update_full_flash` | `HIDCONFIG` / `HIDIO` / `HIDFEATURE` | multi-transport full-flash sequence |

ssgg has the lighting (`0x3A`), the driver-mode pair (`0x6C`) and the whole onboard file chain, and none
of `0x24`, `0x65`/`0x61`/`0x66`/`0x67`, `0x6D`/`0x42`/`0x6E`, `0x74`, `0x60`, `0x1A`, `0xF2`/`0xF5`/`0x75`,
button mappings or macros. The 2024 models (`0x1640`, `0x1650`, `0x1652`) add nothing beyond a firmware
transport override; `0x1640` is confirmed for `0x3A` on hardware (2026-11-02), which is also why the Gen 3
`0x4B` + `0x40` pair does nothing on it.

### `base_apex_pro` (Gen 1 analog keys, 71 keys)

An add-on base included by `apex_pro` (`0x1610`), `apex_pro_tkl` (`0x1614`) and `apex_pro_2024`
(`0x1640`). All frames are feature reports with report id `0` and the key count `0x47` in the payload:

| api | frame |
|-----|-------|
| `hall_thresholds` | `00 31 47 ...` |
| `second_actuation_thresholds` | `00 33 02 47 ...` (layer `0x02`) |
| `release_mode` | `00 32 47 ...` |
| `protection_mode` | `00 35 47 ...` |
| `rapid_trigger_sensitivity` | `00 34 47 ...` |
| `read_button_second_actuation_mappings` | feature-report read |

Note the clash: on the Gen 1 analog protocol, `0x35` is `protection_mode`, while ssgg's
`CommandCode::ActuationControl = 0x2D` and the Gen 3 `0x38 61` group are different protocols entirely.
ssgg implements none of `0x31` to `0x35` on Gen 1 boards; `ssgg actuation` is wired to `0x38` only.

---

## Apex 3 (`0x161A`) and Apex 3 TKL (`0x1622`)

Two unrelated protocols behind adjacent PIDs. Both are Holtek-class controllers with 33-byte output
reports.

### Apex 3, `0x161A`

| api | frame |
|-----|-------|
| `set_led_colors`, `write_all_illumination` | `00 0B 00` plus 10 RGB triples (33 bytes) |
| `led_color`, `z1_color` to `z5_color` | `00 05 00 <location> <r> <g> <b> <brightness = 100>` |
| `global_brightness` | `00 0A 00 <0..100>` |
| `color_effect` | `00 06 00 <effect 0..8> <speed>` |
| `save_to_flash` | `00 09 00` |
| `start_driver_mode` | `00 03 00 00 <profile_mode> <macro_mode = 0x01>` |
| `end_driver_mode` | `00 03 00 00 <profile_mode> 0x01` (the specification shows the same `0x01`) |
| `polling_rate` | `00 04 00` |
| `reset` | `00 01 00` |
| `fw_version` | `00 10 00` |
| `reset_all_illumination` | `HIDIO` |
| `fw_update` | `SSFW`, pid `0x161A`, bootloader pid `0x161B` |

`save_to_flash` (`00 09 00`) is the only save command on this board; there is no APPLY. ssgg's
`CommandCode::Apex3Lighting` sends `0x0B` and `0x0A` only.

### Apex 3 TKL, `0x1622`

| api | frame |
|-----|-------|
| `direct_write` | `00 21 <zone mask> <8 RGB triples>` |
| `write_all_illumination` | `00 21 FF <8 RGB triples>` |
| `reset_all_illumination` | `00 22 FF` |
| `global_brightness` | `00 23 <0..16>` |
| `global_brightness_read` | `00 A3 ...` -> `00 23 00 <level>` |
| `start_driver_mode` | `00 6C 00 01 01` |
| `end_driver_mode` | `00 6C 00 00 00` |
| `fw_version` | `00 90 00` -> `00 90 <15 bytes>` |
| `fw_update` | `SSFWFIZZ`, pid `0x1622`, bootloader pid `0x1623` |

ssgg maps `CommandCode::RgbControl = 0x21` and `CommandCode::Brightness = 0x22` onto this board but
treats `0x23` as a placeholder brightness command; the specification calls it `global_brightness` with a
`0..16` range, not `0..100`. Neither driver mode is sent, which is why the APPLY question in
`.claude/rules/experimental-protocol.md` (issues #173 and #257) is still open.

---

## Apex M750 (`0x0616`) and M650 (`0x0615`): packet protocol

Nine-byte header, then data. The header is `00 00 00 <size high> <size low> <class> <id>`, so the byte
ssgg reads as a command code is the low byte of a 16-bit data size when the payload is longer than 255
bytes. `custom_led_raw` shows this plainly: `00 00 00 01 8E 01 03 06 16` has `data_size = 0x018E = 398`,
`class = 0x01`, `id = 0x03`, six rows, 22 columns, then 396 bytes of RGB. `CommandCode::ApexMDirect = 0x8E`
is that size byte, not a command.

| api | frame |
|-----|-------|
| `led_effect` | `00 00 00 00 04 01 00 <effect> <direction> <speed> <mode>` |
| `back_light`, `write_all_illumination` | `00 00 00 00 05 01 01 <flags 1 or 0> 00 ...` |
| `brightness` | `00 00 00 00 03 01 02 01 00 <0..255>` |
| `custom_led` | `00 00 00 00 FF 01 03 06 16 <6 * 22 * 3 bytes>` |
| `custom_led_raw` | `00 00 00 01 8E 01 03 06 16 <6 * 22 * 3 bytes>` |
| `input_led_effect` | `00 00 00 00 03 01 04 ...` |
| `input_back_light` | `00 00 00 00 05 01 05 01 00 ...` |
| `input_brightness` | `00 00 00 00 02 01 06 01 ...` |
| `fw_version` | `00 00 00 00 03 00 81` -> `00 02 00 00 03 00 81 <high> <mid> <low>` |
| `polling_rate` | `00 00 00 00 01 00 04 ...` |
| `debounce_time` | `00 00 00 00 01 00 03 ...` |
| `region_layout_id` | `00 00 00 00 01 00 85 ...` |

The M650 (`0x0615`) shares the scheme with its own class and id table (`01 00` effect, `01 01` back light,
`01 02` brightness, `01 03` custom LED, `01 04` input effect, `01 05` input back light, `01 06` input
brightness, `00 81` firmware, `01 04` polling, `01 03` debounce) plus `single_led_change` with a 7-byte
size. ssgg sends a single solid colour to the M750 and has no M650 driver at all.

---

## Legacy multi-zone (`apex`, `0x1202`) and Holtek small boards

### `apex` (`0x1202`), also `0x1200`, `0x1204`, `0x1206`, `0x1208`, `0x120A`

| api | frame |
|-----|-------|
| `fw_version` | `00 10 00` |
| `region_layout_id` | `00 06 00 00` |
| `led_color`, `z1_color` to `z5_color` | `00 08 <zone 1..5> <r> <g> <b>` |
| `global_brightness` | `00 05 00 <level>` |
| `polling_rate` | `00 04 00` |
| `apex_mode` | `00 02 00` |
| `write_all_illumination` | `00 07 00 <5 zone brightness>` |
| `get_current_illumination_zones` | `00 17 00` |
| `save_to_flash` | `00 09 00` |
| `fw_update` | `SSFWLEGACY`, pid `0x1202`, bootloader pid `0x1203` |

`apex-raw` (`0x1200`) is a subset with brightness on `00 05 01`; `apex-350` (`0x1206`) and `apex-300`
(`0x1208`) include it, so they inherit the same opcodes.

### Holtek small boards

| Device | PIDs | Frames | Firmware |
|--------|------|--------|----------|
| `apex-m400` | `0x160C`/`0x160D` | `00 10 00` fw, `00 04 00` polling, `00 06 00 00` region, `00 07 00` pulsation, `00 05 00` brightness and illumination, `00 09 00` save, `00 01 00` reset | `SSFWGENERIC`, block size 512, size 255 |
| `apex-m500`, `eg-apex-m500` | `0x1607`/`0x1608`, `0x1609`/`0x160A` | include `apex-m400` | same |
| `apex-150` | `0x1616`/`0x1617` | `00 10 00` fw, `00 01 00` reset, `00 04 00` polling, `00 05 00` brightness, `00 06 00` colour effect, `00 09 00` save, `00 0A 00` per-zone colour, `00 0B 00` all colours | `HOLTEKFW` |
| `apex-100` | `0x160E`/`0x160F` | 12 apis of the same shape | `HOLTEKFW`, size 511 |
| `apex-mech` (Apex M800) | `0x1600`/`0x1601`, `0x1603` to `0x1606` | 56 apis of its own; not catalogued here | `SSFW`, pid `0x1202` |

ssgg has no driver for any of these PIDs, beyond the shared `product_ids` entry.

---

## Firmware-update transports

Five schemes, each tied to a hardware class. All of them are spec-only in ssgg: nothing sends firmware.

| Transport | Used by | Parameters in the specification |
|-----------|---------|---------------------------------|
| `SSFW` | Apex 3, Apex M800 | pid and bootloader pid |
| `SSFWLEGACY` | legacy multi-zone `0x1200`/`0x1202` | pid and bootloader pid |
| `SSFWGENERIC` | Apex M400/M500 | block size 512, transfer size 255 |
| `SSFWFIZZ` | Apex 3 TKL, Apex Pro TKL 2022, Apex Pro TKL 2024 | pid, bootloader pid, fs id, fs file |
| `HOLTEKFW` | Apex 100, Apex 150 | pid, bootloader pid, transfer size |
| full flash | Gen 1 Apex 5/7/Pro and 2024 refresh | `HIDCONFIG` plus `HIDIO` and `HIDFEATURE` phases |

`SSFWSTATUS` is the progress query the Gen 3 wireless base uses instead of a full-flash api. A firmware
update is out of scope for ssgg and stays that way until a hardware-confirmed sequence exists.

---

## ssgg coverage summary

| Family | ssgg status |
|--------|-------------|
| Gen 1 `base_apex_legacy` (`0x1610`, `0x1612`, `0x1614`, `0x1618`, `0x161C`, 2024 refresh) | Lighting `0x3A`, driver mode `0x6C`, onboard read/write/deploy confirmed on `0x1612`; brightness, OLED, idle, layout, meta, mappings, macros missing |
| Gen 1 analog `base_apex_pro` (`0x1610`, `0x1614`, `0x1640`) | Nothing; the `0x2D` guess was retired on `0x1628` and is not the Gen 1 protocol |
| Gen 3 wired `base_apex_2022_wired` (`0x1628` and siblings) | On `0x1628` (hardware-confirmed 2026-11-10/11): `0x40` lighting and per-key effects, `0x41`, `0x1F 81/82`, `38 83` on-board OLED image, `38 61` layer 0, `83 01` profile read, `36`/`B6` key mappings, `B7` macro read, `F2`/`F5`/`90` device info. `[EXPERIMENTAL]`, implemented but not replayed from ssgg: the gated profile write chain (`02 01`, `03 01` x25, `B1`, `B2`, `34`), `35`, `75`, `38 62/65/66/67`. Missing: macro write and event decoding, `37`/`3C` macro and meta-mapped data, wireless-side config. Siblings: see the capability table in the rules file |
| Gen 3 wireless `base_apex_2022_wireless` | Only the dongle form of `direct_write` (`0x61`) |
| Apex 3 `0x161A` | `0x0B`, `0x0A` only |
| Apex 3 TKL `0x1622` | `0x21`, `0x22`; `0x23` is a placeholder with the wrong range |
| Apex M750 `0x0616` | One solid colour through the `0x8E` size byte |
| Apex M650 `0x0615`, legacy multi-zone, Holtek small boards, M800 | No driver |

---

## Appendix: keyboard PID to family

| PID | GG name | Family |
|-----|---------|--------|
| `0x0615` | apex-m650 | M650 packet |
| `0x0616`, `0x0617`, `0x0619` | apex-m750, m750-tkl, pubg m750-tkl | M750 packet |
| `0x1200`/`0x1201` | apex-raw | legacy multi-zone |
| `0x1202`/`0x1203` | apex | legacy multi-zone |
| `0x1204`/`0x1205` | guildwars2-keyboard | legacy multi-zone |
| `0x1206`/`0x1207` | apex-350 | legacy multi-zone |
| `0x1208`/`0x1209` | apex-300 | legacy multi-zone |
| `0x120A`/`0x120B` | hp_omen_apex-350 | legacy multi-zone |
| `0x1600`/`0x1601` | apex-mech (Apex M800) | M800 |
| `0x1603`/`0x1604`, `0x1605`/`0x1606` | dota2-apex-m800 dongle and keyboard | M800 |
| `0x1607`/`0x1608` | apex-m500 | Holtek small |
| `0x1609`/`0x160A` | eg-apex-m500 | Holtek small |
| `0x160C`/`0x160D` | apex-m400 | Holtek small |
| `0x160E`/`0x160F` | apex-100 | Holtek small |
| `0x1610`/`0x1611` | apex_pro | Gen 1 plus Gen 1 analog |
| `0x1612`/`0x1613` | apex_7 | Gen 1 |
| `0x1614`/`0x1615` | apex_pro_tkl | Gen 1 plus Gen 1 analog |
| `0x1616`/`0x1617` | apex_150 | Holtek small |
| `0x1618`/`0x1619` | apex_7_tkl | Gen 1 |
| `0x161A`/`0x161B` | apex_3 | Apex 3 |
| `0x161C`/`0x161D` | apex_5 | Gen 1 |
| `0x161E`/`0x161F` | apex_pro_mini | Gen 3 wired |
| `0x1620`/`0x1621` | apex_9_mini | Gen 3 wired |
| `0x1622`/`0x1623` | apex_3_tkl | Apex 3 TKL |
| `0x1624`/`0x1625` | apex_pro_mini_wireless_dongle | Gen 3 wireless |
| `0x1626`/`0x1627` | apex_pro_mini_wireless_keyboard | Gen 3 wireless |
| `0x1628`/`0x1629` | apex_pro_tkl_2022 | Gen 3 wired |
| `0x1630`/`0x1631` | apex_pro_tkl_wireless_dongle | Gen 3 wireless |
| `0x1632`/`0x1633` | apex_pro_tkl_wireless_keyboard | Gen 3 wireless |
| `0x1634`/`0x1635` | apex_9_tkl | Gen 3 wired |
| `0x1640`/`0x1641` | apex_pro_2024 | Gen 1 plus Gen 1 analog |
| `0x1642`/`0x1643` | apex_pro_tkl_2024 | Gen 3 wired |
| `0x1644`/`0x1645` | apex_pro_tkl_wireless_2024_dongle | Gen 3 wireless |
| `0x1646`/`0x1647` | apex_pro_tkl_wireless_2024_keyboard | Gen 3 wireless |
| `0x1648`/`0x1649` | apex_pro_mini_2024 | Gen 3 wired |
| `0x1650`/`0x1651` | apex_5_2024 | Gen 1 |
| `0x1652`/`0x1653` | apex_7_2024 | Gen 1 |
| `0x1660`, `0x1662` (plus bootloaders `0x1661`, `0x1663`) | apex_pro_tkl_2024_cyber, apex_pro_tkl_2024_steel | Gen 3 wired |

Every keyboard PID in [gg-parity.md](gg-parity.md) that GG ships a specification for appears in this
table. MSI OEM boards (`0x1159`, `0x115A`, `0x115D`, `0x115E`, `0x202A`, `0x202B`, `0x2046`, `0x2056`,
`0x2062` and the rest) belong to the parity class `msi oem`, not `keyboard`, and are out of scope here.
