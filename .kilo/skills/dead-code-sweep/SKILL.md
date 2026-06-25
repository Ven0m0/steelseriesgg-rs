---
name: dead-code-sweep
description: Use when user wants to Find and remove genuine dead code using deadcode + vulture. Filters known false positives (dataclass fields, protocol methods, dunder hooks). Run before a cleanup commit.
---

# Dead Code Sweep

Finds unreferenced code using two complementary tools and removes confirmed dead code.

## Step 1 - Run scanners

```bash
uv run deadcode src/ 2>&1
uv run vulture src/ --min-confidence 80 2>&1
```

## Step 2 - Filter false positives

Before removing anything, verify each reported symbol is actually unused:

## Check all callers of a symbol

rg -rn 'symbol_name' src/ tests/ scripts/

**Verification workflow for each candidate:**

1. `rg -rn 'ClassName\|method_name\|CONSTANT_NAME' src/ tests/ scripts/ .github/` - if any matches beyond definition, it's live
2. If a class: check if it's a dataclass (`@dataclass`), NamedTuple, or Protocol
3. If a function: check if it's registered as a Textual action or event handler

### Step 3 - Remove confirmed dead code

Only remove when rg confirms zero callers and the symbol has no interface obligation.

After removal, run:

uv run ruff check src/ tests/
uv run pytest --tb=short -q

Both must pass before committing.

### Step 4 - Commit

git add CHANGED_FILES
git commit -m "chore: remove dead code (SYMBOLS_REMOVED)"
