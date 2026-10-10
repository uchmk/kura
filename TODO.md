# TODO

済んだ節（未完了の `- [ ]` が 1 つも無い節）は [TODO-DONE.md](TODO-DONE.md) に移してある（v0.73.15）。
節の中で済んだ項目は、その節が全部済むまでここに残る。

**行の末尾の印は、その項目を進めるのが誰か**（v0.73.15）。印の無い未完了の項目が、開発のセッション
（対話でも自動実行モードでも）の取る分で、印の付いた項目は飛ばす:

- `【人】` 持ち主が手を動かすか決めること（機械の設定、ソフトを入れる、リリース、好み）
- `【QA】` TESTING.md の文言や、テストの探索など、QA セッションの分（`.claude/qa-role.md`）
- `【実機】` Windows の実機でしか測れないこと。順番表（`.claude/windows-role.md`）に積んで待つ

- [x] （v0.86.13。49.10 を `claude` だけにし、`cmd` / `bash` の後半を 49.10a に分けた）（#307・#308 の提案）TESTING.md の 49.10 を書き直す: PSReadLine は pwsh で `<S-Enter>` を `AddLine` に結ぶので「kitty を求めないシェルで変わらない」の後半は通らない。前半（`claude` で 2 行）と後半（`cmd` か `bash` で測る）に分ける。`【QA】`
- [x] （v0.86.17）（#307 の提案 1）TESTING.md の 49.1 に「コマンドの間に 13 行以上出力する（ペインは 12 行）」を書く。`【QA】`
- [x] （v0.86.24。1.43 と 40.16 を書き直し、再テストに積んだ）（#308 の提案 2）TESTING.md の 1.43 / 40.16 の `<C-c>` を 49.9 に寄せる（`<C-c>` は v0.86.0 から素の `\x03`、`(Ctrl+c)` の記録は出ない）。1.43 は `(Alt+b)` と `(Up)` だけにする。`【QA】`
- [x] （v0.86.8）（#307 の提案 3・5、#308 の提案 3）`--keys` に生バイトの貼り付けと `<Clip:>` の状態、クリックとホバーの手順を足す（49.3 / 49.6）。`<Wheel:N>` と `<Drag:…>` の例を README のキー表に書く。
  - [x] （v0.86.5。`<Click:C-0.5,0.4>` の修飾キーと `<Hover:x,y>` を足し、README に書いた。`<Wheel:>` / `<Drag:>` の例は README に既にある）
  - [x] （v0.86.8。`<Paste:text>` と、`<State:>` の `cursor icon:` を足した。クリップボードの中身は `clipboard set:` の行が既に出しているので `<Clip:>` は足さない。持ち主のクリップボードを `<State:>` が読まないため）
- [ ] （#308 の所見）`wezterm imgcat` で 2560x1280 の PNG が何も描かれない（160x80 は描かれる）。tsumugi 側で調べる。【pane】
- [x] （v0.86.26。済んだ行は合流側の #311 / #312 の整理で外れていた。待ち一覧に残っていた 16.3b を外し、ほかの済んだ行の一覧に足した）（#307・#308 の提案、マージ担当は `.claude/` に書けないのでここに積む）`.claude/windows-role.md` の x64 と ARM64 の両方の「Re-tests of changed behaviour」から、済んだ 25.4e、49.9、49.11 を外す（x64 は #307、ARM64 は #308 で `[x]`）。残りは 49.10（行の書き換え待ち）、49.6（マウスが要る）、1.43 / 40.16（書き換え待ち）、1.44 / 16.13 / 16.13a / 33.7（直し待ち）。次の run は ARM64 が節 1・19・40 のホイール・ドラッグ・nvim の行を押し直す。`【人】`
- [ ] （#306 の所見）49.7 が x64 で落ちた: 同梱の ConPTY（1.24.2607.10001）が OSC 8 の始まりと終わりを落とし、ペインには `click me` しか届かない（`FILER_PTY_LOG`）。sixel（`chafa -f sixels`）も DCS を落とされて本文が文字で出る。filer か tsumugi が ConPTY にパススルーを頼めるか調べる。【pane】
- [ ] （#306 の所見）`wezterm imgcat` がペインの中で `attempt to divide by zero` で panic する（`wezterm\src\main.rs:383`）。セルのピクセル寸法が 0 で返っている疑い（`CSI 16 t` / `CSI 14 t` の返事、`TIOCGWINSZ`）。49.11 と同じ点。【pane】
- [x] （v0.86.29。49.7 は ConPTY の限界を前書きにし、49.8 は OSC 1337 を名指しにした）（#306 の提案 2）TESTING.md の 49.7 を ConPTY の落とす OSC 8 に合わせて書き直し、49.8 は `chafa -f sixels` でなく通る iTerm2 の OSC 1337 を名指しにする。【QA】
- [ ] （#305 の提案 1）`scripts/make-fixtures.ps1`（と `.sh`）に `outline.docx`（見出し 1 / 2 / 1 と本文 2 段落の最小 OOXML）を足す。16.3 / 16.3a / 16.3b が要る。【実機】
- [x] （v0.86.26。16.3b・37.8b・49.5 は一覧から外した）（#305・#306 の提案、マージ担当は `.claude/` に書けないのでここに積む）`.claude/windows-role.md` の x64 と ARM64 の両方の「Re-tests of changed behaviour」から、済んだ 16.3b、37.8b、49.5 を外す（16.3b は #305 / #306 で `[x]`、49.5 と 37.8b は x64 で `[x]`）。ARM64 の表には 37.8b・49.5 を ARM64 の二台目として足してよい。`【人】`
- [x] （v0.85.7。`pane title:` の行を足した）`<State:>` に `pane title:` の行（tsumugi-pane が返すタイトル。空でも出す）を足す。1.35 で確認ダイアログの文言は読めるが、v0.85.3 が変えた「タイトルが空か」は読めず、x64 と ARM64 を同じ行で比べられない（#300 の提案 1）。
- [x] （v0.86.19。`scripts\mcp.ps1` を足した: `filer mcp` を起動して `initialize` から `tools/call`（`-Tool` / `-Arguments`）か `tools/list`（`-List`）まで流し、結果を JSON で出す。`-Address` は子プロセスだけの `FILER_ADDRESS`。Linux で書いたので Windows では未実行、最初のレーンが 50.1 / 50.2 で一度確かめる）（#302・#303 の提案）`scripts\mcp.ps1`（`-Address … -Tool filer_state`）を足し、`filer mcp` に JSON-RPC を流す道具にする。ARM64 は `Start-Mcp` / `Rpc` / `Call` を scratch に書いて `$args` の衝突で 1 回余計に回した。`【実機】`
- [x] （v0.86.33。前書きに `FILER_ADDRESS` と `claude -p` の `init` の読み方を書いた）（#302・#303 の提案）TESTING.md の節 50 の前書きに `FILER_ADDRESS`（持ち主の窓と扉を奪い合わない）と `claude -p --mcp-config … --output-format stream-json` の `init` で 50.1 の `/mcp` を読めることを書く。【QA】
- [x] （v0.85.9。`mcp:` の行を足した）`<State:>` に `mcp:`（扉の名前か `off`）の行を足す。「窓が無い」と「`enable = false`」を 50.4 / 50.5 で見分けるには `config:` の行から推すしかない。
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
- [x] （v0.99.1。`keys::display_seq` が macOS で `<C-q>` を `⌘Q`、`<C-A-x>` を `⌥⌘X` と出し、ヘルプ・which-key・パレットの右端・キーを案内するトースト・オーバーレイの案内が使う。照合は今までの `render_seq`）macOS のヘルプのキーの名前を人の読む形（`⌘Q` など）にする。
  - [ ] 設定の画面の説明文に固定で書いた `Ctrl+=` `Ctrl+F5` `Ctrl+T` を、macOS では `⌘` で出す。macOS の目での確認は TESTING に積む【後】
- [ ] 窓の位置と大きさを覚える処理を ito に寄せる（tsumugi と yagura にも同じものが要る）【人】（ito リポジトリに新しい部品を足す作業で、このセッションの GitHub 権限は kura だけ。ito 側で `ito-prefs` 等に足してから、kura の `rev` を上げる）
- [ ] 15.10・15.11（倍率がほかのアプリに伝わる）は tsumugi と yagura が common.toml を読むようになってから確かめる【実機】

## 多言語対応（2026-10-10、持ち主の希望）

持ち主は filer も後で多言語にしたい。言語は uchmk のアプリ（filer・tsumugi・mimamori）で共通の `<設定のフォルダ>/uchmk/common.toml` の
`language = "auto" | "en" | "ja" | …` で選び、アプリごとの設定に `language` があればそちらが勝つ。`auto` は OS の言語（`sys-locale`）。
仕組みは先に mimamori の `src/i18n.rs`（`tr("key")` / `trf("key", &[..])`、英語の表は `src/lang/en.toml`）で作った。

- [ ] mimamori の `i18n` を tsumugi の workspace の共有クレートに切り出す（tsumugi を `access:"push"` で付け直す。tsumugi の設定画面の言語の欄 `prefs.rs` もつなぐ）。【後】
- [ ] filer の画面の文字（ヘルプの見出し・トースト・オーバーレイ・ステータスバー・確認の文）を `tr()` に通し、英語の表を足す。
  yazi の keymap の `desc` は利用者の設定から来るので訳さない。表に無いキーはテストで落とす。【後】
- [ ] 日本語の表を足す。CJK のフォントは tsumugi の `fonts.rs` の取り方を使う。filer.toml に `language` を足し、README に書く。【後】

## テストと QA の運用を軽くする（2026-10-11、持ち主の判断）

x64 を唯一のメインの機械にし、ARM64 はリリースのときだけ arch に関わる行を押す（`cargo test` と起動の確認は GitHub の ARM64 ランナー）。
所見・バグ・確認事項は GitHub Issues に移し、共通の道具は ito に置く。段階ごとに 1 つの版で出す。

- [x] （v0.101.0）段階 0: 止まったレーンが外から見え、自分で立ち直る。タスクの入口を `scripts/lane-boot.ps1`（main の写しを動かす）にし、
  汚れた作業フォルダは `rescue/` ブランチに救出し、Merge lanes が古い状態の issue に `lane-stalled` を付ける。
- [x] x64 の `filer-auto-wintest` を `lane-boot.ps1` で登録し直す（2026-10-11、持ち主の PC のセッション）
- [ ] ARM64 の `filer-auto-wintest-arm` を `lane-boot.ps1 -Lane arm` で登録し直す（`auto-wintest.ps1` の冒頭の手順）。
  登録し直すまでは古い `-Command` が作業フォルダの写しを動かす。段階 3 で毎時のタスクを外すなら不要【人】
- [ ] わざと汚した作業フォルダから起動し、`rescue/` ブランチができること・状態の issue にコメントが付くこと・次の :20 で PR が出ることを見る【実機】
- [ ] tsumugi の :50 のタスクにも `lane-boot.ps1` と救出を写す（段階 5 で ito に一本化する）【後】
- [x] （v0.101.1）段階 1: 片付け（TODO.md の済んだ「実機のセッションの報告」を TODO-DONE.md へ、`auto-todo.sh` の削除、ci.yml の重複した `--check`、
  windows-role.md の再テストの履歴、qa-reports/ を archive に。
  dev-routine.md は CLAUDE.md が変わったときだけ読み直す）
- [ ] 段階 2: ARM64 を CI に移す（`windows-11-arm` と `ubuntu-24.04-arm` で `cargo test`、ARM64 の zip の起動の確認、リリースの条件に）
- [ ] 段階 3: 順番表を 1 つにし、ARM64 はリリースのときだけ（`TESTING-ARM64.md`、`-Release`）。ARM 機の毎時のタスクを外すのは【人】
- [ ] 段階 4: 所見・再テスト・確認事項を Issues に移す（`scripts/labels.sh`、`.github/ISSUE_TEMPLATE/`、merge-lanes.py の票の集計）
- [ ] 段階 5: 共通の道具を ito に移す（`ito-testcheck` と `scripts/lanes/`）。ito をセッションに付けてから【後】
- [ ] 段階 6: tsumugi・yagura に展開する【後】

## マージで止めている実機の PR

マージのワークフローか Routine が止めた実機の PR。ここにある `#N` はワークフロー（`scripts/merge-lanes.py`）がマージしない。
持ち主の答えがここに書かれたら、Routine がその回のうちにそのとおりにする（`.claude/merge-role.md` の 3）。

いまは無い

## ARM64 の再テストの所見（#295）

- [ ] （#295 の所見 1）16.13 / 16.13a が ARM64（build 28000）で失敗する: `has_default_app`（`src/exec.rs`）は `AssocQueryStringW(ASSOCSTR_EXECUTABLE, ".xyz")` の成功を「アプリあり」と見るが、この機械では関連付けの無い拡張子でも `OpenWith.exe` を返し、`start` が「アプリを選ぶ」の箱を出す。x64（v0.82.0 で通った）との差。`OpenWith.exe`（と `rundll32.exe` 系）を「アプリ無し」に数える案。x64 でも同じ値が出るか先に確かめる【実機】
- [x] （v0.85.1。トーストの 1 行目のホームフォルダを `~` にした）（#295 の所見 2）設定ファイルが長いパスにあると警告のトーストが 3 行に折れてプレビューの先頭を覆う（33.7 の「1 行」に反する）。1 行目のパスを `~` や相対に短縮してから折る

## Sonnet の見張り（2026-10-05 まで）

- [x] （Sonnet の見張り）v0.79.4（`f3b0910`）は「Ended the shell」が同じシェルの `Started …` のトーストを差し替えるようにして 1.38 を外し、両レーンの再テストに積んだが、TESTING.md の 1.38 の行（と `scripts/testcheck-ja.toml` の訳）は前のままで、新しい期待（終えた後に `Started pwsh …` のトーストが残らず、**Ended the shell** だけが出る）は順番表の行にしか書かれていない。再テストが済んで順番表から消えると、行の文からはこの動きが読めず、戻っても次の回が気づかない。
  1.38 の期待に「`<C-t>` で始めてすぐ `<C-S-t>` で終えても、`Started …` のトーストは残らず **Ended the shell** だけが見える（v0.79.4、#280 の提案 2）」を足し、訳を合わせ、`make-testcheck` で作り直す。再テストは既に積んである
- [x] （v0.80.4。4 本を書き、`sb.ps1` は #235 の名前の `scripts/read-statusbar.ps1` にした。どれも冒頭に Windows で未実行と書き、順番表の 45.11 と「48.2 / 48.6 at each release」の行、TESTING.md の 32.9a と 48 節の前書きが名指す）（Sonnet の見張り）v0.78.236（`38a9171`）は残りの 5 項目に印を付けて「開いている項目は 0」としたが、うち 2 つの `【実機】` は印の意味（Windows の機械でしか測れないもの）に合わない。`scripts/verify-release.ps1`（「PowerShell は Windows でしか確かめられない」）は、v0.78.233 の `scripts/keys.ps1` と同じく書いて「Linux で動かせず未実行」と書けば進められ、確かめは実機の回が初めて使うときでよい。`sb.ps1` / `run48.ps1` / `fx45.ps1` を `scripts/` に置く項目（「元のスクリプトは機械の証拠フォルダにしかない」）も、ARM64 のレーンが既にそうしたように、45.11・48.6 と順番表の行の文から書ける。
  2 つの項目の `【実機】` を外して書く（`sb.ps1` はサクラのステータスバーを読むので、行の文で足りなければその 1 本だけ `【実機】` に残す）。スクリプトの冒頭に「Windows で未実行」と書き、順番表の該当の行が `scripts\…` を名指すようにする。filer の動きは変わらないので再テストは要らない
- [x] （v0.78.233。結果でないとき exit 1、時間切れは exit 2 にし、冒頭に書いた。PowerShell は Linux で動かせず未実行）（Sonnet の見張り）v0.78.230（`0c59bd7`）は `scripts/keys.ps1` の項目（#229 の提案 2）を「v0.78.135 のスクリプトが満たしている」として済みにしたが、項目が求めた「`keys: done` でなければ非 0 で返す」をスクリプトはしていない（done ファイルが無いとき・最後の行が `keys: done` でないとき・時間切れのとき、`Write-Warning` を出して `$Out` を返すだけで、終了コードは 0）。呼ぶ側（実機の回、`count-vanishing.ps1` のような繰り返し）は警告の文を読まないと「結果でない」回を見分けられない。
  結果でない 3 つの場合に `$Out` を出したうえで `exit 1`（時間切れは `exit 2` など分けてもよい）にし、冒頭のコメントに終了コードを書く。filer の動きは変わらないので再テストは要らない
- [x] （v0.78.229。`<Shot:>` の数から数え、足りない回に印を付けた）（Sonnet の見張り）v0.78.227（`0e386af`）の `scripts/count-vanishing.ps1` は、キーの列に `<Shot:p1>` `<Shot:p2>` `<Shot:p3>` `<Shot:txt>` の 4 つがあるのに、最後の表の `png` の行が `(3 per run expected)` と言う（1 回の実行で png は 4 枚できる）。実機の回が 80 / 20 回を見て「多い」と読むか、3 枚の回を見落とす。
  `(4 per run expected)` に直す（`<Shot:>` の数から数えるならなお良い）。あわせて、`Png` の列が 4 でない回を表で目立たせると、描けずに終わった回が分かる。動きは変わらないので再テストは要らない
- [x] （v0.78.222。`compare_tree_lines` に切り出し、`shown()` の順に出して `, matches hidden` を足し、単体テストを足した）（Sonnet の見張り）v0.78.220（`972954f`）の `src/main.rs` の `state_report` は、フォルダ比較の `compare tree row i:` を `rows` の全部から出し、`z`（`DiffOverlay::hide_same`）で隠した行も並べる。項目のもとの #255 の提案 2 と Q91 の「`z` で何行残るか」を文字で読む目的には、画面に見えている行（`DiffOverlay::shown()`）が要る。今は `z` を押しても状態ファイルが変わらない。
  `compare tree:` の行に `, matches hidden` を足し（`hide_same` のとき）、行は `shown()` の順に出す。あわせて、`compare_row_line` と同じく行を組む部分を小さな関数に切り出し、4 つの状態とフォルダの末尾の `/`、`hide_same` で `same` の行が抜けること、`truncated` の末尾を試す単体テストを足す（今は単体テストが無い）。状態ファイルは `--keys` のためのもので、実機の行の動きは変わらないので再テストは要らない
- [x] （v0.78.214。`backend_row` に切り出し、`auto` 以外で末尾が付かないテストを足した）（Sonnet の見張り）v0.78.212（`e7a2ed4`）は v0.78.209 の項目を済みにしたが、項目が求めた 3 つのうち「`auto` 以外で何も付かない」は試していない。`auto_note` のテストは `"Gl"` と空の 2 つだけで、末尾を付けるかを決める `matches!(cfg.ui.backend_name(), Ok(None))` は `src/envreport.rs` の `config()` の中に残っている。
  `[ui] backend = "gl"` の `Config` で `config()` の `Backend` の行に `(this machine:` も `(wgpu's own pick here)` も付かないことを試すテストを足す（`WGPU_BACKEND` に左右されないよう、環境変数を見ない形に切り出すか、変数が無いときだけ assert する）。動きは変わらないので再テストは要らない
- [x] （v0.78.212。25.4f を足し、`auto_note` に切り出してテストした）（Sonnet の見張り）v0.78.209（`37e856d`）は `filer env` の `Backend` の行を変えた（`auto` のとき末尾に `(this machine: Gl)` / `(wgpu's own pick here)`）が、TESTING.md に行が無く、`scripts/testcheck-ja.toml` の訳も、`windows-role.md` の 2 つの再テストも無い（CLAUDE.md の作業ルール）。単体テストも無い。
  25 節に「`[ui] backend` を書かない（`auto`）で `filer env`: `Backend` の行は `[ui] backend = "auto" (this machine: Gl)`（GL のある Windows）。`WGPU_BACKEND` を入れたとき・`"gl"` などを書いたときは末尾が付かない」の行（25.4f など）を足し、訳を合わせ、`make-testcheck` で作り直し、両レーンの再テストに積む。
  末尾の文を作る部分を `predicted_fallback` の結果を引数に取る小さな関数に切り出し、`"Gl"` で `(this machine: Gl)`、空で `(wgpu's own pick here)`、`auto` 以外で何も付かないことを試すテストを足す
