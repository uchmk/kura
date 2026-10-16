# 引き継ぎ（2026-10-16、持ち主の Windows の PC のセッション）

次のセッションへのメモ。この日の PC のセッション 3 つの分。2 回目（夕方）は 1 回目のメモの「残っていること」を進め、
3 回目（夜）は持ち主の依頼でミニマップのホバーを広げ、HTML のプレビューに取りかかった。

**やりかけの作業が 1 つある: HTML のプレビュー**（下の「3 回目の 4」）。途中の変更はブランチ `wip/html-preview`（0ee9f13、origin にも push 済み）にある。
ほかはすべて main に入っている。作業用の worktree と R: のビルドは片付けた。

- ito: 0.46.1（ebe1cad）
- kura: v0.116.3（このメモの版）。ito は 0.46.1
- yagura: origin/main は v0.66.2 で、ito は 0.43.0（2ec61d0）のまま。yagura のセッション（yagura-65）が ito 0.45・0.46 の分を進めていた
- tsumugi: v0.114.11（ce5c37b）。#181 は v0.114.7 で直って閉じた
- minato: v0.50.0。この日の PC のセッションは v0.46.1 から先を触っていない

`C:\dev\kura` はこのメモを push したあと `git pull --ff-only` で origin に合わせた。ほかの手元の main のクローン（`ito`・`yagura`・`minato`・`tsumugi`）は origin より古いまま。
どのセッションも `origin/main` から C: に worktree を作って作業した（ビルドは `R:\cargo-target\<名前>`）。

## 3 回目のセッション（夜）でやったこと

### 1. ミニマップのホバーを 7 行に（kura v0.116.0、a3883fe、CI 緑）

- 依頼: 「ミニマップのマウスオーバーで 5 行くらい出すことはできる？1 行だと何をやっているかわからない。性能的にどうかな？」
- `HOVER_CONTEXT = 3`: 指した行の前後 3 行ずつ、7 行を出す。
  - 指した行はポインタの高さに置き、カーソルの色で印を付ける。行に共通の字下げは落とす。
  - ファイルの頭と終わり、ペインの上と下では、はみ出さないようにカードの中の窓をずらす。
- 性能（1 フレームの描画）: 基準は約 0.39 ms。1 行で +0.02 ms、7 行で +0.19 ms（16.7 ms の約 1 %）、15 行で +0.28 ms。ポインタが止まっていればほぼ 0。
  15 行にしても問題ない。行数を設定にすることもできる（していない）。
- 実機: `C:\dev\filer-evidence\handoff3-20261016\scripts\hover.ps1`（説明は同じフォルダの `README.md`）で、4000 行ほどのファイルを撮った。
  中ほどで 1922–1928 行、いちばん下で 3979–3985 行が出て、印はポインタの横にあった。
  - 見つけたこと: カードはペインの幅の 60 % までなので、1 行が 30 字ほどで切れる。0.85–0.9 に広げる案を持ち主に出す（まだ答えをもらっていない）。
- §42 はすべて自動のテストなので、再テストの issue は無い。

### 2. ito の rev 上げ（yagura のセッション「yagura-65」の依頼）

| 版 | ito | 中身 | TESTING | 再テスト |
| --- | --- | --- | --- | --- |
| v0.116.1（4c156a1） | 0.45.0（e9e686c） | Windows でペインを閉じても、シェルの木の外のプロセスを終わらせない（持ち主の PC では Explorer が終わっていた。tsumugi #175） | 1.51 を足した | #415 |
| v0.116.2（80e7a72） | 0.46.1（ebe1cad） | Windows でペインの幅が変わっても、プロンプトの印（OSC 133）が残る（tsumugi #16） | 49.12 を足した | #416 |

- どちらも CI 緑。`Cargo.toml` の 16 か所の `rev` を全部上げた（`scripts/lanes.sh check` も verify の中で通った）。
- 終わったことは yagura-65 にメッセージで知らせた。

### 3. 2 回目の「残っていること」

- tsumugi #181 は、tsumugi のセッションが v0.114.7（1630961）で直して閉じた（`TSUMUGI_WINDOW_AT` が `x,y,scale` になった）。
  3.34 を倍率の違う 2 枚の画面で押すのは残る。
