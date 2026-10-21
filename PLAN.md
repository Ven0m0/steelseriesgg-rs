# PLAN: next steps

Single backlog for steelseriesgg-rs. Replaces the old `PLAN.md`, `TODO.md` and `handoff.md`.
Last reviewed 2026-10-21 against branch `feat/backlog-2026-10` (software-only round: everything below marked
"untested" has no hardware result yet).

Read `AGENTS.md` (same file as `CLAUDE.md`) and `.claude/rules/experimental-protocol.md` before any code change.
Protocol facts for the Apex Pro TKL 2023 are in `AGENTS.md` ("Protocol research notes") and
`docs/development/apex-pro-tkl-2023-gg-capture.md`; this file only tracks work.

## Baseline (verified 2026-10-15, Windows)

- `cargo clippy-ci` (alias for `clippy --all-targets --locked -- -D warnings`): clean.
- `cargo test --locked`: 303 passed, 2 ignored (Windows, branch `feat/backlog-2026-10`, 2026-10-21); 311 passed, 2
  ignored on Linux with `--features remap` (WSL Ubuntu, before the last review-fix commit).
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
   Code state (2026-10-21): `rgb perkey` on `0x1628` sends the GG-layout 84-key frame, merged onto the last frame
   `ssgg` itself sent (`per_key_color`/`save_last_frame`/`load_last_frame` in `apex_pro_tkl_2023.rs`) so unlisted
   keys keep their previous colour instead of going black; falls back to black with a warning when no frame was
   saved yet. `[HW] confirmed 2026-10-21`: `rgb perkey set-key W red` lit W red, other keys unaffected by the fix
   (prior behaviour, also confirmed same day: without a saved frame, unlisted keys went dark - this is why,
   see `docs/development/apex-pro-tkl-2023-gg-capture.md`). Branch `feat/backlog-2026-10`: on direct-protocol
   models `rgb brightness` no longer sends `00 22 <level>`; it scales the frame colours in software (as GG does) and
   resends the last saved frame. Untested on hardware. Limit: the level lives in the running process only (see the
   `shortcut:` note on `set_direct_brightness`), so a later `ssgg rgb color` run sends full-brightness colours;
   persisting it needs the daemon to stop pre-scaling through `RgbController` first. Per-key *effects*
   (`set_per_key_effect`, `trigger_key_reactive`, `apply_per_key_effect_with_brightness`) still reach the generic
   `0x23` placeholder and `0x09` apply on direct-protocol models, but only `ssgg validate` calls them; the macro
   `impl_keyboard_with_delegation!` generates them, so overriding needs a macro split. Daemon animation on these
   models still collapses to the first zone colour (`set_zone_colors`); a per-key wave needs a key-position table
   the repo does not have.
2. **Extra output report before each frame.** The existing `initialize()` path sends `00 09 00 ..` as an output report
   before frames. Code comments say `0x09` makes the controller fall back to its stored profile. Check (by USBPcap
   during step 1) whether `0x1628` still sends it; if yes, test with it skipped for `0x1628` and keep whichever
   variant gives a stable frame. Code still sends it from `initialize()` for `0x1628` and `0x1642` (left alone on
   purpose: the confirmed lighting results above were obtained with it); `apply()` already suppresses it.
3. **[HW, Linux] hidraw path.** `send_feature` on Unix uses `info.path` for `0x1628`. Untested. Needs a Linux machine
   with the keyboard; record the result in the capture note.
4. **Status decision (ask the user).** After steps 1-3, ask whether to drop `[EXPERIMENTAL]` for the `0x40` lighting
   frame on `0x1628`. If yes: update `.claude/rules/experimental-protocol.md`, the code labels and the docs in one
   commit naming "Apex Pro TKL 2023 (wired, PID 0x1628)" and the test date.

## P2. Apex Pro TKL 2023 wired: actuation and profile (blocked on read-back, now confirmed permanent)

