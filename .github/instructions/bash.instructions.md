---
applyTo: "*.{sh,bash,zsh},PKGBUILD"
description: "Optimized bash/shell standards for performance and safety"
---

# Bash/Shell Standards

**Role:** Shell script optimizer — safe codemods, performance, correctness.
**Scope:** `*.sh`, `*.bash`, `*.zsh`, PKGBUILD, shell configs. Exclude: `.git`, `node_modules`, vendor.

## Core Rules

- **Format:** `shfmt -i 2 -bn -ci -ln bash`; max 1 empty line
- **Lint:** `shellcheck --severity=error`; `shellharden --replace` when safe
- **Safety:** `set -euo pipefail`; quote all vars `"${var}"`; no `eval`, `ls` parsing, backticks
- **Perf:** Bash builtins > subshells; arrays/mapfile > loops; native expansion
- **Directives:** User>Rules. Edit>Create. Minimal diff.

## Script Template

```bash
#!/usr/bin/env bash
# shellcheck enable=all shell=bash source-path=SCRIPTDIR
set -euo pipefail
shopt -s nullglob globstar
export LC_ALL=C
IFS=$'\n\t'
s=${BASH_SOURCE[0]}
[[ $s != /* ]] && s=$PWD/$s
cd -P -- "${s%/*}"
has() { command -v -- "$1" &>/dev/null; }

# Cleanup handler
cleanup() {
  [[ -n "${TEMP_DIR:-}" && -d "$TEMP_DIR" ]] && rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

# Vars
readonly SCRIPT_NAME="$(basename "$0")"
TEMP_DIR=""

# Functions
main() {
  TEMP_DIR="$(mktemp -d)"
  # Main logic
}

main "$@"
```

## Performance Optimizations

```bash
# Date (no fork)
date() { local x="${1:-%d/%m/%y-%R}"; printf "%($x)T\n" '-1'; }

# Read file (no cat)
fcat() { printf '%s\n' "$(<${1})"; }

# Sleep without fork (when safe)
sleepy() { read -rt "${1:-1}" -- <> <(:) &>/dev/null || :; }
```

## Transformations

1. **Compact syntax:** `() {` → `(){`; `> file` → `>file`; `2>&1 >/dev/null` → `&>/dev/null`
1. **Modernize:** `[ ... ]` → `[[ ... ]]` (when safe)
1. **Inline:** Functions ≤6 lines, ≤2 call sites, no complex flow
1. **Dedupe:** Extract repeated blocks >3 lines into functions
1. **JSON/YAML:** Use `jq`/`yq` parsers, not grep/awk/sed

## Forbidden Patterns

- `eval` or runtime piping into shell
- Unquoted expansions: `$var` → `"${var}"`
- Parsing `ls` output
- Unnecessary subshells: `$(cat file)` → `"$(<file)"`
- Runtime sourcing external files (prefer standalone)

## Error Handling

- Validate params before execution
- Use `mktemp` for temp files/dirs
- Trap cleanup on EXIT
- Clear error messages with context
- `readonly` for immutable values

## Deliverables

- Unified diff
- Final standalone script(s)
- One-line risk note
- Lint clean (shellcheck + shfmt)

**Pipeline:** Transform → shfmt → shellcheck → shellharden → re-check → PR
