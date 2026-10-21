# steelseriesgg-rs — Agent Handbook

## Project context

Open-source SteelSeries GG replacement for Linux. Controls SteelSeries keyboards and headsets via USB HID: RGB lighting effects, a GameSense-compatible HTTP server (port 27301), device profiles, and optional PulseAudio/PipeWire mixer and Sonar integration.

- **Language**: Rust 2024 edition (MSRV and toolchain: see Source-of-truth files)
- **Primary binary**: `ssgg` (`src/main.rs`, clap CLI)
- **Library crate**: `steelseries_gg` (`src/lib.rs`)
- **Platform**: Linux-first; Windows build supported but not primary
- **Config**: `$XDG_CONFIG_HOME/ssgg/config.toml`
- **Repo**: `https://github.com/Ven0m0/steelseriesgg-rs`

---

## Quick orientation

Use `rg --files src/` for the full tree. Non-obvious entry points:

```
src/main.rs                    main() and most cmd_* handlers
src/cli.rs                     clap types (Cli, Commands, *Action) and argument parse helpers
src/daemon.rs                  daemon loop, hot-plug monitoring, GameSense startup
src/error.rs                   crate::error::Error (thiserror) + Result alias
src/devices/hid_reports.rs     HidReportBuilder and typed HID commands - use this, not raw bytes
src/devices/discovery.rs       DeviceManager: enumeration, hot-plug, fingerprinting
src/devices/keyboards/         Keyboard impls (apex.rs, apex_pro_tkl_2023.rs = experimental)
src/gamesense/server.rs        Axum HTTP server (port 27301)
src/audio/                     feature `audio` (pulse.rs) and `sonar` (sonar.rs)
src/bin/                       helper binaries, not part of `ssgg` (discover_actuation probes actuation firmware)
examples/                      device_readback.rs (hardware read-back example)
scripts/capture-device.ps1     Windows USBPcap capture helper for new-device reports (see docs/development/adding-hardware.md)
assets/                        udev rules, systemd user unit
tests/                         integration tests
docs/development/              reverse-engineering notes; index and per-file state in docs/development/README.md
```

---

## Source-of-truth files

When prose docs and code disagree, always trust these files. Do not quote toolchain, MSRV, or pinned dependency versions from memory or from this document; read them.

| File | What it controls |
|------|-----------------|
| `Cargo.toml` | dependency versions and pins, features, MSRV, edition |
| `rust-toolchain.toml` | toolchain channel and components |
| `.github/workflows/ci.yml` | exact CI commands, pinned CI toolchain, feature matrix |
| `src/devices/hid_reports.rs` | HID command codes and report layouts |

---

## Toolchain and CI

### Local commands

Run the smallest relevant subset for any given change.

| Step | Command |
|------|---------|
| Format check | `cargo fmt --all -- --check` |
| Lint | `cargo clippy --all-targets --locked -- -D warnings` |
| Test | `cargo test --locked` |
| Release build | `cargo build --release --locked` |

Add `--features <flag>` when touching feature-gated code. The CI feature matrix is in
`.github/workflows/ci.yml`; the `ci-matrix` skill runs it locally. The `audio` feature needs `libpulse-dev` on
Debian/Ubuntu. On Windows, `--features audio` does not link (`LNK1181: cannot open input file ':libpulse-0.dll.lib'`);
this is expected, CI builds and lints `audio` on Linux only.

---

## Feature flags

| Flag | Enables | Extra dep |
|------|---------|-----------|
| *(none)* | Core RGB, GameSense, profiles | — |
| `audio` | AudioMixer, PulseAudio/PipeWire backend | `libpulse-dev` |
| `sonar` | SonarClient HTTP integration | — (reqwest already in tree) |

`audio` and `sonar` are independent; do not couple them. Experimental protocol paths are not feature-gated; they carry
`[EXPERIMENTAL]` labels instead (see `.claude/rules/experimental-protocol.md`).

---

## Hard constraints