- ほかは進めていない（下の「残っていること」）。

### 4. HTML のプレビュー（やりかけ。v0.117.0 にする予定）

**決まったこと**

- 持ち主: 「表示の場所に html は表示できるようになる？ソースと HTML が切り替えできるのがよいと思うんだけど。」
- 読み物としての表示を提案し、承認された（「読み物としての表示（上の案）でよければ。この案でお願いします。」）。
  - HTML を pulldown-cmark のイベントに読み替え、Markdown のプレビューの組版（`markdown::lay_out`）でそのまま出す。見出しのアウトライン・表・コード・リストが Markdown と同じに見える。
  - `M`（整えた表示とソースの切り替え）で HTML のソースと切り替える。出し始めは設定 `render_markdown`（既定 true）に従う。
    HTML も既定で整えて出るようになるので、CHANGELOG には **変更** としても書く。
  - WebView（wry・WebView2）は使わない。依存が大きく、6 つのターゲットへのクロスコンパイルが重くなり、egui のペインの中に描けず、ページのスクリプトを動かすことになるため。
  - CSS の配置・スクリプト・フォームは扱わない。画像は alt の字で出す。
- 持ち主の質問「html.rs は ito に作っておいた方がよい？」への答え: **今は kura に置く。**
  - ito の `ito-browser` は tsumugi の本物の WebView（wry）で、目的が違う。
  - C:\dev のアプリで pulldown-cmark を使うのは kura だけ。html.rs は kura の `markdown::lay_out` に渡すものなので、ito に置いても使う側が無い。
  - ito に置くと、直すたびに ito の push と 16 か所の rev 上げが要る。
  - ただし、あとで markdown.rs と一緒に `ito-markdown` のようなクレートへ移せるように、html.rs は「HTML → イベント」だけにし、言語の判定は関数で受け取る。

**今ある分**（ブランチ `wip/html-preview`、0ee9f13。80e7a72 の上の 1 コミットで、`cargo check` は通る。テストはまだ回していない）

- `markdown::render` は `Parser::new_ext(text, options()).into_offset_iter()` を集め、
  `pub(super) fn lay_out(text, events: &[(Event<'_>, Range<usize>)], cols, theme, syntaxes) -> (Doc, bool)` を呼ぶ。
- `outline()` はイベントの列を読む。見出しの中の SoftBreak・HardBreak は空白にする。
- コードの塊の最初の Text が、コードの始まりの行を決める（`<pre>` のため）。
- `decode_entities` は `pub(super)`。名前付きの文字参照を増やした（`&ensp;`・`&emsp;`・`&thinsp;` は空白）。

**続け方**

```bash
git -C /c/dev/kura fetch origin
git -C /c/dev/kura worktree add /c/dev/kura-html wip/html-preview
pwsh -NoProfile -File "$LOCALAPPDATA/uchmk-build/cargo-ramdisk.ps1"   # C:\dev\kura-html の中で回す（ビルドを R:\cargo-target\kura-html に出す）
git -C /c/dev/kura-html rebase origin/main   # main が進んでいれば
```

push するときは WIP のコミットを `v0.117.0: …` の 1 コミットにまとめ、`scripts/push-main.sh` で main に入れる。入ったらブランチ `wip/html-preview` を消す（手元と origin）。

**残りの手順**

1. `src/preview/html.rs` を書く（設計は下）。`src/preview/mod.rs` の `mod` の並び（14–23 行）に `mod html;` を足す。
2. `src/preview/text.rs` の `render_cut`（122 行から）:
   - `is_html = req.mime == "text/html"`、`renders = is_markdown || is_html`。
   - `head` は `renders && req.markdown_rendered` のとき 0。
   - 前倒しの payload（192–198 行）と最後（244–249 行）の両方で、`html::render` か `markdown::render` を呼ぶ。
   - フェンスの塗り直しと見出しの太字（214–241 行）は Markdown だけのまま。`symbols.feed`（207 行）は `!renders` のときだけ。
   - テストの `request(ext)`（434–450 行）に `"html" => "text/html"` を足す。
