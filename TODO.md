# TODO

済んだ項目は、その場で [TODO-DONE.md](TODO-DONE.md) に移す（`python scripts/archive-done.py`。コミットの前に回す）。
セッションはここに残る未完了の項目だけを読む。TODO-DONE.md は全部を読まず、Grep か Read の `offset`・`limit` で引く。

**行の末尾の印は、その項目を進めるのが誰か**（v0.73.15）。印の無い未完了の項目が、開発のセッション
（対話でも自動実行モードでも）の取る分で、印の付いた項目は飛ばす:

- `【人】` 持ち主が手を動かすか決めること（機械の設定、ソフトを入れる、リリース、好み）
- `【QA】` TESTING.md の文言や、テストの探索など、QA セッションの分（`.claude/qa-role.md`）
- `【実機】` Windows の実機でしか測れないこと。順番表（`.claude/windows-role.md`）に積んで待つ

- [ ] （#308 の所見）`wezterm imgcat` で 2560x1280 の PNG が何も描かれない（160x80 は描かれる）。ito（ito-pane）で調べる。【pane】
- [ ] （#306 の所見の残り）sixel（`chafa -f sixels`）は同梱の ConPTY に DCS を落とされて本文が文字で出る。kura か ito（ito-pane）が ConPTY にパススルーを頼めるか調べる（`KURA_PTY_LOG` で見る）。【pane】
- [ ] （#306 の所見）`wezterm imgcat` がペインの中で `attempt to divide by zero` で panic する（`wezterm\src\main.rs:383`）。セルのピクセル寸法が 0 で返っている疑い（`CSI 16 t` / `CSI 14 t` の返事、`TIOCGWINSZ`）。49.11 と同じ点。【pane】
- [ ] （#302 の提案）50.7 の「コンソールの窓が出ない」は、ARM64 は測れないとして `[ ]` に残し、x64 は約 5ms 間隔の窓の列挙（753 回）で出なかったと `[x]` にした。5ms より短い窓は取りこぼすので、持ち主が外観の行として扱うか決める。【人】
- [ ] 再テストの順番表に「前の run から変わった行」を書く欄を足し、1 行の修正で約 50 行を押し直させないようにする。`.claude/windows-role.md` の段落（#300 の提案 2）【人】
- [ ] TESTING.md の節 50（`filer mcp`、v0.85.0）は標準入出力の JSON-RPC と `<State:>` で文字で読める行が多いのに、どちらのレーンの表にも無く 0 / 7 のまま。ARM64 の「Unticked rows no queue owns」に節 50 を足す（#301 の提案 1）【人】
- [ ] （2026-10-10、持ち主の窓）x64（RX 9070 XT、Adrenalin 26.8.1 = ドライバ 32.0.31041.1004。最新）で、`cargo run` の filer が panic の出力なしに `exit code: 0xc000041d` で落ちる。イベントログ（Application、ID 1000）では 6:43 と 8:33 の 2 回とも、AMD の GL ドライバ `atio6axx.dll` の同じオフセット `0x51b5b9` で `0xc0000005` が出て、2〜3 秒後に同じ箇所で `0xc000041d`（窓のコールバックの中の例外）。既定の `auto` が GL を選ぶ機械（#204）。きっかけを切り分ける: (1) 落ちた時刻にレーン（別の filer の起動・終了、キー、スクリーンセーバーを止める処理）が動いていたか、(2) 画面の電源オフ・ロック・解像度や DPI の変更・窓のリサイズや最小化で再現するか、(3) WER の LocalDumps（`HKLM\SOFTWARE\Microsoft\Windows\Windows Error Reporting\LocalDumps\filer.exe`）でダンプを取り、`atio6axx.dll` を呼んだ wgpu の関数を見る、(4) `[ui] backend = "dx12"` で同じきっかけを当てて落ちないか（あわせて #204 のアイドルで 1 コアが今のドライバで戻るか）。filer の側で避けられるなら（GL の面の作り直し、既定の見直し）提案する。【実機】

