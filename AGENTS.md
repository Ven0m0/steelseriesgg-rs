# AGENTS.md - AI Agent Quick Reference

Comprehensive guide for AI assistants (Claude, Gemini, Copilot, etc.) working with **steelseriesgg-rs**.

---

## Project

**steelseriesgg-rs** is a complete open-source replacement for SteelSeries GG on Linux: RGB lighting control, GameSense HTTP server, and audio management for SteelSeries keyboards and headsets.

**Primary language**: Rust (100% application code)
**Edition**: 2021 (formatted with edition 2024 style)
**Toolchain**: 1.93.1 (pinned in `rust-toolchain.toml`)
**Binary**: `ssgg` — unified CLI + daemon
**Library**: `steelseries_gg` — public API crate
**License**: MIT

### Technology Stack

| Component | Crate | Version |
|-----------|-------|---------|
| Async runtime | tokio | 1.49+ (rt-multi-thread, macros, signal, fs) |
| HTTP server | axum | 0.8+ |
| HID communication | hidapi | **=2.6.5** (pinned — do not change) |
| CLI parsing | clap | 4.5+ (derive API) |
| Serialization | serde + serde\_json + toml | 1.0 / 1.0 / 1.0 |
| Error handling | thiserror (lib) + anyhow (bin) | 2.0 / 1.0 |
| Logging | tracing + tracing-subscriber | 0.1 / 0.3 |
| Locks | parking\_lot | 0.12 |
| HTTP middleware | tower-http (CORS) | 0.6 |

### Feature Flags

| Flag | Adds | Extra system dep |
|------|------|-----------------|
| `default` | (none) | — |
| `audio` | libpulse-binding 2.30 | libpulse-dev |
| `sonar` | reqwest 0.13 | — |

```bash
cargo build                      # default
cargo build --features audio     # with PulseAudio
cargo build --features sonar     # with Sonar HTTP client
cargo build --all-features       # everything
```

---

## Structure

```
steelseriesgg-rs/
├── @Cargo.toml                   # Package manifest, all deps, release profile
├── rust-toolchain.toml           # Pinned toolchain: 1.93.1 + rustfmt + clippy
├── rustfmt.toml                  # Edition 2024, max_width 120, Unix newlines
├── .editorconfig                 # 4 spaces for .rs, 2 spaces elsewhere
│
├── @src/
│   ├── @main.rs                  # CLI entry point (~3300 LOC, 15+ subcommands)
│   ├── @lib.rs                   # Library root: module declarations + prelude
│   ├── @error.rs                 # Error enum (thiserror) + Result alias
│   │
│   ├── @devices/                 # Hardware abstraction layer
│   │   ├── @mod.rs               # Device trait, DeviceInfo, product IDs
│   │   ├── @discovery.rs         # DeviceManager — hidapi enumeration
│   │   ├── @hid_reports.rs       # HidReportBuilder (type-safe 65/64-byte reports)
│   │   ├── diagnostics.rs        # Health checks & HID logging
│   │   ├── key_mapping.rs        # Per-key addressing & keyboard layouts
│   │   ├── zone_mapping.rs       # RGB zone definitions
│   │   ├── fuzz.rs               # Protocol fuzzing (dev tool)
│   │   ├── keyboards/
│   │   │   ├── @mod.rs           # Keyboard trait (25+ methods)
│   │   │   ├── apex.rs           # Generic Apex implementations
│   │   │   └── @apex_pro_tkl_2023.rs  # PID 0x1628 — primary device
│   │   └── headsets/
│   │       └── mod.rs            # Headset implementations
│   │
│   ├── @rgb/
│   │   ├── @mod.rs               # Color, Effect, EffectEngine, RgbController
│   │   └── tests.rs              # 11 unit tests
│   │
│   ├── @gamesense/
│   │   ├── mod.rs                # GameMetadata, GameEvent structs
│   │   ├── @server.rs            # Axum HTTP server (port 27301)
│   │   └── @handlers.rs          # CORS-enabled request handlers
│   │
│   ├── profiles/
│   │   ├── mod.rs                # Profile struct & management
│   │   └── tests.rs              # Serialization tests
│   ├── config/mod.rs             # Config (~/.config/ssgg/config.toml)
│   ├── audio/                    # feature = "audio" | "sonar"
│   │   ├── mod.rs                # AudioMixer trait
│   │   ├── pulse.rs              # PulseAudio integration
│   │   └── sonar.rs              # SonarClient HTTP API
│   ├── bin/                      # Utility binaries
│   │   ├── discover_actuation.rs # Actuation point discovery
│   │   ├── sonar_control.rs      # Sonar control (requires sonar feature)
│   │   ├── verify_key_mapping.rs # Per-key RGB verification (dev)
│   │   └── benchmark_rgb_alloc.rs # RGB allocation benchmark (dev)
│   ├── performance.rs            # Real-time stats & adaptive timing
│   ├── validation.rs             # Resource leak detection
│   ├── device_state.rs           # Async state persistence (JSON)
│   ├── diagnostics_export.rs     # Bug report generation
│   └── pollrate.rs               # USB poll rate via sysfs
│
├── tests/
│   ├── device_readback.rs        # Integration tests
│   └── cors_security.rs          # CORS security tests
│
├── assets/
│   ├── 99-steelseries.rules      # udev rules (USB permissions)
│   └── ssgg.service              # systemd user service
│
├── .github/
│   ├── workflows/
│   │   ├── @ci.yml               # Format + Clippy + Test + Build (matrix)
│   │   └── release-arch.yml      # Arch Linux package release
│   ├── copilot-instructions.md   # GitHub Copilot guardrails (subset of this file)
│   └── dependabot.yml            # Weekly dep updates
│
└── docs/
    ├── development/              # Protocol research & dev notes
    │   ├── APEX_PRO_PROTOCOL.md
    │   ├── KEY_MAPPING_RESEARCH.md
    │   ├── PROTOCOL_RESEARCH.md
    │   ├── RGB_CONTROL_ANALYSIS.md
    │   └── OPTIMIZATION_REPORT.md
    └── archive/                  # Historical docs
```

