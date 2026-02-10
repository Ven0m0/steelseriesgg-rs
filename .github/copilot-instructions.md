# GitHub Copilot Instructions - steelseriesgg-rs

Code generation guardrails for **steelseriesgg-rs** - an open-source SteelSeries GG replacement for Linux.

## Project Context

- **Language**: Rust 2021 (rustfmt edition 2024, 100 char max)
- **Stack**: Tokio (async), Axum (HTTP), hidapi 2.6.4 (HID), clap 4.5 (CLI)
- **Binary**: `ssgg` (CLI + daemon for RGB lighting, GameSense server, audio management)
- **Library**: `steelseries_gg` (public API)
- **Target**: SteelSeries keyboards & headsets on Linux

**Key Documentation**: `AGENTS.md` (comprehensive AI guide), `README.md` (user docs)

---

## Core Principles

1. **User instructions override defaults** - Direct requests take precedence
2. **Edit > Create** - Modify existing code, avoid creating new files
3. **Subtraction > Addition** - Minimal changes, no over-engineering
4. **Follow existing patterns** - Match codebase conventions
5. **Safety first** - No security vulnerabilities (injection, XSS, etc.)
6. **No speculative features** - Only implement what's requested

---

## Rust Style & Conventions

### Formatting (ENFORCED by CI)

```rust
// Naming
fn my_function() {}                 // snake_case
struct MyType {}                    // PascalCase
const MY_CONST: u32 = 42;          // SCREAMING_SNAKE_CASE

// Style (rustfmt.toml)
max_width = 100
tab_spaces = 4
hard_tabs = false
newline_style = "Unix"
edition = "2024"
```

### Error Handling

```rust
// Binaries: anyhow::Result
use anyhow::{Result, Context};
fn cmd() -> Result<()> {
    device.init().context("Failed to init")?;
    Ok(())
}

// Libraries: thiserror::Error
#[derive(Error, Debug)]
pub enum DeviceError {
    #[error("Device not found: {0}")]
    NotFound(String),
    #[error("HID error: {0}")]
    HidError(#[from] hidapi::HidError),
}

// NEVER .unwrap() or .expect() in production
// Use ? operator or proper handling
```

### Async/Tokio Patterns

```rust
// Shared state
use tokio::sync::{Mutex, RwLock};
let shared = Arc::new(Mutex::new(state));

// Background tasks
tokio::spawn(async move {
    loop {
        // work
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
});

// Axum HTTP server
let app = Router::new()
    .route("/event", post(handler))
    .layer(CorsLayer::permissive())
    .with_state(state);

let listener = TcpListener::bind("127.0.0.1:27301").await?;
axum::serve(listener, app).await?;
```

---

## Architecture Patterns (CRITICAL)

### 1. HID Report Builder (ALWAYS USE)

```rust
// CORRECT - Type-safe, guaranteed valid
let report = HidReportBuilder::new(HidDeviceType::Keyboard)
    .command(CommandCode::RgbControl)
    .zone_data(zone, &color)
    .build()?;
device.send_report(&report)?;

// WRONG - Manual construction (fragile)
let mut report = [0u8; 65];
report[0] = 0x00;
report[1] = 0x21;  // Error-prone
```

**Critical constants:**
- Keyboards: 65 bytes (with report ID)
- Headsets: 64 bytes (no report ID)
- `STEELSERIES_VENDOR_ID = 0x1038`
- `APEX_PRO_TKL_2023_PRODUCT_ID = 0x1628` (NOT 0x1618!)

### 2. Device Trait Pattern

```rust
// Common interface for all hardware
pub trait Device: Send + Sync {
    fn info(&self) -> &DeviceInfo;
    fn initialize(&mut self) -> Result<()>;
    fn send_report(&mut self, data: &[u8]) -> Result<()>;
    // ... 20+ methods
}

// Keyboard extends Device with RGB
pub trait Keyboard: Device {
    fn set_color(&mut self, color: Color) -> Result<()>;
    fn set_zone_colors(&mut self, colors: &[Color]) -> Result<()>;
    fn set_brightness(&mut self, brightness: u8) -> Result<()>;
    fn supports_per_key_rgb(&self) -> bool;
    // ... 25+ methods
}
```

### 3. RGB Effect Engine with Caching

