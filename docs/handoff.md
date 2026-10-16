# 引き継ぎ（2026-10-16、持ち主の Windows の PC のセッション）

次のセッションへのメモ。この日の PC のセッション 2 つの分で、2 回目（夕方）は 1 回目のメモの「残っていること」を進めた。
**やりかけの作業はない。**どちらの分もすべて main に入っている。作業用の worktree・ブランチ・R: のビルドは片付けた。

- ito: 0.44.1（0f13464）
- kura: v0.115.1（このメモの版）
- yagura: v0.66.2（7a51f3f）。ito は 0.43.0 のまま
- tsumugi: v0.114.4（d28904c）。ほかのセッションの #179 のマージ。ito は 0.43.0 のまま
- minato: v0.46.1 から先は触っていない

手元の main のクローン（`C:\dev\kura`・`ito`・`yagura`・`minato`・`tsumugi`）は origin より古いまま。どちらのセッションも pull せず、
`origin/main` から C: に worktree を作って作業した（ビルドは `R:\cargo-target\<名前>`）。

## 2 回目のセッション（夕方）でやったこと

### 1. レーンの報告の残り（kura #403・#405・#407）

- v0.114.7（b8ed473）: TESTING 16.13a のトーストのキーを `O` に、25.19f の一時フォルダのファイルを「リダイレクトしたハンドルごとに 1 つ」に書き直した。
  動きは変わっていないので、印はそのまま。
- #403・#405・#407 を閉じた。行が全部 `[x]` の #404 と、済んだ調整の #361 も、根拠をコメントに書いて閉じた。

### 2. 設定の検索を設定ファイルのキーでも（kura #388、#403）

- ito 0.44.0（a1f341b）: `ito_prefs::keyed_row` と `GENERAL_KEYS`。キーは画面に出さない。docs/common-spec.md にも書いた（`notify_page` の行はまだキーを持たない）。
- kura v0.115.0（68d25ba、CI 緑）: config.toml と common.toml のキー（`ui.font_size`・`clock.show` など）で行が見つかる。TESTING 54.10a を足し、再テストは #413。
- この PC で、設定を分けた日本語の画面を `--keys` で動かして確かめた。
  - `<C-,><C-f>font_size<Enter>18` で `font_size = 18.0` が入った。
  - `clock.show<Enter><Space>` で `[clock] show = false` が入った。
- yagura と tsumugi は、次に ito の `rev` を上げたときに `keyed_row` を使える（今は `row` のまま）。

### 3. merge-lanes.py の「何も無い」

- ito 0.44.0: `None new.`・`None filed this run.`・`None this run.` を「何も無い」と読む。#366・#368・#370・#376・#386・#395 は、これだけで仕分けの issue になっていた。
- ito 0.44.1（0f13464）: `None from this run.` も読む（#412）。kura は v0.115.1 で写した。
  yagura と tsumugi には、次の `rev` 上げと `scripts/lanes.sh sync` で入る。

### 4. tsumugi（1 回目のメモの残り）

- **#181 を立てた**（`menu::window_at()` の件）。引き剥がしたタブの窓（`TSUMUGI_WINDOW_AT`）は、倍率の違う 2 枚の画面で場所がずれ、どの画面にも出ないこともある。
  - v0.114.3 の release 版で、設定を分けて起動し、`GetWindowRect` で測った。
    - 150 % の画面の窓から 100 % の画面へ落とすと、150 % の画面に 2/3 の大きさで開く。
    - 100 % の画面の窓から引き剥がすと、原点から 1.5 倍の場所に開く。x ≈ 4270 px より右で落とすと、どの画面にも出ない。
  - 原因は #176 と同じ「2 回設定」に加え、`torn_at` が元の窓の倍率を送らないこと。直し方の案（`x,y,scale` を送り、`Window` を作って `put`）は issue に書いた。
  - 直していない。C:\dev\tsumugi ではほかのセッションが #179 をブランチで進めていたので、tsumugi のセッションに任せた。
- **#170 を閉じた。**x64 の v0.114.3 で確かめた。
  - 最小化して閉じても、`window.toml` には `w = 1280.0, h = 800.0` が残り、次の起動は 1920x1200 px だった。
  - 手で書いた 16x16 の `window.toml` は読まずに既定の大きさで開き、書き直した。
  - ARM64 でまだ出るなら開け直す、とコメントした。

## 1 回目のセッションでやったこと

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

