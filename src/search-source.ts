export interface SearchSource {
  text(): string;
  selectionStart(): number | null;
  select(start: number, length: number): boolean;
  partial(): boolean;
}

const SKIPPED = new Set(["BUTTON", "SCRIPT", "STYLE", "TITLE", "DESC"]);

interface Piece {
  node: Text;
  start: number;
}

export function domSearchSource(root: HTMLElement): SearchSource {
  const collect = () => {
    const pieces: Piece[] = [];
    let text = "";
    const walk = (node: Node) => {
      for (const child of node.childNodes) {
        if (child instanceof Element) {
          if (child.hasAttribute("hidden") || SKIPPED.has(child.tagName.toUpperCase()) || child.classList.contains("katex-mathml")) continue;
          walk(child);
        } else if (child.nodeType === Node.TEXT_NODE) {
          pieces.push({ node: child as Text, start: text.length });
          text += (child as Text).data;
        }
      }
    };
    walk(root);
    return { text, pieces };
  };

  const locate = (pieces: Piece[], at: number, end: boolean) =>
    pieces.find(({ node, start }) => (end ? at > start && at <= start + node.length : at >= start && at < start + node.length));

  return {
    text: () => collect().text,
    selectionStart() {
      const selection = getSelection();
      const range = selection?.rangeCount ? selection.getRangeAt(0) : null;
      if (!range || range.collapsed) return null;
      const piece = collect().pieces.find((p) => p.node === range.startContainer);
      return piece ? piece.start + range.startOffset : null;
    },
    select(start, length) {
      const { pieces } = collect();
      const from = locate(pieces, start, false);
      const to = locate(pieces, start + length, true);
      if (!from || !to) return false;
      const range = document.createRange();
      range.setStart(from.node, start - from.start);
      range.setEnd(to.node, start + length - to.start);
      const selection = getSelection();
      selection?.removeAllRanges();
      selection?.addRange(range);
      const target = from.node.parentElement;
      target?.scrollIntoView({ block: "center", behavior: "smooth" });
      return true;
    },
    partial: () => root.querySelector(".textLayer:empty") !== null,
  };
}
