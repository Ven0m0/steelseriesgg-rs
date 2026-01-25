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
- **Per-Key RGB**: Individual key addressing support (experimental, protocol research ongoing)
- **Actuation Control**: Adjustable actuation points for Apex Pro keyboards (experimental)
- **GameSense Server**: HTTP API compatible with official SteelSeries GameSense protocol
- **Performance Monitoring**: Real-time CPU, memory, and latency tracking with adaptive timing
- **Resource Validation**: Automatic detection of resource leaks and performance issues
- **Profile Management**: Save/load device configurations with TOML
- **Diagnostic Tools**: HID logging, bug reports, device testing, protocol fuzzing
- **Audio Mixing**: PulseAudio integration (optional `audio` feature)
- **Sonar Integration**: SteelSeries Sonar HTTP client (optional `sonar` feature)

### Project Metadata
- **Binary**: `ssgg`
- **Library**: `steelseries_gg`
- **Package**: `steelseries-gg-linux`
- **Language**: Rust 2021 edition (1.70+)
- **License**: MIT
- **Repository**: https://github.com/Ven0m0/steelseriesgg-rs

### Feature Flags
| Flag | Dependencies | Description |
|------|--------------|-------------|
| `default` | None | RGB control + GameSense server |
| `audio` | libpulse-binding | PulseAudio integration |
| `sonar` | reqwest | SteelSeries Sonar API client |

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
cargo test                               # Run ~77 unit tests
cargo test --all-features                # Test with all features

# Device Operations
RUST_LOG=debug cargo run -- devices      # List devices with debug logs
cargo run -- rgb color red               # Test static RGB
cargo run -- rgb effect breathing        # Test animated effect
cargo run -- rgb perkey status           # Check per-key RGB support

# Debugging
cargo run -- --debug-hid devices         # Enable HID diagnostics
cargo run -- hid-logs                    # View HID communication logs
cargo run -- bug-report                  # Generate diagnostic report

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
| `KEYBOARD_REPORT_SIZE` | 65 bytes | `hid_reports.rs` | Keyboard HID report (includes report ID) |
| `HEADSET_REPORT_SIZE` | 64 bytes | `hid_reports.rs` | Headset HID report (no report ID) |
| `MAX_RGB_ZONES` | 12 | `hid_reports.rs` | Maximum RGB zones |
| `GAMESENSE_DEFAULT_PORT` | 27301 | `gamesense/` | GameSense HTTP port |
| `CACHE_THRESHOLD_MS` | 16 | `rgb/` | RGB cache (~60 FPS) |
| `APEX_PRO_TKL_2023_PRODUCT_ID` | `0x1628` | `devices/` | Primary focus device |

### HID Command Codes

| Code | Name | Purpose |
|------|------|---------|
| `0x09` | Apply | Save/apply settings |
| `0x21` | RgbControl | Zone-based RGB |
| `0x22` | Brightness | Brightness control |
| `0x25` | ReactiveMode | Reactive effects |
| `0x26` | ColorShift | Color shift effects |
| `0x2A` | PerKeyRgb | Per-key RGB (placeholder, protocol TBD) |
| `0x2D` | ActuationControl | Actuation point (experimental) |

---

## Architecture

### Module Structure

```
src/
├── lib.rs                 # Library entry, module declarations, prelude
├── main.rs                # CLI (clap-based, 15+ commands)
├── error.rs               # Error types (thiserror)
│
├── devices/               # Device discovery & HID communication
│   ├── mod.rs             # Device trait, DeviceInfo, product mappings
│   ├── discovery.rs       # DeviceManager with hidapi enumeration
│   ├── hid_reports.rs     # HID report builders & command protocol
│   ├── diagnostics.rs     # Device health checks & HID logging
│   ├── key_mapping.rs     # Per-key addressing & keyboard layouts
│   ├── zone_mapping.rs    # RGB zone definitions & mapping
│   ├── fuzz.rs            # Protocol fuzzing tools (developer)
│   ├── keyboards/
│   │   ├── mod.rs         # Keyboard trait (25+ methods)
│   │   ├── apex.rs        # Generic Apex-series implementations
│   │   └── apex_pro_tkl_2023.rs  # Apex Pro TKL 2023 specific (0x1628)
│   └── headsets/
│       └── mod.rs         # Headset implementations
│
├── rgb/                   # Color & lighting effects
│   ├── mod.rs             # Color, Effect, EffectEngine, RgbController
│   └── tests.rs           # RGB unit tests (11 tests)
│
├── gamesense/             # GameSense HTTP server
│   ├── mod.rs             # Data structures (GameMetadata, GameEvent)
│   ├── server.rs          # Axum HTTP server
│   └── handlers.rs        # Request handlers
│
├── performance.rs         # Performance monitoring & stats tracking
├── validation.rs          # Resource validation & leak detection
├── diagnostics_export.rs  # Bug report & diagnostic export
├── device_state.rs        # Device state tracking (async persistence)
│
├── audio/                 # Audio mixer (optional)
│   ├── mod.rs             # AudioMixer (PulseAudio)
│   └── sonar.rs           # SonarClient HTTP API
│
├── profiles/              # Configuration persistence
│   ├── mod.rs             # Profile struct & management
│   └── tests.rs           # Profile serialization tests
│
├── config/                # TOML configuration
│   └── mod.rs             # Config struct (~/.config/ssgg/)
│
└── pollrate.rs            # USB poll rate control (sysfs)
```

