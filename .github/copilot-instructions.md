# GitHub Copilot Instructions - steelseriesgg-rs

Comprehensive code generation guardrails for **steelseriesgg-rs** - an open-source SteelSeries GG replacement for Linux.

## Project Context

- **Language**: Rust 2021 (Rust 1.82+ toolchain; rustfmt 2024 style_edition)
- **Binary**: `ssgg` (CLI + daemon)
- **Purpose**: RGB lighting control, GameSense server, audio management for SteelSeries devices
- **Key Docs**: `CLAUDE.md` (comprehensive), `AGENTS.md` (quick ref), `README.md` (user docs)

## Core Principles

1. **User commands > Rules** - Direct instructions override defaults
2. **Edit > Create** - Modify existing code, avoid new files
3. **Subtraction > Addition** - Minimal changes, no over-engineering
4. **Align with patterns** - Follow existing codebase conventions
5. **Safety first** - No security vulnerabilities (command injection, XSS, SQL injection)

## Rust-Specific Guidelines

### Code Style

```rust
// Naming conventions
fn my_function() {}           // snake_case for functions/vars
struct MyStruct {}            // PascalCase for types
const MY_CONSTANT: u32 = 42;  // SCREAMING_SNAKE_CASE for constants

max_width = 100
hard_tabs = false
tab_spaces = 4
newline_style = "Unix"
```

**Before commit (REQUIRED):**
```bash
cargo fmt                     # Format
cargo clippy --all-features   # Lint
cargo test                    # Test
```

### Error Handling

```rust
// Prefer Result over panic
use anyhow::Result;           // For binaries
use thiserror::Error;         // For libraries

fn operation() -> Result<()> {
    device.send_report()?;    // Propagate errors
    Ok(())
}

// NEVER use .unwrap() or .expect() in production code
// Use ? operator or proper error handling
```

### Device Communication Patterns

**ALWAYS use HID report builders:**
```rust
// CORRECT
let report = HidReportBuilder::new(HidDeviceType::Keyboard)
    .command(CommandCode::RgbControl)
    .zone_data(zone, &color)
    .build()?;

// WRONG - manual buffer construction
let mut report = [0u8; 65];
report[0] = 0x21;  // Fragile, error-prone
```

**Critical constants:**
```rust
STEELSERIES_VENDOR_ID = 0x1038
KEYBOARD_REPORT_SIZE = 65     // With report ID
HEADSET_REPORT_SIZE = 64      // Without report ID
MAX_RGB_ZONES = 12
GAMESENSE_DEFAULT_PORT = 27301
```

### Async/Tokio Patterns

```rust
// Use tokio for async operations
use tokio::sync::{Mutex, RwLock};
use tokio::time::{sleep, Duration};

// Prefer Arc<Mutex<T>> for shared state in async
let shared = Arc::new(Mutex::new(state));

// Use tokio::spawn for background tasks
tokio::spawn(async move {
    // Background work
});
```

### Feature Flags

```rust
// Conditional compilation for features
#[cfg(feature = "audio")]
pub mod audio;

#[cfg(feature = "sonar")]
use crate::audio::sonar::SonarClient;

// Always test with all feature combinations
// cargo test --all-features
```

## Architecture Patterns

### Device Abstraction

```rust
// Implement Device trait for new hardware
pub trait Device: Send + Sync {
    fn info(&self) -> &DeviceInfo;
    fn initialize(&mut self) -> Result<()>;
    fn send_report(&mut self, data: &[u8]) -> Result<()>;
    // ... 20+ methods
}

// Use DeviceManager for discovery
let mut manager = DeviceManager::new()?;
manager.refresh()?;
let device = manager.open_device(vendor_id, product_id, interface)?;
```

### RGB Effect Engine

```rust
// Use EffectEngine with caching
let mut engine = EffectEngine::new(Effect::Breathing {
    color: Color::new(0, 255, 255),
    speed: 2.0,
});

// Caches results for 16ms (~60 FPS)
let colors = engine.compute(num_zones, elapsed);
```

### Performance Monitoring

```rust
// Track performance in critical paths
let perf = PerformanceManager::new();
perf.start_operation("rgb_update");
// ... work
perf.end_operation("rgb_update");
```

## Critical Gotchas

