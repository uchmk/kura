#!/usr/bin/env bash
#
# TESTING rows 70.1 to 70.4, 70.7 (part) and 70.8: converting images with `c i`.
# Done through the real window on a virtual display (scripts/xrun.sh), one run
# of kura per scenario, each with a folder, settings and state of its own:
#
#   jpeg      png + transparent png + png with a Japanese name + jpg + txt -> JPEG      (70.1, 70.2 white background, 70.4 text skipped, 70.7 Japanese name)
#   bg        transparent png, config alpha_background = "#00ff00" -> JPEG              (70.2 background colour)
#   png       jpg -> PNG: same size, same pixel                                         (70.2)
#   webp      jpg -> WebP (lossless): same size, same pixel                             (70.2)
#   conflict  png with a jpg already beside it: the dialog, Cancel                      (70.3)
#   errors    two png and one with mode 000 -> JPEG: an error row, the rest converted   (70.4, chmod 000 stands in for the Windows exclusive open)
#   readonly  png in a read-only folder -> JPEG: an error row, nothing left behind      (70.7)
#   archive   zip opened with `l`, then `c i`: refused                                  (70.8)
#   menu      S-F10 (the context menu) lists Convert                                    (70.8)
#
#     scripts/check-convert.sh run [--out DIR]
#     scripts/check-convert.sh check OUT_DIR RUN_ID NONCE
#     scripts/check-convert.sh keys NAME
#
# run    Starts kura once per scenario on the work folder DIR (default: a new
#        temporary one). Results: DIR/out/NAME with the state files, keys.done,
#        before.manifest / after.manifest (hash of every file in the folder before
#        and after) and files/ (a copy of the folder under test afterwards).
#        The run id and the nonce: DIR/run-id and DIR/nonce. Each run's id is
#        RUN_ID-NAME and the folder under test is DIR/cv-NAME-NONCE.
#        Needs target/debug/kura, ImageMagick (`magick`, or `convert` + `identify`
#        of ImageMagick 6) and what scripts/xrun.sh needs.
# check  Reads only OUT_DIR and judges it; never runs kura. A result is only this
#        run's when each done file carries its run id and each cwd carries the
#        nonce, so a folder left from an earlier run never passes.
# keys   Prints the key script of one scenario, on one line.
#
# Exit 0 and `check-convert: ok` only when everything holds. Exit 3 when the
# run is root: chmod does not stop root, so the `errors` and `readonly` scenarios
# cannot be measured (they are left out, and nothing says they passed). Every
# failed check prints its own `FAIL: ...` line on stderr; the self-test
# (scripts/check-convert-selftest.sh) matches on those lines, so keep them stable.
#
# Toasts are read from the `toasts:` line of the state report (all toasts of the
# run, oldest first, joined with " | "), not from the screen.

set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PY="${PYTHON:-python3}"

SCENARIOS="jpeg bg png webp conflict errors readonly archive menu"

PICK_JPEG='<C-a>ci<State:p1>JPEG<Enter><Wait:3000><State:c1>'
PICK_PNG='<C-a>ci<State:p1>PNG<Enter><Wait:3000><State:c1>'
PICK_WEBP='<C-a>ci<State:p1>WebP<Enter><Wait:3000><State:c1>'

# keys_for NAME: the key script of one scenario (one line; `ci` is `c` `i`, the
# letters after it filter the list of formats and <Enter> takes the first row).
keys_for() {
  case "$1" in
    jpeg | bg | errors | readonly) printf '%s' "<Size:1200x900>$PICK_JPEG<Quit>" ;;
    png) printf '%s' "<Size:1200x900>$PICK_PNG<Quit>" ;;
    webp) printf '%s' "<Size:1200x900>$PICK_WEBP<Quit>" ;;
    conflict) printf '%s' "<Size:1200x900><C-a>ci<State:p1>JPEG<Enter><Wait:1500><State:k1>q<Wait:1500><State:k2><Quit>" ;;
    archive) printf '%s' "<Size:1200x900>l<Wait:1500><State:a1>ci<Wait:500><State:a2><Quit>" ;;
    menu) printf '%s' "<Size:1200x900><S-F10><State:m1><Esc><Quit>" ;;
    *) return 1 ;;
  esac
}

# The refusal inside an archive starts like this (src/app.rs archive_refusal).
EN_ARCHIVE='Inside an archive:'

# ------------------------------------------------------------ ImageMagick ---

