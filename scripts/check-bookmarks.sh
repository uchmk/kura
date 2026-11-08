#!/usr/bin/env bash
#
# TESTING rows 65.1 to 65.4: the bookmark keys (`bs`, `bb`, `bd`, `bD`), the
# toasts they give in English and in Japanese, and what is stored in
# bookmarks.toml. Done through the real window on a virtual display
# (scripts/xrun.sh), three runs of kura, each with state and settings folders
# of its own:
#
#   en    bsa bsq bb | Esc bsq bb | Esc bdq bb      (65.1, 65.2, English half of 65.3)
#   ja    bsa bsq bsq bdq | bsa bD y bb             (Japanese half of 65.3, 65.4)
#   disk  bsa bsq bsq                               (what bookmarks.toml holds)
#
#     scripts/check-bookmarks.sh run [--out DIR] [--keys-file-en FILE] [--keys-file-ja FILE] [--keys-file-disk FILE]
#     scripts/check-bookmarks.sh check OUT_DIR RUN_ID NONCE
#     scripts/check-bookmarks.sh keys en|ja|disk
#
# run    Starts kura three times on the work folder DIR (default: a new
#        temporary one), the folder under test being DIR/bm-NONCE, so the
#        nonce shows in the bookmarked path. Needs target/debug/kura and what
#        scripts/xrun.sh needs. Results: DIR/out/{en,ja,disk}; settings and
#        state folders: DIR/config-*, DIR/uchmk-*, DIR/state-*; the run id
#        and the nonce: DIR/run-id and DIR/nonce. Each run's id is RUN_ID-en,
#        RUN_ID-ja and RUN_ID-disk.
#        --keys-file-NAME FILE   press this script instead (the self-test breaks copies)
# check  Reads only OUT_DIR and judges it; never runs kura. A result is only
#        this run's when each done file carries its run id and each cwd
#        carries the nonce, so a folder left from an earlier run never passes.
# keys   Prints one key script, on one line.
#
# Exit 0 and `check-bookmarks: ok` only when everything holds. Every failed
# check prints its own `FAIL: ...` line on stderr (and what was read and
# expected); scripts/check-bookmarks-selftest.sh matches on those lines, so
# keep them stable.
#
# Toasts are read from the `toasts:` line of the state report (all toasts of
# the run, oldest first, joined with " | "), not from the screen; the check
# compares the END of that line, because kura may add start-up toasts first.

set -u

# One step per line; joined into one line when pressed. `bsa` is `b` `s` `a`.
# A <State:NAME> writes NAME.txt beside keys.done.
read -r -d '' KEYS_EN <<'EOF'
<Size:1200x900>bsabsqbb<State:b1><Esc>
bsq<State:b2>
bb<State:b3><Esc>
bdq<State:b4>
bb<State:b5>
<Quit>
EOF

read -r -d '' KEYS_JA <<'EOF'
<Size:1200x900>bsabsqbsqbdq<State:j1>
bsabD<State:j2>
y<State:j3>
bb<State:j4>
<Quit>
EOF

read -r -d '' KEYS_DISK <<'EOF'
<Size:1200x900>bsabsqbsq<State:d1>
<Quit>
EOF

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PY="${PYTHON:-python3}"

# The toasts, English and Japanese (src/lang/en.toml, ja.toml).
EN_SAVED='Bookmark `a` saved'
EN_MOVED='Bookmark moved from `a` to `q`'
EN_ALREADY='Already bookmarked as `q`'
EN_DELETED='Bookmark `q` deleted'
JA_SAVED='ブックマーク `a` を保存'
JA_MOVED='ブックマークを `a` から `q` へ移動'
JA_ALREADY='すでに `q` にブックマーク済み'
JA_DELETED='ブックマーク `q` を削除'
JA_ALL='ブックマークを全部削除'
JA_CONFIRM='ブックマークをすべて削除しますか？'

# ---------------------------------------------------------------- check ---

FAILS=0
fail() { echo "FAIL: $*" >&2; FAILS=$((FAILS + 1)); }

# field FILE NAME: the text after "NAME: " on the first line that starts so.
field() { sed -n "s/^$2: //p" "$1" | head -1; }

# toasts_end FILE WANT: the `toasts:` line ends with exactly the toast list WANT.
toasts_end() {
  local f="$1" want="$2" got base
  base="$(basename "$f")"
  [ -f "$f" ] || { fail "$base: missing"; return; }
  got="$(field "$f" toasts)"
  if [ "$got" = "$want" ] || [[ "$got" == *" | $want" ]]; then return; fi
  fail "$base: toasts expected to end with [$want], got [${got:-none}]"
}

