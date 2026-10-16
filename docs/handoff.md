# 引き継ぎ（2026-10-16、持ち主の Windows の PC のセッション）

次のセッションへのメモ。**やりかけの作業はない。**このセッションの分はすべて main に入り、CI は緑。作業用の worktree・ブランチ・R: のビルドは片付けた。

- ito: 0.43.0（2ec61d0）
- kura: v0.114.5（5034831）
- yagura: v0.66.2（7a51f3f）
- tsumugi: v0.114.2（c42e16c）。このセッションのマージは 2b194f6
- minato: v0.46.1 から先は触っていない

手元の main のクローン（`C:\dev\kura`・`ito`・`yagura`・`minato`）は origin より古いまま。このセッションは pull せず、`origin/main` から C: に worktree を作って作業した（ビルドは `R:\cargo-target\<名前>`）。

## このセッションでやったこと

### 1. 選んだときのコピーと選択の色を ito-select にまとめた

- 依頼は「kura のプレビューで選んでもコピーされない。tsumugi はされる」と「選択の色を tsumugi に合わせて」。持ち主の答えは「色は ito-select で定義する」「文字の下に塗る処理も ito に移す」「対象のアプリは全部」。
- **ito 0.37.0（f30ada7）**
  - 選択の色 `ito_select::fill(ground, accent, light)` を足した。ito-theme の `Colors::selection()` はこれを呼ぶ。
  - 選択は文字の後ろに塗る。
  - ドラッグを離したとき・ダブルクリック・トリプルクリックでコピーする（`Driven { copy, open }`）。
  - URL の中をダブルクリックするとアドレス全体を選ぶ。
- kura v0.112.7（7ec049e）、minato v0.46.1（79caeb3）、tsumugi v0.111.2（964deed）が使う。
- **見送ったもの**
  - Alt+ドラッグの矩形選択
  - 端での自動スクロール
  - Linux の primary selection
  - 行の右端までの塗り
  - プレビューの検索の一致の背景が選択の上に残る件
- michi と yagura には文字の範囲選択が無い。minato は台本で動かせないので、実際には動かしていない。

### 2. kura の再テスト

| 再テスト | 結果 |
| --- | --- |
| #380・#378・#382 | PR #385、v0.112.10 でマージ。途中で #381（選択が次のファイルに残る）を見つけ、v0.112.8 で直した |
| #349 | PR #398（54.1 の `[~]`→`[x]` が規則外だったので、持ち主の (a) の指示で手でマージ）。#396（白い `overall` で字が読めない）と #397（選んでいるタブの番号）を v0.113.6 で直し、再テストは #399 |
| #399・#389・#390 ほか | PR #402、v0.113.7。29.16 が落ちて #401 になった |
| 全部 `[x]` の 7 件 | #380・#378・#382・#355・#339・#333・#330。根拠をコメントに書いて閉じた |

- #383（`--keys` の `<C-c>` が本物の Ctrl+C と別のイベントになる）は、このセッションで立て、ほかで閉じられた。

### 3. 2 枚目の画面で閉じた窓が主の画面に開く（kura #401 → yagura #58・tsumugi #176）

- **原因:** egui が新しい窓の場所を 2 回設定する。1 回目は窓を作った画面の倍率、2 回目は窓が着いた画面の倍率で同じ数を読む。そのため、倍率の違う画面（150 % と 100 %）では両方に合わない。
- 段階:
  - ito 0.39.0（816ae06）・kura v0.113.8（38e866f）: `window.toml` に `scale`（画面の倍率）と `zoom`（アプリの拡大）を書き、窓を見せる前に画素で置く。
  - ito 0.41.0（dbb1718）: 窓を追って書く `Tracker` を共通にした。yagura v0.65.2（2b7d2c5）、tsumugi v0.113.2（1d3a8a1）、kura v0.113.9（fc87de4）。
  - ito 0.43.0（2ec61d0）: 読み書きをまるごと `window::Saved` にした。kura v0.114.3（7f38554）、yagura v0.66.1（9f56708）、tsumugi v0.114.1（17bc100、マージは 2b194f6）。
- 持ち主の依頼で、kura の文書の mimamori を yagura に直した（v0.113.8。CHANGELOG と過去の報告は記録なのでそのまま）。

### 4. 最後の再テスト

- **kura #408（29.16）:** レーンの PR #410（v0.114.5）が先に押していた。issue は自動で閉じなかったので、手で閉じた。
- **yagura #59（5.22）:** この PC で release 版を使って 30 項目すべて通した。PR #60 をワークフローがマージし（v0.66.2）、#59 は手で閉じた。

## 窓の場所の今の作り（ito `crates/ito-common/src/window.rs`）

- `Saved::load(dir)` → `viewport(builder)` → 窓ができたら `put(window)` → 毎フレーム `tracker()` で得た `Tracker::follow(ctx)`。
  - `Tracker` は、窓が止まって `SETTLE`（0.8 秒）たったときと、閉じるときに `window.toml` を書く。最小化の場所は書かない。
- `load()` は古い形も読む。
  - kura・yagura の古い形（`w`/`h`）は common.toml の `scale` で見ていた大きさとして読み、すぐ今の形で書き直す。
  - tsumugi の古い形（`width`/`height`）も読む。