IM=""
ID=""
find_im() {
  if command -v magick >/dev/null 2>&1; then
    IM="magick"
    ID="magick identify"
  elif command -v convert >/dev/null 2>&1 && convert -version 2>/dev/null | grep -q ImageMagick; then
    IM="convert"
    ID="identify"
  fi
}

# dims FILE: "W H" of the first frame; fails when it does not decode.
dims() { $ID -format '%w %h' "${1}[0]" 2>/dev/null; }

# pixel FILE: "R G B" at 5,5 (8 bits).
pixel() {
  $IM "${1}[0]" -colorspace sRGB -type TrueColor -crop 1x1+5+5 +repage -depth 8 txt:- 2>/dev/null |
    sed -n 's/^[^(]*(\([0-9]*\), *\([0-9]*\), *\([0-9]*\).*/\1 \2 \3/p' | head -1
}

# near "R G B" "R G B" TOL: every channel within TOL.
near() {
  local -a a b
  read -r -a a <<<"$1"
  read -r -a b <<<"$2"
  [ "${#a[@]}" -eq 3 ] && [ "${#b[@]}" -eq 3 ] || return 1
  local i d
  for i in 0 1 2; do
    d=$((a[i] - b[i]))
    [ "${d#-}" -le "$3" ] || return 1
  done
}

sum_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -- "$1" | cut -d' ' -f1
  else
    shasum -a 256 -- "$1" | cut -d' ' -f1
  fi
}

# manifest DIR: "hash<TAB>./path" per file, sorted by path.
manifest() {
  (
    cd "$1" || exit 1
    find . -type f | LC_ALL=C sort | while IFS= read -r f; do
      printf '%s\t%s\n' "$(sum_of "$f")" "$f"
    done
  )
}

# ---------------------------------------------------------------- check ---

FAILS=0
UNMEASURED=0
fail() { echo "FAIL: $*" >&2; FAILS=$((FAILS + 1)); }

field() { sed -n "s/^$2: //p" "$1" | head -1; }

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

# state_ok DIR NAME NONCE STATE...: each state file is there and was taken in this run's folder.
state_ok() {
  local dir="$1" name="$2" nonce="$3" s got
  shift 3
  for s in "$@"; do
    if [ ! -f "$dir/$s.txt" ]; then
      fail "$name/$s.txt: missing"
      continue
    fi
    got="$(field "$dir/$s.txt" cwd)"
    case "$got" in
      *"cv-$name-$nonce"*) ;;
      *) fail "$name/$s.txt: cwd expected to contain cv-$name-$nonce, got ${got:-none}" ;;
    esac
  done
}

overlay_is() {
  local f="$1" want="$2" got
  [ -f "$f" ] || return
  got="$(field "$f" overlay)"
  [ "$got" = "$want" ] || fail "$(basename "$(dirname "$f")")/$(basename "$f"): overlay expected $want, got ${got:-none}"
}

# last_toast FILE: the newest toast.
last_toast() {
  local t
  t="$(field "$1" toasts)"
  printf '%s' "${t##* | }"
}

# converted_toast FILE N SKIPPED: the newest toast is "N image(s) converted: A → B (x%); SKIPPED skipped".
converted_toast() {
  local f="$1" n="$2" s="$3" t re
  [ -f "$f" ] || return
  t="$(last_toast "$f")"
  re="^$n image\\(s\\) converted: .+ → .+ \\(.+\\); $s skipped\$"
  if ! [[ "$t" =~ $re ]]; then
    fail "$(basename "$(dirname "$f")")/$(basename "$f"): last toast expected '$n image(s) converted: ... ; $s skipped', got [${t:-none}]"
  else
    echo "   read: $(basename "$(dirname "$f")"): $t"
  fi
}

# task_line FILE STATE FILES: the task list has one row that ends with " STATE FILES/FILES files".
task_line() {
  local f="$1" st="$2" n="$3" l
  [ -f "$f" ] || return
  l="$(grep '^task: ' "$f" | tail -1)"
  case "$l" in
    *" $st $n/$n files") ;;
    *) fail "$(basename "$(dirname "$f")")/$(basename "$f"): task row expected to end with '$st $n/$n files', got [${l:-none}]" ;;
  esac
}

