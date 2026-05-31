# steelseriesgg-rs — Agent Handbook

Open-source SteelSeries GG replacement for Linux. Controls SteelSeries keyboards and headsets: RGB lighting, GameSense-compatible HTTP server, profiles, and optional audio or Sonar integration.

---

## Quick orientation

```
src/main.rs                    CLI entry point (clap, subcommands)
src/lib.rs                     Public crate root; re-exports prelude
src/error.rs                   crate::error::Error (thiserror) + Result alias
src/devices/hid_reports.rs     Type-safe HID report builder — use this, not raw bytes
src/devices/key_mapping.rs     Key address and ID types
src/devices/zone_mapping.rs    Zone to HID mapping
src/devices/keyboards/apex.rs  Per-keyboard protocol implementations
src/devices/headsets/mod.rs    Headset protocol implementations
src/devices/discovery.rs       Hot-plug and device fingerprinting
src/devices/diagnostics.rs     Runtime diagnostics
src/rgb/mod.rs                 Color, Effect, RgbController, PerKeyEffect
src/gamesense/mod.rs           Axum HTTP server (port 27301), GameSense handlers
src/audio/mod.rs               AudioMixer (pulse.rs), SonarClient (sonar.rs)
src/profiles/mod.rs            Save and load device state as TOML
src/config/mod.rs              ~/.config/ssgg/config.toml parsing
src/validation.rs              RgbValidator, ValidationReport
src/performance.rs             PerformanceManager
src/bin/                       One-shot helper binaries (see below)
assets/99-steelseries.rules    udev rules
assets/ssgg.service            systemd user unit
docs/development/              Protocol reverse-engineering notes (historical)
tests/                         Integration tests (cors_security, device_readback)
```

Helper binaries (not part of the primary CLI):
- `src/bin/discover_actuation.rs` — probe actuation firmware commands
- `src/bin/verify_key_mapping.rs` — map validation on hardware
- `src/bin/benchmark_rgb_alloc.rs` and `src/bin/benchmark_fragment.rs` — allocation benchmarks
- `src/bin/sonar_control.rs` — Sonar HTTP API exerciser (feature `sonar`)

---

## Source-of-truth files

When prose docs and code disagree, trust these files:

| File | What it controls |
|------|-----------------|
| `Cargo.toml` | dependency versions, features, edition |
| `rust-toolchain.toml` | pinned channel (1.95.0), components |
| `.github/workflows/ci.yml` | exact CI commands and feature matrix |
| `src/devices/hid_reports.rs` | HID command codes and report layouts |

---

## Toolchain and CI

**Toolchain**: Rust 1.95.0 stable (pinned in `rust-toolchain.toml`).
MSRV declared in `Cargo.toml` (`rust-version`): 1.94.1.

| Step | Command |
|------|---------|
| Format check | `cargo fmt --all -- --check` |
| Lint | `cargo clippy --all-targets --locked -- -D warnings` |
| Test | `cargo test --locked` |
| Release build | `cargo build --release --locked` |

CI feature matrix per job:
- **fmt**: default features only
- **clippy** and **build**: `""`, `--features sonar`, `--features audio`
- **test**: `""`, `--features sonar` (audio excluded — requires `libpulse-dev`)

The `audio` feature requires `libpulse-dev` on Debian or Ubuntu.

**Run the smallest relevant subset** for any given change. For a pure Rust change, full matrix is overkill; for a feature-gated module, include its feature flag.

---

## Feature flags

| Flag | Enables | Extra dep needed |
|------|---------|-----------------|
| *(none)* | Core RGB, GameSense, profiles | — |
| `audio` | AudioMixer, PulseAudio or PipeWire | `libpulse-dev` |
| `sonar` | SonarClient HTTP integration | — (reqwest) |
| `experimental-apex-2023` | Apex Pro TKL 2023 direct per-key RGB | — |

`audio` and `sonar` are independent; do not couple them. `experimental-apex-2023` is behind a feature flag until validated on hardware — do not present it as production-ready.

---

## Hard constraints

1. **HID reports**: always use `HidReportBuilder` and existing typed helpers in `src/devices/hid_reports.rs`. Never build raw byte arrays by hand.
2. **hidapi pin**: `hidapi = "=2.6.6"` — do not change unless the task explicitly requires it and you can justify the change.
3. **GameSense CORS**: localhost-only origin policy. Do not loosen it.
4. **No `.unwrap()` or `.expect()` in production paths** — return `Err(...)` or propagate with `?`.
5. **Error types**: `crate::error::Error` with `thiserror` at library boundaries; `anyhow` with `.context(...)` in `src/main.rs` and binaries.
6. **Protocol accuracy**: `experimental-apex-2023` and `PerKeyRgb (0x23)` are reverse-engineered and unverified. Label any new protocol work clearly; do not state it as confirmed unless tested on hardware.
7. **Minimal scope**: fix what the task asks, nothing more. No opportunistic refactors.

---

## Rust conventions

- Naming: `snake_case` for functions and modules, `PascalCase` for types and traits, `SCREAMING_SNAKE_CASE` for constants.
- `rustfmt.toml`: rustfmt `edition = "2024"` style (distinct from Cargo's `edition = "2021"`), max 120 columns, 4-space indentation.
- Prefer `&T` borrows over owned values when ownership is not needed.
- No panics in non-test code unless explicitly fatal and documented.
- No mutable global state.
- No ignoring `Result` values.
- `unsafe` blocks must document the safety invariant inline.
- New dependencies require justification; avoid heavy transitive deps.

---

## Active backlog (as of 2026-03-27)

| Item | Status | Notes |
|------|--------|-------|
| Audio hang (#120) | Done | 5s timeout added to `PulseHandler::new()` in `src/audio/pulse.rs` |
| Issue #6 (Arch compile) | Stalled | No activity since Jan 2026; needs Arch environment to reproduce |
| Apex Pro TKL 2023 per-key RGB | In progress | `0x40` path is experimental; real protocol unconfirmed |
| Actuation read-back | In progress | Firmware command unknown; use `src/bin/discover_actuation.rs` binary to probe |

---

## File-reading strategy

Large files cost tokens. Use targeted reads:

```bash
rg 'pattern' src/            # find relevant line first
# then read ~20 lines around it
```

Avoid reading entire large files (`src/main.rs`, `src/devices/hid_reports.rs`) unless you need the whole picture. Use `rg --files` or `ls` for structure discovery.

---

## Protocol research notes

- `docs/development/` contains historical reverse-engineering notes. These may be outdated; verify against current source before acting on them.
- `CommandCode::PerKeyRgb (0x23)` is a placeholder; actual per-key command is unknown.
- `CommandCode::Apex2023Direct (0x40)` is experimental (see `experimental-apex-2023` feature).
- `CommandCode::ActuationControl (0x2D)` is experimental.

---

## What NOT to do

- Do not duplicate large blocks of context from this file into responses.
- Do not state toolchain versions, feature relationships, or CI commands from memory — read the source-of-truth files.
- Do not replace `HidReportBuilder` with ad-hoc byte arrays.
- Do not add features, error handling, or abstractions beyond what the task requires.
- Do not add comments that restate what the code already says; only comment non-obvious invariants or workarounds.
- Do not create planning or analysis documents unless the user requests one.
