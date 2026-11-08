#!/usr/bin/env bash
#
# TESTING rows 57.2 and 57.3: what the Settings -> Shell card sets (the
# arguments, the variables) really reaches the shell a terminal pane starts,
# and what the card removes is really gone from the next one. Done through
# the real window on a virtual display (scripts/xrun.sh), with a probe
# program standing in for the shell: it writes the argv and the variables it
# was started with into a file.
#
#     scripts/check-term-settings.sh run [--out DIR] [--keys-file FILE] [--shell PATH]
#                                        [--keep-env NAME]... [--readonly-config]
#     scripts/check-term-settings.sh check OUT_DIR RUN_ID NONCE
#     scripts/check-term-settings.sh keys
#
# run    Starts kura on the work folder DIR (default: a new temporary one)
#        with a config of its own, presses the key script below, copies
#        config.toml next to the other results and calls `check`. Needs
#        target/debug/kura and target/debug/examples/term-probe (cargo build
#        --example term-probe) and what scripts/xrun.sh needs. The results
#        are DIR/out, the config folders are DIR/config and DIR/uchmk, the
#        run id and the nonce are DIR/run-id and DIR/nonce.
#        --keys-file FILE   press this script instead (the self-test breaks copies of it)
#        --shell PATH       use this program as the terminal's shell instead of the probe
#        --keep-env NAME    do not unset NAME before the run (the self-test sets KURA_T)
#        --readonly-config  make the config folder unwritable first (exit 3 if that cannot be done, e.g. as root)
# check  Reads only OUT_DIR and judges it; never runs kura. A result is only
#        this run's when its done file carries RUN_ID and the probe files
#        carry NONCE, so a folder left from an earlier run never passes.
# keys   Prints the key script, on one line.
#
# Exit 0 and `check-term-settings: ok` only when everything holds. Every
# failed check prints its own `FAIL: ...` line on stderr; scripts/
# check-term-settings-selftest.sh matches on those lines, so keep them stable.

set -u

# The key script, one step per line. It is joined into one line when pressed
# (a newline is not a key). The Shell card is the last card of the "kura"
# page and lies below the window's bottom edge, and `<ClickId:>` does not
# scroll, so each time the page is opened (or the editor grows) the wheel is
# turned down by 90 lines first; at the end of the page more is harmless.
# `<C-,>` always opens the settings on the General page, so `<C-Tab><C-Tab>`
# goes on to "kura" (General, Theme, kura).
read -r -d '' KEYS <<'EOF'
<Size:1200x900><C-,><C-Tab><C-Tab><Wheel:-30@0.6,0.5><Wheel:-30@0.6,0.5><Wheel:-30@0.6,0.5>
<ClickId:settings.field.shell-args><FocusedId:settings.field.shell-args><Paste:-NoLogo><Enter>
<ValueId:settings.field.shell-args=-NoLogo><State:s1>
<ClickId:settings.shell.env.edit><Id:settings.shell.env.add><Wheel:-30@0.6,0.5>
<ClickId:settings.shell.env.add><Id:settings.shell.env.row.0.name><Wheel:-30@0.6,0.5>
<ClickId:settings.shell.env.row.0.name><FocusedId:settings.shell.env.row.0.name><Paste:KURA_X>
<ClickId:settings.shell.env.cancel><Id:!settings.shell.env.row.0.name>
<ClickId:settings.shell.env.edit><Id:settings.shell.env.add><Wheel:-30@0.6,0.5>
<ClickId:settings.shell.env.add><Id:settings.shell.env.row.0.name><Wheel:-30@0.6,0.5>
<ClickId:settings.shell.env.row.0.name><FocusedId:settings.shell.env.row.0.name><Paste:KURA_T>
<ClickId:settings.shell.env.row.0.value><FocusedId:settings.shell.env.row.0.value><Paste:hello>
<ValueId:settings.shell.env.row.0.name=KURA_T><ValueId:settings.shell.env.row.0.value=hello>
<ClickId:settings.shell.env.save><Id:!settings.shell.env.row.0.name><State:s2>
<Esc><State:t0><C-t><ReadHas:PaneText=probe ready 1><PaneText:p1><C-S-t><State:t1>
<C-,><C-Tab><C-Tab><Wheel:-30@0.6,0.5><Wheel:-30@0.6,0.5><Wheel:-30@0.6,0.5><ClickId:settings.shell.env.edit><ValueId:settings.shell.env.row.0.name=KURA_T><Wheel:-30@0.6,0.5>
<ClickId:settings.shell.env.row.0.remove><Id:!settings.shell.env.row.0.name>
<ClickId:settings.shell.env.save><Id:!settings.shell.env.save><State:s3>
<Esc><C-t><ReadHas:PaneText=probe ready 2><PaneText:p2><C-S-t><Quit>
EOF

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PY="${PYTHON:-python3}"