## filer から kura への改名（v0.95.0、2026-10-11）

- [ ] Claude Code の MCP を登録し直す: `claude mcp remove filer` → `claude mcp add kura -- kura mcp`（道具の名前も `kura_state` / `kura_reveal` に変わった）【人】
- [ ] シェルのプロファイルの `filer shell-hook` を `kura shell-hook` に、自分で設定した `FILER_*` の環境変数を `KURA_*` に、keymap の `%FILER_CONFIG_HOME%` を `%KURA_CONFIG_HOME%` に直す【人】
- [ ] タスクバーのピン・ショートカット・PATH の `filer.exe` を `kura.exe` に直す【人】
- [ ] `C:\deviler` を `C:\dev\kura` に変える。変えたら `git -C C:\dev\kura worktree repair C:\deviler-wintest` で実機の作業フォルダのつながりを直す（ARM64 の机も同じ）【人】
- [ ] 実機のレーンの名前（作業フォルダ `C:\deviler-wintest` / `filer-armtest`、タスク `filer-auto-wintest*`、`%LOCALAPPDATA%iler-wintest`、`C:\deviler-evidence`）は動いているタスクを壊さないよう filer のまま残した。変えるなら、タスクを登録し直すのと一緒に `scripts/auto-wintest.ps1` と `.claude/windows-role.md`・`docs/claude/lanes.md` を直す【人】
- [ ] 実機の机のデスクトップの `filer-fixtures` は `kura-fixtures` になった。次の run の前に `scripts\make-fixtures.ps1` を回す【人】

## uchmk のアプリを揃える（v0.99.0、2026-10-11）

