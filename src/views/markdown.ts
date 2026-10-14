import katex from "katex";
import "katex/dist/katex.min.css";

import type { Output } from "../api";
import { highlightCode } from "../highlight";
import { headingOutline, type OutlineNode } from "../outline";
import type { Scroll } from "../scroll";
import { sanitize } from "../sanitize";
import { isDark } from "../theme";
import { stepZoom } from "../zoom";
import { addCopyButtons } from "./code-copy";
import { enableDiagramZoom } from "./diagram-zoom";
import type { RenderContext, View, ZoomDirection } from "./view";

const BASE_FONT_PX = 16;

function renderMath(root: HTMLElement) {
  for (const el of root.querySelectorAll<HTMLElement>("[data-math-style]")) {
    katex.render(el.textContent ?? "", el, {
      displayMode: el.dataset.mathStyle === "display",
      throwOnError: false,
    });
  }
}

async function renderDiagrams(root: HTMLElement, ctx: RenderContext) {
  const blocks = [
    ...root.querySelectorAll<HTMLElement>("pre > code.language-mermaid"),
  ];
  if (blocks.length === 0) return;

  const { default: mermaid } = await import("mermaid");
  if (!ctx.isCurrent()) return;
  mermaid.initialize({
    startOnLoad: false,
    theme: isDark() ? "dark" : "default",
    securityLevel: "strict",
  });
  const targets = blocks.map((code) => {
    const div = document.createElement("div");
    div.className = "mermaid";
    div.textContent = code.textContent;
    code.parentElement!.replaceWith(div);
    return div;
  });
  await mermaid.run({ nodes: targets }).catch(() => {});
  if (ctx.isCurrent()) enableDiagramZoom(root);
}

export class MarkdownView implements View {
  readonly accepts = "html" as const;

  private zoomLevel = 1;

  constructor(
    readonly el: HTMLElement,
    private readonly scroll: Scroll,
  ) {}

  async show(output: Output, ctx: RenderContext) {
    if (output.kind !== "html") return;
    this.el.innerHTML = sanitize(output.html);
    renderMath(this.el);
    highlightCode(this.el);
    addCopyButtons(this.el);
    ctx.restoreScroll();
    await renderDiagrams(this.el, ctx);
  }

  async zoom(direction: ZoomDirection, ctx: RenderContext) {
    this.zoomLevel = stepZoom(this.zoomLevel, direction);
    this.el.style.fontSize = `${BASE_FONT_PX * this.zoomLevel}px`;
    ctx.restoreScroll();
  }

  resetZoom() {
    this.zoomLevel = 1;
    this.el.style.fontSize = "";
  }

  async outline(): Promise<OutlineNode[]> {
    return headingOutline(this.el, this.scroll);
  }
}
