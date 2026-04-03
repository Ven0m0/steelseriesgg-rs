<!-- Generated: 2026-03-26 | Updated: 2026-03-27 -->

# steelseriesgg-rs

## Purpose

steelseriesgg-rs is an open-source Linux replacement for SteelSeries GG, providing:
- **RGB lighting control** for SteelSeries keyboards
- **GameSense HTTP server** on port 27301 compatible with SteelSeries GameSense API
- **Audio management** for SteelSeries audio devices (optional, feature-gated)
- Hardware abstraction for multiple SteelSeries device types

**Language**: Rust 2021 (Edition 2021)
**Toolchain**: Pinned at 1.94.1 in `rust-toolchain.toml`
**License**: MIT
**Binary**: `ssgg` (src/main.rs, ~3500+ LOC, 15+ subcommands via clap derive)
**Library**: `steelseries_gg` (src/lib.rs)

---

## Key Files

| File | Description |
|------|-------------|
| `Cargo.toml` | Package manifest, features, dependency versions (source of truth) |
| `Cargo.lock` | Dependency lockfile for reproducible builds |
| `rust-toolchain.toml` | Pinned Rust version (1.94.1) and components |
| `rustfmt.toml` | Formatter config (edition 2024, style_edition 2024, 120-char max line) |
| `CLAUDE.md` | Claude Code agent instructions (architecture, conventions, commands) |
| `.github/workflows/` | CI/CD pipelines (ci.yml, build.yml, release-arch.yml) |
| `assets/99-steelseries.rules` | udev rules for non-root HID access |
| `assets/ssgg.service` | systemd service unit |
| `PKGBUILD` | Arch Linux package build script |
| `renovate.json` | Automated dependency update configuration |

---

## Subdirectories

| Directory | Purpose |
|-----------|---------|
| `src/` | Main source code (library + binary implementation) |
| `src/devices/` | Device abstraction layer (keyboards, headsets, HID communication) |
| `src/devices/keyboards/` | Keyboard device implementations (Apex series) |
| `src/devices/headsets/` | Headset device implementations (Arctis series) |
| `src/rgb/` | RGB control engine (Color, Effect, EffectEngine, RgbController) |
| `src/gamesense/` | GameSense HTTP server implementation (Axum, CORS security) |
| `src/profiles/` | Profile management system (load/save device configurations) |
| `src/config/` | Configuration file handling (~/.config/ssgg/config.toml) |
| `src/audio/` | Audio mixer integration (PulseAudio, Sonar API) — feature-gated |
| `src/bin/` | Standalone binary utilities (discover_actuation, sonar_control, etc.) |
| `tests/` | Integration tests (CORS security, device communication) |
| `docs/development/` | Protocol research and reverse-engineering notes |
| `assets/` | System integration files (udev rules, systemd service) |
| `mock_udev/` | Mock udev library for CI environments |
| `.cargo/` | Cargo build configuration |

---

## For AI Agents

### Working In This Directory

When working on steelseriesgg-rs:

1. **Always prefer CLAUDE.md and live code** over this file when architectural details conflict
   - Cargo.toml is source of truth for dependency versions and feature flags
   - src/devices/hid_reports.rs is source of truth for HID report format
   - .github/workflows/ define CI requirements

2. **Never use `.unwrap()` or `.expect()`**
   - Library code (src/lib.rs and submodules): use `thiserror` variants from `crate::error::Error` with `?` operator
   - Binary code (src/main.rs, src/bin/): use `anyhow` + `.context("description")` with `?` operator

3. **Never construct raw byte arrays** for HID communication
   - Always use `HidReportBuilder` from `src/devices/hid_reports.rs`
   - Always use `CommandCode` enum + typed command structs
   - Keyboard reports: 65 bytes (report ID in position 0) → `KEYBOARD_REPORT_SIZE`
   - Headset reports: 64 bytes (no report ID) → `HEADSET_REPORT_SIZE`

