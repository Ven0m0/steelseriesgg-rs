# AGENTS.md - AI Agent Quick Reference

Comprehensive guide for AI assistants working with **steelseriesgg-rs**.

## Project Overview

**steelseriesgg-rs** is an open-source SteelSeries GG replacement for Linux, providing RGB lighting control, GameSense server, and audio management for SteelSeries keyboards and headsets.

### Stack

- **Language**: Rust 2021 edition (1.70+)
- **Binary**: `ssgg` (CLI + daemon)
- **Library**: `steelseries_gg`
- **Framework**: Axum (HTTP server), Tokio (async runtime)
- **HID**: hidapi 2.6.4
- **License**: MIT
- **Repository**: https://github.com/Ven0m0/steelseriesgg-rs

### Core Features

- RGB lighting (7+ effects: static, breathing, spectrum, wave, reactive, gradient, custom)
- GameSense HTTP server (port 27301, compatible with SteelSeries API)
- Profile management (TOML-based)
- Performance monitoring & validation
- Audio mixer (optional `audio` feature)
- Sonar API client (optional `sonar` feature)
- Per-key RGB (experimental, protocol research ongoing)

### Feature Flags

| Flag | Description | Dependencies |
|------|-------------|--------------|
| `default` | RGB + GameSense only | Core |
| `audio` | PulseAudio integration | libpulse-binding |
| `sonar` | SteelSeries Sonar API | reqwest |

---

## Repository Structure

### Key Files (@ = critical for understanding)

```
steelseriesgg-rs/
├── @CLAUDE.md                    # Comprehensive developer guide (28KB)
├── @README.md                    # User documentation
├── @Cargo.toml                   # Dependencies & build config
├── @src/
│   ├── @main.rs                  # CLI entry (121KB, 15+ commands)
│   ├── @lib.rs                   # Library entry & module declarations
│   ├── @error.rs                 # Error types (thiserror)
│   │
│   ├── @devices/                 # Hardware communication layer
│   │   ├── mod.rs                # Device trait, discovery, product IDs
│   │   ├── hid_reports.rs        # HID report builders (65-byte protocol)
│   │   ├── keyboards/
│   │   │   ├── mod.rs            # Keyboard trait (25+ methods)
│   │   │   └── apex_pro_tkl_2023.rs  # Primary device (PID 0x1628)
│   │   └── headsets/
│   │       └── mod.rs            # Headset implementations
│   │
│   ├── @rgb/                     # Color & effects engine
│   │   └── mod.rs                # Color, Effect, EffectEngine, RgbController
│   │
│   ├── @gamesense/               # GameSense HTTP API
│   │   ├── mod.rs                # Data structures
│   │   ├── server.rs             # Axum server
│   │   └── handlers.rs           # Event handlers
│   │
│   ├── performance.rs            # Real-time monitoring
│   ├── validation.rs             # Resource leak detection
│   ├── device_state.rs           # Async state persistence
│   ├── profiles/                 # TOML profiles
│   ├── audio/                    # PulseAudio mixer (optional)
│   └── config/                   # User config (~/.config/ssgg/)
│
├── .editorconfig                 # Editor settings (4 spaces for Rust)
├── rustfmt.toml                  # Rust formatting (100 char max)
├── .github/
│   ├── workflows/ci.yml          # CI pipeline (fmt, clippy, test, build)
│   └── copilot-instructions.md   # GitHub Copilot guardrails
│
└── docs/development/             # Protocol research & notes
```

---

## Development Workflows

### Setup

**Prerequisites:**
```bash
# Debian/Ubuntu
sudo apt install libudev-dev libhidapi-dev

# Fedora
sudo dnf install systemd-devel hidapi-devel

# Arch
sudo pacman -S hidapi
```

**Optional (audio feature):**
```bash
sudo apt install libpulse-dev        # Debian/Ubuntu
sudo dnf install pulseaudio-libs-devel  # Fedora
```

**Clone & Build:**
```bash
git clone https://github.com/Ven0m0/steelseriesgg-rs.git
cd steelseriesgg-rs
cargo build --release
```

### Build Commands

| Command | Purpose |
|---------|---------|
| `cargo build` | Debug build (~5-10s) |
| `cargo build --release` | Optimized release (LTO, strip, ~30-60s) |
| `cargo build --features audio` | With audio support |
| `cargo build --all-features` | All optional features |

### Testing

**Run tests:**
```bash
cargo test                        # ~77 unit tests (default features)
cargo test --all-features         # All feature combinations
cargo test rgb::tests             # Specific module
cargo test -- --nocapture         # Show test output
```