1. **HID reports**: always use `HidReportBuilder` and typed helpers in `src/devices/hid_reports.rs`. Never build raw byte arrays by hand.
2. **hidapi pin**: keep the exact `=` pin on `hidapi` in `Cargo.toml`; changing it requires explicit task justification.
3. **GameSense CORS**: enforce localhost-only origin; the policy may only be tightened, never relaxed.
4. **Propagate errors**: use `?` or return an explicit `Err` value — no `unwrap` or `expect` in production paths.
5. **Error types**: `crate::error::Error` with `thiserror` at library boundaries; `anyhow` with `context()` in `src/main.rs` and binaries.
6. **Protocol accuracy**: the direct-mode frames (`0x40`, `0x61`, `0x3A`), the profile blob and `PerKeyRgb (0x23)` are reverse-engineered; only one solid-colour `0x40` frame was replayed on hardware. Label new protocol work clearly; never state it as confirmed unless tested on hardware.
7. **Minimal scope**: fix what the task asks, nothing more. No opportunistic refactors.
8. **No mutable global state**, no panics in non-test code (unless explicitly fatal and documented), no ignored `Result`s.
9. **`unsafe` blocks** must document the safety invariant inline.
10. **Platform-gated deps**: Unix-only crates (`rustix`) live under `[target.'cfg(unix)'.dependencies]`; keep uses behind matching `cfg`.

---

## Rust conventions

- Naming: `snake_case` for functions and modules, `PascalCase` for types and traits, `SCREAMING_SNAKE_CASE` for constants.
- `rustfmt.toml`: edition 2024 style, max 120 columns, 4-space indentation.
- Prefer `&T` borrows over owned values when ownership is not needed.
- New dependencies require justification; avoid heavy transitive deps.
- No comments that restate what the code does — comment only non-obvious invariants or workarounds.

---

## Working rules

- **Search before reading**: `rg 'TypeName|fn_name' src/`, then read only the relevant lines. Large files: `src/main.rs`, `src/devices/hid_reports.rs`.
- **Documents**: only create planning or analysis documents when explicitly requested.

---

## Protocol research notes

- `docs/development/` was checked against source on 2026-10-15 (`docs/development/README.md` lists each file and
  its state). Source still wins; verify before acting on a doc claim.
- Command-code status (confirmed / experimental / placeholder) lives in `.claude/rules/experimental-protocol.md`.

### Apex Pro TKL 2023, wired (PID `0x1628`): SteelSeries GG wire protocol

Captured from official GG on Windows with USBPcap on 2026-10-14 (firmware `1.19.7`). Full write-up, slot map and
offsets: `docs/development/apex-pro-tkl-2023-gg-capture.md`. All of it is `[EXPERIMENTAL]`; only the lighting frame
was replayed from `ssgg` (one solid colour, see below).

- **Where GG keeps the protocol**: the Electron UI (`app.asar`) only talks to the local core. HID code lives in
  `C:\Program Files\SteelSeries\GG\apps\engine`: `SteelSeriesEngine.exe` (Go) -> `SSEdevice.dll` -> `HIDDLL.dll`.
  `deviceSpecifications\*.edevice` are encrypted and were not decrypted.
- **Transport**: HID class SET_REPORT to USB interface 1 (`wIndex 1`). Feature report ID 0, 644 wire bytes
  (`wValue 0x0300`) for lighting and profile upload. Output report ID 0, 64 bytes (`wValue 0x0200`) for commands.
  Replies on interrupt EP `0x82`.
- **Lighting**: `0x40`, count `0x54` (84), then `[usage][R][G][B]` x84 (USB HID usages), zero padded. GG streams
  about 19 fps and overwrites other programs while its engine runs; with GG closed the keyboard keeps the last frame.
  GG brightness is software scaling of the RGB values (brightness 0 = all-black frame).
- **Code state**: `0x1628` uses the direct path with GG's 84-key table `WIRED_2023_KEYS`, a 645-byte buffer
  (report-ID byte + 644 wire bytes) and skips the `0x4B` init (`apex_pro_tkl_2023.rs`). Gen3 models keep `TKL_KEYS`
  (112) and 643. Unix `send_feature` is hidapi `send_feature_report` on the handle discovery opened (`keyboards/mod.rs`); untested on `0x1628` under Linux.
  Per-key (`rgb perkey`) sends the same 84-key frame with unlisted keys black (no read-back exists).
  `ssgg rgb` warns on Windows when `SteelSeriesEngine.exe` runs (`warn_if_gg_engine_running`, `src/main.rs`).