---

## Dev Workflow

### System Dependencies

```bash
# Debian/Ubuntu
sudo apt-get install -y libudev-dev libhidapi-dev
# + audio feature: libpulse-dev

# Fedora
sudo dnf install systemd-devel hidapi-devel

# Arch
sudo pacman -S hidapi
```

### Build

```bash
cargo build                        # debug (~5-10s)
cargo build --release              # optimized, LTO, stripped (~30-60s, ~2-3 MB)
cargo build --features audio       # with PulseAudio
cargo build --all-features         # all optional features
```

### Test

```bash
cargo test                         # ~77 unit tests
cargo test --all-features
cargo test rgb::tests              # specific module
cargo test test_color_blending     # specific test
cargo test -- --nocapture          # show println! output
cargo test -- --test-threads=1     # sequential (debugging)
```

### Code Quality (required before every commit)

```bash
cargo fmt                                          # 1. format
cargo clippy --all-features -- -D warnings         # 2. lint (zero warnings)
cargo test --all-features                          # 3. test
cargo build --release                              # 4. build
```

### Install & Run

```bash
# udev rules (one-time)
sudo cp assets/99-steelseries.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
sudo usermod -aG input $USER   # log out/in to apply

# Run daemon
cargo run --release -- daemon

# Debug
RUST_LOG=debug cargo run -- daemon
RUST_LOG=steelseries_gg::devices=debug cargo run -- devices
```

---

## Conventions

### Naming

| Item | Style | Example |
|------|-------|---------|
| Functions / variables | `snake_case` | `set_rgb_color`, `device_manager` |
| Types / structs / enums | `PascalCase` | `DeviceManager`, `RgbController` |
| Constants | `SCREAMING_SNAKE_CASE` | `STEELSERIES_VENDOR_ID`, `MAX_RGB_ZONES` |
| Modules | `snake_case` | `devices`, `gamesense`, `rgb` |

### Formatting

- **4 spaces** indentation for Rust (`.editorconfig` + `rustfmt.toml`)
- **120 chars** max line length (`rustfmt.toml: max_width = 120`)
- **Unix LF** newlines
- `cargo fmt` enforced in CI — must pass before merge

### Error Handling

```rust
// Library code → thiserror structured errors
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DeviceError {
    #[error("Device not found: {0}")]
    NotFound(String),
    #[error("HID error: {0}")]
    HidError(#[from] hidapi::HidError),
}

// Binary/CLI code → anyhow for context
use anyhow::{Context, Result};

fn cmd_devices() -> Result<()> {
    let manager = DeviceManager::new().context("Failed to init device manager")?;
    Ok(())
}

// NEVER use .unwrap() or .expect() in production code — always use ?
```

