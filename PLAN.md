---
goal: Execution-ready implementation plan for the open backlog (issues + TODO).
date_created: 2026-03-17
last_updated: 2026-08-18
last_reviewed: 2026-06-15
status: In Progress
sources: GitHub issues #173 · TODO.md · OpenRGB SteelSeriesApex8ZoneController · prior PLAN.md
---

# PLAN: implementation priorities

Converts the open GitHub issues (excluding the Renovate Dependency Dashboard #126) and the
active TODO backlog into one prioritized, execution-ready order for a future agent session.

---

## 1. Overview

The only open code-bug issue is **#173 (Apex 3 TKL RGB not applied)** — now the top priority
because a concrete protocol reference exists (OpenRGB's 8-zone controller). The remaining work
is reverse-engineering and accuracy/maintainability cleanup carried over from `TODO.md`.

Device-registration issues #211 (Arctis Nova Pro Omni `0x2290`) and #165 (Apex Pro TKL Wireless
2023 `0x1630`) are **implemented in code** (`src/devices/mod.rs`, `discovery.rs`, with tests).
They remain open on GitHub only because they have not been closed; verify on hardware / close
them out separately. Issue **#6 (Arch compile)** was **closed `not_planned` on 2026-05-23** and
is dropped from this plan.

---

## 2. Constraints to preserve

- Read `CLAUDE.md` (same content as `AGENTS.md`) before making code changes.
- Source-of-truth files when prose drifts:
  `Cargo.toml`, `rust-toolchain.toml`, `.github/workflows/ci.yml`, `src/devices/hid_reports.rs`.
- Keep `hidapi = "=2.6.6"` pinned unless the task explicitly requires changing it.
- Do not loosen the localhost-only GameSense CORS policy.
- Keep `audio` and `sonar` feature gating independent.
- Always build HID reports with `HidReportBuilder` / typed helpers — never hand-rolled byte arrays.
- No `.unwrap()` / `.expect()` in production paths.
- Do not present placeholder or unverified protocol support as confirmed hardware behavior.
- Minimal scope: fix what the task asks, no opportunistic refactors.

---

## 3. Status summary

### Completed

| Item | Notes |
|------|-------|
| Issue #120 — audio hang | 5 s timeout in `src/audio/pulse.rs` |
| `experimental-apex-2023` feature flag | present in `Cargo.toml` |
| CLI expansion (HidLogs, TestDevice, VerifyPerformance, Fuzz) | subcommands in `src/main.rs` |
| Issue #211 — Arctis Nova Pro Omni PID `0x2290` | `ARCTIS_NOVA_PRO_OMNI` const + match arms + test (`src/devices/mod.rs:453`); GitHub issue still open — close after hardware confirmation |
| Issue #165 — Apex Pro TKL Wireless 2023 PID `0x1630` | `APEX_PRO_TKL_2023_WIRELESS_2` registered in `mod.rs` + `discovery.rs` + tests; GitHub issue still open — close after hardware confirmation |
| Issue #6 — Arch compile | Closed `not_planned` 2026-05-23 — no action |

### Open — ranked

| # | Item | Priority | Phase |
|---|------|----------|-------|
| #173 | Apex 3 TKL — commands succeed but RGB does nothing | High | 1 |
| TODO | Apex Pro TKL 2023 key matrix RE + actuation read-back | Medium | 2 |
| Done | Protocol docs reconciliation | Low | 3 |
| Done | Apex Pro capability accuracy cleanup | Low | 4 |
| — | `src/main.rs` refactor | Low | 5 |
| Done | Windows pollrate status UX (unsupported-IOCTL messaging) | Low | 6 |
| TODO | Triage `ssgg_bug_report.json` | Medium | 7 |
| Done | SignalRGB reverse-engineering research | Low | 8 |

---

## 4. Phase 1 — Apex 3 TKL RGB not applied (Issue #173) · High

### Context

`ssgg rgb solid #ff0000` on an Apex 3 TKL (PID `0x1622`) reports success but nothing changes;
multiple users confirm (`#173`), and runtime logs `WARN No key mapping available for product ID
0x1622 - per-key RGB disabled`. The Apex 3 TKL is a **zone** keyboard (8–10 zones), not per-key,
so the missing key map is expected — the real bug is the zone-color command path.

`Apex3Tkl` (`src/devices/keyboards/apex.rs`) delegates `set_color` / `set_zone_colors` to
`GenericKeyboard`, and its own `CMD_RGB_EFFECT = 0x23` (line 24) was written speculatively.

### Concrete lead (OpenRGB 8-zone protocol)

Reference: OpenRGB `SteelSeriesApex8ZoneController.h` (linked by a user in #173, owner acked
2026-06-15). 65-byte report (report ID byte + 64 payload):

| Command | Byte 0 | Layout |
|---------|--------|--------|
| Set zone colors | `0x21` | byte 1 = LED bitmask (`0xFF` = all 8), bytes 2–25 = `R G B` × 8 zones, rest zero-padded |
| Rainbow wave | `0x22` | byte 1 = `0xFF` |
| Brightness | `0x23` | byte 1 = `0x00`–`0x10` (multiplier, persists across Mod+F11/F12) |

**Likely root cause (as originally drafted):** the current code uses `0x23` (brightness in this
dialect) to push color, so the firmware applies a brightness write and ignores the intended
color. The correct set-color command is **`0x21`** with the per-zone RGB layout above.

### Research update (2026-08-18) — premise above is stale, verified against current source

Checked `src/devices/hid_reports.rs`, `src/devices/keyboards/mod.rs`, `src/devices/keyboards/apex.rs`,
`src/devices/discovery.rs` against the OpenRGB lead. The `0x23`-overwrites-color theory does **not**
match current code:

- `GenericKeyboard::set_color` / `set_zone_colors` (`keyboards/mod.rs:521-540`) already call
  `RgbZoneCommand::new_all_zones` / `new_specific_zone`, whose `command_code()` is
  `CommandCode::RgbControl = 0x21` (`hid_reports.rs:35,173-174`). `Apex3Tkl` delegates `set_color`/
  `set_zone_colors` straight to this inner `GenericKeyboard` (`apex.rs:118-125`) — so the live color
  path is already `0x21`, not `0x23`.
- `RgbZoneCommand::serialize` (`hid_reports.rs:178-215`) writes `[report_id=0x00][cmd=0x21]
  [zone_selector][R G B]×N` — byte-for-byte the OpenRGB 8-zone layout the plan cites (`zone_selector
  = 0xFF` for all-zones, from `send_zone_buffer_async`).
- `Apex3Tkl::CMD_RGB_EFFECT = 0x23` (`apex.rs:24`) is real but dead on this path: it's only used by
  `set_rgb_effect()`, an unused experimental method never called from `set_color`/`set_zone_colors`.
- Device routing is correct: PID `0x1622` → `Apex3Tkl::new(generic_keyboard)`
  (`discovery.rs:449`), so the wrapper in question is the one actually instantiated.

**So issue #173 is not explained by the `0x21` vs `0x23` mixup** — that part of the protocol was
already fixed (commit history shows `hid_reports.rs` and this call path are new since the plan was
drafted; PLAN.md's status table was not updated to match). Two things from the OpenRGB lead are
still genuinely open, and are the real remaining candidates:

1. **Zone count mismatch (still unresolved, same as Open Question 1 below):** code sends 9 zones
   (`zone_count_for_product_id` → `APEX_3_TKL => 9`, `discovery.rs mod.rs:621`); OpenRGB models this
   family as 8. `MAX_RGB_ZONES = 12` so the 9th zone doesn't overflow the buffer, but if firmware
   expects exactly 8 `RGB` triplets at fixed offsets, byte 26 onward may be read as garbage/next-field
   by the firmware rather than ignored.
2. **Command-code table mismatch vs the OpenRGB reference itself:** code's
   `CommandCode::Brightness = 0x22` (`hid_reports.rs:39-40`), but the OpenRGB table this plan quotes
   has `0x22 = rainbow wave` and `0x23 = brightness`. Nothing in the current color path sends a
   brightness/apply write automatically, so this mismatch isn't yet proven to matter — but if `0x21`
   alone doesn't light the keyboard on hardware, sending `CommandCode::Brightness` (currently 0x22)
   is untested against this specific 8-zone family and may be the wrong byte per OpenRGB's numbering.

Given the code already matches the primary OpenRGB lead, **Phase 1's remaining blocker is hardware
confirmation, not further code changes** — retest `ssgg rgb solid` on real Apex 3 TKL hardware first;
only chase zone-count/brightness-byte changes if that retest still shows no LED response.

### Primary files

- `src/devices/keyboards/apex.rs` — `Apex3Tkl` (`CMD_RGB_EFFECT`, color delegation lines 118–146)
- `src/devices/keyboards/mod.rs` — `GenericKeyboard::set_color` / `set_zone_colors`
- `src/devices/hid_reports.rs` — add/confirm the `0x21` zone-color command code via `HidReportBuilder`
- `src/devices/zone_mapping.rs:311` — `APEX_3_TKL` zone mapping (currently registered)

### Tasks

1. ~~Reference the existing typed command code for the 8-zone set-color (CommandCode::RgbControl,
   0x21) in hid_reports.rs; build the report with HidReportBuilder~~ — **done**: `RgbZoneCommand`
   already uses `CommandCode::RgbControl = 0x21` and is built via `HidReportBuilder`
   (`hid_reports.rs:141-215`).
2. ~~Override `set_color` and `set_zone_colors` in `Apex3Tkl`~~ — **not needed**: `Apex3Tkl` delegates
   to `GenericKeyboard::set_color`/`set_zone_colors`, which already send `0x21` with bitmask `0xFF`
   and the `R G B`×N layout (`keyboards/mod.rs:521-540`). No separate override required unless
   hardware retest shows the delegated path is wrong for this SKU specifically.
3. **Still open:** reconcile the zone count (code sends 9 via `zone_count_for_product_id`, OpenRGB
   models 8) — see Research update above and Open Questions §10.1.
4. Confirm whether a separate `0x23` brightness write is needed before/after color for the LEDs
   to be visible at non-zero brightness.
5. Mark anything still unverified with an explicit `experimental` doc comment.

### Validation

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
# On hardware (Apex 3 TKL, PID 0x1622):
ssgg rgb solid "#ff0000"   # red should appear
ssgg rgb solid "#000000"   # off
```

### Success criteria

- `ssgg rgb solid` visibly changes the keyboard on real hardware.
- `GenericKeyboard` behavior for other keyboards is unchanged.
- `cargo test --locked` passes.

### Complexity

⭐⭐ Medium — protocol now documented; small code change, but **final confirmation needs the
Apex 3 TKL hardware** (the OpenRGB layout is a strong lead, not a guarantee for this SKU).

---

## 5. Phase 2 — Apex Pro TKL 2023 key matrix RE + actuation read-back (TODO) · Medium

### Context

All 87 `KeyId → KeyAddress` mappings for `ApexProTkl2023` in `src/devices/key_mapping.rs` are
placeholder data; runtime falls back to `simulate_per_key_with_zones()`. Real per-key RGB and an
actuation read-back command remain unverified. ANSI/ISO layout and unsupported-key handling also
need hardware validation. This is the "Continue Apex Pro TKL 2023 protocol and RGB work" backlog
item.

### Research methods (in order)

1. Binary RE of `SteelSeriesGG107.exe` for key-address lookup tables.
2. USB capture (Wireshark/USBPcap) while SteelSeries GG sets per-key colors; decode byte offsets.
3. Community sources: `AstroSnail/apexctl`, `FrankGrimm/apex7tkl_linux`, `not-jan/apex-tux`.

### Primary files

- `src/devices/key_mapping.rs` — `KeyMappingDatabase::new()` for `ApexProTkl2023`
- `src/devices/keyboards/apex_pro_tkl_2023.rs`
- `src/bin/discover_actuation.rs` — probe actuation firmware commands
- `src/bin/verify_key_mapping.rs` — validate mappings on hardware
- `docs/development/protocol-keyboard.md` — per-key status (Key Addressing section)

### Tasks

1. Extract verified HID codes / matrix addresses for the keys (at least alphanumeric + modifiers).
2. Replace placeholder `KeyAddress::new(...)` calls with verified values.
3. Replace the speculative `0x23` per-key path once the real command is confirmed; keep
   `CommandCode::PerKeyRgb (0x23)` and `Apex2023Direct (0x40)` labeled experimental until then.
4. Use `discover_actuation.rs` to find an actuation read-back command if firmware exposes one.
5. Validate ANSI/ISO differences and unsupported-key handling on hardware.
6. Update `docs/development/protocol-keyboard.md` to reflect verified vs. still-placeholder keys.

### Validation

```bash
cargo build  --locked --features experimental-apex-2023
cargo test   --locked --features experimental-apex-2023
```

### Complexity

⭐⭐⭐ High — gated on RE/capture; code change is small once addresses are known.

---

## 6. Phase 3 — Protocol docs reconciliation (TODO) · Low

### Objective

Make `docs/development/` safe to follow: separate confirmed behavior, placeholder code, and
speculation; remove dead references.

### Primary files

- `docs/development/protocol-keyboard.md`, `devices.md`, `database-schemas.md`

### Tasks

1. Confirm every helper-binary reference still resolves (`discover_actuation`, `verify_key_mapping`,
   `benchmark_rgb_alloc`, `benchmark_fragment`, `sonar_control`); fix any dangling ones.
2. Add `> ⚠️ UNVERIFIED` callouts wherever a command code or address is a placeholder.
3. Cross-link the OpenRGB 8-zone reference (Phase 1) into the Apex 3 TKL notes.

### Success criteria

No dev doc points contributors at a nonexistent tool; confirmed vs. speculative is clearly marked.

### Complexity

⭐ Low — docs only.

### Resolution (2026-08-18)

All three tasks were already satisfied by the current `docs/development/` files — no doc edits
were needed. Only PLAN.md itself was stale: it referenced nonexistent filenames
`APEX_PRO_PROTOCOL.md`, `PROTOCOL_RESEARCH.md`, `KEY_MAPPING_RESEARCH.md`; the real files are
`protocol-keyboard.md`, `devices.md`, `database-schemas.md`. References fixed in §5 and §6 above.
Helper-binary references (`discover_actuation.rs`, `verify_key_mapping.rs`, `benchmark_rgb_alloc.rs`,
`benchmark_fragment.rs`, `sonar_control.rs`) all resolve to real files in `src/bin/`. `⚠️ UNVERIFIED`
callouts and the OpenRGB cross-link already exist in `protocol-keyboard.md`.

**Flag for follow-up (not resolved here):** `devices.md`/`protocol-keyboard.md` mark `0x40`
(`Apex2023Direct`) and `0x2D` (`ActuationControl`) as `✅ Confirmed` against connected hardware
(Apex Pro TKL 2023, dated 2026-05-26/27), while `.claude/rules/experimental-protocol.md` and
`CLAUDE.md`'s active backlog still list both as Experimental/unconfirmed. This is a status-promotion
decision the rule file requires an explicit commit for (device model + date) — left to the user
rather than auto-promoted here.

---

## 7. Phase 4 — Apex Pro capability accuracy cleanup (TODO) · Low

### Objective

Ensure placeholder per-key support is never presented as verified. Accuracy/maintainability only —
no runtime behavior change.

### Primary files

- `src/devices/hid_reports.rs`, `src/devices/key_mapping.rs`,
  `src/devices/keyboards/mod.rs`, `src/devices/keyboards/apex_pro_tkl_2023.rs`

### Tasks

1. Find every place placeholder command codes (`PerKeyRgb (0x23)`, `Apex2023Direct (0x40)`,
   `ActuationControl (0x2D)`) are described/logged without an `⚠️ experimental` qualifier.
2. Add qualifiers in doc comments, log messages, or CLI help text.
3. Do not change runtime behavior.

### Validation

```bash
cargo test --locked
cargo test --locked --features experimental-apex-2023
```

### Complexity

⭐ Low — comments/strings only.

### Resolution (2026-08-18)

Already satisfied — no code change needed. `CommandCode::PerKeyRgb`, `Apex2023Direct`, and
`ActuationControl` in `src/devices/hid_reports.rs` all carry doc-comment qualifiers (`placeholder`,
`experimental`, or `EXPERIMENTAL`) and their `Display` impl appends `_EXPERIMENTAL` to the printed
name; `apex_pro_tkl_2023.rs` gates every 2023-direct code path behind the `experimental-apex-2023`
feature and names its methods `experimental_*`. No unlabeled placeholder references found.

---

## 8. Phase 5 — `src/main.rs` refactor (deferred) · Low

### Objective

Improve maintainability of the CLI dispatch layer without behavior change.

### Tasks

1. Extract low-churn command families into focused modules.
2. Preserve clap behavior, output, and feature gating exactly.
3. Start only after Phases 1–4 are stable.

### Validation

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

### Complexity

⭐⭐⭐ High — large surface; lowest priority.

---

## 8b. Phase 6 — Windows pollrate status UX (TODO) · Low

### Context

User report: `ssgg pollrate status` on Windows prints, for both mouse and keyboard, "Error:
Device communication error: Poll rate query is not supported by this device's HID driver. USB HID
devices (including SteelSeries keyboards) do not expose poll rate via the Windows HID class IOCTL
interface." This is **already the intended, documented behavior** —
`windows_hid_poll_ioctl` (`src/pollrate.rs`) maps `ERROR_INVALID_FUNCTION` (raw OS error `1`) from
`IOCTL_HID_GET_POLL_FREQUENCY_MSEC` to this explanatory `Error::DeviceCommunication`, because
`hidusb.sys` never implements that IOCTL (only PS/2 `kbdhid.sys` does). Not a protocol bug.

### Primary files

- `src/pollrate.rs` — `windows_hid_poll_ioctl`, `get_poll_rate`
- `src/main.rs` — pollrate status command output formatting

### Tasks

1. Decide UX: keep as a clearly labeled "unsupported on this driver" notice (not phrased as an
   `Error:`), or suppress entirely for devices known to lack the IOCTL.
2. If changed, keep the explanation text — it is accurate and helps users file correct reports.
3. No change to `get_poll_rate` / `set_poll_rate` return semantics without a task specifically
   requesting it.

### Complexity

⭐ Low — messaging/formatting only; root cause already understood and documented in code.

### Resolution (2026-08-18)

Chose "keep as a labeled notice" (not suppress) per task 1, since the explanation text is accurate
and helps users file correct reports (task 2). `cmd_pollrate`'s `Status` arm in `src/main.rs` now
detects the specific "not supported by this device's HID driver" message and prints
`  Mouse/Keyboard: unsupported on this driver — <detail>` instead of `Error: <detail>`; any other
`DeviceCommunication` error still prints as `Error:`. `get_poll_rate`/`set_poll_rate` return
semantics unchanged (task 3). Verified: `cargo build --locked`, `cargo fmt --all -- --check`,
`cargo test --locked --lib pollrate` (4 passed) all clean.

---

## 8c. Phase 7 — Triage `ssgg_bug_report.json` (TODO) · Medium

### Context

TODO.md references a `ssgg_bug_report.json` file to read; it is not present in this repository
(not tracked, not attached to an issue at time of writing).

### Tasks

1. Locate the file (ask the reporting user, or check GitHub issue attachments) before acting.
2. Once available, extract the failure signature and cross-reference against `src/error.rs` and
   the relevant device/module to see if it matches a known issue (e.g. #173, pollrate) or is new.
3. File or update a GitHub issue with the findings; do not speculate about its contents here.

### Complexity

⭐⭐ Medium — blocked on obtaining the file; triage effort depends on its contents.

---

## 8d. Phase 8 — SignalRGB reverse-engineering research (TODO) · Low

### Context

TODO.md asks whether SignalRGB (`https://signalrgb.com`, docs at `https://docs.signalrgb.com`)
can be reverse-engineered for ideas on driving SteelSeries keyboard lighting on Windows 11.
Research-only; no reusable code identified yet.

### Tasks

1. Read `docs.signalrgb.com` for any public plugin/SDK API relevant to third-party device control.
2. Determine whether SignalRGB implements its own SteelSeries protocol or delegates to SteelSeries
   Engine/GG — if the latter, it offers no new protocol information for Phase 1/2.
3. Record findings in `docs/development/` only if concretely reusable; otherwise fold a one-line
   conclusion back into TODO.md's Deferred research section.

### Complexity

⭐ Low — reading/triage; no code change expected unless a concrete protocol lead surfaces.

### Resolution (2026-08-18)

Read `docs.signalrgb.com`'s developer overview, plugins index, and SteelSeries brand-specific
troubleshooting page. SignalRGB does **not** delegate to SteelSeries GG/Engine — it controls devices
directly through its own JS USB/HID plugin framework, and its own docs instruct users to disable GG's
Prism engine to stop it from fighting SignalRGB for control of the same device. No SteelSeries-specific
protocol bytes were in the public docs (would need an actual plugin's source, not the docs site); its
plugin-dev tutorial documents a USB-capture-to-plugin workflow that matches the approach already
planned for Phase 2, confirming methodology but adding no new protocol lead. No `docs/development/`
change was warranted (task 3's bar); conclusion recorded in TODO.md's Deferred research section
instead.

---

## 9. Dependencies

```
Phase 1 (Apex 3 TKL RGB)  ── protocol known; needs hardware to confirm ──┐
Phase 2 (Key matrix RE)   ── needs RE/USB capture session ───────────────┤
Phase 3 (Docs reconcile)  ── unblocked ──────────────────────────────────┤──► Phase 5 (main.rs)
Phase 4 (Capability acc.) ── unblocked ──────────────────────────────────┘
```

| Phase | Blocked by | Reason |
|-------|------------|--------|
| 1 | Apex 3 TKL hardware (confirmation only) | OpenRGB layout is a lead, not SKU-confirmed |
| 2 | RE / USB capture | Key addresses + actuation command unknown |
| 3, 4 | Nothing | Documentation / accuracy only |
| 5 | 1–4 stable | Refactor risk |
| 6 | Nothing | UX-only, root cause already known |
| 7 | Obtaining `ssgg_bug_report.json` | File not present in repo |
| 8 | Nothing | Research/reading only |

---

## 10. Open questions / blockers

1. **Apex 3 TKL zone count:** code registers **9** zones (`mod.rs:581`, `zone_mapping.rs:311`)
   but OpenRGB models it as **8**. Confirm the true count before mapping `set_zone_colors`.
2. **Report ID / framing — resolved by code read (2026-08-18):** `HidDeviceType::Keyboard::
   includes_report_id()` is `true`, so `RgbZoneCommand::serialize` writes a leading `0x00`
   report-ID byte then `[0x21][zone_selector][R G B]×N`, matching the 65-byte (1+64) OpenRGB
   report exactly. No code change needed here; still worth confirming the write actually reaches
   the device on the hidraw path during hardware retest.
3. **Brightness coupling:** still open. No brightness/apply write is sent automatically by
   `set_color`/`set_zone_colors` today. Also note the command-code table mismatch found in the
   research update above: code's `CommandCode::Brightness = 0x22`, but OpenRGB's table (quoted in
   §4) has `0x22 = rainbow wave` / `0x23 = brightness` — untested which byte this SKU actually
   wants for brightness, if a brightness write turns out to be required at all.
4. **Hardware access:** Phases 1 and 2 both need the physical devices for final confirmation; the
   owner has the Apex Pro TKL 2023 and acked checking the OpenRGB reference for #173 (2026-06-15).
5. **Close-out:** #211 and #165 are implemented but still open on GitHub — confirm on hardware and
   close, or leave open pending user reports.

---

## 11. Deferred research (reference only)

- **Open-G-Hub** (`https://github.com/Sharper-Flow/Open-G-Hub`) — defer unless a concrete blocker
  suggests reusable logic.
- Apex protocol: `AstroSnail/apexctl`, `FrankGrimm/apex7tkl_linux`, `not-jan/apex-tux`;
  OpenRGB `SteelSeriesApex8ZoneController` (primary lead for Phase 1).
- Sonar/audio (when Sonar work resumes): `PrzemekkkYT/GGSonarRev`, `wex/sonar-rev`,
  `Mark7888/steelseries-sonar-py`, `codeberg.org/Birbwell/linuxmix`, `Dymstro/nova-chatmix-linux`.
- Research-only: `flozz/rivalcfg`, `llMBQll/OmniLED`.

---

## 12. Suggested next move

Start **Phase 1** — it is the only open code bug, multi-user confirmed, and now has a documented
protocol. Land the `0x21` zone-color path behind hardware confirmation; **Phases 3, 4, 6, 8** are
low-effort, fully unblocked fillers if hardware is unavailable. **Phase 7** needs the bug-report
file obtained first.
