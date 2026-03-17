---
goal: Turn the current TODO backlog into an evidence-driven execution plan for a follow-up agent.
date_created: 2026-03-17
status: Planned
source: TODO.md
---

# PLAN: TODO backlog research and implementation

This file translates `TODO.md` into a concrete plan for the next agent.

The goal is not to implement everything at once. The goal is to research first, verify assumptions against the current repository state, and then make the smallest safe code and documentation changes needed for each TODO item.

## 1. Ground rules for the follow-up agent

- Read `AGENTS.md` before editing code.
- Treat the following files as the source of truth when they disagree with prose docs:
  - `Cargo.toml`
  - `rust-toolchain.toml`
  - `.github/workflows/ci.yml`
  - `src/devices/hid_reports.rs`
- Do not hand-build HID buffers when existing typed helpers already exist. Use `HidReportBuilder` and the current command abstractions.
- Do not loosen GameSense localhost CORS restrictions while working adjacent code.
- Preserve the exact `hidapi = "=2.6.5"` dependency pin unless a task explicitly proves that a change is required.
- Prefer evidence-backed implementation over speculative protocol changes, especially for Apex Pro TKL 2023 per-key RGB and actuation read-back.

## 2. Repository facts that should shape the work

- `TODO.md` currently contains four broad workstreams:
  1. investigate `Sharper-Flow/Open-G-Hub`
  2. fix GitHub issues `#6` and `#120`
  3. continue Apex Pro TKL 2023 protocol and RGB work
  4. research a list of related open-source projects
- The current protocol docs explicitly say per-key RGB is still a placeholder and actuation read-back is not implemented.
- The protocol docs also reference a `src/bin/bulk_test.rs` harness that is not present in the current tree. Before relying on those instructions, reconcile the docs with the actual tooling in `src/bin/`.
- Existing protocol-related docs to review before making changes:
  - `docs/development/APEX_PRO_PROTOCOL.md`
  - `docs/development/PROTOCOL_RESEARCH.md`
  - `docs/development/KEY_MAPPING_RESEARCH.md`
  - `docs/development/RGB_CONTROL_ANALYSIS.md`
- Existing helper binaries that are actually present:
  - `src/bin/discover_actuation.rs`
  - `src/bin/verify_key_mapping.rs`
  - `src/bin/benchmark_rgb_alloc.rs`
  - `src/bin/sonar_control.rs`

## 3. Global execution strategy

Work in this order unless new evidence forces a change:

1. establish a fresh research baseline
2. reconcile stale documentation and tooling references
3. investigate issue `#6`
4. investigate issue `#120`
5. research external projects and extract only the parts that clearly fit this codebase
6. continue Apex Pro TKL 2023 protocol and RGB implementation only after the research above narrows the next safe step

Do not start with speculative Apex protocol edits. The repository already contains placeholder structures and research notes; the next useful step is to reduce uncertainty, not add more guesswork.

## 4. Phase 1: establish the research baseline

### Objective

Create a reliable starting point so later changes are based on the live repo, live issues, and current docs rather than stale assumptions.

### Tasks

- Read and summarize:
  - `TODO.md`
  - `AGENTS.md`
  - issue `#6`: `https://github.com/Ven0m0/steelseriesgg-rs/issues/6`
  - issue `#120`: `https://github.com/Ven0m0/steelseriesgg-rs/issues/120`
- Confirm current build and test behavior from `.github/workflows/ci.yml`.
- Create a dedicated research note for the backlog, preferably under a tracking directory if one is acceptable for the current task, otherwise under `docs/development/`.
- Record what is confirmed, what is unverified, and what requires hardware.

### Recommended deliverable

A single research log that includes:

- the original TODO items
- linked issue summaries
- current protocol status
- a list of dead or stale references in the docs
- a list of external projects worth deeper comparison

### Validation commands

```bash
cd /home/runner/work/steelseriesgg-rs/steelseriesgg-rs
cargo fmt --all -- --check
cargo test --locked
```

## 5. Phase 2: reconcile stale docs and missing tooling references

### Objective

Make sure future protocol work points at real files and current workflows.

### Tasks

- Audit these files for references to tools or flows that no longer exist:
  - `docs/development/APEX_PRO_PROTOCOL.md`
  - `docs/development/PROTOCOL_RESEARCH.md`
  - `docs/development/KEY_MAPPING_RESEARCH.md`
- Specifically resolve references to `src/bin/bulk_test.rs`, because the current tree does not contain that binary.
- Decide whether the correct fix is:
  - restore the missing tool,
  - replace those instructions with the existing tools in `src/bin/`, or
  - clearly mark the old workflow as historical and provide the new path.
- Update docs so they distinguish between:
  - confirmed hardware behavior
  - placeholder implementations
  - speculative next steps

### Success criteria

- No protocol doc tells contributors to use a nonexistent binary without explanation.
- The repo clearly shows which current tools support research today.

## 6. Phase 3: investigate TODO item 1 (`Open-G-Hub`)

### Objective

