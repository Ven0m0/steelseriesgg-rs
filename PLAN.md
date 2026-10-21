# PLAN: next steps

Single backlog for steelseriesgg-rs. Replaces the old `PLAN.md`, `TODO.md` and `handoff.md`.
Last reviewed 2026-10-15 against `main` at `a7e5915`.

Read `AGENTS.md` (same file as `CLAUDE.md`) and `.claude/rules/experimental-protocol.md` before any code change.
Protocol facts for the Apex Pro TKL 2023 are in `AGENTS.md` ("Protocol research notes") and
`docs/development/apex-pro-tkl-2023-gg-capture.md`; this file only tracks work.

## Baseline (verified 2026-10-15, Windows)

- `cargo clippy-ci` (alias for `clippy --all-targets --locked -- -D warnings`): clean.
- `cargo test --locked`: 140 passed, 2 ignored (Windows); 141 passed, 2 ignored on Linux (WSL Ubuntu, 2026-10-15).
- Linux `cargo clippy --all-targets -- -D warnings`, also with `--features sonar`: clean (WSL, 2026-10-15).
- `cargo report future-incompatibilities --id 1`: "no reports are currently available".
- `cargo test --locked --features audio`: fails to link on Windows (no libpulse). Expected; not a code bug. CI covers
  `audio` on Linux for clippy and release build.

## How to work this list

- Items marked **[HW]** need the physical device. The owner has an Apex Pro TKL 2023 wired (`0x1628`) on Windows 11
  with SteelSeries GG installed. Other devices need an outside tester (named per item).
- Hardware steps: prepare the build and the exact commands, then ask the user to run them and report what the
  keyboard shows. Do not claim a result the user did not confirm.
- Stop GG before any `ssgg rgb` test on Windows (kill every `SteelSeries*` process); GG overwrites lighting at ~19 fps.
- Protocol labels: anything not confirmed on hardware stays `[EXPERIMENTAL]` / `[PLACEHOLDER]`. Promotion needs a
  commit naming the device model and test date.
- Run `cargo fmt --all -- --check`, `cargo clippy-ci` and `cargo test --locked` after every code change. `cfg(unix)`
  code does not compile on Windows; check it in WSL (`CARGO_TARGET_DIR` outside `/mnt/c`).

---

## P1. Apex Pro TKL 2023 wired (`0x1628`): finish lighting

Lighting is the only part replayed from `ssgg` so far (one solid red, 2026-10-14).

1. **[HW] Lighting test matrix**, GG stopped, `.\target\debug\ssgg.exe`:
   - `rgb color 00FF00`, `rgb color 0000FF`, `rgb color FFFFFF` (all keys, correct colour?)
   - `rgb off`
   - `rgb brightness 50` then `rgb brightness 100` (does the brightness path scale RGB for this model, or send a
     command the keyboard ignores? GG does software scaling only.)
   - `rgb effect breathing`, `rgb effect spectrum`, `rgb effect wave` (animated effects need a running loop; note
     whether the CLI returns immediately)
   - `rgb perkey ...` (read `PerKeyAction` in `src/cli.rs` for syntax): set W, A, Esc, F1, arrow keys. Esc, F-row and
     nav cluster are in `WIRED_2023_KEYS` for lighting even though they have no profile slot.
   - Capture one run with USBPcap to confirm frames still match the GG layout (filter in `AGENTS.md`).
   Fix whatever fails. If brightness sends a no-op command for `0x1628`, implement it as RGB scaling in the 2023
   direct path (matches GG behaviour).
   Code state (2026-10-15): `rgb perkey` on `0x1628` now sends the GG-layout 84-key frame with unlisted keys black
   (`set_key_colors` in `apex_pro_tkl_2023.rs`); `rgb brightness` still sends the generic `00 22 <level>` output
   report. Expect per-key to light only the named keys and the rest to go dark (`set-key`, `set-keys`,
   `set-region` and `test-matrix` all use the frame). Per-key *effects* (`set_per_key_effect`,
   `trigger_key_reactive`, `apply_per_key_effect_with_brightness`) still reach the generic `0x23` placeholder and
   `0x09` apply on direct-protocol models; route them through the frame if the effect tests fail.
2. **Extra output report before each frame.** The existing `initialize()` path sends `00 09 00 ..` as an output report
   before frames. Code comments say `0x09` makes the controller fall back to its stored profile. Check (by USBPcap
   during step 1) whether `0x1628` still sends it; if yes, test with it skipped for `0x1628` and keep whichever
   variant gives a stable frame.
3. **[HW, Linux] hidraw path.** `send_feature` on Unix uses `info.path` for `0x1628`. Untested. Needs a Linux machine
   with the keyboard; record the result in the capture note.
