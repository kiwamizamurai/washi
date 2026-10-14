#import "@preview/cetz:0.5.2": canvas, draw
#import "@preview/fletcher:0.5.8": diagram, node, edge

#set page(width: 15cm, height: auto, margin: 1cm)
#set text(lang: "en", size: 10pt)

= `@preview` package examples

== A figure drawn with CeTZ

#align(center, canvas({
  import draw: *
  circle((0, 0), radius: 1.2, stroke: rgb("#b5483a"), fill: rgb("#f6efe6"))
  line((-2.2, 0), (2.2, 0), mark: (end: ">"))
  line((0, -1.6), (0, 1.6), mark: (end: ">"))
  content((0, 0), [Washi])
  content((2.4, -0.3), [$x$])
  content((0.3, 1.8), [$y$])
}))

== A Fletcher diagram

#align(center, diagram(
  node-stroke: 0.8pt,
  node-inset: 8pt,
  node((0, 0), [Source]),
  edge("->", [load]),
  node((1, 0), [Renderer]),
  edge("->", [HTML]),
  node((2, 0), [WebView]),
  edge((1, 0), (1, 1), "->", [SVG]),
  node((1, 1), [Drawings]),
  edge((1, 1), (2, 0), "->"),
))
