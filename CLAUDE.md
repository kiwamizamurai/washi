# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

Washi is a viewer for Markdown / Typst / LaTeX / Mermaid (Tauri 2 + Rust + TypeScript, no framework, plain TS). It always opens in a "reading" view; ⌘E opens a split-view editor (CodeMirror 6, lazy-loaded). It watches the file for saves and re-renders while keeping the scroll position and zoom. UI text (menus, screens, error messages) is in English. The README is English only and kept short (the comparison table lists only the essentials). The landing page has two pages, English and Japanese (`docs/index.html` is the English default, `docs/ja/index.html` is Japanese; GitHub Pages serves `/docs`). When you change the content, keep both in sync.

## Commands

```sh
pnpm install
pnpm tauri dev                  # run the app (pass a file: pnpm tauri dev -- -- --gui examples/showcase.md)
pnpm build                      # tsc + vite build (also the type check)
pnpm test                       # frontend tests (vitest run)
pnpm vitest run src/viewer.test.ts            # a single test file
pnpm vitest run -t "test name"                # filter by name
cargo test --workspace                                               # Rust tests
cargo test -p washi-core <name>                                       # a single Rust test
cargo test -p washi-core -- --ignored    # tests that need the network (examples/packages.typ etc.)
pnpm release                    # build the distributable .app (scripts/release.sh; strips local paths with a RUSTFLAGS remap)
```

There is no lint or formatter configuration.

### Checking screens in a browser only (no Tauri)

`e2e/` is a harness that saves real Rust render output and returns it through a mocked Tauri API (`e2e/mock-tauri.ts`).

```sh
cargo test -p washi-core dump -- --ignored   # generate e2e/fixtures
pnpm dev
# http://localhost:1420/e2e/harness.html?file=/x/examples/showcase.md&outline=1&theme=dark
# scroll to a heading: &scroll=4.%20Math (the start of the heading text); wait time: &settle=5000 (ms)
```

## Architecture

**Rust renders; the frontend only displays and keeps state.** The Cargo workspace has two crates: `crates/washi-core` (no tauri dependency: `render/`, `cli/`, `editor.rs`) and `src-tauri` (the tauri app, depends on `washi-core`). For rendering and the CLI alone, `cargo test -p washi-core` avoids building tauri. `main.rs` calls `washi_core::cli::run` first and starts the GUI only when it returns `None`.