### CLI Commands

**Core Commands**:
- `devices` - List connected SteelSeries devices
- `rgb` - RGB lighting control (color, effects, per-key, brightness)
- `actuation` - Actuation point control (experimental)
- `profile` - Profile management (save/load/list/delete)
- `pollrate` - USB polling rate configuration
- `server` - Start GameSense HTTP server
- `daemon` - Run as background daemon

**Diagnostic Commands**:
- `validate` - Run validation tests on connected devices
- `performance` - Monitor RGB performance
- `status` - Real-time device connection status
- `bug-report` - Generate comprehensive diagnostic reports
- `test-device` - Automated device testing
- `verify-performance` - Verify RGB performance metrics
- `hid-logs` - View HID communication logs
- `fuzz` (hidden) - Protocol fuzzing (developer tool)

**Feature-gated Commands**:
- `audio` (feature: `audio`) - Audio mixer control
- `sonar` (feature: `sonar`) - SteelSeries Sonar API control

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
    // ... HID communication methods
}
```

**Key points**:
- `Device` trait provides common interface
- `DeviceManager` handles discovery via hidapi
- Product IDs map to device types and names
- Per-device implementations in `keyboards/` and `headsets/`

#### 2. Keyboard Trait (25+ Methods)

**Purpose**: Extended keyboard-specific functionality

```rust
pub trait Keyboard: Device {
    // RGB Control
    fn set_color(&mut self, color: Color) -> Result<()>;
    fn set_zone_colors(&mut self, colors: &[Color]) -> Result<()>;
    fn set_brightness(&mut self, brightness: u8) -> Result<()>;

    // Per-Key RGB
    fn supports_per_key_rgb(&self) -> bool;
    fn get_key_mapping(&self) -> Option<&KeyMapping>;
    fn set_key_color(&mut self, key_id: KeyId, color: Color) -> Result<()>;
    fn set_key_colors(&mut self, key_colors: &[(KeyId, Color)]) -> Result<()>;
    fn set_key_region(&mut self, start_row: u8, start_col: u8, rows: u8, cols: u8, color: Color) -> Result<()>;

    // Zone-based Fallback
    fn get_zone_mapping(&self) -> Option<&ZoneMap>;
    fn simulate_per_key_with_zones(&mut self, key_colors: &[(KeyId, Color)]) -> Result<()>;

    // Per-Key Effects
    fn set_per_key_effect(&mut self, effect: PerKeyEffect) -> Result<()>;
    fn trigger_key_reactive(&mut self, keys: &[KeyId], duration: f32) -> Result<()>;

    // Performance
    fn get_rgb_performance_stats(&self) -> Option<&PerformanceStats>;
    fn cleanup_rgb_caches(&mut self);

    // Actuation (Apex Pro)
    fn set_actuation_point(&mut self, value: u8) -> Result<()>;
    fn set_actuation_point_mm(&mut self, mm: f32) -> Result<()>;
    fn read_actuation_point(&mut self) -> Result<u8>;  // Placeholder
}
```

#### 3. HID Report Protocol

**Purpose**: Low-level hardware communication

**Report Structure**:
```rust
// Keyboard (65 bytes with report ID)
[Report ID: 1] [Command Code: 1] [Data: 63]

