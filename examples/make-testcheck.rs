//! Write `TESTING-CHECKS.md` (and, with `--lane linux`, `TESTING-LINUX.md`):
//! the manual half of TESTING.md, in Japanese, with a box to tick per check
//! and the commands each section needs.
//!
//! The generator is ito's `ito-testcheck` (shared with tsumugi); this is
//! kura's configuration of it: the paths, the two lanes and their preambles.
//! Ticks survive regeneration, rows a test names are left out, and the
//! Japanese comes from `scripts/testcheck-ja.toml` beside TESTING.md's English.
//!
//!     cargo run --example make-testcheck [-- --lane linux] [--check | --stats]
//!
//! `--check` writes nothing and exits 1 when the file is out of date (what CI
//! runs). The Windows file keeps `[~]` (an appearance row an agent judged from
//! a screenshot, not done until the owner looks); the Linux one keeps `[-]`
//! (the row does not apply on Linux).

use std::fmt::Write as _;

use ito_testcheck::testcheck::{run, Config, Counts, Lane};

const SRC: &str = "TESTING.md";

fn main() {
    run(&Config {
        source: SRC,
        notes: "scripts/testcheck-ja.toml",
        src_dirs: &["src"],
        // The terminal pane's tests live in ito, and name kura's checks.
        dep_crates: &["ito-pane"],
        // The rows the ARM64 machine presses once per release (v0.101.3).
        release_list: Some("TESTING-ARM64.md"),
        command: "cargo run --example make-testcheck",
        lanes: &[
            Lane { name: "windows", out: "TESTING-CHECKS.md", marks: &['~'], setup: true, check_release: true, preamble: windows },
            Lane { name: "linux", out: "TESTING-LINUX.md", marks: &['-'], setup: false, check_release: false, preamble: linux },
        ],
    });
}

fn windows(c: &Counts) -> String {
    let mut out = common(c, false);
    writeln!(
        out,
        "\n## 使い方\n\n\
         1. `kura.exe` と、`scripts\\make-fixtures.ps1` が作るテスト用ファイルを用意する\
         （詳しくは {SRC} の\n   「What you need」）。\n\
         2. 節ごとに「準備」を走らせてから、上から押していく。\n\
         3. 期待どおりなら `[ ]` を `[x]` にする。違ったら `<F12>` で issue を出すか、\
         そのまま書き留める。\n\
         `[~]` は、見た目の行を Agent が画面の画像で判断したもの（証拠の画像は PR と、その実行の\
         `qa-reports/` の報告にある）。同じ画像を見て正しければ `[x]` に変える。\n\n\
         キーの網羅は別ファイル（[TESTING-KEYS.md](TESTING-KEYS.md)）で、\
         こちらは「1 つのキーでは\n確かめられない振る舞い」の側。"
    )
    .unwrap();
    out
}

fn linux(c: &Counts) -> String {
    let mut out = common(c, true);
    writeln!(
        out,
        "\n## 使い方\n\n\
         **このファイルの印は Linux で確かめたもの**で、[TESTING-CHECKS.md](TESTING-CHECKS.md)\
         （Windows 実機）とは別に\n数える。書き方と規則は `.claude/linux-role.md`。\n\n\
         - `[x]`: Linux（X11、Xvfb と CPU 描画）で操作し、期待値を**読めるテキストかファイルの状態**で確かめた。\n\
         - `[-]`: Linux では対象外（UNC、ConPTY、ごみ箱の Windows 側など）。理由は PR に書く。\n\
         - 見た目の行（色、滑らかさ、フォント）は付けない。CPU 描画では実機の代わりにならない。"
    )
    .unwrap();
    out
}

/// What both lanes say first. Only numbers a tick cannot move go here, so a
/// pull request that only ticks never touches it.
fn common(c: &Counts, linux: bool) -> String {
    let mut out = String::new();
    let flag = if linux { " -- --lane linux" } else { "" };
    writeln!(out, "{}", if linux { "# Linux チェックリスト" } else { "# 実機チェックリスト" }).unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "`{SRC}` から `cargo run --example make-testcheck{flag}` で生成している。\
         **正は {SRC}**（英語）で、\nこのファイルはそれを日本語で並べ替えたもの。\
         食い違ったら {SRC} を信じること。各行の\n末尾の *斜体* が {SRC} の原文で、\
         訳はその手前にある。\n\n\
         **チェック（`[x]`）だけは手で書いてよく、生成し直しても残る。**\
         それ以外を書き換えても次の\n生成で消える。"
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "押すものは {} 件（{SRC} の全 {} 件のうち、`cargo test` が見ている {} 件は外してある）。\n\
         **済みの数はこのファイルに書かない**（チェックを付けた PR が毎回ここを書き換え、並んだ PR が\n\
         必ず衝突していた）。節ごとの進み具合は `cargo run --example make-testcheck -- {lane_arg}--stats` で出る。\n\
         `[~]` は済みに数えない（持ち主が同じ画像を見て `[x]` にするまで）。",
        c.manual,
        c.all,
        c.all - c.manual,
        lane_arg = if linux { "--lane linux " } else { "" },
    )
    .unwrap();
    if c.untranslated > 0 {
        writeln!(out, "\n未訳 {} 件は原文のまま `〔未訳〕` を付けて出している。", c.untranslated).unwrap();
    }
    out
}
