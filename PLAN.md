---
goal: Prioritized implementation plan combining open GitHub issues and TODO backlog.
date_created: 2026-03-17
last_updated: 2026-05-22
last_reviewed: 2026-05-22
status: In Progress
sources: GitHub issues #6, #165, #173, #211 · TODO.md · prior PLAN.md
---

# PLAN: implementation priorities

This plan converts all open GitHub issues (excluding the Dependency Dashboard) and the
active TODO backlog into a single prioritized implementation order.

---

## 1. Constraints to preserve

- Read `CLAUDE.md` (same content as `AGENTS.md`) before making code changes.
- Treat these files as source of truth when prose docs drift:
  - `Cargo.toml`, `rust-toolchain.toml`, `.github/workflows/ci.yml`, `src/devices/hid_reports.rs`
- Keep `hidapi = "=2.6.6"` pinned unless a task explicitly requires changing it.
- Do not loosen the localhost-only GameSense CORS policy.
- Keep `audio` and `sonar` feature gating independent.
- Do not replace typed HID helpers (`HidReportBuilder`) with manual byte arrays.
- Do not present placeholder or unverified protocol support as confirmed hardware behavior.
- No `.unwrap()` / `.expect()` in production paths.

---

## 2. Status summary

### Completed

| Item | Closed | Notes |
|------|--------|-------|
| Issue #120 — audio hang | 2026-03-27 | 5 s timeout added in `src/audio/pulse.rs` |
| experimental-apex-2023 feature flag | 2026-03-26 | exists in `Cargo.toml` |
| CLI command expansion (HidLogs, TestDevice, VerifyPerformance, Fuzz) | 2026-03-26 | subcommands present in `src/main.rs` |

### Open — ranked by priority

| # | Issue / Item | Priority | Phase |
|---|--------------|----------|-------|
| #211 | Arctis Nova Pro Omni not recognised (PID `0x2290`) | High | 1 |
| #165 | Apex Pro TKL Wireless 2023 not recognised (PID `0x1630`) | High | 2 |
| #173 | Apex 3 TKL — commands succeed but RGB does nothing | High | 3 |
| #6   | Fails to compile on Arch | Medium | 4 |
| TODO | Apex Pro TKL 2023 key matrix RE (placeholder data) | Medium | 5 |
| TODO | Protocol docs reconciliation | Low | 6 |
| TODO | Apex Pro capability accuracy cleanup | Low | 7 |
| —    | `src/main.rs` refactor | Low | 8 |

---

## 3. Phase 1 — Add Arctis Nova Pro Omni (Issue #211)

### Context

User reports `ssgg devices` shows the headset as `Unknown SteelSeries Device [Unknown]` with
VID `0x1038`, PID `0x2290`. The device is a USB Audio + HID composite (5 interfaces). The
`lsusb -v` dump in the issue shows two HID interfaces (3 and 4); interface 3 carries a 64-byte
interrupt endpoint, which matches the pattern used by other Arctis headsets.

### Root cause

`0x2290` is absent from `src/devices/mod.rs` `product_ids` module, so
`device_type_from_product_id` returns `Unknown` and `device_name_from_product_id` returns
the fallback string.

### Primary files

- `src/devices/mod.rs` — `product_ids`, `device_type_from_product_id`, `device_name_from_product_id`, `zone_count_for_product_id`

### Implementation tasks

1. Add `pub const ARCTIS_NOVA_PRO_OMNI: u16 = 0x2290;` to the `product_ids` block.
2. Add `ARCTIS_NOVA_PRO_OMNI` to the `DeviceType::Headset` arm of `device_type_from_product_id`.
3. Add `ARCTIS_NOVA_PRO_OMNI => "Arctis Nova Pro Omni"` to `device_name_from_product_id`.
4. No change needed to `zone_count_for_product_id` (keyboard-only); headset PIDs should rely on existing defaults until RGB support is implemented.
5. Extend the existing unit test block to cover the new PID.

### Notes

The Arctis Nova Pro Omni uses USB Audio interfaces 1 and 2 for audio streaming. Control messages
are expected on HID interface 3 (64-byte interrupt endpoint). Do not attempt to implement volume/
chat-mix control until the HID report format is captured; just getting the device recognised and
correctly typed is the goal for this phase.

### Success criteria

- `ssgg devices` names the headset correctly and classifies it as `Headset`.
- `cargo test --locked` passes.
- No existing headset or keyboard behaviour changes.

### Validation

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

