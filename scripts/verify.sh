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

# Formatting first: it is the cheapest, and CI's `fmt` job fails on it.
run cargo fmt --all --check

# Then the build, and `--locked`: CI builds that way, and a plain cargo command quietly
# rewrites a stale Cargo.lock, so the commit that bumped Cargo.toml without it
# passed every later step here (v0.78.42 to v0.78.44 went red on main). When
# this fails, run `cargo build`, then commit Cargo.lock with the version bump.
run cargo build -q --locked
run cargo test -q
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
echo "ALL OK: $tests"
