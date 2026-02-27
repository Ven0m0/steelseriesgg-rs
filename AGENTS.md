# AGENTS.md - AI Agent Quick Reference

Comprehensive guide for AI assistants (Claude, Gemini, Copilot, etc.) working with **steelseriesgg-rs**.

## Project Overview

**steelseriesgg-rs** is a complete open-source replacement for SteelSeries GG on Linux, providing RGB lighting control, GameSense server, and audio management for SteelSeries keyboards and headsets.

### Technology Stack

| Component | Technology | Version |
|-----------|-----------|---------|
| **Language** | Rust | 2021 edition (1.70+) |
| **Binary** | `ssgg` | CLI + daemon |
| **Library** | `steelseries_gg` | Public API |
| **Async Runtime** | Tokio | 1.49+ (multi-thread) |
| **HTTP Server** | Axum | 0.8+ |
| **HID Layer** | hidapi | 2.6.4 (pinned) |
| **CLI** | clap | 4.5+ (derive API) |
| **Config** | TOML | 0.9+ |
| **Logging** | tracing | 0.1+ |
| **License** | MIT | - |

### Core Capabilities

- **RGB Control**: 7+ effects (static, breathing, spectrum, wave, reactive, gradient, custom)
- **Per-Key RGB**: Individual key addressing (experimental, protocol research ongoing)
- **Actuation Control**: Adjustable actuation points for Apex Pro keyboards (experimental)
- **GameSense Server**: HTTP API compatible with SteelSeries GameSense protocol (port 27301)
- **Performance Monitoring**: Real-time CPU, memory, latency tracking with adaptive timing
- **Resource Validation**: Automatic leak detection and performance regression testing
- **Profile Management**: Save/load device configurations with TOML
- **Diagnostic Tools**: HID logging, bug reports, device testing, protocol fuzzing
- **Audio Mixing**: PulseAudio integration (optional `audio` feature)
- **Sonar Integration**: SteelSeries Sonar HTTP client (optional `sonar` feature)

### Feature Flags

| Flag | Dependencies | Description |
|------|--------------|-------------|
| `default` | None | RGB control + GameSense server |
| `audio` | libpulse-binding 2.30 | PulseAudio mixer integration |
| `sonar` | reqwest 0.13 | SteelSeries Sonar API client |

**Build examples:**
```bash
cargo build                      # Default features
cargo build --features audio     # With audio
cargo build --all-features       # Everything
```

---

## Repository Structure

### Key Files (@ = critical for understanding)