3. `src/app.rs` 2859 行あたりの `cols: if matches!(mime, "text/markdown" | "text/csv")` に `"text/html"` を足し、コメントも直す。
   `mime.rs` は html・htm・xhtml を `text/html` にする（確かめてある）。
   `Payload::Markdown` を見る所（`body_matches`・`outline_entries`・`outline_source_line`・`toggle_render`・`markdown_line_to_src` / `src_to_line`、
   `ui/preview.rs` 218 行、`M` のテスト `ui/preview.rs` 1973–1990 行・`ui/mod.rs` 4510 行）は中身の種類を見ないはずなので、確かめるだけ。
4. 言葉に HTML も入れる:
   - `ui/settings.rs` 161–162 行（英 "Rendered Markdown" / "How a Markdown preview starts; M switches the one on screen. Read at start"）。
   - `ui/settings.rs` 245–246 行（日「Markdown を整えて出す」/「Markdown のプレビューの出し始め。画面のものは M で切り替える。起動時に読む」）。
   - `src/config/defaults/keymap.toml` の `M` の desc（205 行あたり）、`ui/keydesc_ja.rs` 36 行、README（345 行の設定・446 行の「Markdown preview」の節・1222 行のキー表）。
5. テスト: html.rs の単体（markdown.rs のテストと同じ `doc()`・`texts()` の形）、text.rs で `.html` が `Payload::Markdown` になること、`M` で切り替わること。
6. 版 v0.117.0（MINOR。`### 追加` と `### 変更`）、TESTING の行と `scripts/testcheck-ja.toml`、
   2 つのチェック表を作り直す（`cargo run --example make-testcheck` と `cargo run --example make-testcheck -- --lane linux`）、
   fmt・clippy・`scripts/verify.sh`・`scripts/push-main.sh`、手で押す行の再テストの issue。
7. 実機: 設定を分けて本物の HTML（rustdoc の出力など）を開き、両方の表示と `M` を `--keys` で確かめる（`hover.ps1` か `keys388.ps1` の形が使える）。

**html.rs の設計（固めたもの）**

- API
  - `pub fn render(text, cols, theme, syntaxes) -> (Doc, bool)` は
    `lay_out(text, &events(text, &|l| find_token(syntaxes, l).is_some()), cols, theme, syntaxes)`。
  - `pub(super) fn events(text: &str, knows: &dyn Fn(&str) -> bool) -> Vec<(Event<'static>, Range<usize>)>`。範囲は `at..at` で、ソースの順に増えていく。
  - 使うもの: pulldown_cmark の `Alignment`・`CodeBlockKind`・`CowStr`・`Event`・`HeadingLevel`・`LinkType`・`Tag`・`TagEnd`、
    `super::markdown::{decode_entities, lay_out}`、`super::text::find_token`（`text.rs` 255 行）。
- 字句
  - 字（Text）、開き（名前は小文字、属性は文字参照を解いた `(String, Option<String>)`、`/>` の印）、閉じ（Close）。
  - `<!--…-->` は飛ばす（閉じなければ最後まで）。`<!…>`・`<?…>` は `>` まで飛ばす。
  - タグは `<` か `</` のあとに ASCII の字が来るときだけ。バイトで読む（止まる所は ASCII なので、そこで切っても安全）。
  - 属性の名前は空白・`=`・`>`・`/` まで。名前が空なら 1 字飛ばして必ず進む。値は引用ありとなしの両方。
  - 引用の探しとタグ全体は 16 KiB まで。超えたり崩れたりしたら、`<` を字として次の `<` まで進む（O(n²) を避ける）。
  - 中身を読まないもの（RAW）: script・style・textarea・title・xmp・iframe・noembed・noframes。
    大文字小文字を無視した `</名前`（英数字が続かないもの）と、その `>` まで捨てる。`/>` で閉じたものはそれだけを捨てる。
  - 空のタグ（VOID）: br hr img input wbr area base col embed link meta param source track keygen。
