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
| `CommandCode::Apex2023Direct` | `0x40` | Experimental — one solid-colour frame replayed on `0x1628` (2026-10-14); per-key, other colours untested |
| `APEX_GEN1_PACKET_ID_DIRECT` (`apex_pro_tkl_2023.rs`) | `0x3A` | Experimental — original Apex Pro `0x1610`, from OpenRGB, untested |
| `Apex8ZoneBrightnessCommand` | `0x23` | Experimental — Apex 3 TKL brightness, from OpenRGB, untested |
| `CommandCode::ProfileChunk` / `ProfileChunkCommand` (Gen3 header `03 01 80`) | `0x03` | Experimental — `0x1628` profile upload chunk; checksum matches captures; never sent (no read-back) |
| `CommandCode::ActuationControl` | `0x2D` | Experimental — firmware command unknown |
| `OnboardDriverModeCommand` | `0x6C` | Experimental — seen in a GG capture only; nothing in `ssgg` sends it |
| `OnboardFirmwareVersionCommand` | `0x90` | Confirmed — Apex 7 `0x1612`, Linux, 2026-10-20: `90 00` and `90 01` both answered |
| `OnboardPayloadPullCommand` | `0x85` | Confirmed — Apex 7 `0x1612`, Linux, 2026-10-20: 121 pulls answered by the input report |
| `OnboardBufferReadCommand` | `0x83` | Confirmed — Apex 7 `0x1612`, Linux, 2026-10-20: a full profile slot collected |
| `OnboardFileOpenCommand` | `0x88` | Confirmed — Apex 7 `0x1612`, Linux, 2026-10-20: answered `88 01 …` |
| `OnboardBufferWriteCommand` (legacy header `03 00`) | `0x03` | Confirmed — Apex 7 `0x1612`, Linux, 2026-10-20: both halves of every write window |
| `OnboardPayloadPushCommand` | `0x05` | Confirmed — Apex 7 `0x1612`, Linux, 2026-10-20: every push answered `05 01 …` |
| `OnboardFileCommitCommand` | `0xEB` | Confirmed — Apex 7 `0x1612`, Linux, 2026-10-20: accepted with no write-specific answer |
| `OnboardProfileDeployCommand` | `0x89` | Confirmed — Apex 7 `0x1612`, Linux, 2026-10-20: the keyboard runs the slot named |

**Apex 7 on-board profile (`0x1612`).** Read, write and deploy of a profile slot are confirmed on hardware
(2026-10-20), and `decode` / `encode` are verified against clean and modified profiles. What stays
experimental on this family: the field meanings inside the OLED and macro blocks, the payload region behind
the decoded payload, and the `0x1614` / `0x1616` / `0x1618` siblings (not enabled, never tested). An on-board
write has no read-back frame: a write is checked from the device by reading the slot back with the read path
and diffing the payload.

## Rules

**Adding new protocol code:** Label it `[EXPERIMENTAL]` or `[PLACEHOLDER]` — silence implies confirmed.

**Promoting experimental to confirmed:** Only after testing on physical hardware. Include device model and date in the commit. Remove the label.

**Never:** State experimental protocol as production-ready in docs or user-facing output. The `experimental-apex-2023`
feature flag was removed on 2026-10-15; experimental paths are labelled in code and docs instead of feature-gated.
Writes that could change persistent device state are sent only on an explicit request (`ssgg onboard write --yes`),
and only on a family whose write sequence is confirmed on hardware — a family without a hardware-confirmed write
sequence stays unwired.