```
steelseriesgg-rs/
├── @Cargo.toml                   # Dependencies, build config, release profile
├── @README.md                    # User documentation & installation guide
├── @AGENTS.md                    # This file (AI assistant guide)
├── @CLAUDE.md → AGENTS.md        # Symlink to this file
├── @GEMINI.md → AGENTS.md        # Symlink to this file
│
├── @src/
│   ├── @main.rs                  # CLI entry point (15+ commands, ~3000 LOC)
│   ├── @lib.rs                   # Library root, module declarations, prelude
│   ├── @error.rs                 # Error types (thiserror-based)
│   │
│   ├── @devices/                 # Hardware abstraction layer
│   │   ├── @mod.rs               # Device trait, DeviceInfo, product IDs (0x1038)
│   │   ├── @discovery.rs         # DeviceManager with hidapi enumeration
│   │   ├── @hid_reports.rs       # HID report builders (65/64-byte protocol)
│   │   ├── diagnostics.rs        # Device health checks & HID logging
│   │   ├── key_mapping.rs        # Per-key addressing & keyboard layouts
│   │   ├── zone_mapping.rs       # RGB zone definitions & mappings
│   │   ├── fuzz.rs               # Protocol fuzzing (developer tool)
│   │   ├── keyboards/
│   │   │   ├── @mod.rs           # Keyboard trait (25+ methods)
│   │   │   ├── apex.rs           # Generic Apex implementations
│   │   │   └── @apex_pro_tkl_2023.rs  # Primary device (PID 0x1628)
│   │   └── headsets/
│   │       └── mod.rs            # Headset implementations
│   │
│   ├── @rgb/                     # Color & lighting engine
│   │   ├── @mod.rs               # Color, Effect, EffectEngine, RgbController
│   │   └── tests.rs              # RGB unit tests (11 tests)
│   │
│   ├── @gamesense/               # GameSense HTTP API
│   │   ├── mod.rs                # Data structures (GameMetadata, GameEvent)
│   │   ├── @server.rs            # Axum HTTP server
│   │   └── @handlers.rs          # Request handlers (CORS-enabled)
│   │
│   ├── performance.rs            # Real-time monitoring & stats
│   ├── validation.rs             # Resource leak detection
│   ├── device_state.rs           # Async state persistence (JSON)
│   ├── diagnostics_export.rs    # Bug report generation
│   ├── pollrate.rs               # USB poll rate control (sysfs)
│   │
│   ├── profiles/                 # Configuration persistence
│   │   ├── mod.rs                # Profile struct & management
│   │   └── tests.rs              # Profile serialization tests
│   │
│   ├── config/                   # User configuration
│   │   └── mod.rs                # Config struct (~/.config/ssgg/)
│   │
│   ├── audio/                    # Optional audio features
│   │   ├── mod.rs                # AudioMixer trait
│   │   ├── pulse.rs              # PulseAudio integration
│   │   └── sonar.rs              # SonarClient HTTP API
│   │
│   └── bin/
│       ├── discover_actuation.rs # Actuation discovery tool
│       └── sonar_control.rs      # Sonar control utility
│
├── .editorconfig                 # Editor settings (4 spaces for Rust)
├── rustfmt.toml                  # Rust formatting (edition 2024, 100 char max)
│
├── .github/
│   ├── workflows/
│   │   ├── @ci.yml               # CI pipeline (fmt, clippy, test, build)
│   │   └── release-arch.yml      # Arch Linux package release
│   ├── dependabot.yml            # Dependency updates
│   └── @copilot-instructions.md  # GitHub Copilot guardrails
│
├── assets/
│   ├── 99-steelseries.rules      # udev rules (USB permissions)
│   └── ssgg.service              # systemd user service
│
├── docs/
│   ├── development/              # Protocol research & development notes
│   │   ├── APEX_PRO_PROTOCOL.md  # HID protocol research
│   │   ├── KEY_MAPPING_RESEARCH.md  # Per-key RGB research
│   │   ├── PROTOCOL_RESEARCH.md  # General protocol findings
│   │   ├── RGB_CONTROL_ANALYSIS.md  # RGB control analysis
│   │   ├── OPTIMIZATION_REPORT.md   # Performance optimizations
│   │   └── todo.md               # Development tasks
│   └── archive/                  # Historical docs
│
├── CONTRIBUTING.md               # Contribution guidelines
├── PLAN.md                       # Development roadmap (per-key RGB focus)
├── PROJECT_INDEX.md              # Complete project structure & exports
├── PERFORMANCE_OPTIMIZATIONS.md  # Optimization findings & benchmarks
├── LICENSE                       # MIT license
└── tests/                        # Integration tests
```

---

## Development Workflows

### Setup

**System dependencies:**
```bash
# Debian/Ubuntu
sudo apt-get update
sudo apt-get install -y libudev-dev libhidapi-dev

# Fedora
sudo dnf install systemd-devel hidapi-devel

# Arch Linux
sudo pacman -S hidapi

# Optional: Audio feature
sudo apt-get install -y libpulse-dev        # Debian/Ubuntu
sudo dnf install pulseaudio-libs-devel      # Fedora
sudo pacman -S libpulse                     # Arch
```

**Clone & build:**
```bash
git clone https://github.com/Ven0m0/steelseriesgg-rs.git
cd steelseriesgg-rs
cargo build --release
```

**Install udev rules (required for device access):**
```bash
sudo cp assets/99-steelseries.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
sudo usermod -aG input $USER  # Log out/in to apply
```

### Build Commands

| Command | Purpose | Build Time |
|---------|---------|------------|
| `cargo build` | Debug build | ~5-10s |
| `cargo build --release` | Optimized release (LTO, strip) | ~30-60s |
| `cargo build --features audio` | With audio support | ~10-15s |
| `cargo build --features sonar` | With Sonar API client | ~10-15s |
| `cargo build --all-features` | All optional features | ~15-20s |

**Release profile optimizations:**
```toml
[profile.release]
strip = true           # Remove debug symbols (~30% size reduction)
lto = "fat"            # Full link-time optimization (~15% perf gain)
codegen-units = 1      # Single unit (better optimization)
panic = "abort"        # No unwinding (~10% size reduction)
opt-level = 3          # Maximum optimization
overflow-checks = false
```

