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
- **Formatting**: 4-space indent, 120-char max line, Unix LF (`rustfmt.toml`). Formatting is CI-enforced project-wide via `cargo fmt`.
- **Errors**: `thiserror` in library code, `anyhow` + `.context()` in binary code; idiomatic Rust pattern matching (`match` or `if let`) is preferred over `.unwrap()`/`.expect()`.
- **HID reports**: always use `HidReportBuilder` — keyboards 65 bytes (with report ID), headsets 64 bytes.

## Essential Commands

```bash
cargo build --all-features          # build
cargo test --all-features           # ~77 tests
cargo fmt                           # format (CI-enforced)
cargo clippy --all-features -- -D warnings  # lint (zero warnings required)
```

## Architecture & Concurrency Rules

- **Split-Phase Concurrency**: High-frequency animation loops (like in `src/main.rs`) use a split-phase concurrency pattern: data is collected into persistent local buffers (e.g., `frame_data_buffer`) while holding the state lock, then the lock is released, and heavy I/O operations (applying colors/overlays) are processed concurrently or sequentially.
- **Memory Reuse**: The animation loop in `cmd_daemon` uses a `Vec<(String, Vec<Color>)>` declared outside the loop to reuse memory across frames, minimizing allocations by using `clear()` and `extend_from_slice()`. The `PerKeyEffectEngine` utilizes an in-place update strategy for `cached_key_colors` using `apply_effect_static`. `DaemonState::device_names` returns a `Vec<String>`, cloning each `name` field from the internal device info.
- **Async I/O**: `Config` provides asynchronous methods (e.g., `load_async`) using `tokio::fs` to prevent blocking the async runtime. `ProfileManager` currently exposes synchronous APIs only (`new`, `load_all`, `save`, `get`, `set`, `delete`). Prefer `tokio::fs` operations over `Path::exists()` and handle `ErrorKind::NotFound` gracefully. The `apply_per_key_effect_with_brightness` method in the `Keyboard` trait is asynchronous (via `#[async_trait]`).
- **Structs with Mutexes**: Structs containing `parking_lot::Mutex` (e.g., `HidOptimizer`) generally should not automatically derive `Clone`; they must be shared via `Arc` or `OnceLock`.

## Security & Validation

- **CORS Configuration**: When configuring CORS via `AllowOrigin::predicate`, avoid `starts_with` checks for `localhost` or `127.0.0.1` as they are vulnerable to prefix-matching bypasses. Use strict exact string or domain matching.
- **Shared Directories**: File operations in shared directories (e.g., `/tmp`) must mitigate symlink and TOCTOU attacks by verifying directory ownership (`uid == getuid`), ensuring no symlinks exist in the path, and using `O_NOFOLLOW` with restrictive permissions (`0o644`/`0o755`).
- **GameSense Limits**: The current GameSense server implementation does **not** yet enforce hard limits on games, events, handlers, identifier length, or request body size. When adding or modifying GameSense functionality, introduce server-side limits (for example: 128 max games, 256 max events per game, 32 max handlers per event, 64-character identifier limits, and a 64KB global request body limit), and keep this document in sync with the code. Be aware that `game_event` currently stores every event value in `event_values` regardless of whether a corresponding binding exists; if that behavior is changed, update these notes accordingly.

## Testing Conventions

- **Global Statics**: Tests interacting with global static variables (like `OnceLock<parking_lot::Mutex<...>>`) must serialize access using a test-specific static Mutex (`static TEST_MUTEX: parking_lot::Mutex<()> = parking_lot::const_mutex(());`).
- **Mock udev**: Testing `hidapi`-dependent code without `libudev` headers requires a full mock setup. The mock udev files (`mock_udev/mock_udev.o`, `mock_udev/lib/libudev.a`) are tracked in git; do not delete them.
- **HTTP Tests**: Use `reqwest` crate for HTTP requests to local servers in tests to prevent connection hanging issues (instead of manual `TcpStream` writes).
- **Validation Skipped Gracefully**: Validation tests must dynamically check hardware capabilities and gracefully skip unsupported features by returning `ValidationResult::success` with a note.
- **Feature Flag Testing**: To replicate CI, run checks with `--features sonar,audio`. When verifying unused imports, run checks with feature flags isolated (e.g., `cargo check --features sonar` without `audio`).
- **Performance Benchmarks**: Skip performance benchmarks during code health or refactoring tasks unless explicitly requested.
- **HidOptimizer Tests**: Unit tests for `HidOptimizer` must be placed within the module (`#[cfg(test)] mod tests`) to access private fields and rely on `mark_report_sent`.

## Domain Gotchas

- **Reqwest Errors**: `reqwest::Error` doesn't support `is_request()`; use `is_timeout()` and `is_connect()`. It is not `Clone`, so format it into error strings *before* returning in retry logic.
- **Consistent Key Ordering**: Features relying on consistent key ordering must iterate over `KeyMapping::get_all_keys()` (slice iteration) rather than the underlying `HashMap` to guarantee stability.
- **DeviceId**: Uses public fields for instantiation (no `new` constructor). The `to_key` method generates a string key where the file path is hashed, causing `from_key` deserialization to lose the original path information.
- **Sonar Channel**: `SonarChannel` does not implement `clap::ValueEnum`; CLI tools must define a local enum. In `Classic` mode, explicitly exclude the `ChatCapture` channel.
- **Type Inference**: Calls to `hidapi::HidDevice` string methods inside `map` closures require explicit type annotations (e.g., `|s: &str|`).
- **VecDeque**: `computation_times` and `hid_times` in `AdaptiveRefreshController` are `VecDeque` structures, requiring `push_back` instead of `push`.
- **Feature-Gated Imports**: Top-level imports only consumed by code guarded by a specific feature flag must be identically gated.
- **CLI Output**: Error messages in CLI modules must dynamically determine the executable name using `std::env::current_exe()`.
- **System Requirements**: Requires `libudev-dev`, `pkg-config`, `libhidapi-dev` (or `libhidapi-libusb0`), and `libpulse-dev`. May experience network timeouts (`cargo ... --offline`).
- **Single HidApi Import**: `src/devices/discovery.rs` must contain a single `use hidapi::HidApi;` statement to avoid `E0252`.
- **Orphaned Helpers**: When removing unused functions, explicitly check for and remove private helper functions that become orphaned.
- **Retry Loops**: Retry loops returning on the final attempt must not have code following the loop; use `unreachable!()` if needed.
- `hidapi` is pinned at **`=2.6.5`** — do not change this constraint.
- Apex Pro TKL 2023 product ID is **`0x1628`** (not `0x1618`).
- Per-key RGB command `0x2A` is a **placeholder** — protocol unknown; use `simulate_per_key_with_zones()` fallback.

## Workflow

- **Deep Planning Mode**: Before starting a task, engage in a 'deep planning mode' to clarify requirements. Once approved, execute autonomously.
- **Dependencies**: The project uses Dependabot and Renovate (`.github/renovate.json`).
