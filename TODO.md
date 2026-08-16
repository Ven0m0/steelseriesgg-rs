# TODOs

> Actionable work is tracked, prioritized, and sequenced in [`PLAN.md`](./PLAN.md).
> This file is the lightweight backlog + research reference.

## Active backlog

1. **Analyze if [signal rgb](https://signalrgb.com) can be reverse engineered.** It can change the keyboards lighting on windows 11. Read "https://docs.signalrgb.com"
2. **Apex 3 TKL RGB (issue #173)** — commands report success but lighting never changes.
   Concrete lead: OpenRGB's 8-zone protocol (`0x21` = set zone colors, `0x23` = brightness);
   the current code uses `0x23` for color. See PLAN.md Phase 1.
3. **Apex Pro TKL 2023 protocol + RGB work** — see PLAN.md Phase 2.
   - Verify the real per-key RGB protocol on hardware and replace the placeholder `0x23` path.
   - Discover an actuation read-back command if the firmware exposes one.
   - Validate unsupported-key handling plus ANSI/ISO layout differences on hardware.

## Deferred research

- Open-G-Hub (`https://github.com/Sharper-Flow/Open-G-Hub`)
  - Defer unless a concrete blocker suggests reusable logic for active backlog items.

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