**Result:** ~2-3 MB binary with excellent performance

### Testing

**Run tests:**
```bash
cargo test                        # ~77 unit tests (default features)
cargo test --all-features         # All feature combinations
cargo test rgb::tests             # Specific module
cargo test test_color_blending    # Specific test
cargo test -- --nocapture         # Show test output
cargo test -- --test-threads=1    # Single-threaded (for debugging)
```

**Test coverage (~77 tests):**
| Module | Test Count | Coverage |
|--------|------------|----------|
| RGB | 11 tests | Color operations, effects, caching, per-key |
| HID Reports | 19 tests | Builders, command encoding, padding |
| Zone Mapping | 8 tests | Zone definitions, fallback logic |
| Keyboards | 7 tests | Keyboard trait implementations |
| Profiles | 6 tests | Serialization, defaults, validation |
| Performance | 6 tests | Stats tracking, threshold detection |
| Validation | 6 tests | Resource leak detection |
| Key Mapping | 6 tests | Key addressing, layout support |
| Pollrate | 5 tests | Conversion, validation |
| GameSense | 3 tests | Color computation, event handling |

**Hardware tests:**
Require actual devices or mocking. Use diagnostic commands:
```bash
cargo run -- devices              # List connected devices
cargo run -- validate             # Run validation tests
cargo run -- test-device <device> # Automated device testing
```

### Code Quality (REQUIRED before commit)

```bash
# 1. Format code
cargo fmt

# 2. Lint with all features
cargo clippy --all-features -- -D warnings

# 3. Run tests
cargo test --all-features

# 4. Build release
cargo build --release
```

### CI Pipeline

**GitHub Actions workflow (`.github/workflows/ci.yml`):**

| Job | Matrix | Checks |
|-----|--------|--------|
| **Format** | - | `cargo fmt --check` |
| **Clippy** | default, audio, sonar | `cargo clippy -- -D warnings` |
| **Test** | default, sonar | `cargo test` |
| **Build** | default, audio, sonar | `cargo build --release` |

**All CI checks must pass** before merging PRs.

### Deployment

**systemd user service:**
```bash
# Install service (done by package manager)
cp assets/ssgg.service ~/.config/systemd/user/

# Enable and start
systemctl --user daemon-reload
systemctl --user enable --now ssgg.service

# Check status
systemctl --user status ssgg.service
journalctl --user -u ssgg.service -f

# Auto-start at boot (no login required)
sudo loginctl enable-linger $USER
```

**Manual daemon:**
```bash
cargo run --release -- daemon     # Foreground
RUST_LOG=debug cargo run -- daemon  # With debug logs
```

---

## Code Conventions

### Naming Conventions

| Item | Convention | Example |
|------|------------|---------|
| Functions/variables | `snake_case` | `set_rgb_color()`, `device_manager` |
| Types/structs/enums | `PascalCase` | `DeviceManager`, `RgbController`, `Effect` |
| Constants | `SCREAMING_SNAKE_CASE` | `STEELSERIES_VENDOR_ID`, `MAX_RGB_ZONES` |
| Modules | `snake_case` | `devices`, `gamesense`, `rgb` |
| Type parameters | `PascalCase` | `T`, `K`, `V` |
| Lifetimes | `'lowercase` | `'a`, `'static` |

### Style Guidelines

| Aspect | Standard | Enforcement |
|--------|----------|-------------|
| **Indentation** | 4 spaces (Rust), 2 spaces (other) | `.editorconfig`, `rustfmt.toml` |
| **Line length** | 100 characters max | `rustfmt.toml` |
| **Format** | `cargo fmt` (edition 2024) | CI check |
| **Tabs** | Spaces only (no hard tabs) | `.editorconfig` |
| **Newlines** | Unix (LF) | `.editorconfig` |
| **Imports** | Auto-reordered by rustfmt | `rustfmt.toml` |
| **Trailing commas** | Multiline collections | `rustfmt` default |

### Architecture Patterns

#### 1. Error Handling

