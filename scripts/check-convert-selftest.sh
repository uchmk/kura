#!/usr/bin/env bash
#
# Proves that scripts/check-convert.sh fails when it should. It builds a
# synthetic "good" result folder (what a correct run of kura leaves: the
# state reports, keys.done, before.manifest and files/ of each scenario, with
# the outputs made by ImageMagick), shows that `check` accepts it, then breaks
# copies of it one way at a time and requires the exact FAIL line (and the
# number of FAIL lines) for each. kura is not run, so this needs no display;
# it needs bash, ImageMagick (with WebP), sha256sum and python3.
#
#     scripts/check-convert-selftest.sh [--out DIR] [--only NAME]
#
# Exit 0 and `check-convert-selftest: ok (N cases ...)` when every case failed
# for the reason it should. The good folder is "as non-root": the `root`
# marker is absent, so errors and readonly are judged too.

set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CHECK="$ROOT/scripts/check-convert.sh"
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

if command -v magick >/dev/null 2>&1; then IM=magick; else IM=convert; fi
PY="${PYTHON:-python3}"

RUN_ID="cv-selftest-1"
NONCE="nselftest-42"
CASES=0
fail() { echo "selftest FAIL: $*" >&2; exit 1; }
selected() { [ -z "$only_name" ] || [ "$only_name" = "$1" ]; }

sum_of() { sha256sum -- "$1" | cut -d' ' -f1; }
manifest() {
  (cd "$1" && find . -type f | LC_ALL=C sort | while IFS= read -r f; do printf '%s\t%s\n' "$(sum_of "$f")" "$f"; done)
}

CONV='3 image(s) converted: 1.2 KB → 600 B (-50%); 2 skipped'

# state FILE NAME OVERLAY TOASTS [EXTRA LINE...]
state() {
  local f="$1" name="$2" ov="$3" toasts="$4"
  shift 4
  {
    echo "cwd: /tmp/selftest/cv-$name-$NONCE"
    echo "overlay: $ov"
    echo "clock: 12:00, lang en"
    local l
    for l in "$@"; do echo "$l"; done
    echo "toast: x"
    echo "toasts: $toasts"
  } >"$f"
}

done_file() { printf 'run: %s\nkeys: done\n' "$RUN_ID-$2" >"$1/keys.done"; }

# scn OUT NAME: make the folder of one scenario (originals only) in OUT/NAME/files and its manifest.
scn() {
  local d="$1/$2" f="$1/$2/files"
  mkdir -p "$f"
  case "$2" in
    jpeg)
      $IM -size 64x48 xc:'rgb(255,0,0)' "PNG24:$f/a.png"
      $IM -size 64x48 xc:none "PNG32:$f/t.png"
      $IM -size 32x24 xc:'rgb(0,0,255)' "PNG24:$f/日本語.png"
      $IM -size 20x20 xc:'rgb(0,200,0)' -quality 95 "$f/b.jpg"
      printf 'not an image\n' >"$f/note.txt"
      ;;
    bg) $IM -size 64x48 xc:none "PNG32:$f/t.png" ;;
    png | webp) $IM -size 50x40 xc:'rgb(255,128,0)' -quality 95 "$f/s.jpg" ;;
    conflict)
      $IM -size 30x20 xc:'rgb(255,0,0)' "PNG24:$f/c.png"
      $IM -size 30x20 xc:'rgb(0,0,255)' -quality 95 "$f/c.jpg"
      ;;
    errors)
      $IM -size 40x30 xc:'rgb(200,0,0)' "PNG24:$f/ok1.png"
      $IM -size 40x30 xc:'rgb(0,200,0)' "PNG24:$f/ok2.png"
      $IM -size 40x30 xc:'rgb(0,0,200)' "PNG24:$f/bad.png"
      ;;
    readonly) $IM -size 40x30 xc:'rgb(200,0,0)' "PNG24:$f/r.png" ;;
    archive) "$PY" -c "import zipfile,sys; zipfile.ZipFile(sys.argv[1],'w').writestr('inner.png','x')" "$f/arc.zip" ;;
    menu) $IM -size 40x30 xc:'rgb(200,0,0)' "PNG24:$f/p.png" ;;
  esac
  manifest "$f" >"$d/before.manifest"
  done_file "$d" "$2"
}