# overlay_is FILE WANT
overlay_is() {
  local f="$1" want="$2" got
  [ -f "$f" ] || { fail "$(basename "$f"): missing"; return; }
  got="$(field "$f" overlay)"
  [ "$got" = "$want" ] || fail "$(basename "$f"): overlay expected $want, got ${got:-none}"
}

# one_pick FILE NONCE: the list shows the folder under test exactly once, as [q].
one_pick() {
  local f="$1" nonce="$2" base got
  base="$(basename "$f")"
  [ -f "$f" ] || { fail "$base: missing"; return; }
  overlay_is "$f" pick
  got="$(field "$f" pick)"
  case "$got" in
    *" | "*) fail "$base: the list has more than one row: [$got]"; return ;;
    "") fail "$base: no pick line"; return ;;
  esac
  case "$got" in
    "[q] "*"bm-$nonce"*) ;;
    *) fail "$base: pick expected one row [q] <folder bm-$nonce>, got [$got]" ;;
  esac
}

# no_pick FILE: no list is open and the state carries no rows.
no_pick() {
  local f="$1" got
  [ -f "$f" ] || { fail "$(basename "$f"): missing"; return; }
  overlay_is "$f" none
  got="$(field "$f" pick)"
  [ -z "$got" ] || fail "$(basename "$f"): the list is not empty: [$got]"
}

# cwd_nonce FILE NONCE: the folder kura was in is this run's.
cwd_nonce() {
  local f="$1" nonce="$2" got
  [ -f "$f" ] || return
  got="$(field "$f" cwd)"
  case "$got" in
    *"bm-$nonce") ;;
    *) fail "$(basename "$f"): cwd expected to end with bm-$nonce, got ${got:-none}" ;;
  esac
}

# done_ok DIR RUN_ID: the harness finished, and this folder is this run's.
done_ok() {
  local dir="$1" run_id="$2" done_file="$1/keys.done" runs found
  [ -d "$dir" ] || { fail "check: $dir is not a folder"; return 1; }
  if [ "$(tail -1 "$done_file" 2>/dev/null)" != "keys: done" ]; then
    fail "$(basename "$dir")/keys.done does not end with 'keys: done' (last line: $(tail -1 "$done_file" 2>/dev/null || echo 'file missing'))"
  fi
  runs="$(grep -c '^run: ' "$done_file" 2>/dev/null)"
  found="$(sed -n 's/^run: //p' "$done_file" 2>/dev/null | head -1)"
  if [ "$runs" != 1 ] || [ "$found" != "$run_id" ]; then
    fail "check: run id mismatch (${found:-none}, wanted $run_id)"
  fi
}

# stored FILE: print "N" then one "key<TAB>path" per bookmark; "unreadable<TAB>why" on a bad file.
stored() {
  "$PY" - "$1" <<'PYEOF'
import sys, tomllib
try:
    with open(sys.argv[1], "rb") as f:
        d = tomllib.load(f)
except Exception as e:
    print("unreadable\t%s" % e)
    sys.exit(0)
b = d.get("bookmark", [])
print(len(b))
for x in b:
    print("%s\t%s" % (x.get("key"), x.get("path")))
PYEOF
}

# stored_is FILE WANT_COUNT [WANT_KEY]: bookmarks.toml holds that many bookmarks (and the one is under WANT_KEY).
stored_is() {
  local f="$1" count="$2" key="${3:-}" verdict first k p
  if [ ! -f "$f" ]; then fail "$(basename "$(dirname "$f")")/bookmarks.toml was not there"; return; fi
  verdict="$(stored "$f")" || { fail "bookmarks.toml: could not run $PY to read it"; return; }
  first="$(printf '%s\n' "$verdict" | head -1)"
  case "$first" in
    unreadable*) fail "bookmarks.toml: not valid TOML (${first#unreadable	})"; return ;;
  esac
  if [ "$first" != "$count" ]; then
    fail "$(basename "$(dirname "$f")")/bookmarks.toml: expected $count bookmark(s), got $first ($(printf '%s\n' "$verdict" | awk -F'\t' 'NR>1{printf "%s%s=%s", sep, $1, $2; sep=" "}'))"
    return
  fi
  if [ -n "$key" ]; then
    IFS=$'\t' read -r k p <<<"$(printf '%s\n' "$verdict" | sed -n 2p)"
    [ "$k" = "$key" ] || fail "$(basename "$(dirname "$f")")/bookmarks.toml: key expected $key, got ${k:-none}"
    echo "   read: bookmarks.toml key $k path $p"
    # The path stored is the folder the run was in (its state's cwd line).
    local cwd
    cwd="$(field "$(dirname "$f")/d1.txt" cwd)"
    [ -n "$cwd" ] && [ "$p" = "$cwd" ] || fail "$(basename "$(dirname "$f")")/bookmarks.toml: path expected $cwd, got ${p:-none}"
  fi
}

