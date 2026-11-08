#!/usr/bin/env bash
#
# Proves that scripts/check-bookmarks.sh fails when it should. It builds a
# synthetic "good" result folder (what a correct run of kura leaves, in the
# format of the state report and of bookmarks.toml), shows that `check`
# accepts it, then breaks copies of it one way at a time and requires the
# exact FAIL line (and the number of FAIL lines) for each. kura is not run,
# so this needs no display; it needs bash and python3 (tomllib).
#
#     scripts/check-bookmarks-selftest.sh [--out DIR] [--only NAME]
#
# Exit 0 and `check-bookmarks-selftest: ok (N cases ...)` when every case
# failed for the reason it should.

set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CHECK="$ROOT/scripts/check-bookmarks.sh"
work="" only_name=""
while [ $# -gt 0 ]; do
  case "$1" in
    --out) work="$2"; shift 2 ;;
    --only) only_name="$2"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
[ -n "$work" ] || work="$(mktemp -d)"
mkdir -p "$work" || exit 2
work="$(cd "$work" && pwd)"

RUN_ID="bm-selftest-1"
NONCE="nselftest-42"
CWD="/tmp/selftest/bm-$NONCE"
EN_S='Bookmark `a` saved'; EN_M='Bookmark moved from `a` to `q`'; EN_A='Already bookmarked as `q`'; EN_D='Bookmark `q` deleted'
JA_S='ブックマーク `a` を保存'; JA_M='ブックマークを `a` から `q` へ移動'; JA_A='すでに `q` にブックマーク済み'
JA_D='ブックマーク `q` を削除'; JA_ALL='ブックマークを全部削除'

CASES=0
fail() { echo "selftest FAIL: $*" >&2; exit 1; }
selected() { [ -z "$only_name" ] || [ "$only_name" = "$1" ]; }

# state FILE CWD OVERLAY TOASTS [PICK] [LANG] [CONFIRM]
state() {
  {
    echo "cwd: $2"
    echo "overlay: $3"
    echo "clock: 12:00, lang ${6:-en}"
    [ -z "${5:-}" ] || echo "pick: $5"
    [ -z "${7:-}" ] || echo "confirm: $7 | Delete everything?"
    echo "toast: x"
    echo "toasts: $4"
  } >"$1"
}

done_file() { printf 'run: %s\nkeys: done\n' "$2" >"$1/keys.done"; }

make_good() {
  local d="$1"
  rm -rf -- "$d"
  mkdir -p "$d/en" "$d/ja" "$d/disk"
  local pick="[q] $CWD (bm-$NONCE)"
  done_file "$d/en" "$RUN_ID-en"; done_file "$d/ja" "$RUN_ID-ja"; done_file "$d/disk" "$RUN_ID-disk"
  state "$d/en/b1.txt" "$CWD" pick "startup | $EN_S | $EN_M" "$pick" en
  state "$d/en/b2.txt" "$CWD" none "startup | $EN_S | $EN_M | $EN_A"
  state "$d/en/b3.txt" "$CWD" pick "startup | $EN_S | $EN_M | $EN_A" "$pick" en
  state "$d/en/b4.txt" "$CWD" none "startup | $EN_S | $EN_M | $EN_A | $EN_D"
  state "$d/en/b5.txt" "$CWD" none "startup | $EN_S | $EN_M | $EN_A | $EN_D | error"
  : >"$d/en/bookmarks.toml"
  local j="$JA_S | $JA_M | $JA_A | $JA_D"
  state "$d/ja/j1.txt" "$CWD" none "$j" "" ja
  state "$d/ja/j2.txt" "$CWD" confirm "$j | $JA_S" "" ja "ブックマークをすべて削除しますか？"
  state "$d/ja/j3.txt" "$CWD" none "$j | $JA_S | $JA_ALL" "" ja
  state "$d/ja/j4.txt" "$CWD" none "$j | $JA_S | $JA_ALL | error" "" ja
  : >"$d/ja/bookmarks.toml"
  state "$d/disk/d1.txt" "$CWD" none "$EN_S | $EN_M | $EN_A"
  printf '[[bookmark]]\nkey = "q"\npath = "%s"\nname = "bm"\n' "$CWD" >"$d/disk/bookmarks.toml"
}

