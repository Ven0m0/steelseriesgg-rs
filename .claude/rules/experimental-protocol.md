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
| `CommandCode::Apex2023Direct` | `0x40` | Experimental — one solid-colour frame replayed on `0x1628` (2026-10-14); per-key frame confirmed on hardware 2026-10-21 (lit one key, rest went black as designed, matching ssgg's frame-replaces-all-keys behaviour); other colours and Linux untested |
| `APEX_GEN1_PACKET_ID_DIRECT` (`apex_pro_tkl_2023.rs`) | `0x3A` | Experimental — original Apex Pro `0x1610`, from OpenRGB, untested |
| `CommandCode::Apex2023Direct` on Apex 9 TKL `0x1634` / Apex 9 Mini `0x1620` (513-byte report, no `0x4B` init) | `0x40` | Experimental - community-fork table, untested on hardware |
| `CommandCode::ApexGen1Direct` on Apex 5 `0x161C`, Apex 7 TKL `0x1618`, Apex Pro TKL `0x1614`, Apex Pro (2024) `0x1640` | `0x3A` | Experimental - community-fork table, untested on hardware |
| `CommandCode::ApexMDirect` / `ApexMDirectCommand` (Apex M750 `0x0616`, 513-byte feature report on interface 2) | `0x8E` (byte 4 of header `00 00 00 01 8E 01 03 06 16`) | Experimental - community fork, untested; solid colour only, the slot-to-key map is unknown |
| `CommandCode::ApexMEnable` / `ApexMEnableCommand` (Apex M750 three-report enable sequence) | `0x01` (byte 4 of reports 1 and 3 only; no shared leading byte) | Experimental - community fork, untested |
| `Apex8ZoneBrightnessCommand` | `0x23` | Experimental — Apex 3 TKL brightness, from OpenRGB, untested |
| `CommandCode::ProfileChunk` / `ProfileChunkCommand` (Gen3 header `03 01 80`) | `0x03` | Experimental — `0x1628` profile upload chunk; checksum matches captures; never sent (no read-back) |
| `CommandCode::ActuationControl` | `0x2D` | Experimental — firmware command unknown; confirmed no-op on `0x1628` wired (owner test, 2026-10-21): no feel or GG-visible change. Live HID trace the same day showed GG itself never queries or sets actuation through any HID command on this model — it rewrites the on-board profile instead (see `docs/development/apex-pro-tkl-2023-gg-capture.md`, "Live HID trace and engine memory read") |
| `OnboardDriverModeCommand` | `0x6C` | Experimental — `6c 00 01` is `DriverModeEnable` in the engine's init-sequence descriptor and is the fourth frame of GG's opening chain (read capture, answered by nothing); `6c 00 00` closes the mode at session close (engine-side only, not in the capture). Nothing in `ssgg` sends either |
| `OnboardFirmwareVersionCommand` | `0x90` | Confirmed — Apex 7 `0x1612`, Linux, 2026-10-20: `90 00` and `90 01` both answered |
| `OnboardPayloadPullCommand` | `0x85` | Confirmed — Apex 7 `0x1612`, Linux, 2026-10-20: 121 pulls answered by the input report |
| `OnboardBufferReadCommand` | `0x83` | Confirmed — Apex 7 `0x1612`, Linux, 2026-10-20: a full profile slot collected |
| `OnboardFileOpenCommand` | `0x88` | Confirmed — Apex 7 `0x1612`, Linux, 2026-10-20: answered `88 01 …` |
| `OnboardBufferWriteCommand` (legacy header `03 00`) | `0x03` | Confirmed — Apex 7 `0x1612`, Linux, 2026-10-20: both halves of every write window |
| `OnboardPayloadPushCommand` | `0x05` | Confirmed — Apex 7 `0x1612`, Linux, 2026-10-20: every push answered `05 01 …` |
| `OnboardFileCommitCommand` | `0xEB` | Confirmed — Apex 7 `0x1612`, Linux, 2026-10-20: accepted with no write-specific answer |
| `OnboardProfileDeployCommand` | `0x89` | Confirmed — Apex 7 `0x1612`, Linux, 2026-10-20: the keyboard runs the slot named |
| `CommandCode::HeadsetSidetone` / `HeadsetSettingCommand` | `0x39` | Experimental — Arctis Nova 7 family sidetone `00 39 <0..=3>`, from community forks, untested |
| `CommandCode::HeadsetMicVolume` / `HeadsetSettingCommand` | `0x37` | Experimental — Arctis Nova 7 family mic volume `00 37 <0..=7>`, from community forks, untested |
| `CommandCode::HeadsetAutoOff` / `HeadsetSettingCommand` | `0xA3` | Experimental — Arctis Nova 7 family auto-off `00 a3 <minutes>`, from community forks, untested |
| `CommandCode::HeadsetStatus` / `HeadsetStatusQueryCommand` | `0xB0` | Experimental — Arctis Nova 7 family status query `00 b0` and `b0 …` reply, from community forks, untested |

## Rules

**Adding new protocol code:** Label it `[EXPERIMENTAL]` or `[PLACEHOLDER]` — silence implies confirmed.

**Promoting experimental to confirmed:** Only after testing on physical hardware. Include device model and date in the commit. Remove the label.

**Never:** State experimental protocol as production-ready in docs or user-facing output. The `experimental-apex-2023`
feature flag was removed on 2026-10-15; experimental paths are labelled in code and docs instead of feature-gated.
Writes that could change persistent device state are sent only on an explicit request (`ssgg onboard write --yes`),
and only on a family whose write sequence is confirmed on hardware — a family without a hardware-confirmed write
sequence stays unwired.