- **Hardware evidence**: on 2026-10-14, with GG stopped, `ssgg rgb color FF0000` turned the keyboard solid red
  (user-confirmed); USBPcap showed `wValue 0x0300, wIndex 1, wLength 644, 40 54 04 ff 00 00 05 ...`. Nothing else
  (other colours, per-key, Linux hidraw, actuation) has been tested.
- **On-board profile**: GG rewrites 12,500 bytes in 25 chunks after every change, chunk header
  `03 01 80 | len u16 LE (500) | offset u16 LE | 00 00`. Per-key tables of 70 slots: actuation at 1084, reset at
  1154, release mode at 1446 (0 off / 2 Rapid Trigger / 3 Key Guard), RT sensitivity at 1516; Rapid Tap enable at
  2700. Slot map is `WIRED_2023_PROFILE_SLOTS` / `profile_slot()`. Actuation raw values: 0.1 mm = `05`,
  1.0 = `15`, 2.0 = `39`, 3.0 = `86` (non-linear).
- **Profile checksum (solved 2026-10-15)**: little-endian u32 at blob offset 12320 = STM32 hardware CRC-32
  (poly `0x04C11DB7`, init `0xFFFFFFFF`, no reflection, no xorout) over bytes `0..12320` read as LE 32-bit words.
  Matches all 26 captured blobs (8 distinct). Code: `keyboards/apex_pro_tkl_2023_profile.rs` (checksum, actuation
  patch, chunk split; nothing sends it). **Still never write the profile**: no read-back command is known, so any
  write would replace the board's other settings with a stale or synthetic blob.
- **64-byte commands seen**: poll set `02 01 80`, `b1`, `b2`, `34`, `90` (`90` returns firmware version);
  `38 66 01/00` with Rapid Tap on/off; `38 67 0a 00 04 07 <n> ...` in an uncontrolled run. Meanings unknown except `90`.
- **GG's stored config**: per-key JSON in `C:\ProgramData\SteelSeries\GG\apps\engine\db\database.db`, table
  `configurations`, column `settings`, `device_id` 242 (`apex_pro_tkl_2022`). Open read-only with GG stopped.

### Windows reverse-engineering environment

- Tools: Wireshark (`C:\Program Files\Wireshark\tshark.exe`), USBPcap (`C:\Program Files\USBPcap\USBPcapCMD.exe`).
  `USBPcapCMD` without arguments is interactive and hangs; always pass `-d \\.\USBPcap1 -o <file> -A`.
- The keyboard was USB device address 3 in past captures; re-check with tshark after changing ports.
- Useful tshark fields: `-e frame.time_epoch -e usb.data_fragment`; filter
  `usb.setup.bRequest==9 && usb.bmRequestType==0x21 && usb.setup.wValue==0x0300` (or `0x0200` for output reports).
- Stop GG: kill every `SteelSeries*` process, or the engine respawns. Restart from `C:\Program Files\SteelSeries\GG`
  with `SteelSeriesGGEZ.exe -dataPath="C:\ProgramData\SteelSeries\GG" -dbEnv=production`
  (`SteelSeriesGG.exe` alone exits with code 2).
- Do not launch GUI capture prompters with `-WindowStyle Hidden`; it hides the form.
- `ctx_execute_file` refuses paths outside the project; run analysis scripts with `python -I <script> <args>`.
- Capture artifacts (outside the repo) in `C:\Users\Ven0m0\gg-capture-artifacts\`: `blobs.bin` / `blobs3.bin`
  (reassembled 12,500-byte profile blobs, 13 each, 8 distinct) and `profile_crc_check.py <blob files>`. The original
  pcaps (`run2.pcap`, `run3.pcap`), step CSVs, `an*.py` scripts and capture prompters were gone on 2026-10-15; the
  blobs were recovered from an old session scratchpad.

---

## Backlog

Tracked in `PLAN.md` only. Do not duplicate status here.
