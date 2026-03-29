# steelseriesgg-rs — Kilocode Rules

Use [`AGENTS.md`](../../AGENTS.md) as the canonical project handbook. Keep this file short, stable, and operational; verify details against live code and the source-of-truth files before changing behavior or docs.

## Do

- Make the smallest change that fully solves the request.
- Prefer editing existing files over creating new ones.
- For Rust production paths, avoid `.unwrap()` / `.expect()`; use `crate::error::Error` / `thiserror` in library code and `anyhow` with `.context(...)` in binaries.
- Use `HidReportBuilder` and existing typed HID abstractions instead of raw byte buffers.
- Treat `Cargo.toml`, `rust-toolchain.toml`, `src/devices/hid_reports.rs`, and `.github/workflows/*.yml` as the source of truth for versions, features, HID formats, and CI behavior.
- Keep GameSense CORS checks exact for localhost origins; do not loosen server origin validation.
- Use `tracing::{debug, info, warn}` in library code instead of `println!`.
- Run the smallest relevant existing validation for the files you changed; for Rust work, mirror the CI commands in `.github/workflows/ci.yml`.

## Avoid

- Duplicating large blocks of repository context from `AGENTS.md`.
- Changing the pinned `hidapi` constraint unless the task explicitly requires it.
- Replacing typed HID helpers with ad hoc protocol code.
- Stating feature relationships, workflow triggers, or toolchain details from memory.

## Quick Reference

- Keyboard HID reports are 65 bytes with the report ID in byte 0; headset reports are 64 bytes without a report ID.
- `HidOptimizer` deduplicates identical reports for 50ms and caches connectivity for 5s.
- `libpulse-dev` is only needed for local `--features audio` builds.