```rust
// For binaries: anyhow::Result
use anyhow::{Result, Context};

fn cmd_devices() -> Result<()> {
    let manager = DeviceManager::new()
        .context("Failed to initialize device manager")?;
    Ok(())
}

// For libraries: thiserror::Error
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DeviceError {
    #[error("Device not found: {0}")]
    NotFound(String),
    #[error("HID error: {0}")]
    HidError(#[from] hidapi::HidError),
}

// NEVER use .unwrap() or .expect() in production code
// Use ? operator or proper error handling
```

#### 2. Device Trait Pattern

```rust
// Device trait provides common interface for all hardware
pub trait Device: Send + Sync {
    fn info(&self) -> &DeviceInfo;
    fn device_type(&self) -> DeviceType;
    fn initialize(&mut self) -> Result<()>;
    fn close(&mut self) -> Result<()>;
    fn is_connected(&self) -> bool;
    fn send_report(&mut self, data: &[u8]) -> Result<()>;
    fn read_report(&mut self, buf: &mut [u8], timeout: i32) -> Result<usize>;
    // ... 15+ more methods
}

// Keyboard extends Device with RGB-specific methods
pub trait Keyboard: Device {
    // RGB Control
    fn set_color(&mut self, color: Color) -> Result<()>;
    fn set_zone_colors(&mut self, colors: &[Color]) -> Result<()>;
    fn set_brightness(&mut self, brightness: u8) -> Result<()>;

    // Per-Key RGB (experimental)
    fn supports_per_key_rgb(&self) -> bool;
    fn set_key_color(&mut self, key_id: KeyId, color: Color) -> Result<()>;
    fn set_key_colors(&mut self, key_colors: &[(KeyId, Color)]) -> Result<()>;

    // ... 20+ more methods
}
```

#### 3. HID Report Builder Pattern (ALWAYS USE)

```rust
// CORRECT - Type-safe, guaranteed valid reports
let report = HidReportBuilder::new(HidDeviceType::Keyboard)
    .command(CommandCode::RgbControl)
    .zone_data(zone, &color)
    .build()?;
device.send_report(&report)?;

// WRONG - Manual buffer construction (fragile, error-prone)
let mut report = [0u8; 65];
report[0] = 0x00;  // Report ID
report[1] = 0x21;  // Command
// ... manual padding
```

**HID Report Sizes:**
- **Keyboards**: 65 bytes (includes report ID)
- **Headsets**: 64 bytes (no report ID)

**Command Codes:**
| Code | Name | Purpose |
|------|------|---------|
| `0x09` | Apply | Save/apply settings |
| `0x21` | RgbControl | Zone-based RGB |
| `0x22` | Brightness | Brightness control |
| `0x25` | ReactiveMode | Reactive effects |
| `0x26` | ColorShift | Color shift effects |
| `0x2A` | PerKeyRgb | Per-key RGB (placeholder, protocol TBD) |
| `0x2D` | ActuationControl | Actuation point (experimental) |

#### 4. RGB Effect Engine with Caching

```rust
// Create effect engine
let mut engine = EffectEngine::new(Effect::Breathing {
    color: Color::new(0, 255, 255),
    speed: 2.0,
});

// Compute colors (cached if Δt < 16ms = ~60 FPS)
let elapsed = Duration::from_millis(timestamp);
let colors = engine.compute(num_zones, elapsed);

// CRITICAL: First call always computes (last_compute_time == 0)
// Cache check: last_compute_time != Duration::ZERO
```

**Supported effects:**
```rust
pub enum Effect {
    Static { color: Color },
    Breathing { color: Color, speed: f32 },
    Spectrum { speed: f32 },
    Wave { colors: Vec<Color>, speed: f32, direction: Direction },
    Reactive { color: Color, decay: f32 },
    Gradient { start: Color, end: Color },
    Custom { colors: Vec<Color> },
    Off,
}
```

#### 5. Async/Tokio Patterns

```rust
// Use tokio for async operations
use tokio::sync::{Mutex, RwLock};
use tokio::time::{sleep, Duration};

// Shared state in async contexts
let shared = Arc::new(Mutex::new(state));

// Background tasks
tokio::spawn(async move {
    loop {
        // Background work
        sleep(Duration::from_secs(1)).await;
    }
});

// HTTP server (Axum)
let app = Router::new()
    .route("/game_event", post(handle_game_event))
    .layer(CorsLayer::permissive())
    .with_state(app_state);

let listener = TcpListener::bind("127.0.0.1:27301").await?;
axum::serve(listener, app).await?;
```

#### 6. Performance Monitoring

