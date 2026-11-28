#!/usr/bin/env bash
# Every check a push to main must pass, in the order CI would fail them, from
# a Linux machine (a cloud session) or the owner's Windows PC. The first
# failure stops with its output.
#
#   scripts/verify.sh
#
# Until 2026-10-04 this lived in one session's scratch folder, and the steps
# were written out again in each routine's instructions. Before that, a chain
# of commands that only looked at the last one's exit code pushed v0.78.13
# with a test build that did not compile (CLAUDE.md, 自動実行モード).
#
# Clippy covers every OS kura ships for -- Linux, Windows and macOS --
# whatever the host: the host first, then the others. Those are type checks
# only -- they never run that code, so read Windows path strings by eye as
# CLAUDE.md asks. No dependency compiles C for those targets, so plain clippy
# works from any host. Before 2026-10-15 the second clippy was always
# Windows, so a run on the Windows PC never compiled Linux and still said
# ALL OK (yagura's v0.62.0 went red on Linux that way); before 2026-10-16
# macOS was left to CI.
# `cargo fmt --all --check` runs first (since v0.109.15); fix a failure with
# `cargo fmt --all`.

set -o pipefail
cd "$(dirname "$0")/.." || exit 1

host=$(rustc -vV | sed -n 's/^host: //p')
others=
for t in x86_64-unknown-linux-gnu x86_64-pc-windows-msvc aarch64-apple-darwin; do
    [ "$t" = "$host" ] && continue
    others="$others $t"
    rustup target list --installed 2>/dev/null | grep -qx "$t" || rustup target add "$t" >/dev/null 2>&1
done

run() {
    local out rc
    out=$("$@" 2>&1)
    rc=$?
    if [ $rc -ne 0 ]; then
        echo "FAILED: $*"
        echo "$out" | tail -30
        exit 1
    fi
    last=$out
}

# On Windows a test binary has blocked forever inside an OS directory call on
# its own %TEMP% folder (NtQueryDirectoryFile under remove_dir_all, once NtClose
# under FindClose), with flat CPU, and verify.sh then sat silent for hours. The
# whole run takes about 13 s, so scripts/hung-tests.ps1 stops a test exe of
# this run that has run longer than VERIFY_TEST_LIMIT seconds (default 300),
# and the run is repeated once; stopping it and running again has always
# passed.
run_tests_watched() {
    local list list_w attempt out hung rc wrc watch pid wpid hlog limit=${VERIFY_TEST_LIMIT:-300}
    # Build the tests first and take the exact paths of their executables, so
    # the watch can touch nothing else. (This cargo puts them under
    # debug/build/<crate>/<hash>/out, not debug/deps.)
    run cargo test -q --no-run --message-format=json-render-diagnostics
    list=$(mktemp)
    echo "$last" | python3 -c '
import json, sys
for line in sys.stdin:
    try:
        m = json.loads(line)
    except ValueError:
        continue
    if isinstance(m, dict) and m.get("reason") == "compiler-artifact" and m.get("executable") and m["profile"]["test"]:
        print(m["executable"])
' >"$list"
    if [ ! -s "$list" ]; then
        echo "FAILED: cargo test --no-run listed no test executables"
        rm -f "$list"
        exit 1
    fi
    list_w=$(cygpath -w "$list")
    for attempt in 1 2; do
        out=$(mktemp)
        hlog=$(mktemp)
        hung=0
        watch=1
        cargo test -q >"$out" 2>&1 &
        pid=$!
        wpid=$(cat "/proc/$pid/winpid" 2>/dev/null)
        if [ -z "$wpid" ]; then
            echo "verify: no Windows process id for cargo test; it runs unwatched"
            watch=0
        fi
        while kill -0 "$pid" 2>/dev/null; do
            sleep 10
            kill -0 "$pid" 2>/dev/null || break
            [ $watch -eq 1 ] || continue
            powershell.exe -NoProfile -File scripts/hung-tests.ps1 -List "$list_w" -Root "$wpid" -Limit "$limit" >>"$hlog" 2>&1
            wrc=$?
            if [ $wrc -eq 3 ]; then
                hung=1
            elif [ $wrc -ne 0 ]; then
                # Say so now: a dead watch is the silent hang it was meant to end.
                echo "verify: the watch for hung tests failed (exit $wrc); cargo test runs unwatched"
                tr -d '\r' <"$hlog"
                : >"$hlog"
                watch=0
            fi
        done
        wait "$pid"
        rc=$?
        if [ $hung -eq 1 ] && [ $attempt -eq 1 ] && [ $rc -ne 0 ]; then
            echo "verify: cargo test hung; stopped it and running it once more"
            tr -d '\r' <"$hlog"
            rm -f "$out" "$hlog"
            continue
        fi
        if [ $rc -eq 0 ]; then
            [ $hung -eq 1 ] && { echo "verify: a test exe was stopped, yet cargo test passed"; tr -d '\r' <"$hlog"; }
            last=$(cat "$out")
            rm -f "$out" "$hlog" "$list"
            return 0
        fi
        echo "FAILED: cargo test -q"
        tail -30 "$out"
        tr -d '\r' <"$hlog"
        rm -f "$out" "$hlog" "$list"
        exit 1
    done
}

# Formatting first: it is the cheapest, and CI's `fmt` job fails on it.
run cargo fmt --all --check

# Then the build, and `--locked`: CI builds that way, and a plain cargo command quietly
# rewrites a stale Cargo.lock, so the commit that bumped Cargo.toml without it
# passed every later step here (v0.78.42 to v0.78.44 went red on main). When
# this fails, run `cargo build`, then commit Cargo.lock with the version bump.
run cargo build -q --locked
case "$host" in
*windows*) run_tests_watched ;;
*) run cargo test -q ;;
esac
tests=$(echo "$last" | grep -m1 'test result')
run cargo +stable clippy -q --all-targets -- -D warnings
for t in $others; do
    run cargo +stable clippy -q --all-targets --target "$t" -- -D warnings
done
run cargo run -q --example make-testcheck -- --check
run cargo run -q --example make-testcheck -- --lane linux --check
run cargo run -q --example make-keycheck -- --check
# The lanes' shared scripts are copies of ito's at Cargo.toml's rev
# (scripts/lanes.sh); an edit made here instead of in ito fails this.
run scripts/lanes.sh check
# The merge-lanes workflow merges the lanes' pull requests with main's copy
# of this script; a broken copy would stop every merge.
run python3 scripts/merge-lanes.py --self-test
# The Windows lanes run scripts/*.ps1, and one that does not parse stops them
# until a person steps in (scripts/check-ps1.ps1). CI parses them too; this is
# the earlier warning, when the machine has pwsh.
if command -v pwsh >/dev/null 2>&1; then
    run pwsh -NoProfile -File scripts/check-ps1.ps1
fi
# ito first: no crate ito already stands in for, and every crate an app needs
# (CI's `ito-first` job).
run python3 scripts/ito-first.py
# pure Rust: no crate that needs a C toolchain on the 6 targets (CI's `ito-first` job).
run python3 scripts/pure-rust.py
# Known vulnerabilities (RustSec) in the tree, as the Audit workflow runs
# weekly. Warnings (an unmaintained crate deep in the tree) do not fail.
command -v cargo-audit >/dev/null 2>&1 || run cargo install cargo-audit --locked
run cargo audit
echo "ALL OK: $tests"