### Complexity

⭐ Low — registry-only change, no protocol work required.

---

## 4. Phase 2 — Add Apex Pro TKL Wireless 2023 PID `0x1630` (Issue #165)

### Context

User connects an Apex Pro TKL Wireless (2023) on Debian 13. `ssgg devices` shows
`Unknown SteelSeries Device [Unknown]` with PID `0x1630`. The existing wireless constant is
`APEX_PRO_TKL_2023_WIRELESS = 0x1632`. The `0x1630` PID is a different firmware or hardware SKU
of the same product line.

### Root cause

`0x1630` is not registered. It likely requires the same interface-3 / raw-hidraw path already
implemented for `0x1632` in `src/devices/discovery.rs`.

### Primary files

- `src/devices/mod.rs` — `product_ids` block, match arms
- `src/devices/discovery.rs` — wireless branch in `open_keyboard`

### Implementation tasks

1. Add `pub const APEX_PRO_TKL_2023_WIRELESS_2: u16 = 0x1630;` (or fold into an alias — pick
   the name that keeps the match arms readable).
2. Mirror all match arms that handle `APEX_PRO_TKL_2023_WIRELESS` to also cover `0x1630`:
   - `device_type_from_product_id` → `Keyboard`
   - `device_name_from_product_id` → `"Apex Pro TKL (2023) Wireless"`
   - `zone_count_for_product_id` → 9
   - `open_keyboard` wireless branch (interface 3, raw hidraw path)
3. Extend unit tests to cover the new PID.

### Notes

Without hardware confirmation, treat `0x1630` as behaving identically to `0x1632`. If the user
can test and report back, the assumption can be verified or corrected cheaply.

### Success criteria

- Device is recognised as `Apex Pro TKL (2023) Wireless` with correct zone count.
- `cargo test --locked` passes.

### Validation

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

### Complexity

⭐ Low — mirrors an existing registration path.

---

## 5. Phase 3 — Apex 3 TKL RGB not applied (Issue #173)

### Context

User reports `ssgg rgb solid #ff0000` on an Apex 3 TKL (PID `0x1622`) returns success but the
keyboard does not change colour. The `Apex3Tkl` struct delegates `set_color` and `set_zone_colors`
entirely to `GenericKeyboard`, which uses `HidReportBuilder` with the standard Apex Pro command
layout. The Apex 3 TKL uses a **different HID report dialect**; the 0x23/0x25/0x26 command bytes
in `Apex3Tkl` were written speculatively and have not been verified on hardware.

### Root cause candidates (in order of likelihood)

1. **Wrong command byte** — `GenericKeyboard::set_color` sends a command code that the Apex 3
   TKL firmware ignores silently.
2. **Missing commit / apply step** — firmware only latches colours after a specific commit report,
   which may differ from the Apex Pro apply sequence.
3. **Wrong interface** — the control HID interface for the Apex 3 TKL may not be interface 0
   (the default for wired keyboards in `open_device`).

### Investigation required before coding

The fix depends on capturing real USB HID traffic from SteelSeries GG on Windows while the Apex
3 TKL changes colour. Until that capture exists, do not guess at protocol bytes.

**Preferred investigation path:**
1. USB capture with Wireshark / USBPcap on a Windows machine with SteelSeries GG and an Apex 3 TKL.
2. Decode the HID feature reports — specifically the report bytes sent when `Lighting → Solid`
   is applied.
3. Cross-reference with `src/devices/hid_reports.rs` to identify the correct `CommandCode`.

**Community sources to check first:**
- `https://github.com/AstroSnail/apexctl` (may cover Apex 3 protocol)
- `https://github.com/FrankGrimm/apex7tkl_linux`

### Primary files

- `src/devices/keyboards/apex.rs` — `Apex3Tkl` implementation
- `src/devices/keyboards/mod.rs` — `GenericKeyboard` command paths
- `src/devices/hid_reports.rs` — command codes

### Implementation tasks (post-investigation)

1. Identify the correct command byte(s) for solid-color and zone-color on Apex 3 TKL.
2. Override `set_color` and `set_zone_colors` in `Apex3Tkl` with the verified protocol instead
   of delegating to `GenericKeyboard`'s Apex Pro path.
3. Verify the apply/commit sequence (if any).
4. Add a doc comment marking any remaining unverified commands as experimental.

### Success criteria

- `ssgg rgb solid #ff0000` visibly changes the keyboard colour on real hardware.
- Existing GenericKeyboard behaviour for other keyboards is unchanged.
- `cargo test --locked` passes.

