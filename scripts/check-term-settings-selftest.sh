#!/usr/bin/env bash
#
# Does scripts/check-term-settings.sh fail when it should? A harness that can
# only say "ok" proves nothing, so this breaks the run one way at a time and
# passes a case only when the harness fails for the one reason that case
# expects. A different failure, or no failure, fails the case.
#
#     scripts/check-term-settings-selftest.sh [--out DIR] [--good DIR] [--only CASES]
#
#   --out DIR     where each case keeps its folder (default: a new temporary one)
#   --good DIR    the `out` folder of a passing `check-term-settings.sh run`
#                 (its parent holds run-id and nonce). Without it, one good run
#                 is made first, if any selected case needs it.
#   --only CASES  comma-separated case names (default: all)
#
# Cases that run kura (need what the run needs: Xvfb and a built kura):
#   a   the first click goes to a field outside the Shell card      -> <FocusedId:settings.field.shell-args> fails
#   b   the Save of the first variable is never pressed             -> <Id:!settings.shell.env.row.0.name> fails
#   c   the first terminal is not closed                            -> <ReadHas:PaneText=probe ready 2> fails
#   d   KURA_T is left set in kura's environment                    -> probe-2.txt: env KURA_T expected (unset), got hello
#   f   the terminal's shell is /bin/true, not the probe            -> <ReadHas:PaneText=probe ready 1> fails
#   g   the config folder cannot be written                         -> <config write> fails with Permission denied
# Cases that only read a copy of the good folder (no kura, run anywhere):
#   e         the done file names another run
#   stale     the good folder, judged as the result of a new run
#   cfg-missing  config.toml is gone
#   cfg-initial  config.toml is the one the run started with
#   cfg-env      config.toml still has KURA_T in [term.env]
#
# Case g needs a user that cannot write a folder it made unwritable, so CI runs
# this on GitHub's ubuntu runner as the non-root user `runner`; as root the case
# ends the whole script with exit 3 (its precondition failed, not the harness).
# Exit 0 only when every selected case passed.
#
# Why the field in (a): `settings.field.convert.jpeg_quality` is on the second
# card of the "kura" page (Convert), so the wheel back up to the top
# of the page (`<Wheel:90@0.6,0.5>`; the run scrolled the page down to reach
# the Shell card) makes it clickable, it takes text, and it is not in the Shell
# card. The focus check that follows then finds the focus on it, not on the
# arguments field.

set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT" || exit 2
CHECK="$ROOT/scripts/check-term-settings.sh"

OUT="" GOOD="" ONLY=""
while [ $# -gt 0 ]; do
  case "$1" in
    --out) [ $# -ge 2 ] || { echo "--out needs a folder" >&2; exit 2; }; OUT="$2"; shift 2 ;;
    --good) [ $# -ge 2 ] || { echo "--good needs a folder" >&2; exit 2; }; GOOD="$2"; shift 2 ;;
    --only) [ $# -ge 2 ] || { echo "--only needs case names" >&2; exit 2; }; ONLY="$2"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
[ -n "$OUT" ] || OUT="$(mktemp -d)"
mkdir -p "$OUT" || exit 2

FAILS=0 RAN=0
fail() { echo "FAIL: $*" >&2; FAILS=$((FAILS + 1)); }
selected() { [ -z "$ONLY" ] || case ",$ONLY," in *",$1,"*) true ;; *) false ;; esac; }

KEYS="$("$CHECK" keys)"

# full NAME EXPECTED_STEP [run options...] -- text-edited keys come from $edited.
# Runs the harness into $OUT/NAME and judges the done file. The harness must fail.
FULL_CODE=0
full() {
  local name="$1" step="$2"
  shift 2
  local dir="$OUT/$name"
  echo "== $name"
  rm -rf -- "$dir"
  mkdir -p "$dir"
  local args=(--out "$dir/work")
  if [ -n "${edited:-}" ]; then
    printf '%s' "$edited" >"$dir/keys.txt"
    args+=(--keys-file "$dir/keys.txt")
  fi
  "$CHECK" run "${args[@]}" "$@" >"$dir/run.log" 2>&1
  FULL_CODE=$?
  if [ "$FULL_CODE" = 3 ]; then
    cat "$dir/run.log" >&2
    echo "check-term-settings-selftest: case $name: precondition failed, not a harness result" >&2
    exit 3
  fi
  local done_file="$dir/work/out/keys.done"
  [ "$FULL_CODE" != 0 ] || fail "$name: the harness passed"
  if [ -n "$step" ]; then
    grep -qx 'keys: failed' "$done_file" 2>/dev/null || fail "$name: keys.done has no 'keys: failed' line"
    grep -qxF "step: $step" "$done_file" 2>/dev/null || fail "$name: keys.done does not name $step as the step"
  fi
  RAN=$((RAN + 1))
}

# A text edit of the good key script that must change it, or the case means nothing.
edited=""
edit() {
  edited="$2"
  [ "$edited" != "$KEYS" ] || { fail "$1: the edit did not change the key script"; return 1; }
}

if selected a; then
  edit a "${KEYS/'<ClickId:settings.field.shell-args>'/'<Wheel:90@0.6,0.5><ClickId:settings.field.convert.jpeg_quality><FocusedId:settings.field.convert.jpeg_quality>'}" &&
    full a '<FocusedId:settings.field.shell-args>'
fi
if selected b; then
  edit b "${KEYS/'<ClickId:settings.shell.env.save>'/}" && full b '<Id:!settings.shell.env.row.0.name>'
