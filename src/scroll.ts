const SCROLL_MARGIN_PX = 16;

export class Scroll {
  constructor(private readonly el: HTMLElement) {}

  get width() {
    return this.el.clientWidth;
  }

  get top() {
    return this.el.scrollTop;
  }

  get atBottom() {
    return this.el.scrollTop + this.el.clientHeight >= this.el.scrollHeight - 2;
  }

  ratio() {
    const max = this.el.scrollHeight - this.el.clientHeight;
    return max > 0 ? this.el.scrollTop / max : 0;
  }

  restore(ratio: number) {
    const max = this.el.scrollHeight - this.el.clientHeight;
    this.el.scrollTop = Math.max(0, max) * ratio;
  }

  offsetOf(target: HTMLElement) {
    return target.getBoundingClientRect().top - this.el.getBoundingClientRect().top + this.el.scrollTop;
  }

  scrollTo(offset: number) {
    this.el.scrollTo({ top: Math.max(0, offset - SCROLL_MARGIN_PX), behavior: "smooth" });
  }

  onScroll(listener: () => void) {
    this.el.addEventListener("scroll", listener, { passive: true });
  }
}