# toasts_contain FILE TEXT: some toast of the run has the text.
toasts_contain() {
  local f="$1" want="$2" t
  [ -f "$f" ] || return
  t="$(field "$f" toasts)"
  case "$t" in
    *"$want"*) echo "   read: $(basename "$(dirname "$f")"): toasts [$t]" ;;
    *) fail "$(basename "$(dirname "$f")")/$(basename "$f"): no toast with [$want], toasts were [${t:-none}]" ;;
  esac
}

# files_ok DIR NAME NEW...: the folder after the run holds every original byte for byte,
# exactly the files NEW (names, relative) besides, and no temporary file.
files_ok() {
  local dir="$1" name="$2" fdir="$1/files" man="$1/before.manifest" want got h p now tmp
  shift 2
  if [ ! -d "$fdir" ] || [ ! -f "$man" ]; then
    fail "$name: files/ or before.manifest is missing in the result"
    return
  fi
  while IFS=$'\t' read -r h p; do
    [ -n "$p" ] || continue
    if [ ! -f "$fdir/$p" ]; then
      fail "$name: original $p is gone"
      continue
    fi
    now="$(sum_of "$fdir/$p")"
    [ "$now" = "$h" ] || fail "$name: original $p was changed"
  done <"$man"
  want="$(printf './%s\n' "$@" | LC_ALL=C sort | tr '\n' ' ')"
  [ "$#" -gt 0 ] || want=""
  got="$(cd "$fdir" && find . -type f | LC_ALL=C sort | while IFS= read -r p; do
    grep -qxF -- "$p" <(cut -f2 "$man") || printf '%s\n' "$p"
  done | tr '\n' ' ')"
  [ "$got" = "$want" ] || fail "$name: new files expected [$want], got [$got]"
  tmp="$(cd "$fdir" && find . \( -name '.kura-convert-*' -o -name '*.tmp' \) | tr '\n' ' ')"
  [ -z "$tmp" ] || fail "$name: leftover temporary file(s): $tmp"
}

# decodes_as DIR NAME FILE WIDTH HEIGHT: the output decodes and has the size of its source.
decodes_as() {
  local f="$1/files/$3" name="$2" got
  [ -f "$f" ] || return
  got="$(dims "$f")"
  if [ -z "$got" ]; then
    fail "$name: $3 does not decode ($($ID "${f}[0]" 2>&1 | head -1))"
  elif [ "$got" != "$4 $5" ]; then
    fail "$name: $3 is $got, expected $4 $5"
  fi
}

# colour_is DIR NAME FILE "R G B" TOL
colour_is() {
  local f="$1/files/$3" name="$2" got
  [ -f "$f" ] || return
  got="$(pixel "$f")"
  if near "$got" "$4" "$5" 2>/dev/null; then
    echo "   read: $name: $3 pixel [$got], wanted [$4] within $5"
  else
    fail "$name: $3 pixel is [${got:-unreadable}], expected [$4] within $5"
  fi
}

# same_pixel_and_size DIR NAME FROM TO
same_pixel_and_size() {
  local d="$1/files" name="$2" a b
  [ -f "$d/$4" ] || return
  a="$(dims "$d/$3")"
  b="$(dims "$d/$4")"
  if [ -z "$b" ]; then
    fail "$name: $4 does not decode ($($ID "$d/$4[0]" 2>&1 | head -1))"
    return
  fi
  [ "$a" = "$b" ] || fail "$name: $4 is $b, the source $3 is $a"
  a="$(pixel "$d/$3")"
  b="$(pixel "$d/$4")"
  [ -n "$b" ] && [ "$a" = "$b" ] || fail "$name: $4 pixel is [${b:-unreadable}], the source $3 has [$a] (a lossless output must match)"
}

