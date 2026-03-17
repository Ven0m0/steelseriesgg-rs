# steelseriesgg-rs

Open-source Linux replacement for SteelSeries GG: RGB lighting control, GameSense HTTP server (port 27301), and audio management for SteelSeries keyboards and headsets.

**Language**: Rust 2021 · **Toolchain**: pinned in `rust-toolchain.toml` (currently `1.94.0`) · **License**: MIT
**Binary**: `ssgg` (`src/main.rs`) · **Library**: `steelseries_gg` (`src/lib.rs`)
**Features**: `audio` (libpulse), `sonar` (reqwest) — default build has neither

---

## Source of Truth

When prose in this file drifts, prefer the live repo configuration:

- `Cargo.toml` — dependency versions, features, package metadata
- `rust-toolchain.toml` — pinned Rust toolchain and components
- `.github/workflows/*.yml` — CI triggers, matrices, and required commands
- `src/devices/hid_reports.rs` — current HID report serialization details

---

## Key Source Layout

```
src/
  main.rs                   # CLI entry (~3500+ LOC, 15+ subcommands via clap derive)
  lib.rs                    # Module declarations + prelude re-exports
  error.rs                  # Error enum (thiserror) + Result alias
  device_state.rs           # DeviceStateStore — per-device runtime state cache
  performance.rs            # PerformanceMonitor, PerformanceManager, RgbTimingMetrics
  pollrate.rs               # USB polling rate control (requires root)
  validation.rs             # RgbValidator, ValidationReport — hardware testing framework
  diagnostics_export.rs     # Bug report generator (JSON export)

  devices/
    mod.rs                  # Device trait, DeviceInfo, DeviceType, product IDs, HidOptimizer
    discovery.rs            # DeviceManager, DeviceFingerprint, HotPlugEvent
    hid_reports.rs          # HidReportBuilder, CommandCode — always use this, never raw buffers
    diagnostics.rs          # Global diagnostics store for HID communication logs
    fuzz.rs                 # Protocol fuzzer (developer tool, hidden CLI command)
    key_mapping.rs          # KeyId, KeyAddress, KeyMapping, KeyMappingDatabase
    zone_mapping.rs         # ZoneMapping, ZoneEffect, ZoneFallback
    keyboards/
      mod.rs                # GenericKeyboard, Keyboard trait
      apex.rs               # Apex 3 TKL (PID 0x1622)
      apex_pro_tkl_2023.rs  # Primary device (PID 0x1628), actuation point control
    headsets/
      mod.rs                # GenericHeadset, Headset trait

  rgb/
    mod.rs                  # Color, Effect, EffectEngine, RgbController, PerKeyRgbController
    tests.rs                # RGB unit tests

  gamesense/
    mod.rs                  # GameSense types (GameMetadata, EventBinding, etc.)
    server.rs               # Axum HTTP server on 127.0.0.1:27301, CORS security
    handlers.rs             # GameSense API request handlers

  config/mod.rs             # Config, GameSenseConfig — ~/.config/ssgg/config.toml
  profiles/
    mod.rs                  # Profile, ProfileManager, KeyboardProfile, HeadsetProfile
    tests.rs                # Profile unit tests

  audio/                    # feature-gated: requires `audio` or `sonar`
    mod.rs                  # AudioMixer, Channel
    pulse.rs                # libpulse-binding integration
    sonar.rs                # SonarClient — SteelSeries Sonar HTTP API client

  bin/
    discover_actuation.rs   # Utility to discover actuation point protocol bytes
    sonar_control.rs        # Standalone Sonar control binary (requires `sonar` feature)
    verify_key_mapping.rs   # Key mapping verification tool
    benchmark_rgb_alloc.rs  # RGB allocation benchmarking

tests/
  cors_security.rs          # Integration tests for GameSense CORS enforcement
  device_readback.rs        # Integration tests for device communication

docs/development/
  APEX_PRO_PROTOCOL.md      # USB HID protocol research notes for Apex Pro
  KEY_MAPPING_RESEARCH.md   # Per-key addressing research
  PROTOCOL_RESEARCH.md      # General protocol reverse-engineering notes
  RGB_CONTROL_ANALYSIS.md   # RGB command analysis

assets/
  99-steelseries.rules      # udev rules (install for non-root HID access)
  ssgg.service              # systemd service unit
```

---

## Supported Devices

### Keyboards (PID constants in `src/devices/mod.rs`)
| Device | PID | Notes |
|--------|-----|-------|
| Apex Pro | `0x1610` | |
| Apex Pro TKL | `0x1614` | |
| Apex Pro TKL (2023) | `0x1628` | **Primary tested device** |
| Apex 3 | `0x161A` | |
| Apex 3 TKL | `0x1622` | |
| Apex 5 | `0x161C` | |
| Apex 7 | `0x1612` | |
| Apex 7 TKL | `0x1616` | |

