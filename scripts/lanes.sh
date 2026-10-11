#!/usr/bin/env bash
# The lanes' shared scripts (merge-lanes.py, claim.sh, push-main.sh, xrun.sh,
# keys.ps1 and the rest that uchmk/ito's scripts/lanes/FILES names) are kept
# in ito since v0.103.0, the same bytes for every app; kura's own values are in
# scripts/lanes.conf. scripts/ holds copies, because the lanes and workflows run
# them from a checkout of kura alone. Edit them in ito, push, move the ito
# `rev` in Cargo.toml (cargo build), then sync.
#
#   scripts/lanes.sh sync    copy them from ito at Cargo.toml's rev into scripts/
#   scripts/lanes.sh check   exit 1 when a copy differs (scripts/verify.sh runs it)
#
# ito is cargo's checkout of that rev. ITO_DIR=../ito takes a local checkout
# instead, to try a change before ito is pushed.

set -euo pipefail
cd "$(dirname "$0")/.."

rev=$(sed -n 's/^ito-pane = .*rev = "\([0-9a-f]*\)".*/\1/p' Cargo.toml)
[ -n "$rev" ] || { echo "lanes: no ito rev in Cargo.toml" >&2; exit 2; }
if [ -z "${ITO_DIR:-}" ]; then
    # cargo's checkout of the rev: <...>/checkouts/ito-<hash>/<short rev>/crates/ito-pane/Cargo.toml
    manifest=$(cargo metadata -q --locked --format-version 1 | python3 -c '
import json, sys
print(next(p["manifest_path"] for p in json.load(sys.stdin)["packages"] if p["name"] == "ito-pane"))')
    ITO_DIR=$(dirname "$(dirname "$(dirname "$manifest")")")
    case $rev in "$(basename "$ITO_DIR")"*) ;; *)
        echo "lanes: cargo's ito checkout $ITO_DIR is not rev $rev; run cargo build" >&2; exit 2 ;;
    esac
fi
src="$ITO_DIR/scripts/lanes"
[ -f "$src/FILES" ] || { echo "lanes: $src/FILES is missing (is ito at $rev older than v0.7.0?)" >&2; exit 2; }
files=$(grep -v '^#' "$src/FILES" | grep .)

case "${1:-}" in
    sync)
        for f in $files; do
            cp "$src/$f" "scripts/$f"
            case $f in *.sh|*.py) chmod +x "scripts/$f" ;; esac
        done
        echo "lanes: $(echo "$files" | wc -l) files from $src" ;;
    check)
        bad=0
        for f in $files; do
            cmp -s "$src/$f" "scripts/$f" || { echo "lanes: scripts/$f differs from ito's ($src/$f)"; bad=1; }
        done
        [ $bad = 0 ] || { echo "lanes: edit them in ito, or run scripts/lanes.sh sync"; exit 1; } ;;
    *)
        echo "usage: scripts/lanes.sh sync|check" >&2; exit 2 ;;
esac