```rust
// Track performance in critical paths
let perf = PerformanceManager::new();

perf.start_operation("rgb_update");
// ... do RGB work
perf.end_operation("rgb_update");

let stats = perf.get_stats();
println!("Avg latency: {}ms", stats.avg_latency_ms);
```

### Critical Constants

```rust
// USB identifiers
pub const STEELSERIES_VENDOR_ID: u16 = 0x1038;
pub const APEX_PRO_TKL_2023_PRODUCT_ID: u16 = 0x1628;  // NOT 0x1618!

// HID protocol
pub const KEYBOARD_REPORT_SIZE: usize = 65;  // With report ID
pub const HEADSET_REPORT_SIZE: usize = 64;   // Without report ID
pub const MAX_RGB_ZONES: usize = 12;

// GameSense
pub const GAMESENSE_DEFAULT_PORT: u16 = 27301;

// Performance
pub const CACHE_THRESHOLD_MS: u64 = 16;  // ~60 FPS
```

---

## Dependencies

### Core Dependencies (default features)

| Crate | Version | Purpose | Size Impact |
|-------|---------|---------|-------------|
| **hidapi** | =2.6.4 | HID device communication | ~50 KB (pinned) |
| **tokio** | 1.49 | Async runtime (rt-multi-thread, macros) | ~500 KB |
| **axum** | 0.8 | GameSense HTTP server | ~200 KB |
| **clap** | 4.5 | CLI argument parsing (derive) | ~150 KB |
| **serde** | 1.0 | Serialization (derive) | ~100 KB |
| **serde_json** | 1.0 | JSON serialization | ~80 KB |
| **toml** | 1.0 | Config file parsing | ~60 KB |
| **thiserror** | 2.0 | Error type macros | ~20 KB |
| **anyhow** | 1.0 | Error handling (binaries) | ~30 KB |
| **tracing** | 0.1 | Structured logging | ~80 KB |
| **tracing-subscriber** | 0.3 | Log formatting (env-filter) | ~120 KB |
| **colored** | 3.1 | Terminal colors | ~20 KB |
| **chrono** | 0.4 | Timestamps (serde) | ~100 KB |
| **tower-http** | 0.6 | HTTP middleware (CORS) | ~80 KB |
| **directories** | 6.0 | XDG base directories | ~10 KB |
| **parking_lot** | 0.12 | High-performance locks | ~30 KB |
| **sysinfo** | 0.38 | System information | ~80 KB |
| **indicatif** | 0.18 | Progress bars | ~50 KB |
| **tabled** | 0.20 | Table formatting | ~60 KB |
| **libc** | 0.2 | C library bindings | ~40 KB |
| **async-trait** | 0.1 | Async trait support | ~10 KB |

### Optional Dependencies

| Feature | Crate | Version | Purpose |
|---------|-------|---------|---------|
| `audio` | libpulse-binding | 2.30 | PulseAudio mixer integration |
| `sonar` | reqwest | 0.13 | HTTP client for Sonar API |

### Dependency Management

**Update dependencies:**
```bash
cargo update           # Update within Cargo.toml constraints
cargo outdated         # Check for outdated dependencies (requires cargo-outdated)
```

**Audit security:**
```bash
cargo audit            # Check for security vulnerabilities (requires cargo-audit)
```

**Dependency tree:**
```bash
cargo tree             # Show full dependency tree
cargo tree -i hidapi   # Show reverse dependencies for hidapi
```

---

## Common Tasks

### Device Operations

```bash
# List connected devices
cargo run -- devices
RUST_LOG=debug cargo run -- devices  # With debug logs

# Test RGB
cargo run -- rgb color red
cargo run -- rgb color "#00FFFF"
cargo run -- rgb effect breathing --color cyan --speed 2.0
cargo run -- rgb effect spectrum --speed 1.5
cargo run -- rgb brightness 80

# Check per-key RGB support (experimental)
cargo run -- rgb perkey status
cargo run -- rgb perkey set-key escape red

# Actuation control (Apex Pro only)
cargo run -- actuation set 1.0  # 1.0mm
cargo run -- actuation read     # Not implemented yet
```

### Profile Management

```bash
# Save current settings
cargo run -- profile save gaming

# Load profile
cargo run -- profile load gaming

# List profiles
cargo run -- profile list

# Delete profile
cargo run -- profile delete gaming

# Profile location: ~/.config/ssgg/profiles/<name>.toml
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

# Restart service
systemctl --user restart ssgg

# Stop service
systemctl --user stop ssgg
```

