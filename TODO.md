# TODOs

> Actionable work is tracked, prioritized, and sequenced in [`PLAN.md`](./PLAN.md).
> This file is the lightweight backlog + research reference.

## Active backlog

Fix the errors in `cargo clippy-ci`

1. **Apex 3 TKL RGB (issue #173)** — commands report success but lighting never changes.
   Concrete lead: OpenRGB's 8-zone protocol (`0x21` = set zone colors, `0x23` = brightness);
   the current code uses `0x21` already (fixed since this note was written). See PLAN.md Phase 1 —
   blocker is now hardware confirmation, not code.
2. **Apex Pro TKL 2023 protocol + RGB work** — see PLAN.md Phase 2.
   - Verify the real per-key RGB protocol on hardware and replace the placeholder `0x23` path.
   - Discover an actuation read-back command if the firmware exposes one.
   - Validate unsupported-key handling plus ANSI/ISO layout differences on hardware.
3. **Read bug report**
  "ssgg_bug_report.json" — not present in the repo yet; blocked until obtained (PLAN.md Phase 7).


## Deferred research

- Open-G-Hub (`https://github.com/Sharper-Flow/Open-G-Hub`)
  - Defer unless a concrete blocker suggests reusable logic for active backlog items.
- SignalRGB (`https://signalrgb.com`, docs at `https://docs.signalrgb.com`) — checked 2026-08-18.
  SignalRGB does **not** delegate to SteelSeries GG/Engine; it talks to devices directly via its own
  JavaScript USB/HID plugin framework (per-device plugins, `docs.signalrgb.com/developer/plugins/`),
  and its own troubleshooting docs tell users to disable GG's Prism engine to stop it fighting for
  control of the same device. No SteelSeries-specific protocol bytes were found in the public docs
  (would require reading an actual SteelSeries plugin's source, not just the docs site). The
  plugin-dev tutorial (`developer/plugins/tutorial/`) documents a USB-capture-to-plugin workflow —
  capture, isolate RGB data, find init packets, map LED positions — that mirrors the USB-capture
  approach already planned for PLAN.md Phase 2; no new protocol lead, but confirms the methodology.

## Reference projects by relevance

### Directly relevant

- Apex keyboard protocol / RGB research
  - OpenRGB SteelSeries 8-zone controller (Apex 3 TKL) —
    https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/SteelSeriesController/SteelSeriesApex8ZoneController/SteelSeriesApex8ZoneController.h
  - https://github.com/AstroSnail/apexctl
  - https://github.com/FrankGrimm/apex7tkl_linux
  - https://github.com/not-jan/apex-tux
- Sonar / audio research
  - https://github.com/PrzemekkkYT/GGSonarRev
  - https://github.com/wex/sonar-rev
  - https://github.com/Mark7888/steelseries-sonar-py
  - https://codeberg.org/Birbwell/linuxmix
  - https://github.com/Dymstro/nova-chatmix-linux

### Research-only

- https://github.com/flozz/rivalcfg
- https://github.com/llMBQll/OmniLED

### Out of scope for the current backlog

- https://github.com/Gibtnix/Apex-Macros
- https://github.com/Gibtnix/MSIKLM
- https://github.com/stephenlacy/msi-keyboard