do_check() {
  local out="$1" run_id="$2" nonce="$3"
  [ -d "$out" ] || { fail "check: $out is not a folder"; return; }
  local en="$out/en" ja="$out/ja" dk="$out/disk" s

  # --- English: 65.1, 65.2 and the English half of 65.3.
  done_ok "$en" "$run_id-en"
  for s in b1 b2 b3 b4 b5; do cwd_nonce "$en/$s.txt" "$nonce"; done
  toasts_end "$en/b1.txt" "$EN_SAVED | $EN_MOVED"
  one_pick "$en/b1.txt" "$nonce"
  toasts_end "$en/b2.txt" "$EN_SAVED | $EN_MOVED | $EN_ALREADY"
  overlay_is "$en/b2.txt" none
  toasts_end "$en/b3.txt" "$EN_SAVED | $EN_MOVED | $EN_ALREADY"
  one_pick "$en/b3.txt" "$nonce"
  # The second look at the list is the same as the first: nothing else changed.
  if [ -f "$en/b1.txt" ] && [ -f "$en/b3.txt" ] && [ "$(field "$en/b1.txt" pick)" != "$(field "$en/b3.txt" pick)" ]; then
    fail "en: the list after 'already bookmarked' is not the list before: [$(field "$en/b1.txt" pick)] then [$(field "$en/b3.txt" pick)]"
  fi
  toasts_end "$en/b4.txt" "$EN_SAVED | $EN_MOVED | $EN_ALREADY | $EN_DELETED"
  overlay_is "$en/b4.txt" none
  no_pick "$en/b5.txt"
  stored_is "$en/bookmarks.toml" 0

  # --- Japanese: the Japanese half of 65.3 and 65.4.
  done_ok "$ja" "$run_id-ja"
  for s in j1 j2 j3 j4; do cwd_nonce "$ja/$s.txt" "$nonce"; done
  toasts_end "$ja/j1.txt" "$JA_SAVED | $JA_MOVED | $JA_ALREADY | $JA_DELETED"
  if [ -f "$ja/j1.txt" ] && ! grep -q '^clock: .*lang ja' "$ja/j1.txt"; then
    fail "ja/j1.txt: the state does not say 'lang ja' (clock: $(field "$ja/j1.txt" clock))"
  fi
  overlay_is "$ja/j2.txt" confirm
  if [ -f "$ja/j2.txt" ] && ! grep -q "^confirm: .*$JA_CONFIRM" "$ja/j2.txt"; then
    fail "ja/j2.txt: no confirm titled '$JA_CONFIRM' (read: $(field "$ja/j2.txt" confirm))"
  fi
  toasts_end "$ja/j3.txt" "$JA_SAVED | $JA_MOVED | $JA_ALREADY | $JA_DELETED | $JA_SAVED | $JA_ALL"
  overlay_is "$ja/j3.txt" none
  no_pick "$ja/j4.txt"
  stored_is "$ja/bookmarks.toml" 0

  # --- On disk: one bookmark per folder, under the last key.
  done_ok "$dk" "$run_id-disk"
  cwd_nonce "$dk/d1.txt" "$nonce"
  stored_is "$dk/bookmarks.toml" 1 q

  local left
  left="$(cd "$out" 2>/dev/null && find . -name '*.tmp' -o -name '*.lock' 2>/dev/null | tr '\n' ' ')"
  [ -z "$left" ] || fail "leftovers: $left"
}

# ------------------------------------------------------------------ run ---

# one_run WORK NAME LANG KEYS RUN_ID NONCE: a kura run with settings and state of its own.
one_run() {
  local work="$1" name="$2" lang="$3" keys="$4" run_id="$5" nonce="$6" out="$work/out/$2"
  mkdir -p "$work/config-$name/kura" "$work/config-$name/yazi" "$work/uchmk-$name" "$out" "$work/state-$name" || exit 2
  find "$out" -mindepth 1 -delete || { echo "could not empty $out" >&2; exit 2; }
  rm -rf -- "$work/state-$name" "$work/xrun-$name.stderr"
  mkdir -p "$work/state-$name"
  # The language the run reads; the text of the toasts depends on it.
  printf 'language = "%s"\n' "$lang" >"$work/uchmk-$name/common.toml"
  : >"$work/config-$name/kura/config.toml"
  keys="$(printf '%s' "$keys" | tr -d '\n')"
  echo "== check-bookmarks: $name run, id $run_id, folder $work/bm-$nonce"
  env -u KURA_CONFIG_HOME -u YAZI_CONFIG_HOME -u UCHMK_CONFIG_DIR -u KURA_STATE_HOME \
    KURA_CONFIG_HOME="$work/config-$name/kura" YAZI_CONFIG_HOME="$work/config-$name/yazi" \
    UCHMK_CONFIG_DIR="$work/uchmk-$name" KURA_STATE_HOME="$work/state-$name" \
    KURA_KEYS_RUN_ID="$run_id" \
    scripts/xrun.sh "$out" "$work/bm-$nonce" --keys "$keys" 2>"$work/xrun-$name.stderr"
  local code=$?
  echo "   xrun.sh exit $code"
  sed 's/^/   stderr: /' "$work/xrun-$name.stderr"
  [ -e "$out/keys.done" ] && sed 's/^/   keys.done: /' "$out/keys.done"
  # Keep the stored bookmarks with the results (xrun.sh empties *.txt etc. at the start only).
  local f
  f="$(find "$work/state-$name" -name bookmarks.toml 2>/dev/null | head -1)"
  if [ -n "$f" ]; then
    cp -- "$f" "$out/bookmarks.toml"
    echo "   bookmarks.toml: $(tr '\n' ' ' <"$f")"
  else
    echo "   bookmarks.toml: not written (state folder holds: $(cd "$work/state-$name" && find . -type f | tr '\n' ' '))"
  fi
  [ "$code" = 0 ] || fail "$name: xrun.sh exit $code, not 0"
}