### HID Reports — Always Use the Builder

```rust
// CORRECT — type-safe, correct sizing
let report = HidReportBuilder::new(HidDeviceType::Keyboard)
    .command(CommandCode::RgbControl)
    .zone_data(zone, &color)
    .build()?;
device.send_report(&report)?;

// WRONG — manual buffer (fragile, wrong sizes)
let mut buf = [0u8; 65];
buf[0] = 0x00;
buf[1] = 0x21;
```

**Report sizes**: Keyboards = 65 bytes (with report ID), Headsets = 64 bytes (no report ID)

### Async Patterns

```rust
use tokio::sync::{Mutex, RwLock};

// Shared state
let state = Arc::new(Mutex::new(data));

// Background tasks
tokio::spawn(async move {
    loop {
        do_work().await;
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
});

// Axum HTTP server
let app = Router::new()
    .route("/game_event", post(handle_game_event))
    .layer(CorsLayer::permissive())
    .with_state(state);
let listener = TcpListener::bind("127.0.0.1:27301").await?;
axum::serve(listener, app).await?;
```

### RGB Effect Engine

```rust
let mut engine = EffectEngine::new(Effect::Breathing {
    color: Color::new(0, 255, 255),
    speed: 2.0,
});

// Cached if Δt < 16ms (~60 FPS)
let colors = engine.compute(num_zones, elapsed);
// IMPORTANT: cache only activates after first call (last_compute_time != Duration::ZERO)
```

Supported effects: `Static`, `Breathing`, `Spectrum`, `Wave`, `Reactive`, `Gradient`, `Custom`, `Off`

### Import Style

```rust
// Standard: grouped, reordered by rustfmt automatically
use std::sync::Arc;
use anyhow::{Context, Result};
use tokio::sync::Mutex;
use crate::devices::DeviceManager;
```

---

## Critical Constants

```rust
pub const STEELSERIES_VENDOR_ID: u16        = 0x1038;
pub const APEX_PRO_TKL_2023_PRODUCT_ID: u16 = 0x1628;  // NOT 0x1618!
pub const KEYBOARD_REPORT_SIZE: usize        = 65;       // includes report ID
pub const HEADSET_REPORT_SIZE: usize         = 64;       // no report ID
pub const MAX_RGB_ZONES: usize               = 12;
pub const GAMESENSE_DEFAULT_PORT: u16        = 27301;
pub const CACHE_THRESHOLD_MS: u64            = 16;       // ~60 FPS
```

---

## Dependencies

### Core (all builds)

| Crate | Purpose |
|-------|---------|
| **hidapi =2.6.5** | HID device communication — **pinned, do not change** |
| **tokio 1.49** | Async runtime (multi-thread) |
| **axum 0.8** | GameSense HTTP server |
| **clap 4.5** | CLI argument parsing (derive) |
| **serde + serde\_json + toml** | Serialization: JSON state, TOML config/profiles |
| **thiserror 2.0** | Library error types |
| **anyhow 1.0** | Binary error context |
| **tracing + tracing-subscriber** | Structured logging with env-filter |
| **parking\_lot 0.12** | High-perf Mutex/RwLock |
| **tower-http 0.6** | CORS middleware for GameSense |
| **directories 6.0** | XDG base dirs (~/.config/ssgg/) |
| **sysinfo 0.38** | System info for diagnostics |
| **chrono 0.4** | Timestamps with serde |
| **colored 3.1** | Terminal colors |
| **tabled 0.20** | Formatted table output |
| **indicatif 0.18** | Progress bars |
| **libc 0.2** | `geteuid` root checks |
| **async-trait 0.1** | Async in traits |

### Optional

| Feature | Crate | Purpose |
|---------|-------|---------|
| `audio` | libpulse-binding 2.30 | PulseAudio mixer |
| `sonar` | reqwest 0.13 | SteelSeries Sonar HTTP client |

### Dev

| Crate | Purpose |
|-------|---------|
| tempfile 3.10 | Temporary files in tests |

---

## Common Tasks

### Add a new CLI subcommand

