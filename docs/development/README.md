# Development notes

Reverse-engineering and reference notes. Source code wins over every file here; command status lives in
`.claude/rules/experimental-protocol.md`, open work in `PLAN.md`.

| File | Content | State |
|------|---------|-------|
| [adding-hardware.md](adding-hardware.md) | How to capture a new device with GG and turn it into a PR | Current |
| [apex-7-gg-capture.md](apex-7-gg-capture.md) | Apex 7 wired (`0x1612`) GG wire protocol: read/write frames, payload layout, checksum; the other Apex keyboards of this legacy platform share it | Frames confirmed on hardware 2026-10-20; payload decoded and encoded by the crate's codec, only the insides of the OLED and macro byte arrays are uninterpreted |
| [apex-pro-tkl-2023-gg-capture.md](apex-pro-tkl-2023-gg-capture.md) | Apex Pro TKL 2023 wired (`0x1628`) GG wire protocol: lighting, profile blob, slot map, and the engine-to-HID transport table | `0x40` frames and `38 61` layer `00` confirmed on hardware (2026-10-14 to 2026-10-25), rest `[EXPERIMENTAL]`; transport mapping static from `SSEdevice.dll` 2026-11-10 |
| [devices.md](devices.md) | GG firmware product IDs, keyboard control endpoint, command codes with status | Checked against source 2026-10-20 |
| [gamesense-api.md](gamesense-api.md) | ssgg GameSense server endpoints and CORS; observed GG responses | Checked against source 2026-10-15; GG part from 2026-05 |
| [../../HANDOFF.md](../../HANDOFF.md) | RE work order: GG map, toolbox, tasks W1-W7 | Rewritten 2026-11-10, local only (gitignored) |
| [gg-parity.md](gg-parity.md) | PIDs in GG's device specs vs `product_ids`, per class, with the missing list | Generated 2026-10-25 by `scripts/gg_parity.py` |
| [gg-gamesense-engine.md](gg-gamesense-engine.md) | GameSense surface of GG's engine binary (internal endpoints, GSI installer) vs ssgg's server | Static string scan 2026-10-25, tagged observed/inferred/unknown |
| [gg-keyboard-apis.md](gg-keyboard-apis.md) | Every keyboard API in GG's device specs, per protocol family: frames, transports, ssgg coverage, PID map | Generated from the decrypted specs 2026-11-10; spec-only, so `[EXPERIMENTAL]` |
| [firmware-notes.md](firmware-notes.md) | Keyboard firmware images GG ships: inventory, how to load them, Apex Pro TKL 2023 1.19.7 HID descriptor and command router, Gen 1 container layout | Static analysis 2026-11-10, `[EXPERIMENTAL]`, load base `0x08008000 +/- 0x1000` |
| [database-schemas.md](database-schemas.md) | GG engine, Prism and Sonar SQLite schemas | Read 2026-05; per-key fields corrected 2026-10-15 |
