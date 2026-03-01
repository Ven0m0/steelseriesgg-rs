# Dependency Audit Report

## Inventory & Vulnerability Check (CVEs)

An extensive audit was performed using `cargo audit`. **No vulnerabilities (CVEs) were found** in any of the dependencies in `Cargo.lock`.

## Deprecation, Bloat & Unused Packages

- All dependencies specified in `Cargo.toml` are imported and actively used within the codebase.
- The crates selected (e.g., `tokio`, `serde`, `axum`, `clap`, `chrono`) are standard, memory-safe, lightweight, and provide optimal performance for Rust projects. There are no redundant dependencies that warrant replacing with builtin alternatives (as Rust's standard library intentionally defers many utilities to external crates).
- `cargo tree -d` was reviewed: while the full dependency graph includes multiple versions of some transitive crates (e.g., `getrandom`, `thiserror`) due to upstream semver-major differences, there are no redundant duplicates among this project's direct dependencies.

## Outdated Packages & Updates

We performed minor and patch updates according to SemVer compatibility. All crates were already on modern major versions. Below is the summary of updates applied:

| Package | Previous Version | Latest Version | Status | Action Taken | Last Publish Date | Weekly Downloads | Update Commands Used |
|---------|-----------------|----------------|--------|--------------|-------------------|------------------|----------------------|
| `axum` | `0.8.0` | `0.8.8` | Outdated | Updated | 2025-12-20 | ~50M | `cargo add axum@0.8.8` |
| `chrono` | `0.4.3x` | `0.4.44` | Outdated | Updated | 2026-02-23 | ~76M | `cargo add chrono@0.4.44` |
| `clap` | `4.5.x` | `4.5.60` | Outdated | Updated | 2026-02-19 | ~106M | `cargo add clap@4.5.60` |
| `colored` | `3.1.x` | `3.1.1` | Outdated | Updated | 2026-01-16 | ~21M | `cargo add colored@3.1.1` |
| `directories` | `6.0.x` | `6.0.0` | Up-to-date | Kept | 2025-01-12 | ~5.7M | N/A |
| `hidapi` | `2.6.5` | `2.6.5` | Up-to-date | Kept | 2026-02-25 | ~600K | N/A |
| `libc` | `0.2.x` | `0.2.182` | Outdated | Updated | 2026-02-13 | ~148M | `cargo add libc@0.2.182` |
| `libpulse-binding`| `2.30.x` | `2.30.1` | Outdated | Updated | 2025-04-19 | ~565K | `cargo add libpulse-binding@2.30.1` |
| `parking_lot` | `0.12.x` | `0.12.5` | Outdated | Updated | 2025-10-03 | ~102M | `cargo add parking_lot@0.12.5` |
| `reqwest` | `0.13.x` | `0.13.2` | Outdated | Updated | 2026-02-06 | ~64M | `cargo add reqwest@0.13.2` |
| `serde` | `1.0.x` | `1.0.228` | Outdated | Updated | 2025-09-27 | ~120M | `cargo add serde@1.0.228` |
| `serde_json` | `1.0.x` | `1.0.149` | Outdated | Updated | 2026-01-06 | ~116M | `cargo add serde_json@1.0.149` |
| `sysinfo` | `0.38.x` | `0.38.2` | Outdated | Updated | 2026-02-15 | ~24M | `cargo add sysinfo@0.38.2` |
| `thiserror` | `2.0.x` | `2.0.18` | Outdated | Updated | 2026-01-18 | ~151M | `cargo add thiserror@2.0.18` |
| `indicatif` | `0.18.x` | `0.18.4` | Outdated | Updated | 2026-02-14 | ~25M | `cargo add indicatif@0.18.4` |
| `tabled` | `0.20.x` | `0.20.0` | Up-to-date | Kept | 2025-06-04 | ~4M | N/A |
| `tokio` | `1.49.x` | `1.49.0` | Outdated | Updated | 2026-01-03 | ~90M | `cargo add tokio@1.49.0` |
| `toml` | `1.0.x` | `1.0.3` | Outdated | Updated | 2026-02-18 | ~77M | `cargo add toml@1.0.3` |
| `tower-http` | `0.6.x` | `0.6.8` | Outdated | Updated | 2025-12-08 | ~47M | `cargo add tower-http@0.6.8` |
| `tracing` | `0.1.x` | `0.1.44` | Outdated | Updated | 2025-12-18 | ~83M | `cargo add tracing@0.1.44` |
| `tracing-subscriber`| `0.3.x` | `0.3.22` | Outdated | Updated | 2025-11-28 | ~58M | `cargo add tracing-subscriber@0.3.22` |
| `anyhow` | `1.0.x` | `1.0.102` | Outdated | Updated | 2026-02-20 | ~89M | `cargo add anyhow@1.0.102` |
| `async-trait` | `0.1.x` | `0.1.89` | Outdated | Updated | 2025-08-14 | ~61M | `cargo add async-trait@0.1.89` |
| `tempfile` | `3.10.x` | `3.17.0` | Outdated | Updated | 2026-02-24 | ~74M | `cargo add tempfile@3.17.0 --dev` |

## Lockfile Changes
- **Before Lockfile Size:** 74871 bytes
- **After Lockfile Size:** 74858 bytes

## Migration Notes
- All updates were within the same major versions (minor/patch bumps). No breaking changes were introduced.
- Tests passed perfectly without any code modifications required.