### Diagnostics

```bash
# Generate bug report
cargo run -- bug-report --output report.json
cargo run -- bug-report --include-hid-logs

# View HID communication logs
cargo run -- hid-logs

# Run validation tests
cargo run -- validate

# Monitor performance
cargo run -- performance

# Real-time status
cargo run -- status

# Test specific device
cargo run -- test-device "Apex Pro TKL (2023)"

# Verify performance metrics
cargo run -- verify-performance

# Protocol fuzzing (developer tool, hidden command)
cargo run -- fuzz --start 0x20 --end 0x30 --delay 200
```

### Audio Features (requires `--features audio`)

```bash
# Build with audio support
cargo build --features audio

# View audio status
cargo run --features audio -- audio status

# Set volume
cargo run --features audio -- audio volume --channel master --level 75
cargo run --features audio -- audio volume --channel game --level 100

# Mute/unmute
cargo run --features audio -- audio mute --channel game
cargo run --features audio -- audio unmute --channel game

# List audio devices
cargo run --features audio -- audio devices
```

### Sonar Integration (requires `--features sonar`)

```bash
# Build with sonar support
cargo build --features sonar

# Discover Sonar port (dynamic)
cargo run --features sonar -- sonar discover

# Check status
cargo run --features sonar -- sonar status

# Control volume
cargo run --features sonar -- sonar volume game 100
cargo run --features sonar -- sonar volume chat 75

# Using the sonar_control binary
cargo run --bin sonar_control --features sonar
```

### USB Poll Rate

```bash
# Set poll rate (125, 250, 500, 1000, 2000, 4000 Hz)
cargo run -- pollrate set 1000

# Get current poll rate
cargo run -- pollrate get

# NOTE: 8000 Hz NOT supported (kernel limitation)
```

---

## Debugging

### Logging Levels

```bash
# Enable debug logs
RUST_LOG=debug cargo run -- devices

# Trace level (verbose)
RUST_LOG=trace cargo run -- daemon

# Module-specific logging
RUST_LOG=steelseries_gg::devices=debug cargo run -- devices
RUST_LOG=steelseries_gg::rgb=trace cargo run -- rgb effect breathing

# Enable HID diagnostics
cargo run -- --debug-hid devices
```

### Common Issues & Solutions

| Issue | Cause | Solution |
|-------|-------|----------|
| Device not detected | udev rules, USB, permissions | Check udev rules, run as root (testing only), verify USB connection |
| RGB not working | Interface number, HID reports | Verify interface (keyboards=1, headsets=3), check `RUST_LOG=debug` |
| Per-key RGB not working | Protocol not reverse-engineered | Use zone-based fallback with `simulate_per_key_with_zones()` |
| GameSense not responding | Port 27301 blocked | Check firewall, verify port availability with `netstat -tuln` |
| High CPU usage | Effect computation timing | Check performance stats, reduce effect speed, enable caching |
| Actuation read failing | Read command not implemented | Write-only for now, read is placeholder |
| Audio not working | PulseAudio not running, feature not enabled | Start PulseAudio, build with `--features audio` |
| Permission denied | User not in input group | `sudo usermod -aG input $USER`, log out/in |
| Daemon not auto-starting | systemd linger disabled | `sudo loginctl enable-linger $USER` |

### Diagnostic Commands

```bash
# Device information
ssgg devices                    # List all connected devices
ssgg status                     # Real-time connection status

# HID communication
ssgg hid-logs                   # View HID communication logs
ssgg --debug-hid devices        # Enable HID diagnostics

# System diagnostics
ssgg bug-report                 # Generate comprehensive diagnostic report
ssgg validate                   # Run validation tests on devices
ssgg test-device "Apex Pro"     # Automated device testing
ssgg verify-performance         # Verify RGB performance metrics

# Performance monitoring
ssgg performance                # Monitor RGB performance in real-time
```

---

## Critical Gotchas & Pitfalls

### 1. HID Report Sizing

**Problem**: Incorrect report size causes communication failures

**Solution**: Always use `HidReportBuilder` - it handles sizing automatically
- **Keyboards**: 65 bytes (includes report ID byte)
- **Headsets**: 64 bytes (no report ID)

