---
name: promote-protocol
description: Checklist for promoting a protocol command from Experimental (or Placeholder) to Confirmed after a hardware test. Use when the user reports a successful hardware test and wants the status, labels and commit updated.
user-invocable: true
disable-model-invocation: true
allowed-tools: Read Grep Glob Edit Bash
---

Promote one command per run. Promotion needs a test on physical hardware; a capture, a doc or a passing unit test is not enough.

## Gather first (ask if missing)

1. Command or constant (e.g. `CommandCode::Apex2023Direct`, `0x40`).
2. Device model and PID, OS, and the date of the test.
3. What was observed (e.g. "keyboard turned solid red, user-confirmed"), and what was NOT tested (other colours, Linux, other devices).

If the test only covered part of the command's behaviour, promote only that part and keep the rest Experimental in the same table row.

## Steps

1. Find every place the status appears: `rg -n "<symbol>|<0xNN>" src docs .claude AGENTS.md PLAN.md`.
2. Update the row in `.claude/rules/experimental-protocol.md`: status `Confirmed`, plus `- <device> <PID>, <OS>, <YYYY-MM-DD>: <what was observed>`.
3. Remove `[EXPERIMENTAL]` from the doc comments and docs you found in step 1 for exactly this command. Leave labels on anything the test did not cover.
4. Update the matching claim in `docs/development/` and, if listed there, `PLAN.md`. Do not duplicate status into `AGENTS.md`.
5. Run `cargo fmt --all -- --check` and `cargo test --locked`.
6. Propose the commit message and wait for approval before committing: `feat(devices): confirm <command> on <device> (<date>)` with device model and date in the body.

## Never

- Promote on a Frida trace or USBPcap capture alone.
- Promote a write that can change persistent device state (profile upload) without the user's explicit request and a recovery path.
- Edit `CHANGELOG.md` or the release-please manifest.