make_good() {
  local o="$1" f
  rm -rf -- "$o"
  mkdir -p "$o"
  local s
  for s in jpeg bg png webp conflict errors readonly archive menu; do scn "$o" "$s"; done

  # jpeg
  f="$o/jpeg/files"
  $IM "$f/a.png" "$f/a.jpg"
  $IM "$f/t.png" -background white -alpha remove -alpha off "$f/t.jpg"
  $IM "$f/日本語.png" "$f/日本語.jpg"
  state "$o/jpeg/p1.txt" jpeg pick "startup" "pick: JPEG | PNG | WebP (lossless)"
  state "$o/jpeg/c1.txt" jpeg none "startup | $CONV" "task: $CONV done 5/5 files"
  # bg
  f="$o/bg/files"
  $IM "$f/t.png" -background '#00ff00' -alpha remove -alpha off "$f/t.jpg"
  state "$o/bg/c1.txt" bg none "1 image(s) converted: 1 KB → 500 B (-50%); 0 skipped"
  # png, webp
  f="$o/png/files"; $IM "$f/s.jpg" "$f/s.png"
  state "$o/png/c1.txt" png none "1 image(s) converted: 1 KB → 2 KB (+50%); 0 skipped"
  f="$o/webp/files"; $IM "$f/s.jpg" -define webp:lossless=true "$f/s.webp" || fail "this ImageMagick cannot write WebP"
  state "$o/webp/c1.txt" webp none "1 image(s) converted: 1 KB → 2 KB (+50%); 0 skipped"
  # conflict: Cancel leaves everything
  state "$o/conflict/p1.txt" conflict pick "startup" "pick: JPEG | PNG | WebP (lossless)"
  state "$o/conflict/k1.txt" conflict confirm "startup" "confirm: File already exists | src:  x" "confirm keys: o a s S r q"
  state "$o/conflict/k2.txt" conflict none "startup | 0 image(s) converted: 0 B → 0 B (0%); 1 skipped | Convert stopped -- what was already done stays"
  # errors
  f="$o/errors/files"
  $IM "$f/ok1.png" "$f/ok1.jpg"
  $IM "$f/ok2.png" "$f/ok2.jpg"
  state "$o/errors/c1.txt" errors none "startup | 2 image(s) converted: 1 KB → 1 KB (0%); 0 skipped | Convert: bad.png: Permission denied" "task: x failed 3/3 files"
  state "$o/readonly/c1.txt" readonly none "startup | 0 image(s) converted: 0 B → 0 B (0%); 0 skipped | Convert: r.png: Permission denied" "task: x failed 1/1 files"
  # archive
  state "$o/archive/a1.txt" archive none "startup" "archive: /tmp/selftest/cv-archive-$NONCE/arc.zip"
  state "$o/archive/a2.txt" archive none "startup | Inside an archive: not available here yet -- y then p in a folder takes a copy out, Esc leaves" "archive: /tmp/selftest/cv-archive-$NONCE/arc.zip"
  # menu
  state "$o/menu/m1.txt" menu pick "startup" "pick: Run a shell command  ·  shell --interactive (;) | Convert selected images  ·  convert (ci)"
}