do_check() {
  local out="$1" run_id="$2" nonce="$3" d
  [ -d "$out" ] || { fail "check: $out is not a folder"; return; }
  find_im
  [ -n "$IM" ] || { echo "check-convert: ImageMagick (magick, or convert + identify) is not installed" >&2; exit 2; }

  # --- 70.1, 70.2 (white), 70.4 (text skipped), 70.7 (Japanese name)
  d="$out/jpeg"
  done_ok "$d" "$run_id-jpeg"
  state_ok "$d" jpeg "$nonce" p1 c1
  overlay_is "$d/p1.txt" pick
  [ ! -f "$d/p1.txt" ] || [ "$(field "$d/p1.txt" pick)" = "JPEG | PNG | WebP (lossless)" ] ||
    fail "jpeg/p1.txt: the format list expected [JPEG | PNG | WebP (lossless)], got [$(field "$d/p1.txt" pick)]"
  overlay_is "$d/c1.txt" none
  converted_toast "$d/c1.txt" 3 2
  task_line "$d/c1.txt" done 5
  files_ok "$d" jpeg a.jpg t.jpg 日本語.jpg
  decodes_as "$d" jpeg a.jpg 64 48
  decodes_as "$d" jpeg t.jpg 64 48
  decodes_as "$d" jpeg 日本語.jpg 32 24
  colour_is "$d" jpeg a.jpg "255 0 0" 24
  colour_is "$d" jpeg t.jpg "255 255 255" 12
  colour_is "$d" jpeg 日本語.jpg "0 0 255" 24

  # --- 70.2: the background of a transparent png is alpha_background
  d="$out/bg"
  done_ok "$d" "$run_id-bg"
  state_ok "$d" bg "$nonce" c1
  converted_toast "$d/c1.txt" 1 0
  files_ok "$d" bg t.jpg
  decodes_as "$d" bg t.jpg 64 48
  colour_is "$d" bg t.jpg "0 255 0" 24

  # --- 70.2: lossless outputs
  d="$out/png"
  done_ok "$d" "$run_id-png"
  state_ok "$d" png "$nonce" c1
  converted_toast "$d/c1.txt" 1 0
  files_ok "$d" png s.png
  same_pixel_and_size "$d" png s.jpg s.png
  d="$out/webp"
  done_ok "$d" "$run_id-webp"
  state_ok "$d" webp "$nonce" c1
  converted_toast "$d/c1.txt" 1 0
  files_ok "$d" webp s.webp
  same_pixel_and_size "$d" webp s.jpg s.webp

  # --- 70.3: an output is there; Cancel leaves it alone
  d="$out/conflict"
  done_ok "$d" "$run_id-conflict"
  state_ok "$d" conflict "$nonce" p1 k1 k2
  overlay_is "$d/k1.txt" confirm
  if [ -f "$d/k1.txt" ] && ! grep -q '^confirm: .*File already exists' "$d/k1.txt"; then
    fail "conflict/k1.txt: no confirm titled 'File already exists' (read: $(field "$d/k1.txt" confirm))"
  elif [ -f "$d/k1.txt" ]; then
    echo "   read: conflict: confirm [$(field "$d/k1.txt" confirm)] keys [$(field "$d/k1.txt" 'confirm keys')]"
  fi
  overlay_is "$d/k2.txt" none
  [ ! -f "$d/k2.txt" ] || echo "   read: conflict after Cancel: toasts [$(field "$d/k2.txt" toasts)]"
  files_ok "$d" conflict

  # --- 70.4 and 70.7: errors. Not measurable as root.
  if [ -f "$out/root" ]; then
    UNMEASURED=1
    echo "   errors and readonly were not run (root: chmod does not stop root)"
  else
    d="$out/errors"
    done_ok "$d" "$run_id-errors"
    state_ok "$d" errors "$nonce" c1
    overlay_is "$d/c1.txt" none
    toasts_contain "$d/c1.txt" "2 image(s) converted"
    toasts_contain "$d/c1.txt" "bad.png"
    task_line "$d/c1.txt" failed 3
    files_ok "$d" errors ok1.jpg ok2.jpg
    decodes_as "$d" errors ok1.jpg 40 30
    decodes_as "$d" errors ok2.jpg 40 30
    d="$out/readonly"
    done_ok "$d" "$run_id-readonly"
    state_ok "$d" readonly "$nonce" c1
    overlay_is "$d/c1.txt" none
    toasts_contain "$d/c1.txt" "r.png"
    task_line "$d/c1.txt" failed 1
    files_ok "$d" readonly
  fi

  # --- 70.8: inside an archive it is refused
  d="$out/archive"
  done_ok "$d" "$run_id-archive"
  state_ok "$d" archive "$nonce" a1 a2
  if [ -f "$d/a1.txt" ] && [ -z "$(field "$d/a1.txt" archive)" ]; then
    fail "archive/a1.txt: kura is not inside the archive (no 'archive:' line; cwd $(field "$d/a1.txt" cwd))"
  fi
  overlay_is "$d/a2.txt" none
  if [ -f "$d/a2.txt" ]; then
    case "$(last_toast "$d/a2.txt")" in
      "$EN_ARCHIVE"*) echo "   read: archive: $(last_toast "$d/a2.txt")" ;;
      *) fail "archive/a2.txt: last toast expected to start with [$EN_ARCHIVE], got [$(last_toast "$d/a2.txt")]" ;;
    esac
  fi
  files_ok "$d" archive

  # --- 70.8: the context menu lists Convert. It is read from the state report's
  # `pick:` line: the menu is painted, so `<Assert:>` cannot see its text.
  d="$out/menu"
  done_ok "$d" "$run_id-menu"
  state_ok "$d" menu "$nonce" m1
  overlay_is "$d/m1.txt" pick
  if [ -f "$d/m1.txt" ]; then
    case "$(field "$d/m1.txt" pick)" in
      *"Convert selected images"*) ;;
      *) fail "menu/m1.txt: the context menu does not list Convert (read: $(field "$d/m1.txt" pick))" ;;
    esac
  fi
  files_ok "$d" menu
}

