---
goal: Turn the current backlog findings into a prioritized implementation plan.
date_created: 2026-03-17
last_updated: 2026-03-22
status: Planned
source: TODO.md
---

# PLAN: implementation priorities from the current findings

This plan converts the current TODO backlog and recent repository findings into a focused implementation order.

The goal is to address the highest-value, lowest-ambiguity improvements first, while preserving the repo's current architectural and security constraints.

## 1. Constraints to preserve while implementing the backlog

- Read `AGENTS.md` before making code changes.
- Treat these files as the source of truth when prose docs drift:
  - `Cargo.toml`
  - `rust-toolchain.toml`
  - `.github/workflows/ci.yml`
  - `src/devices/hid_reports.rs`
- Preserve the exact `hidapi = "=2.6.5"` dependency pin unless a task explicitly proves that a change is required.
- Do not loosen the localhost-only GameSense CORS policy.
- Keep `audio` and `sonar` feature gating independent.
- Do not replace typed HID helpers such as `HidReportBuilder` with manual byte arrays.
- Do not present placeholder Apex Pro TKL 2023 protocol support as verified hardware behavior.

## 2. Verified baseline before follow-up implementation

The repository already passed the default local validation path during the latest review cycle:

```bash
cd /home/runner/work/steelseriesgg-rs/steelseriesgg-rs
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

This means the next implementation step should stay narrow and should validate only the affected area unless a broader regression is suspected.

## 3. Findings that should drive the next implementation work

### Finding A: the audio path needs fail-fast behavior

- The highest-impact reliability issue is the reported hang in issue `#120`.
- The likely implementation surface is:
  - `src/audio/pulse.rs`
  - `src/audio/mod.rs`
  - `src/main.rs`
- The current plan should assume the fix will involve bounded waits, explicit timeout/error propagation, and preserving CLI responsiveness when PulseAudio is unavailable or partially configured.

### Finding B: protocol docs drift from the current tooling

- The development docs reference `src/bin/bulk_test.rs`, but that binary is not present in the repository.
- The current helper binaries that do exist are:
  - `src/bin/discover_actuation.rs`
  - `src/bin/verify_key_mapping.rs`
  - `src/bin/benchmark_rgb_alloc.rs`
  - `src/bin/sonar_control.rs`
- Before further protocol work, the docs should point contributors to real tooling and clearly label historical vs current workflows.

### Finding C: Apex Pro per-key support needs more accurate capability reporting

- The current code and docs indicate that parts of the Apex Pro TKL 2023 per-key RGB path are still placeholder or unverified.
- The implementation plan should treat capability accuracy as a reliability and maintainability task, not as a new protocol expansion.
- The likely implementation surface is:
  - `src/devices/hid_reports.rs`
  - `src/devices/key_mapping.rs`
  - `src/devices/keyboards/mod.rs`
  - `src/main.rs`

### Finding D: Arch/audio validation needs a concrete reproduction path

- Issue `#6` remains important, but the right fix depends on whether the failure is still reproducible on current code.
- The likely validation and packaging surface is:
  - `PKGBUILD`
  - `ssgg.install`
  - `.github/workflows/ci.yml`
  - `.github/workflows/release-arch.yml`
- This work should stay evidence-driven and should not change code if the issue turns out to be packaging-only or already stale.

### Finding E: `src/main.rs` is a refactor candidate, not the first fix

- `src/main.rs` remains oversized and should eventually be split into smaller command-focused modules.
- That refactor is lower priority than the current reliability and correctness fixes.
- It should only start after the higher-priority backlog items above are under control.

## 4. Recommended implementation order

Work in this order unless new evidence requires a change:

1. fix the audio hang path behind issue `#120`
2. reconcile stale protocol docs and missing tooling references
3. tighten Apex Pro capability reporting so placeholders are not presented as verified support
4. reproduce and resolve issue `#6` using the smallest code, packaging, or CI change that matches the evidence
5. refactor `src/main.rs` only after the higher-priority fixes above are stable

## 5. Phase 1: fix issue `#120` first

### Objective

Make `ssgg audio ...` fail fast with a clear error instead of hanging.

### Primary files

- `src/audio/pulse.rs`
- `src/audio/mod.rs`
- `src/main.rs`

### Implementation tasks

- Reproduce the reported hang with the `audio` feature enabled.
- Identify every unbounded wait in the PulseAudio setup and sink enumeration path.
- Add bounded waiting or timeout-based exits where the current code can block indefinitely.
- Propagate explicit errors back to the CLI so the user gets a failure instead of an apparent freeze.
- Keep the change local to the audio path unless the investigation proves a different root cause.

### Success criteria

- `ssgg audio status` returns promptly.
- `ssgg audio chat-mix 50` returns promptly.
- Missing or unhealthy PulseAudio environments produce explicit errors.
- The fix does not change unrelated feature behavior.

### Validation

