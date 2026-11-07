---
name: dev-orchestra-kura
description: "kura-specific rules for /dev-orchestra: spec fields, version/CHANGELOG/TESTING handling, commit and push-main.sh. Read together with the global dev-orchestra skill when the repo is C:\\dev\\kura."
---

# dev-orchestra の kura 用メモ

グローバルの `dev-orchestra` に足す、kura だけの決まり。正は `CLAUDE.md` と `docs/claude/`。

## 流れ
1. 設計（設計の判断があれば Codex の設計レビューを回す。枠は十分ある）→ 実装役の選択 → 仕様ファイルを scratchpad に置く。
   Codex は Agent ツールの subagent（`general-purpose`、`model: haiku`）を包みにして、その中で
   `cd /c/dev/kura && codex exec -s read-only -C 'C:/dev/kura' --color never -o "<出力>" "<依頼文>"` を前面で 1 回回す（グローバルの `dev-orchestra` の「起動のしかた」）。
2. 実装は Sonnet のワーカー。**Agent ツールの subagent**（`subagent_type: general-purpose`、`model: sonnet`）として起動し、prompt に仕様ファイルの絶対パスを書く。
   裏の子プロセスや Orca の `worker-start` では起動しない（持ち主は subagent の一覧で見る。Orca は持ち主が頼んだときだけで、そのときはグローバルの `dev-orchestra` の手順）。
   渡したら完了の通知を待つ（ポーリングしない）。直しは SendMessage で同じ subagent に戻す。
   subagent の権限の確認は持ち主に出る。代わりに承諾せず、拒否されたら迂回せずに持ち主に知らせる。
3. 戻ったら main が差分を確かめる → `audit`（差分をラベル付きで渡す）→ 指摘は読んで確かめてから反映。
4. 実機でしか測れない行があれば、ビルド後に `wintest` に行 ID・期待値・exe・fixtures・scratch を渡して測らせる
   （規則は `docs/claude/wintest-measure.md`）。結果は main が確かめる。`[x]` を付けるのは `docs/claude/lanes.md` の「`[x]` の規則（正）」のとおり。
5. TODO の済んだ項目は `python scripts/archive-done.py`。動きが変わった行は `[x]` を外して `retest` の issue（立てるかは main が判断）。
6. `verify`（`ALL OK`）→ main が `git add` を自分のパスだけにしてコミット・push。版の繰り上げと CHANGELOG はセット。
7. 大きな記録ファイル（TODO-DONE・TESTING-CHECKS・TESTING-LINUX・CHANGELOG・TESTING）は全部を読まない。

## 仕様に必ず書く
- Git Bash は `C:\Program Files\Git\bin\bash.exe`（素の `bash` は WSL）。
- ビルドは R:（`R:\cargo-target\kura`）。ソースと worktree は C:。`--target-dir` で C: に作らない。
- 検査: `cargo fmt --all --check`、文書の変更なら `cargo run --example make-testcheck -- --check` と `-- --lane linux --check`。
- 改行は LF。TESTING.md は英語、`scripts/testcheck-ja.toml` は日本語（行 ID は引用符つきのキー）。
- ワーカーに触らせない: `Cargo.toml` の版・`Cargo.lock`・`CHANGELOG.md`・`TODO.md`。これは指示役がコミット前に行う。

## 指示役の側
- 動きが変わる変更には TESTING.md の行を足す（`[x]` を付けてよいのは実機のセッションと持ち主だけ）。
- 版: PATCH を上げ（文書だけでも）、`cargo build` で Cargo.lock、CHANGELOG（JST: Git Bash では `TZ=JST-9 date +%F`。この PC の Git Bash は `TZ=Asia/Tokyo` を解さず UTC の日付を出す）、件名 `vX.Y.Z: …`。
- 実行中の `kura.exe` がリンクを塞ぐ（`os error 5`）ときは、止めてよい（事前確認不要）。
- 一時ファイルは `R:\Temp`（`mktemp -p /r/Temp`）。
- `git add` は自分のパスを 1 つずつ。`git diff --staged --name-only` で確かめ、コミット後に `git show --stat HEAD`。
- push は `scripts/push-main.sh`（verify・CI 待ち・版の付け直しを含む）。`--force` と `git merge origin/main` は使わない。
  ito の `rev` を上げるコミットは自動で付け直されない。手で rebase して版を付け直す。
- 動きが変わった行は TESTING-CHECKS.md の `[x]` を外し、`retest` の issue を立てる（立てるかは main が判断する。持ち主に聞かない。`CLAUDE.md` の規則）。
- レーン／QA の PR（`test/*`）はマージしない（`Merge lanes` ワークフローの仕事）。