`ssgg actuation` sends the experimental `0x2D` command; confirmed no-op on hardware (owner, 2026-10-21: no
feel or GG-visible change). GG instead rewrites the 12,500-byte on-board profile, and a live HID trace of the
engine (2026-10-21, see `docs/development/apex-pro-tkl-2023-gg-capture.md`, "Live HID trace and engine memory
read") found **no `GET_FEATURE` transfer at all** over a minute that included the owner viewing GG's own
actuation page - GG's UI reads its cached config, never the board. This is not a gap in `ssgg`'s HID coverage;
there is nothing on the wire to read on this firmware. Writing the profile stays blocked on a read-back
command for the same reason: the checksum is solved, but there is no way to get a current blob to modify, and
none is expected to turn up without new firmware.

1. **Done 2026-10-15, still blocked:** `keyboards/apex_pro_tkl_2023_profile.rs` has the checksum, `set_actuation`
   (rejects unknown mm values and keys without a slot) and the 25-chunk split, all unit-tested. Not wired to any
   command: no read-back exists (confirmed 2026-10-21, not just undiscovered), so there is no current blob to
   modify and a write would overwrite the board's other on-board settings with a stale/synthetic blob. Leave
   unwired unless a read-back command surfaces on a firmware update.
2. ~~Actuation calibration conflict~~ resolved 2026-10-21: `ACTUATION_POINTS` 1.0mm point updated to the live
   `(47,42)` reading (owner's board, GG UI showed 1.0mm), replacing the earlier, never-UI-verified `(21,18)`.
3. ~~Capture GG startup to check for a read-back command~~ superseded by the 2026-10-21 live trace above: there
   is none. **Still open, needs a capture** (USBPcap, started before launching GG): the init-sequence byte
   meanings `b1`, `b2`, `34`, `38 66`, `38 67`; the profile tables at blob offsets 1080, 1224, 1590.
   `second_actuation_thresholds`'s field name is now known (2026-10-21 live read, all zero on the owner's
   board) but not its blob offset.

## P3. Apex 3 TKL (`0x1622`) RGB not applied (issues #173, #257)

The code already sends `0x21` with selector `0xFF` and `R G B` per zone (OpenRGB 8-zone layout). Changed on the
branch (untested): `Apex3Tkl` no longer sends the APPLY `0x09` from `initialize()`/`apply()`, which OpenRGB's
8-zone controller never sends (checked against its source 2026-10-21) and which our own notes say reverts direct
lighting on other models. Open differences:

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

- #211 (Arctis Nova Pro Omni `0x2290`): now detect-only (its protocol is unknown; `GenericHeadset`'s guessed `06 xx`
  bytes are no longer sent to it). Needs a capture (`scripts/capture-device.ps1`) before any support. #165 (Apex Pro
  TKL Wireless 2023 `0x1630`): registered in code with tests. Ask the reporter to confirm on hardware, then close.
- #276 (Aerox 9 Wireless `0x1858`, Arctis Nova 7 `0x22A1`, Apex M750 `0x0616`): all three now have `[EXPERIMENTAL]`
  support on the branch (mouse Modern family, Nova 7 family, M750 solid colour). Ask the reporter to test with exact
  commands; none ran on hardware.
- #280: repo link is 404. No longer needed for the checksum; decrypted `.edevice` files would still help with P2.3.
- #277 (Sonar deeplink config import): format researched 2026-10-15, see `database-schemas.md` ("Shared config
  links"). Link = base64 CDN URL to the config JSON. Still unknown: how to create a config in Sonar (body of
  `POST /configs` or the `retrieveFromUrlHash` hash form). Scope after that is known.

## P6. Maintenance (low priority)

- CodeQL default setup fails on every push: its `c-cpp` job finds no sources (the only C file,
  `mock_udev/include/libudev.h`, is deleted in the pending commit). The REST API cannot set a language list that
  includes Rust, so untick C/C++ in Settings > Code security > CodeQL (manual, 2026-10-15).
- ~~Typed `HidCommand`s for the direct frame and `0x4B` init~~ done on the branch (`DirectFrameCommand`,
  `DirectInitCommand` in `hid_reports.rs`; bytes identical to before, covered by the existing frame tests).
- `src/main.rs` split: see the branch history (`refactor: move onboard, performance and diagnostics commands out
  of main.rs`); the active families (`cmd_rgb`, `cmd_profile`, `cmd_headset`, ...) stay in `main.rs`.

## P7. Fork imports not yet taken (analysed 2026-10-21)

Source forks: SarhanSoft/steelseriesgg-rs `linux-parity` (a parallel rewrite from 2026-09-25; nothing cherry-picks,
re-apply by hand) and JaimeMarques/steelseriesgg-rs `fix/audit-safety-and-ui`. Rules for every item: typed
`HidCommand`s (no raw byte arrays), `[EXPERIMENTAL]` labels, no setting write that also saves to the device, any
save behind `--yes` and only on a hardware-confirmed family. Fork headset data derives from HeadsetControl (GPL-3.0)
and OpenRGB (GPL-2.0): take facts, write our own code and test vectors.