**Test coverage:**
- RGB: 11 tests (color, effects, caching)
- HID Reports: 19 tests (builders, encoding)
- Zone Mapping: 8 tests
- Keyboards: 7 tests
- Profiles: 6 tests
- Performance: 6 tests
- Validation: 6 tests

### Code Quality

**Before commit (REQUIRED):**
```bash
cargo fmt                         # Format code
cargo clippy --all-features       # Lint all feature paths
cargo test                        # Run tests
```

**CI pipeline checks:**
1. Format: `cargo fmt --check`
2. Clippy: `cargo clippy -- -D warnings` (matrix: default, audio, sonar)
3. Tests: `cargo test` (matrix: default, sonar)
4. Build: `cargo build --release` (matrix: all features)

### Deployment

**systemd user service:**
```bash
# After package install
systemctl --user daemon-reload
systemctl --user enable --now ssgg.service
journalctl --user -u ssgg.service -f

# Auto-start at boot (no login required)
sudo loginctl enable-linger $USER
```

**udev permissions:**
```bash
sudo cp assets/99-steelseries.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
sudo usermod -aG input $USER  # Log out/in to apply
```

---

## Code Conventions

### Naming

| Item | Convention | Example |
|------|------------|---------|
| Functions/vars | `snake_case` | `set_rgb_color()`, `device_manager` |
| Types/structs | `PascalCase` | `DeviceManager`, `RgbController` |
| Constants | `SCREAMING_SNAKE_CASE` | `STEELSERIES_VENDOR_ID`, `MAX_RGB_ZONES` |
| Modules | `snake_case` | `devices`, `gamesense`, `rgb` |

### Style

| Aspect | Standard |
|--------|----------|
| Indentation | 4 spaces (Rust), 2 spaces (others) |
| Line length | 100 characters max |
| Format | `cargo fmt` (rustfmt.toml: edition 2024) |
| Tabs | Spaces only (no hard tabs) |
| Newlines | Unix (LF) |
| Imports | Auto-reordered by rustfmt |

### Patterns

**Error handling:**
```rust
use anyhow::Result;  // For binaries
use thiserror::Error;  // For libraries

// Prefer Result over panic
fn operation() -> Result<()> {
    device.send_report()?;  // Propagate errors
    Ok(())
}
```

**Device trait implementation:**
```rust
pub trait Device: Send + Sync {
    fn info(&self) -> &DeviceInfo;
    fn initialize(&mut self) -> Result<()>;
    // ... 20+ methods
}
```

**HID reports (ALWAYS use builders):**
```rust
let report = HidReportBuilder::new(HidDeviceType::Keyboard)
    .command(CommandCode::RgbControl)
    .zone_data(zone, &color)
    .build()?;
device.send(&report)?;
```

**Effect engine with caching:**
```rust
let mut engine = EffectEngine::new(Effect::Breathing { color, speed: 2.0 });
let colors = engine.compute(num_zones, elapsed);  // Cached if Δt < 16ms
```

### Critical Constants

```rust
STEELSERIES_VENDOR_ID = 0x1038
KEYBOARD_REPORT_SIZE = 65  // With report ID
HEADSET_REPORT_SIZE = 64   // Without report ID
MAX_RGB_ZONES = 12
GAMESENSE_DEFAULT_PORT = 27301
CACHE_THRESHOLD_MS = 16    // ~60 FPS
```

---

## Dependencies

### Core (default)

| Crate | Version | Purpose |
|-------|---------|---------|
| hidapi | 2.6.4 | HID device communication |
| tokio | 1.49 | Async runtime (multi-thread) |
| axum | 0.8 | GameSense HTTP server |
| clap | 4.5 | CLI argument parsing |
| serde/serde_json | 1.0 | Serialization |
| toml | 0.9 | Config files |
| thiserror | 2.0 | Error types |
| tracing | 0.1 | Structured logging |
| colored | 3.1 | Terminal colors |
| chrono | 0.4 | Timestamps |

### Optional Features

| Feature | Crate | Purpose |
|---------|-------|---------|
| `audio` | libpulse-binding 2.30 | PulseAudio integration |
| `sonar` | reqwest 0.13 | HTTP client for Sonar API |

### Performance

**Release profile (Cargo.toml):**
```toml
[profile.release]
strip = true           # Remove debug symbols
lto = "fat"            # Full link-time optimization
codegen-units = 1      # Single unit (better optimization)
panic = "abort"        # No unwinding
opt-level = 3          # Maximum optimization
overflow-checks = false
```