4. **Use HidOptimizer correctly**
   - Global singleton in `src/devices/mod.rs` via `OnceLock`
   - FNV-1a hash-based deduplication (50ms cache window)
   - 5s connectivity cache TTL
   - Silently skips identical reports — document this behavior in code

5. **Logging in library code**
   - Use `tracing::{debug, info, warn}` — never `println!`
   - Initialize with `FmtSubscriber` in `main.rs` only
   - `RUST_LOG=debug` or `--debug` flag enables verbose output

6. **Naming conventions**
   - `snake_case`: functions, variables, modules
   - `PascalCase`: types, traits, enums
   - `SCREAMING_SNAKE_CASE`: constants

7. **Formatting**
   - 4-space indent, 120-char max line, Unix LF
   - Run `cargo fmt` before all commits — CI enforces this
   - Use `cargo clippy --all-targets --locked -- -D warnings` (3 feature combos in CI)

### Testing Requirements

Before pushing changes:

```bash
# Format (CI-enforced)
cargo fmt

# Lint (CI-enforced, 3 feature combos)
cargo clippy --all-targets --locked -- -D warnings
cargo clippy --all-targets --locked --features sonar -- -D warnings
cargo clippy --all-targets --locked --features audio -- -D warnings

# Test (CI runs 2 combos)
cargo test                          # default feature set
cargo test --features sonar         # CI-covered optional features
cargo test --all-features           # broader local verification
cargo test <test_name>              # single test (substring match)

# Build (CI runs 3 combos)
cargo build
cargo build --features sonar
cargo build --features audio
```

All tests must pass and all clippy warnings must be resolved before commit.

### Common Patterns

**Device Discovery**
```
DeviceManager (src/devices/discovery.rs)
  → enumerate via hidapi
  → create DeviceInfo + DeviceFingerprint
  → match by product ID → instantiate Keyboard or Headset
  → hot-plug support via HotPlugEvent channel
```

**HID Communication Pipeline**
```
CLI command
  → HidCommand trait impl (RgbZoneCommand, ActuationCommand, etc.)
  → HidReportBuilder::build_report() [validation + serialization]
  → write_padded_report() [HidOptimizer dedup + actual write]
  → hidapi::HidDevice::write()
```

**RGB Control**
- `Color`: RGB or named color
- `Effect`: breathing, spectrum, wave, static, off
- `EffectEngine`: per-device effect computation with caching
- `RgbController`: zone-based RGB control
- `PerKeyRgbController`: per-key RGB (Apex Pro TKL 2023 with `experimental-apex-2023` feature)

**Error Handling Pattern (Library)**
```rust
use crate::error::{Error, Result};

fn some_operation() -> Result<T> {
    let device = DeviceManager::find(pid)?;
    device.send_command(cmd)?;
    Ok(value)
}
```

**Error Handling Pattern (Binary)**
```rust
use anyhow::{Context, Result};

fn main() -> Result<()> {
    let device = DeviceManager::find(pid)
        .context("failed to find device")?;
    Ok(())
}
```

**Profile Management**
- `Profile`: top-level JSON config with optional KeyboardProfile + HeadsetProfile
- Stored in `~/.config/ssgg/profiles/`
- Managed via `ProfileManager`
- `Config` at `~/.config/ssgg/config.toml` controls defaults

**GameSense Server**
- Axum HTTP server on `127.0.0.1:27301`
- CORS restricted to same-origin (`127.0.0.1` only) — do NOT loosen
- State: `Arc<RwLock<ServerState>>`
- Game registration, event binding, RGB callbacks

### Supported Devices

**Keyboards** (PIDs in src/devices/mod.rs)
| Device | PID | Notes |
|--------|-----|-------|
| Apex Pro | 0x1610 | |
| Apex Pro TKL | 0x1614 | |
| Apex Pro TKL (2023) | 0x1628 | Primary tested device, actuation control |
| Apex 3 | 0x161A | |
| Apex 3 TKL | 0x1622 | |
| Apex 5 | 0x161C | |
| Apex 7 | 0x1612 | |
| Apex 7 TKL | 0x1616 | |