- **tsumugi #181**（上の 2 回目の 4）: 直すのは tsumugi のセッション。直ったら 3.34 を倍率の違う 2 枚の画面で押す。
  マウスを使わない測り直しは `C:\dev\filer-evidence\handoff2-20261016\scripts\windowat.ps1`（`WINDOW_AT` を `x,y,scale` にする）。
- **tsumugi #178**（18.14・18.16）: 画面をまたいでドラッグする確認。Windows のレーンが手で押す。
- **54.18 の字の読みやすさ:** `F` で見つけた字が Catppuccin Latte で 3.71:1、tsumugi Light で 4.24:1 と、4.5:1 を下回る。持ち主の判断待ちで、issue は無い。
- kura #413（54.10a、設定の検索のキー）: x64 のレーンが押す。
- kura #316: 54.19 だけ残る。Windows 全体のライト/ダークを切り替えるので押していない。
- kura #390: ARM64（build 28000）でも押すまで開けておく。
- kura #393（verify の中でテストが 1 回 60 秒以上止まった。再現しない）と #345（横断のレビューの提案）は前からあるもの。どちらのセッションも触っていない。

## 落とし穴（この日に踏んだもの）

- **Merge lanes のワークフローがマージしても、`Closes #N` の issue が閉じないことがある**（kura #389・#408、yagura #59）。
  マージを確かめてから、PR を指すコメント付きで `gh issue close` する。
- **レーンの PR が変えてよい印は、`[ ]`→`[x]` と `[ ]`→`[~]` だけ。**`[~]`→`[x]` は持ち主だけ（`.claude/merge-role.md`）。破るとワークフローが止める（#398）。
- `cargo --config` で手元の ito を差し込むと、`cargo clippy` だけがそれを落とし、`Cargo.lock` を元に戻す。
  `.cargo/config.toml` の `[patch]` を使う（メモリ `local-ito-patch`）。
- `scripts/push-main.sh` は、ito の rev を上げたコミットの版を付け直せない。手で積み直す（メモリ `push-main-rev-bump`）。
- `git worktree remove` の前に、シェルの cwd を worktree の外に出す。`target` のジャンクションが残ったら `cmd //c rmdir` で消す（メモリ `worktree-remove-cwd`）。
- PowerShell の `git commit -F - @'…'@` は効かない。メッセージはファイルに書いて `-F <file>` で渡す。`gh … -q` の jq は Bash で書く（PowerShell では引用が崩れる）。
- **merge-lanes.py が知らない「何も無い」の書き方の報告は、仕分けの issue（「what its report left to sort」）になる。**
  中身が「無い」だけなら閉じ、ito の `NOTHING` に書き方を足す（`--self-test` に例が並ぶ）。
- **C:\dev\tsumugi は、ほかのセッションがブランチで使っていることがある**（2 回目では `fix/server-stop-179`）。直すなら origin/main から C: に worktree を作る。
- 比べるために tsumugi を分けて起動すると、新しいタブの既定が `claude` で claude が起動する。pwsh で開く設定にしてから起動する。

## 実機で試すときの約束（持ち主が PC を使っている）

- **実マウスを動かさない。**窓は `SetWindowPos`・`ShowWindow`・`WM_CLOSE` で動かし、キーは `--keys` か `SendInput`。最初の回でドラッグが持ち主のマウスと重なったことがある。
- デスクトップのロック `Local\wintest-desktop` を取ってから操作する。ほかのレーンとぶつからないようにするため。
- 設定は分ける: `YAGURA_STATE_HOME`・`YAGURA_CONFIG_HOME`・`UCHMK_CONFIG_DIR` など。持ち主の `%LOCALAPPDATA%\uchmk` と `%APPDATA%\uchmk` には書かない。
- 持ち主の tsumugi とサーバーは止めない（この日は PID 35820 と 16740。再起動で変わる）。
- ほかのセッションの worktree には触らない: `kura-verify`、`filer-wintest`、`tsumugi-verify`、`tsumugi-wintest`、`yagura-e089`、`yagura-wintest`。
- 使い回せる台本:
  - `C:\dev\filer-evidence\yagura-retest59-20261016\scripts\retest-5-22.ps1`（`WinPlace.cs`・`RealInput.cs`。窓の場所を読み、窓を動かして閉じる）。
  - `C:\dev\filer-evidence\handoff2-20261016\scripts\`: tsumugi を `WINDOW_AT` で起動して場所を測る `windowat.ps1`、最小化して閉じる `min170.ps1`、
    kura の `--keys` を分けた設定の日本語の画面で回す `keys388.ps1`。
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
| `handoff2-20261016` | 2 回目: tsumugi #181 の測定・#170 の確認・kura #388 の `--keys` の台本 |
