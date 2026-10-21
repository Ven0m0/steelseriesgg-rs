---
name: ci-matrix
description: Run the full local CI matrix for steelseriesgg-rs, matching .github/workflows/ci.yml exactly. Use this whenever the user asks to run CI, check all feature variants, run the full test suite, verify before pushing, or run clippy/fmt/tests across features. Invoke proactively before any push or PR review.
---

Run the full CI matrix locally. Stop at the first failure and report it clearly.

## Steps

Run each command in order. If a command fails, report the error output and stop — do not continue to the next step.

**1. Format check**
```
cargo fmt --all -- --check
```

**2. Clippy — default features**
```
cargo clippy --all-targets --locked -- -D warnings
```

**3. Clippy — sonar feature**
```
cargo clippy --all-targets --locked --features sonar -- -D warnings
```

**4. Clippy — audio feature**
```
cargo clippy --all-targets --locked --features audio -- -D warnings
```
Skip and note as skipped if `libpulse-dev` is not installed (the error will mention `libpulse-binding`).

**4b. Clippy — remap feature** (Linux only; the evdev layer compiles out elsewhere)

```
cargo clippy --all-targets --locked --features remap -- -D warnings
```

**5. Tests — default features**
```
cargo test --locked
```

**6. Tests — sonar feature**
```
cargo test --locked --features sonar
```

**6b. Tests — remap feature**

```
cargo test --locked --features remap
```

**7. Build — default features**
```
cargo build --release --locked
```

**8. Build — sonar feature**
```
cargo build --release --locked --features sonar
```

**9. Build — audio feature**
```
cargo build --release --locked --features audio
```
Skip and note as skipped if `libpulse-dev` is not installed (same condition as step 4).

**10. Build — remap feature**

```
cargo build --release --locked --features remap
```

## Report format

After all steps pass, report:
- Which steps ran and passed
- Any steps skipped and why
- Total time if notable

If a step fails, report:
- Which step failed
- The relevant error lines (trim noise, keep the actionable part)
- What to fix
