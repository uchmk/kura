# ARM64 のリリースの行

ARM64 の Windows 機で、**リリースのたびに 1 回だけ**確かめる行（v0.101.3 から）。毎回の `cargo test` と
起動の確認は GitHub の ARM64 ランナーがする（`.github/workflows/arm64.yml`）。ここに残るのは、
ARM64 の機械が無いと答えられないもの: x64 版をエミュレーションで動かしたときの見え方と、
ARM64 の zip そのもの。ほかの行はすべて x64 の機械が TESTING-CHECKS.md で押す。

**正は TESTING.md**（行の中身はそこにある。ここは ID と短い訳だけ）。`cargo run --example make-testcheck -- --check`
が、ここの ID が TESTING.md にあることを確かめる。

- ここにはチェックを付けない。リリースのワークフロー（release.yml の `arm64-rows`）が、この一覧を
  チェックリストにした `release-arm64` の issue を版ごとに立てる。前の版の issue は閉じる。
- ARM64 のレーン（`auto-wintest.ps1 -Lane arm`）は、その issue が開いている間だけ動き、issue の中で押して閉じる。
  手で押して閉じてもよい。
- x64 と違う結果は `bug` の issue にし、リリースの issue からリンクする。

- **25.5** ARM64 機で x64 ビルドの `kura env` → `OS arch` と `Process arch` が食い違う
- **25.21** `kura env` → `Executable` の行。x64 ビルドの `Process arch` は `x86_64 (emulated on aarch64)`、ARM64 ビルドは `aarch64` だけ
- **26.5** ARM64 ビルドの不具合報告 → OS arch と Process arch がどちらも `aarch64`
- **26.6** x64 ビルド（エミュレーション）の不具合報告 → OS arch `aarch64`、Process arch `x86_64`
- **41.8** 両方の Windows の zip の `kura.exe` で spot の `<Tab>` → `Architecture` が zip の名前と一致する
- **48.1** 両方の zip を展開 → フォルダが 1 つ、中はちょうど 5 ファイル（`scripts\verify-release.ps1 -Tag <タグ>`）
- **48.2** 各フォルダで `.\kura.exe --version` → `(x86_64)` と `(aarch64)`（同上。ARM64 機でしか両方は動かない）
- **48.3** 各 zip の 4 つのバイナリの PE machine → x64 は `8664`、ARM64 は `AA64`（同上）
- **48.6** 同じ PE machine の別の `conpty.dll` を `PATH` に置いて起動 → 同梱のものを読む（`scripts\run48.ps1 -Folder <フォルダ> -Decoy <dll>`）
