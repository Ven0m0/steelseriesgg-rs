# steelseriesgg-rs — Agent Handbook

## Project context

Open-source SteelSeries GG replacement for Linux. Controls SteelSeries keyboards and headsets via USB HID: RGB lighting effects, a GameSense-compatible HTTP server (port 27301), device profiles, and optional PulseAudio/PipeWire mixer and Sonar integration.

- **Language**: Rust 2024 edition, MSRV 1.97.1
- **Primary binary**: `ssgg` (`src/main.rs`, clap CLI)
- **Library crate**: `steelseries_gg` (`src/lib.rs`)
- **Platform**: Linux-first; Windows build supported but not primary
- **Config**: `$XDG_CONFIG_HOME/ssgg/config.toml`
- **Repo**: `https://github.com/Ven0m0/steelseriesgg-rs`

---

## Quick orientation

### Primary modules

```
src/main.rs                              CLI entry point (clap, subcommands)
src/lib.rs                               Public crate root; re-exports prelude
src/error.rs                             crate::error::Error (thiserror) + Result alias
src/device_state.rs                      Device state snapshots and diffs
src/diagnostics_export.rs               Export diagnostics to file or stdout
src/fs_utils.rs                          Filesystem helpers (path resolution, atomic writes)
src/pollrate.rs                          HID polling rate query and control
src/performance.rs                       PerformanceManager
src/validation.rs                        RgbValidator, ValidationReport
src/config/mod.rs                        $XDG_CONFIG_HOME/ssgg/config.toml parsing
src/profiles/mod.rs                      Save and load device state as TOML
src/rgb/mod.rs                           Color, Effect, RgbController, PerKeyEffect
src/gamesense/mod.rs                     GameSense handler registration
src/gamesense/server.rs                  Axum HTTP server (port 27301)
src/audio/mod.rs                         AudioMixer type (feature `audio`)
src/audio/pulse.rs                       PulseAudio/PipeWire backend
src/audio/sonar.rs                       SonarClient HTTP integration (feature `sonar`)
```

### Devices

```
src/devices/mod.rs                       Device trait and shared types
src/devices/hid_reports.rs              Type-safe HID report builder — use this, not raw bytes
src/devices/key_mapping.rs              Key address and ID types
src/devices/zone_mapping.rs             Zone to HID mapping
src/devices/discovery.rs               Hot-plug and device fingerprinting
src/devices/diagnostics.rs             Runtime diagnostics
src/devices/fuzz.rs                    Fuzzing targets for HID report parsing
src/devices/keyboards/mod.rs           Keyboard device registry
src/devices/keyboards/apex.rs          Apex series protocol implementation
src/devices/keyboards/apex_pro_tkl_2023.rs  Apex Pro TKL 2023 (experimental, feature `experimental-apex-2023`)
src/devices/headsets/mod.rs            Headset protocol implementations
```

### Helper binaries (not part of the primary `ssgg` CLI)

```
src/bin/discover_actuation.rs          Probe actuation firmware commands
src/bin/verify_key_mapping.rs          Map validation on hardware
src/bin/benchmark_rgb_alloc.rs         RGB allocation benchmark
src/bin/benchmark_fragment.rs          Fragment benchmark
src/bin/benchmark_validation_io.rs     Validation I/O benchmark
src/bin/sonar_control.rs               Sonar HTTP API exerciser (feature `sonar`)
```

### Assets and tests

```
assets/99-steelseries.rules            udev rules (deploy to udev rules.d on target system)
assets/ssgg.service                    systemd user unit
docs/development/                      Protocol reverse-engineering notes (historical)
tests/                                 Integration tests (cors_security, device_readback)
```

---

## Source-of-truth files

When prose docs and code disagree, always trust these files:

| File | What it controls |
|------|-----------------|
| `Cargo.toml` | dependency versions, features, MSRV, edition |
| `rust-toolchain.toml` | toolchain channel and components |
| `.github/workflows/ci.yml` | exact CI commands and feature matrix |
| `src/devices/hid_reports.rs` | HID command codes and report layouts |

---

## Toolchain and CI

**Toolchain**: `rust-toolchain.toml` pins `channel = "stable"`. CI jobs explicitly pin **1.97.1** (via the `dtolnay` rust-toolchain action). MSRV declared in `Cargo.toml`: **1.97.1**.

> Verify the toolchain by reading `rust-toolchain.toml` and `.github/workflows/ci.yml` before quoting any version — memory is unreliable here.

### Local commands

Run the smallest relevant subset for any given change.

| Step | Command |
|------|---------|
| Format check | `cargo fmt --all -- --check` |
| Lint | `cargo clippy --all-targets --locked -- -D warnings` |
| Test | `cargo test --locked` |
| Release build | `cargo build --release --locked` |