**Headsets** (Arctis Series)
Arctis 1, Arctis 1 Wireless, Arctis 5, Arctis 7 (2019), Arctis 9, Arctis Pro, Arctis Pro Wireless, Arctis Nova Pro, Arctis Nova Pro Wireless, Arctis Nova 5, Arctis Nova 3, Arctis Nova 1.

### Critical Constraints

- **hidapi pinned at `=2.6.5`** in Cargo.toml — do NOT change this constraint
- **Apex Pro TKL 2023 PID is `0x1628`** (not 0x1618)
- **Actuation point write** via command `0x2D` (`ActuationControl`); read-back not yet implemented
- **Per-key RGB protocol** still evolving — verify implementation in src/devices/hid_reports.rs before changes
- **CLI RGB commands** are one-shot; continuous animations require `ssgg daemon`
- **Report deduplication** silently skips identical reports within 50ms (HidOptimizer)
- **Hot-plug support** via DeviceManager hot-plug event channel
- **Use rustix for safe syscalls** instead of raw libc (e.g., `rustix::process::getuid()` for permission checks)
- **Hidden fuzzer** at `fuzz` subcommand (raw HID bytes, protocol research only)

---

## Dependencies

### External Crates (from Cargo.toml)

| Crate | Version | Purpose | Features |
|-------|---------|---------|----------|
| axum | 0.8.8 | HTTP server (GameSense API) | tokio, http1, http2, json |
| chrono | 0.4.44 | Timestamps in diagnostics | serde |
| clap | 4.6.0 | CLI argument parsing | derive |
| colored | 3.1.1 | Terminal output coloring | — |
| directories | 6.0.0 | User config directories | — |
| hidapi | =2.6.5 | USB HID communication | linux-native-basic-udev |
| libc | 0.2.183 | Legacy C interop (deprecated — use rustix for permission checks) | — |
| libpulse-binding | 2.30.1 | PulseAudio integration | optional (`audio` feature) |
| parking_lot | 0.12.5 | Faster parking lot locks | — |
| reqwest | 0.13.2 | HTTP client (Sonar API) | json, optional (`sonar` feature) |
| serde | 1.0.228 | Serialization framework | derive |
| serde_json | 1.0.149 | JSON serialization | — |
| sysinfo | 0.38.4 | System info for diagnostics | — |
| thiserror | 2.0.18 | Typed error handling | — |
| indicatif | 0.18.4 | Progress bars/spinners | — |
| tabled | 0.20.0 | Table formatting | — |
| tokio | 1.50.0 | Async runtime | rt-multi-thread, macros, signal, fs |
| toml | 1.1.0 | TOML config parsing | — |
| tower-http | 0.6.8 | HTTP middleware | cors |
| tracing | 0.1.44 | Structured logging | — |
| tracing-subscriber | 0.3.23 | Tracing implementation | env-filter |
| anyhow | 1.0.102 | Error context (binary use) | — |
| async-trait | 0.1.89 | Async trait support | — |
| rustix | 1.1.4 | Safe syscalls | process |

### System Dependencies

**Default build (no optional features)**
No system packages required.

**Full build (with all optional features)**
```bash
sudo apt-get install -y libpulse-dev    # Required for 'audio' feature
```

---

## Feature Flags

| Feature | Enables | Dependencies | Default |
|---------|---------|--------------|---------|
| *(none)* | Core RGB, GameSense server, profiles, device discovery | — | Yes |
| `audio` | AudioMixer, Channel, PulseAudio integration | libpulse-binding | No |
| `sonar` | SonarClient — SteelSeries Sonar HTTP API | reqwest | No |
| `experimental-apex-2023` | Direct RGB command path for Apex Pro TKL 2023 (PID 0x1628) | — | No |