### Headsets (Arctis Series)
Arctis 1, Arctis 1 Wireless, Arctis 5, Arctis 7 (2019), Arctis 9, Arctis Pro, Arctis Pro Wireless, Arctis Nova Pro, Arctis Nova Pro Wireless, Arctis Nova 5, Arctis Nova 3, Arctis Nova 1.

---

## Conventions

### Naming
- `snake_case` — functions, variables, modules
- `PascalCase` — types, traits, enums
- `SCREAMING_SNAKE_CASE` — constants

### Formatting
- 4-space indent, 120-char max line, Unix LF
- `rustfmt.toml` uses `edition = "2024"`, `style_edition = "2024"`
- Run `cargo fmt` before committing — CI enforces this

### Error Handling
- **Library code** (`src/lib.rs` and submodules): use `thiserror` variants from `crate::error::Error`
- **Binary code** (`src/main.rs`, `src/bin/`): use `anyhow` + `.context("description")`
- Never use `.unwrap()` or `.expect()` — use `?` or map to a typed error

### HID Reports
- Always use `HidReportBuilder` from `src/devices/hid_reports.rs`
- **Keyboards**: 65-byte reports (report ID byte in position 0) → `KEYBOARD_REPORT_SIZE`
- **Headsets**: 64-byte reports (no report ID) → `HEADSET_REPORT_SIZE`
- Never construct raw byte arrays manually; use `CommandCode` enum + typed command structs

### Logging
- Use `tracing::{debug, info, warn}` — never `println!` in library code
- Initialize with `FmtSubscriber` in `main.rs`
- `RUST_LOG=debug` or `--debug` flag enables verbose output

---

## Essential Commands

```bash
# Local prerequisites
# No extra packages are required for the default build.
sudo apt-get install -y libpulse-dev            # only for --features audio
./setup_mock_udev.sh                            # optional fallback in CI-like environments

# Build
cargo build                             # default (no audio/sonar)
cargo build --features sonar            # Sonar client
cargo build --features audio            # with libpulse audio mixer
cargo build --features audio,sonar      # all optional features without --all-features
cargo build --all-features              # everything

# Test
cargo test                              # default feature set
cargo test --features sonar             # CI-covered optional feature set
cargo test --all-features               # broader local verification
cargo test -p steelseries-gg-linux      # explicit package

# Format & Lint (CI-enforced, must pass before push)
cargo fmt                               # format all code
cargo fmt --all -- --check              # CI format check
cargo clippy --all-targets --locked -- -D warnings
cargo clippy --all-targets --locked --features sonar -- -D warnings
cargo clippy --all-targets --locked --features audio -- -D warnings

# Run CLI
cargo run -- devices                    # list connected devices
cargo run -- rgb color FF0000           # set red
cargo run -- rgb effect breathing       # breathing animation
cargo run -- server                     # start GameSense server on :27301
cargo run -- daemon                     # full daemon mode
cargo run -- validate                   # validate connected hardware
cargo run -- bug-report                 # generate JSON diagnostic report
```

---

## Feature Flags

| Feature | Enables | Deps |
|---------|---------|------|
| *(none)* | Core RGB, GameSense server, profiles | — |
| `audio` | `AudioMixer`, `Channel`, PulseAudio integration | `libpulse-binding` |
| `sonar` | `SonarClient` — SteelSeries Sonar HTTP API | `reqwest` |
| `--all-features` | Everything above | All system deps |

**System packages** needed for full build (Ubuntu/Debian):
```bash
sudo apt-get install -y libpulse-dev
```

---

## Architecture Patterns

### Device Abstraction
- `Device` trait (`src/devices/mod.rs`) — base interface: `initialize`, `send_raw`, `receive_raw`, `close`
- `Keyboard` trait (`src/devices/keyboards/mod.rs`) — extends `Device` with RGB, actuation
- `Headset` trait (`src/devices/headsets/mod.rs`) — extends `Device` with audio control
- `DeviceManager` (`src/devices/discovery.rs`) — HID enumeration via `hidapi`, hot-plug support

### HID Communication Pipeline
```
CLI command
  → HidCommand trait impl (RgbZoneCommand, ActuationCommand, etc.)
  → HidReportBuilder::build_report()  [validation + serialization]
  → write_padded_report()             [HidOptimizer dedup cache + actual write]
  → hidapi::HidDevice::write()
```