do_run() {
  local work="" kf_en="" kf_ja="" kf_disk=""
  while [ $# -gt 0 ]; do
    case "$1" in
      --out) [ $# -ge 2 ] || { echo "--out needs a folder" >&2; exit 2; }; work="$2"; shift 2 ;;
      --keys-file-en) [ $# -ge 2 ] || { echo "--keys-file-en needs a file" >&2; exit 2; }; kf_en="$2"; shift 2 ;;
      --keys-file-ja) [ $# -ge 2 ] || { echo "--keys-file-ja needs a file" >&2; exit 2; }; kf_ja="$2"; shift 2 ;;
      --keys-file-disk) [ $# -ge 2 ] || { echo "--keys-file-disk needs a file" >&2; exit 2; }; kf_disk="$2"; shift 2 ;;
      *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
  done

  cd "$ROOT" || exit 2
  [ -x target/debug/kura ] || { echo "no target/debug/kura; run cargo build first" >&2; exit 2; }
  [ -n "$work" ] || work="$(mktemp -d)"
  mkdir -p "$work/out" || exit 2
  work="$(cd "$work" && pwd)"
  rm -f -- "$work/run-id" "$work/nonce"
  local run_id="bm-$(date +%s)-$$-$RANDOM" nonce="n$(date +%s)-$$-$RANDOM"
  printf '%s\n' "$run_id" >"$work/run-id"
  printf '%s\n' "$nonce" >"$work/nonce"
  # The folder under test: its path carries the nonce.
  rm -rf -- "$work/bm-$nonce"
  mkdir -p "$work/bm-$nonce" && printf 'x\n' >"$work/bm-$nonce/a.txt" || exit 2

  local ke="$KEYS_EN" kj="$KEYS_JA" kd="$KEYS_DISK"
  [ -z "$kf_en" ] || ke="$(cat -- "$kf_en")"
  [ -z "$kf_ja" ] || kj="$(cat -- "$kf_ja")"
  [ -z "$kf_disk" ] || kd="$(cat -- "$kf_disk")"

  one_run "$work" en en "$ke" "$run_id-en" "$nonce"
  one_run "$work" ja ja "$kj" "$run_id-ja" "$nonce"
  one_run "$work" disk en "$kd" "$run_id-disk" "$nonce"

  do_check "$work/out" "$run_id" "$nonce"
  if [ "$FAILS" -gt 0 ]; then
    echo "check-bookmarks: $FAILS check(s) failed; the run is in $work" >&2
    exit 1
  fi
  echo "check-bookmarks: ok"
}

mode="${1:-}"
[ $# -gt 0 ] && shift
case "$mode" in
  run) do_run "$@" ;;
  check)
    [ $# -eq 3 ] || { echo "usage: $0 check OUT_DIR RUN_ID NONCE" >&2; exit 2; }
    do_check "$1" "$2" "$3"
    if [ "$FAILS" -gt 0 ]; then
      echo "check-bookmarks: $FAILS check(s) failed" >&2
      exit 1
    fi
    echo "check-bookmarks: ok"
    ;;
  keys)
    case "${1:-}" in
      en) printf '%s' "$KEYS_EN" | tr -d '\n'; echo ;;
      ja) printf '%s' "$KEYS_JA" | tr -d '\n'; echo ;;
      disk) printf '%s' "$KEYS_DISK" | tr -d '\n'; echo ;;
      *) echo "usage: $0 keys en|ja|disk" >&2; exit 2 ;;
    esac
    ;;
  *) echo "usage: $0 run [--out DIR] [--keys-file-en|-ja|-disk FILE] | check OUT_DIR RUN_ID NONCE | keys en|ja|disk" >&2; exit 2 ;;
esac