- 飛ばすもの
  - SKIP（同じ名前の入れ子を数える）: template・svg・math・object・canvas・video・audio・select・datalist・noscript。
  - `<head>` は Head の状態にする。`</head>` か、HEAD_OK（title meta link style script base noscript template head）に無い開き（body も）で終わり、その開きはふつうに読む。
  - `hidden` と `style` の `display:none` は、div span section article nav aside header footer ul ol menu table form figure button dialog a のときだけ飛ばす。
    `open` の無い `<dialog>` も飛ばす。
- 入れ子の枠（深さは 512 まで）
  - 役: Inline・Block・Para・Heading・Quote・List・Item・Table・FlatTable・Section{head}・Row・Cell・Caption。
  - 文脈: Cell・Heading・Caption → Tight、Item・List・Para → Flow、Quote → Loose、Table・Section・Row → Drop、ほか（Inline・Block・FlatTable）は親のまま。一番上は Loose。
  - 枠は閉じるときに出すイベント（`End(…)`）を持つ。閉じは「表の行・表の後始末 → 閉じのイベント → Block・FlatTable なら境目」の順。
- 字の流れ
  - Loose で段落の外に字が来たら、暗黙の段落を開く。Drop の中の字は捨てる。
  - 字と字の間: `<br>` の数（2 まで、字が出たあとだけ数える）か箱の境目があれば、その数だけ HardBreak。
    なければ、改行を含む空白は SoftBreak（日本語どうしは空白なしでつながる）、同じ行の空白は `' '`。
  - 塊の Start・End（Paragraph・Heading・BlockQuote・List・Item・CodeBlock・Table・TableHead・TableRow・TableCell）は、
    続いた字を出し、暗黙の段落を閉じてから出す。
    Builder は Start(Paragraph)・Start(Heading) で flush するので、セルや見出しの中に塊のイベントを出してはいけない。
  - インラインの開き（Strong など）は段落を開かない。前に空白があれば、先にそれを出してから開く。
- タグの対応
  - b・strong → Strong、i・em・cite・dfn・var → Emphasis、s・del・strike → Strikethrough、`href` のある a → `Link { Inline, href, "", "" }`。
  - h1–h6 → Heading（Tight の中では境目と太字の Block）。開いた見出しがあれば先に閉じる。
  - p・figcaption・legend・summary → Paragraph（summary は太字。Tight の中では Block）。
  - 箱（div section article main header footer nav aside address figure form fieldset details center hgroup dl body html search dialog）は段落を閉じ、境目だけを置く。
    class の `language-`・`lang-`・`highlight-source-`・`highlight-` は、中の pre の言語の手がかりとして覚える。
  - dt・dd: 前の dt・dd を閉じ（壁は dl・table・td・th・caption）、境目。dt は太字。
  - blockquote → BlockQuote（Tight では Block）。ul・menu・dir → `List(None)`、ol → `List(Some(start か 1))`。
  - li: 前の li を閉じ（壁は ul・ol・menu・dir・table・td・th・caption）、Item。Tight の中では境目と「• 」。
  - hr → Rule。br → 改行の数。
  - img → Image と alt の字。alt が空、alt が無くて src が無いか `data:`、幅か高さが 0・1・0px・1px、`display:none`、`aria-hidden="true"` は出さない。
  - `input type=checkbox` → TaskListMarker（`checked`）。
  - code・kbd・samp・tt → Code の状態: 中の字は文字参照を解き、同じ名前を数え、br は空白。塊のタグ（BLOCKS）の開きか閉じで終わり、そのタグはふつうに読む。
    空白を詰め、空でなければ Event::Code。
  - pre・listing → Pre の状態（下）。知らないタグは中身だけを読む（透明な Inline）。
- pre
  - 開くときに段落を閉じ、近くの Block・Inline の枠から文脈と言語の手がかりを取る。同じ名前を数え、br は改行、中の `<code>` の class・`data-lang` も集める。
  - 最初の字の頭の改行を 1 つ落とし（位置もずらす）、`\r` を消し、末尾の空白を落とす。空なら出さない。
  - Loose・Flow: `Start(CodeBlock(Fenced(lang)))`（pre の位置）、`Text(code + "\n")`（字の位置）、`End(CodeBlock)`。Tight: 空でない行ごとに Code と境目。Drop: 出さない。
  - 言語の順: 接頭語の付いた class の語（pre、次に code）、`data-lang`・`data-language`、外の箱の手がかり、接頭語の無い class の語。`knows()` が真の最初のもの。
    `lang` 属性は自然言語（`cs` など）なので使わない。