Add `--features <flag>` when touching feature-gated code.

### CI feature matrix

| Job | Feature variants |
|-----|-----------------|
| **fmt** | default only |
| **clippy** | `""`, `--features sonar`, `--features audio` |
| **test** | `""`, `--features sonar` (audio excluded — needs `libpulse-dev`) |
| **build** | `""`, `--features sonar`, `--features audio` |

The `audio` feature requires `libpulse-dev` on Debian/Ubuntu (`sudo apt-get install -y libpulse-dev`).

---

## Feature flags

| Flag | Enables | Extra dep |
|------|---------|-----------|
| *(none)* | Core RGB, GameSense, profiles | — |
| `audio` | AudioMixer, PulseAudio/PipeWire backend | `libpulse-dev` |
| `sonar` | SonarClient HTTP integration | — (reqwest already in tree) |
| `experimental-apex-2023` | Apex Pro TKL 2023 direct per-key RGB | — |

`audio` and `sonar` are independent; do not couple them. `experimental-apex-2023` is behind a feature flag until validated on hardware — do not present it as production-ready.

---

## Hard constraints

1. **HID reports**: always use `HidReportBuilder` and typed helpers in `src/devices/hid_reports.rs`. Never build raw byte arrays by hand.
2. **hidapi pin**: keep `hidapi = "=2.6.6"` exactly; changing it requires explicit task justification.
3. **GameSense CORS**: enforce localhost-only origin; the policy may only be tightened, never relaxed.
4. **Propagate errors**: use `?` or return an explicit `Err` value — no `unwrap` or `expect` in production paths.
5. **Error types**: `crate::error::Error` with `thiserror` at library boundaries; `anyhow` with `context()` in `src/main.rs` and binaries.
6. **Protocol accuracy**: `experimental-apex-2023` and `PerKeyRgb (0x23)` are reverse-engineered and unverified. Label new protocol work clearly; never state it as confirmed unless tested on hardware.
7. **Minimal scope**: fix what the task asks, nothing more. No opportunistic refactors.
8. **No mutable global state**, no panics in non-test code (unless explicitly fatal and documented), no ignored `Result`s.
9. **`unsafe` blocks** must document the safety invariant inline.

---

## Rust conventions

- Naming: `snake_case` for functions and modules, `PascalCase` for types and traits, `SCREAMING_SNAKE_CASE` for constants.
- `rustfmt.toml`: edition 2024 style, max 120 columns, 4-space indentation.
- Prefer `&T` borrows over owned values when ownership is not needed.
- New dependencies require justification; avoid heavy transitive deps.
- No comments that restate what the code does — comment only non-obvious invariants or workarounds.

---

## File-reading strategy

Large files cost tokens. Always search before reading:

```bash
rg 'TypeName\|fn_name' src/     # locate the symbol first
# then read only the relevant lines
```

Read only the lines you need. Use `rg --files src/` or `ls src/` for structure discovery; use targeted `rg` before opening any large file (`src/main.rs`, `src/devices/hid_reports.rs`).

---

## Protocol research notes

- `docs/development/` contains historical reverse-engineering notes. These may be outdated; verify against current source before acting on them.
- `CommandCode::PerKeyRgb (0x23)` is a placeholder; actual per-key command is unknown.
- `CommandCode::Apex2023Direct (0x40)` is experimental (see `src/devices/keyboards/apex_pro_tkl_2023.rs`).
- `CommandCode::ActuationControl (0x2D)` is experimental.

---

## Active backlog (as of 2026-06-15)

| Item | Status | Notes |
|------|--------|-------|
| Audio hang (#120) | Done | 5s timeout added to `PulseHandler::new()` in `src/audio/pulse.rs` |
| Issue #6 (Arch compile) | Stalled | No activity since Jan 2026; needs Arch environment to reproduce |
| Apex Pro TKL 2023 per-key RGB | In progress | `0x40` path is experimental; real protocol unconfirmed |
| Actuation read-back | In progress | Firmware command unknown; use `src/bin/discover_actuation.rs` to probe |

---

## Key reminders

- **HID reports**: always use `HidReportBuilder` — never build raw byte arrays by hand.
- **GameSense CORS**: keep the localhost-only origin policy; tightening is fine, loosening is not.
- **Toolchain and CI facts**: always read `rust-toolchain.toml` and `.github/workflows/ci.yml` — treat memory as unreliable.
- **Scope discipline**: implement exactly what the task asks; leave surrounding code untouched.
- **Documents**: only create planning or analysis documents when explicitly requested.
