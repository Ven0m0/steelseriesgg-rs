# Dependency Audit Report

## 1. Inventory

| Package | Current Version | Latest Version | Last Publish Date | Weekly Downloads | Status | Action Taken |
|---------|-----------------|----------------|-------------------|------------------|--------|--------------|
| `anyhow` | 1.0.102 | 1.0.102 | 2026-02-20 | 94,073,941 | Up to date | Updated via `cargo update` |
| `async-trait` | 0.1.89 | 0.1.89 | 2025-08-14 | 65,154,152 | Up to date | Updated via `cargo update` |
| `axum` | 0.8.8 | 0.8.8 | 2025-12-20 | 53,020,289 | Up to date | Updated via `cargo update` |
| `chrono` | 0.4.44 | 0.4.44 | 2026-02-23 | 80,167,621 | Up to date | Updated via `cargo update` |
| `clap` | 4.5.60 | 4.5.60 | 2026-02-19 | 111,704,350 | Up to date | Updated via `cargo update` |
| `colored` | 3.1.1 | 3.1.1 | 2026-01-16 | 23,287,583 | Up to date | Updated via `cargo update` |
| `directories` | 6.0.0 | 6.0.0 | 2025-01-12 | 6,117,106 | Up to date | Updated via `cargo update` |
| `hidapi` | 2.6.5 | 2.6.5 | 2026-02-25 | 651,869 | Up to date | Updated via `cargo update` |
| `indicatif` | 0.18.4 | 0.18.4 | 2026-02-14 | 26,502,685 | Up to date | Updated via `cargo update` |
| `libc` | 0.2.182 | 0.2.182 | 2026-02-13 | 154,908,012 | Up to date | Updated via `cargo update` |
| `libpulse-binding` | 2.30.1 | 2.30.1 | 2025-04-19 | 583,292 | Up to date | Updated via `cargo update` |
| `parking_lot` | 0.12.5 | 0.12.5 | 2025-10-03 | 107,330,226 | Up to date | Updated via `cargo update` |
| `reqwest` | 0.13.2 | 0.13.2 | 2026-02-06 | 69,014,348 | Up to date | Updated via `cargo update` |
| `serde` | 1.0.228 | 1.0.228 | 2025-09-27 | 126,096,496 | Up to date | Updated via `cargo update` |
| `serde_json` | 1.0.149 | 1.0.149 | 2026-01-06 | 122,292,525 | Up to date | Updated via `cargo update` |
| `sysinfo` | 0.38.3 | 0.38.3 | 2026-03-02 | 26,569,796 | Up to date | Updated via `cargo update` |
| `tabled` | 0.20.0 | 0.20.0 | 2025-06-04 | 4,328,214 | Up to date | Updated via `cargo update` |
| `thiserror` | 2.0.18 | 2.0.18 | 2026-01-18 | 158,641,488 | Up to date | Updated via `cargo update` |
| `tokio` | 1.50.0 | 1.50.0 | 2026-03-03 | 95,520,482 | Up to date | Updated via `cargo update` |
| `toml` | 1.0.6+spec-1.1.0 | 1.0.6+spec-1.1.0 | 2026-03-06 | 82,349,611 | Up to date | Updated via `cargo update` |
| `tower-http` | 0.6.8 | 0.6.8 | 2025-12-08 | 50,486,909 | Up to date | Updated via `cargo update` |
| `tracing` | 0.1.44 | 0.1.44 | 2025-12-18 | 88,285,619 | Up to date | Updated via `cargo update` |
| `tracing-subscriber` | 0.3.22 | 0.3.22 | 2025-11-28 | 61,134,693 | Up to date | Updated via `cargo update` |
| `tempfile` | 3.26.0 | 3.26.0 | 2026-02-24 | 79,134,515 | Up to date | Updated via `cargo update` |


## 2. CVEs
* **0 Vulnerabilities found.** `cargo audit` reported 0 advisories in our currently checked-in `Cargo.lock`.

## 3. Unused / Transitive Dependencies
* Evaluated via `cargo clippy --features sonar,audio`. No unused shared or direct dependencies were reported.
* `async-trait` usage was checked and is actively used across `src/devices/keyboards`.

## 4. Bloat Review
* **`anyhow`**: Currently used across the diagnostic tools and binary CLIs for generic error bubbling, which is appropriate for top-level binaries. Replacing this with custom types/`Box<dyn std::error::Error>` would add boilerplate without substantial runtime benefit.
* **`chrono`**: Used for diagnostic timestamp formatting (`to_rfc3339()` and string formats) as well as within serialized struct shapes. To fully transition to something like `jiff` or std `SystemTime` would require moderate re-working of serde serialization types, though is possible in the future if a smaller binary footprint is rigidly required.

## 5. Outdated Dependencies
* All outdated transient crates have been patched.
* Applied updates automatically using `cargo update`. No major semantic version bumps are required.

## 6. Duplicates
* Checked `cargo tree -d`. None found.

## 7. Update Summary
* Ran `cargo update`.
* Initial lockfile size: **74K**
* Post-update lockfile size: **74K** (mostly transient deps updated)
* No migration steps needed for major changes since all bumps applied are within compatible semver limits.