```bash
cd /home/runner/work/steelseriesgg-rs/steelseriesgg-rs
cargo build --locked --features audio
cargo clippy --all-targets --locked --features audio -- -D warnings
cargo test --locked --features audio
RUST_LOG=debug cargo run --features audio -- audio status
RUST_LOG=debug cargo run --features audio -- audio chat-mix 50
```

## 6. Phase 2: reconcile stale protocol docs

### Objective

Make the protocol and reverse-engineering docs safe to follow again.

### Primary files

- `docs/development/APEX_PRO_PROTOCOL.md`
- `docs/development/PROTOCOL_RESEARCH.md`
- `docs/development/KEY_MAPPING_RESEARCH.md`

### Implementation tasks

- Replace or explain all references to missing tooling such as `src/bin/bulk_test.rs`.
- Point contributors to the helper binaries that actually exist today.
- Clearly separate:
  - confirmed behavior
  - placeholder code
  - speculative next steps

### Success criteria

- No development doc instructs contributors to use a nonexistent repository tool without explanation.
- The current reverse-engineering workflow is understandable from the docs alone.

### Validation

- Review the updated docs manually.
- Search for stale references such as `bulk_test` and confirm they are either removed or clearly marked historical.

## 7. Phase 3: tighten Apex Pro capability reporting

### Objective

Reduce misleading support claims for Apex Pro TKL 2023 per-key features until the protocol is verified.

### Primary files

- `src/devices/hid_reports.rs`
- `src/devices/key_mapping.rs`
- `src/devices/keyboards/mod.rs`
- `src/main.rs`

### Implementation tasks

- Identify where placeholder mappings or placeholder command support currently appear as confirmed support.
- Separate "mapping exists" from "behavior verified on hardware" where the current code conflates the two.
- Keep the public behavior conservative: it is better to describe support as provisional than to imply fully verified functionality.
- Update nearby docs or CLI help only if needed to keep the behavior consistent.

### Success criteria

- Placeholder per-key support is no longer described as fully verified support.
- Existing verified RGB and actuation behavior remains unchanged.

### Validation

```bash
cd /home/runner/work/steelseriesgg-rs/steelseriesgg-rs
cargo build --locked
cargo test --locked
```

If hardware is available:

- verify zone RGB still works
- verify actuation writes still work
- verify any per-key status output now reflects the intended support level

## 8. Phase 4: reproduce and resolve issue `#6`

### Objective

Determine whether issue `#6` still represents a live failure and fix the smallest affected surface.

### Primary files

- `PKGBUILD`
- `ssgg.install`
- `Cargo.toml`
- `rust-toolchain.toml`
- `.github/workflows/ci.yml`
- `.github/workflows/release-arch.yml`

### Implementation tasks

- Retrieve the original failure details and compare them against the current repository state.
- Reproduce the failure in the smallest realistic environment.
- Decide whether the fix belongs in:
  - Rust code
  - packaging metadata
  - installation documentation
  - CI or release configuration
- Avoid code changes if the problem is already fixed or only affects packaging/docs.

### Success criteria

- The issue is either reproduced and fixed, or shown to be stale with current evidence.
- Any fix remains aligned with the current CI feature matrix.

### Validation

```bash
cd /home/runner/work/steelseriesgg-rs/steelseriesgg-rs
cargo build --release --locked
cargo test --locked
```

If an Arch-like environment is available:

```bash
makepkg -sf --noconfirm
```

## 9. Phase 5: refactor `src/main.rs` only after the blockers above

### Objective

Improve long-term maintainability without increasing risk to the backlog fixes above.

### Primary file

- `src/main.rs`

### Implementation tasks

- Identify command families that can be extracted with low churn.
- Move handlers into focused modules only after correctness-sensitive work is complete.
- Preserve clap behavior, output, and feature gating exactly.

### Success criteria

- The command dispatch surface is easier to navigate and test.
- No CLI behavior changes unintentionally.

### Validation

```bash
cd /home/runner/work/steelseriesgg-rs/steelseriesgg-rs
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

## 10. Deferred research tracks

These remain useful, but they should not outrank the implementation priorities above:

- compare `https://github.com/Sharper-Flow/Open-G-Hub` against the current architecture only when a concrete blocker suggests reusable ideas
- review the remaining external research list from `TODO.md` and classify each project as:
  - directly relevant
  - research-only
  - documentation-only
  - out of scope

This research should support the active backlog items rather than expand the scope on its own.

## 11. Completion criteria

The backlog should be considered meaningfully advanced when:

- issue `#120` has a verified non-hanging behavior and a clear validation story
- stale protocol tooling references are resolved
- Apex Pro capability reporting distinguishes placeholders from verified support
- issue `#6` has either a verified fix or current evidence that it is stale
- any refactor work begins only after the correctness and reliability fixes above are stable

## 12. Suggested next move

If only one follow-up task is taken next, start with issue `#120`.

It has the clearest user impact, the smallest likely implementation surface, and the strongest evidence that a local fail-fast fix in the audio path will improve the current codebase immediately.
