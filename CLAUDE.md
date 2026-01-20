# CLAUDE.md - Developer Guide

Comprehensive guidance for AI assistants and developers working with **steelseriesgg-rs**.

## Table of Contents

1. [Project Overview](#project-overview)
2. [Quick Reference](#quick-reference)
3. [Architecture](#architecture)
4. [Module Documentation](#module-documentation)
5. [Development Workflows](#development-workflows)
6. [Testing](#testing)
7. [Common Gotchas](#common-gotchas)
8. [Performance & Optimization](#performance--optimization)

---

## Project Overview

**steelseriesgg-rs** is a complete open-source replacement for SteelSeries GG on Linux.

### Core Capabilities
- **RGB Control**: Multi-zone lighting with 7+ effects (static, breathing, spectrum, wave, reactive, gradient, custom)
- **GameSense Server**: HTTP API compatible with official SteelSeries GameSense protocol
- **Performance Monitoring**: Real-time CPU, memory, and latency tracking
- **Resource Validation**: Automatic detection of resource leaks and performance issues
- **Profile Management**: Save/load device configurations with TOML
- **Audio Mixing**: PulseAudio integration (optional `audio` feature)
- **Sonar Integration**: SteelSeries Sonar HTTP client (optional `sonar` feature)

### Project Metadata
- **Binary**: `ssgg`
- **Library**: `steelseries_gg`
- **Language**: Rust 2021 edition (1.70+)
- **License**: MIT
- **Repository**: https://github.com/Ven0m0/steelseriesgg-rs

### Feature Flags
| Flag | Dependencies | Description |
|------|--------------|-------------|
| `default` | None | RGB control + GameSense server |
| `audio` | libpulse-binding | PulseAudio integration |
| `sonar` | reqwest, audio | SteelSeries Sonar API client |

---

## Quick Reference

### Essential Commands

```bash
# Build
cargo build                              # Debug build
cargo build --release                    # Optimized release build
cargo build --all-features               # Include audio + sonar

# Code Quality
cargo fmt                                # Format (required before commit)
cargo clippy --all-features              # Lint all code paths
cargo test                               # Run ~66 unit tests
cargo test --all-features                # Test with all features

# Device Operations
RUST_LOG=debug cargo run -- devices     # List devices with debug logs
cargo run -- rgb --color red             # Test static RGB
cargo run -- rgb --effect breathing      # Test animated effect

# Daemon
cargo run --release -- daemon            # Run in foreground
systemctl --user status ssgg             # Check systemd service
journalctl --user -u ssgg -f             # Follow daemon logs
```

### Code Style

| Aspect | Standard |
|--------|----------|
| **Indentation** | 4 spaces (`.editorconfig` enforced) |
| **Line length** | 100 characters max |
| **Format** | `cargo fmt` before commit |
| **Lints** | Fix all `cargo clippy` warnings |
| **Naming** | `snake_case` (functions/vars), `PascalCase` (types), `SCREAMING_SNAKE_CASE` (constants) |

### Critical Constants

| Constant | Value | Location | Purpose |
|----------|-------|----------|---------|
| `STEELSERIES_VENDOR_ID` | `0x1038` | `lib.rs` | USB vendor ID filter |
| `KEYBOARD_REPORT_SIZE` | 641 bytes | `hid_reports.rs` | Standard keyboard HID report |
| `HEADSET_REPORT_SIZE` | 65 bytes | `hid_reports.rs` | Standard headset HID report |
| `MAX_RGB_ZONES` | 9 | `hid_reports.rs` | Maximum RGB zones |
| `GAMESENSE_DEFAULT_PORT` | 27301 | `gamesense/` | GameSense HTTP port |
| `CACHE_THRESHOLD_MS` | 16 | `rgb/` | RGB cache (~60 FPS) |

---

## Architecture

### Module Structure

```
src/
├── lib.rs              # Library entry, module declarations, prelude
├── main.rs             # CLI (clap-based)
├── error.rs            # Error types (thiserror)
│
├── devices/            # Device discovery & HID communication
│   ├── mod.rs          # Device trait, DeviceInfo, product mappings
│   ├── discovery.rs    # DeviceManager with hidapi enumeration
│   ├── hid_reports.rs  # HID report builders & command protocol
│   ├── diagnostics.rs  # Device health checks & diagnostics
│   ├── key_mapping.rs  # Per-key addressing & keyboard layouts
│   ├── zone_mapping.rs # RGB zone definitions & mapping
│   ├── keyboards/
│   │   ├── mod.rs      # Keyboard trait & common logic
│   │   └── apex.rs     # Apex-series specific implementations
│   └── headsets/
│       └── mod.rs      # Headset implementations
│
├── rgb/                # Color & lighting effects
│   ├── mod.rs          # Color, Effect, EffectEngine, RgbController
│   └── tests.rs        # RGB unit tests
│
├── gamesense/          # GameSense HTTP server
│   ├── mod.rs          # Data structures (GameMetadata, GameEvent)
│   ├── server.rs       # Axum HTTP server
│   └── handlers.rs     # Request handlers
│
├── performance.rs      # Performance monitoring & stats tracking
├── validation.rs       # Resource validation & leak detection
│
├── audio/              # Audio mixer (optional)
│   ├── mod.rs          # AudioMixer (PulseAudio)
│   └── sonar.rs        # SonarClient HTTP API
│
├── profiles/           # Configuration persistence
│   ├── mod.rs          # Profile struct & management
│   └── tests.rs        # Profile serialization tests
│
├── config/             # TOML configuration
│   └── mod.rs          # Config struct (~/.config/ssgg/)
│
├── pollrate.rs         # USB poll rate control (sysfs)
└── device_state.rs     # Device state tracking for daemon
```

### Key Architectural Patterns

#### 1. Device Abstraction Layer

**Purpose**: Uniform interface for all SteelSeries hardware

```rust
pub trait Device: Send + Sync {
    fn info(&self) -> &DeviceInfo;
    fn device_type(&self) -> DeviceType;
    fn initialize(&mut self) -> Result<()>;
    fn close(&mut self) -> Result<()>;
    fn is_connected(&self) -> bool;
    // ... RGB control methods
}
```

**Key points**:
- `Device` trait provides common interface
- `DeviceManager` handles discovery via hidapi
- Product IDs map to device types and names
- Per-device implementations in `keyboards/` and `headsets/`

#### 2. HID Report Protocol

**Purpose**: Low-level hardware communication

**Report Structure** (varies by device type):
```rust
// Keyboard (641 bytes)
[Report ID: 1] [Command Code: 1] [Data: 639]

// Headset (65 bytes)
[Report ID: 1] [Command Code: 1] [Data: 63]
```

**HidReportBuilder Pattern**:
```rust
let report = HidReportBuilder::new(HidDeviceType::Keyboard)
    .command(CommandCode::SetRgbZone)
    .zone_data(zone, &color)
    .build()?;
device.send(&report)?;
```

**Always use builders** to ensure correct padding and structure.

#### 3. Device Discovery & Caching

**Purpose**: O(1) device lookup with efficient enumeration

```rust
pub struct DeviceManager {
    api: HidApi,
    devices: HashMap<String, DeviceInfo>,          // path -> info
    device_cache: HashMap<(u16, u16, i32), String>, // (vid, pid, iface) -> path
}
```

**Optimization**:
- `refresh()` scans USB once, filters by vendor ID `0x1038`
- Cache keyed by `(vendor_id, product_id, interface_number)`
- `open_device()` uses cache for O(1) lookup
- Call `refresh()` after hotplug events

#### 4. RGB Effect Engine with Caching

**Purpose**: Compute animated effects efficiently

```rust
pub struct EffectEngine {
    effect: Effect,
    cached_colors: Vec<Color>,
    last_compute_time: Duration,
    cache_threshold: Duration,  // 16ms = ~60 FPS
}
```

**Caching strategy**:
- Returns cached colors if `Δt < 16ms`
- **CRITICAL**: First call always computes (`last_compute_time == 0`)
- Reuses `Vec<Color>` to avoid allocations
- Supports 7+ effect types with custom parameters

#### 5. Performance Monitoring

**Purpose**: Real-time tracking of resource usage

```rust
pub struct PerformanceManager {
    stats: PerformanceStats,
    thresholds: PerformanceThresholds,
    // ... internal tracking
}
```

**Tracked metrics**:
- CPU usage per operation
- Memory allocations
- HID write latency
- Effect computation time
- Event loop timing

**Usage**: Enable with `PerformanceManager::new()` in daemon mode

#### 6. Resource Validation

**Purpose**: Detect resource leaks and performance degradation

```rust
pub struct RgbValidator {
    validation_history: Vec<ValidationResult>,
    performance_baseline: Option<PerformanceStats>,
}
```

**Checks**:
- Memory leak detection
- CPU spike detection
- Latency regression
- Resource cleanup verification

**Auto-runs** in daemon mode every 30 seconds

#### 7. GameSense Server (Axum)

**Purpose**: HTTP API for game integration

**Architecture**:
- Async HTTP server (tokio + axum)
- CORS-enabled for browser access
- Event-driven reactive lighting

**Flow**:
```
Game → POST /game_event → Handler → RgbController → HID Report → Device
```

**Key endpoints**:
- `POST /game_metadata` - Register game
- `POST /bind_game_event` - Bind event to device
- `POST /game_event` - Send game state

---

## Module Documentation

### `devices` - Hardware Communication

**Core responsibilities**:
- USB device enumeration (hidapi)
- HID report construction & transmission
- Product ID mapping to device types
- Interface selection (keyboards use #1, headsets use #3)
- Diagnostic health checks

**Key types**:
- `DeviceManager` - Discovery & connection management
- `Device` trait - Common device interface
- `DeviceInfo` - Metadata (VID, PID, path, serial)
- `HidReportBuilder` - Type-safe HID report construction
- `KeyMapping` - Per-key addressing for RGB
- `ZoneMapping` - Zone definitions & fallback logic

**Adding a new device**:
1. Find product ID: `lsusb | grep SteelSeries`
2. Add constant to `devices/mod.rs` product_ids module
3. Update `device_type_from_product_id()` match
4. Update `device_name_from_product_id()` match
5. (Optional) Create specialized module in `keyboards/` or `headsets/`

### `rgb` - Color & Lighting

**Core responsibilities**:
- RGB color representation with blend/scale/HSV
- Effect computation (7+ types)
- Brightness control
- Per-key RGB support
- Effect caching for performance

**Key types**:
- `Color` - RGB triplet with color operations
- `Effect` - Enum of all effect types
- `EffectEngine` - Computes effect colors over time
- `RgbController` - High-level RGB API with brightness
- `PerKeyRgbController` - Per-key addressable RGB

**Supported effects**:
```rust
Static, Breathing, Spectrum, Wave, Reactive, Gradient, Custom, Off
```

**Performance**: Animated effects require daemon mode for continuous updates

### `gamesense` - Game Integration

**Core responsibilities**:
- HTTP server for GameSense protocol
- Event registration & binding
- Handler execution (map events to RGB)
- CORS support for web integration

**Key types**:
- `GameMetadata` - Game registration
- `GameEvent` - Event definition
- `GameEventHandler` - Maps values to effects
- `GamesenseServer` - HTTP server

**Example event binding**:
```json
{
  "game": "CSGO",
  "event": "health",
  "handlers": [{
    "device-type": "keyboard",
    "zone": "all",
    "color": {
      "gradient": {
        "zero": {"red": 255, "green": 0, "blue": 0},
        "hundred": {"red": 0, "green": 255, "blue": 0}
      }
    }
  }]
}
```

### `performance` - Resource Monitoring

**Core responsibilities**:
- Real-time performance tracking
- CPU/memory profiling
- Latency measurement
- Threshold alerts

**Key types**:
- `PerformanceManager` - Tracking orchestrator
- `PerformanceStats` - Current metrics
- `PerformanceThresholds` - Alert limits

**Usage**:
```rust
let perf = PerformanceManager::new();
perf.start_operation("rgb_update");
// ... do work
perf.end_operation("rgb_update");
let stats = perf.get_stats();
```

### `validation` - Quality Assurance

**Core responsibilities**:
- Resource leak detection
- Performance regression testing
- Automatic health checks
- Validation reporting

**Key types**:
- `RgbValidator` - RGB system validator
- `ValidationResult` - Pass/fail results
- `ValidationReport` - Detailed findings

**Auto-validates**: Memory, CPU, latency, cleanup every 30s in daemon

### `audio` - Audio Mixing (Optional)

**Feature gates**: `audio`, `sonar`

**Core responsibilities**:
- Multi-channel mixing (Game, Chat, Media, Aux, Mic)
- PulseAudio integration
- Sonar HTTP API client

**Key types**:
- `AudioMixer` - Channel mixer
- `SonarClient` - HTTP client for Sonar API

**Note**: Sonar uses dynamic ports; use `ssgg sonar discover` to find port

### `profiles` - Configuration Persistence

**Storage**: `~/.config/ssgg/profiles/<name>.toml`

**Structure**:
```toml
[keyboard]
rgb_effect = "Static"
rgb_color = "#FF0000"
brightness = 80

[headset]
rgb_effect = "Breathing"
rgb_color = "#00FFFF"
```

**API**:
```rust
let profile = Profile::load("gaming")?;
profile.apply_to_device(&mut device)?;
profile.save("gaming")?;
```

### `pollrate` - USB Poll Rate

**Supported rates**: 125, 250, 500, 1000, 2000, 4000 Hz

**Note**: 8000 Hz NOT supported (kernel limitation)

**Implementation**: Writes to `/sys/bus/usb/devices/<device>/bInterval`

---

## Development Workflows

### Adding a New CLI Command

1. **Define command**:
```rust
// src/main.rs
#[derive(Args)]
struct MyCommandArgs {
    #[arg(long)]
    option: String,
}

enum Commands {
    MyCommand(MyCommandArgs),
    // ...
}
```

2. **Implement handler**:
```rust
fn cmd_my_command(args: MyCommandArgs) -> Result<()> {
    // Implementation
    Ok(())
}
```

3. **Wire up**:
```rust
match cli.command {
    Commands::MyCommand(args) => cmd_my_command(args)?,
    // ...
}
```

### Testing Device Communication

**Debug logging**:
```bash
RUST_LOG=debug cargo run -- devices
RUST_LOG=trace cargo run -- rgb --color red
```

**Inspect HID reports**:
```rust
use tracing::debug;
debug!("HID report: {:02X?}", &report);
```

**Test RGB**:
```bash
cargo run -- rgb --color red
cargo run -- rgb --effect breathing --color cyan
cargo run -- rgb --effect spectrum
```

### Working with Feature Flags

**Test all combinations**:
```bash
cargo test                      # Default
cargo test --features audio     # With audio
cargo test --all-features       # Everything
```

**Conditional compilation**:
```rust
#[cfg(feature = "audio")]
pub mod audio;

#[cfg(any(feature = "audio", feature = "sonar"))]
use crate::audio::AudioMixer;
```

**IMPORTANT**: Ensure code compiles with ALL feature combinations

---

## Testing

### Current Coverage

**~66 unit tests** across modules:
- RGB: Color, effects, caching, per-key addressing
- Profiles: Serialization, defaults, validation
- Pollrate: Conversion, validation
- GameSense: Color computation, event handling
- Performance: Stats tracking, threshold detection
- Validation: Resource leak detection
- HID Reports: Builder patterns, command encoding

### Running Tests

```bash
cargo test                              # All tests
cargo test --all-features               # With all features
cargo test rgb::tests                   # Specific module
cargo test test_effect_engine_static    # Specific test
cargo test -- --nocapture               # Show output
```

### Test Organization

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature() {
        // Arrange
        let input = setup();

        // Act
        let result = function(input);

        // Assert
        assert_eq!(result, expected);
    }
}
```

### Adding Tests

1. Create `#[cfg(test)] mod tests` in module
2. Import: `use super::*;`
3. Test public API + edge cases
4. Run `cargo test` to verify
5. Consider property-based testing for complex logic

**Note**: Hardware tests require actual devices or mocking

---

## Common Gotchas

### 1. HID Report Sizing

**Problem**: Incorrect report size causes communication failures

**Solution**: Always use `HidReportBuilder`:
```rust
HidReportBuilder::new(HidDeviceType::Keyboard)  // 641 bytes
HidReportBuilder::new(HidDeviceType::Headset)   // 65 bytes
```

**Why different sizes?**: Keyboards support per-key RGB (more data)

### 2. Product ID Conflicts

**Problem**: Some product IDs are shared (e.g., `0x12AD` = Arctis 1 or Arctis 7 2017)

**Solution**: Device name shows "Arctis 1 / Arctis 7" for ambiguous IDs

### 3. Apex Pro TKL 2023 Product ID

**Problem**: Documentation shows `0x1618`, hardware uses `0x1628`

**Solution**: Always use `0x1628` (hardware-verified)

### 4. Interface Numbers

**Problem**: Devices have multiple HID interfaces

**Solution**:
- Keyboards: Interface 1 for control
- Headsets: Interface 3 for control
- Use `DeviceManager::open_device()` for automatic selection

### 5. Animated Effects

**Problem**: `ssgg rgb --effect breathing` shows no animation

**Why**: CLI commands are one-shot; animations require continuous updates

**Solution**: Use daemon mode:
```bash
ssgg daemon
# Now effects animate continuously
```

### 6. RGB Caching Bug (Fixed)

**Problem** (2026-01-12): First `EffectEngine::compute()` returned black

**Root cause**: Cache returned empty `Vec<Color>` on first call

**Fix**: Check `last_compute_time != Duration::ZERO` before using cache

### 7. Feature Dependencies

**Problem**: `--features sonar` fails without `audio`

**Solution**: `Cargo.toml` enforces: `sonar = ["audio", ...]`

Always use: `--features audio,sonar` or `--all-features`

### 8. udev Permissions

**Problem**: "Permission denied" on device access

**Solution**:
```bash
sudo cp assets/99-steelseries.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo usermod -aG input $USER
# Log out and back in
```

### 9. Systemd Service Won't Auto-Start

**Problem**: Service only starts after login

**Solution**:
```bash
sudo loginctl enable-linger $USER
```

### 10. HID Byte Order

**Problem**: RGB colors wrong (red shows as blue)

**Why**: Some devices use BGR instead of RGB

**Solution**: Check device docs; most SteelSeries use RGB order

---

## Performance & Optimization

### Release Profile

Optimized for size and speed:

```toml
[profile.release]
strip = true           # Remove debug symbols
lto = "fat"            # Full link-time optimization
codegen-units = 1      # Single unit (better optimization)
panic = "abort"        # No unwinding (smaller binary)
opt-level = 3          # Maximum optimization
debug = 0              # No debug info
overflow-checks = false # Disable overflow checks
```

**Result**: ~2-3 MB binary with excellent performance

### Performance Best Practices

1. **RGB updates**: Use caching (16ms threshold = 60 FPS)
2. **HID writes**: Batch when possible, reuse buffers
3. **Effect computation**: Leverage `EffectEngine` cache
4. **Device polling**: Adaptive timing in daemon mode
5. **Memory**: Reuse `Vec<Color>` allocations
6. **Monitoring**: Enable `PerformanceManager` to track bottlenecks

### Recent Optimizations

- **20% CPU reduction**: Optimized HID communication protocol
- **Adaptive timing**: Dynamic effect computation intervals
- **Resource validation**: Automatic leak detection
- **Zero-copy**: Per-key RGB buffer reuse

---

## Configuration

### Config Location

`~/.config/ssgg/config.toml`

### Default Config

```toml
[gamesense]
enabled = true
bind = "127.0.0.1"
port = 27301

[audio]
master_volume = 100
game_volume = 100
chat_volume = 100

[general]
default_profile = "default"
debug = false
```

### Profiles

`~/.config/ssgg/profiles/<name>.toml`

---

## Debugging

### Enable Logging

```bash
RUST_LOG=debug cargo run -- devices
RUST_LOG=trace cargo run -- daemon
```

### Common Issues

| Issue | Check |
|-------|-------|
| Device not detected | udev rules, USB connection, permissions |
| RGB not working | Interface number, HID reports (RUST_LOG=debug) |
| GameSense not responding | Port 27301 availability, firewall |
| Audio failing | PulseAudio running, feature flags enabled |
| High CPU | Performance stats, effect computation timing |

### Reporting Bugs

1. Enable `RUST_LOG=debug`
2. Capture full output
3. Run `ssgg devices` for device info
4. Check existing GitHub issues
5. Include Rust version: `rustc --version`

---

## Documentation Files

### Root
- `README.md` - User documentation
- `CLAUDE.md` - This file (developer guide)
- `CONTRIBUTING.md` - Contribution guidelines
- `LICENSE` - MIT license

### Development Docs
- `docs/development/` - Reports & notes
  - `OPTIMIZATION_REPORT.md` - Performance findings
  - `DEPENDENCY_AUDIT_REPORT.md` - Security audit
  - `todo.md` - Development roadmap

### Archive
- `docs/archive/` - Historical reports (not needed for active dev)

---

## Additional Resources

- **USB protocol**: See `docs/development/` for HID protocol notes
- **Device support**: Check README.md for current device list
- **GameSense API**: See `gamesense/handlers.rs` for endpoint details
- **RGB effects**: See `rgb/tests.rs` for effect examples

---

**Last Updated**: 2026-01-20
**Maintainer**: steelseriesgg-rs contributors
**Version**: 0.1.0