# ---------------------------------------------------------------- check ---

FAILS=0
fail() { echo "FAIL: $*" >&2; FAILS=$((FAILS + 1)); }

# field FILE NAME: the text after "NAME: " on the first line that starts so.
field() { sed -n "s/^$2: //p" "$1" | head -1; }

# probe_check FILE NONCE ARGV NAME=WANT...: the fields of one probe file.
probe_check() {
  local f="$1" nonce="$2" argv="$3"
  shift 3
  local base got pair want name
  base="$(basename "$f")"
  got="$(field "$f" nonce)"
  [ "$got" = "$nonce" ] || fail "$base: nonce expected $nonce, got ${got:-none}"
  got="$(field "$f" argv)"
  [ "$got" = "$argv" ] || fail "$base: argv expected $argv, got ${got:-none}"
  for pair in "$@"; do
    name="${pair%%=*}"
    want="${pair#*=}"
    got="$(field "$f" "env $name")"
    [ "$got" = "$want" ] || fail "$base: env $name expected $want, got ${got:-none}"
  done
}

do_check() {
  local out="$1" run_id="$2" nonce="$3"
  [ -d "$out" ] || { fail "check: $out is not a folder"; return; }

  local done_file="$out/keys.done"
  if [ "$(tail -1 "$done_file" 2>/dev/null)" != "keys: done" ]; then
    fail "keys.done does not end with 'keys: done' (last line: $(tail -1 "$done_file" 2>/dev/null || echo 'file missing'))"
  fi
  local runs found
  runs="$(grep -c '^run: ' "$done_file" 2>/dev/null)"
  found="$(sed -n 's/^run: //p' "$done_file" 2>/dev/null | head -1)"
  if [ "$runs" != 1 ] || [ "$found" != "$run_id" ]; then
    fail "check: run id mismatch (${found:-none})"
  fi

  local s
  for s in s1 s2 s3; do
    grep -qx 'config writes pending: no' "$out/$s.txt" 2>/dev/null || fail "$s.txt: no 'config writes pending: no'"
    grep -qx 'config write failed: none' "$out/$s.txt" 2>/dev/null || fail "$s.txt: no 'config write failed: none'"
  done
  for s in t0 t1; do
    grep -qx 'pane: closed' "$out/$s.txt" 2>/dev/null || fail "$s.txt: no 'pane: closed'"
  done
  grep -qx 'term env names: \["KURA_T"\]' "$out/s2.txt" 2>/dev/null || fail "s2.txt: no 'term env names: [\"KURA_T\"]'"

  # The config file kura wrote, read as TOML (not grep'ed).
  if [ ! -f "$out/config.toml" ]; then
    fail "config.toml was not there"
  else
    local verdict
    verdict="$("$PY" - "$out/config.toml" <<'PYEOF'
import sys, tomllib
try:
    with open(sys.argv[1], "rb") as f:
        d = tomllib.load(f)
except Exception as e:
    print("unreadable\t%s" % e)
    sys.exit(0)
t = d.get("term", {})
args = t.get("args")
if args != ["-NoLogo"]:
    print("args\t%r" % (args,))
env = t.get("env", {})
left = [n for n in ("KURA_T", "KURA_X") if n in env]
if left:
    print("env\t%s" % ", ".join(left))
PYEOF
    )" || fail "config.toml: could not run $PY to read it"
    local kind rest
    while IFS=$'\t' read -r kind rest; do
      case "$kind" in
        '') ;;
        unreadable) fail "config.toml: not valid TOML ($rest)" ;;
        args) fail "config.toml: term.args expected ['-NoLogo'], got $rest" ;;
        env) fail "config.toml: term.env still has $rest" ;;
      esac
    done <<<"$verdict"
  fi

  # The probe files: exactly one per terminal that was started.
  local files extra
  files="$(cd "$out" 2>/dev/null && ls probe-*.txt 2>/dev/null | tr '\n' ' ')"
  extra="$(cd "$out" 2>/dev/null && ls probe-*.txt 2>/dev/null | grep -v -x -e probe-1.txt -e probe-2.txt | tr '\n' ' ')"
  [ -z "$extra" ] || fail "probe files: expected only probe-1.txt and probe-2.txt, also found $extra"
  if [ ! -f "$out/probe-1.txt" ] || [ ! -f "$out/probe-2.txt" ]; then
    fail "probe files: expected probe-1.txt and probe-2.txt, found ${files:-none}"
  else
    probe_check "$out/probe-1.txt" "$nonce" '["-NoLogo"]' 'KURA_T=hello' 'KURA_X=(unset)'
    probe_check "$out/probe-2.txt" "$nonce" '["-NoLogo"]' 'KURA_T=(unset)'
    local p1 p2
    p1="$(field "$out/probe-1.txt" pid)"
    p2="$(field "$out/probe-2.txt" pid)"
    if [ -z "$p1" ] || [ -z "$p2" ] || [ "$p1" = "$p2" ]; then
      fail "probe pids: expected two different ones, got ${p1:-none} and ${p2:-none}"
    fi
  fi
  local left
  left="$(cd "$out" 2>/dev/null && ls probe-*.tmp probe-*.lock 2>/dev/null | tr '\n' ' ')"
  [ -z "$left" ] || fail "probe leftovers: $left"
}