# only NAME RUN_ID NONCE WANT COUNT MUTATOR: break a copy of the good folder
# with the mutator (called with the copy's path), check it, and require exit 1,
# the FAIL line (exact; a WANT that starts with ~ is matched as a substring),
# and COUNT FAIL lines (0 = any number).
only() {
  local name="$1" run_id="$2" nonce="$3" want="$4" count="$5" mut="$6"
  selected "$name" || return 0
  local d="$work/case-$name" err code n
  rm -rf -- "$d"; cp -R "$work/good" "$d"
  "$mut" "$d"
  err="$("$CHECK" check "$d" "$run_id" "$nonce" 2>&1 >/dev/null)"; code=$?
  [ "$code" = 1 ] || fail "$name: expected exit 1, got $code ($err)"
  if [ "${want:0:1}" = "~" ]; then
    printf '%s\n' "$err" | grep -F -q -- "FAIL: ${want:1}" || fail "$name: no line containing 'FAIL: ${want:1}' in: $err"
  else
    printf '%s\n' "$err" | grep -F -x -q -- "FAIL: $want" || fail "$name: no line 'FAIL: $want' in: $err"
  fi
  n="$(printf '%s\n' "$err" | grep -c '^FAIL: ')"
  [ "$count" = 0 ] || [ "$n" = "$count" ] || fail "$name: expected $count FAIL line(s), got $n: $err"
  echo "   case $name: FAIL: ${want#\~}"
  CASES=$((CASES + 1))
}

keep() { :; }
sub() { sed -i "$1" "$2"; }

m_wrong_toast() { sub 's/^toasts: .*/toasts: startup | 4 image(s) converted: 1.2 KB → 600 B (-50%); 2 skipped/' "$1/jpeg/c1.txt"; }
m_skipped_wrong() { sub 's/; 2 skipped/; 0 skipped/' "$1/jpeg/c1.txt"; }
m_missing_output() { rm -f "$1/jpeg/files/a.jpg"; }
m_missing_jp_output() { rm -f "$1/jpeg/files/日本語.jpg"; }
m_extra_output() { cp "$1/jpeg/files/a.jpg" "$1/jpeg/files/note.jpg"; }
m_changed_original() { printf 'x' >>"$1/jpeg/files/a.png"; }
m_original_gone() { rm -f "$1/jpeg/files/b.jpg"; }
m_leftover_tmp() { : >"$1/jpeg/files/.kura-convert-1-0.tmp"; }
m_wrong_colour() { $IM -size 64x48 xc:'rgb(255,0,0)' "$1/jpeg/files/t.jpg"; }
m_black_background() { $IM -size 64x48 xc:'rgb(0,0,0)' "$1/jpeg/files/t.jpg"; }
m_bg_ignored() { $IM -size 64x48 xc:white "$1/bg/files/t.jpg"; }
m_wrong_size() { $IM -size 32x24 xc:'rgb(255,0,0)' "$1/jpeg/files/a.jpg"; }
m_undecodable() { printf 'garbage' >"$1/webp/files/s.webp"; }
m_lossy_png() { $IM -size 50x40 xc:'rgb(250,120,10)' "$1/png/files/s.png"; }
m_conflict_overwritten() { $IM -size 30x20 xc:'rgb(9,9,9)' -quality 95 "$1/conflict/files/c.jpg"; }
m_conflict_new() { cp "$1/conflict/files/c.jpg" "$1/conflict/files/c2.jpg"; }
m_conflict_tmp() { : >"$1/conflict/files/.kura-convert-7-0.tmp"; }
m_no_dialog() { sub 's/^overlay: confirm/overlay: none/' "$1/conflict/k1.txt"; }
m_dialog_open() { sub 's/^overlay: none/overlay: confirm/' "$1/conflict/k2.txt"; }
m_no_error_row() { sub 's/ failed / done /' "$1/errors/c1.txt"; }
m_error_toast_missing() { sub 's/ | Convert: bad.png: Permission denied//' "$1/errors/c1.txt"; }
m_bad_converted() { $IM -size 40x30 xc:'rgb(0,0,200)' "PNG24:$1/errors/files/bad.png"; }
m_ro_wrote() { cp "$1/readonly/files/r.png" "$1/readonly/files/r.jpg"; }
m_archive_not_refused() { sub 's/Inside an archive: /Converted: /' "$1/archive/a2.txt"; }
m_not_in_archive() { sub '/^archive: /d' "$1/archive/a1.txt"; }
m_archive_changed() { printf 'x' >>"$1/archive/files/arc.zip"; }
m_menu_fails() { printf 'run: %s\nkeys: failed\nstep: 3\nwhy: no text Convert\n' "$RUN_ID-menu" >"$1/menu/keys.done"; }
m_menu_no_convert() { sed -i 's/^pick: .*/pick: Run a shell command/' "$1/menu/m1.txt"; }
m_menu_closed() { sed -i 's/^overlay: pick/overlay: none/' "$1/menu/m1.txt"; }
m_old_runid() { printf 'run: old-run\nkeys: done\n' >"$1/jpeg/keys.done"; }
m_old_folder() { sub 's|^cwd: .*|cwd: /tmp/old/cv-jpeg-nOLD|' "$1/jpeg/c1.txt"; }
m_bad_list() { sub 's/^pick: .*/pick: JPEG | PNG/' "$1/jpeg/p1.txt"; }
m_root() { printf 'root\n' >"$1/root"; rm -rf "$1/errors" "$1/readonly"; }