- [ ] （Sonnet の見張り）v0.78.209 は、項目の「GL を確かめるのに起動 1 回分の手間がかかるので、測ってから」を Linux の 11 ms で済みにしたが、`predicted_fallback` は `auto_backends(cfg!(windows), …)` なので **Windows 以外では GL の確かめ（`has_adapter`）を呼ばない**。測ったのは確かめの無い経路だけ。
  しかも前は `[ui] backend` の警告があるときだけ呼んでいたのが、既定の `auto` で毎回呼ぶようになった。x64 と ARM64 で `Measure-Command { filer env --out x.txt }` を v0.78.208 と v0.78.209 で比べ、目に見えて遅ければ確かめを警告のあるときに戻すか、前の起動の `last-run.toml` の Adapter から言う形にする。【実機】

- [x] （v0.78.207。46.17b を足し、訳と再テストも）（Sonnet の見張り）v0.78.205（`d0414b8`）は spot の `C` を、2 つ以上選んでいるときに選んだファイルごとのパネルを `#### <パス>` で並べて写すようにした（動きが変わった）が、TESTING.md に行が無く、`scripts/testcheck-ja.toml` の訳も、`windows-role.md` の 2 つの再テストも無い（CLAUDE.md の作業ルール）。46.17 は 1 つのパネルの形しか言っていない。
  46 節に「2 つ以上選んで spot を開き `C`: クリップボードは選んだファイルごとに `#### <フルパス>` の行、その下に 46.17 の形のパネル、ファイルの間は空行 1 つ。トーストは `Copied the spot panels of N files`。1 つだけ選んでいるとき・選んでいないときは 46.17 のまま」の行（46.17b など）を足し、訳を合わせ、`make-testcheck` で作り直し、両レーンの再テストに積む
- [x] （v0.78.207。パスから作り、単体テストを足した）（Sonnet の見張り）v0.78.205（`d0414b8`）の `src/app.rs` の `copy_spot_of_selection` は、写すファイルを `tab.current.entries` から `selected` に含まれるものだけ拾って作るが、`selected` はフォルダを移っても残る（`Tab::targets` はそのまま全部を返す）。フォルダ A で 2 つ選んでから B へ移って spot の `C` を押すと、0 件のまま `Copied the spot panels of 0 files` と出てクリップボードが空になる（A と B の両方で選んでいれば B の分だけ）。
  `selected` の各パスから `Entry::from_path` で作る（読めないものは飛ばす）ようにし、今いるフォルダの外のファイルも入ることを試す単体テストを足す。直したら上の項目の 46.17b にも「別のフォルダで選んだファイルも入る」を書く
- [x] （v0.78.204。`unknown.xyz` を両方のスクリプトに足し、HKCR\.xyz の断りを書いた）（Sonnet の見張り）v0.78.202（`1e6eb7d`）が足した TESTING.md 16.13 は `fixtures\unknown.xyz` を使うが、`scripts/make-fixtures.ps1` はそのファイルを作らない（`xyz` も `unknown` もスクリプトに無い）。CLAUDE.md の作業ルールは「テスト用ファイルは `scripts/make-fixtures.ps1` が作る」で、実機の回は 16.13 を押す前に自分で作るか、行を飛ばすことになる。
  `make-fixtures.ps1` に関連付けの無い拡張子のファイル `unknown.xyz`（中身は 1 行のテキスト）を足し、`make-fixtures.sh` も揃える。`HKCR\.xyz` がある機械では 16.13 が成り立たないので、スクリプトか 16.13 にその断りを書く。動きは変わらないので再テストは要らない
- [x] （v0.78.199。テストの doc コメントを入れ替えた）（Sonnet の見張り）v0.78.197（`b7070c3`）の `src/ui/overlay.rs` の `tests` で、足したテスト `the_spot_thumb_shows_only_when_lines_overflow` が、`the_spot_key_column_fits_its_widest_key` の doc コメント（`/// #214: the value column moves out past the widest key …`）とその関数の間に入り、コメントが新しいテストに付いている（v0.78.185 の 33.23 の項目と同じ取り違え）。
  新しいテストを doc コメントの前に移し、必要なら自分の doc コメント（`/// #224: …`）を付ける。動きは変わらないので再テストは要らない
- [x] （v0.78.196。`Paste` / `Link` / `Hardlink` を書き換える側へ移し、単体テストと 21.19 の `r` `p` を足した）（Sonnet の見張り）v0.78.193（`1557d87`）の `src/app.rs` の `archive_refusal` は、`Act::Paste`・`Act::Link`・`Act::Hardlink` を「まだ無いだけ」（`not available here yet`）に分けているが、どれも書庫の中に物を作る、つまり書庫を書き換える操作で、`filer never writes` の側のはず（`not available here yet` は、いつか書庫に貼れるようになると読める）。
  `Paste` / `Link` / `Hardlink` を書き換える側へ移し、`archive_refusal` に単体テスト（`x` `d` `a` `r` `p` が `would change the archive`、`Extract` などが `not available here yet`）を足す。TESTING.md 21.19 は `x` `d` `a` しか押さないので、`p` も押して同じ文が出ることを足し（`scripts/testcheck-ja.toml` も）、`make-testcheck` で作り直し、両レーンの再テストの 21.19 の説明を合わせる
- [x] （v0.78.192。注記を `exec::no_line_note` に切り出し、Word で付き `code -g` と行なしで付かないことを試すテストを足した）（Sonnet の見張り）v0.78.190（`285321a`）は v0.78.188 の注記の項目を済みにしたが、足したテストは `exec::at_line("winword %s")` が `None` を返すことだけで、項目が求めた「注記が付く／付かない」は試していない。
  注記を組むのは `src/app.rs` の `open_at` の中（`(line {n} not passed: this opener takes no line)`）で、ここが消えたり、行を受け取るエディタでも付いたりしても CI は気づかない。
  注記の文を作る部分を小さな関数（`exec` か `app` の中、`line` と `at_line` の結果から文を返す）に切り出し、`winword %s` で付き、`code -g %s`（`at_line` が `Some`）と `line` が `None` のときに付かないことを試す
- [x] （v0.78.190。16.3b を足し、両レーンの再テストに積み、`winword` が行を受け取らない単体テストを足した）（Sonnet の見張り）v0.78.188（`6c5b5c5`）は、行つきで開いたとき（アウトラインの `<Enter>`）オープナーが行を受け取れなければトーストに `(line N not passed: this opener takes no line)` を足すようにした（動きが変わった）が、
  TESTING.md に行が無く、両レーンの再テストにも積んでいない（CLAUDE.md の作業ルール）。単体テストも無い。16.3（`<Enter>` が見出しへ飛ぶ）はこのトーストを言っていないので、押しても変わった動きは通らない。
  16 節に「Word で開くアウトラインの `<Enter>` は、トーストの末尾に `(line N not passed: this opener takes no line)`。行を受け取るエディタ（`code -g` など）では付かない」の行（16.3b など）を足し、
  `scripts/testcheck-ja.toml` の訳も合わせ、`make-testcheck` で作り直し、`windows-role.md` の 2 つの再テストに積む。`exec::at_line` が `None` を返すオープナーで注記が付き、返すもので付かないことを試すテストも足す
- [x] （v0.78.187。33.23 を足し、両レーンの再テストに積み、doc コメントを戻した）（Sonnet の見張り）v0.78.185（`15079d0`）はヘルプのパネルの警告の行を折り返すようにした（`~` の画面の見え方が変わる）が、TESTING.md に行が無く、両レーンの再テストにも積んでいない（CLAUDE.md の作業ルール）。
  33 節に「パネルより長い警告（33.22 の `[[mgr.keymap]]` の警告をパネルを狭めて出すなど）は右端で切れず、字下げして次の行に続く。`C` で写した文は 1 行のまま」の行（33.23 など）を足し、`scripts/testcheck-ja.toml` の訳も合わせ、`make-testcheck` で作り直し、`windows-role.md` の 2 つの再テストに積む。
  あわせて `src/ui/overlay.rs` の `help_config_rows` で、足したテスト `a_warning_wider_than_the_panel_continues_on_the_next_row` が `a_long_warning_takes_a_row_per_line` の doc コメント（「A warning of several lines is several rows …」）とその関数の間に入り、コメントが新しいテストに付いている。新しいテストをコメントの前に移す
- [x] （v0.78.180。マージの回が両レーンの再テストに 46.19 / 46.20 を積んだ）（Sonnet の見張り）v0.78.179（`931161d`）は spot の Git の節で `remote.origin.url` と `origin/HEAD` を `.git` ごとに覚えるようにした。
  覚えた値を捨てる時（`git remote set-head origin -d` / `-a` の後）を確かめる 46.19 / 46.20 を再テストに積んでいなかった（この項目の親が「46.20 の再テストは set-head の後にもう一度見る」と書いていた）
- [x] （v0.78.181。`the_remembered_default_branch_follows_set_head` を足した）（Sonnet の見張り）v0.78.179（`931161d`）の `src/fs/git.rs` の `remote_info` に単体テストが無い。CI は覚えた値が古いまま残っても気づかない。
  一時リポジトリ（`util::test_dir`）で `origin` を足して `not_merged_into` を聞き、`git remote set-head origin -d` の後にもう一度聞いて `unknown` に変わること、
  `-a` で戻ることを試すテストを足す（`git` が無い環境では飛ばす）
- [x] （v0.78.178。46.17 の文言と訳を直し、`[x]` を外して両レーンの再テストに積んだ）（Sonnet の見張り）v0.78.176（`e2ad2b5`）は spot の `C` の出力を変えた（`Came in via` の `#42` と `bb23a4e` の間が 2 つの空白から TAB に）が、それを受け持つ TESTING.md 46.17（`Label<TAB>value`）を直さず、TESTING-CHECKS.md の 46.17 の `[x]` も外さず、両レーンの再テストにも積んでいない（CLAUDE.md の作業ルール）。46.17 に「`Came in via` は `Came in via<TAB>#n<TAB>hash` の 3 列になる（v0.78.176）」を足し、`scripts/testcheck-ja.toml` の訳も合わせ、`make-testcheck` で作り直して 46.17 の `[x]` を外し、`windows-role.md` の 2 つの再テストに 46.17 を積む
- [x] （v0.78.169。`$seenKnown` に取ってから条件に使った。この環境に pwsh が無く ParseFile は回せていない）（Sonnet の見張り）**急ぎ。**v0.78.167（`40163e9`）が足した `scripts/auto-wintest.ps1` の 339 行目 `if (-not $Force -and $seen -and (git -C $Work cat-file -e "$seen^{commit}" 2>$null; $LASTEXITCODE -eq 0))` は、`( )` の中に `;` で 2 つの文を置いていて、PowerShell の構文エラーになる（`[System.Management.Automation.Language.Parser]::ParseFile` で 7 件。最初は `339: Missing closing ')' in expression.`。pwsh 7.4.6 で確かめた）。
  スクリプト全体が読めないので、両レーンの実行が止まる。しかもスクリプトは自分で作業フォルダを `origin/main` に合わせるので、壊れた版が作業フォルダに入った後は、直した版を自分では取りに行けない。
  `( … )` を `$( … )` にする（または `git … cat-file` を前の行で回して `$LASTEXITCODE` だけを条件にする）。直したら ParseFile でエラー 0 を確かめ、CI か `scripts/verify.sh` で PowerShell の構文を検査する手を足すかも考える。
  持ち主には、直った版が `main` に入った後で `git -C C:\dev\filer-wintest checkout --detach origin/main`（ARM64 は `filer-armtest`）を手で回してもらうよう通知した
- [x] （v0.80.6。`4764141` が v0.78.98 の修正であることを確かめ、TESTING.md と訳を直して作り直した。`[x]` はそのまま）（Sonnet の見張り）v0.78.163（`765ee72`）は訳の版を TESTING.md の行に合わせたが、6.17 の訳を `（v0.78.98、#270）` から `（v0.78.96、#270）` に変えている（CHANGELOG にも 6.17 は書いていない）。
  走っている間に `0s` と出さない修正が入ったのは v0.78.98（`4764141`）で、v0.78.96 は記録だけの版なので、間違っているのは TESTING.md 6.17 の `(v0.78.96, #270)` のほう。
  TESTING.md 6.17 と訳を `v0.78.98` にし、`make-testcheck` で作り直す（動きは変わらないので `[x]` はそのまま）【QA】
- [x] （v0.78.154。47.6 / 47.8 の文言を直し、`[x]` を外して両レーンの再テストに積んだ）（Sonnet の見張り）v0.78.148（`4bd1ed4`）は `filer env` の `Adapter` の行の末尾を `(Gl, …; set by [ui] backend = "gl")` に変えた（`backend_setting` は既定でも `[ui] backend = "auto"` が入るので、どの起動でも付く）が、TESTING.md に行が無く、再テストにも積んでいない（CLAUDE.md の作業ルール）。
  47.6 と 47.8 は「`Adapter` の行が `(Gl, …)` で終わる」と書いていて、文字どおりにはもう合わない。47.6 / 47.8 の文言（`… (Gl, …; set by [ui] backend = "gl")`、`WGPU_BACKEND` が勝ったときは `set by WGPU_BACKEND=…`）と `scripts/testcheck-ja.toml` の訳を直し、TESTING-CHECKS.md の両行の `[x]` を外して、両レーンの再テストに積む
- [x] （v0.78.149。23.6 に近い名前へ置く 1 文を足し、両レーンの再テストに積んだ）（Sonnet の見張り）v0.78.146（`26a0e3a`）は、打った名前が無くて親を開いたときのカーソルを一番近い名前に置くように動きを変えたが、TESTING.md に行が無く、再テストにも積んでいない（CLAUDE.md の作業ルール）。
  23.6 は「親が開き、トーストが出る」までで、カーソルの位置を言っていない。23.6 に「カーソルは `typo` の上（近い名前が無ければ先頭）」を足すか 23.6b を足し、`scripts/testcheck-ja.toml` の訳も合わせ、両レーンの再テストに積む（`<State:x>` のカーソルの下で読める）
- [x] （v0.78.133。飛ばしたトーストに `(skipped …: not found)` を足し、32.21・訳・再テストに書いた。Q87 は反映済み）（Sonnet の見張り）v0.78.128（`0601e1e`）は Q87 を多数決の選択肢 1 で入れたが、選択肢 1 の「トーストに `(skipped no-such-tool: not found)` のような 1 句を足す」が無い
  （`src/app.rs` は飛ばす先を変えただけ。それでは選択肢 3）。飛ばしたときのトーストに 1 句を足し、32.21 の行と訳にも書き、両レーンの再テストに積む。QUESTIONS.md の Q87 は `反映済み（多数決）` にする。
  また、32.21 の行を変えたのに両レーンの再テストに積んでいなかった（マージの回が v0.78.130 で積んだ）
- [x] （v0.78.126。13.12 に、プレビュー欄が `not found, os error 2` と言うことを 1 文足した）（Sonnet の見張り）v0.78.120（`e333a41`）は壊れたリンクのプレビューの誤りを `Resolves` と揃え、13.12 の `[x]` を外して再テストに積んだが、TESTING.md 13.12 の行は `Resolves` のことしか書いておらず、
  プレビュー欄が `not found, os error 2` と言うこと（`access denied, os error 5` ではないこと）が行に無い。再テストの説明（`windows-role.md`）にだけある。13.12 に 1 文足すか 13.12b を足し、`scripts/testcheck-ja.toml` の訳も合わせる
- [x] （v0.78.114。TESTING.md に 32.22 を足し、訳・再テストに積んだ）（Sonnet の見張り）v0.78.111（`854edd1`）が Windows で `;` を演算子に数えなくした（Q85）が、TESTING.md に `;` の行を確かめる行が無い。
  32.20 は `;echo hello >out.txt`（`>` で止まる）と `;echo %*` だけなので、両レーンの再テストに積まれた 32.20 を押しても、変わった動きは通らない。
  32.20 に「`;echo a;b >semi.txt` は Windows では `a;b` だけ（`>` があるので足さない）、`;echo a;b` のトーストはパスが末尾に付き注記が無い」を足すか 32.22 を足し、
  `scripts/testcheck-ja.toml` の訳も合わせ、TESTING-CHECKS.md の 32.20 の `[x]` を外す（CLAUDE.md の作業ルール）
- [x] （v0.78.41。23.6 の文言と訳を直した）（Sonnet の見張り）v0.78.31（`4a98776`）がトーストを `No such file or folder: <全体のパス> — showing <フォルダ>` に変えたが、TESTING.md 23.6 の期待
  （`No such file or folder: tpyo — showing <the folder>`）を直していない。23.6 は自動テスト済みの行なので再テストは要らないが、文言を新しいトーストに合わせ、
  `scripts/testcheck-ja.toml` の訳も合わせる（CLAUDE.md の「機能を足したら」。TODO の項目にも「TESTING.md の文言も合わせる」とあった）
- [x] （v0.78.41。6.16 を足し、訳と再テストに積んだ）（Sonnet の見張り）v0.78.32（`d97b31a`）が `w` でファイルの仕事を止めたときのトースト（`Copy stopped — what was already done stays`）を足したが、
  TESTING.md に行が無く、再テストにも積んでいない。単体テストは `OpEvent::Finished` を手で作るだけで、本物の取り消しの経路は通らない。
  6 節に「大きいコピーを `w` で止める → トースト」の行を足し、`testcheck-ja.toml` と両レーンの再テストに積む（CLAUDE.md の作業ルール）
- [x] （v0.78.48。`scripts/verify.sh` の最初に `cargo build --locked` を置いた。古い lock では落ちることを確かめた）（Sonnet の見張り）v0.78.42〜0.78.44（`a31b1b4`、`36bf0b2`、`c448dc4`）は、どれも `Cargo.lock` の `filer` が `Cargo.toml` より 1 つ前の版のまま
  コミットされ、CI の `--locked` で全ジョブが落ちていた（`main` が 3 版続けて赤。v0.78.46 のマージする側のコミットで lock を直した）。
  版を上げた**後**に `cargo build` してから `Cargo.lock` ごとコミットする（CLAUDE.md の手順 3）。`scripts/verify.sh` を `--locked` で回すようにすれば、
  push の前にこの取り違えを止められる（スクリプトの変更なので開発の側で）