```rust
// Create effect
let mut engine = EffectEngine::new(Effect::Breathing {
    color: Color::new(0, 255, 255),
    speed: 2.0,
});

// Compute (cached if Δt < 16ms = ~60 FPS)
let colors = engine.compute(num_zones, elapsed);

// CRITICAL: First call always computes
// Cache check: last_compute_time != Duration::ZERO
```

**Supported effects:**
- Static, Breathing, Spectrum, Wave, Reactive, Gradient, Custom, Off

### 4. Performance Monitoring

```rust
// Track critical paths
let perf = PerformanceManager::new();
perf.start_operation("rgb_update");
// ... work
perf.end_operation("rgb_update");
```

---

## Code Generation Guidelines

### Prefer Built-in Patterns

```rust
// PREFER
if let Some(val) = option { ... }
let items: Vec<_> = iter.collect();

// AVOID
match option { Some(val) => { ... }, None => {} }
let mut items = Vec::new();
for item in iter { items.push(item); }
```

### Minimal Diffs

When editing:
- Change only what's necessary
- Don't reformat unrelated code
- Preserve existing comments
- Keep existing error messages

### NO Over-Engineering

**DON'T add unless requested:**
- Extra abstraction layers
- Premature optimization
- Speculative features
- Excessive docstrings (on private functions)
- Error handling for impossible cases
- Feature flags for hypothetical needs
- Backwards-compatibility hacks

**Example - TOO MUCH:**
```rust
/// Internal color setter (UNNEEDED for internal fn)
fn set_color_internal(&mut self, color: Color) -> Result<()> {
    // Validate (UNNEEDED - type guarantees validity)
    if color.r > 255 { return Err(...); }
    self.color = color;
    Ok(())
}
```

**Example - JUST RIGHT:**
```rust
fn set_color_internal(&mut self, color: Color) {
    self.color = color;
}
```

---

## Security Requirements (MANDATORY)

**NEVER introduce:**
- Command injection (validate shell inputs)
- Path traversal (sanitize file paths)
- Buffer overflows (use safe Rust patterns)
- Unvalidated user input to system calls
- Hardcoded credentials/API keys

**For HID communication:**
- Always validate buffer sizes
- Use type-safe builders (HidReportBuilder)
- Bounds check array access
- Sanitize device paths

---

## Testing Standards

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_blending() {
        let c1 = Color::new(255, 0, 0);
        let c2 = Color::new(0, 0, 255);
        let blended = c1.blend(&c2, 0.5);
        assert_eq!(blended, Color::new(127, 0, 127));
    }
}
```

**Coverage expectations:**
- Unit tests for all public APIs
- Edge cases (empty, max, invalid)
- Error paths (Result::Err cases)
- Feature flag combinations

**Current: ~77 tests** (RGB, HID, zones, keyboards, profiles, performance, validation)

---

## Dependencies (DO NOT add without justification)

**Core:**
- hidapi 2.6.4 (pinned)
- tokio 1.49 (rt-multi-thread, macros)
- axum 0.8 (HTTP server)
- clap 4.5 (CLI derive)
- serde/serde_json (serialization)
- thiserror 2.0 (errors)

**Optional:**
- libpulse-binding 2.30 (audio feature)
- reqwest 0.13 (sonar feature)

---

## Critical Gotchas

1. **HID Report Sizing**: Keyboards=65 bytes, Headsets=64 bytes (always use HidReportBuilder)
2. **Interface Numbers**: Keyboards=interface 1, Headsets=interface 3
3. **Product IDs**: Apex Pro TKL 2023 = `0x1628` (NOT `0x1618`)
4. **Animated Effects**: CLI is one-shot; use daemon mode for animations
5. **Per-Key RGB**: Protocol not reverse-engineered (use zone fallback)
6. **RGB Caching**: First compute always executes (check `last_compute_time != Duration::ZERO`)
7. **Actuation Read**: Not implemented (write-only, read is placeholder)

---

## Feature Flags

```rust
#[cfg(feature = "audio")]
pub mod audio;

#[cfg(feature = "sonar")]
use crate::audio::sonar::SonarClient;

// Always test with all feature combinations:
// cargo test --all-features
```

---

## CLI Commands (clap derive API)

```rust
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Rgb {
        #[arg(long)]
        color: String,
        #[arg(long)]
        brightness: Option<u8>,
    },
}