### 2. Apex Pro TKL 2023 Product ID

**Problem**: Documentation shows `0x1618`, hardware actually uses `0x1628`

**Solution**: Always use `0x1628` (hardware-verified)

### 3. Interface Numbers

**Problem**: Devices have multiple HID interfaces

**Solution**:
- Keyboards: Interface 1 for control
- Headsets: Interface 3 for control
- Use `DeviceManager::open_device()` for automatic selection

### 4. Animated Effects in CLI

**Problem**: `ssgg rgb effect breathing` shows no animation

**Why**: CLI commands are one-shot; animations require continuous updates

**Solution**: Use daemon mode:
```bash
ssgg daemon  # Now effects animate continuously
```

### 5. RGB Caching Bug (Fixed 2026-01-12)

**Problem**: First `EffectEngine::compute()` returned black/empty colors

**Root cause**: Cache returned empty `Vec<Color>` on first call

**Fix**: Check `last_compute_time != Duration::ZERO` before using cache

### 6. Per-Key RGB Status

**Problem**: Per-key RGB commands have no effect

**Why**: HID command `0x2A` is a placeholder - protocol not reverse-engineered

**Workaround**: Use zone-based fallback with `simulate_per_key_with_zones()`

### 7. Actuation Point Reading

**Problem**: `read_actuation_point()` always returns error

**Why**: HID command to read actuation settings has not been discovered

**Status**: Write-only for now (set works, read is placeholder)

### 8. Product ID Conflicts

**Problem**: Some product IDs are shared (e.g., `0x12AD` = Arctis 1 or Arctis 7 2017)

**Solution**: Device name shows "Arctis 1 / Arctis 7" for ambiguous IDs

### 9. udev Permissions

**Problem**: "Permission denied" on device access

**Solution**:
```bash
sudo cp assets/99-steelseries.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
sudo usermod -aG input $USER  # Log out and back in
```

### 10. systemd Service Auto-Start

**Problem**: Service only starts after login

**Solution**: Enable linger for user account
```bash
sudo loginctl enable-linger $USER
```

### 11. Feature Flag Dependencies

**Problem**: `--features sonar` requires audio infrastructure

**Solution**: Use `--features audio,sonar` or `--all-features`

### 12. HID Byte Order

**Problem**: RGB colors wrong (red shows as blue)

**Why**: Some devices use BGR instead of RGB

**Solution**: Most SteelSeries use RGB order; check device docs if issues

---

## Performance & Optimization

### Release Build Optimization

```toml
[profile.release]
strip = true           # Remove debug symbols (~30% size reduction)
lto = "fat"            # Full link-time optimization (~15% perf gain)
codegen-units = 1      # Single unit (better optimization)
panic = "abort"        # No unwinding (~10% size reduction)
opt-level = 3          # Maximum optimization
debug = 0              # No debug info
overflow-checks = false # Disable overflow checks
```

**Result**:
- Binary size: ~2-3 MB
- Startup time: <100ms
- RGB update latency: <5ms
- Memory usage: ~10-20 MB

### Performance Best Practices

1. **RGB Updates**: Use caching (16ms threshold = 60 FPS)
   ```rust
   let colors = engine.compute(num_zones, elapsed);  // Cached if Δt < 16ms
   ```

2. **HID Writes**: Batch when possible, reuse buffers
   ```rust
   let mut buffer = Vec::with_capacity(MAX_ZONES);
   loop {
       buffer.clear();  // Reuse allocation
       compute_colors(&mut buffer);
   }
   ```

3. **Effect Computation**: Leverage `EffectEngine` cache
   ```rust
   // Cache automatically used if elapsed < 16ms
   let colors = engine.compute(num_zones, elapsed);
   ```

4. **Device Polling**: Adaptive timing in daemon mode
   ```rust
   // Adjusts polling interval based on activity
   let interval = adaptive_interval(last_update);
   ```

5. **Memory**: Reuse `Vec<Color>` allocations
   ```rust
   // GOOD
   let mut colors = Vec::with_capacity(12);
   loop { colors.clear(); /* reuse */ }

   // BAD
   loop { let colors = Vec::new(); /* allocates every time */ }
   ```

6. **Monitoring**: Enable `PerformanceManager` to track bottlenecks
   ```rust
   let perf = PerformanceManager::new();
   perf.start_operation("rgb_update");
   // ... work
   perf.end_operation("rgb_update");
   ```

