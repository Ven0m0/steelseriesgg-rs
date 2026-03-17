# GitHub Copilot Instructions — steelseriesgg-rs

Use [`AGENTS.md`](../AGENTS.md) as the canonical project handbook. Keep this file short, stable, and operational.

## Do

- Make the smallest change that fully solves the request.
- Prefer editing existing files over creating new ones.
- Match local Rust patterns: `snake_case` functions/modules, `PascalCase` types, `SCREAMING_SNAKE_CASE` constants.
- Use `crate::error::Error`/`thiserror` in library code and `anyhow` with `.context(...)` in binaries.
- Use `HidReportBuilder` and existing HID abstractions instead of hand-built report buffers.
- Preserve the pinned `hidapi = "=2.6.5"` dependency constraint unless the task explicitly requires changing it.
- Treat `Cargo.toml`, `rust-toolchain.toml`, and `.github/workflows/*.yml` as the source of truth for versions, features, and CI behavior.
- Keep CORS checks exact for localhost origins; do not loosen GameSense server origin validation.
- Verify evolving protocol details against current source before documenting or changing them.

## Avoid

- Duplicating large blocks of repo context from `AGENTS.md`.
- Introducing `.unwrap()` or `.expect()` in production paths.
- Stating feature relationships, workflow triggers, or toolchain versions from memory.
- Replacing existing typed helpers with manual byte buffers or ad hoc protocol code.

## Validation

- Run the smallest relevant existing checks for the files you changed.
- For Rust changes, mirror the CI commands in `.github/workflows/ci.yml`.
- For local builds, the default feature set does not require extra HID development packages; `libpulse-dev` is only needed for `--features audio`.