- `PRIMARY_GUESS` と `GRAB` は ito の 1 か所にある。tsumugi の `winplace.rs` に残るのは `dir()` と `wanted()` だけ。
- **前からの制限（変えていない）**
  - kura・yagura の古い形で、2 枚目の画面で閉じたものは捨てて主の画面に開く。古い形には倍率が無いため。
  - 古い形を最大化で開いている間、`window.toml` は `scale = 0.0`。元に戻して閉じると本当の値が入る。設計どおりで、レーンも同じ値を見た。

## 残っていること

このセッションから出たもの:

- **tsumugi #178**（18.14・18.16）: 画面をまたいでドラッグする確認。Windows のレーンが手で押す。
- **tsumugi の `menu::window_at()`**（引き剥がしたタブの窓の場所）: 同じ「2 回設定」の問題があるかもしれない。範囲外として触っておらず、issue も立てていない。
- **tsumugi #170**（最小化して閉じると 16x16 で保存）: v0.112.3 で見つかったもの。共通の `Tracker`（v0.113.2 から）で直っているかは確かめていない。
- **54.18 の字の読みやすさ:** `F` で見つけた字が Catppuccin Latte で 3.71:1、tsumugi Light で 4.24:1 と、4.5:1 を下回る。持ち主の判断待ちで、issue は無い。

持ち主の判断・ほかのセッション待ち:

- kura #316: 54.19 だけ残る。Windows 全体のライト/ダークを切り替えるので押していない。
- kura #390: ARM64（build 28000）でも押すまで開けておく。
- kura #403・#407（レーンの報告の残り）、#405（25.19f の文）。
  - #403 には次の 3 つがある。
    - 16.13a の `<S-Enter>` を `O` に直す。
    - ito の `merge-lanes.py` が「none new」などを「何も無い」と読めるようにする。
    - 設定の検索で設定のキーでも探せるようにする（#388）。
- kura #404: 行は全部 `[x]`。ほかのセッションが立てたもので、このセッションでは触っていない。

## 落とし穴（このセッションで踏んだもの）

- **Merge lanes のワークフローがマージしても、`Closes #N` の issue が閉じないことがある**（kura #389・#408、yagura #59）。
  マージを確かめてから、PR を指すコメント付きで `gh issue close` する。
- **レーンの PR が変えてよい印は、`[ ]`→`[x]` と `[ ]`→`[~]` だけ。**`[~]`→`[x]` は持ち主だけ（`.claude/merge-role.md`）。破るとワークフローが止める（#398）。
- `cargo --config` で手元の ito を差し込むと、`cargo clippy` だけがそれを落とし、`Cargo.lock` を元に戻す。
  `.cargo/config.toml` の `[patch]` を使う（メモリ `local-ito-patch`）。
- `scripts/push-main.sh` は、ito の rev を上げたコミットの版を付け直せない。手で積み直す（メモリ `push-main-rev-bump`）。
- `git worktree remove` の前に、シェルの cwd を worktree の外に出す。`target` のジャンクションが残ったら `cmd //c rmdir` で消す（メモリ `worktree-remove-cwd`）。
- PowerShell の `git commit -F - @'…'@` は効かない。メッセージはファイルに書いて `-F <file>` で渡す。`gh … -q` の jq は Bash で書く（PowerShell では引用が崩れる）。
- 比べるために tsumugi を分けて起動すると、新しいタブの既定が `claude` で claude が起動する。pwsh で開く設定にしてから起動する。

## 実機で試すときの約束（持ち主が PC を使っている）

- **実マウスを動かさない。**窓は `SetWindowPos`・`ShowWindow`・`WM_CLOSE` で動かし、キーは `--keys` か `SendInput`。最初の回でドラッグが持ち主のマウスと重なったことがある。
- デスクトップのロック `Local\wintest-desktop` を取ってから操作する。ほかのレーンとぶつからないようにするため。
- 設定は分ける: `YAGURA_STATE_HOME`・`YAGURA_CONFIG_HOME`・`UCHMK_CONFIG_DIR` など。持ち主の `%LOCALAPPDATA%\uchmk` と `%APPDATA%\uchmk` には書かない。
- 持ち主の tsumugi とサーバーは止めない（このセッションでは PID 35820 と 16740。再起動で変わる）。
- ほかのセッションの worktree には触らない: `kura-verify`、`filer-wintest`、`tsumugi-verify`、`tsumugi-wintest`、`yagura-e089`、`yagura-wintest`。
- 使い回せる台本: `C:\dev\filer-evidence\yagura-retest59-20261016\scripts\retest-5-22.ps1`（`WinPlace.cs`・`RealInput.cs`。窓の場所を読み、窓を動かして閉じる）。
- 画面の画像は持ち主のデスクトップが写るので、C: にだけ置き、PR には貼らない。
- この PC: DISPLAY1 3840x2160 150 %（主）、DISPLAY2 2560x1440 100 %（右、x 3840-6400）。Windows 11 x64 build 26300。

## 証拠の置き場（`C:\dev\filer-evidence\`）

| フォルダ | 中身 |
| --- | --- |
| `win-retest-380-20261016` | #380（選択のコピーと色） |
| `win-retest-349-20261016` | #349（マウスと色） |
| `win-retest-399-20261016`・`20261016-1141-retest-399` | #399 |
| `fix401-20261016` | kura #401 |
| `fix58-20261016`・`fix176-20261016`・`kura-tracker-20261016` | yagura #58・tsumugi #176・kura の Tracker |
| `ito-saved-20261016` | ito 0.43.0 の `window::Saved` |
| `yagura-retest59-20261016` | yagura #59 |
