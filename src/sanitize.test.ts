// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { sanitize } from "./sanitize";

describe("sanitize", () => {
  it("removes scripts, frames and other elements that load or run code", () => {
    const out = sanitize('<p>a</p><script>alert(1)</script><iframe src="https://x"></iframe><object data="x"></object><embed src="x"><base href="/"><meta http-equiv="refresh" content="0"><link rel="stylesheet" href="https://x/a.css">');
    expect(out).toBe("<p>a</p>");
  });

  it("removes event handler attributes whatever their case", () => {
    const out = sanitize('<p onclick="x()" ONMOUSEOVER="y()" class="keep">a</p>');
    expect(out).toBe('<p class="keep">a</p>');
  });

  it("removes javascript, data and script-like links even when disguised", () => {
    for (const href of ["javascript:alert(1)", " JaVaScRiPt:alert(1)", "java\tscript:alert(1)", "data:text/html,<b>x</b>", "vbscript:x"]) {
      expect(sanitize(`<a href="${href}">x</a>`), href).toBe("<a>x</a>");
    }
  });

  it("keeps ordinary links", () => {
    for (const href of ["https://example.com/a?b=1", "http://example.com", "mailto:a@b.c", "#section", "notes/other.md", "../up.tex", "/abs/path.typ"]) {
      expect(sanitize(`<a href="${href}">x</a>`), href).toBe(`<a href="${href}">x</a>`);
    }
  });

  it("keeps embedded images but not scripts disguised as images", () => {
    expect(sanitize('<img src="data:image/png;base64,AAAA">')).toBe('<img src="data:image/png;base64,AAAA">');
    expect(sanitize('<img src="javascript:alert(1)">')).toBe("<img>");
    expect(sanitize('<img src="data:text/html,<script>1</script>">')).toBe("<img>");
  });

  it("keeps what Typst, pandoc and Markdown rely on", () => {
    const html =
      '<style>mtable{math-style:inherit}</style><math display="block"><mfrac><mi>a</mi><mn>2</mn></mfrac></math>' +
      '<p style="margin:0" data-sourcepos="3:1-3:9" data-source-file="part.typ">x</p>' +
      '<span data-math-style="inline">x^2</span><pre><code class="language-mermaid">A--&gt;B</code></pre>' +
      '<input type="checkbox" disabled="">';
    expect(sanitize(html)).toBe(html);
  });

  it("keeps a Typst figure drawn as inline SVG, including its glyph references", () => {
    const svg =
      '<div class="washi-figure"><svg style="width: 2em; height: 1em; display: block" viewBox="0 0 20 10" xmlns="http://www.w3.org/2000/svg">' +
      '<use xlink:href="#g1" x="0" y="0" fill="#000000"></use><defs><symbol id="g1" overflow="visible"><path d="M 0 0 L 1 1"></path></symbol></defs></svg></div>';
    const out = sanitize(svg);
    expect(out).toContain('xlink:href="#g1"');
    expect(out).toContain("<symbol");
    expect(out).toContain('class="washi-figure"');
  });

  it("keeps a TikZ drawing embedded as an SVG data image", () => {
    const img = '<img src="data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=">';
    expect(sanitize(img)).toBe(img);
  });
});
