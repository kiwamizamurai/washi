# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 概要

Washi（和紙）は Markdown / Typst / LaTeX / Mermaid / PDF を読むためのビューア（Tauri 2 + Rust + TypeScript、フレームワークなしの素の TS）。開くのは常に「読む」画面で、⌘E で分割表示のエディタ（CodeMirror 6、遅延読み込み）を開ける。ファイルの保存を監視し、スクロール位置とズームを保ったまま再描画する。UI の文言（メニュー・画面・エラーメッセージ）は英語。README は英語のみで、短く保つ（比較表は要点だけ）。ランディングは日英の 2 ページ（`docs/index.html` が英語で既定、`docs/ja/index.html` が日本語。GitHub Pages の公開元は `/docs`）で、内容を変えるときは両方を揃える。

## コマンド

```sh
pnpm install
pnpm tauri dev                  # 起動（ファイルを渡す: pnpm tauri dev -- -- --gui examples/showcase.md）
pnpm build                      # tsc + vite build（型チェックを兼ねる）
pnpm test                       # フロントエンドのテスト（vitest run）
pnpm vitest run src/viewer.test.ts            # 単一ファイルのテスト
pnpm vitest run -t "テスト名"                  # 名前で絞る
cargo test --workspace                                               # Rust のテスト
cargo test -p washi-core <name>                                       # Rust の単一テスト
cargo test -p washi-core -- --ignored    # ネットワーク必須のテスト（examples/packages.typ など）
pnpm release                    # 配布用 .app ビルド（scripts/release.sh。ローカルパスを RUSTFLAGS の remap で除去する）
```

Lint / formatter の設定はない。

### ブラウザだけでの画面確認（Tauri 不要）

`e2e/` は Rust の実際の描画結果を保存し、`e2e/mock-tauri.ts` で Tauri API をモックして返すハーネス。

```sh
cargo test -p washi-core dump -- --ignored   # e2e/fixtures を生成
pnpm dev
# http://localhost:1420/e2e/harness.html?file=/x/examples/showcase.md&outline=1&theme=dark
# 見出しまでスクロール: &scroll=4.%20Math（見出しの先頭の文字）、待ち時間: &settle=5000（ミリ秒）
```

## アーキテクチャ

**Rust がレンダリングし、フロントは表示と状態保持だけを担う。** Cargo workspace は 2 crate: `crates/washi-core`（tauri 非依存。`render/`・`cli/`・`editor.rs`）と `src-tauri`（tauri アプリ本体。`washi-core` に依存）。描画と CLI だけなら `cargo test -p washi-core` で tauri をビルドせずに済む。`main.rs` は先に `washi_core::cli::run` を呼び、`None` のときだけ GUI を起動する。