### Recent Optimizations

- **20% CPU reduction**: Optimized HID communication protocol
- **Adaptive timing**: Dynamic effect computation intervals
- **Resource validation**: Automatic leak detection
- **Zero-copy**: Per-key RGB buffer reuse
- **Ring buffer**: Frame timing history for smooth metrics
- **Write-behind caching**: Async device state persistence

---

## Configuration Files

### Config Location

`~/.config/ssgg/config.toml`

### Default Configuration

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

### Profile Storage

**Location**: `~/.config/ssgg/profiles/<name>.toml`

**Example profile:**
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

### Device State Storage

**Location**: `~/.config/ssgg/state.json`

**Purpose**: Tracks last-applied device settings for daemon mode (async persistence)

---

## Git Workflow

### Commit Message Convention

```
<type>: <description>

[optional body]

[optional footer]
```

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `refactor`: Code refactoring
- `docs`: Documentation changes
- `test`: Test additions/changes
- `perf`: Performance improvements
- `chore`: Build/tooling changes
- `style`: Code style changes (formatting)

**Examples:**
```
feat: Add per-key RGB fallback using zones
fix: RGB caching returns black on first compute
refactor: Extract HID report builder pattern
docs: Update AGENTS.md with common gotchas
test: Add unit tests for Color blending
perf: Optimize HID communication protocol by 20%
```

### Pre-Commit Checklist

```bash
# 1. Format
cargo fmt

# 2. Lint
cargo clippy --all-features -- -D warnings

# 3. Test
cargo test --all-features

# 4. Build
cargo build --release

# 5. Check documentation
cargo doc --no-deps --all-features
```

---

## Additional Resources

### Documentation Files

| File | Purpose | Size |
|------|---------|------|
| **AGENTS.md** | This file (AI assistant quick reference) | ~50 KB |
| **README.md** | User documentation & installation | ~7 KB |
| **CONTRIBUTING.md** | Contribution guidelines | ~7 KB |
| **PROJECT_INDEX.md** | Complete project structure & exports | ~17 KB |
| **PLAN.md** | Development roadmap (per-key RGB focus) | ~3 KB |
| **PERFORMANCE_OPTIMIZATIONS.md** | Optimization findings & benchmarks | ~8 KB |

### Development Documentation

| File | Purpose |
|------|---------|
| **docs/development/APEX_PRO_PROTOCOL.md** | Apex Pro HID protocol research |
| **docs/development/KEY_MAPPING_RESEARCH.md** | Per-key addressing research |
| **docs/development/PROTOCOL_RESEARCH.md** | General protocol findings |
| **docs/development/RGB_CONTROL_ANALYSIS.md** | RGB control analysis |
| **docs/development/OPTIMIZATION_REPORT.md** | Performance optimization findings |
| **docs/development/DEPENDENCY_AUDIT_REPORT.md** | Security audit results |
| **docs/development/todo.md** | Development tasks & roadmap |

### External Resources

- **Repository**: https://github.com/Ven0m0/steelseriesgg-rs
- **Issues**: https://github.com/Ven0m0/steelseriesgg-rs/issues
- **Rust Documentation**: https://doc.rust-lang.org/
- **hidapi Documentation**: https://docs.rs/hidapi/
- **Tokio Documentation**: https://docs.rs/tokio/
- **Axum Documentation**: https://docs.rs/axum/

---

## Current Development Focus

### Primary: Per-Key RGB Control (Apex Pro TKL 2023)

**Status**: Protocol reverse engineering in progress

**Challenges**:
- Command code `0x2A` is placeholder
- Actual HID command sequence unknown
- Key addressing scheme not documented

**Interim Solution**: Zone-based fallback working

**Progress**: See `docs/development/KEY_MAPPING_RESEARCH.md`

### Secondary: Actuation Point Control

**Status**: Partial implementation

**Working**:
- Write: `set_actuation_point()` using command `0x2D`
- Write (mm): `set_actuation_point_mm()` (converts mm to raw value)

**Not Working**:
- Read: `read_actuation_point()` - HID command not discovered

**Limitations**:
- Apex Pro series only
- Write-only for now

---

**Version**: 0.1.0
**Last Updated**: 2026-02-10
**Maintainer**: steelseriesgg-rs contributors
**License**: MIT