- [x] （v0.78.68。`\` を `/` にしてから比べる）（Sonnet の見張り、急ぎ: `main` の Windows の `test` が赤）v0.78.54（`6df88ea`）の `keyscript::tests::a_panic_is_reported_for_the_script_that_was_running`
  が Windows で落ちる。`at: src/keyscript.rs:` を決め打ちしているが、Windows の `Location::file()` は `src\keyscript.rs`
  （CI の log: `at: src\keyscript.rs:514`）。区切りに依らない形で比べる（`at: src` と `keyscript.rs:` を別々に見るなど）。
  `scripts/verify.sh` は Linux でしかテストを回さないので、`file!()` やパスを assert するテストは Windows の CI で初めて落ちる（CLAUDE.md の「Linux 上で作業する場合」）
- [x] （v0.78.68。`windows-role.md` に足した）（Sonnet の見張り）v0.78.52（`efb6fac`）は実機の所見（#260 の所見 2、#250 の所見 9: `--keys` の `<C-v>` が貼らない）を直したが、再テストに積んでいない。
  TESTING.md に該当する行は無いので、`windows-role.md` の両レーンの再テストに「`--keys` の `<C-v>` がプロンプトに貼る」を 1 文足す

- [x] （v0.78.80。6.17 を足し、両レーンの再テストに積んだ）（Sonnet の見張り）v0.78.74（`f0e2a16`）は、実機の所見（#241 の所見 1: 小さいファイルが多いコピーの進み具合と残り時間）から来た動きの変更なのに、
  TESTING.md に行が無く、再テストにも積んでいない（CLAUDE.md の作業ルール「修正が直したはずの行は同じ所に足す」）。6 節に
  「小さいファイルを多数含むフォルダのコピーで、バーと残り時間がファイル数でも進む」の行を足し、`testcheck-ja.toml` と両レーンの再テストに積む

- [x] （v0.78.90。直した。1.40 を両レーンの再テストに積んだ） `filer env` の `Terminal pane` が `did not start: Terminal failed: `nosuchshell` was not found on PATH …` と `Terminal failed: ` を重ねて出す（#269 の所見 1）。`app.rs` の
  `remember_shell_failed` にトーストの全文（`said`）を渡しているので、接頭辞を除いた理由だけを渡す（単体テストは理由だけを渡すので通っている）。直したら 1.40 は x64 で再テスト済みの行に戻す
- [x] （v0.78.90。直した。1.40 を両レーンの再テストに積んだ） 起動に失敗したシェルのトーストと `filer env` から、`util::locate` が見つけられなかったときの `(指定されたファイルが見つかりません。 (os error 2))` を外す（#269 の提案 2）。
  `` `nosuchshell` was not found on PATH `` で言い尽くしており、2 回目は OS の言語の文になる（v0.78.29 が spot の `Resolves` で同じ文を外した理由と同じ）
- [x] （v0.78.89。持ち主の判断で PTY ログで読む形にした。両レーンの再テストに積んだ）1.41 の 2 文目（bubbletea のプログラムがすぐ始まる）を PTY ログで読める形に書き直す（#269 の提案 3）: `in reply` の行が `out` の問い合わせの 50 ms 以内に来る
  （x64 では 0 ms と 2 ms）。`gh dash` は人が見る任意の確認として残す。今は持ち主の GitHub にサインインするプログラムしか無く、無人の run には起動できない。
  1.41 は v0.78.88 で `[ ]` に戻した: #269 をマージしたのは持ち主ではなく 14:39 JST のマージの回（モデル未設定で Sonnet）で、後半は誰も押していない
  （v0.78.86 は `merged_by: uchmk` を持ち主と読んだ）。書き直したら 1.41 を両レーンの再テストに積む
- [x] （v0.78.90。直した。1.40 を両レーンの再テストに積んだ） `<name>.panic` の `at:` を、どの OS でも `/` で書く（#269 の提案 1。`panic_report` で `location.file().replace('\\', "/")`）。v0.78.68 はテストの比べ方を直しただけで、
  x64 と Linux の `.panic` を比べると同じ場所が違って見える

- [x] （v0.78.95。7.7b を足して両レーンの再テストに積んだ）（Sonnet の見張り）v0.78.87 の `FILER_CONFIG_HOME` の行
- [x] （v0.78.95。32.20 を足して両レーンの再テストに積み、`exec.rs` の doc コメントを戻した）（Sonnet の見張り）v0.78.92 の `:` / `;` の行

- [x] （v0.78.112。47.7 を直し、両レーンの再テストに積んだ）（Sonnet の見張り）v0.78.107（`ded0fd0`）が `filer env` の `[ui] backend` の警告を `drawing with Gl instead` に変えたが、TESTING.md 47.7 の期待は
  「`filer env` keeps `drawing with the default`」のまま（もう嘘）。47.7 は x64・ARM64 とも `[x]`。文言を新しい動きに合わせ（GL のある Windows では `filer env` も
  `drawing with Gl instead`）、`scripts/testcheck-ja.toml` の訳も合わせ、TESTING-CHECKS.md の `[x]` を外して両レーンの再テストに積む（CLAUDE.md の作業ルール）
- [x] （v0.78.115。5.12 を足し、自動テストと両レーンの再テストを積んだ）（Sonnet の見張り）v0.78.108（`1311053`）がファイル同士の比較の下端に `N differences` を足したが、TESTING.md に行が無く、再テストにも積んでいない。
  単体テストは `count_changes` だけで、下端の文字列は通らない。5 節に「`a.txt` と `b.txt` の比較の下端が `x–y of z  ·  N differences` で、N は `n` が止まる数」の行を足し
  （`ui::overlay::diff_frame` で読めるなら自動テストにしてもよい）、訳と両レーンの再テストに積む

## 実機のセッションの報告（v0.78.170 で受けた分: #294、ARM64 の実行 66）

印は付けていない。`cargo test` は 700 / 0。消える `--keys` の数は、#293 から `src/` も tsumugi-pane の `rev` も変わっていないので順番表の決まりどおり飛ばした（ARM64 で 0 / 140 のまま）。所見は無い。
提案 1（見えるものが何も変わっていなければ実行そのものを飛ばす）は、v0.78.167 と v0.78.169 で入った判定（`windows-role.md` の差分が結果の行だけなら起動しない）で足りる。#294 は、
v0.78.167 のスクリプトが構文エラーで読めなかった間に、前の版のスクリプトが #293 の記録で起動した回。v0.78.169 の後も同じ空振りが続くようなら、改めて項目にする。

## LLM との連携（2026-10-09、持ち主の希望）

filer・tsumugi・これから作るアプリを LLM とつなげたい。ソースを使い回せる形にすることも含めて課題にする（持ち主の言葉）。
`ito-pane`（旧 `tsumugi-pane`）と同じく、共有のクレートに入れて各アプリが `rev` で固定して読む形が第一候補。

- [x] （2026-10-09、v0.84.1）案を書く: [docs/llm-integration.md](docs/llm-integration.md)。(a) アプリが MCP サーバーになる・(b) アプリが LLM を呼ぶ・(c) 両方を比べ、
  (a) を読むだけの道具から始めるのを勧めた。GUI は標準入出力を持たないので、同じ exe の `filer mcp` が Claude Code と窓の間の橋になる。
  口は tsumugi の `tsumugi-mux` の通信を `tsumugi-ipc` に切り出して使い回す
- [x] 範囲を決める（2026-10-09、持ち主: Q95・Q96 とも 1）
- [x] （tsumugi v0.75.0）段 1・2: `tsumugi-ipc` を `tsumugi-mux` から切り出し、`tsumugi-mcp` と `tsumugi mcp`（`tsumugi_sessions` / `tsumugi_screen`）
- [x] （v0.85.0）段 3: 窓の口（`[mcp] enable`、既定は入）と `filer mcp`、`filer_state` / `filer_reveal`。Xvfb で窓に JSON-RPC を流して確かめた
- [x] （#302 / #303、v0.85.8）段 4: 実機で Claude Code に登録して呼ぶ（TESTING.md 50 節）
- [ ] 段 5: 書く道具（窓の確認の箱つき）。範囲は QUESTIONS.md の Q97 で聞いている 【人】
- [x] 「別アプリと双方向 API」（ロードマップの案 3）とまとめて考える: (a) の `filer_reveal` が Q16 の「このファイルへ飛べ」になる（docs/llm-integration.md）

## 印の意味（2026-10-04）

`【人】` `【QA】` `【実機】` は開発のセッションが進めない印（CLAUDE.md の作業ルール）。**`【後】` は今週（2026-10-10 まで）の範囲から外した大きい項目**
（ConPTY の exe への埋め込み、Wayland、次のプロジェクト）で、開発の Routine も飛ばす。今週の「完了」は、残りが `【人】` と `【後】` だけになること。

## ロードマップ

### 1. ディレクトリ・ファイル操作と UI の拡張

- [x] 2 分割ペイン（Split View / Dual Pane）
  - [x] 左右 2 ペインの表示とフォーカス移動。
    - キーは `<C-w>`（Q2 の回答）。押すたびに分割 → 反対のペインへ移る。`<C-S-w>` で閉じる。
      コマンドは `split [open|close]` と `pane_focus [left|right]`。
    - 2 つ目のペインは「もう 1 つのタブ」を横に並べたもの（`App::split` は相手のタブ番号だけ持つ）。
      キーを持つペインは常に `App::active` なので、cd・yank・paste・filter などの既存コマンドは
      ペインを知らないまま動く。相手へのコピーは `y` → `<C-w>` → `p` でできる。
    - 親ディレクトリの列と入れ替わりで出る（ペイン・ペイン・プレビューの 3 列のまま）。
    - タブが 1 つだけのときは同じディレクトリで 2 つ目を作る。複数あるときは次のタブを借りる。
      タブの追加・削除・入れ替えでも相手のタブを見失わない（`split_after_remove` / `split_after_swap`)。
      相手のタブを閉じると分割も閉じる。
    - キーを持つ側は枠で囲み、反対側はカーソルを暗くする。暗い側のクリックはまずキーを取る。
    - 受動側のペインも監視・再スキャンの対象（`scan_low` なので前面のディレクトリを待たせない）。
  - [x] 相手のペインへコピー・移動する 1 キー。`<A-c>` コピー / `<A-m>` 移動（Q4 の回答）。
    F5 / F6 はどちらの当て方も事故を招くので採らなかった（理由は QUESTIONS.md の Q4）。
  - [x] 左右ペインの間でファイルをドラッグ＆ドロップできるようにする（`Shift` 併用で移動）。
    投下先のペインを枠で示し、ポインタ横のラベルが「コピー」か「移動」かを先に伝える。
- [x] キーボードとマウスの複数選択を統合する
  - Shift+クリックで範囲選択、Ctrl+クリック（macOS は Cmd）で 1 件ずつ選択を切り替える。
    egui の `Modifiers::command` を使うので OS の差は自動で吸収される。
  - Space / Visual モードとマウスは同じ `Tab::selected`（`BTreeSet<PathBuf>`）を使う。
    `Tab::click` / `ctrl_click` / `shift_click` が入り口で、Shift の起点は `Tab::mouse_range`
    が持つ。起点はプレーンクリック・キー操作（`sync_visual`）・選択解除・cd で落ちる。
  - 修飾キーを押したクリックはダブルクリック扱いにせず、開く動作に化けないようにした。
- [x] コマンドパレット（Command Palette）
  - `Ctrl+Shift+P`（macOS は Cmd+Shift+P）で開く。`palette` コマンドを足し、既定の keymap で
    `<C-S-p>` に割り当てた。`keys::from_egui` が Cmd を ctrl 扱いにするので OS 差は自動で吸収される。
  - 既存の Pick オーバーレイを使い回した（`PickAction::Command`）。項目は `mgr` の全バインドで、
    ラベルは `説明  ·  コマンド文字列` なのでどちらでも引ける。右端に実行キー、選択は fuzzy 検索。
  - 同じコマンドが複数キーにあるときは keymap で最初に出てくるキーの 1 件だけを出す。
    `noop` と unsupported なコマンドは出さない（`app::palette_items`）。
  - 実行前にオーバーレイを閉じるので、`rename` や `shell` のように入力を求めるコマンドもそのまま動く。

### 2. Git 統合

- [x] Git の状態を一覧に表示する
  - マーク: `M` 変更（黄）/ `+` ステージ済み（緑）/ `?` 未追跡（グレー）/ `D` 削除（赤）/
    `!` 未解決のマージ（赤）。名前と linemode の間に出すので、名前が伸びても衝突しない。
  - ディレクトリは配下の最も強い状態を持つ（コンフリクトはツリーの上から見える）。
    `State` の宣言順 = 強さの順で、derive した `Ord` がそのまま roll-up になる。
  - **git2 ではなく git CLI を呼ぶ方式にした**（方針変更）。git2 は libgit2-sys 経由で C 依存になり、
    CLAUDE.md の「Pure Rust 優先・ARM64 クロスコンパイルを容易に」とぶつかるため。
    `git status --porcelain=v1 -z` 1 回で必要な情報が全部取れるので、パースするだけで済む。
    新規依存ゼロ、ビルド手順の追加もなし。git が無い／リポジトリでない場所では単に無印になる。
  - 非同期方式は **既存のワーカー + crossbeam**（newest-wins）。Tokio は入れない。
    ディレクトリを次々移動しても in-flight は常に 1 件で、古い答えは到着時に捨てる。
  - 取得のトリガは `apply_listing`。ファイル操作や watcher 由来の再スキャンでも一緒に更新される。
    ペインが表示しているディレクトリだけを対象にする。
  - porcelain の非自明な点 2 つ:
    - パスはリポジトリのトップからの相対。`rev-parse --show-prefix` で一覧中のディレクトリ基準に戻す
      （これが「そもそもリポジトリか」の判定も兼ねる）。
    - `-unormal` は未追跡ディレクトリを 1 レコードに畳むので、**その中に入って一覧すると**
      自分自身しか報告されない。この場合は全行にその状態を適用する（実際に中身は全部未追跡）。
      実リポジトリに対する往復テストでこのバグを発見した。
  - テスト 8 件（2 文字コードの解釈 / 行ごとの状態 / ディレクトリの roll-up / prefix の処理 /
    未追跡ディレクトリ内 / 改行を含むパス / 実リポジトリに対する往復 / 非リポジトリ）。
  - テーマは `theme.toml` の `[git]`（`modified` / `added` / `untracked` / `deleted` / `updated`）。
    yazi の git プラグインと同じキー。
- 対象外（方針）: ステージング・diff・コミットはやらない。状態表示のみ。
- [x] 残: ステータスバーにブランチ名を出す。一覧の位置を調べる `rev-parse` に相乗りするので追加コストなし。
- [x] 残: リポジトリでない場所での `git rev-parse` の連打をやめた。ディレクトリ単位で 10 秒だけ
  「リポジトリでない」を覚える（`NOT_REPO_TTL`）。`git init` は 10 秒以内に反映される。

- [x] spot パネルに Git 節（v0.47.0）。hover したパスの直近コミット（短縮ハッシュ・日付・
  件名・作者）と、触れたコミット数（50 まで数えてそれ以上は `50+`）。`git log` 1 回で
  両方取れる。一覧の印が git の**状態**なのに対し、こちらは**履歴**。
- [ ] 残: **コミットグラフのビューア**（SourceTree のような画面）。枝線、ブランチ/リモートの 【後】
  ラベル、日時・作者・ハッシュの列、コミットを選んで差分へ。専用オーバーレイ、keymap 層、
  スクロール、枝の割り当てアルゴリズムが要る。**比較ビュー（45 節）と同等かそれ以上の規模。**
  2026-10-09、持ち主: 保留。別のアプリにするかもしれない。
  - 先に spot の Git 節を実機で使ってみて、それで足りるかを見てから判断する。
  - **filer はファイルマネージャで、SourceTree や lazygit は git クライアント。**同じ土俵で
    戦う価値があるかは、作る前に決めること。

### 3. アーカイブ（圧縮・解凍）

- [x] ワンキーで圧縮・解凍する
  - 解凍 `e` / 圧縮 `E`。どちらも未割り当てだったので衝突なし（`a` は create、`x` は切り取りで使用中）。
  - 圧縮は専用ダイアログではなく `create` / `rename` と同じ入力プロンプトにした。
    既定値は `<名前>.zip` で、**打った拡張子が形式を決める**（`.tar.gz` に変えればそれになる）。
    ダイアログを 1 枚減らせて、既存の入力オーバーレイをそのまま使える。
  - 対応形式: zip / tar / tar.gz は読み書き、7z は読み取りのみ（sevenz-rust が確実に書ける範囲に合わせた）。
  - 解凍先はアーカイブ名から拡張子を落としたフォルダ（`report.tar.gz` → `report`）。
    既にある名前は `unique_name` でずらす（中身を混ぜない）。
  - アーカイブでない選択物が混ざっていたら、アーカイブだけ処理して「N 件スキップ」を toast で出す。
- [x] ライブラリ構成
  - `zip` / `tar` / `flate2` / `sevenz-rust` を採用。C 依存は 1 つも増えていない（`cargo tree` で確認）。
  - `zip` は `default-features = false` + `deflate` のみ。これで `bzip2-sys` / `zstd-sys` が入らない。
    `flate2` は `rust_backend`（miniz_oxide）、`tar` は `default-features = false`（xattr を外す）。
  - 7-Zip CLI 連携は不要になったので実装しない。
- 実装の要点（`src/fs/archive.rs` 新規、`src/fs/ops.rs` に `OpKind::Extract` / `Compress`）
  - コピー・移動と同じジョブワーカーで動く。UI は止まらず、タスクパネル（`w`）に進捗が出る。
    アーカイブ 1 個 = 進捗 1 単位（中身の数を数えるには 2 度読みが要るため）。1 個を処理中は
    エントリ名が流れる。既存名との衝突は paste と同じ上書き／リネームのプロンプト。
  - **セキュリティ**: アーカイブのエントリ名は信用しない。`safe_dest` を全 3 リーダーの共通関門にし、
    `..` で外へ出る名前・絶対パス・ドライブレター付きは書き込まず拒否して報告する（zip slip 対策）。
  - パック時の走査はディレクトリのシンボリックリンクを辿らない（ループを永久に詰め込むため）。
  - 各ライターは Drop 任せにせず明示的に閉じる。gzip のトレーラや zip のセントラルディレクトリを
    デストラクタで書くとエラーの報告先が無く、切り詰められたアーカイブが「開けそうに見える」ため。
  - テスト 4 件（形式判定 / zip slip 拒否 / 解凍先の命名 / zip・tar・tar.gz の往復）。
- 対象外（要判断）: `.rar` は展開もライセンス上の制約がある。扱うなら別途判断が必要。
- [x] 残: アーカイブの中身をプレビューに一覧する。索引のある形式は目次だけを読むので中身を展開しない。
  先頭 2000 件まで。

### 4. 組み込みターミナルとバックグラウンドタスク

- [x] 組み込みターミナル（PTY 連携）
  - `<C-t>` で下部ペインにシェルを開く。表示中のディレクトリで起動する。
  - `alacritty_terminal` が PTY（Unix は本物、Windows は ConPTY）・読み取りスレッド・
    エスケープシーケンスの解析を全部持っている。**`portable-pty` は不要だった**（PTY 実装が
    二重になるだけ）。新規の `-sys` クレートはゼロ（`cargo tree` の差分で確認済み）。
  - このアプリ側の仕事は 3 つだけ:
    1. 起動ディレクトリと cd 追従。シェルに現在地を問う方法は無いので、人がやるのと同じく
       `cd` を打ち込む。ディレクトリが実際に変わったときだけ送る。
    2. キー → バイト列の変換。ここだけが配管でなくロジックなのでテストがある。
       `encode` は (キー, 修飾, カーソルモード) の純粋関数。vim は SS3 の矢印を期待し、
       シェルは CSI を期待するので、端末のモードを読んで切り替える。
    3. グリッドの描画。一覧と同じフォント、背景は run 単位でまとめ描き、
       ANSI 16 色はテーマから取るので 2 つのペインが同じプログラムに見える。
  - ペインがキーを持っている間は**全キーがシェルに行く**。`[term]` セクションに書いたものだけ
    アプリ側が取る（閉じる / キーを返す）。和音は見ない（1 打鍵の完全一致のみ）。
    キーを保留して次を待つのはシェル相手に唯一やってはいけないこと。
  - `<A-t>` で選択パスをシェルの行に打ち込む。スペース入りも 1 語になるようクォートする。実行はしない。
  - **未検証: ディスプレイの無い環境で書いたので、グリッドの描画は一度も目視していない。**
    エスケープシーケンスと 256 色キューブにはテストがあるが、見た目は実機確認が要る。
  - テスト 7 件（プレーンキー / カーソルモードによる矢印の切り替え / 修飾付きの
    パラメータ形式 / Ctrl+文字の制御コード / パスのクォート / 色キューブ / セル数の計算）。
- [x] バックグラウンドタスクの進捗を見せる
  - ステータスバー: `Copy 42%  18.4 M/s  1m12s  +2`（％・速度・残り時間・待機中の件数）。
  - 速度はレポート間隔ではなく 0.25 秒以上のウィンドウで測り、指数平滑する。
    レポートは毎秒約 20 回来るので 2 回の差分は測定値にならない。
    残り時間はバイト数が分かるときだけ（ファイル数ではサイズが分からない）。
  - タスクパネル（`w`）にカーソルを付けた: `j`/`k` 移動、`p` 一時停止/再開、`x` キャンセル、
    `t` 待機中のジョブを先頭へ。コマンドは `task_toggle` / `task_cancel` / `task_top`。
  - 一時停止はファイルの切れ目か大きいファイルのチャンクの切れ目で止まる（書き込み途中では止まらない）。
    停止中はスピンせず制御チャネルで待ち、アプリ終了時に解放される。
  - Runner のジョブチャネルを Mutex 付きキューに変更（並べ替えるため。チャネルは覗けない）。
  - 副次的に `q` でタスクパネルを閉じる処理がイベントループのハードコードだったのを
    `[tasks]` キーマップに移した。clippy の既存警告 1 件（`collapsible_match`）も解消。
- [x] 残: ターミナルのスクロールバック操作・マウス選択・検索。キーは Shift 付き（Q5 の回答）。
  ホイールと `<S-PageUp>` / `<S-PageDown>` / `<S-Home>` / `<S-End>`、検索は `<C-S-f>` /
  `<C-S-n>` / `<C-S-b>`。ドラッグで選択して離すとクリップボードへ入る。
- [x] 残: cd 追従で OSC 7 を受け取るようにした。シェルが現在地を通知していれば `cd` を打ち込まない。
  `<A-Up>`（ターミナル内）でファイル一覧をシェルの現在地へ合わせられる。

### 5. プラグインと設定の拡張

- [x] カスタムアクションを GUI から使えるようにする（Lua プラグイン相当）
  - `keymap.toml` や `yazi.toml` にあるシェルコマンドやカスタムアクションを、右クリックメニューとコマンドパレットの選択肢に動的に出す。
  - **右クリックメニュー**を足した（`menu` コマンド / `<S-F10>` / 右クリック）。中身は設定の読み戻しで、
    固定リストは持たない。並びは 3 群:
    1. `yazi.toml` の `[opener]`（`[open].rules` がそのファイルに選んだもの）
    2. `keymap.toml` で `shell` を走らせるバインド（＝カスタムアクション）
    3. ファイルに作用するバインド（open / yank / paste / rename / remove / link / copy / spot …）
  - カーソル移動や表示切り替えは出さない。その線引きは `app::acts_on_file`。`Unsupported` も出さない。
  - opener は「ファイルを埋め込んだシェルコマンド」なので `Act::Shell` として走らせた。
    結果としてメニュー全体が `PickAction::Command` になり、Pick オーバーレイ・fuzzy 検索・
    キー操作をそのまま使い回せた（パレットと同じ作り）。
  - コマンドパレットの末尾にも同じ opener を `Open with <desc>` として出す。
  - 選択範囲の中で右クリックすると選択を保ったまま全件に作用し、外で右クリックすると選択を捨てて
    その 1 行に作用する（Explorer と同じ）。`Tab::right_click` がその規則。タイトルに件数を出す。
  - キーは `<S-F10>`（Windows / Linux の OS 標準の「コンテキストメニュー」キー）。
    他と衝突せず、規約に沿うので既定で入れた（要確認: Q3 — 回答が来なくてもこのままで可）。
  - テストを 2 件足した（`menu_items` の 3 群の並びと除外、パレット末尾の opener）。
  - README に「Context menu」節と、Default keys / Commands implemented / 既知の制限を更新。

### 6. ネットワークドライブ

- [x] UNC パス（`\\IPアドレス\共有名`）とマウント済みのパスを扱う
  - UNC パスやネットワーク共有をそのまま `PathBuf` として扱い、参照も操作もできるようにする。
  - `util::normalize` を直した。`..` がルート（ドライブ・共有・`/`）より上に行かないようにした。
    直す前は `C:\a\..\..` が `C:\..`、`\\host\share\a\..\..` が `\\host\share\..` になっていた。
    `normalize` は cd・コピー・移動の同一判定（`fs::ops::same_path` / `is_inside`）でも使うので、
    ドライブ側の同じバグも一緒に消えた。
  - Windows の prefix は `\` 表記に揃える（`//host/share/x` → `\\host\share\x`）。表示と比較が揃う。
  - `\\host`（共有名なし）は std が prefix と見なさず `\host` に潰れていた。先頭の 2 本を残し、
    `resolve_against` でも絶対パス扱いにしたので、タブの cwd に繋がれて別の場所へ行くことがなくなる。
    ホストの共有一覧（NetShareEnum）は未実装。
  - README に「Network paths (UNC)」の節を足した。