# only NAME RUN_ID NONCE WANT COUNT MUTATOR: break a copy of the good folder
# with the mutator (called with the copy's path), check it, and require exit
# 1, the exact FAIL line, and COUNT FAIL lines (0 = any number).
only() {
  local name="$1" run_id="$2" nonce="$3" want="$4" count="$5" mut="$6"
  selected "$name" || return 0
  local d="$work/case-$name" err code n
  rm -rf -- "$d"; cp -R "$work/good" "$d"
  "$mut" "$d"
  err="$("$CHECK" check "$d" "$run_id" "$nonce" 2>&1 >/dev/null)"; code=$?
  [ "$code" = 1 ] || fail "$name: expected exit 1, got $code ($err)"
  printf '%s\n' "$err" | grep -F -x -q -- "FAIL: $want" || fail "$name: no line 'FAIL: $want' in: $err"
  n="$(printf '%s\n' "$err" | grep -c '^FAIL: ')"
  [ "$count" = 0 ] || [ "$n" = "$count" ] || fail "$name: expected $count FAIL line(s), got $n: $err"
  echo "   case $name: FAIL: $want"
  CASES=$((CASES + 1))
}

keep() { :; }
sub() { sed -i "$1" "$2"; }

m_wrong_toast() { sub 's/Already bookmarked as `q`/Bookmark `q` saved/' "$1/en/b2.txt"; }
m_toast_missing_ja() { sub "s/ | $JA_ALL//" "$1/ja/j3.txt"; }
m_list_twice() { sub "s|^pick: .*|pick: [a] $CWD (bm) \| [q] $CWD (bm)|" "$1/en/b1.txt"; }
m_empty_list_open() { sub 's/^overlay: none/overlay: pick/;s/^toast: x/pick: [q] x/' "$1/ja/j4.txt"; }
m_old_runid() { printf 'run: old-run\nkeys: done\n' >"$1/en/keys.done"; }
m_two_runs() { printf 'run: %s\nrun: %s\nkeys: done\n' "$RUN_ID-ja" "$RUN_ID-ja" >"$1/ja/keys.done"; }
m_not_done() { printf 'run: %s\nkeys: failed\nstep: 3\n' "$RUN_ID-disk" >"$1/disk/keys.done"; }
m_missing_state() { rm -f "$1/en/b3.txt"; }
m_missing_toml() { rm -f "$1/disk/bookmarks.toml"; }
m_two_bookmarks() { printf '[[bookmark]]\nkey = "q"\npath = "%s"\n[[bookmark]]\nkey = "a"\npath = "%s"\n' "$CWD" "$CWD" >"$1/disk/bookmarks.toml"; }
m_wrong_path() { printf '[[bookmark]]\nkey = "q"\npath = "/elsewhere"\n' >"$1/disk/bookmarks.toml"; }
m_wrong_key() { sub 's/key = "q"/key = "a"/' "$1/disk/bookmarks.toml"; }
m_bad_toml() { printf 'this is [not toml\n' >"$1/disk/bookmarks.toml"; }
m_not_deleted() { printf '[[bookmark]]\nkey = "q"\npath = "%s"\n' "$CWD" >"$1/ja/bookmarks.toml"; }
m_old_folder() { sub 's|^cwd: .*|cwd: /tmp/old/bm-nOLD|' "$1/disk/d1.txt"; }
m_ja_not_applied() { sub 's/lang ja/lang en/' "$1/ja/j1.txt"; }
m_no_confirm() { sub 's/^overlay: confirm/overlay: none/' "$1/ja/j2.txt"; }
m_wrong_confirm() { sub 's/^confirm: .*/confirm: Delete all bookmarks? | x/' "$1/ja/j2.txt"; }
m_leftover() { : >"$1/en/x.tmp"; }

# ----------------------------------------------------------- the cases ---

make_good "$work/good"
if selected good; then
  out="$("$CHECK" check "$work/good" "$RUN_ID" "$NONCE" 2>&1)"; code=$?
  [ "$code" = 0 ] && printf '%s\n' "$out" | grep -qx 'check-bookmarks: ok' || fail "the good folder was not accepted (exit $code): $out"
  echo "   case good: accepted"
  CASES=$((CASES + 1))
