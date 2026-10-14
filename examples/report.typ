#set document(title: "Washi Technical Report", author: "kiwamizamurai")
#set page(
  paper: "a4",
  margin: (x: 2.2cm, y: 2.6cm),
  header: context {
    if counter(page).get().first() > 1 {
      set text(size: 8.5pt, fill: luma(110))
      [Washi Technical Report #h(1fr) 2026-10-06]
      v(-4pt)
      line(length: 100%, stroke: 0.4pt + luma(200))
    }
  },
  footer: context align(center, text(size: 9pt, fill: luma(110))[
    #counter(page).display("1 / 1", both: true)
  ]),
)
#set text(font: ("Libertinus Serif", "Hiragino Mincho ProN", "Noto Serif CJK JP"), lang: "en", size: 10.5pt)
#set par(justify: true, leading: 0.85em)
#set heading(numbering: "1.1")
#set math.equation(numbering: "(1)")
#show heading: set block(above: 1.4em, below: 0.8em)
#show link: set text(fill: rgb("#b5483a"))
#show raw.where(block: true): block.with(fill: luma(247), inset: 9pt, radius: 3pt, width: 100%)

#let accent = rgb("#b5483a")
#let callout(title: "Note", body) = block(
  width: 100%,
  inset: 10pt,
  radius: 3pt,
  fill: rgb("#f6efe6"),
  stroke: (left: 3pt + accent),
)[#text(weight: "bold", fill: accent)[#title] \ #body]

#align(center)[
  #v(1cm)
  #text(size: 24pt, weight: "bold")[Washi Technical Report]
  #v(2pt)
  #text(size: 12pt, fill: luma(100))[Reading Typst, Markdown and LaTeX quietly]
  #v(6pt)
  kiwamizamurai · 2026-10-06
]

#v(0.6cm)
#outline(title: [Contents], depth: 2, indent: 1.2em)
#pagebreak()

= Overview

Washi is a lightweight viewer that *re-renders local documents the moment you save them*.
It shows Markdown, Typst and LaTeX as HTML, with drawings as SVG @knuth1984 @madje2023.
This report is an example that uses most Typst features: heading numbers, cross-references, tables, math, figures, footnotes, columns and a bibliography.

#callout(title: "Key point")[
  Conversion is hidden behind a common interface called a *Renderer*, so adding a format takes one implementation and one registration.
]

= Architecture <sec:arch>

The rendering flow is shown in @fig:pipeline, and the supported formats in @tbl:formats.

#figure(
  grid(
    columns: (1fr, auto, 1fr, auto, 1fr),
    align: horizon + center,
    gutter: 8pt,
    rect(inset: 10pt, radius: 4pt, fill: luma(245), stroke: 0.6pt + luma(160))[Source \ #text(size: 8pt)[.md / .typ / .tex]],
    text(fill: accent, size: 14pt)[→],
    rect(inset: 10pt, radius: 4pt, fill: rgb("#f6efe6"), stroke: 0.6pt + accent)[*Renderer*],
    text(fill: accent, size: 14pt)[→],
    rect(inset: 10pt, radius: 4pt, fill: luma(245), stroke: 0.6pt + luma(160))[HTML],
  ),
  caption: [The rendering pipeline],
) <fig:pipeline>

#figure(
  table(
    columns: (auto, 1fr),
    align: (left, left),
    stroke: (x: none, y: 0.5pt + luma(190)),
    inset: 7pt,
    table.header([*Format*], [*Rendering path*]),
    [Markdown], [comrak → HTML → WebView (KaTeX / Mermaid)],
    [Typst], [typst crate → HTML (drawings as SVG)],
    [LaTeX], [pandoc → HTML (TikZ via a TeX engine)],
    [Mermaid], [Rendered as a fenced block in Markdown],
  ),
  caption: [Supported formats and their rendering paths],
) <tbl:formats>

== Design principles

- Each format implements the `Renderer` trait and is looked up by extension.
- External commands (pandoc, and a TeX engine for TikZ) sit behind `HtmlConverter` and `TexCompiler` and are swapped out in tests.
- Every format becomes HTML, so the UI shows all of them through one view.

== Math

The basic formulas for an integral and a sum:
$ integral_0^1 x^2 dif x = 1/3, quad sum_(k=1)^n k = (n(n+1))/2 $ <eq:basic>

A matrix and its inverse (continuing from @eq:basic):
$ mat(a, b; c, d)^(-1) = 1/(a d - b c) mat(d, -b; -c, a) $

The Fourier transform:
$ hat(f)(xi) = integral_(-oo)^(oo) f(x) e^(-2 pi i x xi) dif x $

= Implementation notes

== Code example

```rust
pub trait Renderer: Sync {
    fn extensions(&self) -> &'static [&'static str];
    fn render(&self, path: &Path) -> Result<Output, String>;
}
```

== Columns and footnotes

#columns(2, gutter: 16pt)[
  #lorem(70)
  #footnote[Even inside columns, footnotes are placed at the bottom of the page.]

  #lorem(60)
]

#v(0.4cm)
See the #link("https://typst.app/docs")[Typst documentation] for details (see also @sec:arch).

#pagebreak()

= Appendix

== Long text

#lorem(180)

#lorem(150)

#bibliography("refs.bib", title: [References], style: "ieee")
