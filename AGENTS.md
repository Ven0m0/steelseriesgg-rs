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
src/main.rs                    main() and the active cmd_* handlers (rgb, profile, headset, mouse, remap, ...)
src/commands/                  moved cmd_* families: onboard.rs, performance.rs, diagnostics.rs
src/input/                     feature `remap`: key remapper (remapper.rs pure core, linux.rs evdev/uinput)
src/cli.rs                     clap types (Cli, Commands, *Action) and argument parse helpers
src/daemon.rs                  daemon loop, hot-plug monitoring, GameSense startup
src/error.rs                   crate::error::Error (thiserror) + Result alias
src/devices/hid_reports.rs     HidReportBuilder and typed HID commands - use this, not raw bytes
src/devices/discovery.rs       DeviceManager: enumeration, hot-plug, fingerprinting
src/devices/keyboards/         Keyboard impls (apex.rs, apex_7.rs, apex_pro_tkl_2023.rs = experimental, apex_m750.rs)
src/devices/headsets/          generic headset + nova7.rs (Arctis Nova 7 family, experimental)
src/devices/mice/              rivalcfg Modern family mice (modern.rs, dpi_tables.rs; experimental)
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
| `remap` | `ssgg remap run`: evdev key remapper (Linux only) | `evdev` (Linux target), `rustix/event` |

`audio`, `sonar` and `remap` are independent; do not couple them. Experimental protocol paths are not feature-gated; they carry
`[EXPERIMENTAL]` labels instead (see `.claude/rules/experimental-protocol.md`).

---

## Hard constraints

1. **HID reports**: always use `HidReportBuilder` and typed helpers in `src/devices/hid_reports.rs`. Never build raw byte arrays by hand.
2. **hidapi pin**: keep the exact `=` pin on `hidapi` in `Cargo.toml`; changing it requires explicit task justification.
3. **GameSense CORS**: enforce localhost-only origin; the policy may only be tightened, never relaxed.
4. **Propagate errors**: use `?` or return an explicit `Err` value — no `unwrap` or `expect` in production paths.
5. **Error types**: `crate::error::Error` with `thiserror` at library boundaries; `anyhow` with `context()` in `src/main.rs` and binaries.
6. **Protocol accuracy**: the direct-mode frames (`0x40`, `0x61`, `0x3A`), the profile blob and `PerKeyRgb (0x23)` are reverse-engineered; only a solid-colour and per-key `0x40` frame were replayed on hardware (wired `0x1628`). Label new protocol work clearly; never state it as confirmed unless tested on hardware.
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
- Command-code status (confirmed / experimental / placeholder): `.claude/rules/experimental-protocol.md`.
- Apex Pro TKL 2023 wired (PID `0x1628`, firmware `1.19.7`): transport, lighting frame, profile blob, slot map,
  checksum, 64-byte commands, GG's stored config and the Frida trace are in
  `docs/development/apex-pro-tkl-2023-gg-capture.md`. All `[EXPERIMENTAL]`. Invariants to keep in mind:
  - **Hardware evidence is thin**: `ssgg rgb color FF0000` turned the board solid red (2026-10-14, GG stopped,
    user-confirmed) and one per-key frame lit one key (2026-10-21). Other colours, Linux hidraw and actuation are untested.
  - **Never write the on-board profile**: no read-back command exists, so any write replaces the board's other
    settings with a stale or synthetic blob. The code (`apex_pro_tkl_2023_profile.rs`) builds blobs; nothing sends them.
  - **No actuation or lighting read-back exists on this firmware** (Frida trace, 2026-10-21: GG never issues
    `GET_FEATURE`). Do not add read-back commands for `0x1628`.
  - **GG overwrites ssgg** at about 19 fps while `SteelSeriesEngine.exe` runs. Stop GG before any hardware test.

### Verification tooling (Windows)

Use these to check protocol claims against what official GG really does, before promoting any status. Never modify
the target process or GG's files. Record device, firmware and date for every result.

| Question | Tool | How |
|----------|------|-----|
| What bytes does GG put on the wire? | USBPcap + tshark | `C:\Program Files\USBPcap\USBPcapCMD.exe -d \\.\USBPcap1 -o <file> -A` (no args = interactive, hangs). Read with `C:\Program Files\Wireshark\tshark.exe -e frame.time_epoch -e usb.data_fragment`; filter `usb.setup.bRequest==9 && usb.bmRequestType==0x21 && usb.setup.wValue==0x0300` (`0x0200` = output reports). Re-check the device address after changing ports. |
| Which HID calls does the engine make, and does a UI action trigger a command? | Frida (`frida`, `frida-trace` in `~/.venv`; or `rea` MCP `start_frida_session` / `instrument_with_frida`) | Attach read-only to `SteelSeriesEngine.exe`; hook kernel32 `ReadFile` / `WriteFile` / `DeviceIoControl` for small buffers only (large-buffer hooks coincided with engine restarts). Heap reads (`ReadProcessMemory`) recover the decoded live settings JSON; `*.edevice` files are encrypted and need no decryption. |
| What does a GG binary do (`HIDDLL.dll`, `SSEdevice.dll`)? | Ghidra, else `rea` MCP (Rizin/Cutter tools) | Ghidra is not on PATH on this machine; use `analyzeHeadless` if installed. Prefer Frida first, static analysis only when runtime tracing cannot answer. |
| What config does GG store? | SQLite, read-only | `C:\ProgramData\SteelSeries\GG\apps\engine\db\database.db`, table `configurations`, column `settings`, `device_id` 242. Open with GG stopped. |

A capture or trace proves what GG sends; it does not make an `ssgg` frame Confirmed. Confirmed needs the frame
replayed from `ssgg` on hardware (see `.claude/rules/experimental-protocol.md`).

Gotchas:
- Stop GG: kill every `SteelSeries*` process, or the engine respawns. Restart from `C:\Program Files\SteelSeries\GG`
  with `SteelSeriesGGEZ.exe -dataPath="C:\ProgramData\SteelSeries\GG" -dbEnv=production`
  (`SteelSeriesGG.exe` alone exits with code 2).
- Never launch GUI capture prompters with `-WindowStyle Hidden`; it hides the form.
- `ctx_execute_file` refuses paths outside the project; run analysis scripts with `python -I <script> <args>`.
- Capture artifacts live outside the repo in `C:\Users\Ven0m0\gg-capture-artifacts\`: `blobs.bin` / `blobs3.bin`
  (12,500-byte profile blobs, 8 distinct) and `profile_crc_check.py <blob files>`. The original pcaps are gone.

---

## Backlog

Tracked in `PLAN.md` only. Do not duplicate status here.