- [x] 応答の遅れと切断を安全に扱う（子項目すべて完了）
  - 低速な環境や応答待ちで UI が止まらないように、ネットワーク越しの操作とディレクトリ走査を非同期で行う（Tokio を検討）。
    → Tokio は入れず、既存のワーカー + crossbeam（newest-wins）で通した。
  - 接続切れやアクセス拒否でもハングやクラッシュをせず、トーストかエラーダイアログで知らせる。
  - [x] 移動（cd）を UI スレッドでブロックしないようにする。
    - `cd` の `target.is_dir()` を外した。切れた共有では `is_dir` が数十秒返らず、UI スレッドが
      その間止まっていた。今はまず移動して *Loading* を出し、スキャンの結果で判断する。
    - スキャンが失敗したら、そのタブを元のディレクトリへ戻してエラーの toast を出す
      （`Tab::cd_failed` → `CdFallout`）。存在しない・拒否された・打ち間違えたパスも同じ道を通る。
    - キャッシュにある（＝一度一覧できた）ディレクトリへの移動は取り消しの対象にしない。
      再スキャンの失敗は今までどおり `LoadState::Error` で見せる。
    - `cd` プロンプトにファイルのパスを入れたときの「親へ移って選択」も、ブロックする
      `is_dir` ではなく失敗時の 1 回だけのフォールバックで実現した（`PendingCd::fallback`）。
    - 取り消しは 1 回で打ち止め（`pending_cd` は 1 度しか答えない）なので、失敗が続いてもループしない。
  - [x] `cd` プロンプトのパス補完（Tab キー）を非同期にする。
    - `App::complete_input` の `std::fs::read_dir` を外した。ネットワーク上のディレクトリで
      Tab を押すと UI スレッドが止まっていた。
    - 「スキャンのワーカー経由か、キャッシュだけか」は **両方** にした。一度一覧した
      ディレクトリ（＝ cwd・親・そのタブが通ってきた場所）は `App::cache` から即答し、
      それ以外は `scan_low` に投げて待つ。キャッシュだけだと初めて触るディレクトリが
      永久に補完できず、毎回スキャンだと近所の補完まで 1 往復待たされる。
    - 待っている間はプロンプトの右端に `…` を出す（キーが効いていないように見えないため）。
      入力欄はその分だけ右を詰める。
    - 答えが返るまでに入力が進んでいたら、その答えは捨てる（`complete_from` が
      `completion_target` の (ディレクトリ, prefix) を突き合わせる）。打っている途中の
      文字列を古い補完が上書きすることはない。取り消し・確定でも待ちは落とす。
    - 補完の対象は `completion_target` / `completion_hits` / `completed_text` の 3 つの
      自由関数に切り出してテストを 2 件足した。
    - ついでに 2 点。パスの解決を `util::resolve_against` に寄せたので、相対パス（`sr`）が
      `cd` と同じくそのタブの cwd 基準で補完されるようになった。末尾の区切りは `\` 決め打ちを
      やめて `std::path::MAIN_SEPARATOR` にした（Windows の挙動は変わらない）。
    - 残った注意点: キャッシュから答えるので、最後に一覧してから増えたディレクトリは
      補完に出ないことがある。画面に出ているディレクトリは watcher が無効化するので
      通常は問題にならない。毎回裏で再スキャンする案は、遅い共有で Tab を連打すると
      スキャンプールが詰まるので採らなかった。
  - [x] 起動時とリンク追跡に残るブロックする IO を外す。
    - `main.rs` の開始パス判定から `p.is_dir()` を外した。コマンドラインのパスはそのまま開き、
      `App::start_unproven` で `cd` と同じ `pending_cd`（`fallback: true`）を立てる。最初の
      スキャンが失敗したら、親へ寄せてファイルを選び、それも駄目なら作業ディレクトリへ戻す。
      `filer C:\dir\file.txt` がそのファイルを選んだ状態で開くようになった（前は無視していた）。
    - `App::follow_link` の `std::fs::canonicalize` を消した。リンクの行き先は `Entry::link_to`
      としてスキャンのワーカーが `read_link` で読む（相対リンクはリンクのあるディレクトリ基準、
      ジャンクションの `\\?\` は `util::unverbatim` で外す）。壊れたリンクは `Kind::Link.broken`
      で判定するので、UI スレッドはディスクに触らない。
    - ファイルへのリンクは親ディレクトリの `memo` にも名前を入れる。一覧が後から届いても
      カーソルがそのファイルに乗る。
  - [x] 残りのブロックする IO を外す。
    - `App::create_tab` の `target.is_dir()` を外した。`cd` と同じく、まずそのパスでタブを開き、
      スキャンの結果で決める。失敗したら開いた元のディレクトリへ戻す（`pushed: false` なので
      履歴は触らない）。`tab_create <path>` のパスは `fallback: true` で、ファイルを指していれば
      親へ寄せてそのファイルを選ぶ。キャッシュにあるディレクトリは `switch_tab` が埋めるので、
      まだ *Loading* のときだけ `pending_cd` を立てる。
    - 行き先の決定を `new_tab_target`（自由関数）に切り出してテストを 2 件足した。
    - `spot.rs` はワーカーで動くので対象外。
  - [x] `Watcher::sync` の `watch()` を UI スレッドから外す。
    - `notify` のウォッチャーを専用スレッド（`watch`）へ移した。UI スレッドの `Watcher::sync` は
      「監視したいディレクトリの集合」をチャネルへ送るだけになり、`watch()` / `unwatch()` の
      ハンドル操作はすべてそのスレッドで動く。切れた共有で止まっても UI は動き続ける。
    - `sync` は毎フレーム呼ばれるので、前回送った集合と同じなら何もしない（`Watcher::sent`）。
      送信に失敗したときは `sent` を更新しないので、次のフレームでまた試す。
    - スレッド側は溜まった依頼を捨てて最後の 1 つだけを適用する（newest-wins）。
      差分の計算は純粋関数 `plan()` に切り出してテストした。
    - `Watcher` を drop するとチャネルが閉じ、スレッドが終わってウォッチャーも落ちる。

### 7. Undo / Redo

- [x] `u` で取り消し、`U` でやり直し（キーは人の指定）。
  - 対象は 4 つ。`d`（ゴミ箱へ送る）、`r`（リネーム）、`R`（一括リネーム）、`x`→`p`（移動）。
    `D`（完全削除）は確認の上で消しているので対象外。コピー・解凍の取り消しは
    「後片付けに削除する」ことになり、元の操作より失敗が痛いので入れない。
  - ゴミ箱からの復元は `trash` クレートの `os_limited::{list, restore_all}`。
    Windows と Freedesktop 準拠の Unix だけにある API で、**macOS には無い**
    （`restore::SUPPORTED` で分け、macOS では `u` がそう言う。リネームの取り消しは効く）。
  - ゴミ箱を読むのは全体の走査なので `OpKind::Restore` として既存のジョブワーカーに乗せた。
    タスクパネルに出て、キャンセルもでき、終わったら一覧が再スキャンされる。
  - 同名のものを 2 回消したときは **`time_deleted` が新しいほうを戻す**。1 件ずつ
    `restore_all` を呼ぶので、1 件が塞がれても残りは戻る。
  - 戻せなかった手順はスタックに残す（名前が塞がれている場合、どかしてもう一度 `u` で通る）。
    取り消したあとに新しい操作をすると redo は捨てる（エディタと同じ）。上限 50 件、ディスクには保存しない。
  - テスト 7 件（スタックの受け渡し / 新操作で redo を捨てる / 失敗時に戻す / 上限 /
    同名の新しいほうを選ぶ / ゴミ箱に無い場合 / パス比較の大文字小文字は Windows だけ無視）。
- [x] 移動（`x`→`p`）の取り消し（v0.13.0）。衝突時のリネームがあるので、`ops.rs` の
  `OpEvent::Finished` が「実際に書いた (src, dest) の組」を `moved` で返す。undo はそこから
  組み立てるので、`x.pdf` として送って `x_1.pdf` に着地したファイルも正しく戻る。
  テストは `mod move_undo`（app.rs）に 3 件。
- [x] 新規作成（`a`）とリンク作成（`-` `_` `=`）の取り消し（v0.60.0）。
  - 作成は `UndoStep::Create`。途中で作ったフォルダも外側から順に持ち、`u` は中身が空のときだけ消す
    （書き込まれたファイルは残してそう言う）。既にあったフォルダへの `a` は何も記録しない。
  - リンクは作業スレッドが実際に作ったもの（衝突で `_1` になった名前も含む）を `ops::Link` で返す。
    `u` はリンクだけを消し、まだ同じリンクか（シンボリックリンクか、ハードリンクなら元と同じファイルか）を確かめる。
    Windows では `std` に安定したファイル ID が無いので、ハードリンクの同一性を大きさと更新時刻で見ている。
  - テスト 6 件（`mod create_and_link_undo`）。Linux の仮想ディスプレイで `a` → `u` と `-` → `u` を確かめた。
- [x] 見送り（2026-10-09、持ち主: 取り消しは保存しない）: 取り消しの永続化。ディスクに保存すると、次回起動時の `u` が前セッションの
  操作を取り消そうとする。その間にファイルは他のプログラムに動かされ、ゴミ箱は空にされ、
  「その名前で最後に捨てられたもの」という当て方が急に弱くなる。README が
  「undo は直前のうっかりのためのもので、セッションの記録ではない」と設計判断として
  書いているので、方針の反転になる。**人と相談した上で入れないと決めた。**
- [ ] 残: macOS でゴミ箱から戻す方法。`osascript` で Finder に戻させる手はあるが、 【後】
  シェルアウトが増えるので入れるか要判断。

### 8. 改善案（人からの提案）

- [x] ジャンプの frecency。`z` の並びを「訪問回数 × 経過時間の重み」にした（zoxide と同じ形）。
  - `history.txt` は `パス\t回数\t時刻` の 3 列になった。**旧形式（パスだけ）も読める**
    （回数 1・時刻不明として扱い、以後に訪問したものより下に並ぶ）。
  - ピッカーの右列に「2h ago」を出す。ブックマークは明示的に付けたものなので並べ替えない。
- [x] ブックマークの既定キー。`b` 移動 / `B` 保存 / `<A-b>` 削除 / `<A-B>` 全削除（Q7）。
  文字キーなのでどの配列でも同じ位置。`m` は yazi 互換の linemode prefix なので触っていない。
- [x] 設定の再読み込み（`config_reload`、`<C-F5>`）。
  - フォントの入れ直しだけはフレームループの仕事なので `App::refont` で依頼する。
    起動時と同じ `apply_fonts` を通るので、再読み込み後のフォントは起動し直したときと同じになる。
  - 手で変えたもの（`,` で選んだソート、Markdown の描画切り替え）は戻さない。
    ウィンドウサイズは起動時のみ。
- [x] 2 ファイルの簡易 diff ビュー（`<A-d>`、`src/diff.rs`）。
  - **`similar` は入れず自前で書いた**。行単位の LCS だが、先に一致する先頭と末尾を剥がすので、
    4000 行のファイルの 1 行変更は表が 1×1 になる。共通部分が無い大きなファイルは
    表を作らず「並べただけ」にして、そう表示する（`rough`）。
  - 読み込みと突き合わせは専用ワーカー（newest-wins）。同一ファイル・非テキストは
    バイト比較の結果だけを言う。
  - `[diff]` キーマップセクションと `feed_diff_key` を用意した（設計の約束事どおり）。
    `n` / `N` で差分ブロック間を移動する。
- [x] バッチリネーム（`R`、`src/rename.rs`）。
  - テンプレート（`{name}` / `{ext}` / `{n}` / `{n:3}`）と `s/正規表現/置換/gi` の 2 通り。
    入力中は毎キーでプレビューを出すので、**ディスクには触らない**（ディレクトリの既存名は
    メモリ上の一覧から取る）。
  - 正規表現は `fancy-regex`。syntect が既に使っているのでビルドに増えるクレートはゼロ。
  - 名前の入れ替え（`a`↔`b`）に対応。一時名へ逃がす順序を `rename::order` が決める。
  - バッチ全体で undo 1 手（`UndoStep::Bulk`）。
- [x] Quick Look 風の中央モーダルプレビュー（`<F3>`、Q8）。
  - オーバーレイではなく「パネル」にしたので、開いたまま `j` / `k` で一覧を歩ける。
  - プレビューの描画は側面ペインと同じ `ui::draw_preview` を通す。開いている間は側面ペインを
    描かない（画像の要求サイズが毎フレーム 2 つの矩形で奪い合うため）。

### 9. プレビューパネルの改善（人からの提案）

- [x] ミニマップ（`src/preview/mod.rs` の `MapRow` / `minimap()`、`src/ui/preview.rs` の
  `split_minimap()` / `minimap()`）。
  - 文字は描かず 1 バンド 1 矩形。行の要約はワーカーで作る（1 行 6 バイト）。
  - クリック・ドラッグは既存の `Drawn` に `scroll_to` を足して返す（`jump` と同じ道）。
  - レンダリング済み Markdown には出さない（行番号が原文とずれるので枠が嘘になる）。
    そのおかげで `preview.cols`（Markdown の折り返し幅）を調整する必要がなかった。
- [x] プレビューの独立スクロール。**`<A-j>` / `<A-k>` とホイールは前から動いていた**ので、
  `K` / `J` の割り当てを足しただけ。バーチャライズドも既にできていた
  （`ui::preview::text` が `lines[start..end]` しか描かない）。
  - `egui::ScrollArea` は採用しない。全高を測るために 1 万行をレイアウトすることになり、
    仮想化で節約した分を捨てる。スクロールバーの代わりがミニマップ。
- [x] 画像のズーム・パン・フィット。
  - `Ctrl` + ホイールはポインタ中心（`app::zoom_at`）。ドラッグでパン、ダブルクリックでフィット。
  - ズームで `box_size` を上げて再デコードする（`app::zoom_box`）。2 の冪で刻み 4096 で打ち止め。
    倍率は `fit` との比で判断する（5% でフィットする写真は 10% でもう 2 倍の解像度を要求している）。
  - **`Payload::Image` に `source` を足した。** `width` / `height` はデコード結果なので、
    画面上の大きさをそれに追従させると鮮明な版が届いた瞬間に絵が飛ぶ。
  - テスト 5 件（ポインタ下が動かない / 上下限 / パンのクランプ / フィットは拡大しない / 箱の段）。
- [x] ミニマップに「ポインタ下の行のプレビュー」を出す（v0.40.0）。
  - 1 行 + 行番号だけ。3 行にすると「そのバンドがその 3 行」と読めてしまうし、帯は
    `BAND_H = 2.0` で `per` 行を要約しているので嘘になる。1 行なら約束しすぎない。
  - 遅延は `interaction.tooltip_delay`（0.4 秒）を**読む**。`main.rs` で設定済みだったのに
    どのウィジェットも使っていなかったので、これが最初の利用者。
  - **ドラッグ中も出す**（人の指定）。egui の `time_since_last_movement()` で測ると
    ドラッグ中はポインタが動き続けて永久に出ないので、**ホバー開始時刻**を持つ方式にした。
    置き場所は egui の一時ストレージ。アプリ状態に持つと、ファイル変更・payload 差し替え・
    リサイズ・ミニマップ切替の 4 通りで古いカードが残る。
  - `theme.preview_hovered` を使った。宣言もマージもされていたのに `src/ui/` から
    一度も読まれていない死んだトークンだった。yazi では「プレビューのホバー行」そのもの。
  - 副産物: `line_at_y` を切り出して**下端のクランプ漏れを直した**。最下端をポイントすると
    `map.len()` を返していた（従来は `saturating_sub` と後段の `max_offset` が吸収していて
    表に出なかったが、`lines[k]` を引いた瞬間に範囲外になる）。
- [x] （Q73、推奨の 1: 今のまま。末尾の注記がどこまで読んだかを言う）残: `max_text_bytes` で切られた先はミニマップにも出ない。全体の形を出すなら
  行の要約だけ最後まで読む経路が要る（ハイライトは不要なので安い）。

### 10. キーマップの比較レビュー（yazi / lf / Neovim との照合）

- [x] 機械的な衝突検出。全 9 層 190 バインドに対し、**同一層内の `on` 重複ゼロ・prefix シャドウ
  ゼロ**を確認した。`config::keymap::tests::no_layer_binds_a_key_twice_or_swallows_its_own_chords`
  で以後も守る（`resolve` は最初の完全一致を採るので、単独キーが後続の和音を殺し得る）。
- [x] `[term]` の `<C-t>` がシェルを破棄していたのを入れ替えた（Q10）。
  `<C-t>` = キーを返す（非破壊）、`<C-S-t>` = シェルを終う。頻度と破壊力を一致させた。
- [x] `[term]` に `<F1>`（ヘルプ）と `<C-S-p>`（パレット）を追加。
  この 2 つが無いとターミナル中はアプリの機能に一切到達できなかった。
  どちらもシェルが使わないキーなので奪っても壊れない。
  - `<C-w>` は入れない。readline の「単語を消す」で頻用されるため。
- [x] `<C-r>` を redo に追加し、選択反転を `<C-S-r>` へ移した（Q6 追記）。
- [x] ブックマークに `'` を追加（`b` と併存）。vim の `m` / `'` の対に指を合わせる。
- [x] `J` / `K` を削除。`<A-j>` / `<A-k>` と完全重複で、大文字は希少資源だった。
- [x] README に「Coming from yazi, lf or vim」の節を追加。lf は `d`=切り取り / `e`=エディタ /
  `u`=選択解除 という別系統の語彙なので、`prepend_keymap` 6 行で戻せることを示した。
  **既定は yazi 互換のまま変えない**（方針）。
- 一次資料での照合は未了: yazi / lf の既定キーマップを取りに行こうとしたが WebFetch が
  使えなかった。比較表は知識ベースなので、特に lf の列は要検証。

### 11. 実機テストの受け渡し

- [x] [TESTING.md](TESTING.md) を作成。**画面に出したことがない項目を 12 節・約 90 チェック**に
  分解した（ターミナル / ミニマップ / 画像ズーム / diff / バルクリネーム / undo /
  Quick Look・ペイン / 設定リロード / アーカイブ / エディタの行ジャンプ / UNC / 厄介な名前）。
  「何が自動で担保されているか」も先頭に書いて、二重に確認しないようにした。
- [x] `scripts/make-fixtures.ps1`。チェックが指すファイルを一式生成する。
  4000 行の Rust、diff 用の 2 ファイル、3200×2400 の格子画像（1:1 でぼけが分かる）、48×48、
  zip とその中身、git の全状態を持つリポジトリ、500 件のディレクトリ、厄介な名前、
  入れ替え用の `ab.txt` / `ba.txt`。
- [x] `.github/workflows/release.yml`。タグ push で公開ビルドを出す。
  **Actions のアーティファクトはログインが必要で 90 日で失効する**ので、人に依頼するなら
  Release でないと渡せない。タグと `Cargo.toml` の版の不一致でジョブを落とし、
  CHANGELOG の該当節を awk でリリースノートに流し込む。
- [x] バルクリネームのバグを修正。`plan` が「移動先が選択内の別ファイルの名前」を無条件に許して
  いたが、**そのファイルが動かない場合は名前は空かない**。プレビューが成功と表示して適用時に
  「Already exists」で落ちていた。新旧の名前を先に全部出してから判定するようにした。
  テスト（TESTING.md の 10.9b。当時は E9b、v0.26.6 で節が通し番号になった）を追加。
- [x] ペイン幾何のテストを追加（`split_minimap` の 3 件）。GPU 不要で、
  「ミニマップが本文を狭める」「狭いと出ない」「アウトライン列は残り幅で判定される」を固定した。
- [x] フレーム全体を GPU 無しで回すテストハーネス（v0.45.0）。`ui::harness::Screen` が
  `egui::Context` と `App` を 1 つずつ持ち、`egui::Event` を `handle_input` に流して
  `ui::draw` を回し、**描かれた文字列と矩形を返す**。egui のフレームは
  「レイアウト → メッシュ」までが純 CPU で、GPU が要るのはメッシュをピクセルに焼く最後の
  一段だけなので、**基準画像を作れないこととフレームを検査できないことは別**だった。
  1 フレーム 0.01 秒。依存追加はゼロ。
  - 初回の実行でヘッダのバグを見つけた。ファイル名を `\` 固定で繋いでいたので、
    Linux / macOS では `/home/you\notes.md` と出ていた（`MAIN_SEPARATOR` に直した）。
  - 足したチェック: 画面の文字（パス・件数・モード・行）、SELECT 表示、
    `<A-m>` の chord が文字も打たないこと（v0.38.0 のバグ）、
    ミニマップが出る / 狭いと出ない、フォルダ比較の記号・集計・カーソル。
- [x] （2026-10-09、持ち主の判断で済みにした。入れない）残: **ピクセル比較**（`egui_kittest` のスナップショット）。上のハーネスは
  「何を描いたか」までで、色や字形の崩れは見ない。wgpu のアダプタが要るが、
  このコンテナでも `mesa-vulkan-drivers`（lavapipe）は apt の候補にあるので入れられる。
  ただしフォントのラスタライズが OS ごとに違うので基準画像が 3 つに割れる。
  費用対効果は上のハーネスより明確に低い。
- [x] （v0.80.14。5 / 9 / 11 / 27 / 43 を読み直した結果、文字列層で届く行はすでに全部テストになっていた。
  5 は 5.11 まで（`a_compare_row_reads_with_its_changed_words`）、9 と 11 は全行、27 は 27.1 以外、43 は 43.1〜43.13 のうち 43.9・43.11 以外。
  残りは色・線・字形の目視（5.3 の色、5.1 の中央の線）と、Excel 製の実ファイル（43.1・43.7）、50 MB（43.9）、冷えた起動の競合（27.1）で、ハーネスの外。
  これらは TESTING.md の各節の冒頭にすでに理由付きで書いてあるので、新しい行は足さない）【QA】
- [x] ハーネスで届く残りの節を埋めた。12 / 13 / 24 / 33 は v0.80.10、38 / 42 は v0.80.11
  （38.7 を theme.toml から `<C-F5>` と同じ読み直しで、42.6 のカードの幅を測る。42.7 の引っかかりだけ目で見る）。
- [x] 33.19・33.21〜33.23、21.17・21.19 をテストにした（v0.80.12）。33.19 / 33.21 / 33.22 は実際のファイルを
  yazi と filer の 2 つのフォルダに書いて読み・読み直す `config::reload_tests`、33.23 は `~` の枠のテスト、
  21.17 / 21.19 はキーを keymap 経由で押す `app::archive_view`。
  - **当初の「18 節が届く」という見積りは楽観的だった。**投げる前に節を読んで
    判断すること。外したものと理由:
    - **20 節（設定とテーマ）**: 全行が設定ファイルの書き換え＋`<C-F5>` の再読込で
      始まる。ハーネスは設定の読み込み先を差し替えられず、`font_size` の変化は目視。
    - **44 節（ディスク使用量）**: 本文自身が「What needs a machine is a real disk」と
      書いている。`node_modules`、300k ファイル、junction、ネットワーク共有は作れない。
      歩き方と並び順は既に単体テスト済みで、残りは実機の領分。
    - **21 節**は 21.1（プレビューが中身を並べる）だけ届く見込み。21.2〜21.5 は
      展開・圧縮のジョブが走る必要があり、ハーネスはワーカーを回さない。
    - **38 節**はターミナル側の 5 項目が Windows 専用（Linux では 1 行も
      コンパイルされない）。アウトライン側の 2 項目だけ届く見込み。
  - **「その行は実際に実行可能か」も監査項目に入れる価値がある。**今日の実機テストで、
    前提や確認手段が書かれていない行が 4 件出た（15.5 のハードリンクの確かめ方、
    15.2 の US 配列前提、13 節の `-` / `_` に開発者モードが要ること、18.11 と 36.17 の
    コードとの食い違い）。**実機でしか確かめられない項目なのに、確かめるのに必要な
    情報が揃っていない**という共通点がある。
  - **QA セッションを 1 本実走させた（v0.45.3、PR #16）。**`.claude/qa-role.md` 全文と
    1 行のプロンプトだけを渡し、ファイルの場所もテーマのフィールド名も雛形も教えずに、
    12 分で PR まで自力で到達した。手順書は単体で立つ。副産物として、私が v0.45.1 で
    TESTING-KEYS.md しか直さず TESTING.md 本文の `193` を見落としていたことが出た。
    定義の側の不足は 1 件だけで、**役割定義とスキルが TESTING.md への注記について
    食い違っていた**（v0.45.4 で規則を役割定義 1 か所に寄せた）。
  - 10 節（yank レジスタ）は `ui::yank_frame` で 8 項目すべて自動化した。
    ヘッダとステータス行の文言に加えて、**マーカーバーの色**もハーネスで見ている
    （`Painted::filled` はテーマの色そのものを探せるので、選択の色が yank に
    勝つことまで固定できる）。
  - 6 / 18 / 36 節（ペインの割り付け）を 1 回でまとめて自動化した。下準備を
    `ui::panes` に寄せて、`split_panes_frame` / `quick_look_frame` /
    `max_preview_frame` の 3 モジュール・25 件。`<F3>` のパネルが窓の 86% × 88% に
    ちょうど収まり、`T` はそのサイズの矩形を描かないこと（＝2 つが別物であること）まで
    測っている。副産物の報告が 2 件 — **18.11 の `b`+文字は現在の keymap に無い**
    （ジャンプは `'`）、**36.17 の「Nothing happens」はコードと食い違う**
    （確認プロンプトはどの文字でも取り消される。終了はしない）。QA-REPORT.md 参照。
  - 33 / 38 / 42 節（設定警告の色・フォーカス罫線・ミニマップのホバーカード）を 1 回で
    自動化した。`ui::config_warning_frame` / `ui::preview::focus_rule_frame` /
    `ui::preview::minimap_hover_frame` の 3 モジュール・19 件。**ハーネスが stroke の色と
    文字の色を読めるようになった**（`Painted::stroked` / `Painted::ink`）ので、1 px の罫線と
    トーストの枠の色が届く。あわせて `Screen::wait` で egui の時計を進められるようにして、
    `tooltip_delay` 越しのホバーカードをポインタイベントで動かしている。
    **38 節はターミナルペインが本当に上がる** — `src/terminal.rs` に `#[cfg(windows)]` は
    無く、PTY は Unix でも開くので、`<C-t>` で本物のシェルを起動して罫線を確かめている
    （Linux で 26 ms、`Drop` で shutdown）。副産物の報告が 3 件 —
    **33.9 の「箱が 5 つ積まれる」は起動時には起こらない**（警告は何件でもトースト 1 件）、
    **42.1 の「about 0.4 s」は 0.5 s**（egui の `tooltip_delay`）、
    **TESTING.md 本文のキー数 `226` が 242 に追従していない**。QA-REPORT.md 参照。
  - 9 / 27 / 43 節（プレビュー）を 1 回でまとめて自動化した。`ui::preview_panes` に
    下準備を寄せて、`outline_end_frame` / `preview_arrival_frame` /
    `csv_table_frame` の 3 モジュール・14 件。**ハーネスに `turn()` / `settle()` を
    足した** —— `Filer::ui` と同じ順で 1 周するだけのもので、本物のプレビュー
    スレッドと本物のチャネルを通す。プレビューは「ワーカーが答えた」ことで初めて
    存在するので、状態を手で組んでも「届いた」ことにならない。43.4（窓をリサイズ
    するとテーブルが組み直る）は 2 つの幅で 2 回描いて初めて見える項目で、
    **ペインの実測幅が要るのでハーネス以外では確かめようがない。**
    報告は 3 件 — **9.1 の `<C-o>` は keymap に無い**（`<BackTab>`）、
    **27.4 の `+` も無い**（`<A-i>` / `<A-o>`、JIS 配列を避けた形）、
    **43.11 はテーブル表示にミニマップが 1 本も出ない**（`ui/preview.rs` が
    レンダリング済み Markdown で early return する）。QA-REPORT.md 参照。