Determine whether `https://github.com/Sharper-Flow/Open-G-Hub` contains logic that can be reused or adapted safely in this project.

### Research questions

- Which parts of Open-G-Hub overlap with this repository?
  - device discovery
  - HID transport
  - RGB control patterns
  - headset/audio handling
  - packaging and permissions
- Which device families overlap, and which do not?
- Does it contain SteelSeries-specific protocol knowledge, or only generic infrastructure?
- Is the design compatible with the current architecture here:
  - `src/devices/`
  - `src/audio/`
  - `src/gamesense/`
  - `src/main.rs`
- Are there licensing or architectural reasons not to port anything directly?

### Expected output

Produce a short comparison table with:

- subsystem
- what Open-G-Hub does
- whether it is directly reusable, adaptable, or irrelevant
- exact local files where an adaptation would land

### Likely local integration points

- `src/devices/discovery.rs`
- `src/devices/mod.rs`
- `src/devices/hid_reports.rs`
- `src/audio/mod.rs`
- `src/audio/pulse.rs`

### Implementation rule

Do not port ideas from Open-G-Hub unless you can point to a concrete mismatch or limitation in the current codebase that the idea would solve.

## 7. Phase 4: investigate and fix issue `#6`

### Objective

Resolve the compile/build problem tracked in `https://github.com/Ven0m0/steelseriesgg-rs/issues/6`, or prove that it is stale and should be updated/closed.

### Known context

- Issue title: `Fails to compile on arch system`
- The issue body references `cargo build --release`.
- Comments suggest the problem may also have appeared on Debian.
- The linked error log is outside the repository, so fetch it before making speculative fixes.

### Tasks

- Retrieve the attached error log from the issue and summarize the actual compiler or packaging failures.
- Reproduce the failure in the smallest realistic environment:
  - Arch-like environment if possible
  - otherwise a controlled Linux environment with matching toolchain assumptions
- Compare the issue against the current repo state:
  - `PKGBUILD`
  - `ssgg.install`
  - `Cargo.toml`
  - `rust-toolchain.toml`
  - `.github/workflows/ci.yml`
- Decide whether the fix belongs in:
  - Rust code
  - packaging metadata
  - README or install instructions
  - CI and release workflow definitions

### Implementation guidance

- If the issue is already fixed on current `main`, update the issue with reproduction evidence instead of inventing a new change.
- If the issue is packaging-only, keep the code untouched and fix the packaging/docs.
- If the issue is a real compiler problem, change only the smallest affected area and verify against the current build matrix.

### Validation commands

```bash
cd /home/runner/work/steelseriesgg-rs/steelseriesgg-rs
cargo build --release --locked
cargo test --locked
```

If you have an Arch environment:

```bash
makepkg -sf --noconfirm
```

## 8. Phase 5: investigate and fix issue `#120`

### Objective

Resolve the hang tracked in `https://github.com/Ven0m0/steelseriesgg-rs/issues/120`, where `ssgg audio ...` becomes unresponsive.

### Known context

- Reported commands that hang:
  - `ssgg audio status`
  - `ssgg audio chat-mix 50`
- The reporter built with:
  - `cargo build --release --features audio`
- The report mentions an `Arctis Pro Wireless` and an unknown additional SteelSeries USB device.

### Highest-priority files to inspect

- `src/audio/pulse.rs`
- `src/audio/mod.rs`
- `src/main.rs`

### Tasks

- Reproduce the hang using the `audio` feature.
- Add temporary instrumentation if needed to identify where execution stops.
- Check for indefinite waits around:
  - PulseAudio context readiness
  - callback wakeups
  - channel receive points
  - sink enumeration or sink-input collection
- Make the code fail fast with an explicit error or timeout instead of appearing frozen.
- Ensure the CLI remains responsive even when PulseAudio is absent, misconfigured, or only partially available.
- Add or update tests for any new timeout/error-path behavior if practical.

### Implementation guidance

- Favor bounded waiting and explicit error propagation over retry loops without timeouts.
- Keep the change local to the audio path unless evidence shows the problem originates elsewhere.
- Preserve current feature gating and do not make `audio` depend on unrelated features.

### Validation commands

```bash
cd /home/runner/work/steelseriesgg-rs/steelseriesgg-rs
cargo build --locked --features audio
cargo clippy --all-targets --locked --features audio -- -D warnings
cargo test --locked --features audio
RUST_LOG=debug cargo run --features audio -- audio status
RUST_LOG=debug cargo run --features audio -- audio chat-mix 50
```

### Manual verification checklist

- commands return promptly
- missing PulseAudio is reported cleanly
- no silent hangs remain
- live audio changes still work when the environment supports them

## 9. Phase 6: continue Apex Pro TKL 2023 protocol and RGB work

### Objective

Advance the Apex Pro TKL 2023 implementation, but only where the next step is supported by evidence from code, docs, captures, or hardware validation.

### Current status to preserve

- zone RGB is implemented
- per-key RGB is still documented as placeholder behavior
- actuation write exists via command `0x2D`
- actuation read-back is still unknown
- placeholder key mappings exist and are not verified hardware truth