### HidOptimizer
Global singleton (`OnceLock<HidOptimizer>`) in `src/devices/mod.rs`:
- FNV-1a hashes each report; caches for 50ms to skip duplicate writes
- Connectivity cache with 5s TTL to reduce redundant open/close checks
- `HidDeviceType::Keyboard` = 65 bytes (with report ID); `Headset` = 64 bytes

### Performance System (`src/performance.rs`)
- `PerformanceMonitor` — ring buffer of last 60 frames for timing/computation history
- `PerformanceManager` — adaptive refresh rates, effect computation caching with smart invalidation
- `RgbTimingMetrics` — serializable snapshot (target/actual FPS, dropped frames, cache hit rate)

### GameSense Server (`src/gamesense/`)
- Axum HTTP server on `127.0.0.1:27301`, compatible with SteelSeries GameSense API
- CORS restricted to same-origin (`127.0.0.1` only) — security enforced in `server.rs`
- Game registration, event binding, RGB callbacks
- State stored in `Arc<RwLock<ServerState>>`

### Profile System (`src/profiles/mod.rs`)
- `Profile` — top-level JSON config with optional `KeyboardProfile` + `HeadsetProfile`
- Stored in `~/.config/ssgg/profiles/`; managed via `ProfileManager`
- `Config` at `~/.config/ssgg/config.toml` controls defaults and GameSense settings

---

## CLI Subcommands

Main binary `ssgg` (via `clap` derive):

| Subcommand | Description |
|-----------|-------------|
| `devices` | List connected SteelSeries devices |
| `rgb color <hex\|name>` | Set static color |
| `rgb brightness <0-100>` | Set brightness |
| `rgb effect <name>` | Set effect (static, breathing, spectrum, wave, off) |
| `rgb perkey` | Per-key RGB control |
| `actuation set <value>` | Set actuation point (Apex Pro TKL 2023) |
| `profile` | Save/load/list profiles |
| `audio` | Audio mixer (requires `audio` feature) |
| `sonar` | Sonar API control (requires `sonar` feature) |
| `pollrate` | USB polling rate (requires root) |
| `server` | Start GameSense HTTP server |
| `daemon` | Run as full daemon (device + server) |
| `validate` | Run hardware validation tests |
| `performance` | Monitor/control RGB performance |
| `bug-report` | Generate JSON diagnostic report |
| `status` | Real-time device connection status |
| `hid-logs` | View HID communication logs |
| `test-device` | Run automated device tests |
| `verify-performance` | Monitor RGB performance metrics |
| `fuzz` | *(hidden)* Protocol fuzzer for reverse engineering |

---

## Architecture & Concurrency Rules

- `hidapi` is pinned at **`=2.6.5`** (`Cargo.toml`) — do not change this constraint
- Apex Pro TKL 2023 product ID is **`0x1628`** (not `0x1618`)
- Per-key RGB protocol details are still evolving — verify the current implementation in `src/devices/hid_reports.rs` and keyboard code before changing it
- Actuation point **write** works via command `0x2D` (`ActuationControl`); read-back is not yet implemented
- CLI RGB commands are one-shot; continuous animations require `ssgg daemon`
- GameSense server CORS is restricted to localhost origin — do not loosen without security review
- `write_padded_report` silently deduplicates identical reports within 50ms (HidOptimizer cache)
- `DeviceManager` supports hot-plug detection via `HotPlugEvent` channel
- The `fuzz` subcommand is hidden from help but sends raw HID bytes — use only for protocol research

---

## Commit Format

```
<type>: <description>
```

Types: `feat` `fix` `refactor` `docs` `test` `perf` `chore` `style`

Examples:
```
feat: add per-key RGB support for Apex 3 TKL
fix: correct actuation point byte order in HID report
perf: reduce HID write latency with report deduplication
docs: update APEX_PRO_PROTOCOL with 0x2D findings
chore: bump hidapi to =2.6.5
```

---

## CI/CD (`.github/workflows/`)

| Workflow | Trigger | Jobs |
|---------|---------|------|
| `ci.yml` | push/PR to `main` | fmt, clippy (3 feature combos), test (2 combos), build (3 combos) |
| `build.yml` | `workflow_dispatch`, `merge_group` | release build |
| `release-arch.yml` | tag push (`v*`), `workflow_dispatch`, `merge_group` | Arch Linux PKGBUILD release |
| `cargo-assist.yml` | push | dependency assistance |
| `dependabot.yml` / `renovate.json` | scheduled | automated dep bumps |

CI runs clippy with `--features ""`, `--features sonar`, `--features audio` separately.
All clippy warnings are treated as errors (`-D warnings`).