fn cmd_rgb(color: &str, brightness: Option<u8>) -> Result<()> {
    let color = Color::parse(color)?;
    let mut controller = RgbController::new()?;
    controller.set_color(color)?;
    if let Some(b) = brightness {
        controller.set_brightness(b)?;
    }
    Ok(())
}
```

---

## Performance Optimization

### Release Profile

```toml
[profile.release]
strip = true           # ~30% size reduction
lto = "fat"            # ~15% perf gain
codegen-units = 1
panic = "abort"        # ~10% size reduction
opt-level = 3
overflow-checks = false
```

**Result:** ~2-3 MB binary, <100ms startup, <5ms RGB latency

### Runtime Best Practices

1. **Reuse buffers:**
   ```rust
   // GOOD
   let mut buf = Vec::with_capacity(MAX_ZONES);
   loop { buf.clear(); /* reuse */ }

   // BAD
   loop { let buf = Vec::new(); /* allocates */ }
   ```

2. **Use caching:** RGB effects cached for 16ms (~60 FPS)
3. **Batch HID writes:** When possible
4. **Minimize allocations:** In hot paths
5. **Use const fn:** For compile-time computation

---

## Git Workflow

**Commit messages:**
```
feat: Add per-key RGB fallback using zones
fix: RGB caching returns black on first compute
refactor: Extract HID report builder pattern
docs: Update AGENTS.md with common gotchas
test: Add unit tests for Color blending
perf: Optimize HID protocol by 20%
```

**Before commit (REQUIRED):**
```bash
cargo fmt                          # Format
cargo clippy --all-features        # Lint (no warnings)
cargo test --all-features          # Test
cargo build --release              # Build
```

---

## Module Organization

```
src/
├── main.rs              # CLI entry (15+ commands)
├── lib.rs               # Library entry
├── error.rs             # Error types
├── devices/             # Hardware layer
│   ├── mod.rs           # Device trait, discovery
│   ├── hid_reports.rs   # HID protocol
│   ├── keyboards/       # Keyboard impls
│   └── headsets/        # Headset impls
├── rgb/                 # Color & effects
├── gamesense/           # HTTP server
├── performance.rs       # Monitoring
├── validation.rs        # Health checks
├── profiles/            # Config persistence
└── audio/               # Audio (optional)
```

**When adding code:**
- Place in appropriate module
- Update `mod.rs` declarations
- Add to `lib.rs` if public API
- Update `PROJECT_INDEX.md` if significant

---

## Documentation Guidelines

**Only add when valuable:**
- Complex algorithms (RGB effects, HID protocol)
- Public API surface
- Non-obvious behavior
- Safety invariants

**Don't document:**
- Self-explanatory code
- Private helpers
- Standard patterns
- Temporary debug code

**Format:**
```rust
/// Computes RGB colors for the current effect.
///
/// Results are cached for 16ms (~60 FPS) to reduce CPU usage.
/// First call always computes (cache is empty).
pub fn compute(&mut self, num_zones: usize, elapsed: Duration) -> &[Color] {
    // Implementation
}
```

---

## Common Tasks Quick Reference

```bash
# Build
cargo build --release

# Test
cargo test --all-features

# Run
cargo run -- devices
cargo run -- rgb color red
cargo run -- daemon

# Debug
RUST_LOG=debug cargo run -- devices
cargo run -- --debug-hid devices

# Lint (REQUIRED before commit)
cargo clippy --all-features -- -D warnings
```

---

## Additional Resources

- **Comprehensive guide**: `AGENTS.md` (50KB AI reference)
- **User docs**: `README.md`
- **Contributing**: `CONTRIBUTING.md`
- **Project structure**: `PROJECT_INDEX.md`
- **Roadmap**: `PLAN.md` (per-key RGB focus)
- **Protocol research**: `docs/development/*.md`

---

## Current Development Focus

**Primary:** Per-key RGB control for Apex Pro TKL 2023
- Protocol reverse engineering in progress
- Command code `0x2A` is placeholder
- Zone-based fallback working as interim

**Secondary:** Actuation point control
- Write working (`0x2D` command)
- Read command not yet discovered
- Limited to Apex Pro series

---

**Version**: 0.1.0
**Last Updated**: 2026-02-10
**License**: MIT
