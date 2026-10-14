# Changelog

All notable changes are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to follow [Semantic Versioning](https://semver.org/).

The `washi` command-line interface (the `--json` output, exit codes, and the stdout / stderr split) is a public contract; see "For AI agents" in the [README](README.md). Any change that is not purely additive raises `format_version` and is listed here before it ships.

## [Unreleased]

## [0.1.0] - 2026-10-15

First release.

- Opens Markdown, Typst, LaTeX and Mermaid files for reading and shows them as HTML. Typst is compiled inside the app; LaTeX is converted by `pandoc`, which must be installed. Typst drawings (CeTZ, Fletcher) and LaTeX TikZ pictures appear as figures.
- Redraws when the file, or a file it includes (`\input`, `.bib`, `#include`, images), is saved by any program, and keeps the scroll position and zoom.
- `⌘`-click the preview to open that line in your editor; `⌘E` opens a split editor with a live preview.
- File sidebar (`⌘B`) with native tabs, outline (`⇧⌘O`), command palette (`⌘K`), find with a match count, recent files, light and dark themes.
- `washi` command-line tool: opens files and returns at once, reads stdin with `-`, and gives machine-readable results with `--json` (`format_version` 1) and stable exit codes.
- Update notice: once a day Washi asks GitHub for the latest release and, when there is a newer one, shows `brew upgrade --cask washi` in a small notice in the bottom-right corner. It never updates itself. The check can be turned off.
- Apple Silicon macOS only. The app has an ad-hoc signature and is not notarized.