fi
if selected c; then
  edit c "${KEYS/'<C-S-t>'/'<C-t>'}" && full c '<ReadHas:PaneText=probe ready 2>'
fi
edited=""
if selected d; then
  KURA_T=hello full d '' --keep-env KURA_T
  grep -qxF 'FAIL: probe-2.txt: env KURA_T expected (unset), got hello' "$OUT/d/run.log" ||
    fail "d: no 'probe-2.txt: env KURA_T expected (unset), got hello' in $OUT/d/run.log"
fi
if selected f; then
  full f '<ReadHas:PaneText=probe ready 1>' --shell /bin/true
  ls "$OUT"/f/work/out/probe-*.txt >/dev/null 2>&1 && fail "f: a probe file was written, so the probe did run"
fi
if selected g; then
  full g '<config write>' --readonly-config
  why="$(sed -n 's/^why: //p' "$OUT/g/work/out/keys.done" 2>/dev/null | head -1)"
  case "$why" in
    *config.toml*Permission\ denied* | *Permission\ denied*config.toml*) ;;
    *) fail "g: the why line has no config.toml and Permission denied (why: ${why:-none})" ;;
  esac
fi

# ---- the cases that only read a copy of the good folder ----

CHECK_ONLY="e stale cfg-missing cfg-initial cfg-env"
need_good=0
for c in $CHECK_ONLY; do selected "$c" && need_good=1; done
if [ "$need_good" = 1 ]; then
  if [ -z "$GOOD" ]; then
    echo "== good run"
    "$CHECK" run --out "$OUT/good" >"$OUT/good.log" 2>&1 || {
      cat "$OUT/good.log" >&2
      echo "check-term-settings-selftest: the good run did not pass, so nothing can be compared with it" >&2
      exit 1
    }
    GOOD="$OUT/good/out"
  fi
  GOOD="$(cd "$GOOD" && pwd)"
  WORK="$(dirname "$GOOD")"
  RUN_ID="$(cat "$WORK/run-id" 2>/dev/null)"
  NONCE="$(cat "$WORK/nonce" 2>/dev/null)"
  [ -n "$RUN_ID" ] && [ -n "$NONCE" ] || { echo "no run-id or nonce next to $GOOD" >&2; exit 2; }
  "$CHECK" check "$GOOD" "$RUN_ID" "$NONCE" >"$OUT/good-check.log" 2>&1 || {
    cat "$OUT/good-check.log" >&2
    echo "check-term-settings-selftest: the good folder does not pass its own check" >&2
    exit 1
  }
fi

# only NAME RUN_ID NONCE WANT COUNT: check the case's copy; it must fail with
# the line WANT, and with COUNT FAIL lines in all (0 = any number).
only() {
  local name="$1" run_id="$2" nonce="$3" want="$4" count="$5"
  local log="$OUT/$name/check.log"
  "$CHECK" check "$OUT/$name/out" "$run_id" "$nonce" >"$log" 2>&1 && fail "$name: check passed"
  grep -qF -- "FAIL: $want" "$log" || fail "$name: no 'FAIL: $want' in $log"
  if [ "$count" != 0 ]; then
    local n
    n="$(grep -c '^FAIL: ' "$log")"
    [ "$n" = "$count" ] || fail "$name: $n FAIL lines, expected $count (in $log)"
  fi
  RAN=$((RAN + 1))
}

# copy_good NAME: a fresh copy of the good folder at $OUT/NAME/out.
copy_good() {
  echo "== $1"
  rm -rf -- "${OUT:?}/$1"
  mkdir -p "$OUT/$1"
  cp -R -- "$GOOD" "$OUT/$1/out"
}

if selected e; then
  copy_good e
  sed -i "s/^run: .*/run: another-run-id/" "$OUT/e/out/keys.done"
  only e "$RUN_ID" "$NONCE" 'check: run id mismatch (another-run-id)' 1
fi
if selected stale; then
  copy_good stale
  only stale tts-new-run-id nnew-nonce 'check: run id mismatch' 0
fi
if selected cfg-missing; then
  copy_good cfg-missing
  rm -f -- "$OUT/cfg-missing/out/config.toml"
  only cfg-missing "$RUN_ID" "$NONCE" 'config.toml was not there' 1
fi
if selected cfg-initial; then
  copy_good cfg-initial
  printf '[term]\nshell = "x"\nargs = ["--initial"]\n' >"$OUT/cfg-initial/out/config.toml"
  only cfg-initial "$RUN_ID" "$NONCE" "config.toml: term.args expected ['-NoLogo'], got ['--initial']" 1
fi
if selected cfg-env; then
  copy_good cfg-env
  cfg="$OUT/cfg-env/out/config.toml"
  if grep -q '^\[term\.env\]' "$cfg"; then
    sed -i 's/^\[term\.env\]$/[term.env]\nKURA_T = "hello"/' "$cfg"
  else
    printf '\n[term.env]\nKURA_T = "hello"\n' >>"$cfg"
  fi
  only cfg-env "$RUN_ID" "$NONCE" 'config.toml: term.env still has KURA_T' 1
fi

if [ "$RAN" = 0 ]; then
  echo "check-term-settings-selftest: no case ran (--only ${ONLY:-all})" >&2
  exit 2
fi
if [ "$FAILS" -gt 0 ]; then
  echo "check-term-settings-selftest: $FAILS check(s) failed; the cases are in $OUT" >&2
  exit 1
fi
echo "check-term-settings-selftest: ok ($RAN cases failed for the reason they should)"