- [x] （v0.75.22、Q74 推奨の 1。`ci.yml` に `test-linux`、macOS は `test-macos.yml` で週 1 回と手動）残: `ci.yml` の `test` を ubuntu と macos にも広げる。
  **macOS ではテストを一度も走らせたことがない。**
  - [x] （v0.75.23）`test-linux` が初回で 1 件落とした: ペインのシェルに `TERM` を渡していなかったので、CI（`TERM` 無し）の bash が dumb 端末として
    長い行を 1 行の中で横に流した（`sending_a_name_opens_a_closed_pane`）。Unix では `TERM=xterm-256color` と `COLORTERM=truecolor` を渡す。1.39 を足した。
  - [x] （v0.75.24）`test-macos.yml` の初回（v0.75.22、手動）は 2 件落ちて 2 件止まった（30 分で止めた）。
    - `sending_a_name_opens_a_closed_pane`: Linux と同じ `TERM` の件（v0.75.23 で直した）
    - `a_symlink_says_where_it_points_and_where_that_lands`: macOS の一時フォルダが `/var` → `/private/var` のリンクの下で、`Resolves` と
      `Target` の文字が違った。テストのフォルダを正規化した
    - `ending_the_shell_from_the_list` と `with_both_panes_open_exactly_one_rule_is_accent` が止まった: macOS は既定のシェルを `/usr/bin/login`
      越しに起動し、alacritty の `Pty` は閉じるときに `SIGHUP` のあと子（`login`）を `wait` する。`login` はシェルを待ち、シェルは PTY の主側が
      閉じるまで終わらず、主側はその `wait` の後で閉じる。filer の `Drop` がそれを UI のスレッドで落としていたので、実機ではペインを閉じると窓が固まる。
      閉じる前に `login` の下のシェルへ `SIGHUP` を送るようにした（見立て。macOS で回して確かめる）。ジョブに 30 分の上限を付けた
  - [x] macOS のテストはやめた（2026-10-04、持ち主の指示。課金が Linux の 10 倍）。v0.75.25 で `test-macos.yml` を消し、回っていた 2 回目は止めた。
    v0.75.24 の `login` の直しは macOS で確かめないまま入っている（見立て）。
- [x] `make-keycheck --check` と CI のステップ（v0.45.1）。keymap を変えて
  TESTING-KEYS.md を再生成し忘れた状態が CI で落ちる。**入れた初回に本物の乖離を
  見つけた** — 9 キーが未登録、説明が 2 件古く、`[help]` 層は v0.34.0 でスクロール
  キーが増えたのに 1 キーのままだった（207 → 226 キー。チェック印は 202 件そのまま
  引き継がれた）。
- [x] QA / テスト専任セッションの定義（v0.45.1）。`.claude/qa-role.md` が役割の唯一の
  定義で、subagent（`.claude/agents/qa-tester.md`）と手順書（`/qa-audit`）と別ターミナルの
  `claude` の 3 つが同じ文を使う。書いてよいのは `#[cfg(test)]` の中だけ、機能コードの
  バグは直さず `QA-REPORT.md` に報告、`main` へ push しない、PR までがロール。
- [x] TESTING.md 自体が未実行だった件。v0.47 から Windows 実機（x64 / ARM64）のレーンが
  TESTING-CHECKS.md を回し、手順の誤りは `qa-reports/` と TODO に積まれて直されている（v0.80.7・v0.80.8 の
  文言直しと行の分割もその分）。項目としては閉じ、個々の誤りは見つかり次第直す。
- [x] 実機チェックリスト `TESTING-CHECKS.md` を生成するようにした（v0.47.10）。
  `examples/make-testcheck.rs` が TESTING.md から id・節・原文を取り、日本語と節ごとの
  準備コマンドを `scripts/testcheck-ja.toml` から足す。**`cargo test` が覆っている項目を
  一覧から引く**のが本題で、覆っているかはテストの doc コメント（`TESTING.md 45.2`）から
  読む。516 項目のうち人が押すのは **359 件**。CI が `--check` を回す。
- [x] 日本語訳を全部埋めた（v0.47.15、**359 / 359**）。`scripts/testcheck-ja.toml` の
  `[row]` に入っている。各行は「操作 → 期待」の両方を日本語で書き、原文を斜体で併記する。
- [x] テストの doc コメントに TESTING.md の id を書いていない節があった（v0.80.10）。
  21・24・33 の id を書き足し、生成器は `N.N:` の形も数える。節の前書きの件数も合わせた。

### 9. プレビューの追加（人からの提案）

- [x] CSV / TSV を桁揃えの表で出す（v0.41.0）。描画は Markdown の表から `table_lines` を
  切り出して共有した。`Payload::Markdown` の形なので `M` で生テキストと行き来できる
  （`Payload::Text` に整形済みの表を入れるとミニマップが一様な塊になる）。
- [x] ディスク使用量の可視化（v0.42.0、`gu`）。**新規オーバーレイ不要** — 検索ビューと同じ
  「パスが実在しない `Folder`」に乗せたので、一覧のキーが全部そのまま効く。
  `search.rs` の走査器は既定で隠しファイルと `.gitignore` を除外するので流用せず、
  `ops::measure` の形（明示スタック・`symlink_metadata`・20 万件の予算）に倣った。
  - [x] （v0.75.21。macOS / Linux は (dev, ino) で 1 回だけ数える。44.20）ハードリンクの二重計上。Windows は std が
    ファイルの番号を出さず、読むには全部のファイルを開くので今のまま（README に書いた）。
  - [x] 1 段だけだった（v0.63.0 で `l` / `Enter` がビューのまま下りて測り直し、`h` で上がる。`gu` を押したフォルダからの `h` で抜ける。測った結果は持たず、着くたびに測り直す）。
- [x] （v0.76.0〜v0.78.0、Q75）残: アーカイブの中を歩く（フォルダに入る / 1 ファイルだけ取り出す）。
  現在は一覧のみ。README の Known limits に挙がっている。
  - [x] 土台: 1 つのメンバー（ファイル、またはフォルダとその中身）だけを取り出す `archive::extract_one`（v0.75.26）。上のフォルダは付けずに書き、
    名前は全体の展開と同じ `safe_dest` を通す。zip / tar / tar.gz / 7z のテストを足した。まだどこからも呼ばない
  - [x] 入り方とキー（Q75、推奨の 1。v0.76.0）。`l` / `<Right>` で入り、書庫を「パスが実在しない `Folder`」（使用量・検索のビューと同じ形）で見せ、
    `h` / `l` で中のフォルダを上り下りし、ファイルは `extract_one` で一時フォルダに出して開く。中は読み取り専用（書き換え系のキーは断る）。21.17〜21.19
  - [x] （v0.77.0。`OpKind::TakeOut`、`archive::split_member`。21.20）`y` → 外のフォルダで `p` で、メンバーを取り出す。ヤンクに「書庫から」の印を持たせ、`p` を
    `extract_one` のジョブにした。いったん隠しフォルダに出してから名前へ移すので、上書きの確認（Overwrite / Skip / Rename）はコピーと同じ。`x` は断ったまま
  - [x] （v0.78.0。21.21）メンバーのプレビュー。4 MB までのファイルは別スレッドで `extract_one` してビューごとの一時フォルダに出し、普通のプレビューに回す
  - [x] （v0.78.0）一時フォルダの片付け。プレビュー用の写しは書庫を出たとき（ビューが消えるとき）に消す。`l` で開いた写しはエディタが掴んでいるかもしれないので
    終了時には消さず、起動のたびに 1 日より古いほかのプロセスの `filer-archive-<pid>` を消す（`util::sweep_archive_scratch`）
