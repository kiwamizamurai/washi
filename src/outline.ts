import type { Scroll } from "./scroll";

export interface OutlineNode {
  title: string;
  level: number;
  offset(): number;
  go(): void;
}

export interface OutlineSink {
  set(nodes: OutlineNode[]): void;
}

const SPY_MARGIN_PX = 48;

export function activeIndex(offsets: number[], scrollTop: number, atBottom = false) {
  if (atBottom && offsets.length > 0) return offsets.length - 1;
  let active = -1;
  offsets.forEach((offset, i) => {
    if (offset <= scrollTop + SPY_MARGIN_PX) active = i;
  });
  return active;
}

export function headingOutline(root: HTMLElement, scroll: Scroll): OutlineNode[] {
  return [...root.querySelectorAll<HTMLElement>("h1, h2, h3, h4")].map((heading) => ({
    title: heading.textContent?.trim() ?? "",
    level: Number(heading.tagName[1]),
    offset: () => scroll.offsetOf(heading),
    go: () => scroll.scrollTo(scroll.offsetOf(heading)),
  }));
}

export class OutlinePanel implements OutlineSink {
  private nodes: OutlineNode[] = [];
  private buttons: HTMLButtonElement[] = [];
  private pending = false;

  constructor(
    private readonly panel: HTMLElement,
    private readonly nav: HTMLElement,
    private readonly scroll: Scroll,
  ) {
    scroll.onScroll(() => this.scheduleSpy());
  }

  get visible() {
    return !this.panel.hidden;
  }

  get headings(): readonly OutlineNode[] {
    return this.nodes;
  }

  show(visible: boolean) {
    this.panel.hidden = !visible;
    if (visible) this.spy();
  }

  set(nodes: OutlineNode[]) {
    this.nodes = nodes;
    const minLevel = Math.min(...nodes.map((n) => n.level), 6);
    this.buttons = nodes.map((node) => {
      const button = document.createElement("button");
      button.type = "button";
      button.textContent = node.title;
      button.style.paddingLeft = `${0.75 + (node.level - minLevel) * 0.9}rem`;
      button.addEventListener("click", () => node.go());
      return button;
    });
    if (nodes.length === 0) {
      const empty = document.createElement("div");
      empty.className = "empty";
      empty.textContent = "No headings";
      this.nav.replaceChildren(empty);
    } else {
      this.nav.replaceChildren(...this.buttons);
    }
    this.spy();
  }

  private scheduleSpy() {
    if (this.pending || !this.visible) return;
    this.pending = true;
    requestAnimationFrame(() => {
      this.pending = false;
      this.spy();
    });
  }

  private spy() {
    if (!this.visible) return;
    const active = activeIndex(this.nodes.map((n) => n.offset()), this.scroll.top, this.scroll.atBottom);
    this.buttons.forEach((b, i) => b.classList.toggle("active", i === active));
    this.buttons[active]?.scrollIntoView({ block: "nearest" });
  }
}
