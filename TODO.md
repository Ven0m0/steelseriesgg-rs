# TODOs

## Resolved

- Issue #120 (`'ssgg audio' becomes unresponsive`) was closed as completed on 2026-03-27.
  The PulseAudio connection path in `src/audio/pulse.rs` now has a 5s timeout.
- Issue #211 (Arctis Nova Pro Omni not recognised) — resolved 2026-05-28.
  `ARCTIS_NOVA_PRO_OMNI = 0x2290` registered in `src/devices/mod.rs`.
- Issue #165 (Apex Pro TKL Wireless 2023 not recognised) — resolved 2026-05-28.
  `APEX_PRO_TKL_2023_WIRELESS_2 = 0x1630` registered in `src/devices/mod.rs` and `discovery.rs`.

## Active backlog

1. Reproduce issue #6 on an Arch-like environment and fix it only if the failure still exists.
   - Validate the `cargo build --release --locked` path used by the package and release workflow.
   - Keep any fix limited to packaging, install metadata, manifests, or CI unless reproduction proves a code change is required.
2. Continue Apex Pro TKL 2023 protocol and RGB work.
   - Verify the real per-key RGB protocol on hardware and replace the placeholder `0x23` path when confirmed.
   - Discover an actuation read-back command if the firmware exposes one.
   - Validate unsupported-key handling plus ANSI/ISO layout differences on hardware.

## Deferred research

- Open-G-Hub (`https://github.com/Sharper-Flow/Open-G-Hub`)
  - Defer unless a concrete blocker suggests reusable logic for active backlog items.

## Reference projects by relevance

### Directly relevant

- Sonar / audio research
  - https://github.com/PrzemekkkYT/GGSonarRev
  - https://github.com/wex/sonar-rev
  - https://github.com/Mark7888/steelseries-sonar-py
  - https://codeberg.org/Birbwell/linuxmix
  - https://github.com/Dymstro/nova-chatmix-linux
- Apex keyboard protocol / RGB research
  - https://github.com/AstroSnail/apexctl
  - https://github.com/FrankGrimm/apex7tkl_linux
  - https://github.com/not-jan/apex-tux

### Research-only

- https://github.com/flozz/rivalcfg
- https://github.com/llMBQll/OmniLED

### Out of scope for the current backlog

- https://github.com/Gibtnix/Apex-Macros
- https://github.com/Gibtnix/MSIKLM
- https://github.com/stephenlacy/msi-keyboard