- [x] ディレクトリ同士の比較（v0.43.0）。`Overlay::Diff` と `[diff]` キーマップを再利用し、
  `Outcome::Tree` を足した（**新規オーバーレイ不要**）。サイズで大半を決め、同サイズの組だけ
  ブロック単位で読んで最初の相違で止める。64 MB 超は `?` にして「一致」と言わない。
  - [x] 行からファイル対を開く（v0.61.0、`[diff]` の `<Enter>`）。`q` で同じ行に戻る（`DiffOverlay::back`）。
  - [x] 「違うものだけ表示」のフィルタ（v0.53.0 の `z`、Q23 で入っていた。印の付け忘れ）。
- [x] 比較の語単位ハイライト（v0.62.0）。対になった行を語（英数字の並び・空白の並び・記号 1 字）に分けて LCS を取り、`diff::Line::changed` に文字の範囲で持つ。描画はフォントで幅を測るので日本語でもずれない。画面に収まらず省略された行は行の色だけ。

### 10. 品質・保守性の整理（v0.44.0）

挙動を変えずに済む整理だけをまとめて入れた。**外から見える動きは変わっていない。**

- [x] **`Theme` の毎フレーム clone を止めた**（速度）。各描画関数が borrow checker 避けに
  冒頭で `cfg.theme.clone()` しており、`Theme` はアイコン表を持つので**既定 102 エントリ ×
  毎フレーム 6 回**のヒープ確保になっていた。`Arc<Theme>` にして参照カウント加算に。
  呼び出し側 6 箇所は無変更（Deref）。`Arc` を外すとコンパイルが通らないテストつき。
- [x] **ディレクトリ比較のフッタ集計**が毎フレーム全行を走査していた（v0.43.0 の持ち込み）。
  `Outcome::Tree` に件数を持たせて 1 回で済ませた。
- [x] **`feed_*_key` の 4 重複を 1 つに**（`PanelLayer` + `feed_overlay_key`）。
  `main.rs` 側の 4 重複 × 2 箇所も 1 腕ずつに。層が自分のセクションを読むことをテストで固定。
- [x] **`act()` を 376 → 231 行**。長い腕をメソッドへ出して腕を 1〜3 行に保つ方向
  （`match` を機械的に割ってはいない）。**10 行超の腕 10 個 → 0 個。**
- [x] **`Entry` に `Default`**。構築 5 箇所の羅列を `..Default::default()` に。
- [x] **`truncated` / `total_lines` を `Extent` に**。`#[allow(too_many_arguments)]` が 5 → 3 に。

残っている整理（判断が要るので未着手）:

- [x] **Linux で `cargo test` が緑になった**（v0.44.1）。`exec` の 3 件は隠さずに
  **プラットフォームに応じたパスを使う形にして双方で走らせた**（行番号とフラグの規則は
  区切り文字の話ではないため）。`normalizes` だけは入力自体が Windows 固有なので
  隣の `#[cfg(windows)]` テストへ移した。CLAUDE.md の「既知の 4 件」も削除した。
- [x] **一時ディレクトリを `util::test_dir` にまとめた**（v0.44.1）。名前はテストの
  スレッド名から取るので、**呼び方を間違えても 2 テストが同じディレクトリを得ない。**
  同じラベルを要求する 2 テストが別ディレクトリになることをテストで固定。
  本番の `TempDir` と、1 テストで複数回呼ぶ `move_undo` の `tmp` は理由つきで据え置き。
- [x] `linemode` を enum にした（v0.44.2）。`SortBy` と同じ形（`parse` + serde の alias）。
  `yazi.toml` の不正な値はファイルの警告、keymap の不正な値は `Act::Unsupported`
  （読み込み時の警告 + help パネル + 押したときの表示）になる。
  `owner` は「受け付けるが未実装」として**打ち間違いと区別**する変種にした。
  副産物として `linemode != "size"` の文字列比較が `wants_dir_size()` になり、
  `Copy` になったことで `clone` が 4 箇所消えた。
- [x] （2026-10-04、持ち主の判断で閉じた。実機の 2 レーンが 466 / 503 を確かめ、残りは見た目と持ち主の行）TESTING.md が 983 行・45 節・**463 項目**で、一度も実行されていない。
  間引くか、「リリース前に必ず通す」部分集合を決めるかの判断が必要。

## 直した 2 件の答え合わせ（完了）

- [x] **1.19 / 1.20 / 1.21 を実機で確認した（PR #63）。**4 シェルすべてで v0.47.34 の 3 件は
  再現しない。**1.19 は (a) と (b) が別々に示された** —— (a) は `NOT-INTERRUPTED` が出ずに
  `STATUS=Stopped`、(b) は filer とシェルの pid が同じでタブも 2 枚残っている。
  - **`<C-c>` がシェルに届くことが確かめられたので、ペインの中で TUI（lazygit / gh-dash）を
    動かす計画も成り立つ。**届かなければ抜け出せない罠になっていた。