**Feature combinations tested in CI:**
- Default (no features)
- `--features sonar`
- `--features audio`

---

## CLI Subcommands

Main binary `ssgg` (via clap derive):

| Subcommand | Description | Feature Gate |
|-----------|-------------|--------------|
| `devices` | List connected SteelSeries devices | — |
| `rgb color <hex\|name>` | Set static color | — |
| `rgb brightness <0-100>` | Set brightness | — |
| `rgb effect <name>` | Set effect (static, breathing, spectrum, wave, off) | — |
| `rgb perkey` | Per-key RGB control | experimental-apex-2023 |
| `actuation set <value>` | Set actuation point (Apex Pro TKL 2023) | — |
| `profile` | Save/load/list profiles | — |
| `audio` | Audio mixer | audio |
| `sonar` | Sonar API control | sonar |
| `pollrate` | USB polling rate (requires root) | — |
| `server` | Start GameSense HTTP server | — |
| `daemon` | Run as full daemon (device + server) | — |
| `validate` | Run hardware validation tests | — |
| `performance` | Monitor/control RGB performance | — |
| `bug-report` | Generate JSON diagnostic report | — |
| `status` | Real-time device connection status | — |
| `hid-logs` | View HID communication logs | — |
| `test-device` | Run automated device tests | — |
| `verify-performance` | Monitor RGB performance metrics | — |
| `fuzz` | *(hidden)* Protocol fuzzer for reverse engineering | — |

---

## Commit Message Format

```
<type>: <description>
```

**Types**: `feat` `fix` `refactor` `docs` `test` `perf` `chore` `style`

**Examples:**
```
feat: add per-key RGB support for Apex 3 TKL
fix: correct actuation point byte order in HID report
perf: reduce HID write latency with report deduplication
docs: update APEX_PRO_PROTOCOL with 0x2D findings
chore: bump hidapi to =2.6.5
```

---

## CI/CD Pipelines

| Workflow | Trigger | Jobs | Requirements |
|---------|---------|------|--------------|
| `ci.yml` | push/PR to main | fmt check, clippy (3 combos), test (2 combos), build (3 combos) | All pass required |
| `build.yml` | workflow_dispatch, merge_group | Release build | — |
| `release-arch.yml` | tag push (v*), workflow_dispatch, merge_group | Arch Linux PKGBUILD release | — |
| `cargo-assist.yml` | push | Dependency assistance | — |
| `dependabot.yml` / `renovate.json` | scheduled | Automated dep bumps | — |

**CI format check:**
```bash
cargo fmt --all -- --check
```

**CI clippy (3 feature combos, all warnings errors):**
```bash
cargo clippy --all-targets --locked -- -D warnings
cargo clippy --all-targets --locked --features sonar -- -D warnings
cargo clippy --all-targets --locked --features audio -- -D warnings
```

**CI test (2 feature combos):**
```bash
cargo test
cargo test --features sonar
```

---

## Key Architecture Decisions

**Device Abstraction**
- `Device` trait: base interface (initialize, send_raw, receive_raw, close)
- `Keyboard` trait: extends Device with RGB and actuation control
- `Headset` trait: extends Device with audio control
- `DeviceManager`: HID enumeration via hidapi with hot-plug support

**HID Optimization**
- `HidOptimizer` singleton (OnceLock) with FNV-1a hashing
- 50ms deduplication cache window prevents redundant writes
- 5s connectivity cache TTL reduces open/close overhead
- Size: Keyboard 65 bytes (report ID), Headset 64 bytes (no report ID)

**Performance System**
- `PerformanceMonitor`: ring buffer of last 60 frames
- `PerformanceManager`: adaptive refresh rates, effect caching
- `RgbTimingMetrics`: serializable snapshot (FPS, dropped frames, cache hit rate)

**Profile Storage**
- User profiles: `~/.config/ssgg/profiles/`
- Main config: `~/.config/ssgg/config.toml`
- Format: JSON for profiles, TOML for main config