fi

only wrong-toast "$RUN_ID" "$NONCE" "b2.txt: toasts expected to end with [$EN_S | $EN_M | $EN_A], got [startup | $EN_S | $EN_M | Bookmark \`q\` saved]" 1 m_wrong_toast
only ja-toast-missing "$RUN_ID" "$NONCE" "j3.txt: toasts expected to end with [$JA_S | $JA_M | $JA_A | $JA_D | $JA_S | $JA_ALL], got [$JA_S | $JA_M | $JA_A | $JA_D | $JA_S]" 1 m_toast_missing_ja
only list-twice "$RUN_ID" "$NONCE" "b1.txt: the list has more than one row: [[a] $CWD (bm) | [q] $CWD (bm)]" 2 m_list_twice
only empty-list-open "$RUN_ID" "$NONCE" "j4.txt: overlay expected none, got pick" 2 m_empty_list_open
only old-run-id "$RUN_ID" "$NONCE" "check: run id mismatch (old-run, wanted $RUN_ID-en)" 1 m_old_runid
only run-twice "$RUN_ID" "$NONCE" "check: run id mismatch ($RUN_ID-ja, wanted $RUN_ID-ja)" 1 m_two_runs
only not-done "$RUN_ID" "$NONCE" "disk/keys.done does not end with 'keys: done' (last line: step: 3)" 1 m_not_done
only wrong-nonce "$RUN_ID" "nOTHER-1" "b1.txt: cwd expected to end with bm-nOTHER-1, got $CWD" 0 keep
only missing-state "$RUN_ID" "$NONCE" "b3.txt: missing" 2 m_missing_state
only missing-toml "$RUN_ID" "$NONCE" "disk/bookmarks.toml was not there" 1 m_missing_toml
only two-bookmarks "$RUN_ID" "$NONCE" "disk/bookmarks.toml: expected 1 bookmark(s), got 2 (q=$CWD a=$CWD)" 1 m_two_bookmarks
only wrong-path "$RUN_ID" "$NONCE" "disk/bookmarks.toml: path expected $CWD, got /elsewhere" 1 m_wrong_path
only wrong-key "$RUN_ID" "$NONCE" "disk/bookmarks.toml: key expected q, got a" 1 m_wrong_key
only not-deleted "$RUN_ID" "$NONCE" "ja/bookmarks.toml: expected 0 bookmark(s), got 1 (q=$CWD)" 1 m_not_deleted
only old-folder "$RUN_ID" "$NONCE" "d1.txt: cwd expected to end with bm-$NONCE, got /tmp/old/bm-nOLD" 0 m_old_folder
only ja-not-applied "$RUN_ID" "$NONCE" "ja/j1.txt: the state does not say 'lang ja' (clock: 12:00, lang en)" 1 m_ja_not_applied
only no-confirm "$RUN_ID" "$NONCE" "j2.txt: overlay expected confirm, got none" 1 m_no_confirm
only wrong-confirm "$RUN_ID" "$NONCE" "ja/j2.txt: no confirm titled 'ブックマークをすべて削除しますか？' (read: Delete all bookmarks? | x)" 1 m_wrong_confirm
only leftover "$RUN_ID" "$NONCE" "leftovers: ./en/x.tmp " 1 m_leftover

# bad TOML: the reason text is Python's, so match only the start.
if selected bad-toml; then
  d="$work/case-bad-toml"; rm -rf -- "$d"; cp -R "$work/good" "$d"; m_bad_toml "$d"
  err="$("$CHECK" check "$d" "$RUN_ID" "$NONCE" 2>&1 >/dev/null)"; code=$?
  [ "$code" = 1 ] && printf '%s\n' "$err" | grep -q '^FAIL: bookmarks.toml: not valid TOML (' || fail "bad-toml: $code: $err"
  echo "   case bad-toml: FAIL: bookmarks.toml: not valid TOML (...)"
  CASES=$((CASES + 1))
fi

echo "check-bookmarks-selftest: ok ($CASES cases failed for the reason they should)"
