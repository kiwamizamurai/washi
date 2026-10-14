import { formatCount, matchStarts, stepTo, summarize } from "./search-count";
import type { SearchSource } from "./search-source";

type FindFn = (
  text: string,
  caseSensitive: boolean,
  backwards: boolean,
  wrap: boolean,
) => boolean;

export class FindBar {
  private readonly input: HTMLInputElement;
  private readonly count: HTMLElement | null;

  constructor(
    private readonly form: HTMLFormElement,
    private readonly source: SearchSource | null = null,
    private readonly find: FindFn = (text, caseSensitive, backwards, wrap) =>
      (window as unknown as { find: FindFn }).find(text, caseSensitive, backwards, wrap),
  ) {
    this.input = form.querySelector("input")!;
    this.count = form.querySelector<HTMLElement>(".count");
    this.input.addEventListener("input", () => this.showCount(false));
    form.addEventListener("submit", (e) => {
      e.preventDefault();
      this.next(false);
    });
    form.querySelector<HTMLElement>("[data-find=prev]")?.addEventListener("click", () => this.next(true));
    form.querySelector<HTMLElement>("[data-find=next]")?.addEventListener("click", () => this.next(false));
    form.querySelector<HTMLElement>("[data-find=close]")?.addEventListener("click", () => this.close());
    this.input.addEventListener("keydown", (e) => {
      if (e.key === "Escape") this.close();
      else if (e.key === "Enter" && e.shiftKey) {
        e.preventDefault();
        this.next(true);
      }
    });
  }

  get isOpen() {
    return !this.form.hidden;
  }

  open() {
    this.form.hidden = false;
    this.input.focus();
    this.input.select();
  }

  close() {
    this.form.hidden = true;
    this.showCount(false, true);
    getSelection()?.removeAllRanges();
  }

  next(backwards: boolean) {
    const text = this.input.value;
    if (!text) return false;
    const found = this.step(text, backwards) ?? this.find(text, false, backwards, true);
    this.form.classList.toggle("not-found", !found);
    if (found && !this.source) revealSelection();
    this.showCount(found);
    this.input.focus();
    return found;
  }

  private step(text: string, backwards: boolean): boolean | null {
    const source = this.source;
    if (!source) return null;
    const starts = matchStarts(source.text(), text);
    if (!starts) return null;
    const at = stepTo(starts, source.selectionStart(), backwards);
    return at !== null && source.select(at, text.length);
  }

  private showCount(jumped: boolean, clear = false) {
    if (!this.count) return;
    const query = this.input.value;
    if (clear || !query || !this.source) {
      this.count.textContent = "";
      if (!query) this.form.classList.remove("not-found");
      return;
    }
    const summary = summarize(this.source.text(), query, jumped ? this.source.selectionStart() : null);
    if (summary && !jumped) this.form.classList.toggle("not-found", summary.total === 0);
    this.count.textContent = formatCount(summary, this.source.partial());
  }
}

function revealSelection() {
  const node = getSelection()?.getRangeAt(0)?.startContainer;
  const target = node instanceof Element ? node : node?.parentElement;
  target?.scrollIntoView({ block: "center", behavior: "smooth" });
}
