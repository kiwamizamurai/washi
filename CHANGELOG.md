# Changelog

All notable changes are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to follow [Semantic Versioning](https://semver.org/) from 1.0.0.

The `washi` command-line interface (the `--json` output, exit codes, and the stdout / stderr split) is a public contract; see "For AI agents" in the [README](README.md). Any change that is not purely additive raises `format_version` and is listed here before it ships.

## [Unreleased]

### Fixed
- With `latexmk`, the engine chosen in `.latexmkrc` (platex, xelatex, lualatex) is no longer overridden. Washi used to pass `-pdf`, which forced pdfLaTeX; it now asks for a PDF only when no rc file has chosen how to make one.
- LaTeX failures now start with a plain explanation when the cause is known: biblatex without `biber`, a document that needs `-shell-escape` (such as `minted`), a document that needs pLaTeX, or a chapter file without `\documentclass`. Before, a missing `biber` showed only "No such file or directory".

### Added
- `% !TEX root = main.tex` is understood. Opening or editing a chapter builds the main file, watches the main file's includes, and follows SyncTeX through it. While the chapter has unsaved changes, the preview keeps the last build and rebuilds on save.
- A "LaTeX notes" section in the README on engines, multi-file projects, biblatex, pLaTeX and shell escape.

## [1.0.0] - 2026-10-07

Washi still opens every file for reading. From 1.0.0 you can also edit, with a live preview beside the source.

### Added

**Editing**
- Press `⌘E` to split the window into a CodeMirror 6 editor and the preview, for Markdown, Typst, LaTeX and Mermaid (not PDF). Press it again to return to reading. The editor is loaded only on first use, so opening a file stays as light as before.
- The preview renders your unsaved text as you type, with relative images, `#include` and `\input` still resolved. Markdown and Mermaid refresh at most every 150 ms, Typst every 400 ms, and LaTeX about 1.2 s after you stop typing and on save. When the text does not compile, the last good preview stays and the error appears in the status bar.
- Syntax highlighting, line numbers, undo and redo, and search and replace. Typst also gets completion (from `typst-ide`) and error and warning squiggles from the compiler.
- Source and preview follow each other: `⌘`-click the preview to move the cursor to the matching source line (Markdown, Typst, LaTeX), and the preview scrolls to the line the cursor is on (View → "Show Cursor Line in Preview" turns it off). The divider between them can be dragged.
- A status bar with `Saved` / `Unsaved`, the cursor position, a character count (and the selection size), and error and warning counts that jump to the next problem when clicked.
- Going back to reading with `⌘E` and returning to the same file keeps the undo history, as long as the file was saved and did not change on disk in between.

**Saving without losing work**
- `⌘S` saves, `●` in the title marks unsaved changes, and closing a window, quitting or opening another file with unsaved changes asks first. Writes are atomic, follow symlinks and keep file permissions. Autosave is off by default (File → "Toggle Autosave"; after one second of idle, at most five seconds).
- If the file changes on disk while you have unsaved edits, a banner lets you keep your version or load the disk's. Washi never overwrites silently, and a window can only write to the file it has open.

**Finding your way**
- A command palette (`⌘K`): run any menu command, reopen a recent file, or type `#` to jump to a heading. Matching is fuzzy and highlights the letters it matched.
- Find shows the number of matches and your position ("3 / 12"; "12+" in a PDF whose far pages are not drawn yet), and stepping with `Enter` / `⇧Enter` stays inside the document.
- A thin reading-progress line along the top edge of the preview.
- Recent files on the start screen (up to eight; files that cannot be opened are dropped; "Clear recent files").
- Menu: File → Save and Toggle Autosave, Edit → Undo, Redo and Cut, View → Edit (Split View) and Command Palette.

**Command line and integration**
- The `washi` CLI contract: every `--json` document carries `format_version` (`1`); failures are a JSON object on stderr with machine-readable `errors[].kind`; success is a JSON object on stdout. Exit codes: `0` success, `1` the app could not be launched, `2` invalid usage or files.
- `files[].format` (`markdown`, `mermaid`, `typst`, `latex`, `pdf`) in the `--json` result, and `formats` plus a sorted `extensions` in `washi formats --json`.
- Re-rendering when files a document includes change: LaTeX `\input` / `\include` / `\bibliography` / `\addbibresource` / `\includegraphics`, Typst `#include` / `#import` / `#image` / `#bibliography` and data files, and Markdown images, including those in subfolders.
- LaTeX builds stop after `WASHI_COMPILE_TIMEOUT` seconds (default 300), including the processes they started, and a newer render of the same file stops an older one.
- Finder registration for `.mmd`, `.mermaid`, `.pdf`, `.latex` and `.mdown`.
- A new logo and app icon (a torn-paper landscape: a vermilion sun over layered indigo mountains), generated by `scripts/make_logo.py`; the About panel shows it in development builds too.
- GitHub Actions: a release workflow (`release.yml`) that, when a GitHub Release is published, builds the app, attaches it to the release and updates the Homebrew tap, and a CI workflow (`ci.yml`) that runs only on demand.

### Changed
- The interface (menus, screens, dialogs and error messages) is in English.
- `washi --json` results list `files` (each with `path` and `format`) instead of `opened`.
- `washi formats` and `washi install` reject unknown options, and `-` may be given only once.
- Rendering and the CLI moved into the `washi-core` crate, which does not depend on Tauri.

## [0.1.0] - 2026-10-06

First release: a viewer for Markdown, Typst, LaTeX, Mermaid and PDF that re-renders on every save while keeping the scroll position and zoom, with an outline, find, text width and theme settings, paste-to-open, jump from a Typst or LaTeX PDF to its source, and a Homebrew tap.