- `crates/washi-core/src/render/`: one `Renderer` trait implementation per format (`markdown` / `mermaid` / `typst` / `latex`). The static `RENDERERS` array in `render/mod.rs` maps extensions to renderers. Every format renders to HTML (`Output::Html`, the only variant); over IPC it is sent as `into_wire()`: a 1-byte tag (0 = HTML) followed by the body. `render_text()` (paste) is an optional method with a default implementation. PDF files are not supported. Format detection for pasted text is in `render/detect.rs`.
- Adding a format: implement `Renderer`, then add one line to `RENDERERS`. The frontend needs no change.
- External commands (pandoc, and a TeX engine for TikZ) go behind traits (`HtmlConverter`, `TexCompiler`) and are replaced in tests. Run them through `run` in `render/process.rs`: it supports a timeout (default 300 s, `WASHI_COMPILE_TIMEOUT`) and kills the whole process group, and `Job::start(path)` lets a new render of the same document cancel the old one (the frontend discards stale results by token, so this is safe). Typst compiles in-process (with `typst` and `typst-html`), so it cannot time out. LaTeX is converted by `pandoc` (`render/latex.rs`).
- File watching covers the document's folder (non-recursive) plus the folders of the files the document loads. The folders returned by `Renderer::dependency_dirs` (`render/deps.rs` scans the source statically: `\input` etc. for LaTeX, `#include` etc. for Typst, images for Markdown) are applied to the watch set by `commands::render` via `FileWatcher::set_dependencies` after every render (even when it fails). There is no recursive watching (it gets heavy when a wide folder such as HOME is opened).
- File sidebar (⌘B): the list comes from `scan` in `washi-core`'s `tree.rs` (only formats Washi can read; entries starting with `.` and `node_modules` are skipped; folder symlinks are not followed; limits of depth 8 and 5,000 entries set `truncated`; `contains` checks that a path is under a root). In `src-tauri`, `list_documents` returns the list and `open_in_tab` opens a file (only under the root). `TreeRoots` pins the root per window and hands it to tabs opened from the sidebar. Tabs are native macOS tabs (`tabs.rs` joins them with `addTabbedWindow`; their labels are `tab-N`) and are excluded from the `window-state` plugin (`launch::remembers_geometry`; otherwise a saved window size is restored and the whole tab group changes size). A file that is still waiting to open counts as open, so repeated clicks do not create several tabs. The frontend is `src/filetree/` (`model.ts` is the DOM-free tree and key movement, `panel.ts` the DOM). There is no recursive watching, so the list refreshes when a document opens, when the window gets focus, and from the refresh button.
- `src-tauri/src/`: `commands.rs` (Tauri commands called from the frontend), `launch.rs` (launch arguments, opening from Finder, creating one window per file), `watch.rs` (file watching; ignores editor temp files and `.DS_Store`; handles atomic saves), `menu.rs` (native menu). In `washi-core`: `cli/` (the `washi` CLI; returns immediately without waiting for startup; `install` / `formats` subcommands; `-` reads stdin) and `editor.rs` (resolves the external editor that `open_source` opens at a file and line; override with `WASHI_EDITOR`).
- `src/viewer.ts`: the center of the render flow. It takes a `Host` (abstraction of Tauri calls) and `View[]` by injection, so tests and the mock environment can replace them. Every re-render increments `token` to discard stale results, and `Scroll` keeps the scroll position.
- `src/views/`: `View` implementations (`MarkdownView` shows every format's HTML, after `sanitize`). `outline.ts` is the outline, `api.ts` the wrapper around Tauri `invoke`.
- Markdown is converted to HTML on the Rust side (comrak); KaTeX, Mermaid, and highlight.js post-process on the frontend. Relative images are embedded, and relative links to `.md` / `.typ` / `.tex` open inside Washi.

## Editing (⌘E split view)

The CLI is unaffected by the editing feature, and editing is a separate, additive path (`render_buffer`). Reading mode and editing produce the same HTML for a saved file, both with `data-sourcepos`, so ⌘-click works while reading too.

- Rust: `render::render_buffer(path, text)` renders the unsaved text and returns `Rendered{output, diagnostics}` (diagnostics are returned even on failure). LaTeX converts a copy of the text in a temporary folder with the document's folder as the working directory (so `\input` works), and a chapter with `% !TEX root` converts its main file. Completion uses `typst_ide::autocomplete`. The conversion between UTF-16 (CodeMirror), UTF-8 bytes (Typst) and code-point columns (diagnostics) lives in one place, `render/offsets.rs`.
- Response format: `[tag u8: 0 html / 2 failure][u32 BE diagnostics JSON length][diagnostics JSON][body]` (`Rendered::into_wire` ⇄ `decodeBuffer` in `api.ts`). The existing `render` format does not change.
- Saving: `write_file` in `files.rs` writes through a temp file in the same folder plus `rename` (symlinks resolved first, permissions preserved) and returns `Conflict` when the disk hash differs from `base_hash`. The Tauri-side `write_file` only writes to a file the window has open (`Documents::owns(label, path)`).
- Frontend `src/editor/`: the pure, DOM-independent parts (`buffer.ts` text and dirty tracking, `scheduler.ts` throttle/debounce, `session.ts` saving, conflicts and autosave, `kinds.ts`, `lint.ts` position mapping, `split.ts`, `sync.ts`) have unit tests. `editor.ts` (CodeMirror setup), `languages.ts` and `complete.ts` are loaded dynamically by `controller.ts` on the first ⌘E, and `manualChunks` in `vite.config.ts` puts them in `vendor-cm` (CodeMirror stays out of the startup chunk).
- Typst language support assembles the `/lezer` parts of `codemirror-lang-typst` 0.6.0 (experimental; the version is pinned) in `languages.ts`. Its built-in linter is turned off (squiggles come only from the compiler's diagnostics). If it fails to load, a simple tokenizer is the fallback.
- Render frequency is `renderPolicy` in `kinds.ts` (Markdown and Mermaid: 150 ms throttle; Typst: 300 ms throttle; LaTeX: 500 ms throttle). The editing entry points on `Viewer` are `renderBuffer` / `leaveBuffer` / `setBufferHooks`.
- Close / quit confirmation: windows use `onCloseRequested`; ⌘Q uses Rust's `RunEvent::ExitRequested` (it stops the exit if `DirtyWindows` has an unsaved window and sends `washi://quit-requested`). Before opening another file, `EditingController.release()` asks first.
- The browser harness (`e2e/`): `mock-tauri.ts` has editing mocks (virtual files `__files`, a `__calls` log, simple rendering). `?realpreview=1` uses a real fixture for the preview (for screenshots). Key input, menus, and the close confirmation still need a check in a real window.

## CLI contract (rules for changing it)

The output of `washi --json`, the exit codes, and the stdout / stderr split are a published contract that AI agents rely on. The types are in `crates/washi-core/src/cli/report.rs` and match "For AI agents" in the README.

- Success is one JSON line on stdout; failure is one JSON line on stderr (stdout empty). Without `--json`, plain sentences. Exit codes are 0 / 1 (launch failure) / 2 (anything else), decided in one place, `report::exit_code`.
- Only adding keys is allowed (`FORMAT_VERSION` is 1). Removing or renaming a key, changing a type or meaning, or changing `kind` or a format name (`format`) requires raising `FORMAT_VERSION` and a CHANGELOG entry.
- The contract tests (`contract_*` in `cli/tests.rs`) pin the key sets, types, and streams. If one fails, decide whether the contract may change before touching the test. `readme_documents_the_contract` detects an `ErrorKind` or format name missing from the README.
- Plain `washi` with no arguments starts the app in the foreground (Finder starts the same binary with no arguments; do not change it to detach and return).

## Conventions

- Listen to Tauri events with `listenHere` (`src/events.ts`), never the global `listen` from `@tauri-apps/api/event`: that one also receives events Rust sends to other windows and tabs with `emit_to`. A test enforces it.

- Do not write comments in code (including Rust `///` and docstrings). Express intent through names and tests.
- UI text, error messages, and commit messages are in English.

## Supporting screen features

- Command palette (⌘K): `src/palette/`. The command IDs in `commands.ts` are the same as the IDs in `menu.rs` / `actions.ts` (a test cross-checks them). `match.ts` is fuzzy search, `items.ts` ordering, `palette.ts` the `<dialog>`.
- Recent files: `src/recent.ts` (localStorage, up to 8). The start screen and the palette use it.
- Search count: `src/search-count.ts` (counting) and `src/search-source.ts` (the visible document's text and the selection position). `window.find` is not used because it searches the whole page and moves focus; instead the counted matches are selected and navigated.
- Reading-progress line: `src/progress.ts`.
- Update notice: `update.rs` in `washi-core` asks the GitHub API for the latest release (`ureq`, already a dependency of the Typst packages; a 10 s timeout; only a `kiwamizamurai/washi/releases/` page is ever opened), and the `latest_release` command returns `{current, latest, url}`. `src/update.ts` decides (at most once a day across all windows and tabs, by a time stored in localStorage before the request; automatic checks can be turned off), and `src/update-notice.ts` is the bottom-right `<aside id="update">`. It only tells the user to run `brew upgrade --cask washi`; it never replaces the app (the Cask has no `auto_updates`). The harness shows it with `&update=2.1.0` (after clearing `washi:update` in localStorage).
- "Rendering …" message: `src/busy.ts` (shown after 0.4 s, because the first build can wait for Typst packages to download).
- The landing page (`docs/`) is HTML and CSS only, plus GSAP (cdnjs) for the feature animations.

## Typst figures (`render/typst.rs`, `render/typst_frames.rs`)

`typst-html` drops content it cannot express (`convert.rs::handle`: anything but text, boxes, blocks, HTML elements and `html.frame`) with only a "X was ignored during HTML export" warning. Washi compiles in up to two passes:

- A prelude of show rules (`typst_frames::PRELUDE`) maps `align`, `columns`, `grid` (track sizes, `align` as one value or per column, `colspan`), `stack` and `pad` to HTML, so their text survives.
- `typst-html` also drops `text(size:, weight:, style:, fill:)` styling silently. `styled_text_wraps` wraps `text(...)` calls that have a content body and literal values in `<span style>` on every pass (colours darker than 35% luminance are left to the theme).
- If the first pass has a significant drop (not `v`, `h`, page breaks or set rules), calls whose output vanished are wrapped in `html.elem("div", class: "washi-figure", html.frame(…))` and compiled again; `html.frame` lays the content out as a page and embeds it as SVG (the approach shiroa uses). A call counts as vanished when no output node's span lies inside it, and it is wrapped only if the evidence points at it: a local `let` function whose body dropped content, a builtin with a drop inside the call, or an imported or module function when another file (a package) dropped content. Calls in `let`, `set`, `show`, imports, closures and named arguments are never wrapped, nor are non-content callees (`len`, `rgb`, `counter`, …).
- `Edits` records every inserted byte (the prelude, the wrappers) so `data-sourcepos` and diagnostics map back to the user's text; a wrapper element takes the position of the call it wraps. Included files are rewritten through `with_static_source_file_resolver`. If the second pass fails on a wrapper, that wrapper is dropped and it retries (up to 4 times), then falls back to the first pass.

## Source positions (jumping and cursor follow)

Every format marks elements with `data-sourcepos="line:col-line:col"`, the format comrak uses, so one set of frontend code (`src/editor/sync.ts`) serves them all: cursor follow (`elementForLine`) and ⌘-click (`sourceAt`). Elements from an included file also carry `data-source-file` (a path relative to the document's folder) and are ignored by cursor follow.

- Markdown: comrak, only when rendering a buffer.
- Typst: `stamp_sources` in `render/typst.rs` walks the `HtmlDocument` and writes each element's span.
- LaTeX: pandoc cannot do this for LaTeX (`+sourcepos` exists only for commonmark and gfm), so `render/latex_marks.rs` puts `\hypertarget{washi-L<line>}{}` at the start of each paragraph, item and environment outside math, tables and verbatim text, and turns the markers that survive in the HTML into `data-sourcepos`. Lines are never added or removed, so numbers stay exact. If pandoc fails on the marked copy, it is run again on the original text. Text pulled in with `\input` has no positions.
- Reading mode: ⌘-click calls `open_source` (an external editor at that file and line; the file must be inside the document's folder). Editing: ⌘-click moves the cursor.

## LaTeX notes

- pandoc must be installed (`tools::find_tool("pandoc")`, which also searches `/opt/homebrew/bin` because an app launched from Finder / the Dock has a minimal `PATH`). Without it the error says `brew install pandoc`; there is no PDF fallback.
- pandoc runs with `--standalone --embed-resources` and a small template (title, author, date, abstract, then the body), so images become data URIs and no `<html>` is returned. `\bibliography{refs}` is found by pandoc's citeproc; a `thebibliography` environment is not used for citations.
- TikZ (`render/tikz.rs`): `tikzpicture` / `tikzcd` / `circuitikz` / `pgfpicture` are found outside comments and verbatim, built one by one as `\documentclass[border=2pt]{standalone}` + the user's preamble (without `geometry` and page styles), by `pdflatex` / `lualatex` / `xelatex` or else `tectonic` (`SystemTex`, through `process::run` and `Job`). The PDF becomes SVG in process with hayro (`render/pdf_svg.rs`, the same settings as `typst-svg`), is cached in `washi-tikz-cache` by the text's hash, and the environment is replaced by `\includegraphics{<svg>}` on the same number of lines. Failures leave a `\emph{[Drawing not shown: …]}` note. This follows pandoc-ext/diagram (standalone + pdflatex) but avoids inkscape.
- What is lost against a PDF build: `siunitx` (`\SI`), `\notag` in `align`, page layout. `.latexmkrc` is never read.
- `% !TEX root = main.tex` is read by `magic_root` in `latex.rs`; the main file is converted.
- Equation numbers (`render/latex_equations.rs`): pandoc does not number equations, so Washi counts them from the original source (`equation`/`multline` once, `align`/`gather`/`flalign`/`alignat`/`eqnarray` per top-level `\\` row, skipping starred environments, `\notag`/`\nonumber` rows and `\tag`), puts `<span class="eq-number">(n)</span>` before each `<math display="block">` when the count of display blocks matches the source, and rewrites `data-reference-type="eqref"`/`"ref"` links to the number.
- `\notag` and `\nonumber` are removed before pandoc runs (pandoc cannot parse them inside `align` and would leave the whole environment as raw TeX; the HTML has no equation numbers anyway).

## CI and release

- `.github/workflows/ci.yml` runs on every pull request to `main`, on every push to `main`, and by hand (`workflow_dispatch`). Pull requests that change only `docs/**` or Markdown skip it. The token is `contents: read`, and each job has a timeout. It has three jobs:
  - `core` (Linux, no Tauri): `cargo test -p washi-core`. Fast feedback on rendering and the CLI.
  - `web` (Linux): `pnpm test` and `pnpm build` (vitest and the type check).
  - `app` (macOS): `cargo test --workspace`, including the tauri crate. The tauri build only needs `frontendDist` (`dist`) to exist, so the job creates an empty `dist` and does not install Node.
- The Rust cache (`Swatinem/rust-cache`) is saved only by runs on `main` (pull requests only restore it). That is why CI also runs on pushes to `main`: caches created on a pull request are not shared with other pull requests.
- Fork pull requests from a first-time contributor wait for approval in the Actions tab before they run. The workflow uses `pull_request`, never `pull_request_target`, so forks get no secrets.
- `.github/workflows/release.yml`: starts when a GitHub Release is published (e.g. `gh release create v0.1.0 --generate-notes`). It checks out that tag and fails unless the tag has the form `v<major>.<minor>.<patch>` and matches the versions in `package.json` / `tauri.conf.json` / `src-tauri/Cargo.toml`. It builds the `.app` with `pnpm release`, attaches `Washi-<version>-aarch64.zip` and `.sha256` to the published Release with `gh release upload`, and generates and pushes `Casks/washi.rb` in `kiwamizamurai/homebrew-tap` (needs the secret `HOMEBREW_TAP_TOKEN`; if it is unset, only the tap update is skipped with a warning). It can also be rerun by hand from Actions with a `tag`.
- Distribution is Apple Silicon macOS only. The signature is ad-hoc, so the Cask's `postflight` removes the quarantine attribute. No tool is bundled; the Cask installs `pandoc` (LaTeX) and `tectonic` (TikZ drawings) with `depends_on formula: ["pandoc", "tectonic"]`, written by the template in `release.yml`. When the tap is updated by hand, keep the dependencies and `desc` in step with that template.
- While `HOMEBREW_TAP_TOKEN` is unset, the tap update is skipped with a warning, so after a release update `version` and `sha256` in `Casks/washi.rb` of `kiwamizamurai/homebrew-tap` by hand. Actions are pinned by commit SHA (comments are not allowed, so look up the version from the SHA).

## Gotchas

- Icons: `python3 scripts/make_logo.py --all` makes `app-icon.svg`, `docs/assets/logo.svg` (for the README), and `docs/favicon.svg`. Then make PNG/ICNS/ICO with `pnpm tauri icon app-icon.svg --output <temp folder>` and replace only the ones whose names already exist in `src-tauri/icons/` (the iOS and Android ones are not used).
- File extensions registered with Finder are `bundle.fileAssociations` in `src-tauri/tauri.conf.json`. When you add a format, add it here too (a test in `src-tauri/src/lib.rs` detects a mismatch with `supported_extensions()`).
- Tauri permissions are managed in `src-tauri/capabilities/` and `tauri.conf.json`. Using a new plugin API requires adding a capability.
- `examples/` holds realistic sample documents for each format (in English; they are the source of the landing screenshots `docs/assets/shot-*.jpg`, taken with the harness at 1221×641 using headless Chrome driven by `puppeteer-core`) and are also the input of the `dump` tests.
- The CLI supports macOS / Linux (Windows is not supported). Signing and notarization are not supported.
