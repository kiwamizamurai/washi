import { beforeEach, describe, expect, it } from "vitest";
import type { BufferResult, Diagnostic, Output } from "./api";
import type { Scroll } from "./scroll";
import { Viewer, type Host } from "./viewer";
import type { RenderContext, View, ZoomDirection } from "./views/view";

const el = () => ({ hidden: true, textContent: "" }) as unknown as HTMLElement;

class FakeScroll {
  position = 0;
  width = 800;
  ratio() {
    return this.position;
  }
  restore(r: number) {
    this.position = r;
  }
}

class FakeView implements View {
  shown: string[] = [];
  zooms: ZoomDirection[] = [];
  el = el();
  constructor(readonly accepts: Output["kind"]) {}
  async show(output: Output, ctx: RenderContext) {
    this.shown.push(output.html);
    ctx.restoreScroll();
  }
  async zoom(direction: ZoomDirection) {
    this.zooms.push(direction);
  }
}

type Deferred = { resolve: (o: Output) => void };

const defaultBuffers = async (_path: string, text: string): Promise<BufferResult> => ({
  ok: true,
  output: { kind: "html", html: `buffer:${text}` },
  diagnostics: [],
});
let buffers: (path: string, text: string) => Promise<BufferResult> = defaultBuffers;

function setup() {
  const pending = new Map<string, Deferred>();
  const titles: string[] = [];
  const host: Host = {
    render: (path) =>
      new Promise((resolve) => pending.set(path, { resolve })),
    renderBuffer: async (path, text) => buffers(path, text),
    renderText: async (text) => ({ kind: "html", html: `text:${text}` }),
    opened: async (path) => void titles.push(path),
    pasted: async () => void titles.push("pasted"),
  };
  const scroll = new FakeScroll();
  const chrome = { empty: el(), error: el() };
  const html = new FakeView("html");
  const viewer = new Viewer(host, scroll as unknown as Scroll, chrome, [html]);
  return { viewer, scroll, chrome, html, pending, titles };
}

const flush = () => new Promise((r) => setTimeout(r, 0));