- 表
  - tr・td・th・thead・tbody・tfoot の前に、開いた caption を閉じる（壁は table）。
  - `<table>`: Tight では FlatTable と境目、Drop では透明な Inline。ほかは段落を閉じて Table の枠。`Start(Table(vec![]))` は最初の行で出し、その位置を覚える。
  - thead・tbody・tfoot は、外が Table のときだけ Section（互いの閉じ忘れを補う）。
  - tr: 閉じ忘れを補う（壁は table・thead・tbody・tfoot）。外の枠が Table・Section なら行を開く。FlatTable なら境目と Inline。
  - td・th: 閉じ忘れを補う（壁は tr・table・thead・tbody・tfoot）。Row ならセル、Table・Section なら行とセル、FlatTable なら空白と Inline。
    セルは `Start(TableCell)`。th は Strong も。揃えは `align` か `style` の `text-align`（`!important` は落とす）。
  - 行の終わり: セルが 0 なら行ごと消す。外が `Section{head}` か（最初の行で全部 th）なら、行の Start を TableHead に替えて `End(TableHead)`。ほかは `End(TableRow)`。
    揃えは揃えのある最初の行から取る。
  - 表の終わり: 行が 0 なら表ごと消す。あれば `Start(Table(aligns))` を書き戻して `End(Table)`。
  - caption: 表が始まる前なら段落（`End(Paragraph)` で閉じる）。FlatTable の中なら Block。ほかは捨てる。colspan は見ない。
- 閉じタグ
  - `</br>` は改行。`</table>` は近い table まで。thead・tbody・tfoot・tr・td・th・caption は同じ名前まで（壁は table）。
  - `</hN>` はどの見出しでも閉じる（壁は td・th・caption・table）。ほかは同じ名前まで（壁は td・th・caption・table）。合わない閉じは捨てる。
- 終わり: Pre・Code の状態を終え、枠を全部閉じ、続いた字を出し、段落を閉じる。
- 受け入れた不正確さ: リンクの前の SoftBreak に下線が付く。複数行のコメントと pre の中の `<br>` で src の行がずれる。表のセルの外の字は捨てる。
  dd は字下げしない。ruby は本文と読みを並べる。sup・sub はふつうの字。深すぎる枠は閉じがずれる。表の行の中の `<form>` は行を早く閉じることがある。
- テストに入れるもの: 見出しとアウトライン、p と div（空行が入るか）、リスト・入れ子・ol の start、表（thead・最初の行が th・揃え・セルの中の表）、
  pre の言語と src の行、インラインのコード、文字参照、script・style・head・svg を飛ばす・`</head>` が無いとき、日本語のつながり、チェックボックス、画像、dt・dd、
  cols 0..=120 で落ちない、`events()` の範囲が減らない、壊れた入力（閉じない引用・タグ・コメント）。
- Markdown の Builder の出し方（テストの期待値の目安）: 表は `["name │  n", "─────┼───", "a    │ 10", "bb   │  2"]`（見出しは太字）、
  リストは `["• one", "[ ] todo", "[x] done", "    ◦ nested", "", "1. first", "2. second"]`、引用の行は「│ 」で始まる。
  コードの行は `LineKind::Code` で、行ごとに src を持ち、頭に空白 1 つ。

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
  - 2 回目では直さず、tsumugi のセッションに任せた（3 回目の時点で v0.114.7 で直って閉じている）。
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

- **HTML のプレビュー**（3 回目の 4）。ブランチ `wip/html-preview` から続ける。
- **ミニマップのカードの幅**（3 回目の 1）: 60 % のままか、0.85–0.9 に広げるか、持ち主に聞く。
- **tsumugi #181 の確認:** v0.114.7 で直った。3.34 を倍率の違う 2 枚の画面で押す。
  マウスを使わない測り直しは `C:\dev\filer-evidence\handoff2-20261016\scripts\windowat.ps1`（`WINDOW_AT` を `x,y,scale` にする）。