- `crates/washi-core/src/render/` — 形式ごとの `Renderer` トレイト実装（`markdown` / `mermaid` / `typst` / `tex` / `pdf`）。`render/mod.rs` の `RENDERERS` 静的配列が拡張子→Renderer の登録表。`render()` は `Output::Html` か `Output::Pdf` を返し、IPC では先頭 1 バイトのタグ（0=HTML, 1=PDF）＋本体の `into_wire()` 形式で送る。`render_text()`（貼り付け）と `locate()`（PDF 座標→ソース行）はデフォルト実装つきの任意メソッド。貼り付け時の形式自動判定は `render/detect.rs`。
- 形式追加: Renderer を実装 → `RENDERERS` に 1 行足す。既存の `Output` 種別ならフロントの変更は不要。
- 外部コマンド（latexmk / tectonic など）は `TexEngine` のようにトレイトの背後に置き、テストでは差し替える。実行は `render/process.rs` の `run` を通す: タイムアウト（既定 300 秒、`WASHI_COMPILE_TIMEOUT`）とプロセスグループごとの kill に対応し、`Job::start(path)` で同じ文書の新しい描画が古い描画を打ち切る（フロントは古い結果を token で捨てるので安全）。このため、同じファイルを並列で描画するテストは直列にすること（`render/mod.rs` の `serial()`）。Typst はプロセス内コンパイルなのでタイムアウトできない。Typst は `typst` クレートで直接コンパイル、LaTeX は `latexmk` 優先で無ければ `tectonic`。
- 保存の監視は文書のあるフォルダ（非再帰）に加えて、文書が読み込むファイルのフォルダも見る。`Renderer::dependency_dirs`（`render/deps.rs` がソースを静的に走査: LaTeX の `\input` など、Typst の `#include` など、Markdown の画像）が返したフォルダを、`commands::render` が描画のたびに `FileWatcher::set_dependencies` で監視先へ反映する（描画に失敗しても）。再帰監視はしない（HOME など広いフォルダを開いたときに重くなるため）。
- `src-tauri/src/` の `commands.rs`（フロントから呼ぶ Tauri コマンド）、`launch.rs`（起動引数・Finder から開く・1 ファイル 1 ウィンドウの生成）、`watch.rs`（ファイル監視。エディタの一時ファイルや `.DS_Store` は無視、アトミック保存に対応）、`menu.rs`（ネイティブメニュー）。`washi-core` 側の `cli/`（`washi` CLI。起動を待たずすぐ戻る。`install` / `formats` サブコマンド、`-` で標準入力）、`editor.rs`（同じく washi-core。⌘クリックでのソースジャンプ先エディタ解決。`WASHI_EDITOR` で上書き）。
- `src/viewer.ts` — 描画の流れの中心。`Host`（Tauri 呼び出しの抽象）・`View[]` を注入して使うので、テストやモック環境でも差し替えられる。再描画のたびに `token` をインクリメントして古い描画結果を破棄し、`Scroll` でスクロール位置を保つ。
- `src/views/` — `View` インターフェースの実装（Markdown の HTML、PDF は pdf.js）。`outline.ts` が目次、`jump.ts` がソースジャンプ、`api.ts` が Tauri `invoke` ラッパー。
- Markdown は Rust 側（comrak）で HTML 化し、KaTeX・Mermaid・highlight.js はフロント側で後処理する。相対画像は埋め込み、`.md` / `.typ` / `.tex` への相対リンクは Washi 内で開く。

## 編集機能（⌘E の分割表示）

読むモードと CLI は編集機能の有無に影響されない（描画の `render(path)` は変えず、編集用は加算の別経路）。

- Rust: `render::render_buffer(path, text)` が保存前の本文を描画し、`Rendered{output, diagnostics}` を返す（失敗しても診断は返る）。Typst は本物のパスで `Session` を保存し、LaTeX は同じフォルダの隠しファイル `.washi-buf-<stem>.tex`（`Mirror`。`Drop` で削除、起動時に 1 時間以上前のものを掃除、読み取り専用なら一時フォルダ）でコンパイルする。補完は `typst_ide::autocomplete`、前方検索は `jump_from_cursor` と `SyncTex::forward`。UTF-16（CodeMirror）⇄ UTF-8 バイト（Typst）⇄ コードポイント列（診断）の変換は `render/offsets.rs` の 1 か所。
- 応答の形式は `[tag u8: 0 html / 1 pdf / 2 失敗][u32 BE 診断 JSON の長さ][診断 JSON][本体]`（`Rendered::into_wire` ⇄ `api.ts` の `decodeBuffer`）。既存の `render` の形式は変えない。
- 保存: `files.rs` の `write_file` が、同じフォルダの一時ファイル＋`rename`（シンボリックリンクは先に解決、権限を引き継ぐ）で書き、ディスクのハッシュと `base_hash` が違えば `Conflict`。Tauri 側の `write_file` は `Documents::owns(label, path)`（そのウィンドウが開いているファイル）にしか書かない。
- フロント `src/editor/`: DOM に依存しない純粋な部分（`buffer.ts` 本文と未保存の判定・`scheduler.ts` throttle/debounce・`session.ts` 保存・衝突・自動保存・`kinds.ts`・`lint.ts` 位置の対応・`split.ts`・`sync.ts`）は単体テストがある。`editor.ts`（CodeMirror の組み立て）・`languages.ts`・`complete.ts` は `controller.ts` が初回の ⌘E で動的に読み込み、`vite.config.ts` の `manualChunks` が `vendor-cm` に分ける（起動時のチャンクに CodeMirror を入れない）。
- Typst の言語対応は `codemirror-lang-typst` 0.6.0（実験的。版を固定）の `/lezer` の部品を `languages.ts` で組み立てる。内蔵の linter は外す（波線はコンパイラの診断に一本化）。読み込めなければ簡易のトークナイザーに落ちる。
- 描画の頻度は `kinds.ts` の `renderPolicy`（Markdown・Mermaid 150 ms throttle、Typst 400 ms throttle、LaTeX 1.2 秒 debounce と保存時）。`Viewer` の編集用の入口は `renderBuffer` / `leaveBuffer` / `setBufferHooks`。
- 閉じる／終了の確認: ウィンドウは `onCloseRequested`、⌘Q は Rust の `RunEvent::ExitRequested`（`DirtyWindows` に未保存のウィンドウが居れば止めて `washi://quit-requested` を送る）。別のファイルを開くときは `EditingController.release()` が先に確認する。
- ブラウザのハーネス（`e2e/`）は、`mock-tauri.ts` に編集用のモック（仮想ファイル `__files`、`__calls` の記録、簡易な描画）がある。`?realpreview=1` でプレビューに本物の fixture を使える（スクリーンショット用）。キー入力・メニュー・閉じる確認は、実機のウィンドウでの確認が別途必要。

