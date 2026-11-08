# 実機の測り方（共通の規則）

実機のセッション（持ち主の PC の main モデル）と、使い捨ての subagent `wintest`（[.claude/agents/wintest.md](../../.claude/agents/wintest.md)）が
共通で守る規則の要約。**詳細と経緯は [.claude/windows-role.md](../../.claude/windows-role.md) が正**（ここから何も移していない）。
チェックを付ける規則は [lanes.md の「`[x]` の規則（正）」](lanes.md#x-の規則正)。

## 1. 測ってから「見た目」と呼ぶ

- 見た目に聞こえる行にも、**読めるもの**が代わりにあることが多い。先に探す（windows-role.md「Measure before you call it a look」の表）。
  位置や色はピクセル、数は文字、状態は `<State:name>`、スクロールは `KURA_KEYS_DONE`、プロセスは `Get-CimInstance Win32_Process`。
- 人の目でしか決まらない行（色が黄色に「見える」か、帯が「目立つ」か、「重なって」いるか、「滑らか」か）は測れない。
  その行は **どの代わりを試し、なぜ主張を支えられなかったか** を書く。「見た目だから」だけは理由にならない。
- 代わりの測定（`conhost.exe` の数だけ、など）は行全体の PASS にならない。測れなかった部分を **未確認** に挙げる。

### 1a. 矩形と画素を測る（Tier 1。2026-11-08 承認）

見た目の行でも、期待値のすべてを機械が測った行は `[x]` になりうる（[lanes.md](lanes.md#x-の規則正)。条件は ito の `docs/visual-test.md` 版 1）。測定に数えるのは次だけ。

- **同じ実行の** `<State:>` / state JSON の値、UI 要素の矩形、その実行の PNG から読んだ画素（矩形から割り出した座標）、持ち主が承認した基準画像との差。
- 閾値は、その行の check spec（`tests/visual/<行>.toml`）に**実行の前に**書いたもの。見たあとで決めた閾値は数えない。
- 期待値 1 つにつき測った数 1 つ。**数・閾値・出どころ（ファイル＋キー、または画素の座標）**を報告する。
- **AI Vision の答え・モデル同士の一致・自分が見た印象は、測定に数えない。**

kura に**今ある**道具: `<State:名前>` の文字（`cwd:`・`hovered:`・`overlay:`・`view:` など。矩形は無い）、`<Shot:名前>` の PNG（窓全体。`shots/名前.png`）、`<Shot:名前@preview>`（プレビュー欄の矩形で切った PNG）、
画素は PNG からスクリプトで読み、座標と値をファイルに残す（目で拾った数は測定に数えない）。`Shot` の付属 JSON（`shots/名前.json`。theme・lang・size・rect・component・ppp）は kura から出る（v0.156.0）。矩形つきの state はまだ出ない（`src/main.rs` の `state_report`、`src/script_run.rs` の `take_shot`/`save_shot`）。
**ito から来るのを待つもの**: state JSON v1、`<Snap:>`、矩形つきの Tree、検査器、証拠の folder（ito の `docs/visual-test.md` 成果物 2・決定 5）。
check spec が無い行、または今の道具では期待値のすべてを測れない行は、**目で見た行のまま**で、`[~]` までにする。

画素の判定は **Windows / macOS の実機だけ**。**Linux のレーン（Xvfb）では画素を判定しない**（CPU 描画は実機の描画の代わりにならない）。

## 2. GUI の走らせ方

- `pwsh -NoProfile` で `scripts\keys.ps1 -Bin <exe> -Dir <dir> -Out <走らせるごとに新しい dir> -Isolate`。
  `-Isolate` で持ち主の設定・common.toml・状態に触れない。終わりは `<Quit>`、プログラムを起こす走りは `-Launches`。
- 読む順は `<State:名前>` が書く `名前.txt` と、`KURA_KEYS_DONE` の最終行 `keys: done`。**`keys: stalled` は結果ではない**（そのまま報告する）。
- 終了コード 0 と `keys: done` は「測定が走った」だけで、行が通った証拠ではない。終了コードは走らせた直後に保存する。
- 古い状態ファイルを使い回さない。期待値の 1 つずつに観測を対応させる（前提 → 操作 → 期待値ごとの観測）。
- マウスは `SendInput`（`PostMessage` は届かない）。`f` は部分列で絞るので、絞ったあと `hovered:` を読み返す。表示倍率は OS で変えない（`KURA_SCALE`）。

## 3. 机・場所・後片付け

- デスクトップを使う走りは **`Local\wintest-desktop`** のロックを、最初の走りから後片付けまで持つ。取れなければ少し待ち、だめなら `BLOCKED`。他のセッションや作業を止めない。
- 書いてよいのは **scratch の下だけ**（新しい GUID のフォルダ）。scratch は x64 では `R:\Temp`（RAM ディスク。ごみ箱が無いので、`d` を押す行は `C:` の一時フォルダ）、ARM64 では `%TEMP%\kura-scratch`。
  自分で別の置き場を作らない。リポジトリ（`C:\dev\kura`）と未コミットのものは R: に置かない。
- 渡されていない fixtures、`qa-reports`、証拠、ごみ箱全体を消さない。`scripts\make-fixtures.ps1` は既存のフォルダを消すので、渡された fixtures には回さない。
- 止めてよいのは **自分で起こしたプロセスだけ**（PID・開始時刻・exe のパスで確かめる）。持ち主の kura・ブラウザのタブ・エディタ・設定は触らない。
- OS の設定を変えない（レジストリ、ファイアウォール、SMB、表示倍率、スクリーンセーバー、プロファイル）。クリップボードは `<State:>` を優先し、実物が要って元の形式を戻せないなら `BLOCKED`。
- `d` で捨てたものは同じ走りで戻す。前の走りの残り（scratch 配下のごみ箱の項目、`file:///` の Chrome のタブ）は数を書いてから片付ける。
- 後片付けは try/finally。失敗したら別に書く。

## 4. 証拠

**R: にだけある証拠は証拠ではない**（再起動で消える）。取ったらすぐ C: か PR・Issue・報告に写す。

- 行ごとに: 行 ID、結果（`PASS` / `FAIL` / `CANNOT-MEASURE(理由)` / `BLOCKED`）、期待値それぞれに対する観測、
  **実行したコマンド**、**終了コード**、**生の出力の抜粋**、証拠の場所、後片付けの結果、未確認。
- 走らせたこと（プログラムが起動した）と確かめたこと（期待値が観測と合った）を混ぜない。
  「見落とすには、画面やディスクが何と違っていなければならなかったか」に答えられないなら、その行は見た目の行で、測定の行ではない。

## 5. 誰が何をするか

- `wintest` は **測って証拠を返すだけ**。ビルド・コード編集・チェックを付ける・Issue・git はしない。
- チェックを付けるかは main が決める（規則は [lanes.md](lanes.md#x-の規則正)）。測れなかった行は付けない。