// Headset (64 bytes without report ID)
[Command Code: 1] [Data: 63]
```

**HidReportBuilder Pattern**:
```rust
let report = HidReportBuilder::new(HidDeviceType::Keyboard)
    .command(CommandCode::RgbControl)
    .zone_data(zone, &color)
    .build()?;
device.send(&report)?;
```

**Always use builders** to ensure correct padding and structure.

#### 4. Device Discovery & Caching

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

#### 5. RGB Effect Engine with Caching

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

#### 6. Performance Monitoring

**Purpose**: Real-time tracking of resource usage

```rust
pub struct PerformanceManager {
    stats: PerformanceStats,
    thresholds: PerformanceThresholds,
    frame_history: RingBuffer<Duration>,  // Last 60 frames
}
```

**Tracked metrics**:
- CPU usage per operation
- Memory allocations
- HID write latency
- Effect computation time
- Event loop timing
- Cache hit/miss rates

**Usage**: Enable with `PerformanceManager::new()` in daemon mode

#### 7. Resource Validation

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

#### 8. GameSense Server (Axum)

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
- Protocol fuzzing for reverse engineering

**Key types**:
- `DeviceManager` - Discovery & connection management
- `Device` trait - Common device interface
- `DeviceInfo` - Metadata (VID, PID, path, serial)
- `HidReportBuilder` - Type-safe HID report construction
- `KeyMapping` - Per-key addressing for RGB
- `ZoneMapping` - Zone definitions & fallback logic
- `CommandCode` - HID command code enum

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
- `PerKeyEffect` - Per-key effect types

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
- `GameSenseServer` - HTTP server

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
- Adaptive frame timing

**Key types**:
- `PerformanceManager` - Tracking orchestrator
- `PerformanceStats` - Current metrics
- `PerformanceThresholds` - Alert limits
- `RgbTimingMetrics` - RGB-specific timing

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

### `diagnostics_export` - Bug Reports

**Core responsibilities**:
- Generate comprehensive bug reports
- Export diagnostic information to JSON
- Collect system information
- Include HID communication logs

**Usage**:
```bash
ssgg bug-report --output report.json --include-hid-logs
```

### `audio` - Audio Mixing (Optional)

**Feature gates**: `audio`, `sonar`

**Core responsibilities**:
- Multi-channel mixing (Game, Chat, Media, Aux, Mic)
- PulseAudio integration
- Sonar HTTP API client

**Key types**:
- `AudioMixer` - Channel mixer
- `Channel` - Audio channel enum
- `MixerState` - Current mixer state
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

### `device_state` - State Persistence

**Storage**: `~/.config/ssgg/state.json`

**Purpose**: Track last-applied device settings for daemon mode

**Key types**:
- `DeviceStateStore` - Async state persistence
- `DeviceId` - FNV hash-based device identifier
- `KeyboardState` - Cached keyboard settings

---

## Development Workflows

### Adding a New CLI Command

1. **Define command**:
```rust
// src/main.rs
#[derive(Subcommand)]
enum Commands {
    MyCommand {
        #[arg(long)]
        option: String,
    },
    // ...
}
```

2. **Implement handler**:
```rust
fn cmd_my_command(option: &str) -> Result<()> {
    // Implementation
    Ok(())
}
```

3. **Wire up in main**:
```rust
match cli.command {
    Commands::MyCommand { option } => cmd_my_command(&option)?,
    // ...
}
```

### Testing Device Communication

**Debug logging**:
```bash
RUST_LOG=debug cargo run -- devices
RUST_LOG=trace cargo run -- rgb color red
cargo run -- --debug-hid devices    # Enable HID diagnostics
```

**Inspect HID reports**:
```rust
use tracing::debug;
debug!("HID report: {:02X?}", &report);
```

**Test RGB**:
```bash
cargo run -- rgb color red
cargo run -- rgb effect breathing
cargo run -- rgb effect spectrum
cargo run -- rgb perkey status
```

**Protocol fuzzing** (developer tool):
```bash
cargo run -- fuzz --start 0x20 --end 0x30 --delay 200
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