## CLI の契約（変えるときの規則）

`washi --json` の出力、終了コード、stdout / stderr の使い分けは、AI エージェントが依存する公開した契約。型は `crates/washi-core/src/cli/report.rs` にあり、README の「For AI agents」と対応する。

- 成功は stdout に JSON 1 行、失敗は stderr に JSON 1 行（stdout は空）。`--json` が無ければ文。終了コードは 0 / 1（起動失敗）/ 2（それ以外）で、決めるのは `report::exit_code` の 1 か所。
- 1.x ではキーの追加だけ可。削除・改名・型や意味の変更、`kind` や形式名（`format`）の変更は、`FORMAT_VERSION` を上げて CHANGELOG に書く。
- 契約テスト（`cli/tests.rs` の `contract_*`）がキー集合・型・ストリームを固定している。落ちたら、テストを直す前に「契約を変えてよいか」を判断する。`readme_documents_the_contract` が、`ErrorKind` や形式名の README への書き忘れを検出する。
- 引数なしの `washi` はフォアグラウンドでアプリを起動する（Finder が同じバイナリを引数なしで起動するため。切り離して戻る挙動に変えない）。

## 規約

- コードにコメントを書かない（Rust の `///` も、docstring も含む）。意図は名前とテストで表す。
- UI の文言とエラーメッセージ、コミットメッセージは英語。

## 画面の補助機能の構成

- コマンドパレット（⌘K）: `src/palette/`。`commands.ts` のコマンド ID は `menu.rs` / `actions.ts` の ID と同じ（テストが突き合わせる）。`match.ts` があいまい検索、`items.ts` が並べ替え、`palette.ts` が `<dialog>`。
- 最近のファイル: `src/recent.ts`（localStorage、最大 8 件）。起動画面とパレットが使う。
- 検索の件数: `src/search-count.ts`（数え方）と `src/search-source.ts`（見えている文書の文字列と選択位置）。`window.find` はページ全体を探してフォーカスを動かすので使わず、数えた一致を選択して移動する。
- 読み進んだ割合の線: `src/progress.ts`。
- ランディング（`docs/`）は、HTML と CSS に、機能紹介のアニメーション用の GSAP（cdnjs）だけ。

## LaTeX の注意