# ------------------------------------------------------------------ run ---

# setup_folder NAME DIR: make the files of one scenario.
setup_folder() {
  local name="$1" d="$2"
  mkdir -p "$d" || return 1
  case "$name" in
    jpeg)
      $IM -size 64x48 xc:'rgb(255,0,0)' "PNG24:$d/a.png" || return 1
      $IM -size 64x48 xc:none "PNG32:$d/t.png" || return 1
      $IM -size 32x24 xc:'rgb(0,0,255)' "PNG24:$d/日本語.png" || return 1
      $IM -size 20x20 xc:'rgb(0,200,0)' -quality 95 "$d/b.jpg" || return 1
      printf 'not an image\n' >"$d/note.txt"
      ;;
    bg)
      $IM -size 64x48 xc:none "PNG32:$d/t.png" || return 1
      ;;
    png | webp)
      $IM -size 50x40 xc:'rgb(255,128,0)' -quality 95 "$d/s.jpg" || return 1
      ;;
    conflict)
      $IM -size 30x20 xc:'rgb(255,0,0)' "PNG24:$d/c.png" || return 1
      $IM -size 30x20 xc:'rgb(0,0,255)' -quality 95 "$d/c.jpg" || return 1
      ;;
    errors)
      $IM -size 40x30 xc:'rgb(200,0,0)' "PNG24:$d/ok1.png" || return 1
      $IM -size 40x30 xc:'rgb(0,200,0)' "PNG24:$d/ok2.png" || return 1
      $IM -size 40x30 xc:'rgb(0,0,200)' "PNG24:$d/bad.png" || return 1
      ;;
    readonly)
      $IM -size 40x30 xc:'rgb(200,0,0)' "PNG24:$d/r.png" || return 1
      ;;
    archive)
      $IM -size 40x30 xc:'rgb(200,0,0)' "PNG24:$d/inner.png" || return 1
      "$PY" - "$d" <<'PYEOF' || return 1
import sys, zipfile, os
d = sys.argv[1]
with zipfile.ZipFile(os.path.join(d, "arc.zip"), "w") as z:
    z.write(os.path.join(d, "inner.png"), "inner.png")
os.remove(os.path.join(d, "inner.png"))
PYEOF
      ;;
    menu)
      $IM -size 40x30 xc:'rgb(200,0,0)' "PNG24:$d/p.png" || return 1
      ;;
  esac
}