- **tsumugi #178**（18.14・18.16）: 画面をまたいでドラッグする確認。Windows のレーンが手で押す。
- **54.18 の字の読みやすさ:** `F` で見つけた字が Catppuccin Latte で 3.71:1、tsumugi Light で 4.24:1 と、4.5:1 を下回る。持ち主の判断待ち。
  `question` の issue（docs/claude/questions.md の書き方で、選択肢を並べて推奨を 1 つ）を立てる予定だったが、まだ立てていない。
- kura #316: 54.19 だけ残る。Windows 全体のライト/ダークを切り替えるので、持ち主に許可をもらってから押す。
- 再テスト: #413（54.10a）・#415（1.7・1.35・1.51）・#416（49.12 ほか）は x64 のレーンが押す。#390 は ARM64（build 28000）でも押すまで開けておく。
- kura #393（verify の中でテストが 1 回 60 秒以上止まった。再現しない）: `an_opened_copy_keeps_its_folders` を繰り返し回して止まるか見る案がある。
- kura #345（横断のレビューの提案）は前からあるもの。どのセッションも触っていない。

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
- （3 回目）Bash で worktree の外へ `cd` すると、セッションの作業フォルダごと移る。コマンドは `cd /c/dev/<worktree> &&` で始める。
- （3 回目）rtk のフックが `grep` を書き換え、`\|` などが rg の正規表現のエラーになる。Grep のツールか `python -I` で探す。
  Python で日本語を print すると cp932 で落ちるので、ascii にするかファイルに書く。Bash の heredoc に Rust のコードを入れると崩れるので、Python の台本をファイルに書いて回す。
- （3 回目）Git Bash の `TZ=Asia/Tokyo date` は効かず、UTC で出た（21:47 JST に 12:47）。CHANGELOG の日付は PowerShell の `Get-Date` で確かめる（0–9 時の JST では日付がずれる）。

## 実機で試すときの約束（持ち主が PC を使っている）

- **実マウスを動かさない。**窓は `SetWindowPos`・`ShowWindow`・`WM_CLOSE` で動かし、キーは `--keys` か `SendInput`。最初の回でドラッグが持ち主のマウスと重なったことがある。
  ポインタの位置が要るときは、窓に `WM_MOUSEMOVE` を `PostMessageW` で送る（`hover.ps1`）。
- デスクトップのロック `Local\wintest-desktop` を取ってから操作する。ほかのレーンとぶつからないようにするため。
- 設定は分ける: `KURA_CONFIG_HOME`・`YAZI_CONFIG_HOME`・`KURA_STATE_HOME`・`KURA_KEYS_DONE`・`YAGURA_STATE_HOME`・`YAGURA_CONFIG_HOME`・`UCHMK_CONFIG_DIR` など。
  持ち主の `%LOCALAPPDATA%\uchmk` と `%APPDATA%\uchmk` には書かない。
- 持ち主の tsumugi とサーバーは止めない（この日は PID 35820 と 16740。再起動で変わる）。
- ほかのセッションの worktree には触らない: `kura-verify`、`filer-wintest`、`tsumugi-verify`、`tsumugi-wintest`、`yagura-e089`、`yagura-wintest`。
- 使い回せる台本:
  - `C:\dev\filer-evidence\yagura-retest59-20261016\scripts\retest-5-22.ps1`（`WinPlace.cs`・`RealInput.cs`。窓の場所を読み、窓を動かして閉じる）。
  - `C:\dev\filer-evidence\handoff2-20261016\scripts\`: tsumugi を `WINDOW_AT` で起動して場所を測る `windowat.ps1`、最小化して閉じる `min170.ps1`、
    kura の `--keys` を分けた設定の日本語の画面で回す `keys388.ps1`。
  - `C:\dev\filer-evidence\handoff3-20261016\scripts\hover.ps1`: kura のミニマップのカードを実マウスなしで撮る（説明は同じフォルダの `README.md`）。
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
| `handoff3-20261016` | 3 回目: ミニマップのカードを撮る `hover.ps1` と `README.md`（画像は R:\Temp\kura-hover にあったが片付けた） |
