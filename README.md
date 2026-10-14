![Washi logo](docs/assets/logo.svg)

# Washi（和紙）

A small macOS app that opens Markdown, Typst, LaTeX and Mermaid in one window, redraws when the file changes, and edits with a live preview.
It opens as a reader. Press `⌘E` for an editor with the preview beside it, or keep using your own editor: Washi redraws either way.

Website: [English](https://kiwamizamurai.github.io/washi/) · [日本語](https://kiwamizamurai.github.io/washi/ja/)

## Install

Apple Silicon macOS, with [Homebrew](https://brew.sh):

```sh
brew install --cask kiwamizamurai/tap/washi
brew upgrade --cask washi     # update
```

- This also puts the `washi` command on your PATH and installs `pandoc`, which Washi uses to show LaTeX, and `tectonic`, which draws TikZ pictures (`pdflatex` is used instead if you have TeX Live). Typst is built in.
- Homebrew trusts the tap for the fully qualified name above. To install by short name, run `brew trust --cask kiwamizamurai/tap/washi` first.
- Washi is not signed or notarized (ad-hoc signature), so the Cask removes the quarantine attribute that Homebrew adds.
- Without Homebrew: download `Washi-<version>-aarch64.zip` from [GitHub Releases](https://github.com/kiwamizamurai/washi/releases), unzip it into `/Applications`, then allow it once under System Settings → Privacy & Security → Open Anyway.
- To build from source: see Development below.

## Usage

- **Open**: drop a file on the window, `⌘O`, Finder's "Open With", or `washi a.md b.typ` (one window per file).
- **Recent files**: the start screen lists the files you opened last.
- **Files**: press `⌘B` for a sidebar that lists the documents in the open file's folder and below it (Markdown, Typst, LaTeX and Mermaid; hidden folders and `node_modules` are skipped). Click one to open it in a new tab, or to switch to its tab if it is already open. The sidebar keeps the folder it started from while you move around inside it.
- **Paste**: copy text and press `⌘V`; Markdown, Typst, LaTeX or Mermaid is detected from the content.
- **Edit**: press `⌘E` to split the window, with the source on the left and the preview on the right, updating as you type. `⌘E` again returns to reading. See [Editing](#editing).
- **Updates**: once a day Washi asks GitHub for the latest release. When there is a newer one, a small notice in the bottom-right corner shows `brew upgrade --cask washi` and a link to the release notes. Washi never updates itself. "Check for Updates…" is in the Washi menu and the palette, and "Toggle Automatic Update Checks" turns the daily check off.
- **Save to refresh**: save the open file, or any file it includes (LaTeX `\input` and `.bib`, Typst `#include`, Markdown images, even in subfolders), and it re-renders in place, keeping your scroll position.

| Action | Shortcut |
|---|---|
| Open / Reload / Print | `⌘O` / `⌘R` / `⌘P` |
| Command palette | `⌘K` (commands, recent files, `#` for headings) |
| Edit (split view) / Save | `⌘E` / `⌘S` |
| Find (and replace, while editing) | `⌘F` (`Enter` next, `⇧Enter` previous, `Esc` close; shows "3 / 12") |
| Toggle the outline / the file list | `⇧⌘O` / `⌘B` |
| Zoom in / out / actual size | `⌘+` / `⌘-` / `⌘0` (`⌘` + wheel and pinch also work) |
| Jump to source | `⌘`-click the preview |
| Theme, text width | "View" menu |

**Jump to source** moves the editor's cursor to that line while you edit. When you are only reading, it opens the line in `code`, `cursor`, `zed`, `subl` or `mate` (first one found; otherwise the default text editor). Set `WASHI_EDITOR` to choose, for example `WASHI_EDITOR="code -g {file}:{line}:{column}"`.

## Editing

Washi always opens a file for reading. Press `⌘E` to edit Markdown, Typst, LaTeX or Mermaid: the window splits into the source and the live preview. The editor is loaded only then, so reading stays light.

- **Live preview**: the preview renders your unsaved text as you type. Relative images, `#include` and `\input` keep working. Typst re-renders at most about three times a second, and LaTeX about twice a second.
- **Editor**: syntax highlighting, line numbers, undo and redo, search and replace (`⌘F`), and for Typst, completion and error squiggles. When the text does not compile, the last good preview stays on screen and the error shows in the status bar. The status bar also shows saved or unsaved, the character count, and error and warning counts: click one to jump to the next problem. Returning to reading with `⌘E` and back keeps your undo history.
- **Saving**: `⌘S` saves, and `●` in the title marks unsaved changes. Closing the window or quitting asks first. Autosave (after a second of idle) is off by default; File → "Toggle autosave" switches it on. If the file changes on disk while you have unsaved edits, Washi shows a banner and lets you keep your version or load the disk's. It never overwrites silently.
- **Source and preview follow each other**: `⌘`-click the preview to move the cursor to that source line, and the preview scrolls to the line your cursor is on (View → "Show the cursor line in the preview" turns this off). Drag the divider to resize the panes.

## Supported formats

| Format | Rendered as | Notes |
|---|---|---|
| Markdown | HTML | GFM, footnotes, math (KaTeX), Mermaid, syntax highlighting, alerts, front matter, relative images and links |
| Typst | HTML | Built-in compiler; `@preview` packages are fetched on first use; errors show line and column. Typst's HTML export is still under development; Washi keeps `align`, `grid` (column widths and alignment) and `columns` as HTML, keeps literal `text(size:, weight:, style:, fill:)` styling, and shows drawings it cannot express (CeTZ, Fletcher, `place`, …) as SVG figures. Page size, margins and page breaks do not apply |
| LaTeX | HTML | Converted by [`pandoc`](https://pandoc.org) (math as MathML); stops after 300 s (`WASHI_COMPILE_TIMEOUT` changes it). See [LaTeX notes](#latex-notes) |
| Mermaid | SVG | `.mmd` / `.mermaid`; click a diagram to enlarge it |

Washi does not open PDF files. It does not build PDFs from Typst or LaTeX either, so the preview is a reading view, not the printed page.

### LaTeX notes

- **pandoc is required.** Install it with `brew install pandoc`. Without it Washi says so instead of showing the document. `.latexmkrc` is never read.
- **TikZ pictures** (`tikzpicture`, `tikzcd`, `circuitikz`, `pgfpicture`) are drawn one by one with your preamble by `pdflatex` (or `lualatex` / `xelatex`), or `tectonic` when TeX Live is missing, and shown as images. Each drawing is cached until its text changes; the first drawing with `tectonic` downloads TikZ and takes about a minute. Without a TeX engine, a short note says so where the drawing would be.
- **Several files**: put `% !TEX root = main.tex` on the first lines of a chapter, and Washi converts the main file when you open or edit the chapter. While you edit a chapter, the preview shows the saved text of the main file and its includes.
- **Bibliographies**: `\bibliography{refs}` with a `.bib` next to the document is resolved by pandoc. A hand-written `thebibliography` environment is not used for citations, so `\cite` keys in the body stay as `(key?)`; use a `.bib` file.
- **Equation numbers** are counted as LaTeX counts them (`equation`, each row of `align` and `gather`, skipping `\notag`, `*` environments and `\tag`), shown beside the math, and `\eqref` / `\ref` to them show the number.
- **What does not show**: `siunitx` units (`\SI`), page layout, headers and footers, and macros from packages pandoc does not know. Math is shown from the TeX source, so custom macros defined with `\newcommand` in the preamble work.
- **Source jump** for LaTeX works for paragraphs, headings and list items in the main file (not for math or table cells, or for text pulled in with `\input`).
- **Not UTF-8**: sources in Shift_JIS or EUC-JP cannot be converted; save them as UTF-8.

## For AI agents

`washi <file>` opens a file and **returns immediately**, so agent commands are never blocked.

```sh
washi plan.md --json                  # {"format_version":1,"launched":true,"files":[{"path":"/abs/plan.md","format":"markdown"}]}
washi plan.md --no-launch --json      # validate only
cat draft.md | washi - --name draft   # open stdin; the same --name updates the open window
washi formats --json                  # supported formats and the tools Washi found
```

- **Streams**: success prints one JSON line on stdout and nothing on stderr. Failure prints one JSON line on stderr, `{"format_version":1,"errors":[{"kind":...,"message":...,"path"?:...}]}`, and nothing on stdout.
- **Exit codes**: `0` success, `1` the app could not be launched (`launch_failed`), `2` anything else. Only `0` means success.
- **`kind`**: `usage`, `not_found`, `is_directory`, `unsupported_format`, `stdin_empty`, `stdin_unreadable`, `io`, `launch_failed`. `files[].format` is one of `markdown`, `mermaid`, `typst`, `latex`.
- **Compatibility**: keys are only added, never removed, renamed or retyped, so ignore keys you do not know. A breaking change raises `format_version` and is announced in the [changelog](CHANGELOG.md).

## Comparison with other apps

| | Washi | [Typeset Viewer](https://typeset.osteele.com) | [SuperGoodViewer](https://github.com/dotsg/SuperGoodViewer) | [Osh](https://github.com/Hyp4tia/Osh) | [hibi](https://github.com/schmayterling/hibi) | [Texpile](https://github.com/texpile/texpile) | [Oleafly](https://github.com/Oleafly/Oleafly) | [Quire Writer](https://github.com/Andesprit/quire-writer) | [Moraya](https://github.com/zouwei/moraya) |
|---|---|---|---|---|---|---|---|---|---|
| Typst files | Yes | Yes | No (read as Markdown) | No (math blocks only) | Yes | Yes | Yes | Yes | Yes (downloads the typst CLI) |
| LaTeX files | Yes (pandoc) | Yes (latexmk) | No | No | Yes (Tectonic, compiled by hand) | Yes | Yes | Yes | No |
| Mermaid `.mmd` files | Yes | Not stated | No | Quick Look only | Yes | No | Diagram Composer import | No | No |
| Other formats | None | PDF | PDF | None | About 15 (rST, AsciiDoc, Org, MDX, HTML, …) | PDF in a folder | PDF imported as LaTeX | Quarto | None |
| Shows | HTML | PDF pages | PDF pages | HTML | Typst SVG, LaTeX PDF | PDF, Typst preview | PDF pages | PDF / HTML / tinymist | WYSIWYG, Typst SVG |
| Redraws when another app saves the open file | Yes | Yes | Yes | Yes, not while editing | Only when you return to the tab | Editor text only; LaTeX needs live preview (off by default) | Folders opened in place, with auto-compile (off by default) | Yes, as an edit to review | Only when the window gets focus |
| Redraws when an included file changes | Yes | Yes | Images only | No | Typst in a workspace | Not stated | No | LaTeX, Typst | No |
| Preview to source jump | Yes | Not stated | No | No | Markdown only | Typst, LaTeX | Typst, LaTeX | Typst | No |
| CLI | Open, `--json`, exit codes, stdin | `--render`, `--doctor`, `--appearance` | Open, export, stdin | `--export-pdf` | No | No | `oleaflyc` (build from source) | No | No |
| Agent integration | CLI | Claude Code / Codex | CLI export | `.skill` files | MCP server | MCP server, ACP | MCP, ACP | ACP agents | MCP client, AI chat |
| Platforms | macOS arm64 | macOS 14+ arm64 | macOS, Windows, Linux | macOS 12+ | macOS, Windows, Linux | macOS, Windows, Linux | macOS arm64, Windows, Linux | macOS arm64; Windows, Linux experimental | macOS, Windows, Linux |
| Notarized | No | Yes | No | Yes | Yes | Yes | Yes | Yes | Yes |
| macOS download | 25.0 MB | 29.6 MB | 40.8 MB | 38.2 MB | 170.1 MB | 261.1 MB | 201.1 MB | 58.8 MB | 8.0 MB |
| License | MIT | Proprietary, free | MIT | GPL-3.0 | GPL-3.0-only | AGPL-3.0 | AGPL-3.0-or-later | GPL-3.0-or-later | Apache-2.0 |
| Version | 0.1.0 | 0.8.0 | 1.1.1 | 1.0.12 | Nightly only | 1.5.0 | 0.4.6 (beta) | 0.2.9 | 0.45.17 |

Typeset Viewer is the closest app to Washi: a reader that redraws on save, follows included and `.bib` files and keeps your place. It shows the real typeset PDF, is notarized and adds notes, AI review and presentation mode, but it is closed source. Washi is open source and shows HTML instead of pages, so it loses page layout and some LaTeX packages (see [LaTeX notes](#latex-notes)). hibi opens the most formats. If you want to write in the app, Texpile, Oleafly, hibi, Quire Writer and Moraya are editors first.

## Development

```sh
pnpm install
pnpm tauri dev          # run (open a file: pnpm tauri dev -- -- --gui examples/showcase.md)
pnpm test               # frontend tests
cargo test --workspace  # Rust tests (cargo test -p washi-core skips Tauri and is fast)
pnpm release            # release build of the .app
```

Rendering and the CLI live in `crates/washi-core` (no Tauri dependency); the app is `src-tauri` plus `src` (TypeScript). To add a format, implement `Renderer` in `crates/washi-core/src/render/`, add it to `RENDERERS` in `render/mod.rs`, and add its extensions to `fileAssociations` in `src-tauri/tauri.conf.json`. Publishing a GitHub Release (`gh release create v0.1.0 --generate-notes`) runs `.github/workflows/release.yml`: it builds the `.app` for that tag, attaches it to the release, and updates the Homebrew tap. CI (`ci.yml`) runs on every pull request to `main` and every push to `main`, and can also be started by hand.

## License

MIT. Dependencies follow their own licenses (Typst: Apache-2.0, KaTeX / Mermaid: MIT, comrak: BSD-2-Clause, highlight.js: BSD-3-Clause, and others).

The logo and app icon are generated by `scripts/make_logo.py`.