- [x] **その過程で別のバグが出た。Git Bash で `'` の無い普通のパスに `cd` できない**
  （v0.48.1 で修正）。`\` を POSIX でも安全扱いしていた。
- [ ] **1.22 / 1.23 を実機で取る。**1.22 が上の修正の答え合わせ、1.23 は `<A-t>` 側で 【実機】
  **報告が「押していない」と明記していた**ぶん。

## ペインで tcell のプログラム（lazygit / gh-dash）を動かすときの不具合

- [x] **`Esc` が効かない（v0.48.6 で修正。実機で確認済み）。**`scripts/keyprobe.ps1` で見ると、
  filer のペインでは Esc 1 回につき**押したレコード（文字 `0x1b`）と離したレコード（VK_ESCAPE）**
  の 2 つが届き、Windows Terminal では押した分しか届かなかった。tcell は離したレコードを
  `ESC [ 27;1;27;0;0;1 _` に組み直して同じ流れに入れるので、50ms の待ちの中に 2 つ目の ESC が
  来て 1 つ目が Alt の前置きになり、Esc は消える。**押したレコードだけを win32-input-mode で
  送る**ようにした。
  - v0.48.5 は「ConPTY が `CSI ? 9001 h` を要求してくる」前提で書いたが、`alacritty_terminal` は
    ConPTY をフラグ 0 で作るので**要求は来ない。**あの修正は一度も動いていなかった。
  - [x] 1.32 を取った（新しい ConPTY の上で、人が確認）。
- [x] **lazygit を起動すると、押していないキーが届いてメニューが開く（v0.49.0 で解決）。**
  **原因は Windows 標準の ConPTY の古さだった。**`FILER_PTY_LOG` と `keyprobe.ps1 -Query` で
  見ると、標準の ConPTY は tcell の問い合わせ（DECRQM と DA1）に**自分で答え**、応答の 1 文字ごとに
  離したレコードを付けてアプリに渡していた。filer は応答を 1 つも返していなかった
  （v0.48.7 の「応答を送り直す」実験が効かなかったのはこのため）。
  - 新しい ConPTY（NuGet の `Microsoft.Windows.Console.ConPTY`）を `filer.exe` の横に置くと、
    問い合わせは filer まで通り、応答も離したレコードも無く返る。コピー画面は出なくなり、
    Esc も効いた（実機で確認）。**Windows のリリースを zip にして同梱した。**
  - tcell の中でどう崩れてメニューになるのかは、最後まで特定していない。ソースから立てた
    仮説は 7 つとも外れた。直ったのは、仕組みを当てたからではなく、環境の違いを
    **測って**、違いの側を差し替えたから。
  - [x] 1.32 を新しい ConPTY で取った（人が確認）。Esc を win32 レコードで送る v0.48.6 の修正は、
    新しい ConPTY の上でも PSReadLine を壊していない。
  - [ ] 新しい ConPTY の上では、v0.48.6 の Esc の修正は要らないかもしれない（離したレコードが
    付かないので）。要らないと分かれば、同梱なしで動かしたときのためだけに残すか決める。 【実機】

## 実機のテストを無人で回す（v0.50.0）

- [x] `scripts/auto-wintest.ps1`: `main` の TESTING.md / TESTING-CHECKS.md / `windows-role.md` が
  変わっていて、`test/win-*` の PR が開いておらず、画面がロックされていなければ、実機のセッションを
  1 本起動する。タスク スケジューラで 15 分おき。Linux 上で `claude` と `gh` を偽物にして
  4 つの分岐（初回・変化なし・PR が開いている・作業ツリーが汚れている）を通した。
- [x] 順番表に、測れる節を 9 つ足した（36、40、30、14、33、24、39、31、22）。表の上の「測る方法」にも
  PTY ログ・子プロセスのコマンドライン・トースト・プロセスの生死を足した。
- [x] （2026-10-04 に閉じた。x64 のレーンが毎時 PR を出している）**実機で 1 回、本物の `claude` で通す**（TESTING.md に積むのではなく、持ち主が登録して
  最初の実行のログを見る）。確かめること: 権限の許可待ちで止まらないか（`--allowedTools` が
  足りているか）、`gh pr create` まで届くか、最後の行が `WINTEST_*` になっているか。
- [ ] 順番表が空になったら、残りの行（見た目の行）を人の作業として一覧にする。 【後】（空になるまで待つ）
- [x] ARM64 のノート PC を 2 本目のレーン（`-Lane arm`、`test/arm-*`）として足した（v0.51.0）。
  順番表は 26.5 / 26.6 / 25.5 / 41.8（ARM64 でしか確かめられない行）、リリースの `windows-arm64.zip`
  そのもの（同梱の ARM64 版 ConPTY を誰も動かしていない）、そしてネイティブのコードを通る節
  （1、21 / 32 / 37）の再確認。
- [x] （2026-10-04 に閉じた。ARM64 のレーンが毎時 PR を出している）**ノート PC で 1 回、本物の `claude` で `-Lane arm -Force` を通す。**

## 実機のセッションの報告（v0.52.3 で受けた分: #86、45 節）

- [x] **何もしていない filer が CPU を 1 秒あたり 1.0 秒使う**（最小化していても、#86）。**原因の最有力を直した
  （v0.54.2）**: プレビューのデバウンスのタイマー（`pending_since`）が、カーソルがファイルからディレクトリ・
  キャッシュ済みのファイル・表示中のファイルへ移ると消えずに残り、`main` がその間 16 ms ごとに再描画を
  頼み続けていた。つまり放置した窓が 60fps で描き続けていた（メインではないスレッドの CPU は、GPU への
  提出を担うドライバのスレッドと読める）。`request_preview` の入口でタイマーを取り出し、まだ待つ経路でだけ
  戻す形にした。テストは直す前のコードで落ちることを確かめた。
  - [ ] **これが測った CPU の原因だったかは、実機の 47 節で確かめる**（x64 の順番表の先頭）。まだ増えるなら 【実機】
    47.4 でスレッドを特定して、別の原因を探す。
- [x] **フォルダ比較の上限（10 万件）を左右で共有している**（バグ）。左が使い切ると右は 0 件しか読まれず、
  読まれなかったぶんが全部「左だけ」と出る。同じ `node_modules` 同士で `5000 only left · 0 only right`。
  上限を左右に分け、フッタに左右それぞれどこまで読んだかを出し、読み切れなかった範囲の `<` / `>` は出さない。
  - **直した（v0.54.5）。**上限を左右で半分ずつにし、読み切れなかった側があるときは、反対側にしか無いように
    見える行を出さない。フッタの「左右それぞれどこまで読んだか」はまだ（`cut short` の 1 語のまま）。
- [x] **フォルダ比較がシンボリックリンクをリンク先の中身で比べる**（バグ、45.11 が落ちた）。`walk` は
  `symlink_metadata` で記録するのに、`same_bytes` の `File::open` がリンクを辿る。`read_link` で比べ、行に
  行き先を出す（`~ ln  → t1.txt | → t2.txt`）。直したら 45.11 を実機でやり直す（`fx45.ps1`）。
  - **比べ方は直した（v0.54.5）**: リンク（シンボリックリンクとジャンクション）は `read_link` の行き先で比べ、
    中身は読まない。直す前の動きではテストが落ちることを確かめた。行に行き先を出すのはまだ。**45.11 を実機でやり直す。**
  - **ARM64 でジャンクションの形は通った（#98）**: 行き先の違うリンクが `~ ln  0 B → 0 B`、同じ行き先なら `= ln`。
    シンボリックリンクの形は、昇格も開発者モードも無い機械では作れず、45.11 は `[ ]` のまま（46.16 と同じく昇格が要る行）。
- [x] ディレクトリへのシンボリックリンクが `? dl`（大きすぎて読めない）と数えられる（バグ）。リンクとして扱う。
  上のリンクの修正で一緒に直った（v0.54.5）。Windows ではジャンクションでテストしている。
- [x] （v0.59.4 で直した。`App::toast_instead`。拡大縮小と `z` の表示も同じ扱いにした）同じ種類のトースト（`At the last difference` と `At the first difference`）は、新しいものが古いものを
  置き換える。今は矛盾する 2 つが並ぶ。
- [x] （v0.59.4 で直した。実機で 45.17）比較のツリーの行の区切り文字が混ざる（`thing\inner.txt` と `thing/`）。Windows では揃える。
- [x] （v0.59.4 で入れた。実機で 45.17）比較の見出しの下に、比べている 2 つのフルパスを出す。同名のフォルダ同士を比べるのが普通なので、
  名前だけではどちらがどちらか分からない。
- [x] 最初の差分にカーソルを置いて開く・`z` で `=` の行を隠す（Q23。v0.53.0）。

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
  埋め込みと書き出しは tsumugi でも要るので、共有のクレート（ito の `ito-pane` の側）に置くのがよい。

## GitHub の情報を見る口（Q16）

- [x] **「このファイルはどこから来たのか」を spot に入れた（v0.48.0）。**`Came in via` に
  PR 番号と merge のハッシュ、`From branch` にブランチ。**ネットワークを使わない** ——
  merge コミットの件名に番号が入っているので、ローカルの `git log` で足りる。
  Q16 で案 2 の障害に挙げた 46.8 / 46.9 は、`git log` が増えるだけなので影響を受けない。
  **案 2 を「ネットワークが要る」前提で評価したのが誤りだった。**要らなかった。
- [ ] **46.12〜46.16 を実機で取る**（Windows のセッション）。**46.16（回線を抜いて同じ出力）が 【実機】
  本体** —— ネットワークを使わないという主張は、実機で示すまで主張のまま。
- [x] （#265。gh-dash v4.26.0 をペインで動かし、起動時の余計なキー無し、`?` の開閉、`/` と `<Esc>`、`q` を `FILER_PTY_LOG` で読んだ。答えなかった問い合わせは OSC 11 だけ。入れたまま）gh-dash を入れて使ってみる（`gh extension install dlvhdr/gh-dash`）。2026-10-04、持ち主の許可で x64 の実機に任せた（`windows-role.md` の「The owner's leave」）
  **spot に入ったのは「ファイルが起点」の向きだけ**なので、PR を一覧で見る用途はまだ空いている。
  そこを既製品で埋めるかは、使ってみてから決める。
- [ ] 残った差分があれば、そのとき初めて案 3（別アプリと双方向 API）を検討する。 【後】
  2026-10-09、持ち主: これから TODO・システムモニター・エディターなどのアプリを作るので、そのときに決める。
  先に決めておくと安いもの（返答で提案した）: 別のアプリにパスを渡すのはコマンドライン引数（`filer <パス>`、`--line N`）を共通の形にする、
  設定の置き場所と名前の付け方を揃える、共有する部品はクレートにする。プロセス間の口（パイプ・ソケット）は 2 つのアプリが実際に話す必要が出てからでよい。

## リリースの macOS x64 ビルドがランナーを取れない

- [x] **案 1 を採った（v0.47.27）。**`macos-14` の上で `x86_64-apple-darwin` を
  クロスビルドする形に変え、`build.yml` も揃えた。**この項目は「判断は人に任せる」と
  書いてあったが、案 2 と案 3 が成果物の構成を変えるのに対し、案 1 だけは文書どおりの
  6 つに戻す案**で、リリースを完走させるにはこれが要る、と判断した。Intel 版をやめるか
  どうか（案 2）は人の決定のまま残っている。**リンクが通るかは実際に回すまで分からない**
  ので、v0.47.28 のリリースが実地の検証になる。そこで落ちたらこの項目を開き直すこと。
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

- [x] （v0.83.4。`scripts/release-notes.sh` が同じ版のコミットを 1 つの節にまとめる。手で直す必要は無くなった）次のリリースで、件名が重複している版のノートを手で直す。`v0.78.29:` が 2 つ（`d9f8153`、`0d0a6b6`）、`v0.78.31:` が 3 つ（`4a98776`、`858761a`、`7ff4e85`）あり、
  リリースノートは merge 以外のコミットの件名から作るので、同じ版の節が重なって出る（2026-10-05 に 3 本のセッションが並んで push した名残。v0.78.38 の `scripts/push-main.sh` からは起きない）
- [x] リポジトリを public にした（2026-09-26）。Actions の分数は無制限になり、
  溜まっていた failure は止まった。同時に v0.8.0 のタグとリリースを削除し、
  **v0.32.2 を初めて ARM64 込みでリリースした**（その経路は一度も走っていなかった）。
  - 動機は Actions の分数。private の無料枠は 2,000 分/月で、**Windows ランナーは 2 倍課金**
    なので実質 1,000 分。main への 1 push で Windows ジョブ 3 本（CI 1 + Build の x64/arm64 2）
    × 5〜8 分 = 約 40 分課金なので、50 push ほどで枯れる。実際 2026-09-26 02:02 に枯れ、
    それ以降のすべての run が**2 秒で failure**（ジョブが 1 ステップも始まらない）になった。
    public なら GitHub ホストランナーは無料・無制限。
  - 下ごしらえは 0.32.1 で済んでいる（`claude.yml` の OWNER 制限、例示アドレスの架空化）。
  - public にしたあとに確かめること:
    - [ ] 自分以外の `@claude` に反応しないこと。**実行しての確認は諦める。**
      他人のアカウントが要り、持っていないため。代わりに読んで確かめた:
      `if` は 3 つのイベントそれぞれで `author_association == 'OWNER'` を見ており、
      この値は GitHub が付けるのでコメント側からは詐称できない。
      **実地の確認は、外部の人が最初に `@claude` と書いたときになる。**
      そのとき run が 1 つも作られなければ正しい（Actions タブに何も出ない）。持ち主が Actions タブで見るまで待つ【人】
    - [x] Release のアセット URL が、サインインしていない人にも通ること
      （`release.yml` の冒頭コメントがこれを予告している）。
      x64 / arm64 とも匿名で **HTTP 206**。同じ手段で v0.8.0 のアセットが 404 を返した
      記録が v0.9.2 のコミットにあるので、public 化の前後で比較できている。
  - 任意: コミット著者 29 件が `DESKTOP-7UBRVG9\yuu06 <yuu0613@gmail.com>` になっている。
    GitHub の「Keep my email addresses private」を入れても**既存コミットは書き換わらない**
    （history 書き換えが必要）。今後のぶんは push 元マシンの `git config user.*` で止まる。
- [x] `sevenz-rust` から `sevenz-rust2` へ移行した（v0.33.2、Q11）。
  - 旧クレートはリポジトリが削除され unmaintained（RUSTSEC-2026-0246）。
    path traversal の脆弱性（RUSTSEC-2026-0245、8.3 high、修正版なし）もあった。
    **filer は `safe_dest` のおかげで影響を受けていなかった**が、根本から外した。
  - API は名前が変わっただけで、ロジックの変更はゼロ。`.cargo/audit.toml` の
    ignore も不要になったので削除した。
- [x] （2026-10-09 に確かめた。`v0.33.7` はもう `6a962b6` を指していて、`v0.32.2` のタグはもう無い。やることは残っていない）**リリースのタグを正しいコミットに付け直す**（人がやる作業。クラウドセッションからは
  タグを push できないため）。0.33.10 で原因は塞いだが、既存の 2 つは動かない。

  ```bash
  git fetch origin --tags
  git tag -f v0.32.2 1ed68b4 && git push -f origin v0.32.2
  git tag -f v0.33.7 6a962b6 && git push -f origin v0.33.7
  ```

  - リリース（アセットとノート）はタグ名に紐づくので、**消えない。**指す先が正しくなるだけ。
  - どちらもダウンロード 0 件なので、動かして困る人はいない。
  - **やらないと次のリリースのノートが壊れる。**「前のタグ」の位置がずれたままなので、
    v0.32.4 以降が丸ごと重複するか、逆に飛ばされる。
- [ ] MCP サーバーの認証・接続 【人】
  - engineering 系（GitHub、Slack、Linear、Notion、Asana、Atlassian、Datadog、PagerDuty）は未認証。claude.ai のコネクタ設定か `/mcp` で認証する。
  - obsidian の MCP サーバーに接続できない（ECONNREFUSED）。Obsidian 側でサーバーが動いているか確認する。
  - 2026-10-09、持ち主が「やりたい」。開発のセッション（Claude）が外のサービスに届くためのもので、filer の機能ではない。
    filer / tsumugi を LLM とつなぐ話は「LLM との連携」の節。
- [x] TODO.md / CLAUDE.md / QUESTIONS.md を git の追跡対象にした。クラウドのセッション
  （クリーンな clone）から中身が見えないと、毎回貼り付けないと作業を継続できなかった。
- [x] clippy の `collapsible_match` を直す（`src/main.rs:351`、Tasks オーバーレイの `q` 判定）。
  - タスクパネルの `q` をイベントループのハードコードから `[tasks]` キーマップへ移した際に消えた。
- [x] clippy の `manual_is_multiple_of` 2 件（`src/preview/image_preview.rs`、`src/spot.rs`）。
  - `x % 2 == 0` → `x.is_multiple_of(2)` に置き換えた。`--all-targets` で警告ゼロを維持している。
- [x] clippy の never_loop を直す（`src/main.rs` の Confirm の Text 処理）。
  - `for c in text.chars() { ...; break; }` を `if let Some(c) = text.chars().next()` に置き換えた。動作は同じ（先頭の 1 文字だけで答える）。
  - これで clippy がエラーなしで終わるようになった。残りは警告 5 件（`unnecessary_sort_by` / `manual_div_ceil` / `unnecessary_min_or_max` など）。
- [x] clippy の残り 5 件の警告を潰す。
  - `extend(x.drain(..))` → `append(&mut x)`（theme.rs 2 箇所）、`sort_by` → `sort_by_key`（sort.rs）、
    手書きの切り上げ除算 → `div_ceil`、意味のない `.max(0)` の削除（overlay.rs）。
  - テスト側の `== false` も `!` に直した（glob.rs）。`--all-targets` で警告ゼロになった。

## Linux レーン（v0.59.6）

- [x] クラウドのコンテナで Xvfb と lavapipe により filer が起動し、`--keys`・クリップボード・ペイン（bash）まで動くことを確かめた。
  役割の定義（`.claude/linux-role.md`）、`TESTING-LINUX.md`（`make-testcheck --lane linux`）、`scripts/xrun.sh`、
  `scripts/make-fixtures.sh` を足した。
- [x] （2026-10-04 に閉じた。TESTING-LINUX.md に 16 行の印があり、もう回っている）最初の 1 本を走らせる（順番表の先頭、25 節）。PR が来たら `merge-role.md` のとおりにマージする。
- [x] （2026-10-04、持ち主の判断: 定期実行にせず、Linux レーンそのものを止めて Windows に集中する。Linux には yazi があり、GUI 版の需要は小さい）定期実行の Routine にするか決める（今は手で `create_session` する）。Windows のレーンは 15 分おきの監視だが、
  クラウドのセッションは走らせるたびに費用がかかる。
- [ ] 【後】Linux レーンを再開する（今後の開発。`linux-role.md` の順番表は 2026-10-04 の時点のまま残してある。再開するなら Routine にするかも合わせて決める）
- [x] Linux で日本語の名前が □ になる。**フォントを入れても □ のままだった**（filer は Windows のフォント置き場しか探して
  いなかった）。v0.59.7 で、Linux（Noto CJK、IPA、Droid）と macOS（ヒラギノ）の置き場を足し、等幅の ASCII の後ろに
  補いとして入れた。`fonts-noto-cjk` を入れたコンテナの Xvfb で、日本語が描かれ ASCII が等幅のままなのを画面で確かめた。
- [x] `xrun.sh` が `--keys` の終わりを待たず、`<Wait:N>` の合計が `XRUN_WAIT` を越える手順を押しかけのまま読んでいた
  （#134 の所見 1。v0.60.0 の確認でも `a` が押されずに当たった）。**v0.60.1 で、filer が `FILER_KEYS_DONE` のファイルを
  最後のキーが落ち着いたときに書き、`xrun.sh` がそれを待つようにした**（既定 120 秒で打ち切り、`keys.done` が無ければそう言う）。
- [ ] 【後】Wayland は確かめられない（コンテナに合成器が無い）。`weston --backend=headless` が apt で入るなら、それで試す。

## 実機のセッションの報告（v0.65.4 で受けた分: #154、ARM64 の 1 節のマウス）

- [x] 1 節のマウスとカーソルの 8 行にチェック（242 → 250）。v0.64.3 のスクリーンセーバー止めが効いて、ARM64 で初めてマウスの行が取れた。
  クリップボード（選択は離したときに自分でコピーする）、テーマの色と 1 桁まで一致するピクセル、`dir` の行番号で測った。
  1 節で残るのは 1.2 と 1.8 で、どちらも見た目（持ち主の目）。ARM64 の順番表から 1 節を外した。次は 22 節。
- [ ] **実行の途中でスクリーンセーバーが一度戻った**（所見 1）。`OpenInputDesktop` が `Screen-saver` を返した測りが 1 回あった。v0.64.3 の見張りは 【実機】
  5 秒おきに `*.scr` を止めるので、その窓に重なったのか、見張りが止まっていたのかは分からない。ログ（`During the run the watcher stopped …`）で
  回数を確かめ、間隔を 1 秒に詰めるか、`ES_DISPLAY_REQUIRED` を見張りの中でも出し直す。実機のセッションの側では、測る前後に入力デスクトップを
  確かめる手順が既にあり、それで誤った印は防げている。
- [x] `--keys` に `<Shot:name>`（その時点の画面を名前付きで保存）を足す（提案 2、Q42 → v0.67.0）。1.3 / 1.3a は 2 つのキーの間の画面が要り、
  1 枚ごとに filer を起動し直していた。
- [x] （v0.78.2。`focus:` と `pane cursor:`）`FILER_KEYS_DONE` の状態に、ペインのカーソル位置とフォーカスを足す（提案 3）: `pane cursor: 10,4`、`pane focus: true`。
  1.3 / 1.3a / 1.9e / 1.10 がどれもこれをピクセルで数えていた。
- [x] （v0.75.16、QA の子が TESTING.md を直した）TESTING.md 1.11 / 1.12 の横に測り方を書く（提案 4。人か QA セッション）:「選択は自分でコピーする。選ばれたセルは `theme.hovered_bg`」。
- [x] （v0.75.16、QA の子が TESTING.md を直した）TESTING.md 1.9d の「なめらかに」を「1 ノッチごとに必ず動く（1〜2 行、両方向）」にする（提案 5。人か QA セッション）。
- 罠として記録（filer の不具合ではない）: PowerShell 7.6 の `Write-Output` で書いた ANSI の色はペインに届かない（`[Console]::Out.Write` なら届く）。
  PSReadLine の履歴の予測が、打っていない文字をプロンプトの行に出す（「打ったもの」を行から読むと間違う）。

## 実機のセッションの報告（v0.67.7 で受けた分: #166、ARM64 の 17 節）

- [x] 17 節（自前のプレビュー）の 18 行すべてにチェック（280 → 298）。画面の文字、`FILER_KEYS_DONE` のトースト、`PATH` の先頭に置いた
  記録用の包み（どの `{n}` で何回走ったか）で測った。ARM64 の順番表から 17 節を外し、20 節を足した。次は 12 節。
- [x] （v0.78.17。`PreviewSlot::in_flight`。17.15）**`App::settled()` が、外部のプレビューの次の絵を待たない**（所見 1、不具合）。`request_preview` の `relayout` の判定が `key.n` を見ず、
  `go_to_picture` が `pending_since` を消すので、ページを送っても「落ち着いた」ままになる。`--keys "<Shot:p1><A-j><Shot:p2>"` で同じ絵が 2 枚撮れる
  （`-f 1` と `-f 2` は両方走っている）。古い絵は出したまま（`relayout` の目的）、「描き直し中」の印を足して `settled()` から見えるようにする。
- [x] （v0.78.101。UTF-8 でなければ OEM コードページで読む。17.12 を両レーンの再テストに積んだ）**外部のプレビューの標準エラーを UTF-8 として読んでいる**（所見 2、不具合、Windows）。日本語の Windows では子の `cmd` が CP932 で答えるので、
  17.12 の文言が文字化けした（`external.rs:44`）。17.5 の `No more: …` も同じ道を通る（pdftoppm が英語で答えたので読めただけ）。
  端末から試すと `chcp 65001` で隠れる。Windows ではコンソールのコードページ（`GetACP` / `GetOEMCP`）で読む。
- [ ] 12 回に 1 回ほど、スクリプトの途中で `filer.exe` が消えた（所見 3）。`FILER_KEYS_DONE` も Windows のエラー報告も無く、その後の 4 回では再現しない。 【実機】
  印はどれもこの実行には頼っていない。もう一度出たら、`filer env` の記録と一緒に追う。
- [x] （v0.78.17。`picture:`）状態ファイルに、プレビューの絵の番号（ページ・秒）を出す（提案）。17 節の 7 行が画面を撮らずに済む。
- [x] （v0.78.1。カードの値を欄の幅で折り返す）メタ情報のカードの `Note` を折り返す（提案）。入れるべきツールの名前のところで切れている。

## 実機のセッションの報告（v0.67.13 で受けた分: #171、ARM64 の 20 節）

- [x] 20 節の 20.1 / 20.2 / 20.3 / 20.4 / 20.6 にチェック（300 → 305）。`FILER_CONFIG_HOME` / `YAZI_CONFIG_HOME` を走らせる間だけ別のフォルダにし、
  `title_format` に `{rows}` を入れて文字の大きさを数で、`<Shot:>` の画素の数で色とミニマップを測った。ARM64 の順番表から 20 節を外した。
- [x] **20.5: 壊れた `filer.toml` で `<C-F5>` を押すと、`[ui]` が既定値に戻る**（所見 1）。トーストは正しいが、`font_size = 28.0` が
  効かなくなる（タイトルの `rows=17` → `33`）。`Config::load()` が `ui` を `Ui::default()` から始め、読めたときだけ入れ替える
  （`src/config/mod.rs`）のを、`reload_config` がそのまま使う。読めた他のファイル（`theme.toml`、`keymap.toml`）は残る。v0.67.18 で Q47 の 1 のとおり直した。
- [ ] **`--keys` の実行が 7 回に 1 回、何も残さずに消えた**（所見 2）。`<Shot:before>` のあと 9 秒でプロセスが無くなり、`FILER_KEYS_DONE` も 【実機】
  Windows のエラー報告も無い（#166 の所見 3 と同じ形）。v0.67.12 の見張りは「止まった」場合しか拾わないので、パニックのフック（提案 3）:
  `FILER_KEYS_DONE` の横に `<名前>.panic` を書き、0 でない終了コードで終わる。
- [x] （v0.81.0、Q84 の 1。「 — the rest in `~`」。33.7 / 33.10 を書き直し、33.6 / 33.7 / 33.9 を再テストに回した）長い設定エラーのトーストが窓の半分を覆う（提案 4）。1 行目に「— `<F1>` で続き」を付けて出し、全文はヘルプと `filer env` に任せる。
- [x] （v0.67.16、Q48 の回答どおり `C`）ヘルプで、キーの一覧を文字として取り出せるようにする（提案 1）。20.3 の「`<F1>` に出る」が絵からしか読めなかった。

## 実機のセッションの報告（v0.78.28 で受けた分: #262 x64 の v0.78.23 の再テスト、#263 ARM64 の 46 節）

- [x] #262（x64、v0.78.24）: v0.78.23 の 3 つの直しを確かめた。`e` / `E` / `r` / `a` の直後の `<State:>` が作ったものを読む（`e` は 8 回中 8 回）、
  書庫を出ると `filer-archive-<pid>` が 0.7 秒で消える、`cargo test` が `filer-archive-*` を残さない。所見なし。#263（ARM64、v0.78.24）: 46 節の 21 行中 19 行が
  ARM64 の `git.exe` で x64 と同じ（印は x64 のもの）。両方 703 / 0
- [x] （v0.78.29。キーは keymap の `compress` から読む。21.6a）`e` をフォルダの上で押すと「ファイルが書庫ではない」と言う（#262 の提案 1）。`inner\ is a folder -- e unpacks an archive …; to pack it, press E` にする
- [x] （v0.78.51。`preview lines: N`、切られたときは `N+`）状態ファイルにプレビューの行数を出す（#262 の提案 2: `preview top: 0 of 14` は最後の先頭位置で、30 行のファイルの行数が読めない）
- [ ] `git` などの出力を読むだけの子プロセスを `DETACHED_PROCESS` で起こす（#263 の提案 1。`CREATE_NO_WINDOW` は窓を隠すだけで、`git.exe` ごとに `conhost.exe` が 1 つ立つ。
  spot を 6 回開いて `conhost` が 21 個）。`src/fs/git.rs`、`exec.rs`、`fs/ops.rs`、`preview/external.rs` の 4 か所。`cmd /S /C` の外部プレビューはコンソールが要らないかを確かめる【実機】（`cmd` を `DETACHED_PROCESS` で起こすと、その子の console アプリが新しい窓を作って
  ちらつくおそれがある。`git.exe` 直起こしだけなら足せるが、Linux では測れず、`conhost` の数と窓のちらつきは Windows でしか見えない）
- [x] （v0.78.184。子の 2 つで済んでいる。残る `rev-parse` と `merge-base` の 2 回ずつは引数が別の問い）spot の Git の節の `git` 呼び出しを減らす（#263 の提案 2: 1 回で 8 回。`rev-parse` と `merge-base` が 2 回ずつ、`remote.origin.url` は毎回）。リポジトリの根ごとに、
  根・`remote.origin.url`・`origin/HEAD` を覚える
  - [x] （v0.78.82）`git::origin` の `rev-list` と、その後の `log -n1` を、`--format=%H%x00%h%x00%s` の `git log` 1 回にまとめた（1 回減る）
  - [x] （v0.78.179。`.git` ごとに `config`・`refs/remotes/origin/HEAD`・`packed-refs` の更新時刻と大きさで覚え、変われば捨てる）`remote.origin.url` と `origin/HEAD` を、リポジトリの根ごとに覚える
- [x] （v0.80.7。TESTING.md と訳を直した）TESTING.md 46.11 に何を測るかを書く（#263 の提案 3: `conhost.exe` の数ではなく、`ConsoleWindowClass` の見える窓が増えないこと。上の `DETACHED_PROCESS` が入れば数で測れる）【QA】
- [x] （v0.78.176。`C` の出力だけ `#42` と `bb23a4e` の間を TAB にした。見た目は変えない）spot の `C` で `Came in via` の 2 つの値を分ける（#263 の提案 4）
- [ ] （所見）前の run（2026-10-04 12:02 の 37 節）が開いた Chrome のタブが ARM64 機に残っている（#263 の所見 1、#256 の所見 2 と同じ）。v0.83.2 から実機の run が閉じる（`.claude/windows-role.md` の「Earlier runs' leftovers」。下の #256 の項目と一緒に片付く）【実機】

## 実機のセッションの報告（v0.78.19 で受けた分: #256 ARM64 の再テスト）

- [x] #256（ARM64、v0.78.18）: 17.15、21.18、21.24 が ARM64 でも成り立ち（`[x]` は x64 のもの）、18.13・21.19・21.20 も 2 台目として同じだった。
  37.8a は 6 項目中 Edge・Neovim・VS Code が起動、`.pdf` の既定と サクラは行の認める飛ばし、Chrome だけ残った
- [x] （v0.80.8。行を分けた）TESTING.md 37.8a を「ブラウザ以外の項目」と「ブラウザの項目（持ち主のブラウザの窓が 1 つも無いときだけ）」に分ける（#256 の Queue 2）。
  今の形では、持ち主が Chrome を開いたまま使う機械で永久に `[x]` にならない【QA】
- [x] （v0.80.7。TESTING.md と訳を直した）TESTING.md 25.11 を Linux / macOS の行にする（#256。Windows では `system_bold_fonts()` が必ず 3 つの太字を候補に入れるので到達できない）【QA】
- [x] （#265。退避して戻し、前後の SHA-256 が一致。リンク先も変わらず）TESTING.md 35.2 は持ち主の `%APPDATA%\yazi\config\yazi.toml` を書き換える行（#256）。2026-10-04、持ち主の許可で、退避して戻す形で x64 の実機に任せた
- [x] （v0.78.39。`<C-j>` / `<C-k>` を足し、4 つを `[pick]` とヘルプ・ヒントに出した。動かすのは今も `src/main.rs` の書き込みで、keymap からは引かない）picker（`O` / `<S-Enter>` / パレット）で `j` は絞り込みの文字になる（#256 の提案 1。`jj` で `Nothing matches`）。Q80 は多数決で 1。
  `<C-n>` / `<C-p>` は `src/main.rs` の `Overlay::Pick` に書き込まれて既に動く（#264・#265 の所見）ので、それを `[pick]` の keymap へ移し、
  `<C-j>` / `<C-k>` を足し、4 つとも足の行とヘルプに出す（#264 の提案 2、#265 の提案 2。「キーは keymap 経由で動かす」）
- [x] （v0.82.0、Q92 = 1。`exec::starts_default` の行とオープナー無しは開く前に引き、無ければ `No default app for .xyz` で開かない。16.13・16.13a）`start ""` の既定のアプリが何も開かないとき、走らせる前に `AssocQueryStringW(ASSOCSTR_EXECUTABLE)` で引いて言う（#256 の提案 2。
  #252 の提案 2 の項目と同じ問題を、開く前に見る形。どちらにするか合わせて決める）
- [ ] （所見）ARM64 の持ち主のごみ箱に、前の run の scratch から来たものが 38 個（#256 の所見 1。#231 の x64 と同じ）。前の run の Chrome のタブも 1 枚
  残っている（所見 2）。役割定義の後片付けが前の run の分に届いていない。v0.83.2 で `.claude/windows-role.md` に「前の run の残り物も片付ける」
  （ごみ箱は元の場所が scratch の根の下のものだけ、Chrome は `file:///` が scratch か fixture の下のタブだけ）を足した。実機の run が 1 回片付けて数を報告したら済み【実機】

## 実機のセッションの報告（v0.78.15 で受けた分: #251 x64 と #252 ARM64 の書庫の中）

- [x] #251（x64、v0.76.3〜0.77.0）: 18.13、21.17、21.19、21.20 に `[x]`、2.2・3.11・24.7 に `[~]`、TESTING-KEYS.md の 4 キー（`<A-g>`、`<A-G>`、ヘルプの `C`、
  比較の `<Enter>`）。#252（ARM64、v0.76.0）: 21.17、3.11、24.7 に `[x]`（3.11 はプレートの輝度と角の画素、24.7 は画像から読んだ省略形で、見た目の行ではなく
  測れた）。21.19 の `[x]` は v0.77.0 の文言で x64 が付けたものを残した。32.15 / 32.16 は ARM64 でも x64 と同じ結果
- [x] （v0.78.16。書庫の表示を抜けるときに中の選択を外し、ごみ箱のジョブは無いパスを名指しの誤りにして数えない。21.24）**書庫で選んだまま `<Esc>` / 一番上の `h` で出ると選択が残り、`d` が `Trashed 2 item(s) — u to undo` と言って何も捨てない**（#251 の所見 1、提案 2）。
  書庫の表示を抜けるときに選択を空にする。ごみ箱の側も、言う数は実際に動いたものだけにする
- [x] （v0.78.16。21.18）**書庫のメンバーの写しが、書庫の中のフォルダを落とした場所に置かれる**（#251 の所見 2、提案 3）。`arc3\tiny.png` が `filer-archive-<pid>\arc3.zip\tiny.png` に
  なり、`a\readme.txt` と `b\readme.txt` が同じ写しを取り合う。中のパスを保って置く。filer を閉じた後もフォルダが残っていた（次の起動の掃除は 24 時間後）
- [x] （v0.78.16。`nested\deep.txt` を開き、`.txt` が使えなければ `.png` を足して使う形にした）TESTING.md 21.18 の `.txt` を「既定のアプリが動いていない種類のメンバー（`.png` など）」にする（#251 の提案 1: x64 は `.txt` の既定が持ち主のサクラエディタ、
  #252 の所見 1: ARM64 は `.txt` の関連付けが壊れていて filer の外からも開かない）。直すまで 21.18 は両レーンで取れない
- [x] （Q92 = 1 で、開いた後の `hProcess` は見ないと決まった。開く前に引く形は v0.82.0。関連付けはあるのに起動に失敗する場合は捉えない）既定のアプリで開いたのに何も起きなかったとき、そう言う（#252 の提案 2）。`open::that_detached` は関連付けが起動に失敗しても `Ok`。Windows では
  `ShellExecuteExW` に `SEE_MASK_NOCLOSEPROCESS` を付け、`hProcess` が無ければ `Opened a copy of <name>, but Windows started nothing for .txt`
  （DDE や既に動いているアプリへの合流でも `hProcess` が無いことがあるので、言い方は設計が要る）
- [x] （v0.78.92。`exec::substitute_line`、Q82 反映）`:` の行にカーソルの下のパスを末尾に足すのを、`&` `|` `>` を含む行ではやめるか、`%*` が無ければ足さない（#252 の提案 3、#250 の所見 5。2 台が独立に当たった）。
  既定の動きを変えるので QUESTIONS.md に出す。**Q82 は多数決で 1**（`&` `|` `>` `<` `;` のいずれかを含む行には足さない。置き場所を書けば入る。x64 の #270 が `;echo hello >out.txt` で、足したパスがリダイレクトの中身に入るのを見た）
- [x] （v0.78.16。`archive:`）状態ファイルに、書庫の中のどこにいるかを出す（`archive: <書庫>\<中のフォルダ>`。#251 の提案 5、#252 の提案 4）。空のフォルダでは `hovered:` が空で段が読めない
- [x] （v0.78.84。`parent: a | b…c.log | …`、列が描かれているときだけ）状態ファイルに親の列の表示文字列を出し、24.7 を文字で読めるようにする（#251 の提案 4）
- [x] （v0.78.193。`archive_refusal`）書庫の中の拒否のトーストを、書庫を書き換える `d` / `a` と、まだ無いだけのものとで分ける（#252 の提案 1。v0.77.0 で `y` は動くようになったので、`x` などの残り）
- [ ] （所見）`<A-G>` の後の画像が 1 度だけ先頭のままだった（#251 の所見 3。状態ファイルは `preview top: 3984 of 3984`）。以後 7 回再現せず、画像も残っていない。
  再現したら `<Shot:>` を残すよう、x64 の順番表に書いておく【実機】

## 実機のセッションの報告（v0.75.20 で受けた分: #247 x64 と #248 ARM64 の 47.7）

- [x] #247 / #248: 47.7 に印（同じ行を 2 台が確かめた）。`"directx"` と `"metal"` は窓で `drawing with Gl instead`、`filer env` は `drawing with the default`、
  `Adapter` は `(Gl, …)`、何もしない窓は 0.000 CPU 秒。両レーンの再テストから 47.7 を外した
- [ ] **x64 で `"vulkan"` の対照も回らなくなった**（#247 の所見 1）。#243 / #246 では約 10.0 CPU 秒だったのが 0.000（10 秒と 20 秒の 2 回）。
  違いは画面の倍率で、今回は 150%（#246 は 100%）。47.1 の空回りが倍率に依るのかもしれない。100% と 150% で `vulkan` を測り直す（#247 の提案）【実機】
- [x] （v0.78.201。`last-run.toml` が実際に使った `backend` と設定値を持ち、`Adapter` の行が `set by …` で言う。v0.78.148）`filer env` の `Last run` が、直前に読んだ設定ではなく機械の最後の起動を言う（#248 の所見 1）。`FILER_CONFIG_HOME` は `State` を動かさないので、
  `"dx12"` で起動した後に `"directx"` の設定で `filer env` を読むと、`Backend : "directx"` の横に `Adapter : (Dx12, …)` が並ぶ。
  実際に使ったバックエンドを `Last run` に残す（#248 の提案 2。#245 の提案 1、上の `last-run.toml` の項目と同じ）
- [x] （v0.78.107。`filer env` も `name_the_fallback` を通す）無効な `[ui] backend` の警告を、窓と `filer env` で同じ文にする（#248 の提案 1）。窓は `drawing with Gl instead`、`filer env` は
  `drawing with the default` で、同じ判断を別の言葉で言っている。落ちた先を言う関数を 1 つにして両方から呼ぶ
- [x] （v0.78.87。24 文字を超えるキーは自分の行に出し、値をその下に字下げする）`filer env` の `Config` の欄の幅が、設定フォルダのパスの長さで決まる（#248 の所見 2、提案 3）。67 文字の `FILER_CONFIG_HOME` で
  `State` などが 83 桁目から始まり、普通のコンソールで折り返す。フォルダのパスを 1 行に出してファイルをその下に字下げするか、欄の幅に上限を付ける
- [ ] ARM64 で 25 節（25.11）は取れない（#248）。25.11 は「どこにも太字が無い」を要るが、`system_bold_fonts()` は `C:\Windows\Fonts\meiryob.ttc` などを
  絶対パスで見るので、普通の Windows ではその状態を作れない。33.9 は 5 つの箱が重ならないこと（画素でしか測れない）【実機】

## 実機のセッションの報告（v0.73.76 で受けた分: #233 x64 の 26.1 と 31 節、#234 ARM64 の 37 節と 32.5）

- [x] #233: 26.1（`[o] / <Enter>`、Q69）、31.2、31.15 に印。x64 の順番表から 29 節（Windows の行は済み）と 33 節（残りは見た目だけ）を外し、
  31 節は昇格の要る 31.7 / 31.8 と書き直し待ちの 31.5a だけにした
- [x] #234: 32.5 に印（空白入りの名前が、ピッカーの順から Neovim、Chrome、Excel、VS Code まで 1 つの引数で届く）。32 節に Windows の行は残っていない。
  37.1〜37.7 は ARM64 でも合格。ARM64 の候補を 5、10、6 節にした
- [x] （v0.75.16、QA の子が TESTING.md を直した）TESTING.md 31.14 / 31.15 に注記: 同じ止まったアドレスへの 2 回目は、Windows が失敗を覚えていて 1 秒で os error 1203 になる。最近試していないアドレスを使う（#233 の所見）
- [x] （v0.75.16、QA の子が TESTING.md を直した）TESTING.md 37.8 を「ピッカーの並び（`pick:` で読める）」と「起動」に分ける（#234 の提案 1）。起動の側は、入っていないプログラムと関連付けの無い拡張子を
  理由を書いて飛ばしてよいことにする。2 レーンとも機械の事情でこの行を残している
- [x] （v0.75.16、QA の子が TESTING.md を直した）TESTING.md 37.7 の「直接起動」が実際と合わない（#234 の所見 2）。`block = false` のオープナーはどれも `cmd /S /C` を通り、フルパスのときは
  `"cmd" /S /C ""…\msedge.exe" "…\doc.pdf""` で、プログラムがその子になる。「行に `start` が無く、自分のコンソール窓が出ない」と書く
- [x] （v0.78.61。件数は `items:`、位置は `position: N/M`。空の理由の `empty:` は未）`FILER_KEYS_DONE` に見出しの件数と位置を出す（#233 の提案 1）: `header: 500 items`（待っている間は `listing…`）、`position: 4/500`、空なら
  `empty: (no shares)`。上の #231 の `items:` / `filter:` の項目と一緒にやる
- [x] （v0.78.75。23.3a）`\\host` への移動が os error 1203 / 53 / 67 で失敗したとき、OS の文のあとに `— no host by that name answered (a typo, or the machine is off)` を足す
  （#233 の提案 2）。2 回目が速いのは Windows が失敗を覚えているから、とも言える
- [ ] ピッカーで、入っていないプログラムの項目に ` (not installed)` を付ける（#234 の提案 3）。`PATH`、`App Paths`、引用符の中のフルパスで 1 回だけ調べる。
  同じ README の例が、持ち主の 2 台で違う意味になっている。ピッカーを開くたびの費用を測ってから【実機】（`App Paths` のレジストリ引きの費用は Windows でしか測れない）
- [ ] 持ち主の ARM64 機で `.pdf` と `.html` に既定のアプリが無い（`HKCR\.pdf` に ProgId が無い。#234 の所見 1、3 回目）。filer の問題ではない。
  直すなら「既定のアプリ」で選び直す。2026-10-09、持ち主: Chrome でよい。Windows は既定のアプリをスクリプトから変えさせない（`UserChoice` のハッシュ）ので
  持ち主の手で: 設定 → アプリ → 既定のアプリ → Google Chrome → 「既定値に設定」（`.pdf` と `.html` が Chrome になったか、その下の一覧で見る） 【人】
- [x] `Start-Process -Wait` はプログラムを起動する run では戻らない（#234 の所見 3）、Chrome のタブを窓ごと閉じない（#234 の所見 4）は、`windows-role.md` に書いた
- [x] 確認の箱に無いキーで箱が黙って閉じる（#233 の提案 3、Q71）。v0.73.78 で、箱に無いキーは無視するようにした。TESTING.md 26.12 を足した

## 実機のセッションの報告（v0.73.48 で受けた分: #217 ARM64 の 26 節、#218 x64 の 3 節）

- [x] #218: 3.1〜3.10 に印（画像の拡大と移動、ピクセルで測った）。x64 の順番表から 3 節を外した。
- [x] #217: 26 節は印なし。26.8 / 26.10 は今の作りでは書いたとおりにならない（下）。26.1 / 26.3 / 26.4 は ARM64 でも x64 と同じ。
- [x] （v0.73.50。Q61 の回答どおり `open` の `shellexecute-on-windows` を有効にし、`ShellExecuteExW` の結果を答えにした）**`<F12>` と、オープナーの無いファイルの `<Enter>` が、ブラウザやアプリが開かなくても成功と言う**（#217 の所見 1）。`open::that_detached` は
  Windows で PowerShell が起動した時点で `Ok` を返す。`<F12>` は報告の URL をどこにも残さない。直し方は Q61（`open` の `shellexecute-on-windows`）。
- [ ] TESTING.md 26.8 / 26.10 を「起動役が動かない」（今も合格）と「関連付けが壊れている」（Q61 待ち）の 2 行ずつに分ける（#217 の所見 2）。 【QA】
  **前提が古い**（v0.75.16、QA の子）: Q61 は v0.73.50 で入り、今は `ShellExecuteExW` を直接呼ぶので、置き換えの `powershell.exe` で
  「起動役が動かない」を作る測り方が成り立たない。関連付けが壊れた状態を実機でどう作るか（`UserChoice` はハッシュで守られている）を先に確かめる。【実機】
- [x] （v0.78.215。すでに入っていた: `<F12>` の `App::bug_report` が直前 20 キーを `keys` 欄に、最新のエラーを `context` 欄に入れる）バグ報告のフォームの「再現するキー操作」に、直前の約 20 キーと最新のエラーのトーストを入れる（#217 の提案 2）。キーの輪のバッファと
  URL の引数 1 つ、`bug_report.yml` の欄の id 1 つ。
- [x] （v0.78.208。README は書いてあり、`fetch-conpty.ps1` が `filer.com` を置くので、`windows-role.md` にその一言を足した）`filer.com` は zip の中の名前で、手元の `cargo build --release` は `filer-com.exe` を作る（#217 の提案 3）。`windows-role.md` と README に
  そう書くか、`fetch-conpty.ps1` のように手元の release にも `filer.com` を置く。
- [x] （v0.73.49。画像の左上の角を物理画素にそろえる `on_pixels`）**等倍（`<A-1>`）の画像が画素の格子から 0.26 / 0.5 px ずれて描かれ、グリッドの線が本来の色で出ない**（#218 の所見 1、提案 1）。
  `Rect::from_center_size(avail.center() + pan, size)` を整数の画素にそろえる。直したら 3.2 を x64 の再テストへ。
- [x] （v0.73.49。`preview::image_area` を描く側と押さえる側の両方が使う）**画像の移動の限界と描く範囲がずれていて、端に寄せると 2〜2.5 px が切れる**（#218 の所見 2、提案 2）。`image_input` は `rect.shrink(8.0)` で
  押さえ、`preview::draw` は `rect.shrink(2.0)` をさらに 8 縮めて描く。同じ矩形を使う（`preview.fit` も同じ矩形から）。直したら 3.4 を再テストへ。
- [x] （#217 の Q60 の票にある「同じコマンドなら出さない」は、Q60 の実装の項目に足した）

## 実機のセッションの報告（v0.67.20 で受けた分: #176、ARM64 の 29 節）

- [x] **`filer env > out.txt` が PowerShell では 0 バイトのファイルになる**（#176 の所見と提案 1）。PowerShell の `>` は GUI サブシステムの
  プロセスを待たずにパイプを閉じる（`>` は 25 ms で戻り、パイプ渡しは 85 ms）。パイプ、`cmd /c "filer.exe env > out.txt"`、
  `Start-Process -Wait -RedirectStandardOutput` は取れる。`src/main.rs` の `say()` の doc コメントは「ファイルにもパイプにも出る」と言っている。
  `filer --help` の末尾と README の `filer env` の所に 1 行
  `Windows: PowerShell's > does not wait for a GUI binary; use a pipe, cmd /c, or Start-Process -Wait` を足し、doc コメントも直す。
  **#183 で範囲が広がった**: `>` だけでなく、filer が**パイプラインの最後**にあると PowerShell は待たない。`$v = & filer env` は 13 ms で
  0 行（`$null`）を返し、`@(& filer env)`、`& filer env 2>&1`、`& filer --version` / `--help` も同じ。後ろに何か 1 つ置けば
  （`| Write-Output`、`| Out-String -Stream`、`| Out-File`）48 行とも届く。README の例外（1260〜1263 行あたり）を「`>`」から
  「filer をパイプラインの最後に置かない」に広げ、`$v = & filer env | Write-Output` の形を例に挙げる（#183 の提案 1）。
  v0.71.0 の `filer.com` で `filer env > out.txt` も `$v = & filer env` も届くようになり、README は同じ版で書き直した（`filer.exe` を直接呼ぶときの
  空振りと `| Write-Output` の例も）。`--help` の 1 行は v0.73.8 の `| Out-Null` の行が代わる。残っていた `say()` の doc コメントを v0.73.13 で直した。
- [ ] **`say()` が標準出力への書き込みに失敗したとき、黙って捨てずにコンソールへ回す**（#183 の提案 2）。今は `let _ = writeln!` で戻る。 【実機】
  まず、パイプの最後に置いたときに書き込みが本当にエラーを返しているのかを Windows で測る（#183 は測っていない）。返していなければ
  この直しは効かないので、その結果を書いて閉じる。
- [x] （v0.73.4 で足した。直近 16 本、消えたものも）**`FILER_KEYS_DONE` に、その実行で出たトーストを全部残す `toasts:` 行を足す**（#190 の提案 2 も同じ。#176 の提案 2）。今の `toast:` は書く時点で出ているもの
  だけなので、トーストが期待値の行（29.7、12.17 / 12.18、13.8a など）は消える前に `<Shot:>` を撮って画像から読むしかない。
  `App` に直近 16 本ほどの履歴を持ち、`state_report` に `|` 区切りで 1 行。今の `toast:` 行は残す。
- [x] （v0.69.0、Q50 の 1）**README の OSC 7 フックを filer から出す**（#176 の提案 3）。`filer shell-hook [pwsh|bash|zsh]` を足し、
  フックが無いときの `<A-Up>` のトーストが `filer shell-hook | Add-Content $PROFILE` を挙げる。README のブロックとの一致はテストで保つ。
- [x] （v0.70.0、Q51 の 1）**ペインのシェルを 1 回の起動だけ変える `FILER_TERM_SHELL`**（#176 の提案 4）。`[term] shell` より優先し、
  他の設定は落とさない。29.7 / 8.2 のように別のシェルで確かめる行は、`FILER_CONFIG_HOME` を空のフォルダに向けなくてよくなった。

## 実機のセッションの報告（v0.72.4 で受けた分: #194、x64 の再テスト）

- [x] v0.72.2 の zip で 48.1（5 ファイル）、48.3（PE machine が x64 は `8664` ×4、ARM64 は `AA64` ×4）、25.19c（`filer.com` の終了コードが
  `--keys "<Tab"` でも 2 つのパスでも 2、10 回ずつ）に印（329）。48.6 は x64 の zip で合格、ARM64 の zip の半分を ARM64 の順番表に積んだ（提案 2 は
  行を分けずに順番表で持つ形にした）。`cargo test` は 623 / 0。Q57 に win の票（1）が入り、3 票とも 1。
- [x] **2 つのパスを断るメッセージがパスの `\` を二重に出す**（#194 の所見）。``more than one path: "C:\\dev" and "C:\\Windows"`` になる。
  `main.rs` の `take_path` が `{:?}` で書いている。引用符だけ付けて `\` はそのまま出す。
  v0.73.5 で直した。TESTING.md の 25.24a（新）を両レーンの再テストに回した。
- [x] （v0.73.33）Q57 を実装するとき、負けた束縛の `run` も警告に並べる（#194 の提案 1）: ``… only `hidden toggle` runs, not `quit` ``。出どころのファイルと一緒に。
- [ ] **CI の Windows の `test` が 1 回、`cargo test` の中で 30 分以上止まった**（2026-10-03、#194 の CI、run 37088219242）。同じコードは main で
  4 分で通り、取り消して流し直すと 5 分で通った。取り消した実行のログは取れなかった（404）。また起きたら、取り消す前に
  `cargo test -- --test-threads=1` などで止まるテストを特定する。止まり続けるテストがあれば、それは不具合。 【後】（また止まるまで調べようがない）

## 実機のレーンから

- [x] （v0.93.7。27.6 / 27.7 に測り方と訳を足した）（#311・#312 の提案）TESTING.md 27.6 / 27.7 の行に測り方を足す: `j<Now>` を N 回 → `<Wait:100>` → `<State:>`、27.6 の前半は `<Now><State:a><Wait:150><State:b>`（素の `<State:>` は先頭の早い画面を隠す）。`scripts/testcheck-ja.toml` の訳も。【QA】
- [x] （v0.86.31。49.10 に「信頼済みのフォルダで」を足した）TESTING.md 49.10 を「信頼済みのフォルダで」と書く（`claude` の信頼ダイアログが出ると行が押せない）。書けないなら人の run に回す。【QA】
- [x] （v0.86.21。待ちを 5→10 秒と 20→40 秒に広げた）（#311 の提案）`ending_a_busy_shell_asks_first`（`src/app.rs`）の待ち時間を見直す。ARM64 の機械で時間切れに近い。
- [x] （v0.86.22。`PreviewSlot::early` を足し、`<State:>` に出した）（#312 の提案）`<State:>` に `preview: early|whole` の行を足す。`App::settled()` が `preview.in_flight` を含むので、普通のキーでは全体が出るまで待ってしまい、v0.86.12 の早い画面が `<Now>` + `<Wait:>` 抜きでは見えない。
- [x] （2026-10-10、#311・#312 の share）再テストの順番表（x64 と ARM64）から `[x]` になった行を外した。残りは 1.44 / 16.13 / 16.13a / 16.3b / 33.7（直し待ち）と 49.7。両方の表が空に近いので、次の run は `WINTEST_NOTHING` に近い。
- 1.43 / 40.16 の書き換えの提案（#312 の 3）は、この節の上にある既存の項目と同じ。足していない。