4. **Status decision (ask the user).** After steps 1-3, ask whether to drop `[EXPERIMENTAL]` for the `0x40` lighting
   frame on `0x1628`. If yes: update `.claude/rules/experimental-protocol.md`, the code labels and the docs in one
   commit naming "Apex Pro TKL 2023 (wired, PID 0x1628)" and the test date.

## P2. Apex Pro TKL 2023 wired: actuation and profile (blocked on read-back)

`ssgg actuation` sends the experimental `0x2D` command; GG instead rewrites the 12,500-byte on-board profile.
Writing the profile is blocked on a read-back command: the checksum is solved, but there is no current blob to modify.

1. **Partly done 2026-10-15:** `keyboards/apex_pro_tkl_2023_profile.rs` has the checksum, `set_actuation` (rejects
   unknown mm values and keys without a slot) and the 25-chunk split, all unit-tested. Not wired to any command:
   without a read-back (step 2) there is no current blob to modify. Original task: implement a profile read-modify-write for actuation on `0x1628` (chunk
   header `03 01 80 | len | offset | 00 00`, 25 chunks), using `WIRED_2023_PROFILE_SLOTS` and the four known raw
   actuation values. Unknown mm values must be rejected, not interpolated, until more points are captured. Get
   explicit user approval before the first write to the keyboard.
2. **Capture GG startup** (USBPcap started before launching GG) to get the init sequence and see whether GG reads
   the profile back. A read-back command would allow safe read-modify-write and replaces the old
   "discover an actuation read-back command" task (`src/bin/discover_actuation.rs`).
3. **Decode remaining commands** while capturing: `b1`, `b2`, `34`, `38 66`, `38 67`; the profile tables at 1080,
   1224, 1590; where `second_actuation_thresholds` is stored. Record findings in the capture note.

## P3. Apex 3 TKL (`0x1622`) RGB not applied (issues #173, #257)

The code already sends `0x21` with selector `0xFF` and `R G B` per zone (OpenRGB 8-zone layout). Open differences:

1. **[HW, outside tester]** `yann-morin-1998` offered to test on #257. Push a branch, post exact commands
   (`ssgg -d rgb color FF0000`, `ssgg rgb off`, `ssgg rgb brightness 50`) and ask for debug logs. Confirm with the
   user before posting on GitHub.
2. #257 duplicates #173; close #257 in favour of #173 once the test result is in (ask the user first).

## P4. Apex Pro original (`0x1610`) RGB does nothing on Linux (issue #283)

Reporter (NixOS) sees successful HID writes but no change. `0x1610` uses the generic 1-zone path.

Done 2026-10-15: ssgg sent a 65-byte `0x21` output report; OpenRGB uses a 643-byte feature
report `00 3A N [usage R G B] xN` on interface 1 with no init. `0x1610` now goes through `ApexProTkl2023` with packet
ID `0x3A`, the 112-key table, and Unix hidraw interface 1. Remaining: ask the reporter to test (confirm with the user before posting). Which
collection Windows discovery picks for `0x1610` is unknown.

## P5. GitHub close-out (ask before any GitHub write)

- #211 (Arctis Nova Pro Omni `0x2290`) and #165 (Apex Pro TKL Wireless 2023 `0x1630`): registered in code with
  tests. Ask the reporters to confirm on hardware, then close.
- #280: repo link is 404. No longer needed for the checksum; decrypted `.edevice` files would still help with P2.3.
- #277 (Sonar deeplink config import): format researched 2026-10-15, see `database-schemas.md` ("Shared config
  links"). Link = base64 CDN URL to the config JSON. Still unknown: how to create a config in Sonar (body of
  `POST /configs` or the `retrieveFromUrlHash` hash form). Scope after that is known.

## P6. Maintenance (low priority)

- CodeQL default setup fails on every push: its `c-cpp` job finds no sources (the only C file,
  `mock_udev/include/libudev.h`, is deleted in the pending commit). The REST API cannot set a language list that
  includes Rust, so untick C/C++ in Settings > Code security > CodeQL (manual, 2026-10-15).
- Move the hand-built direct frame (`build_direct_frame`) and `0x4B` init buffer in `apex_pro_tkl_2023.rs` to typed
  `HidCommand`s in `hid_reports.rs` (hard constraint 1; pre-existing, found in review 2026-10-15).

- `src/main.rs` is still ~2,000 lines after `cli.rs` and `daemon.rs` were split out. Extract the remaining
  low-churn `cmd_*` families into modules without changing clap behaviour, output or feature gating. Do this only
  when no P1-P4 work is in flight.

## P7. Fork imports not yet taken (analysed 2026-10-21)

