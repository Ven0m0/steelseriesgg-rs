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
src/commands/                  moved cmd_* families: onboard.rs, gen3_profile.rs, keymap.rs, apex_2023.rs, performance.rs, diagnostics.rs
src/input/                     feature `remap`: key remapper (remapper.rs pure core, linux.rs evdev/uinput)
src/cli.rs                     clap types (Cli, Commands, *Action) and argument parse helpers
src/daemon.rs                  daemon loop, hot-plug monitoring, GameSense startup
src/error.rs                   crate::error::Error (thiserror) + Result alias
src/devices/hid_reports.rs     HidReportBuilder and typed HID commands - use this, not raw bytes
src/devices/discovery.rs       DeviceManager: enumeration, hot-plug, fingerprinting
src/devices/keyboards/         Keyboard impls (apex.rs, apex_7.rs, apex_pro_tkl_2023.rs = mixed, status in the rules file, apex_m750.rs)
src/devices/headsets/          generic headset + nova7.rs (Arctis Nova 7 family, experimental)
src/devices/mice/              rivalcfg Modern family mice (modern.rs, dpi_tables.rs; experimental)
src/gamesense/server.rs        Axum HTTP server (port 27301)
src/audio/                     feature `audio` (pulse.rs) and `sonar` (sonar.rs)
src/gui.rs                     editor + lighting API server, served by `ssgg-gui` (lib module)
src/bin/                       helper binaries, not part of `ssgg` (discover_actuation probes actuation firmware;
                               ssgg-gui = native window around the editor, feature `desktop`)
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

`.cargo/config.toml` builds for `x86-64-v3` by default; release CI also builds generic `x86-64` by setting
`RUSTFLAGS=-Ctarget-cpu=x86-64` (env overrides the config; `SSGG_TARGET_CPU` does the same inside the PKGBUILD).
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
| `desktop` | `ssgg-gui`: native window around the profile editor (`build.rs` minifies its HTML/CSS/JS) | `wry`, `tao`; build-deps `minify-html`, `oxc_*`, `lightningcss`; Linux needs `libwebkit2gtk-4.1-dev`, `libgtk-3-dev` |

`audio`, `sonar`, `remap` and `desktop` are independent; do not couple them. Experimental protocol paths are not feature-gated; they carry
`[EXPERIMENTAL]` labels instead (see `.claude/rules/experimental-protocol.md`).

---

## Hard constraints

1. **HID reports**: always use `HidReportBuilder` and typed helpers in `src/devices/hid_reports.rs`. Never build raw byte arrays by hand.
2. **hidapi pin**: keep the exact `=` pin on `hidapi` in `Cargo.toml`; changing it requires explicit task justification.
3. **GameSense CORS**: enforce localhost-only origin; the policy may only be tightened, never relaxed.
4. **Propagate errors**: use `?` or return an explicit `Err` value — no `unwrap` or `expect` in production paths.
5. **Error types**: `crate::error::Error` with `thiserror` at library boundaries; `anyhow` with `context()` in `src/main.rs` and binaries.
6. **Protocol accuracy**: the direct-mode frames (`0x40`, `0x61`, `0x3A`), the profile blob and `PerKeyRgb (0x23)` are reverse-engineered; what was replayed on hardware is listed per command in `.claude/rules/experimental-protocol.md`. Label new protocol work clearly; never state it as confirmed unless tested on hardware.
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

## Project tooling (`.claude/`)

- Skills: `ci-matrix` (full local CI), `hid-protocol` (auto-loads for `src/devices/`), `hw-capture`, `promote-protocol`, `dep-audit`, `fuzz-runner`, `benchmark-compare`.
- Review agents: `protocol-reviewer` (device diffs), `security-reviewer` (GameSense server), `dependency-auditor`, `docs-drift-auditor`, `re-verifier`, `release-reviewer`.

---

## Protocol research notes

- `docs/development/README.md` lists each doc and its state. Source still wins; verify before acting on a doc claim.
- Command-code status (confirmed / experimental / placeholder): `.claude/rules/experimental-protocol.md`.
- Apex Pro TKL 2023 wired (PID `0x1628`, firmware `1.19.7`): transport, lighting frame, profile blob, slot map,
  checksum, 64-byte commands, GG's stored config and the Frida trace are in
  `docs/development/apex-pro-tkl-2023-gg-capture.md`. Status per frame in the rules file. Invariants to keep in mind:
  - **Confirmed on hardware** (Windows, fw 1.19.7): `0x40` solid colours, per-key frames, software brightness and
    25 fps effect streaming; `0x41` release; `1F 81/82` and `38 83` OLED; `38 61` layer 0 actuation; the `83 01`
    profile read-back of all five slots; `36`/`B6`/`B7` key mappings and macro read; `F2`/`F5` device info
    (2026-11-10/11). Linux hidraw is untested.
  - **On-board profile writes are gated, not forbidden** (2026-11-10): the read-back works, so a write is allowed only
    through `ssgg onboard gen3-write --yes` (`gen3-restore` for saved blobs) after every slot was read and saved in
    that run and the read blob of the target slot re-encodes byte-identically (`[EXPERIMENTAL]`, not yet replayed from
    `ssgg` on hardware). GG itself persists bindings, rapid-tap pairs, lock colours and the OLED image by
    rewriting the whole slot (`02 01`, 25 x `03 01`, `B1`, `B2`, `34 00`); it never sends `36`/`B6` on this model.
  - **Lighting read-back**: none (the board keeps no effect; GG streams `0x40` host-rendered frames). The profile
    blob is the only read-back for settings.
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
- Capture artifacts live outside the repo in `%USERPROFILE%\gg-capture-artifacts\`: `blobs.bin` / `blobs3.bin`
  (12,500-byte profile blobs, 8 distinct) and `profile_crc_check.py <blob files>`. The original pcaps are gone.

---

## Backlog

Tracked in `PLAN.md` only. Do not duplicate status here.
