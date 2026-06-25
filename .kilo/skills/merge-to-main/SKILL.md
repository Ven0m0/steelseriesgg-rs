---
name: merge-to-main
description: Use when the user wants to commit staged changes, sync with upstream, merge into main, and push to the fork. Triggers on "merge my changes", "push to remote", "commit and push", "merge into main", "sync local changes", "get my changes onto main", "sync with upstream", "pull upstream changes", or any request to integrate staged/branch work into main and push. Pulls from upstream first so the fork always contains the latest game data and CI-generated files, then applies local changes on top and pushes to remote. Never pushes to the upstream repo.
---

# Merge to Main

Full two-way sync: pulls upstream changes, integrates local work, pushes to the fork.

## Step 1: Assess state

```bash
git status
git branch -a
git log --oneline -5
```
Note which files are staged, which branch you're on, and whether a feature branch already exists.

## Step 2: Ensure upstream remote exists

```bash
git remote -v | grep upstream
```
If missing:

```bash
git remote add upstream https://github.com/Ven0m0/example.git
```
## Step 3: Branch staged changes off main (if needed)

**If on `main` with staged changes** → create a feature branch first so the upstream merge lands cleanly on main before the rebase:

```bash
git checkout -b feature/<short-description>
```

Name the branch from the content (3–5 words, kebab-case).

**Already on a feature branch with staged changes** → proceed to commit.

**Feature branch already fully committed** → skip to Step 5.

## Step 4: Commit

```bash
git commit -m "$(cat <<'EOF'
<short imperative summary under 72 chars>

- <group of related changes>
- <group of related changes>
EOF
)"
```
Do NOT use `--no-verify`. Fix hook failures instead:

| Hook | Symptom | Fix |
|------|---------|-----|
| `check-shebang-scripts-are-executable` | `.sh` has shebang but isn't executable | `rtk git add --chmod=+x <file>` |
| `biome check` exits 1 "no files processed" | Staged JSON is in biome's ignore list | Add `--no-errors-on-unmatched` to the `entry:` in `.pre-commit-config.yaml`, then re-stage that file |
| `ruff` lint errors | Python lint violations | `uv run ruff check --fix src/ tests/ scripts/` then re-stage |
| `ruff-format` | Formatting violations | `uv run ruff format src/ tests/ scripts/` then re-stage |

After each fix, re-stage affected files and retry.

## Step 5: Sync main with upstream

```bash
git checkout main
git fetch upstream
git pull --autostash upstream main
```
`--autostash` saves uncommitted main changes, applies the upstream merge, then re-applies them. If the autostash re-apply fails, run `git stash pop` and resolve manually.

### Conflict resolution strategy

| File type | Rule |
|-----------|------|
| Code files (`*.py`, `*.toml`, etc.) | Keep local changes (`git checkout --ours`) or merge manually |
| Docs / config | Merge manually, preferring whichever is more recent |

After resolving: `git add <resolved-files> && git merge --continue`

## Step 6: Sync main with origin too

```bash
git pull -r --autostash origin main
```
Picks up any commits pushed directly to the fork since the last sync.

## Step 7: Merge feature branch

Prefer fast-forward for a linear history:

```bash
git merge --ff-only feature/<branch-name>
```
If histories have diverged:

```bash
git merge feature/<branch-name>
```
Resolve conflicts, then `git merge --continue`.

## Step 8: Push to fork

```bash
git push origin main
```
`origin` must point to `https://github.com/Ven0m0/arc-raiders-autoscrapper`.
Verify with `rtk git remote -v` if unsure. **Never push to `zappybiby/ArcRaiders-AutoScrapper`.**

## Step 9: Clean up feature branch

```bash
git branch -d feature/<branch-name>
git push origin --delete feature/<branch-name>
```
## Step 10: Commit any remaining workflow changes
Files modified during the workflow itself (e.g. `AGENTS.md`, `.pre-commit-config.yaml`) won't be on the feature branch. Commit them directly to main:

```bash
git add <files>
git commit -m "<short message>"
git push origin main
```
