# steelseriesgg-rs

Open-source Linux replacement for SteelSeries GG: RGB lighting control, GameSense HTTP server (port 27301), and audio management for SteelSeries keyboards and headsets.

**Language**: Rust 2021 · **Toolchain**: 1.93.1 (pinned) · **License**: MIT
**Binary**: `ssgg` (`src/main.rs`) · **Library**: `steelseries_gg` (`src/lib.rs`)
**Features**: `audio` (libpulse), `sonar` (reqwest) — default build has neither

## Key Source Layout

```
src/
  main.rs              # CLI entry (~3300 LOC, 15+ subcommands via clap derive)
  lib.rs               # Module declarations + prelude re-exports
  error.rs             # Error enum (thiserror) + Result alias
  devices/
    mod.rs             # Device trait, DeviceInfo, product IDs
    discovery.rs       # DeviceManager — hidapi enumeration
    hid_reports.rs     # HidReportBuilder — always use this, never raw buffers
    keyboards/apex_pro_tkl_2023.rs  # Primary device (PID 0x1628)
  rgb/mod.rs           # Color, Effect, EffectEngine, RgbController
  gamesense/server.rs  # Axum HTTP server on 127.0.0.1:27301
  config/mod.rs        # ~/.config/ssgg/config.toml
```

## Conventions

- **Naming**: `snake_case` functions/vars, `PascalCase` types, `SCREAMING_SNAKE` constants
- **Formatting**: 4-space indent, 120-char max line, Unix LF (`rustfmt.toml`)
- **Errors**: `thiserror` in library code, `anyhow` + `.context()` in binary code; never `.unwrap()`/`.expect()`
- **HID reports**: always use `HidReportBuilder` — keyboards 65 bytes (with report ID), headsets 64 bytes

## Essential Commands

```bash
cargo build --all-features          # build
cargo test --all-features           # ~77 tests
cargo fmt                           # format (CI-enforced)
cargo clippy --all-features -- -D warnings  # lint (zero warnings required)
```

## Critical Facts

- `hidapi` is pinned at **`=2.6.5`** — do not change this constraint
- Apex Pro TKL 2023 product ID is **`0x1628`** (not `0x1618`)
- Per-key RGB command `0x2A` is a **placeholder** — protocol unknown; use `simulate_per_key_with_zones()` fallback
- Actuation point **write** works (`0x2D`); read is not yet implemented
- `sonar` feature requires `audio` too — use `--features audio,sonar` or `--all-features`
- CLI RGB commands are one-shot; continuous animations require `ssgg daemon`
- Commit format: `<type>: <description>` where type ∈ `feat fix refactor docs test perf chore style`
