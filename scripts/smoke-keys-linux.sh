#!/usr/bin/env bash
#
# Press keys in kura on a virtual display through scripts/xrun.sh, three ways,
# and fail unless only the run that did what its script said comes back as a
# result.
#
# From v0.151.0 a `--keys` run that could not do what the script said ends
# `keys: failed` (or unsettled, or closed) and kura exits 3, and from ito
# v0.262.7 (#202) xrun.sh turns any done file whose last line is not
# `keys: done` into a non-zero exit. The Linux lane reads a zero exit as a
# result and anything else as none, so this is the check that the two agree on
# Linux, where the lane runs:
#
#   done     `j<State:s>`         exit 0, last line `keys: done`, s.txt written
#   failed   `j<State:x>k` with   non-zero, `keys: failed`: the window opened,
#            x.txt a folder       the write failed, and the window closed itself
#   refused  `j<Assert:foo>k`     non-zero, `keys: failed`: refused before the
#                                 window, so xrun.sh sees no window
#
# The refused-before-the-window case is also in tests/keys_fail_closed.rs;
# here it is the lane's tool that has to say "no result" for it.
#
#     scripts/smoke-keys-linux.sh [--out DIR]
#
# Needs what scripts/xrun.sh needs (Xvfb, xdotool, xclip, ImageMagick, a
# software Vulkan driver) and a built target/debug/kura. Each case's folder
# (done file, shot, kura.log, xrun.sh's stderr) is left under DIR.

set -u

OUT=""
while [ $# -gt 0 ]; do
  case "$1" in
    --out)
      [ $# -ge 2 ] || { echo "--out needs a folder" >&2; exit 2; }
      OUT="$2"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT" || exit 2
[ -x target/debug/kura ] || { echo "no target/debug/kura; run cargo build first" >&2; exit 2; }
[ -n "$OUT" ] || OUT="$(mktemp -d)"
mkdir -p "$OUT"

# Nothing from the runner's own config or state: a theme, a keymap or a last
# session there would change what the keys do. The same variables as the
# lanes' `isolate=` in scripts/lanes.conf.
export KURA_CONFIG_HOME="$OUT/cfg/kura" YAZI_CONFIG_HOME="$OUT/cfg/yazi" \
  KURA_STATE_HOME="$OUT/state" UCHMK_CONFIG_DIR="$OUT/cfg"
FIX="$OUT/fixtures"
mkdir -p "$FIX"
for f in a.txt b.txt c.txt; do printf '%s\n' "$f" >"$FIX/$f"; done

FAILS=0
fail() { echo "FAIL: $*" >&2; FAILS=$((FAILS + 1)); }

# run NAME KEYS: xrun.sh into $OUT/NAME, its exit code in $code.
run() {
  local name="$1" keys="$2"
  echo "== $name: --keys \"$keys\""
  scripts/xrun.sh "$OUT/$name" "$FIX" --keys "$keys" 2>"$OUT/$name.stderr"
  code=$?
  echo "   xrun.sh exit $code"
  sed 's/^/   stderr: /' "$OUT/$name.stderr"
  [ -e "$OUT/$name/keys.done" ] && sed 's/^/   keys.done: /' "$OUT/$name/keys.done"
}

run done 'j<State:s>'
[ "$code" = 0 ] || fail "done: xrun.sh exit $code, not 0"
[ "$(tail -1 "$OUT/done/keys.done" 2>/dev/null)" = "keys: done" ] || fail "done: keys.done does not end 'keys: done'"
grep -q '^settled: ' "$OUT/done/s.txt" 2>/dev/null || fail "done: s.txt has no 'settled:' line"

# A folder where `<State:x>` would write x.txt. xrun.sh keeps folders when it
# empties a run folder, and `.keys-run` marks the folder as its own.
mkdir -p "$OUT/failed/x.txt"
printf 'an xrun.sh run folder\n' >"$OUT/failed/.keys-run"
run failed 'j<State:x>k'
[ "$code" != 0 ] || fail "failed: xrun.sh exit 0 for a run that could not write <State:x>"
[ "$(head -1 "$OUT/failed/keys.done" 2>/dev/null)" = "keys: failed" ] || fail "failed: keys.done does not start 'keys: failed'"

run refused 'j<Assert:foo>k'
[ "$code" != 0 ] || fail "refused: xrun.sh exit 0 for a script kura refused"
[ "$(head -1 "$OUT/refused/keys.done" 2>/dev/null)" = "keys: failed" ] || fail "refused: keys.done does not start 'keys: failed'"

if [ "$FAILS" -gt 0 ]; then
  echo "smoke-keys-linux: $FAILS check(s) failed; the runs are in $OUT" >&2
  exit 1
fi
echo "smoke-keys-linux: ok (done was a result, failed and refused were not)"