### Validation

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
# On hardware:
ssgg rgb solid "#ff0000"   # red should appear
ssgg rgb solid "#000000"   # off should work
```

### Complexity

⭐⭐⭐ High — protocol investigation is a hard dependency; the code change itself will be small
once the correct bytes are known.

---

## 6. Phase 4 — Arch compile failure (Issue #6)

### Context

Issue #6 reports `cargo build --release` fails on Arch. No activity since Jan 2026; it may be
stale. The error log in the issue is the primary evidence. The PKGBUILD and release-arch workflow
are the most likely fix surfaces if the root cause is packaging rather than code.

### Primary files

- `PKGBUILD`, `ssgg.install`
- `Cargo.toml`, `rust-toolchain.toml`
- `.github/workflows/ci.yml`, `.github/workflows/release-arch.yml`

### Implementation tasks

1. Read the attached error log from issue #6 to identify the failing crate or linker step.
2. Compare the failure against current `Cargo.toml` — dependencies may already be updated.
3. If still reproducible: fix in the smallest surface (packaging / CI / code) that matches
   the evidence.
4. If stale: close the issue with a comment citing the current passing CI state.

### Success criteria

- `cargo build --release --locked` passes on an Arch-like environment, **or**
- Issue is confirmed stale with evidence and closed.

### Validation

```bash
cargo build --release --locked
cargo test --locked
# If Arch env available:
makepkg -sf --noconfirm
```

### Complexity

⭐⭐ Medium if still live; ⭐ Low if stale.

---

## 7. Phase 5 — Apex Pro TKL 2023 key matrix RE (TODO backlog)

### Context

All 87 `KeyId → KeyAddress` mappings in `src/devices/key_mapping.rs` are placeholder data
(estimated 6×17 matrix with guessed row/col values). The runtime falls back to
`simulate_per_key_with_zones()`. Real addresses are needed for true per-key RGB.

### Research methods (in order of preference)

1. Binary RE of `SteelSeriesGG107.exe` for key address lookup tables.
2. USB capture while SteelSeries GG sets per-key colours; decode byte positions.
3. Community sources: `https://github.com/AstroSnail/apexctl` and
   `https://github.com/FrankGrimm/apex7tkl_linux`.

### Primary files

- `src/devices/key_mapping.rs` — `KeyMappingDatabase::new()` for `ApexProTkl2023`
- `docs/development/KEY_MAPPING_RESEARCH.md` — status per key

### Implementation tasks

1. Extract verified HID codes or physical matrix addresses for all 87 keys (or the TKL subset).
2. Replace placeholder KeyAddress::new(hid_code) calls with verified values.
3. Remove or update `⚠️ PLACEHOLDER` warnings in `KEY_MAPPING_RESEARCH.md`.
4. Confirm `simulate_per_key_with_zones()` fallback is no longer triggered.

### Success criteria

- At least the alphanumeric block and modifier keys have verified addresses.
- `cargo build --locked --features experimental-apex-2023` passes.
- `cargo test --locked --features experimental-apex-2023` passes.
- `KEY_MAPPING_RESEARCH.md` reflects verified vs. still-placeholder keys.

### Complexity

⭐⭐⭐ High — depends on RE outcome; code change is straightforward once addresses are known.

---

## 8. Phase 6 — Protocol docs reconciliation (TODO backlog)

### Objective

Make `docs/development/` safe to follow. Remove or explain stale references; clearly separate
confirmed behavior, placeholder code, and speculative next steps.

### Primary files

- `docs/development/APEX_PRO_PROTOCOL.md`
- `docs/development/PROTOCOL_RESEARCH.md`
- `docs/development/KEY_MAPPING_RESEARCH.md`

### Implementation tasks

1. Verify no doc references a nonexistent binary (historical `bulk_test.rs` was resolved; confirm
   no others were missed).
2. Confirm each reference to a helper binary (`discover_actuation`, `verify_key_mapping`,
   `benchmark_rgb_alloc`, `sonar_control`) still points to a file that exists.
3. Add clear `> ⚠️ UNVERIFIED` callouts wherever a command code or address is a placeholder.

### Success criteria

- No development doc instructs contributors to use a nonexistent tool.
- Confirmed vs. speculative protocol sections are clearly separated.

### Complexity

⭐ Low — documentation only.

---

## 9. Phase 7 — Apex Pro capability accuracy cleanup (TODO backlog)