# ----------------------------------------------------------- the cases ---

make_good "$work/good" || fail "could not make the good folder"
if selected good; then
  out="$("$CHECK" check "$work/good" "$RUN_ID" "$NONCE" 2>&1)"; code=$?
  [ "$code" = 0 ] && printf '%s\n' "$out" | grep -qx 'check-convert: ok' || fail "the good folder was not accepted (exit $code): $out"
  echo "   case good: accepted"
  CASES=$((CASES + 1))
fi

if selected root; then
  d="$work/case-root"; rm -rf -- "$d"; cp -R "$work/good" "$d"; m_root "$d"
  out="$("$CHECK" check "$d" "$RUN_ID" "$NONCE" 2>&1)"; code=$?
  [ "$code" = 3 ] || fail "root: expected exit 3, got $code: $out"
  echo "   case root: exit 3 (errors and readonly not measured)"
  CASES=$((CASES + 1))
fi

only wrong-toast "$RUN_ID" "$NONCE" "~jpeg/c1.txt: last toast expected '3 image(s) converted: ... ; 2 skipped', got [4 image(s) converted" 1 m_wrong_toast
only skipped-wrong "$RUN_ID" "$NONCE" "~jpeg/c1.txt: last toast expected '3 image(s) converted: ... ; 2 skipped'" 1 m_skipped_wrong
only missing-output "$RUN_ID" "$NONCE" "jpeg: new files expected [./a.jpg ./t.jpg ./日本語.jpg ], got [./t.jpg ./日本語.jpg ]" 1 m_missing_output
only missing-japanese-output "$RUN_ID" "$NONCE" "jpeg: new files expected [./a.jpg ./t.jpg ./日本語.jpg ], got [./a.jpg ./t.jpg ]" 1 m_missing_jp_output
only extra-output "$RUN_ID" "$NONCE" "~jpeg: new files expected [./a.jpg ./t.jpg ./日本語.jpg ], got [./a.jpg ./note.jpg " 1 m_extra_output
only changed-original "$RUN_ID" "$NONCE" "jpeg: original ./a.png was changed" 1 m_changed_original
only original-gone "$RUN_ID" "$NONCE" "jpeg: original ./b.jpg is gone" 1 m_original_gone
only leftover-tmp "$RUN_ID" "$NONCE" "~jpeg: leftover temporary file(s): ./.kura-convert-1-0.tmp" 0 m_leftover_tmp
only wrong-jpeg-colour "$RUN_ID" "$NONCE" "~jpeg: t.jpg pixel is [" 1 m_wrong_colour
only black-background "$RUN_ID" "$NONCE" "~jpeg: t.jpg pixel is [0 0 0], expected [255 255 255] within 12" 1 m_black_background
only background-ignored "$RUN_ID" "$NONCE" "~bg: t.jpg pixel is [255 255 255], expected [0 255 0] within 24" 1 m_bg_ignored
only wrong-size "$RUN_ID" "$NONCE" "jpeg: a.jpg is 32 24, expected 64 48" 1 m_wrong_size
only undecodable "$RUN_ID" "$NONCE" "~webp: s.webp does not decode (" 0 m_undecodable
only lossy-png "$RUN_ID" "$NONCE" "~png: s.png pixel is [250 120 10], the source s.jpg has [" 1 m_lossy_png
only conflict-overwritten "$RUN_ID" "$NONCE" "conflict: original ./c.jpg was changed" 1 m_conflict_overwritten
only conflict-new-file "$RUN_ID" "$NONCE" "conflict: new files expected [], got [./c2.jpg ]" 1 m_conflict_new
only conflict-tmp "$RUN_ID" "$NONCE" "~conflict: leftover temporary file(s): ./.kura-convert-7-0.tmp" 0 m_conflict_tmp
only no-dialog "$RUN_ID" "$NONCE" "conflict/k1.txt: overlay expected confirm, got none" 1 m_no_dialog
only dialog-left-open "$RUN_ID" "$NONCE" "conflict/k2.txt: overlay expected none, got confirm" 1 m_dialog_open
only no-error-row "$RUN_ID" "$NONCE" "~errors/c1.txt: task row expected to end with 'failed 3/3 files', got [task: x done 3/3 files]" 1 m_no_error_row
only error-toast-missing "$RUN_ID" "$NONCE" "~errors/c1.txt: no toast with [bad.png], toasts were [" 1 m_error_toast_missing
only bad-png-converted "$RUN_ID" "$NONCE" "errors: original ./bad.png was changed" 1 m_bad_converted
only readonly-wrote "$RUN_ID" "$NONCE" "readonly: new files expected [], got [./r.jpg ]" 1 m_ro_wrote
only archive-not-refused "$RUN_ID" "$NONCE" "~archive/a2.txt: last toast expected to start with [Inside an archive:], got [Converted: " 1 m_archive_not_refused
only not-in-archive "$RUN_ID" "$NONCE" "~archive/a1.txt: kura is not inside the archive" 1 m_not_in_archive
only archive-changed "$RUN_ID" "$NONCE" "archive: original ./arc.zip was changed" 1 m_archive_changed
only menu-fails "$RUN_ID" "$NONCE" "menu/keys.done does not end with 'keys: done' (last line: why: no text Convert)" 1 m_menu_fails
only menu-no-convert "$RUN_ID" "$NONCE" "menu/m1.txt: the context menu does not list Convert (read: Run a shell command)" 1 m_menu_no_convert
only menu-closed "$RUN_ID" "$NONCE" "menu/m1.txt: overlay expected pick, got none" 1 m_menu_closed
only old-run-id "$RUN_ID" "$NONCE" "check: run id mismatch (old-run, wanted $RUN_ID-jpeg)" 1 m_old_runid
only wrong-nonce "$RUN_ID" "nOTHER-1" "~jpeg/p1.txt: cwd expected to contain cv-jpeg-nOTHER-1, got /tmp/selftest/cv-jpeg-$NONCE" 0 keep
only old-folder "$RUN_ID" "$NONCE" "~jpeg/c1.txt: cwd expected to contain cv-jpeg-$NONCE, got /tmp/old/cv-jpeg-nOLD" 1 m_old_folder
only bad-format-list "$RUN_ID" "$NONCE" "jpeg/p1.txt: the format list expected [JPEG | PNG | WebP (lossless)], got [JPEG | PNG]" 1 m_bad_list

echo "check-convert-selftest: ok ($CASES cases failed for the reason they should)"