Source forks: SarhanSoft/steelseriesgg-rs `linux-parity` (a parallel rewrite from 2026-09-25; nothing cherry-picks,
re-apply by hand) and JaimeMarques/steelseriesgg-rs `fix/audit-safety-and-ui`. Rules for every item: typed
`HidCommand`s (no raw byte arrays), `[EXPERIMENTAL]` labels, no setting write that also saves to the device, any
save behind `--yes` and only on a hardware-confirmed family. Fork headset data derives from HeadsetControl (GPL-3.0)
and OpenRGB (GPL-2.0): take facts, write our own code and test vectors.

- Pollrate rebind and persistence: Sarhan `src/pollrate.rs` unbinds and rebinds the SteelSeries `usbhid`
  interfaces so a changed `mousepoll`/`kbpoll` applies at once, and `--persistent` writes
  `/etc/modprobe.d/ssgg-usbhid.conf`. Ours only writes the parameter and saves to `config.toml`.
- Input remapper: Sarhan `src/input/*` (pure clock-injected `Remapper`, 25 tests; evdev grab + uinput re-emit).
  Needs the `evdev` crate and a uinput uaccess rule, which is new attack surface.
- Mice: Sarhan `src/devices/mice/*` (data-driven, from rivalcfg, WTFPL; byte-checked against rivalcfg's tests, never
  run on hardware). Upstream has no mouse support. Its onboard-save action is ungated.
- Headset model table: Sarhan `src/devices/headsets/models.rs`. Its `with_save` appends the save sequence to every
  setting write; remove that before using the data.
- OpenRGB family commands for PIDs upstream does not cover (Apex 3 TKL 8-zone, Apex M, Apex 9): Sarhan
  `src/devices/keyboards/protocol.rs` and the 19 `HidCommand` impls in its `hid_reports.rs`. Do not take its PID
  routing: it sends `0x1612` down the OpenRGB `0x3A` path (upstream's Apex 7 onboard class is hardware-confirmed)
  and labels `0x1628` hardware-verified on the legacy zone path.
- Nova 7 Gen 2 (`0x227e`) writes (sidetone, auto-off, Save `0x09`): Jaime `src/devices/headsets/nova7_gen2.rs`.
  Untested, and Save persists to the headset.
- Skipped on purpose: Sarhan engine/control API (`src/engine`, good token and Host-check design to copy), PipeWire
  mixer, wry GUI app, rebranding; Jaime Electron app, native PipeWire RPC service (unconditional `pipewire`/`libspa`
  deps), Snap/.deb, `apex_gen3.rs` (duplicates `0x1642`), branding.

---

## Done or dropped (do not redo)

- Device registration (#276): `0x22A1` and `0x0616` are detect-only (`is_detect_only`); Aerox 9 mouse skipped by user
  decision.
- Issue #120 audio hang (5 s timeout in `src/audio/pulse.rs`).
- Protocol-docs reconciliation, capability-label cleanup, Windows pollrate "unsupported on this driver" notice,
  SignalRGB research (it drives devices directly via its own plugins; no SteelSeries bytes in public docs).
- Apex Pro TKL 2023 wired: GG capture, 84-key lighting table, 645-byte frame, interface-1 path, profile slot map
  (`WIRED_2023_PROFILE_SLOTS`), GG-engine warning. Committed in `f430380`.
- Old TODO items: clippy-ci / future-incompat errors (resolved, see Baseline); `ssgg_bug_report.json` triage
  (dropped: the file was never provided); issue #6 Arch compile (closed `not_planned`).
- `docs/development/` consolidation (2026-10-15): capture-note "Transport" section matches the 645-byte /
  `info.path` code for `0x1628`; `0x40` and `0x2D` relabelled `[EXPERIMENTAL]` with a link to the capture note;
  `protocol-keyboard.md` merged into `devices.md`; index in `docs/development/README.md`.
- Old Phase 2 task "replace placeholder `KeyAddress` values for the 2023 model": superseded for `0x1628` by the
  HID-usage table from the GG capture.

## Reference projects

- Apex RGB: OpenRGB `SteelSeriesApex8ZoneController`
  (https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/SteelSeriesController/SteelSeriesApex8ZoneController/SteelSeriesApex8ZoneController.h),
  https://github.com/AstroSnail/apexctl, https://github.com/FrankGrimm/apex7tkl_linux,
  https://github.com/not-jan/apex-tux
- Sonar / audio: https://github.com/PrzemekkkYT/GGSonarRev, https://github.com/wex/sonar-rev,
  https://github.com/Mark7888/steelseries-sonar-py, https://codeberg.org/Birbwell/linuxmix,
  https://github.com/Dymstro/nova-chatmix-linux
- Forks (see P7): https://github.com/SarhanSoft/steelseriesgg-rs, https://github.com/JaimeMarques/steelseriesgg-rs
- Research only: https://github.com/flozz/rivalcfg, https://github.com/llMBQll/OmniLED,
  https://github.com/Sharper-Flow/Open-G-Hub
