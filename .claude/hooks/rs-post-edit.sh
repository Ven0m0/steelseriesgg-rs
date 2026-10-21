#!/usr/bin/env bash
# PostToolUse (Edit|Write): rustfmt the edited .rs file, then flag unwrap/expect in its non-test code.
# Exit 2 feeds stderr back to Claude; exit 0 stays silent.
f=$(jq -r '.tool_input.file_path // empty')
p=${f//\\//}
case "$p" in *.rs) ;; *) exit 0 ;; esac
[ -f "$f" ] || exit 0

if ! out=$(rustfmt --edition 2024 --config-path "${CLAUDE_PROJECT_DIR:-.}/rustfmt.toml" "$f" 2>&1); then
    printf 'rustfmt failed on %s:\n%s\n' "$p" "$out" >&2
    exit 2
fi

# Hard constraint 4: no unwrap/expect in production paths. Everything after the first #[cfg(test)] is test code.
case "$p" in */src/bin/*) exit 0 ;; */src/*) ;; *) exit 0 ;; esac
hits=$(awk '/#\[cfg\(test\)\]/ {exit} /\.unwrap\(\)|\.expect\(/ {print NR": "$0}' "$f")
if [ -n "$hits" ]; then
    printf 'unwrap()/expect() outside #[cfg(test)] in %s (CLAUDE.md hard constraint 4):\n%s\n' "$p" "$hits" >&2
    exit 2
fi