### Objective

Ensure placeholder per-key support is not presented as verified. This is a
maintainability/accuracy task, not a protocol expansion.

### Primary files

- `src/devices/hid_reports.rs`
- `src/devices/key_mapping.rs`
- `src/devices/keyboards/mod.rs` and `apex_pro_tkl_2023.rs`

### Implementation tasks

1. Identify every location where placeholder command codes (`PerKeyRgb (0x23)`,
   `Apex2023Direct (0x40)`) are described or logged without a `⚠️ experimental` qualifier.
2. Add explicit qualifiers in doc comments, log messages, or CLI help text as needed.
3. Do not change runtime behavior — accuracy only.

### Success criteria

- Placeholder support is consistently labelled as provisional or experimental.
- `cargo test --locked` passes.
- `cargo test --locked --features experimental-apex-2023` passes.

### Complexity

⭐ Low — comments and strings only.

---

## 10. Phase 8 — `src/main.rs` refactor (deferred)

### Objective

Improve long-term maintainability of the CLI dispatch layer.

### Primary file

- `src/main.rs`

### Implementation tasks

1. Identify command families that can be extracted into focused modules with low churn.
2. Move handlers only after all higher-priority fixes are stable.
3. Preserve clap behavior, output, and feature gating exactly.

### Success criteria

- Command dispatch is easier to navigate and test.
- No CLI behavior changes.

### Validation

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

### Complexity

⭐⭐⭐ High — large surface; must not start until Phases 1–7 are stable.

---

## 11. Phase dependencies

```
Phase 1 (Nova Pro Omni PID)   ──────────────────────────────┐
Phase 2 (Wireless 0x1630 PID) ──────────────────────────────┤
Phase 4 (Arch compile)        ──────────────────────────────┤──► Phase 8 (main.rs refactor)
Phase 6 (Docs reconcile)      ──────────────────────────────┤
Phase 7 (Capability accuracy) ──────────────────────────────┘

Phase 3 (Apex 3 TKL RGB)     ──► requires USB capture first ──► Phase 8
Phase 5 (Key matrix RE)      ──► requires RE session first  ──► Phase 8
```

| Phase | Blocked by | Reason |
|-------|------------|--------|
| 1, 2  | Nothing    | Registry-only; self-contained |
| 3     | USB capture | Protocol bytes unknown without trace |
| 4     | Nothing    | Independent investigation |
| 5     | RE session  | Key addresses unknown |
| 6, 7  | Nothing    | Documentation only |
| 8     | 1–7 stable  | Refactor risk |

---

## 12. Deferred research

- **Open-G-Hub** (`https://github.com/Sharper-Flow/Open-G-Hub`) — defer unless a concrete
  blocker suggests reusable logic.
- Sonar / audio research links (from TODO.md) — relevant when Sonar protocol work resumes:
  - https://github.com/PrzemekkkYT/GGSonarRev
  - https://github.com/wex/sonar-rev
  - https://github.com/Mark7888/steelseries-sonar-py
  - https://codeberg.org/Birbwell/linuxmix
  - https://github.com/Dymstro/nova-chatmix-linux
- Apex protocol research:
  - https://github.com/AstroSnail/apexctl
  - https://github.com/FrankGrimm/apex7tkl_linux
  - https://github.com/not-jan/apex-tux
- Research-only:
  - https://github.com/flozz/rivalcfg
  - https://github.com/llMBQll/OmniLED

---

## 13. Effort estimates

| Phase | Complexity | Estimate | Confidence |
|-------|------------|----------|------------|
| 1 | Low    | 30–60 min  | High |
| 2 | Low    | 30–60 min  | High |
| 3 | High   | 4–16 h     | Low (gated on capture) |
| 4 | Medium | 2–8 h      | Low (may be stale) |
| 5 | High   | 4–16 h     | Low (gated on RE) |
| 6 | Low    | 1–2 h      | High |
| 7 | Low    | 1–2 h      | High |
| 8 | High   | 8–16 h     | Medium |

**Total estimated:** 21–61 h (if all phases completed)

---

## 14. Suggested next move

Start with **Phase 1** (Arctis Nova Pro Omni PID registration). It is a self-contained
registry addition — a single `const`, four match arms, and a unit test — with no protocol
risk and immediate user impact.

Follow immediately with **Phase 2** (Apex Pro TKL Wireless `0x1630`), which is structurally
identical.

Both can be done in one sitting and committed together.