倍率・キー・置き場所・環境変数を ito の共通仕様（[docs/common-spec.md](https://github.com/uchmk/ito/blob/main/docs/common-spec.md)）に揃えた。

- [ ] 設定が `%APPDATA%\kura` から `%APPDATA%\uchmk\kura` へ自動で移った（`kura.toml` は `config.toml`）。自分で `KURA_CONFIG_HOME` を使っていれば、そのフォルダの `kura.toml` も `config.toml` に改名される。古いフォルダが残っていれば中身を確かめて消す【人】
- [ ] 実機のレーンを common.toml から切り離す: レーンの kura が `<C-=>` を押すと持ち主の common.toml の倍率が変わり、机のほかの uchmk のアプリも大きくなる。`scripts/auto-wintest.ps1` で `UCHMK_CONFIG_DIR` を一時フォルダに向けるか、`KURA_SCALE=1` を渡す【実機】
- [ ] 窓の位置と大きさを覚える処理を ito に寄せる（yagura などほかのアプリにも同じものが要る）【人】（ito リポジトリに新しい部品を足す作業で、このセッションの GitHub 権限は kura だけ。ito 側で `ito-prefs` 等に足してから、kura の `rev` を上げる）
- [ ] 15.10・15.11（倍率がほかのアプリに伝わる）は yagura を横に開いて確かめる（yagura は common.toml の倍率に従う）【実機】

## 多言語対応（2026-10-10、持ち主の希望）

持ち主は filer も後で多言語にしたい。言語は uchmk のアプリ（filer・tsumugi・yagura（旧 mimamori））で共通の `<設定のフォルダ>/uchmk/common.toml` の
`language = "auto" | "en" | "ja" | …` で選び、アプリごとの設定に `language` があればそちらが勝つ。`auto` は OS の言語（`sys-locale`）。
仕組みは先に yagura の `src/i18n.rs`（`tr("key")` / `trf("key", &[..])`、英語の表は `src/lang/en.toml`）で作った。

- [ ] filer の画面の文字（ヘルプの見出し・トースト・オーバーレイ・ステータスバー・確認の文）を `tr()` に通し、英語の表を足す。
  yazi の keymap の `desc` は利用者の設定から来るので訳さない。表に無いキーはテストで落とす。【後】
- [ ] 日本語の表を足す。CJK のフォントは ito の `ito-fonts` を使う。filer.toml に `language` を足し、README に書く。【後】

## テストと QA の運用を軽くする（2026-10-11、持ち主の判断）

x64 を唯一のメインの機械にし、ARM64 はリリースのときだけ arch に関わる行を押す（`cargo test` と起動の確認は GitHub の ARM64 ランナー）。
所見・バグ・確認事項は GitHub Issues に移し、共通の道具は ito に置く。段階ごとに 1 つの版で出す。

- [ ] わざと汚した作業フォルダから起動し、`rescue/` ブランチができること・状態の issue にコメントが付くこと・次の :20 で PR が出ることを見る【実機】
- [ ] 次のリリースで `release-arm64` の issue が立ち、ARM64 の起動がそれを押して閉じ、`ARMTEST_DONE` で終わることを見る【実機】
- [ ] v0.103.0 の `lane-boot.ps1` が `scripts/lanes.conf` の `state` を読み、今までと同じ `filer-wintest` の状態フォルダを使うこと（次の :20 の起動のログと状態の issue で見る）【実機】
- [ ] kura のトーストを `ito_a11y::announce` で読み上げる（今は何も announce しないので、`<Announced:x>` は必ず落ちる）。
  uchmk/ito#207（`announce` の live の直しと、トーストの読み上げの共通の部品 `Told`・`announce_toast`）が入ったら、`rev` を上げて
  `Toast` に `Told` を持たせ、`draw_toasts` の先頭で読む。Linux の smoke `announced`、TESTING 25.17h の文言と 71 節を足す【後】
- [ ] 見た目の行のパイロット（59.5・41.15・49.4・54.18）: ito の uchmk/ito#205（state JSON v1・`<Snap:>`・矩形つきの Tree・Shot の付属 JSON・検査器・証拠のフォルダ）が出たら、`rev` を上げて state の中身を入れ、check spec を書いて回す【後】

## マージで止めている実機の PR

マージのワークフローか Routine が止めた実機の PR。ここにある `#N` はワークフロー（`scripts/merge-lanes.py`）がマージしない。
持ち主の答えがここに書かれたら、Routine がその回のうちにそのとおりにする（`.claude/merge-role.md` の 3）。

いまは無い。

## ARM64 の再テストの所見（#295）

- [ ] （#295 の所見 1）16.13 / 16.13a が ARM64（build 28000）で失敗する: `has_default_app`（`src/exec.rs`）は `AssocQueryStringW(ASSOCSTR_EXECUTABLE, ".xyz")` の成功を「アプリあり」と見るが、この機械では関連付けの無い拡張子でも `OpenWith.exe` を返し、`start` が「アプリを選ぶ」の箱を出す。x64（v0.82.0 で通った）との差。`OpenWith.exe`（と `rundll32.exe` 系）を「アプリ無し」に数える案。x64 でも同じ値が出るか先に確かめる【実機】

## Sonnet の見張り（2026-10-05 まで）

- [ ] （Sonnet の見張り）v0.78.209 は、項目の「GL を確かめるのに起動 1 回分の手間がかかるので、測ってから」を Linux の 11 ms で済みにしたが、`predicted_fallback` は `auto_backends(cfg!(windows), …)` なので **Windows 以外では GL の確かめ（`has_adapter`）を呼ばない**。測ったのは確かめの無い経路だけ。
  しかも前は `[ui] backend` の警告があるときだけ呼んでいたのが、既定の `auto` で毎回呼ぶようになった。x64 と ARM64 で `Measure-Command { filer env --out x.txt }` を v0.78.208 と v0.78.209 で比べ、目に見えて遅ければ確かめを警告のあるときに戻すか、前の起動の `last-run.toml` の Adapter から言う形にする。【実機】

## LLM との連携（2026-10-09、持ち主の希望）

filer・tsumugi・これから作るアプリを LLM とつなげたい。ソースを使い回せる形にすることも含めて課題にする（持ち主の言葉）。
`ito-pane`（旧 `tsumugi-pane`）と同じく、共有のクレートに入れて各アプリが `rev` で固定して読む形が第一候補。

- [ ] 段 5: 書く道具（窓の確認の箱つき）。#315 の 1 で決まり、kura は v0.109.0 で入れた（TESTING.md 50.8-50.13）【実機】。tsumugi のセッションへの入力はやらない（2026-11-08、tsumugi をメンテナンスの対象から外した）

## 印の意味（2026-10-04）

`【人】` `【QA】` `【実機】` は開発のセッションが進めない印（CLAUDE.md の作業ルール）。**`【後】` は今週（2026-10-10 まで）の範囲から外した大きい項目**
（ConPTY の exe への埋め込み、Wayland、次のプロジェクト）で、開発の Routine も飛ばす。今週の「完了」は、残りが `【人】` と `【後】` だけになること。

## ロードマップ

### 2. Git 統合

- 対象外（方針）: ステージング・diff・コミットはやらない。状態表示のみ。

- [ ] 残: **コミットグラフのビューア**（SourceTree のような画面）。枝線、ブランチ/リモートの 【後】
  ラベル、日時・作者・ハッシュの列、コミットを選んで差分へ。専用オーバーレイ、keymap 層、
  スクロール、枝の割り当てアルゴリズムが要る。**比較ビュー（45 節）と同等かそれ以上の規模。**
  2026-10-09、持ち主: 保留。別のアプリにするかもしれない。
  - 先に spot の Git 節を実機で使ってみて、それで足りるかを見てから判断する。
  - **filer はファイルマネージャで、SourceTree や lazygit は git クライアント。**同じ土俵で
    戦う価値があるかは、作る前に決めること。

### 7. Undo / Redo

- [ ] 残: macOS でゴミ箱から戻す方法。`osascript` で Finder に戻させる手はあるが、 【後】
  シェルアウトが増えるので入れるか要判断。

## 直した 2 件の答え合わせ（完了）

- [ ] **1.22 / 1.23 を実機で取る。**1.22 が上の修正の答え合わせ、1.23 は `<A-t>` 側で 【実機】
  **報告が「押していない」と明記していた**ぶん。

## ペインで tcell のプログラム（lazygit / gh-dash）を動かすときの不具合

## 実機のテストを無人で回す（v0.50.0）

- [ ] 順番表が空になったら、残りの行（見た目の行）を人の作業として一覧にする。 【後】（空になるまで待つ）

## 実機のセッションの報告（v0.52.3 で受けた分: #86、45 節）

## Windows 版を exe 1 つで配れるようにする

v0.49.0 から Windows 版は zip（`filer.exe` + `conpty.dll` + `OpenConsole.exe` + ライセンス表示）。
新しい ConPTY を exe に埋め込めば、また exe 1 つで配れる。

- [ ] 作る。【後】
  1. ビルド時に 2 つのファイルを `include_bytes!` で埋め込む（約 1.2MB 増）。**ファイルがあるときだけ**
     埋め込み（build.rs で有無を見て cfg を立てる）、無ければ今までどおり Windows 標準の ConPTY を使う。
     こうしておけば、手元の `cargo build` はファイル無しでも今のまま通る。
  2. ペインを初めて開くときに `%LOCALAPPDATA%\filer\conpty\<版>\` へ書き出す。**毎回 SHA-256 を
     照合**し、違えば書き直す（すり替えられた DLL を読まないため）。古い版のフォルダは片付ける。
  3. 書き出した `conpty.dll` を **フルパスで `LoadLibraryW`** しておく。同じ名前の DLL が読み込み済みなら
     Windows はそれを使うので、`alacritty_terminal` が後で名前だけで読んでも、書き出したものが使われる
     （`alacritty_terminal` は改造しなくて済む）。`conpty.dll` は自分の横の `OpenConsole.exe` を起動する。
  4. MIT の表示を、書き出したフォルダと `filer env`（またはヘルプ）に出す。
- [ ] **実機で Windows Defender / SmartScreen に止められないか確かめる。ここが通るまで配布形は変えない。** 【実機】
  「exe が別の exe を書き出して起動する」はドロッパーと同じ形。書き出す 2 つは Microsoft の
  Authenticode 署名付き（署名の有無と署名者名は確認済み、検証まではしていない）だが、書き出す側の
  `filer.exe` は署名が無い（Q14）ので、どう判定されるかは試さないと分からない。
- [ ] 通ったら、release.yml の Windows の成果物を zip から exe に戻す（zip を残すかも決める）。 【実機】
  2026-10-09、持ち主: 最終的には filer も tsumugi も、これから作る別のアプリも exe 1 つにしたい。方向は決まったので、上の Defender の確認待ち。
  埋め込みと書き出しはほかの uchmk のアプリでも要るので、共有のクレート（ito の `ito-pane` の側）に置くのがよい。

## GitHub の情報を見る口（Q16）

- [ ] **46.12〜46.16 を実機で取る**（Windows のセッション）。**46.16（回線を抜いて同じ出力）が 【実機】
  本体** —— ネットワークを使わないという主張は、実機で示すまで主張のまま。
- [ ] 残った差分があれば、そのとき初めて案 3（別アプリと双方向 API）を検討する。 【後】
  2026-10-09、持ち主: これから TODO・システムモニター・エディターなどのアプリを作るので、そのときに決める。
  先に決めておくと安いもの（返答で提案した）: 別のアプリにパスを渡すのはコマンドライン引数（`filer <パス>`、`--line N`）を共通の形にする、
  設定の置き場所と名前の付け方を揃える、共有する部品はクレートにする。プロセス間の口（パイプ・ソケット）は 2 つのアプリが実際に話す必要が出てからでよい。

## リリースの macOS x64 ビルドがランナーを取れない

- [ ] `release.yml` の `macos-x64` が `macos-13`（Intel イメージ）を指していて、v0.47.10 の 【後】
  リリースでは **20 分以上 queued のままランナーが割り当てられなかった**（`runner_id: 0`）。
  他の 5 ターゲットは 5 分以内に終わっている。GitHub が Intel の macOS ランナーを縮小して
  いるため、**今後は取れないことが常態になる可能性がある。**
  - 影響は限定的: リリース自体は Windows ジョブが作るので公開され、残り 5 つの成果物も付く。
    欠けるのは `filer-vX.Y.Z-macos-x64.tar.gz` の 1 つだけ。
  - 直し方の候補:
    1. **`macos-14`（ARM）の上で `x86_64-apple-darwin` をクロスビルドする（推奨）。**
       Rust のクロスは `rustup target add` だけで通る見込みで、ランナーの奪い合いから降りられる。
       ただしリンクが通るかは実際に回すまで分からない。
    2. Intel の macOS ビルドをやめる。Apple Silicon への移行が進んでいるので、
       いずれこうなる。やめるなら README の「Platforms」から消すこと。
    3. そのままにして、取れた回だけ 6 つ目が付く状態を許容する。
  - **判断は人に任せる**（どれもリリースの成果物の構成を変えるので）。

## 環境・その他

- [ ] MCP サーバーの認証・接続 【人】
  - engineering 系（GitHub、Slack、Linear、Notion、Asana、Atlassian、Datadog、PagerDuty）は未認証。claude.ai のコネクタ設定か `/mcp` で認証する。
  - obsidian の MCP サーバーに接続できない（ECONNREFUSED）。Obsidian 側でサーバーが動いているか確認する。
  - 2026-10-09、持ち主が「やりたい」。開発のセッション（Claude）が外のサービスに届くためのもので、filer の機能ではない。
    kura などの uchmk のアプリを LLM とつなぐ話は「LLM との連携」の節。

## Linux レーン（v0.59.6）

- [ ] 【後】Linux レーンを再開する（今後の開発。`linux-role.md` の順番表は 2026-10-04 の時点のまま残してある。再開するなら Routine にするかも合わせて決める）
- [ ] 【後】Wayland は確かめられない（コンテナに合成器が無い）。`weston --backend=headless` が apt で入るなら、それで試す。

## 実機のセッションの報告（v0.65.4 で受けた分: #154、ARM64 の 1 節のマウス）

- [ ] **実行の途中でスクリーンセーバーが一度戻った**（所見 1）。`OpenInputDesktop` が `Screen-saver` を返した測りが 1 回あった。v0.64.3 の見張りは 【実機】
  5 秒おきに `*.scr` を止めるので、その窓に重なったのか、見張りが止まっていたのかは分からない。ログ（`During the run the watcher stopped …`）で
  回数を確かめ、間隔を 1 秒に詰めるか、`ES_DISPLAY_REQUIRED` を見張りの中でも出し直す。実機のセッションの側では、測る前後に入力デスクトップを
  確かめる手順が既にあり、それで誤った印は防げている。
- 罠として記録（filer の不具合ではない）: PowerShell 7.6 の `Write-Output` で書いた ANSI の色はペインに届かない（`[Console]::Out.Write` なら届く）。
  PSReadLine の履歴の予測が、打っていない文字をプロンプトの行に出す（「打ったもの」を行から読むと間違う）。

## 実機のセッションの報告（v0.67.7 で受けた分: #166、ARM64 の 17 節）

- [ ] 12 回に 1 回ほど、スクリプトの途中で `filer.exe` が消えた（所見 3）。`FILER_KEYS_DONE` も Windows のエラー報告も無く、その後の 4 回では再現しない。 【実機】
  印はどれもこの実行には頼っていない。もう一度出たら、`filer env` の記録と一緒に追う。

## 実機のセッションの報告（v0.67.13 で受けた分: #171、ARM64 の 20 節）

- [ ] **`--keys` の実行が 7 回に 1 回、何も残さずに消えた**（所見 2）。`<Shot:before>` のあと 9 秒でプロセスが無くなり、`FILER_KEYS_DONE` も 【実機】
  Windows のエラー報告も無い（#166 の所見 3 と同じ形）。v0.67.12 の見張りは「止まった」場合しか拾わないので、パニックのフック（提案 3）:
  `FILER_KEYS_DONE` の横に `<名前>.panic` を書き、0 でない終了コードで終わる。

## 実機のセッションの報告（v0.78.28 で受けた分: #262 x64 の v0.78.23 の再テスト、#263 ARM64 の 46 節）

- [ ] （v0.150.1 で `src/fs/git.rs` の `git.exe` 直起こしは `DETACHED_PROCESS` にした。残りは `cmd` 経由の外部プレビューなど。46.11a を実機で）`git` などの出力を読むだけの子プロセスを `DETACHED_PROCESS` で起こす（#263 の提案 1。`CREATE_NO_WINDOW` は窓を隠すだけで、`git.exe` ごとに `conhost.exe` が 1 つ立つ。
  spot を 6 回開いて `conhost` が 21 個）。`src/fs/git.rs`、`exec.rs`、`fs/ops.rs`、`preview/external.rs` の 4 か所。`cmd /S /C` の外部プレビューはコンソールが要らないかを確かめる【実機】（`cmd` を `DETACHED_PROCESS` で起こすと、その子の console アプリが新しい窓を作って
  ちらつくおそれがある。`git.exe` 直起こしだけなら足せるが、Linux では測れず、`conhost` の数と窓のちらつきは Windows でしか見えない）
- [ ] （所見）前の run（2026-10-04 12:02 の 37 節）が開いた Chrome のタブが ARM64 機に残っている（#263 の所見 1、#256 の所見 2 と同じ）。v0.83.2 から実機の run が閉じる（`.claude/windows-role.md` の「Earlier runs' leftovers」。下の #256 の項目と一緒に片付く）【実機】

## 実機のセッションの報告（v0.78.15 で受けた分: #251 x64 と #252 ARM64 の書庫の中）

- [ ] （所見）`<A-G>` の後の画像が 1 度だけ先頭のままだった（#251 の所見 3。状態ファイルは `preview top: 3984 of 3984`）。以後 7 回再現せず、画像も残っていない。
  再現したら `<Shot:>` を残すよう、x64 の順番表に書いておく【実機】

## 実機のセッションの報告（v0.75.20 で受けた分: #247 x64 と #248 ARM64 の 47.7）

- [ ] **x64 で `"vulkan"` の対照も回らなくなった**（#247 の所見 1）。#243 / #246 では約 10.0 CPU 秒だったのが 0.000（10 秒と 20 秒の 2 回）。
  違いは画面の倍率で、今回は 150%（#246 は 100%）。47.1 の空回りが倍率に依るのかもしれない。100% と 150% で `vulkan` を測り直す（#247 の提案）【実機】
- [ ] ARM64 で 25 節（25.11）は取れない（#248）。25.11 は「どこにも太字が無い」を要るが、`system_bold_fonts()` は `C:\Windows\Fonts\meiryob.ttc` などを
  絶対パスで見るので、普通の Windows ではその状態を作れない。33.9 は 5 つの箱が重ならないこと（画素でしか測れない）【実機】

## 実機のセッションの報告（v0.73.76 で受けた分: #233 x64 の 26.1 と 31 節、#234 ARM64 の 37 節と 32.5）

- [ ] ピッカーで、入っていないプログラムの項目に ` (not installed)` を付ける（#234 の提案 3）。`PATH`、`App Paths`、引用符の中のフルパスで 1 回だけ調べる。
  同じ README の例が、持ち主の 2 台で違う意味になっている。ピッカーを開くたびの費用を測ってから【実機】（`App Paths` のレジストリ引きの費用は Windows でしか測れない）
- [ ] 持ち主の ARM64 機で `.pdf` と `.html` に既定のアプリが無い（`HKCR\.pdf` に ProgId が無い。#234 の所見 1、3 回目）。filer の問題ではない。
  直すなら「既定のアプリ」で選び直す。2026-10-09、持ち主: Chrome でよい。Windows は既定のアプリをスクリプトから変えさせない（`UserChoice` のハッシュ）ので
  持ち主の手で: 設定 → アプリ → 既定のアプリ → Google Chrome → 「既定値に設定」（`.pdf` と `.html` が Chrome になったか、その下の一覧で見る） 【人】

## 実機のセッションの報告（v0.73.48 で受けた分: #217 ARM64 の 26 節、#218 x64 の 3 節）

- [ ] TESTING.md 26.8 / 26.10 を「起動役が動かない」（今も合格）と「関連付けが壊れている」（Q61 待ち）の 2 行ずつに分ける（#217 の所見 2）。 【QA】
  **前提が古い**（v0.75.16、QA の子）: Q61 は v0.73.50 で入り、今は `ShellExecuteExW` を直接呼ぶので、置き換えの `powershell.exe` で
  「起動役が動かない」を作る測り方が成り立たない。関連付けが壊れた状態を実機でどう作るか（`UserChoice` はハッシュで守られている）を先に確かめる。【実機】

## 実機のセッションの報告（v0.67.20 で受けた分: #176、ARM64 の 29 節）

- [ ] **`say()` が標準出力への書き込みに失敗したとき、黙って捨てずにコンソールへ回す**（#183 の提案 2）。今は `let _ = writeln!` で戻る。 【実機】
  まず、パイプの最後に置いたときに書き込みが本当にエラーを返しているのかを Windows で測る（#183 は測っていない）。返していなければ
  この直しは効かないので、その結果を書いて閉じる。

## 実機のセッションの報告（v0.72.4 で受けた分: #194、x64 の再テスト）

- [ ] **CI の Windows の `test` が 1 回、`cargo test` の中で 30 分以上止まった**（2026-10-03、#194 の CI、run 37088219242）。同じコードは main で
  4 分で通り、取り消して流し直すと 5 分で通った。取り消した実行のログは取れなかった（404）。また起きたら、取り消す前に
  `cargo test -- --test-threads=1` などで止まるテストを特定する。止まり続けるテストがあれば、それは不具合。 【後】（また止まるまで調べようがない）

## 巨大ログのプレビュー（v0.133.0、#454）