- latexmk の引数は `-キー=値` の等号形だけ（`-outdir DIR` と空白で区切ると `unknown option` で何も実行されない）。公式の latexmk で確かめる: `WASHI_TEST_LATEXMK_PL=<latexmk.pl の場所> cargo test -p washi-core real_ -- --ignored --test-threads=1`（偽のエンジンで、Washi の実コードを通す）。
- latexmk には `-pdf` を渡さない（`.latexmkrc` のエンジン指定を上書きするため）。`-e '$pdf_mode = 1 if !$pdf_mode;'` で、rc が決めていないときだけ PDF にする。
- 文書のフォルダの `latexmkrc` / `.latexmkrc` は任意のコードを実行できるので、内容のハッシュで「信頼済み」を記録するまで使わない（`render/trust.rs`）。未信頼のときは `-norc` に、自分のホームの rc だけ `-r` で読み直す。確認は `.tex` を開いたときの `<dialog id="trust">`。
- `% !TEX root = main.tex` は `tex.rs` の `magic_root` が読み、ルートをビルドする。章に未保存の変更があるあいだは、直前のビルドを見せる。
- 失敗メッセージの 1 行目は、既知の原因の案内（`failure_hint`）、なければ本当のエラー（`first_error`: tectonic の `error: file:line: msg`、`-file-line-error`、`! msg`）。案内の対象は `failure_hint`（shell-escape、pLaTeX、biber と biblatex の版の食い違い、biber が無い、`\documentclass` なし）。
- 一時の出力フォルダ（`washi-<pid>`）は、起動時に持ち主のいない古いものを、終了時に自分のものを消す（`cleanup_temp` / `cleanup_own_temp`）。
- `latexmk` を選ぶのは、エンジンもあるときだけ（`latexmk_usable`）。
- 子プロセスには `tools::path_with_fallbacks()` を `PATH` として渡す（Finder / Dock から起動したアプリの `PATH` は最小で、`/opt/homebrew/bin` の biber や `/Library/TeX/texbin` の pdflatex が見えないため）。
- biblatex は、tectonic（同梱の biblatex 3.17 は biber 2.17 を要求するが、Homebrew の biber は 2.22）では動かない。TeX Live の latexmk なら動く。`backend=bibtex` は tectonic で動く。

## CI とリリース

- `.github/workflows/ci.yml`: `washi-core` のテスト（Linux、Tauri なし）とアプリ全体のテスト・ビルド（macOS）。毎回は走らせない方針なので、手動実行（`workflow_dispatch`）のみ。
- `.github/workflows/release.yml`: GitHub Release を公開（publish）すると起動する（`gh release create v0.1.0 --generate-notes` など）。そのタグを checkout し、タグが `v<major>.<minor>.<patch>` の形で、`package.json` / `tauri.conf.json` / `src-tauri/Cargo.toml` のバージョンと一致しないと失敗する。`pnpm release` で `.app` を作り、`Washi-<version>-aarch64.zip` と `.sha256` を、公開済みの Release に `gh release upload` で添付し、`kiwamizamurai/homebrew-tap` の `Casks/washi.rb` を生成して push する（secret `HOMEBREW_TAP_TOKEN` が必要。未設定なら、この tap の更新だけ警告を出して飛ばす）。Actions から `tag` を指定して手動で再実行もできる。
- 配布は Apple Silicon の macOS のみ。ad-hoc 署名なので Cask の `postflight` で quarantine を外している。LaTeX エンジンは同梱せず、Cask の `depends_on formula: "tectonic"` で入れる。
- v1.0.0 のリリースで `release.yml` は実際に動いた。`HOMEBREW_TAP_TOKEN` が未設定の間は、tap の更新が警告つきで飛ばされるので、リリースのあとに `kiwamizamurai/homebrew-tap` の `Casks/washi.rb` の `version` と `sha256` を手で更新する。アクションはコミットの SHA で固定している（コメントを書かない方針なので、版は SHA から引く）。

## 注意点

- アイコンは `python3 scripts/make_logo.py --all` で `app-icon.svg`・`docs/assets/logo.svg`（README 用）・`docs/favicon.svg` を作り、`pnpm tauri icon app-icon.svg --output <一時フォルダ>` で作った PNG/ICNS/ICO のうち、`src-tauri/icons/` に既にある名前のものだけを入れ替える（iOS・Android 用は使わない）。
- Finder への登録拡張子は `src-tauri/tauri.conf.json` の `bundle.fileAssociations`。形式を足したら、ここにも足す（`src-tauri/src/lib.rs` のテストが、`supported_extensions()` との不一致を検出する）。
- Tauri の権限は `src-tauri/capabilities/` と `tauri.conf.json` で管理。新しいプラグイン API を使うときは capability の追加が必要。
- `examples/` は各形式の実文書に近いサンプル（英語。ランディングのスクリーンショット `docs/assets/shot-*.jpg` の元で、ハーネスを 1221×641 で撮ったもの。PDF の描画を待てるよう、ヘッドレス Chrome を `puppeteer-core` で操作して撮った）で、`dump` テストの入力にもなる。
- CLI は macOS / Linux 向け（Windows 未対応）。署名・公証は未対応。
