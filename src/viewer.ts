import type { BufferResult, Diagnostic, Output } from "./api";
import type { OutlineSink } from "./outline";
import type { Scroll } from "./scroll";
import type { RenderContext, View, ZoomDirection } from "./views/view";

export interface Host {
  render(path: string): Promise<Output>;
  renderBuffer(path: string, text: string): Promise<BufferResult>;
  renderText(text: string): Promise<Output>;
  opened(path: string): Promise<void>;
  pasted(): Promise<void>;
}

export interface Chrome {
  empty: HTMLElement;
  error: HTMLElement;
}

export interface BufferHooks {
  diagnostics(list: Diagnostic[]): void;
  failed(message: string | null): void;
}

const NO_HOOKS: BufferHooks = { diagnostics: () => {}, failed: () => {} };

type Source = { file: string } | { text: string } | { buffer: { path: string; text: string } };

export class Viewer {
  private source: Source | null = null;
  private active: View | null = null;
  private token = 0;
  private hooks: BufferHooks = NO_HOOKS;

  constructor(
    private readonly host: Host,
    private readonly scroll: Scroll,
    private readonly chrome: Chrome,
    private readonly views: View[],
    private readonly outline: OutlineSink = { set: () => {} },
  ) {}

  get currentPath() {
    if (!this.source) return null;
    if ("file" in this.source) return this.source.file;
    return "buffer" in this.source ? this.source.buffer.path : null;
  }

  setBufferHooks(hooks: BufferHooks | null) {
    this.hooks = hooks ?? NO_HOOKS;
  }

  async renderBuffer(path: string, text: string) {
    this.source = { buffer: { path, text } };
    await this.reload();
  }

  async leaveBuffer() {
    if (!this.source || !("buffer" in this.source)) return;
    this.source = { file: this.source.buffer.path };
    this.hooks.diagnostics([]);
    this.hooks.failed(null);
    await this.reload();
  }

  async load(path: string) {
    this.source = { file: path };
    this.resetZoom();
    try {
      await this.host.opened(path);
    } catch (e) {
      this.showError(String(e));
    }
    await this.reload();
  }

  async showText(text: string) {
    this.source = { text };
    this.resetZoom();
    await this.host.pasted();
    await this.reload();
  }

  async reload() {
    const source = this.source;
    if (!source) return;
    const mine = ++this.token;
    try {
      let output: Output;
      if ("buffer" in source) {
        const result = await this.host.renderBuffer(source.buffer.path, source.buffer.text);
        if (mine !== this.token) return;
        this.hooks.diagnostics(result.diagnostics);
        if (!result.ok) {
          this.hooks.failed(result.message);
          return;
        }
        this.hooks.failed(null);
        output = result.output;
      } else {
        output =
          "file" in source
            ? await this.host.render(source.file)
            : await this.host.renderText(source.text);
        if (mine !== this.token) return;
      }
      const view = this.views.find((v) => v.accepts === output.kind);
      if (!view) throw new Error(`Cannot display this output: ${output.kind}`);

      const ratio = view === this.active ? this.scroll.ratio() : 0;
      this.activate(view);
      await view.show(output, this.context(mine, ratio));
      if (mine !== this.token) return;
      this.scroll.restore(ratio);
      this.showError(null);
      this.outline.set((await view.outline?.()) ?? []);
    } catch (e) {
      if (mine === this.token) this.showError(String(e));
    }
  }

  async relayout() {
    const view = this.active;
    if (!view?.relayout) return;
    await view.relayout(this.context(this.token, this.scroll.ratio()));
  }

  async zoom(direction: ZoomDirection) {
    const view = this.active;
    if (!view?.zoom) return;
    await view.zoom(direction, this.context(this.token, this.scroll.ratio()));
  }

  private resetZoom() {
    this.views.forEach((v) => v.resetZoom?.());
  }

  private context(mine: number, ratio: number): RenderContext {
    return {
      width: this.scroll.width,
      isCurrent: () => mine === this.token,
      restoreScroll: () => this.scroll.restore(ratio),
    };
  }

  private activate(view: View) {
    this.chrome.empty.hidden = true;
    for (const v of this.views) v.el.hidden = v !== view;
    this.active = view;
  }

  private showError(message: string | null) {
    this.chrome.error.hidden = message === null;
    this.chrome.error.textContent = message ?? "";
  }
}