- Pollrate rebind and persistence: Sarhan `src/pollrate.rs` unbinds and rebinds the SteelSeries `usbhid`
  interfaces so a changed `mousepoll`/`kbpoll` applies at once, and `--persistent` writes
  `/etc/modprobe.d/ssgg-usbhid.conf`. Ours only writes the parameter and saves to `config.toml`.
Taken on the branch (all `[EXPERIMENTAL]`, none run on hardware):
- Input remapper: feature `remap` (`src/input/`, `ssgg remap run|list`). Only key-to-key and key-to-combo, no
  macros/text/launch; escape chord LCtrl+RCtrl+LShift+RShift held 2 s. Never run against a real keyboard.
- Mice, rivalcfg "Modern" family only (24 PIDs: Aerox 3/5/9, Prime Mini/Wireless, Rival 3 Gen 2, Rival 5): DPI, poll
  rate, colour, battery via `ssgg mouse`. No save command anywhere.
- Headsets: Arctis Nova 7 family (15 PIDs) with sidetone, auto-off, mic volume, status via `ssgg headset`. No save
  `00 09` is ever sent, so sidetone may not stick on Gen 2.
- OpenRGB / fork keyboard families: Apex 9 TKL/Mini (`0x40`, 513 bytes, no init), Gen 1 `0x3A` models (Apex 5,
  Apex 7 TKL, Apex Pro TKL, Apex Pro 2024) on the direct path, Apex M750 solid colour (slot-to-key map unknown).

Still open:
- Other mice families (Prime/Prime+, Old, Rival 3, 310/TEN, wireless-old) and the rest of Sarhan's mouse table.
- Other headset families from Sarhan's `models.rs` (Arctis 1/7/9, Pro Wireless, Nova Pro Wireless, 7+, Nova 3/5):
  its `with_save` appends the save sequence to every write; strip that before using the data.
- Firmware probe: OpenRGB says Apex Pro TKL / Apex 9 switch to the Gen 3 protocol from firmware 1.19.7; ssgg does
  not probe firmware to choose the packet.
- Apex 3 (`0x161A`, tri-zone) and Old Apex (`0x1202`, `0x1206`) layouts exist in Sarhan's `protocol.rs`; not taken.
- Nova 7 Gen 2 (`0x227e`) Save `0x09` (Jaime `nova7_gen2.rs`): Save persists to the headset; not taken.
- Remapper gaps: no hot-plug (an unplugged keyboard ends the session), no LED capability on the virtual device.
- Skipped on purpose: Sarhan engine/control API (`src/engine`, good token and Host-check design to copy), PipeWire
  mixer, wry GUI app, rebranding; Jaime Electron app, native PipeWire RPC service (unconditional `pipewire`/`libspa`
  deps), Snap/.deb, `apex_gen3.rs` (duplicates `0x1642`), branding.

## P8. Hardware checks owed by the 2026-10-21 branch

Nothing below has run on a device. Ask whoever owns the device; record device, firmware and date; promote a status
only per `.claude/rules/experimental-protocol.md`.

- **[HW, owner, GG stopped]** Apex Pro TKL 2023 `0x1628`: `rgb color 00FF00`, `rgb brightness 50` then `100` (frame
  dims and restores?), then `rgb color` again in a new run (full brightness: known limit).
- **[HW, outside]** Apex 3 TKL `0x1622` (#173/#257): `ssgg -d rgb color FF0000`, `rgb off`, `rgb brightness 50` with the
  new no-APPLY behaviour.
- **[HW, outside]** #283 `0x1610`, #276 (`0x1858` `ssgg mouse`, `0x22A1` `ssgg headset`, `0x0616` `rgb color`),
  Apex 9 (`0x1634`, `0x1620`), Apex 5 / Apex 7 TKL / Apex Pro TKL / Apex Pro 2024 (`0x161C`, `0x1618`, `0x1614`,
  `0x1640`), other Nova 7 PIDs, other Modern-family mice.
- **[HW, Linux]** `ssgg remap run` on a real keyboard, including the escape chord and unplug behaviour.

---

## Done or dropped (do not redo)

- Device registration (#276): superseded on `feat/backlog-2026-10`, which implements all three devices as
  `[EXPERIMENTAL]` (the Aerox 9 skip is lifted by user decision 2026-10-21). `0x2290` is now the detect-only one.
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