1. **HID Report Sizing**: Keyboards = 65 bytes (with report ID), Headsets = 64 bytes
2. **Interface Numbers**: Keyboards use interface 1, headsets use interface 3
3. **Product IDs**: Apex Pro TKL 2023 = `0x1628` (NOT `0x1618`)
4. **Animated Effects**: Require daemon mode for continuous updates
5. **Per-Key RGB**: Protocol not reverse-engineered yet (use zone fallback)
6. **RGB Caching**: First compute always executes (check `last_compute_time != Duration::ZERO`)

## Security Requirements

**NEVER introduce these vulnerabilities:**
- Command injection (validate all shell inputs)
- Path traversal (sanitize file paths)
- Buffer overflows (use safe Rust patterns)
- Unvalidated user input to system calls
- Hardcoded credentials or API keys

**For HID communication:**
- Always validate buffer sizes before sending
- Use type-safe builders (HidReportBuilder)
- Bounds check array access
- Sanitize device paths

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

**Test coverage expectations:**
- Unit tests for all public APIs
- Edge cases (empty, max, invalid input)
- Error paths (Result::Err cases)
- Feature flag combinations

**Current coverage:** ~77 tests across RGB, HID, zones, keyboards, profiles, performance

## Dependencies

**DO NOT add new dependencies without justification**

**Core deps:**
- hidapi 2.6.4 - HID communication (pinned)
- tokio 1.49 - Async runtime
- axum 0.8 - HTTP server
- clap 4.5 - CLI parsing
- serde/serde_json - Serialization
- thiserror 2.0 - Error types

**Optional:**
- libpulse-binding 2.30 (audio feature)
- reqwest 0.13 (sonar feature)

## Code Generation Guidelines

### Prefer Built-ins

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

### No Over-Engineering

**DON'T add unless requested:**
- Extra abstraction layers
- Premature optimization
- Speculative features
- Excessive documentation (docstrings on every private fn)
- Error handling for impossible cases

**Example - TOO MUCH:**
```rust
/// Sets the RGB color (UNNEEDED docstring for internal function)
fn set_color_internal(&mut self, color: Color) -> Result<()> {
    // Validate color (UNNEEDED - Color type guarantees validity)
    if color.r > 255 || color.g > 255 || color.b > 255 {
        return Err(anyhow!("Invalid color"));
    }
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

## Module Organization

```
src/
├── main.rs              # CLI entry (15+ commands)
├── lib.rs               # Library entry
├── error.rs             # Error types
├── devices/             # Hardware layer
│   ├── mod.rs           # Device trait, discovery
│   ├── hid_reports.rs   # HID protocol
│   ├── keyboards/       # Keyboard implementations
│   └── headsets/        # Headset implementations
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

## CLI Command Patterns

```rust
// Use clap derive API
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

// Implement handler
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

## Performance Considerations

**Release profile:**
```toml
[profile.release]
strip = true
lto = "fat"
codegen-units = 1
opt-level = 3
```

**Runtime optimization:**
- Reuse buffers (Vec, String) where possible
- Cache expensive computations (RGB effects)
- Batch HID writes when possible
- Use const fn for compile-time computation
- Minimize allocations in hot paths

**Example:**
```rust
// GOOD - reuse buffer
let mut buffer = Vec::with_capacity(MAX_ZONES);
loop {
    buffer.clear();
    compute_colors(&mut buffer);
    device.send(&buffer)?;
}

// BAD - allocate every iteration
loop {
    let buffer = compute_colors();  // New allocation
    device.send(&buffer)?;
}
```

## Git Workflow

**Commit messages:**
```
feat: Add per-key RGB fallback using zones
fix: RGB caching returns black on first compute
refactor: Extract HID report builder pattern
docs: Update AGENTS.md with common gotchas
test: Add unit tests for Color blending
```

**Before commit:**
```bash
cargo fmt
cargo clippy --all-features
cargo test --all-features
```

## Documentation

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

# Lint
cargo clippy --all-features -- -D warnings
```

## Additional Resources

- **Comprehensive guide**: `CLAUDE.md` (28KB developer reference)
- **Quick reference**: `AGENTS.md` (AI agent guide)
- **User docs**: `README.md`
- **Contributing**: `CONTRIBUTING.md`
- **Project structure**: `PROJECT_INDEX.md`
- **Roadmap**: `PLAN.md`

---

**Last Updated**: 2026-02-10
**Version**: 0.1.0
