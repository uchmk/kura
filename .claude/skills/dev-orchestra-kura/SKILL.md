---
name: dev-orchestra-kura
description: "kura-specific rules for /dev-orchestra: spec fields, version/CHANGELOG/TESTING handling, commit and push-main.sh. Read together with the global dev-orchestra skill when the repo is C:\\dev\\kura."
---

# dev-orchestra の kura 用メモ

グローバルの `dev-orchestra` に足す、kura だけの決まり。正は `CLAUDE.md` と `docs/claude/`。

## 流れ
1. 設計（必要なら Codex 設計レビュー）→ 実装役の選択 → 仕様ファイルを scratchpad に置く。
2. 実装は Sonnet のワーカー。**`worker-start --agent claude` は Bypass Permissions の確認画面で止まることがある。**
   通常モードで動かすには `orca terminal create --worktree current --title <名> --command "claude --model sonnet --effort high"` で端末を作り、
   `worker-start --spec-file <仕様> --terminal <handle>` で渡す。渡したらターンを終えて Orca の通知を待つ
   （ポーリング・再送をしない。`start_unknown` / `turn_start_unobserved` は待つか、`orca terminal read` で確かめる）。
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
- 版: PATCH を上げ（文書だけでも）、`cargo build` で Cargo.lock、CHANGELOG（JST: `TZ=Asia/Tokyo date +%F`）、件名 `vX.Y.Z: …`。
- 実行中の `kura.exe` がリンクを塞ぐ（`os error 5`）ときは、止めてよい（事前確認不要）。
- 一時ファイルは `R:\Temp`（`mktemp -p /r/Temp`）。
- `git add` は自分のパスを 1 つずつ。`git diff --staged --name-only` で確かめ、コミット後に `git show --stat HEAD`。
- push は `scripts/push-main.sh`（verify・CI 待ち・版の付け直しを含む）。`--force` と `git merge origin/main` は使わない。
  ito の `rev` を上げるコミットは自動で付け直されない。手で rebase して版を付け直す。
- 動きが変わった行は TESTING-CHECKS.md の `[x]` を外し、`retest` の issue を立てる（立てるかは main が判断する。持ち主に聞かない。`CLAUDE.md` の規則）。
- レーン／QA の PR（`test/*`）はマージしない（`Merge lanes` ワークフローの仕事）。
