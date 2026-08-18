---
name: dep-audit
description: Audit Cargo.toml/Cargo.lock for vulnerable, duplicated, or unjustified dependencies. Use when adding a dependency, before a release, or when asked to check for supply-chain issues or outdated crates.
allowed-tools: Read Grep Glob Bash
---

Run a dependency supply-chain check for steelseriesgg-rs. Report findings, do not fix anything unless asked.

## Steps

**1. Security advisories**
```
cargo audit
```
If `cargo-audit` isn't installed, say so and skip (don't install it silently) — `cargo install cargo-audit --locked` is the fix if the user wants it.

**2. Duplicate versions**
```
cargo tree -d
```
Any crate pulled in at two major versions is worth flagging, especially in the `audio`/`sonar` feature-gated deps — they should stay independent per CLAUDE.md.

**3. Outdated crates** (informational only)
```
cargo outdated
```
Skip and note as skipped if `cargo-outdated` isn't installed.

**4. hidapi pin check**
```
grep 'hidapi = ' Cargo.toml
```
Confirm it still reads `"=2.6.6"` exactly. Any diff here needs explicit justification per CLAUDE.md — flag, don't silently accept.

## Report format

- Advisories found (crate, RUSTSEC id, severity) or "none"
- Duplicate major versions found or "none"
- Outdated crates (informational) or "skipped: cargo-outdated not installed"
- hidapi pin status: unchanged / **changed — needs justification**
