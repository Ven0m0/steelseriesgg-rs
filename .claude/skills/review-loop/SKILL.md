---
name: review-loop
description: Review one change (a branch that will become one PR) with every review agent in .claude/agents/, fix the real findings, and repeat until a full round finds nothing to fix. Use only when the user asks for a review loop; it starts many agents and costs many tokens.
user-invocable: true
disable-model-invocation: true
---

Run the project's review agents against one change in rounds. Fix what they find, then review again, until a full
round with every agent has nothing to fix. Report each round to the user in the user's language.

## Before round 1

1. Find the change: the branch and its base (usually `main`). The scope is `git diff <base>...HEAD` and
   `git log <base>..HEAD`. If the PR text exists outside the repo (for example `../pr/*.md`), note its path.
2. Check out the branch, so agents read the right files. Agents must not switch branches or edit files.
3. Ask the user, if not yet clear:
   - which rule (if any) the change adds or enforces; agents that check that rule may review the whole repo, not
     only the diff (see "Scope");
   - the round limit (default 5 rounds before the final round).

## Agents

Start every agent in `.claude/agents/` in round 1. Do not pre-filter by changed files: an agent with nothing to
check says so. Today these are:

| Agent | Area |
|---|---|
| `protocol-reviewer` | `src/devices/`, HID frames, error handling, labels, `.claude/rules/device-capabilities.md` |
| `gui-reviewer` | `tools/profile-gui/`, `src/gui.rs`, `build.rs` minifier |
| `security-reviewer` | GameSense server, `ssgg-gui` API, device write gates |
| `dependency-auditor` | `Cargo.toml`, `Cargo.lock`, build dependencies |
| `docs-drift-auditor` | `docs/`, `.claude/`, `AGENTS.md`, READMEs, doc comments, the PR text |
| `re-verifier` | hardware status claims against `.claude/rules/experimental-protocol.md` |
| `release-reviewer` | commit messages, PR title and squash message, version bump |

The area is what the review-loop prompt gives each agent; it can be wider than the agent's own file (for example
`docs-drift-auditor` also reads the skills and the PR text). If `.claude/agents/` has more agents than this table,
start them too.

## Scope

Give every agent the same frame in its prompt:

- the repo path, the branch, and "do not switch branches, do not edit";
- the scope: `git diff <base>...HEAD`. Agents criticise only what this change adds or changes;
- exception: an agent that checks a rule this change adds (for example `.claude/rules/device-capabilities.md`)
  reviews the whole repo against that rule, and marks each finding "PR scope" or "rule scope";
- in round 2 and later: the commits with the fixes since its last run, and "do not repeat findings that are fixed";
- "report only real problems with file:line and a fix; say explicitly if nothing needs fixing".

Start the agents of one round in one message, so they run at the same time.

## Each round

1. Wait for every agent of the round. Report each result to the user in a few lines when it arrives.
2. Sort the findings:
   - **Real problem**: a bug, a wrong or overstated claim, a broken rule, a missing test. Fix it.
   - **Decision**: the fix depends on what the user wants (a version bump, a data format, a scope). Ask the user
     with `AskUserQuestion`; do not guess.
   - **Not a problem** or outside the change and outside any rule in scope: note it as a follow-up for the user
     (the project backlog is `PLAN.md`) and do not fix it here.
   - **Contradiction**: two agents ask for opposite things. Stop the loop and ask the user.
3. Fix the real problems on the branch. Keep each fix minimal; do not refactor beyond the finding.
4. Run the checks (all must pass before the next round):
   - `cargo fmt --all -- --check`
   - `cargo clippy --all-targets --locked -- -D warnings`, and with each feature of the CI matrix (`audio`,
     `sonar`, `remap`, `desktop`; see `.github/workflows/ci.yml`, or run the `ci-matrix` skill)
   - `cargo test --locked`
   - `node tools/profile-gui/sync-products.mjs --check` (regenerate without `--check` when Rust tables changed)
   - the `gui-selftest` skill (source and minified selftest) when the editor or `src/gui.rs` changed
5. For every new test, break the code once on purpose and check that the test fails; then restore the code.
6. Commit the fixes with a conventional commit message. Do not push.

## Next rounds

Run the agents that had findings. Also run an agent that had none when the fixes since its last run touch its
area (see the table): a fix can create a new problem in an area that was clean.

Cosmetic findings do not need another run. A cosmetic finding changes no behaviour and no claim: line wrapping,
spelling, word order, or a clearer phrase that says the same thing. Fix them, but if they are an agent's only
findings, do not run that agent again for them, and do not start a new round only because of them. A wording fix
that changes what a text claims (a status, a scope, a device list) is not cosmetic.

## Final round and stop

When a narrow round has nothing to fix, run one final round with every agent on the whole change, to catch silent
regressions. Stop when:

- a full round has nothing to fix, or only cosmetic findings (fix those and stop); or
- agents contradict each other (ask the user); or
- the round limit is reached: tell the user what is still open and ask whether to continue.

When the final rounds only find wording in older, nearby docs, say so and let the user decide whether to run
another full round.

## After the loop

1. Summarise for the user: rounds, what was fixed, decisions taken, what was left for later.
2. Give the user the follow-ups; add them to `PLAN.md` only if the user asks.
3. Do not push or open a PR unless the user asks.