**Result:** ~2-3 MB binary, excellent performance

---

## Common Tasks

### Device Operations

```bash
# List connected devices
RUST_LOG=debug cargo run -- devices

# Test RGB
cargo run -- rgb color red
cargo run -- rgb effect breathing --color cyan
cargo run -- rgb brightness 80

# Check per-key RGB support (experimental)
cargo run -- rgb perkey status
```

### Profile Management

```bash
# Save current settings
cargo run -- profile save gaming

# Load profile
cargo run -- profile load gaming

# List profiles
cargo run -- profile list
```

### Daemon & Server

```bash
# Run daemon (foreground)
cargo run --release -- daemon

# Start GameSense server only
cargo run -- server

# Check systemd service
systemctl --user status ssgg
journalctl --user -u ssgg -f
```

### Diagnostics

```bash
# Generate bug report
cargo run -- bug-report --output report.json

# View HID communication logs
cargo run -- hid-logs

# Run validation tests
cargo run -- validate

# Monitor performance
cargo run -- performance

# Protocol fuzzing (developer tool)
cargo run -- fuzz --start 0x20 --end 0x30
```

### Audio (with `--features audio`)

```bash
# View audio status
cargo run --features audio -- audio status

# Set volume
cargo run --features audio -- audio volume --channel master --level 75

# Mute/unmute
cargo run --features audio -- audio mute --channel game
```

### Sonar (with `--features sonar`)

```bash
# Discover Sonar port (dynamic)
cargo run --features sonar -- sonar discover

# Check status
cargo run --features sonar -- sonar status

# Control volume
cargo run --features sonar -- sonar volume game 100
```

---

## Debugging

### Logging

```bash
# Enable debug logs
RUST_LOG=debug cargo run -- devices

# Trace level (verbose)
RUST_LOG=trace cargo run -- daemon

# Enable HID diagnostics
cargo run -- --debug-hid devices
```

### Common Issues

| Issue | Solution |
|-------|----------|
| Device not detected | Check udev rules, USB connection, permissions |
| RGB not working | Verify interface number, check HID reports with `RUST_LOG=debug` |
| Per-key RGB not working | Protocol not reverse-engineered yet (use zone fallback) |
| GameSense not responding | Check port 27301, firewall rules |
| High CPU usage | Check performance stats, effect computation timing |
| Actuation read failing | Read command not implemented (write-only) |

### Diagnostic Commands

```bash
ssgg devices                    # List devices
ssgg status                     # Real-time status
ssgg hid-logs                   # HID communication
ssgg bug-report                 # Diagnostic report
ssgg validate                   # Run validation
ssgg test-device <device>       # Automated testing
```

---

## Important Gotchas

1. **HID Report Sizing**: Keyboards use 65 bytes (with report ID), headsets use 64 bytes (no report ID). Always use `HidReportBuilder`.

2. **Product ID**: Apex Pro TKL 2023 uses `0x1628` (not `0x1618` as in some docs).

3. **Interface Numbers**: Keyboards use interface 1, headsets use interface 3.

4. **Animated Effects**: CLI commands are one-shot. Use daemon mode for continuous animations.

5. **Per-Key RGB**: Command code `0x2A` is a placeholder. Protocol not yet reverse-engineered. Use zone-based fallback.

6. **Actuation Point**: Write works (`0x2D`), read not implemented.

7. **RGB Caching**: First `EffectEngine::compute()` always computes (cache check: `last_compute_time != Duration::ZERO`).

8. **udev Permissions**: Must be in `input` group and have udev rules installed.

---

## Additional Resources

- **Full Developer Guide**: `CLAUDE.md` (28KB comprehensive reference)
- **User Docs**: `README.md`
- **Contributing**: `CONTRIBUTING.md`
- **Project Structure**: `PROJECT_INDEX.md`
- **Roadmap**: `PLAN.md` (per-key RGB focus)
- **Performance**: `PERFORMANCE_OPTIMIZATIONS.md`
- **Protocol Research**: `docs/development/*.md`

---

## Development Focus (Current)

**Primary:** Per-key RGB control for Apex Pro TKL 2023
- Protocol reverse engineering ongoing
- Command code `0x2A` placeholder
- Zone-based fallback working as interim

**Secondary:** Actuation point control
- Write working (`0x2D`)
- Read command not discovered
- Limited to Apex Pro series

---

**Version**: 0.1.0
**Last Updated**: 2026-02-10
**Maintainer**: steelseriesgg-rs contributors