**~77 unit tests** across modules:
- RGB: Color, effects, caching, per-key addressing (11 tests)
- HID Reports: Builder patterns, command encoding (19 tests)
- Zone Mapping: Zone mapping, fallback logic (8 tests)
- Keyboards: Keyboard trait implementations (7 tests)
- Profiles: Serialization, defaults, validation (6 tests)
- Performance: Stats tracking, threshold detection (6 tests)
- Validation: Resource leak detection (6 tests)
- Key Mapping: Key addressing, layout support (6 tests)
- Pollrate: Conversion, validation (5 tests)
- GameSense: Color computation, event handling (3 tests)

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
HidReportBuilder::new(HidDeviceType::Keyboard)  // 65 bytes (with report ID)
HidReportBuilder::new(HidDeviceType::Headset)   // 64 bytes (no report ID)
```

**Why different sizes?**: Keyboards include report ID byte, headsets don't

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

**Problem**: `ssgg rgb effect breathing` shows no animation

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

**Problem**: `--features sonar` requires audio infrastructure

**Solution**: `Cargo.toml` does NOT enforce dependency (unlike earlier versions)

Use: `--features audio,sonar` or `--all-features` when using Sonar

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

### 11. Per-Key RGB Not Working

**Problem**: Per-key RGB commands have no effect

**Why**: The per-key RGB HID command (0x2A) is a placeholder - the actual protocol has not been reverse-engineered yet

**Workaround**: Use zone-based fallback with `simulate_per_key_with_zones()`

### 12. Actuation Point Read Not Implemented

**Problem**: `read_actuation_point()` always returns error

**Why**: The HID command to read actuation settings has not been discovered

**Status**: Write-only for now; read is a placeholder

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
- **Ring buffer**: Frame timing history for smooth metrics
- **Write-behind caching**: Async device state persistence

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

### Device State

`~/.config/ssgg/state.json` (async persistence)

---

## Debugging

### Enable Logging

```bash
RUST_LOG=debug cargo run -- devices
RUST_LOG=trace cargo run -- daemon
cargo run -- --debug-hid devices    # HID diagnostics
```

### Common Issues

| Issue | Check |
|-------|-------|
| Device not detected | udev rules, USB connection, permissions |
| RGB not working | Interface number, HID reports (RUST_LOG=debug) |
| Per-key RGB not working | Protocol not yet reverse-engineered (use zone fallback) |
| GameSense not responding | Port 27301 availability, firewall |
| Audio failing | PulseAudio running, feature flags enabled |
| High CPU | Performance stats, effect computation timing |
| Actuation not reading | Read command not implemented (write-only) |

### Diagnostic Tools

```bash
ssgg devices                    # List connected devices
ssgg status                     # Real-time status monitoring
ssgg hid-logs                   # View HID communication
ssgg bug-report                 # Generate diagnostic report
ssgg validate                   # Run validation tests
ssgg test-device <device>       # Automated device testing
ssgg verify-performance         # Check RGB performance
```

### Reporting Bugs

1. Enable `RUST_LOG=debug`
2. Generate bug report: `ssgg bug-report --include-hid-logs`
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
- `PROJECT_INDEX.md` - Complete project structure & exports
- `PLAN.md` - Development roadmap (per-key RGB focus)
- `AGENTS.md` - AI agent guidance
- `PERFORMANCE_OPTIMIZATIONS.md` - Optimization findings

### Development Docs
- `docs/development/` - Reports & notes
  - `APEX_PRO_PROTOCOL.md` - Apex Pro HID protocol research
  - `KEY_MAPPING_RESEARCH.md` - Per-key addressing research
  - `PROTOCOL_RESEARCH.md` - General protocol findings
  - `RGB_CONTROL_ANALYSIS.md` - RGB control analysis
  - `OPTIMIZATION_REPORT.md` - Performance findings
  - `DEPENDENCY_AUDIT_REPORT.md` - Security audit
  - `todo.md` - Development tasks

### Archive
- `docs/archive/` - Historical reports (not needed for active dev)

---

## Additional Resources

- **USB protocol**: See `docs/development/` for HID protocol notes
- **Device support**: Check README.md for current device list
- **GameSense API**: See `gamesense/handlers.rs` for endpoint details
- **RGB effects**: See `rgb/tests.rs` for effect examples
- **Per-key RGB**: See `PLAN.md` for implementation roadmap

---

## Current Development Focus

**Primary**: Per-key RGB control for Apex Pro TKL 2023
- Protocol reverse engineering in progress
- Command code 0x2A is placeholder
- Zone-based fallback working as interim solution
- See `docs/development/KEY_MAPPING_RESEARCH.md` for progress

**Secondary**: Actuation point control
- Write working (0x2D command)
- Read command not yet discovered
- Limited to Apex Pro series

---

**Last Updated**: 2026-01-25
**Maintainer**: steelseriesgg-rs contributors
**Version**: 0.1.0