# one_run WORK NAME RUN_ID NONCE
one_run() {
  local work="$1" name="$2" run_id="$3" nonce="$4" out="$work/out/$2" dir="$work/cv-$2-$4"
  mkdir -p "$work/config-$name/kura" "$work/config-$name/yazi" "$work/uchmk-$name" "$out" || exit 2
  chmod -R u+rwX "$dir" 2>/dev/null
  rm -rf -- "$dir" "$out/files" "$work/state-$name" "$work/xrun-$name.stderr"
  find "$out" -mindepth 1 -delete || { echo "could not empty $out" >&2; exit 2; }
  mkdir -p "$work/state-$name"
  printf 'language = "en"\n' >"$work/uchmk-$name/common.toml"
  if [ "$name" = bg ]; then
    printf '[convert]\nalpha_background = "#00ff00"\n' >"$work/config-$name/kura/config.toml"
  else
    : >"$work/config-$name/kura/config.toml"
  fi
  setup_folder "$name" "$dir" || { echo "could not make the files for $name" >&2; exit 2; }
  # Kept out of $out until xrun has run: xrun.sh refuses a folder with files it did not write.
  manifest "$dir" >"$work/before-$name.manifest"
  case "$name" in
    errors) chmod 000 "$dir/bad.png" ;;
    readonly) chmod 555 "$dir" ;;
  esac
  local keys
  keys="$(keys_for "$name")"
  echo "== check-convert: $name run, id $run_id, folder $dir"
  env -u KURA_CONFIG_HOME -u YAZI_CONFIG_HOME -u UCHMK_CONFIG_DIR -u KURA_STATE_HOME \
    KURA_CONFIG_HOME="$work/config-$name/kura" YAZI_CONFIG_HOME="$work/config-$name/yazi" \
    UCHMK_CONFIG_DIR="$work/uchmk-$name" KURA_STATE_HOME="$work/state-$name" \
    KURA_KEYS_RUN_ID="$run_id" \
    scripts/xrun.sh "$out" "$dir" --keys "$keys" 2>"$work/xrun-$name.stderr"
  local code=$?
  chmod -R u+rwX "$dir" 2>/dev/null
  mv -- "$work/before-$name.manifest" "$out/before.manifest" || echo "   could not move the manifest to $out" >&2
  echo "   xrun.sh exit $code"
  sed 's/^/   stderr: /' "$work/xrun-$name.stderr"
  [ -e "$out/keys.done" ] && sed 's/^/   keys.done: /' "$out/keys.done"
  cp -a -- "$dir" "$out/files" || echo "   could not copy $dir to $out/files" >&2
  manifest "$dir" >"$out/after.manifest"
  echo "   folder after: $(cut -f2 "$out/after.manifest" | tr '\n' ' ')"
  [ "$code" = 0 ] || fail "$name: xrun.sh exit $code, not 0"
}

do_run() {
  local work=""
  while [ $# -gt 0 ]; do
    case "$1" in
      --out) [ $# -ge 2 ] || { echo "--out needs a folder" >&2; exit 2; }; work="$2"; shift 2 ;;
      *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
  done

  cd "$ROOT" || exit 2
  [ -x target/debug/kura ] || { echo "no target/debug/kura; run cargo build first" >&2; exit 2; }
  find_im
  [ -n "$IM" ] || { echo "ImageMagick (magick, or convert + identify) is not installed" >&2; exit 2; }
  [ -n "$work" ] || work="$(mktemp -d)"
  mkdir -p "$work/out" || exit 2
  work="$(cd "$work" && pwd)"
  rm -f -- "$work/run-id" "$work/nonce" "$work/out/root"
  local run_id="cv-$(date +%s)-$$-$RANDOM" nonce="n$(date +%s)-$$-$RANDOM" n
  printf '%s\n' "$run_id" >"$work/run-id"
  printf '%s\n' "$nonce" >"$work/nonce"
  echo "   imagemagick: $IM ($($IM -version | head -1))"

  for n in $SCENARIOS; do
    if [ "$(id -u)" = 0 ] && { [ "$n" = errors ] || [ "$n" = readonly ]; }; then
      echo "== check-convert: $n not run (root)"
      printf 'root\n' >"$work/out/root"
      continue
    fi
    one_run "$work" "$n" "$run_id-$n" "$nonce"
  done

  do_check "$work/out" "$run_id" "$nonce"
  finish "$work"
}

finish() {
  if [ "$FAILS" -gt 0 ]; then
    echo "check-convert: $FAILS check(s) failed${1:+; the run is in $1}" >&2
    exit 1
  fi
  if [ "$UNMEASURED" -gt 0 ]; then
    echo "check-convert: ok so far, but 70.4's unreadable file and 70.7's read-only folder cannot be measured as root" >&2
    exit 3
  fi
  echo "check-convert: ok"
}

mode="${1:-}"
[ $# -gt 0 ] && shift
case "$mode" in
  run) do_run "$@" ;;
  check)
    [ $# -eq 3 ] || { echo "usage: $0 check OUT_DIR RUN_ID NONCE" >&2; exit 2; }
    do_check "$1" "$2" "$3"
    finish ""
    ;;
  keys)
    keys_for "${1:-}" || { echo "usage: $0 keys NAME ($SCENARIOS)" >&2; exit 2; }
    echo
    ;;
  *) echo "usage: $0 run [--out DIR] | check OUT_DIR RUN_ID NONCE | keys NAME" >&2; exit 2 ;;
esac
