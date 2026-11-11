---
name: gui-selftest
description: Run the profile editor selftest twice, as CI does - on the source files and on the minified files build.rs writes for ssgg-gui. Use after editing tools/profile-gui/ or src/gui.rs, and after any oxc, lightningcss or minify-html bump.
disable-model-invocation: true
---

Run both selftest jobs from `.github/workflows/ci.yml` (`profile-gui`, `profile-gui-minified`). Stop at the first failure and report its output.

## Steps

**1. Source selftest**

```bash
node tools/profile-gui/selftest.mjs
```

**2. Minified selftest**

`build.rs` writes the minified files into the build-script `out_dir`; the selftest reads them via `SSGG_GUI_DIR`.

```bash
out_dir=$(cargo check --locked --features desktop --message-format=json \
  | jq -r 'select(.reason == "build-script-executed" and (.package_id | test("steelseries"))) | .out_dir' \
  | tail -1)
test -f "$out_dir/app.js"
SSGG_GUI_DIR="$out_dir" node tools/profile-gui/selftest.mjs
```

## Notes

- Step 2 compiles the `desktop` feature. On Linux it needs `libwebkit2gtk-4.1-dev` and `libgtk-3-dev`. If cargo hits bogus E0463/ICE errors on Windows, that is memory exhaustion: retry with `-j 1`.
- Minification wraps internals in IIFEs and keeps only the `globalThis.Ssgg*` exports, so check exports, not top-level function names.
- Report which step ran, pass/fail and the failing check names. Do not claim the minified run passed unless step 2 actually executed.