describe("Viewer", () => {
  it("shows the view that accepts the output", async () => {
    const t = setup();
    const done = t.viewer.load("/a.md");
    await flush();
    t.pending.get("/a.md")!.resolve({ kind: "html", html: "A" });
    await done;
    expect(t.html.shown).toEqual(["A"]);
    expect(t.html.el.hidden).toBe(false);
    expect(t.chrome.empty.hidden).toBe(true);
    expect(t.chrome.error.hidden).toBe(true);
    expect(t.titles).toEqual(["/a.md"]);
  });

  it("discards a stale render when a newer one finishes first", async () => {
    const t = setup();
    const first = t.viewer.load("/slow.md");
    await flush();
    const second = t.viewer.load("/fast.md");
    await flush();
    t.pending.get("/fast.md")!.resolve({ kind: "html", html: "fast" });
    await second;
    t.pending.get("/slow.md")!.resolve({ kind: "html", html: "slow" });
    await first;
    expect(t.html.shown).toEqual(["fast"]);
  });

  it("keeps the scroll position when the same view re-renders", async () => {
    const t = setup();
    const load = t.viewer.load("/a.md");
    await flush();
    t.pending.get("/a.md")!.resolve({ kind: "html", html: "1" });
    await load;
    t.scroll.position = 0.6;

    const again = t.viewer.reload();
    await flush();
    t.pending.get("/a.md")!.resolve({ kind: "html", html: "2" });
    await again;
    expect(t.scroll.position).toBe(0.6);
  });

  it("shows errors and clears them on the next success", async () => {
    const failing: Host = {
      render: async () => {
        throw new Error("boom");
      },
      renderBuffer: defaultBuffers,
      renderText: async () => ({ kind: "html", html: "ok" }),
      opened: async () => {},
      pasted: async () => {},
    };
    const chrome = { empty: el(), error: el() };
    const viewer = new Viewer(failing, new FakeScroll() as unknown as Scroll, chrome, [new FakeView("html")]);
    await viewer.load("/x.md");
    expect(chrome.error.hidden).toBe(false);
    expect(chrome.error.textContent).toContain("boom");

    await viewer.showText("fine");
    expect(chrome.error.hidden).toBe(true);
  });

  it("renders pasted text and has no current path", async () => {
    const t = setup();
    await t.viewer.showText("hello");
    expect(t.html.shown).toEqual(["text:hello"]);
    expect(t.viewer.currentPath).toBeNull();
    expect(t.titles).toEqual(["pasted"]);
  });

  it("forwards zoom to the active view only", async () => {
    const t = setup();
    await t.viewer.zoom("in");
    expect(t.html.zooms).toEqual([]);
    await t.viewer.showText("x");
    await t.viewer.zoom("in");
    expect(t.html.zooms).toEqual(["in"]);
  });

  describe("a buffer being edited", () => {
    beforeEach(() => {
      buffers = defaultBuffers;
    });

    const ok = (html: string, diagnostics: Diagnostic[] = []): BufferResult => ({
      ok: true,
      output: { kind: "html", html },
      diagnostics,
    });
    const warning: Diagnostic = {
      file: null,
      line: 1,
      column: 1,
      endLine: 1,
      endColumn: 2,
      severity: "warning",
      message: "w",
      hints: [],
    };

    it("renders the text it is given and reports the path as the current one", async () => {
      const t = setup();
      buffers = async (path, text) => ok(`${path}|${text}`);
      await t.viewer.renderBuffer("/a.md", "# draft");
      expect(t.html.shown).toEqual(["/a.md|# draft"]);
      expect(t.viewer.currentPath).toBe("/a.md");
    });

    it("does not reset the zoom or reopen the file", async () => {
      const t = setup();
      await t.viewer.renderBuffer("/a.md", "x");
      expect(t.titles).toEqual([]);
    });

    it("sends the diagnostics to the hooks on every render", async () => {
      const t = setup();
      const seen: Diagnostic[][] = [];
      const failures: (string | null)[] = [];
      t.viewer.setBufferHooks({ diagnostics: (d) => void seen.push(d), failed: (m) => void failures.push(m) });
      buffers = async () => ok("a", [warning]);
      await t.viewer.renderBuffer("/a.typ", "x");
      expect(seen).toEqual([[warning]]);
      expect(failures).toEqual([null]);
    });

    it("keeps the last good preview when a render fails, and reports the failure instead of an error page", async () => {
      const t = setup();
      const failures: (string | null)[] = [];
      t.viewer.setBufferHooks({ diagnostics: () => {}, failed: (m) => void failures.push(m) });
      buffers = async () => ok("good");
      await t.viewer.renderBuffer("/a.typ", "ok");
      buffers = async () => ({ ok: false, message: "unclosed delimiter", diagnostics: [warning] });
      await t.viewer.renderBuffer("/a.typ", "#let x = (");
      expect(t.html.shown).toEqual(["good"]);
      expect(t.chrome.error.hidden).toBe(true);
      expect(failures).toEqual([null, "unclosed delimiter"]);
    });

    it("discards a stale buffer render that finishes after a newer one", async () => {
      const t = setup();
      const resolvers: ((r: BufferResult) => void)[] = [];
      buffers = () => new Promise((resolve) => void resolvers.push(resolve));
      const first = t.viewer.renderBuffer("/a.md", "one");
      await flush();
      const second = t.viewer.renderBuffer("/a.md", "two");
      await flush();
      resolvers[1](ok("two"));
      await second;
      resolvers[0](ok("one"));
      await first;
      expect(t.html.shown).toEqual(["two"]);
    });

    it("leaving the buffer shows the file on disk again and clears the diagnostics", async () => {
      const t = setup();
      const seen: Diagnostic[][] = [];
      t.viewer.setBufferHooks({ diagnostics: (d) => void seen.push(d), failed: () => {} });
      await t.viewer.renderBuffer("/a.md", "draft");
      const leaving = t.viewer.leaveBuffer();
      await flush();
      t.pending.get("/a.md")!.resolve({ kind: "html", html: "on disk" });
      await leaving;
      expect(t.html.shown[t.html.shown.length - 1]).toBe("on disk");
      expect(seen[seen.length - 1]).toEqual([]);
    });

    it("leaveBuffer does nothing when no buffer is being edited", async () => {
      const t = setup();
      await t.viewer.leaveBuffer();
      expect(t.html.shown).toEqual([]);
    });
  });
});