1. Add variant to the `Commands` enum in `src/main.rs`
2. Add arm to the `match cli.command` block
3. Implement the handler function (returns `Result<()>`)
4. Follow existing patterns: use `anyhow::Context`, print with `colored`

### Add a new RGB effect

1. Add variant to `Effect` enum in `src/rgb/mod.rs`
2. Implement computation in `EffectEngine::compute()`
3. Add serialization/deserialization (serde `rename_all`)
4. Add unit test in `src/rgb/tests.rs`
5. Expose in CLI (`src/main.rs` effect parsing)

### Add a new device

1. Create `src/devices/keyboards/<device>.rs` or `headsets/<device>.rs`
2. Add `pub const <NAME>_PRODUCT_ID: u16 = 0xXXXX;` in `src/devices/mod.rs`
3. Implement `Device` trait (required) and `Keyboard`/`Headset` trait
4. Register in `DeviceManager::open_device()` in `src/devices/discovery.rs`

### Add a test

```rust
// Unit test — co-located with source, in module's tests.rs or inline
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_my_feature() {
        let result = my_function();
        assert_eq!(result, expected);
    }
}
```

### Update dependencies

```bash
cargo update              # update within Cargo.toml semver constraints
cargo outdated            # check for newer versions (cargo-outdated)
cargo audit               # security vulnerability check (cargo-audit)
# Always re-run cargo test after dependency updates
```

---

## CI/CD

### What runs on every PR and push to `main`

**File**: `.github/workflows/ci.yml`

| Job | Matrix | Command |
|-----|--------|---------|
| **fmt** | — | `cargo fmt --all -- --check` |
| **clippy** | `""`, `--features audio`, `--features sonar` | `cargo clippy --all-targets --locked -- -D warnings` |
| **test** | `""`, `--features sonar` | `cargo test --locked` |
| **build** | `""`, `--features audio`, `--features sonar` | `cargo build --release --locked` |

System deps installed in CI: `libudev-dev libhidapi-dev` (+ `libpulse-dev` for audio jobs)
Rust cache: `Swatinem/rust-cache@v2` per feature variant

**All jobs must pass** before a PR can be merged.

### Release

**File**: `.github/workflows/release-arch.yml` — triggered on version tags, produces Arch Linux package.

### Dependency automation

**File**: `.github/dependabot.yml` — weekly updates for GitHub Actions, Cargo deps, Rust toolchain.

---

## Tool Preferences

| Tool | Setting |
|------|---------|
| **Formatter** | `cargo fmt` (rustfmt edition 2024, max\_width 120) |
| **Linter** | `cargo clippy -- -D warnings` (zero-warnings policy) |
| **Package manager** | `cargo` (Rust standard) |
| **Runtime** | Tokio multi-thread |
| **Test runner** | `cargo test` |
| **Logging env var** | `RUST_LOG=debug\|trace\|info` |

---

## Debugging & Diagnostics

```bash
# Logging
RUST_LOG=debug cargo run -- devices
RUST_LOG=trace cargo run -- daemon
RUST_LOG=steelseries_gg::devices=debug cargo run -- devices
cargo run -- --debug-hid devices       # HID-level diagnostics

# Device checks
cargo run -- devices                   # list connected devices
cargo run -- validate                  # resource leak / validation tests
cargo run -- test-device "Apex Pro TKL (2023)"
cargo run -- verify-performance        # RGB performance metrics
cargo run -- bug-report --output report.json
```

### Common Issues

| Problem | Cause | Fix |
|---------|-------|-----|
| Device not found | udev rules missing | Install rules, reload, re-trigger |
| Permission denied | Not in `input` group | `sudo usermod -aG input $USER` + re-login |
| RGB not working | Wrong interface | Interface 1 (keyboards), 3 (headsets); check `RUST_LOG=debug` |
| Per-key RGB no-op | Protocol not reversed | Use zone fallback `simulate_per_key_with_zones()` |
| GameSense silent | Port 27301 blocked | Check `netstat -tuln`, firewall |
| High CPU | Effect timing | Check perf stats, reduce speed, enable caching |
| Audio missing | Feature not enabled | Build with `--features audio`, ensure PulseAudio running |
| Service not auto-starting | linger disabled | `sudo loginctl enable-linger $USER` |

---

## Gotchas & Pitfalls