# ------------------------------------------------------------------ run ---

do_run() {
  local work="" keys_file="" shell="" readonly_cfg=0 keep=()
  while [ $# -gt 0 ]; do
    case "$1" in
      --out) [ $# -ge 2 ] || { echo "--out needs a folder" >&2; exit 2; }; work="$2"; shift 2 ;;
      --keys-file) [ $# -ge 2 ] || { echo "--keys-file needs a file" >&2; exit 2; }; keys_file="$2"; shift 2 ;;
      --shell) [ $# -ge 2 ] || { echo "--shell needs a program" >&2; exit 2; }; shell="$2"; shift 2 ;;
      --keep-env) [ $# -ge 2 ] || { echo "--keep-env needs a name" >&2; exit 2; }; keep+=("$2"); shift 2 ;;
      --readonly-config) readonly_cfg=1; shift ;;
      *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
  done

  cd "$ROOT" || exit 2
  [ -x target/debug/kura ] || { echo "no target/debug/kura; run cargo build first" >&2; exit 2; }
  [ -x target/debug/examples/term-probe ] || { echo "no target/debug/examples/term-probe; run cargo build --example term-probe first" >&2; exit 2; }
  [ -n "$work" ] || work="$(mktemp -d)"
  mkdir -p "$work/config/kura" "$work/config/yazi" "$work/uchmk" "$work/out" "$work/fixtures" || exit 2
  work="$(cd "$work" && pwd)"
  local out="$work/out" cfg="$work/config/kura"
  chmod u+w "$cfg" 2>/dev/null
  # An earlier run's files must not be read as this run's (files only, as
  # xrun.sh does; it also empties keys.done, *.txt, *.png, *.json, *.log).
  find "$out" -maxdepth 1 -type f -delete || { echo "could not empty $out" >&2; exit 2; }
  rm -f -- "$work/xrun.stderr" "$work/run-id" "$work/nonce"
  for f in a.txt b.txt c.txt; do printf '%s\n' "$f" >"$work/fixtures/$f"; done

  local probe="$ROOT/target/debug/examples/term-probe"
  [ -n "$shell" ] || shell="$probe"
  local run_id="tts-$(date +%s)-$$-$RANDOM" nonce="n$(date +%s)-$$-$RANDOM"
  printf '%s\n' "$run_id" >"$work/run-id"
  printf '%s\n' "$nonce" >"$work/nonce"

  # The shell and the arguments the settings page starts from. The run sets
  # the arguments to -NoLogo and the variables to KURA_T, and takes them off.
  printf '[term]\nshell = "%s"\nargs = ["--initial"]\n' "$shell" >"$cfg/config.toml"

  local keys
  if [ -n "$keys_file" ]; then keys="$(cat -- "$keys_file")"; else keys="$KEYS"; fi
  keys="$(printf '%s' "$keys" | tr -d '\n')"

  # Nothing from the runner's own environment, and everything pointing into
  # the work folder. xrun.sh starts kura with `env KURA_KEYS_DONE=... kura`,
  # so what is exported here reaches kura, and kura's pane passes it on to
  # the shell it starts (the probe), apart from what [term.env] changes.
  local unset_args=() n k
  for n in KURA_T KURA_X KURA_TERM_SHELL KURA_TERM_ARGS ITO_PROBE_DIR ITO_PROBE_NONCE ITO_PROBE_ENV UCHMK_CONFIG_DIR; do
    for k in "${keep[@]+"${keep[@]}"}"; do [ "$k" = "$n" ] && continue 2; done
    unset_args+=(-u "$n")
  done

  if [ "$readonly_cfg" = 1 ]; then
    chmod 555 "$cfg"
    if touch "$cfg/.write-probe" 2>/dev/null; then
      rm -f -- "$cfg/.write-probe"
      chmod 755 "$cfg"
      echo "precondition: cannot make the config folder unwritable (running as root?)" >&2
      exit 3
    fi
  fi

  echo "== check-term-settings: run id $run_id, work folder $work"
  env "${unset_args[@]}" \
    KURA_CONFIG_HOME="$cfg" YAZI_CONFIG_HOME="$work/config/yazi" UCHMK_CONFIG_DIR="$work/uchmk" \
    KURA_STATE_HOME="$work/state" ITO_PROBE_DIR="$out" ITO_PROBE_ENV=KURA_T,KURA_X \
    KURA_KEYS_RUN_ID="$run_id" ITO_PROBE_NONCE="$nonce" \
    scripts/xrun.sh "$out" "$work/fixtures" --keys "$keys" 2>"$work/xrun.stderr"
  local code=$?
  echo "   xrun.sh exit $code"
  sed 's/^/   stderr: /' "$work/xrun.stderr"
  [ -e "$out/keys.done" ] && sed 's/^/   keys.done: /' "$out/keys.done"

  [ "$readonly_cfg" = 1 ] && chmod 755 "$cfg"
  if ! cp -- "$cfg/config.toml" "$out/config.toml" 2>/dev/null; then
    fail "config.toml was not there after the run"
  fi

  [ "$code" = 0 ] || fail "xrun.sh exit $code, not 0"
  do_check "$out" "$run_id" "$nonce"

  if [ "$FAILS" -gt 0 ]; then
    echo "check-term-settings: $FAILS check(s) failed; the run is in $work" >&2
    exit 1
  fi
  echo "check-term-settings: ok"
}

mode="${1:-}"
[ $# -gt 0 ] && shift
case "$mode" in
  run) do_run "$@" ;;
  check)
    [ $# -eq 3 ] || { echo "usage: $0 check OUT_DIR RUN_ID NONCE" >&2; exit 2; }
    do_check "$1" "$2" "$3"
    if [ "$FAILS" -gt 0 ]; then
      echo "check-term-settings: $FAILS check(s) failed" >&2
      exit 1
    fi
    echo "check-term-settings: ok"
    ;;
  keys) printf '%s' "$KEYS" | tr -d '\n'; echo ;;
  *) echo "usage: $0 run [--out DIR] [--keys-file FILE] [--shell PATH] [--keep-env NAME]... [--readonly-config] | check OUT_DIR RUN_ID NONCE | keys" >&2; exit 2 ;;
esac
