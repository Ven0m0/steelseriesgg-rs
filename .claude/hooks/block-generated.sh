#!/usr/bin/env bash
# PreToolUse (Edit|Write): block hand edits to files owned by cargo or release-please.
f=$(jq -r '.tool_input.file_path // empty')
p=${f//\\//}
case "$p" in
    */Cargo.lock | */CHANGELOG.md | */.release-please-manifest.json)
        echo "BLOCKED: $p is generated (cargo / release-please). Run cargo or let release-please update it." >&2
        exit 2
        ;;
esac