**Security (GameSense Server)**
- CORS restricted to `127.0.0.1` only — do NOT loosen
- Arc<RwLock<ServerState>> for thread-safe access
- Axum middleware security enforcement

---

<!-- MANUAL: Any manually added notes below this line are preserved on regeneration -->

# context-mode — MANDATORY routing rules

You have context-mode MCP tools available. These rules are NOT optional — they protect your context window from flooding. A single unrouted command can dump 56 KB into context and waste the entire session.

## BLOCKED commands — do NOT attempt these

### curl / wget — BLOCKED
Any shell command containing `curl` or `wget` will be intercepted and blocked by the context-mode plugin. Do NOT retry.
Instead use:
- `context-mode_ctx_fetch_and_index(url, source)` to fetch and index web pages
- `context-mode_ctx_execute(language: "javascript", code: "const r = await fetch(...)")` to run HTTP calls in sandbox

### Inline HTTP — BLOCKED
Any shell command containing `fetch('http`, `requests.get(`, `requests.post(`, `http.get(`, or `http.request(` will be intercepted and blocked. Do NOT retry with shell.
Instead use:
- `context-mode_ctx_execute(language, code)` to run HTTP calls in sandbox — only stdout enters context

### Direct web fetching — BLOCKED
Do NOT use any direct URL fetching tool. Use the sandbox equivalent.
Instead use:
- `context-mode_ctx_fetch_and_index(url, source)` then `context-mode_ctx_search(queries)` to query the indexed content

## REDIRECTED tools — use sandbox equivalents

### Shell (>20 lines output)
Shell is ONLY for: `git`, `mkdir`, `rm`, `mv`, `cd`, `ls`, `npm install`, `pip install`, and other short-output commands.
For everything else, use:
- `context-mode_ctx_batch_execute(commands, queries)` — run multiple commands + search in ONE call
- `context-mode_ctx_execute(language: "shell", code: "...")` — run in sandbox, only stdout enters context

### File reading (for analysis)
If you are reading a file to **edit** it → reading is correct (edit needs content in context).
If you are reading to **analyze, explore, or summarize** → use `context-mode_ctx_execute_file(path, language, code)` instead. Only your printed summary enters context.

### grep / search (large results)
Search results can flood context. Use `context-mode_ctx_execute(language: "shell", code: "grep ...")` to run searches in sandbox. Only your printed summary enters context.

## Tool selection hierarchy

1. **GATHER**: `context-mode_ctx_batch_execute(commands, queries)` — Primary tool. Runs all commands, auto-indexes output, returns search results. ONE call replaces 30+ individual calls.
2. **FOLLOW-UP**: `context-mode_ctx_search(queries: ["q1", "q2", ...])` — Query indexed content. Pass ALL questions as array in ONE call.
3. **PROCESSING**: `context-mode_ctx_execute(language, code)` | `context-mode_ctx_execute_file(path, language, code)` — Sandbox execution. Only stdout enters context.
4. **WEB**: `context-mode_ctx_fetch_and_index(url, source)` then `context-mode_ctx_search(queries)` — Fetch, chunk, index, query. Raw HTML never enters context.
5. **INDEX**: `context-mode_ctx_index(content, source)` — Store content in FTS5 knowledge base for later search.

## Output constraints

- Keep responses under 500 words.
- Write artifacts (code, configs, PRDs) to FILES — never return them as inline text. Return only: file path + 1-line description.
- When indexing content, use descriptive source labels so others can `search(source: "label")` later.

## ctx commands

| Command | Action |
|---------|--------|
| `ctx stats` | Call the `stats` MCP tool and display the full output verbatim |
| `ctx doctor` | Call the `doctor` MCP tool, run the returned shell command, display as checklist |
| `ctx upgrade` | Call the `upgrade` MCP tool, run the returned shell command, display as checklist |