1. **hidapi is pinned at `=2.6.5`** — exact version, do not loosen the constraint.
2. **Apex Pro TKL 2023 PID is `0x1628`** — not `0x1618`; hardware-verified.
3. **Interface numbers matter**: keyboards use interface 1, headsets interface 3.
4. **CLI effects are one-shot** — animations require daemon mode (`ssgg daemon`).
5. **RGB cache first-call**: `EffectEngine::compute()` cache only activates after the first call (when `last_compute_time != Duration::ZERO`). First call always computes.
6. **Per-key RGB (`0x2A`) is a placeholder** — actual protocol unknown; use zone fallback.
7. **Actuation read unimplemented** — write works (`0x2D`), read command unknown.
8. **Some PIDs are ambiguous** — `0x12AD` = Arctis 1 or Arctis 7 2017; device name reflects this.
9. **`sonar` feature implicitly needs audio** — use `--features audio,sonar` or `--all-features`.
10. **HID byte order**: SteelSeries uses RGB order (not BGR); verify if colors appear wrong.
11. **Never skip `cargo fmt`** — CI will fail on any formatting diff.
12. **`rustfmt.toml` max_width is 120** — not 100; the `.editorconfig` Rust line is a soft guide.

---

## Configuration

### Config file: `~/.config/ssgg/config.toml`

```toml
[gamesense]
enabled = true
bind = "127.0.0.1"
port = 27301

[audio]
master_volume = 100
game_volume = 100
chat_volume = 100
media_volume = 100
aux_volume = 100
mic_volume = 100

[general]
default_profile = "default"
debug = false
log_level = "info"
```

### Profiles: `~/.config/ssgg/profiles/<name>.toml`

```toml
[keyboard]
rgb_effect = "Static"
rgb_color = "#FF0000"
brightness = 80
poll_rate = 1000

[headset]
rgb_effect = "Breathing"
rgb_color = "#00FFFF"
brightness = 100
```

### State: `~/.config/ssgg/state.json`

Async-persisted device state for daemon mode (last-applied settings).

---

## Release Build Profile

```toml
[profile.release]
strip = true            # ~30% size reduction
lto = "fat"             # ~15% perf gain
codegen-units = 1       # better optimization
panic = "abort"         # ~10% size reduction
opt-level = 3
debug = 0
overflow-checks = false
```

Result: ~2–3 MB binary, <100ms startup, <5ms RGB update latency, ~10–20 MB RSS.

---

## Git Conventions

### Commit message format

```
<type>: <short description>

[optional body]
```

Types: `feat`, `fix`, `refactor`, `docs`, `test`, `perf`, `chore`, `style`

Examples:
```
feat: add per-key RGB fallback using zones
fix: RGB caching returns black on first compute
perf: reduce HID communication overhead by 20%
docs: update AGENTS.md with accurate hidapi version
```

### Pre-commit checklist

```bash
cargo fmt
cargo clippy --all-features -- -D warnings
cargo test --all-features
cargo build --release
```

---

## Current Development Focus

### 1. Per-Key RGB (Apex Pro TKL 2023)

**Status**: Protocol reverse-engineering in progress
**Blocker**: HID command `0x2A` is a placeholder; actual sequence unknown
**Interim**: Zone-based fallback working via `simulate_per_key_with_zones()`
**Docs**: `docs/development/KEY_MAPPING_RESEARCH.md`

### 2. Actuation Point Read

**Status**: Write works (`set_actuation_point()` via `0x2D`), read not implemented
**Blocker**: HID read command not discovered
**Scope**: Apex Pro series only

---

## Additional Documentation

| File | Purpose |
|------|---------|
| `README.md` | User installation & usage guide |
| `CONTRIBUTING.md` | Contribution guidelines |
| `PLAN.md` | Development roadmap |
| `PROJECT_INDEX.md` | Full module/export index |
| `PERFORMANCE_OPTIMIZATIONS.md` | Benchmark findings |
| `docs/development/APEX_PRO_PROTOCOL.md` | HID protocol research |
| `docs/development/KEY_MAPPING_RESEARCH.md` | Per-key addressing research |
| `docs/development/PROTOCOL_RESEARCH.md` | General protocol findings |
| `docs/development/RGB_CONTROL_ANALYSIS.md` | RGB control deep-dive |
| `docs/development/OPTIMIZATION_REPORT.md` | Performance optimization notes |

---

**Version**: 0.1.0
**Last updated**: 2026-02-28
**License**: MIT
