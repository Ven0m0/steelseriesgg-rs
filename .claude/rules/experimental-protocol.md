# Experimental Protocol Rules

Protocol work has three states. Never mix them up or upgrade a status without hardware evidence.

## Status table

| Status | Meaning | Doc label |
|--------|---------|-----------|
| Confirmed | Tested on real hardware | (none) |
| Experimental | Reverse-engineered, untested | `[EXPERIMENTAL]` |
| Placeholder | Code exists, actual command unknown | `[PLACEHOLDER]` |

## Current command code status

| Constant | Code | Status |
|----------|------|--------|
| `CommandCode::PerKeyRgb` | `0x23` | Placeholder — actual per-key command unknown |
| `CommandCode::Apex2023Direct` | `0x40` | Experimental — one solid-colour frame replayed on `0x1628` (2026-10-14); per-key frame confirmed on hardware 2026-10-21 (lit one key; at that time unlisted keys were sent black because a frame replaces all keys, and a later per-key request now merges onto the last frame ssgg saved, `rgb perkey set-key W red` lit W with other keys unaffected, 2026-10-21); software brightness scaling of the frame (replaces the `00 22` report on direct models) and other colours and Linux untested |
| `CommandCode::Apex2023DirectWireless` / `DirectFrameCommand` | `0x61` | Experimental — wireless Apex Pro TKL models (`0x1630`, `0x1632`, `0x1644`, `0x1646`); a code comment reports it working on Gen 3 wireless, no test record in this repo |
| `CommandCode::Apex2023Init` / `DirectInitCommand` | `0x4B` | Experimental — Gen 3 init feature report from OpenRGB, sent once before the first direct frame; not sent to `0x1628` (GG does not) |
| `CommandCode::ApexGen1Direct` / `DirectFrameCommand` | `0x3A` | Experimental — original Apex Pro `0x1610`, from OpenRGB, untested |
| Apex 3 TKL `0x1622`: no APPLY (`0x09`) from `initialize()` / `apply()` | `0x09` (not sent) | Experimental — OpenRGB's 8-zone controller never sends it; suspected cause of issues #173/#257, untested on hardware |
| `CommandCode::Apex2023Direct` on Apex 9 TKL `0x1634` / Apex 9 Mini `0x1620` (513-byte report, no `0x4B` init) | `0x40` | Experimental - community-fork table, untested on hardware |
| `CommandCode::ApexGen1Direct` on Apex 5 `0x161C`, Apex 7 TKL `0x1618`, Apex Pro TKL `0x1614`, Apex Pro (2024) `0x1640` | `0x3A` | Experimental - community-fork table, untested on hardware |
| `CommandCode::ApexMDirect` / `ApexMDirectCommand` (Apex M750 `0x0616`, 513-byte feature report on interface 2) | `0x8E` (byte 4 of header `00 00 00 01 8E 01 03 06 16`) | Experimental - community fork, untested; solid colour only, the slot-to-key map is unknown |
| `CommandCode::ApexMEnable` / `ApexMEnableCommand` (Apex M750 three-report enable sequence) | `0x01` (byte 4 of reports 1 and 3 only; no shared leading byte) | Experimental - community fork, untested |
| `CommandCode::Apex3Lighting` (tag `0xF5`) / `Apex3Command`, Apex 3 `0x161A`: `00 0B 00 <RGB x10>` colours, `00 0A 00 <0..=100>` brightness, 33-byte output reports | `0x0B`, `0x0A` | Experimental - GG spec `apex_3`, 2026-10-25; class `Apex3`, never APPLY (`00 09 00` is `save_to_flash` there), GG's driver-mode frame `00 03 00 00 01` not sent; untested |
| Firmware-picked protocol on Apex Pro TKL `0x1614`, Apex 9 TKL `0x1634`, Apex 9 Mini `0x1620` (`90 00` poll, 1.19.7+ = Gen 3: `0x4B` init, and `0x40` instead of `0x3A` on `0x1614`) | `0x90`, `0x4B`, `0x40` | Experimental - OpenRGB `SteelSeriesApexController::SendInitialization`, 2026-10-25; a failed poll keeps the pre-probe protocol; untested |
| Legacy keyboard output path (`GenericKeyboard::send_report*`) | all `0x21`/`0x22`/`0x09`/`0x2D`/... | Fixed 2026-10-25: frames go out as built (`00 <cmd>`), not `00 00 <cmd>`. Every earlier hardware result through this path came from shifted frames |
| `Apex8ZoneBrightnessCommand` | `0x23` | Experimental — Apex 3 TKL brightness, from OpenRGB, untested |
| `CommandCode::ProfileChunk` / `ProfileChunkCommand` (Gen3 header `03 01 80`) | `0x03` | Experimental — `0x1628` profile upload chunk; checksum matches captures; never sent (no read-back) |
| `CommandCode::ActuationControl` | `0x2D` | Experimental — unverified guess, now sent only to models other than `0x1628` (which gets `AnalogKeyCommand`, below); confirmed no-op on `0x1628` wired (owner test, 2026-10-21): no feel or GG-visible change. Live HID trace the same day showed GG itself never queries or sets actuation through any HID command on this model — it rewrites the on-board profile instead (see `docs/development/apex-pro-tkl-2023-gg-capture.md`, "Live HID trace and engine memory read") |
| `CommandCode::AnalogKeys` / `AnalogKeyCommand`, Apex Pro TKL 2023 `0x1628`: `00 38 61` hall thresholds (layer `00`/`02`), `38 62` release mode, `38 65` rapid-trigger sensitivity (feature reports, 645 bytes, all 68 analog keys), `38 66` rapid-tap enable, `38 67` rapid-tap pairs (output reports) | `0x38` | Experimental - read from GG's decrypted device spec (`apex_pro_tkl_2022`), 2026-10-24; implemented 2026-10-25 (`ssgg actuation ...`; `set-key` merges per-key changes onto the last frame ssgg sent, since every frame carries all 68 keys). `38 61` layer `00` confirmed on hardware 2026-10-25 (Windows, fw 1.19.7): `set 0.1` turned a 4.0 mm board hair-trigger; GG sends byte-identical `38 61`/`38 62`/`38 66` frames (Frida), and the board's OLED sync reports (`0f h l`) match GG's 40-step mm table. Other distances, layer `02`, `set-key`, `62`/`65`/`67` from ssgg, Linux and persistence across power cycles untested. Supersedes the `0x2D` guess on this model |
| `CommandCode::Gen3ProfileRead` (tag `0xF4`) / `Gen3ProfileReadCommand`: `00 83 01 <fs-id = 0x80 + slot> <size u16 LE> <offset u32 LE>`, 500-byte chunks, reply data behind a 3-byte header | `0x83` | Experimental - read from GG's decrypted device spec (`base_apex_2022_wired`), 2026-10-24; implemented 2026-10-25 (`ssgg onboard gen3-read`), read-only, never run on hardware. Byte order matches GG's captured `03 01 80` write chunks. Differs from the legacy `OnboardBufferReadCommand` header, hence the tag |
| `CommandCode::Apex2023DirectClear` / `Apex2022DisplayCommand::ClearDirect` (`00 41`, output report) | `0x41` | Experimental - GG spec `clear_direct_write` (`base_apex_2022_wired`); `ssgg rgb release`, `0x1628` only, untested |
| `CommandCode::Apex2023Oled` / `Apex2022DisplayCommand` (`00 1F 81 <640 bytes>` feature report, `00 1F 82` output report) | `0x1F` | Experimental - GG spec `oled_direct_write` / `oled_reset_direct_write` (`apex_pro_tkl_2022`); `ssgg oled show|reset`, `0x1628` only, untested; pixel packing of the 640 bytes unknown |
| Arctis 7+ family (`0x220E`, `0x2212`, `0x2216`, `0x2236`) through `HeadsetSettingCommand` / `HeadsetStatusQueryCommand` | `0x39`, `0x37`, `0xA3`, `0xB0` | Experimental - GG spec `base_arctis_7_plus_tx`, 2026-10-25: Nova 7 frames, battery `0..=5`, mic volume `1..=7`, no sidetone on 7P+, no ChatMix on 7P+/7X+; untested |
| Arctis Nova 4 / 4X (`0x12F2`, `0x12F6`) and Nova 7 WoW upgrade (`0x22AB`) through the same commands | `0x39`, `0x37`, `0xA3`, `0xB0` | Experimental - GG specs `arctis_nova_4_tx`, `arctis_nova_4x_tx`, `base_arctis_nova_7_gen2_tx`, 2026-10-25: byte-identical to the Nova 7 TX (the WoW upgrade reports battery in percent); untested |
| Nova 5 / Nova 3 Wireless family (`0x2232`, `0x2253`, `0x2264`, `0x2269`, `0x226D`) through the same commands | `0x39`, `0x37`, `0xA3`, `0xB0` | Experimental - GG specs `base_arctis_nova_5_tx`, `base_arctis_nova_3_wireless_tx`, 2026-10-25: sidetone `0..=10`, mic volume `0..=15`, status reply has an unused byte before the percent battery; untested |
| Nova Pro family (`0x12CB`, `0x12CD`, `0x12FA` wired; `0x12E0`, `0x12E5`, `0x225D` wireless TX) and wired Nova 3 (`0x12EC`): `HeadsetSettingCommand` with report id `06`, `HeadsetBatteryQueryCommand` (`06 b7`, wireless only) | `0x39`, `0x37`, `0xB7` | Experimental - GG specs `base_arctis_nova_pro`, `base_arctis_nova_pro_wireless`, `arctis-nova-3`, 2026-10-25: sidetone `0..=3`, mic volume `1..=10`, battery `0..=8`, no auto-off; untested |
| `GenericHeadset` (every headset without a family above) | none | No protocol: every command answers unsupported. The `06 18/35/37/39/33/45/31` guesses were removed 2026-10-25 (GG's specs use other frames; `06 33` is mic volume on the Arctis 7). `0x12CF` is detect-only: GG's spec names it the Nova Pro Xbox bootloader |
| `OnboardDriverModeCommand` | `0x6C` | Experimental — `6c 00 01` is `DriverModeEnable` in the engine's init-sequence descriptor and is the fourth frame of GG's opening chain (read capture, answered by nothing); `6c 00 00` closes the mode at session close (engine-side only, not in the capture). Nothing in `ssgg` sends either |
| `OnboardFirmwareVersionCommand` | `0x90` | Confirmed — Apex 7 `0x1612`, Linux, 2026-10-20: `90 00` and `90 01` both answered |
| `OnboardPayloadPullCommand` | `0x85` | Confirmed — Apex 7 `0x1612`, Linux, 2026-10-20: 121 pulls answered by the input report |
| `OnboardBufferReadCommand` | `0x83` | Confirmed — Apex 7 `0x1612`, Linux, 2026-10-20: a full profile slot collected |
| `OnboardFileOpenCommand` | `0x88` | Confirmed — Apex 7 `0x1612`, Linux, 2026-10-20: answered `88 01 …` |
| `OnboardBufferWriteCommand` (legacy header `03 00`) | `0x03` | Confirmed — Apex 7 `0x1612`, Linux, 2026-10-20: both halves of every write window |
| `OnboardPayloadPushCommand` | `0x05` | Confirmed — Apex 7 `0x1612`, Linux, 2026-10-20: every push answered `05 01 …` |
| `OnboardFileCommitCommand` | `0xEB` | Confirmed — Apex 7 `0x1612`, Linux, 2026-10-20: accepted with no write-specific answer |
| `OnboardProfileDeployCommand` | `0x89` | Confirmed — Apex 7 `0x1612`, Linux, 2026-10-20: the keyboard runs the slot named |
| `MouseDpiCommand` / `CommandCode::MouseDpi` (tag `0xF2`, wire `0x2D`, `0x34` on Rival 3 Gen 2) | `0x2D` | Experimental — Modern-family mice, community rivalcfg-derived fork, untested. The wire byte clashes with `ActuationControl`, hence the tag |
| `MouseColorCommand` / `CommandCode::MouseColor` (tag `0xF3`, wire `0x21`, `0x61` on a 2.4 GHz receiver PID) | `0x21` | Experimental — Modern-family mice, community rivalcfg-derived fork, untested. The wire bytes clash with `RgbControl` and `Apex2023DirectWireless`, hence the tag |
| `CommandCode::MousePollRate` / `MousePollRateCommand` | `0x2B` | Experimental — Modern-family mice, community rivalcfg-derived fork, untested (`0x6B` on a receiver PID) |
| `CommandCode::MouseBattery` / `MouseBatteryQueryCommand` | `0x92` | Experimental — wireless Modern-family mice, community rivalcfg-derived fork, untested (`0xD2` on a receiver PID) |
| Mouse save to on-board memory (`11 00`) | `0x11` | Not implemented — persistent write, no hardware-confirmed family; `ssgg mouse` has no save action |
| `CommandCode::HeadsetSidetone` / `HeadsetSettingCommand` | `0x39` | Confirmed on `0x227E` only — Arctis Nova 7 Gen 2, Linux, 2026-10-25: `00 39 03` audible sidetone, `00 39 00` none (owner test); falls back to the headset's saved level after a power cycle (no save frame). Steps 1-2 untested. Experimental on every other PID (community forks) |
| `CommandCode::HeadsetMicVolume` / `HeadsetSettingCommand` | `0x37` | Confirmed on `0x227E` only — Arctis Nova 7 Gen 2, Linux, 2026-10-25: recorded mic level -80 dBFS at `00 37 00`, -39 dBFS at `00 37 07` (alternating takes); sidetone does not follow it. Middle steps untested. Experimental on every other PID (community forks) |
| `CommandCode::HeadsetAutoOff` / `HeadsetSettingCommand` | `0xA3` | Confirmed on `0x227E` only — Arctis Nova 7 Gen 2, Linux, 2026-10-25: `00 a3 01` turned the headset off after about a minute idle (owner test); `00 a3 00` (disable) sent but not observed. Experimental on every other PID (community forks) |
| `CommandCode::HeadsetStatus` / `HeadsetStatusQueryCommand` | `0xB0` | Confirmed on `0x227E` only — Arctis Nova 7 Gen 2, Linux, 2026-10-25: percent battery matched an independent reading, charging flipped on USB plug-in, ChatMix read -1.00 / +0.02 / +1.00 at full game / middle / full chat (owner test). Experimental on every other PID (community forks) |

## Rules

**Adding new protocol code:** Label it `[EXPERIMENTAL]` or `[PLACEHOLDER]` — silence implies confirmed.

**Promoting experimental to confirmed:** Only after testing on physical hardware. Include device model and date in the commit. Remove the label.

**Never:** State experimental protocol as production-ready in docs or user-facing output. The `experimental-apex-2023`
feature flag was removed on 2026-10-15; experimental paths are labelled in code and docs instead of feature-gated.
Writes that could change persistent device state are sent only on an explicit request (`ssgg onboard write --yes`),
and only on a family whose write sequence is confirmed on hardware — a family without a hardware-confirmed write
sequence stays unwired.
