# AGENTS.md

This file helps Autohand understand how to work with this project.

## Project Overview

- **Language**: Rust
- **Framework**: Axum
- **Package Manager**: cargo

## Commands

- **Build**: `cargo build`
- **Build (release)**: `cargo build --release`
- **Run**: `cargo run`
- **Test**: `cargo test`
- **Check**: `cargo check`
- **Format**: `cargo fmt`
- **Lint**: `cargo clippy`

## Code Style

- Follow Rust naming conventions (snake_case for functions)
- Use descriptive error types
- Prefer `Result` over panicking
- Follow existing patterns in the codebase
- Use meaningful variable and function names
- Add comments for complex logic
- Keep functions focused and small

## Constraints

- Do not modify files outside the project directory
- Ask before making breaking changes
- Prefer editing existing files over creating new ones
- Do not delete files without confirmation
- Keep dependencies minimal - avoid adding new ones without good reason
- Do not commit sensitive data (API keys, secrets, credentials)
