# GitHub Copilot Instructions — steelseriesgg-rs

Code-generation guardrails for **steelseriesgg-rs**: open-source SteelSeries GG replacement for Linux.

> Full AI agent reference: [`AGENTS.md`](../AGENTS.md)

---

## Project Context

- **Language**: Rust 2021, toolchain pinned at 1.93.1 (`rust-toolchain.toml`)
- **Formatting**: `rustfmt` edition 2024, `max_width = 120`, 4-space indent
- **Binary**: `ssgg` — CLI + daemon (RGB lighting, GameSense server, audio)
- **Library**: `steelseries_gg` — public API
- **Stack**: Tokio (async), Axum (HTTP), hidapi **=2.6.5** (pinned), clap 4.5 (CLI)
- **Features**: `audio` (libpulse), `sonar` (reqwest)

---

## Core Principles

1. **Edit > Create** — modify existing code; avoid new files unless essential
2. **Minimal changes** — only what is requested; no speculative features
3. **Follow existing patterns** — match conventions already in the codebase
4. **Safety first** — no injection, no panics in production, no security regressions
5. **Zero warnings** — `cargo clippy -- -D warnings` must pass

---

## Naming Conventions

```rust
fn snake_case_function() {}          // functions & variables
struct PascalCaseType {}             // types, structs, enums
const SCREAMING_SNAKE: u16 = 0x1038; // constants
mod snake_case_module {}             // modules
```

---

## Error Handling & Reliability

```rust
// Library code — thiserror
use thiserror::Error;
#[derive(Error, Debug)]
pub enum DeviceError {
    #[error("Device not found: {0}")]
    NotFound(String),
    #[error("HID error: {0}")]
    HidError(#[from] hidapi::HidError),
}

// Binary/CLI code — anyhow
use anyhow::{Context, Result};
fn cmd() -> Result<()> {
    device.init().context("Failed to init device")?;
    Ok(())
}

// NEVER .unwrap() or .expect() in production — always use match, if let, or ?
```

- **Unwraps**: Idiomatic Rust pattern matching (`match` or `if let`) is strongly preferred over `.unwrap()` or `.expect()`.
- **reqwest::Error**: Does not support `.is_request()`. Use `.is_timeout()` and `.is_connect()`. It is not `Clone`.

---

## HID Reports — Use the Builder

```rust
// CORRECT
let report = HidReportBuilder::new(HidDeviceType::Keyboard)
    .command(CommandCode::RgbControl)
    .zone_data(zone, &color)
    .build()?;

// WRONG — manual buffer construction
let mut buf = [0u8; 65];
buf[1] = 0x21;
```

- Keyboards: **65 bytes** (with report ID)
- Headsets: **64 bytes** (no report ID)

---

## Async Patterns & File I/O

```rust
use tokio::sync::{Mutex, RwLock};
let state = Arc::new(Mutex::new(data));

tokio::spawn(async move {
    loop {
        work().await;
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
});
```

- **tokio::fs**: Always use `tokio::fs` for file operations to prevent blocking the async runtime.
- **Exists checks**: Replace `Path::exists()` with `tokio::fs` operations that gracefully handle `ErrorKind::NotFound`.
- **Mutexes**: Structs containing `parking_lot::Mutex` prevent `Clone` derivation; wrap them in `Arc` or `OnceLock`.

---

## Security Guardrails

- **CORS Exact Matching**: Avoid `.starts_with("http://localhost")` checks for CORS origins, as they bypass security (e.g., `localhost.evil.com`). Use strict exact string or domain matching.
- **Shared Directories**: Use `O_NOFOLLOW`, restrictive permissions (`0o644`/`0o755`), and verify directory ownership when writing to `/tmp` to avoid symlink/TOCTOU attacks.

---

## Critical Constants

```rust
STEELSERIES_VENDOR_ID: u16        = 0x1038
APEX_PRO_TKL_2023_PRODUCT_ID: u16 = 0x1628  // NOT 0x1618
KEYBOARD_REPORT_SIZE: usize        = 65
HEADSET_REPORT_SIZE: usize         = 64
MAX_RGB_ZONES: usize               = 12
GAMESENSE_DEFAULT_PORT: u16        = 27301
```

---

## Required Commands Before Commit

```bash
cargo fmt
cargo clippy --all-features -- -D warnings
cargo test --all-features
cargo build --release
```

---

## Key Gotchas

- `hidapi` is pinned at `=2.6.5` — do not change the version specifier
- Apex Pro TKL 2023 PID is `0x1628` (not `0x1618`)
- Per-key RGB (`0x2A`) is a placeholder — protocol not reversed
- CLI effect commands are one-shot; animations need `ssgg daemon`
- `rustfmt` max\_width is **120** (not 100)
- **Dependencies**: Managed by Dependabot and Renovate (`.github/renovate.json`).
