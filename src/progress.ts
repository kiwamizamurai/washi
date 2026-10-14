export function progressOf(scrollTop: number, scrollHeight: number, clientHeight: number): number | null {
  const max = scrollHeight - clientHeight;
  if (!(max > 1)) return null;
  return Math.min(1, Math.max(0, scrollTop / max));
}

export class ReadingProgress {
  private frame = 0;

  constructor(
    private readonly bar: HTMLElement,
    private readonly scroller: HTMLElement,
    watch: HTMLElement[] = [],
  ) {
    scroller.addEventListener("scroll", () => this.schedule(), { passive: true });
    const observer = new ResizeObserver(() => this.schedule());
    observer.observe(scroller);
    for (const el of watch) observer.observe(el);
    this.schedule();
  }

  private schedule() {
    if (this.frame) return;
    this.frame = requestAnimationFrame(() => {
      this.frame = 0;
      this.update();
    });
  }

  update() {
    const { scrollTop, scrollHeight, clientHeight } = this.scroller;
    const progress = progressOf(scrollTop, scrollHeight, clientHeight);
    this.bar.hidden = progress === null;
    if (progress !== null) this.bar.style.setProperty("--progress", String(progress));
  }
}