### First tasks before changing code

- Compare the current implementation against the latest protocol docs:
  - `src/devices/hid_reports.rs`
  - `src/devices/keyboards/mod.rs`
  - `src/devices/keyboards/apex_pro_tkl_2023.rs`
  - `src/devices/key_mapping.rs`
  - `src/devices/zone_mapping.rs`
- Reconcile any drift between the docs and the current code.
- Decide which single next protocol goal is most realistic:
  - per-key RGB discovery
  - key address verification
  - actuation read-back discovery
  - OSD/menu command abstraction

### Recommended research workflow

1. confirm the available research tooling in `src/bin/`
2. compare with official behavior if protocol capture is feasible
3. test one protocol surface at a time
4. update docs immediately when a behavior becomes confirmed or disproven
5. only then update command builders and higher-level APIs

### Potential implementation targets

- `src/devices/hid_reports.rs`
- `src/devices/key_mapping.rs`
- `src/devices/zone_mapping.rs`
- `src/devices/keyboards/apex_pro_tkl_2023.rs`
- `src/rgb/mod.rs`
- `src/rgb/tests.rs`
- `docs/development/APEX_PRO_PROTOCOL.md`
- `docs/development/KEY_MAPPING_RESEARCH.md`
- `docs/development/PROTOCOL_RESEARCH.md`

### Guardrails

- Do not present placeholder per-key support as confirmed support.
- Do not replace typed HID helpers with manual byte arrays.
- Do not expand device support claims without hardware-backed evidence.

### Validation commands

```bash
cd /home/runner/work/steelseriesgg-rs/steelseriesgg-rs
cargo build --locked
cargo test --locked
cargo run -- devices
cargo run -- validate
cargo run -- bug-report
```

If hardware is available, add focused manual checks for:

- zone RGB still working
- actuation writes still working
- any newly discovered command affecting only the intended keys or settings

## 10. Phase 7: research the remaining external projects from TODO item 4

### Objective

Extract actionable ideas from the external project list without cargo-culting unrelated implementations.

### Projects to review

- `https://github.com/flozz/rivalcfg`
- `https://github.com/PrzemekkkYT/GGSonarRev`
- `https://github.com/llMBQll/OmniLED`
- `https://codeberg.org/Birbwell/linuxmix`
- `https://github.com/AstroSnail/apexctl`
- `https://github.com/FrankGrimm/apex7tkl_linux`
- `https://github.com/Gibtnix/Apex-Macros`
- `https://github.com/wex/sonar-rev`
- `https://github.com/Mark7888/steelseries-sonar-py`
- `https://github.com/Dymstro/nova-chatmix-linux`
- `https://github.com/not-jan/apex-tux`
- `https://github.com/Gibtnix/MSIKLM`
- `https://github.com/stephenlacy/msi-keyboard`

### Suggested categorization

For each project, classify it under one or more of:

- keyboard RGB / HID protocol
- headset or Sonar reverse engineering
- device discovery and permissions
- packaging or distro integration
- ideas that are unrelated to this repository

### Required output

For each project, record:

- what it targets
- why it is relevant or not relevant
- what exact local file or subsystem it could influence
- whether it should be:
  - ignored
  - referenced in docs
  - used as research input only
  - adapted into a concrete implementation task

### Important rule

Do not broaden scope just because a project is interesting. Only convert research into implementation work if it directly supports one of the TODO items or a blocker discovered while fixing them.

## 11. Completion criteria

The TODO backlog should be considered meaningfully advanced only when:

- every TODO item has a documented research outcome
- issue `#6` has either a verified fix or evidence that it is stale
- issue `#120` no longer hangs and has a clear validation story
- Apex protocol docs accurately distinguish confirmed behavior from placeholders
- stale protocol-tooling references are resolved
- any code changes are accompanied by the smallest relevant existing validation commands

## 12. Minimum validation matrix for future implementation work

Default path:

```bash
cd /home/runner/work/steelseriesgg-rs/steelseriesgg-rs
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

Sonar-related work:

```bash
cd /home/runner/work/steelseriesgg-rs/steelseriesgg-rs
cargo clippy --all-targets --locked --features sonar -- -D warnings
cargo test --locked --features sonar
```

Audio-related work:

```bash
cd /home/runner/work/steelseriesgg-rs/steelseriesgg-rs
cargo clippy --all-targets --locked --features audio -- -D warnings
cargo test --locked --features audio
```

## 13. Suggested first move for the next agent

Start by creating a short research note that answers these five questions before writing code:

1. What in the current docs is stale or unverifiable?
2. What exactly caused issue `#6`, based on the attached log?
3. Where exactly does issue `#120` block or wait?
4. Which external project has the highest-value overlap with the current blockers?
5. What is the single safest next Apex Pro TKL 2023 protocol step that is backed by evidence?

If those five answers are not clear yet, keep researching. If they are clear, implement the smallest next change and validate it immediately.
