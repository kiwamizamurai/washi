import { describe, expect, it } from "vitest";
import type { SaveResult } from "../api";
import type { Disk } from "./buffer";
import type { Timers } from "./scheduler";
import { AUTOSAVE_IDLE_MS, AUTOSAVE_MAX_MS, EditSession, type Banner, type SessionHost } from "./session";

class FakeClock implements Timers {
  time = 0;
  private next = 1;
  private jobs = new Map<number, { at: number; fn: () => void }>();
  now() {
    return this.time;
  }
  set(fn: () => void, ms: number) {
    const id = this.next++;
    this.jobs.set(id, { at: this.time + ms, fn });
    return id;
  }
  clear(handle: unknown) {
    this.jobs.delete(handle as number);
  }
  async advance(ms: number) {
    const end = this.time + ms;
    for (;;) {
      const due = [...this.jobs.entries()].filter(([, j]) => j.at <= end).sort((a, b) => a[1].at - b[1].at)[0];
      if (!due) break;
      this.jobs.delete(due[0]);
      this.time = Math.max(this.time, due[1].at);
      due[1].fn();
      await settle();
    }
    this.time = end;
    await settle();
  }
}

const settle = () => new Promise((r) => setTimeout(r, 0));
const disk = (text: string): Disk => ({ text, hash: `h:${text}` });

interface Setup {
  session: EditSession;
  host: SessionHost;
  clock: FakeClock;
  rendered: string[];
  writes: { text: string; base: string | null; force: boolean }[];
  banners: Banner[];
  dirty: boolean[];
  texts: string[];
  notes: string[];
  onDisk: { current: Disk | null };
  nextWrite: { current: () => SaveResult | Promise<SaveResult> };
}

function setup(opts: { kind?: "markdown" | "latex"; autosave?: boolean; initial?: string } = {}): Setup {
  const clock = new FakeClock();
  const rendered: string[] = [];
  const writes: Setup["writes"] = [];
  const banners: Banner[] = [];
  const dirty: boolean[] = [];
  const texts: string[] = [];
  const notes: string[] = [];
  const onDisk = { current: disk(opts.initial ?? "base") as Disk | null };
  const nextWrite: Setup["nextWrite"] = { current: () => ({ status: "saved", hash: "unset" }) };
  const host: SessionHost = {
    readText: async () => {
      if (!onDisk.current) throw new Error("missing");
      return onDisk.current;
    },
    writeFile: async (_path, text, base, force) => {
      writes.push({ text, base, force });
      const result = await nextWrite.current();
      if (result.status === "saved") onDisk.current = { text, hash: result.hash };
      return result;
    },
    render: async (_path, text) => void rendered.push(text),
    setText: (text) => void texts.push(text),
    dirtyChanged: (d) => void dirty.push(d),
    banner: (b) => void banners.push(b),
    notify: (m) => void notes.push(m),
  };
  nextWrite.current = () => ({ status: "saved", hash: "h:saved" });
  const session = new EditSession(host, "/p/a.md", disk(opts.initial ?? "base"), {
    kind: opts.kind ?? "markdown",
    autosave: () => opts.autosave ?? false,
    timers: clock,
  });
  return { session, host, clock, rendered, writes, banners, dirty, texts, notes, onDisk, nextWrite };
}

describe("EditSession: rendering", () => {
  it("renders the current text when editing starts", async () => {
    const t = setup();
    t.session.start();
    await settle();
    expect(t.rendered).toEqual(["base"]);
  });

  it("renders the latest text after an edit (throttled for markdown)", async () => {
    const t = setup();
    t.session.start();
    await settle();
    t.session.edit("one");
    await settle();
    t.session.edit("two");
    t.session.edit("three");
    await t.clock.advance(200);
    expect(t.rendered[t.rendered.length - 1]).toBe("three");
  });

  it("renders LaTeX at most every half second while typing, and at once on save", async () => {
    const t = setup({ kind: "latex" });
    t.session.edit("\\section{A}");
    await t.clock.advance(100);
    t.session.edit("\\section{AB}");
    t.session.edit("\\section{ABC}");
    expect(t.rendered.length).toBeLessThanOrEqual(1);
    await t.clock.advance(600);
    expect(t.rendered[t.rendered.length - 1]).toBe("\\section{ABC}");
    t.session.edit("\\section{B}");
    await t.session.save();
    expect(t.rendered[t.rendered.length - 1]).toBe("\\section{B}");
  });

  it("ignores an edit that does not change the text", async () => {
    const t = setup();
    t.session.edit("base");
    await t.clock.advance(1000);
    expect(t.rendered).toEqual([]);
    expect(t.dirty).toEqual([]);
  });
});

describe("EditSession: dirty state", () => {
  it("reports changes of the dirty state only when it flips", () => {
    const t = setup();
    t.session.edit("a");
    t.session.edit("ab");
    t.session.edit("base");
    t.session.edit("x");
    expect(t.dirty).toEqual([true, false, true]);
  });
});

describe("EditSession: saving", () => {
  it("writes the text with the base hash, then is clean and renders", async () => {
    const t = setup();
    t.session.edit("new");
    expect(await t.session.save()).toBe("saved");
    expect(t.writes).toEqual([{ text: "new", base: "h:base", force: false }]);
    expect(t.session.dirty).toBe(false);
    expect(t.dirty).toEqual([true, false]);
    expect(t.session.buffer.baseHash).toBe("h:saved");
  });

  it("does nothing when there is nothing to save", async () => {
    const t = setup();
    expect(await t.session.save()).toBe("clean");
    expect(t.writes).toEqual([]);
  });

  it("stays dirty when more was typed while the save was in flight", async () => {
    const t = setup();
    let release: (r: SaveResult) => void = () => {};
    t.nextWrite.current = () => new Promise<SaveResult>((resolve) => (release = resolve));
    t.session.edit("first");
    const saving = t.session.save();
    await settle();
    t.session.edit("first and more");
    release({ status: "saved", hash: "h:first" });
    expect(await saving).toBe("saved");
    expect(t.session.dirty).toBe(true);
    expect(t.session.buffer.baseHash).toBe("h:first");
  });

  it("queues a second save behind the first", async () => {
    const t = setup();
    const order: string[] = [];
    t.nextWrite.current = async () => {
      order.push("write");
      await settle();
      return { status: "saved", hash: "h:x" };
    };
    t.session.edit("a");
    const first = t.session.save();
    const second = t.session.save();
    expect(await Promise.all([first, second])).toEqual(["saved", "clean"]);
    expect(order).toEqual(["write"]);
  });

  it("shows a conflict instead of overwriting, and keeps the buffer dirty", async () => {
    const t = setup();
    t.nextWrite.current = () => ({ status: "conflict", disk_hash: "h:theirs" });
    t.onDisk.current = disk("theirs");
    t.session.edit("mine");
    expect(await t.session.save()).toBe("conflict");
    expect(t.banners).toEqual([{ kind: "conflict" }]);
    expect(t.session.dirty).toBe(true);
  });

  it("'keep mine' rebases, so the next save overwrites on purpose", async () => {
    const t = setup();
    t.nextWrite.current = () => ({ status: "conflict", disk_hash: "h:theirs" });
    t.onDisk.current = disk("theirs");
    t.session.edit("mine");
    await t.session.save();
    t.nextWrite.current = () => ({ status: "saved", hash: "h:mine" });
    t.session.keepMine();
    expect(t.banners[t.banners.length - 1]).toBeNull();
    expect(await t.session.save()).toBe("saved");
    expect(t.writes[t.writes.length - 1]).toEqual({ text: "mine", base: "h:theirs", force: false });
  });

  it("'load the disk version' replaces the text and drops my edits", async () => {
    const t = setup();
    t.nextWrite.current = () => ({ status: "conflict", disk_hash: "h:theirs" });
    t.onDisk.current = disk("theirs");
    t.session.edit("mine");
    await t.session.save();
    await t.session.loadDisk();
    expect(t.texts).toEqual(["theirs"]);
    expect(t.session.dirty).toBe(false);
    expect(t.session.text).toBe("theirs");
    expect(t.banners[t.banners.length - 1]).toBeNull();
  });

  it("reports a failed write with a banner and a notification", async () => {
    const t = setup();
    t.nextWrite.current = () => Promise.reject(new Error("disk full"));
    t.session.edit("x");
    expect(await t.session.save()).toBe("failed");
    expect(t.banners[0]).toEqual({ kind: "error", message: "Error: disk full" });
    expect(t.notes).toEqual(["Error: disk full"]);
    expect(t.session.dirty).toBe(true);
  });
});

describe("EditSession: the file changing on disk", () => {
  it("ignores the notification caused by our own save", async () => {
    const t = setup();
    t.session.edit("new");
    await t.session.save();
    await t.session.diskChanged();
    expect(t.texts).toEqual([]);
    expect(t.banners.filter(Boolean)).toEqual([]);
  });

  it("quietly reloads a clean buffer", async () => {
    const t = setup();
    t.onDisk.current = disk("edited by an agent");
    await t.session.diskChanged();
    expect(t.texts).toEqual(["edited by an agent"]);
    expect(t.session.text).toBe("edited by an agent");
  });

  it("raises a conflict when the buffer has unsaved edits", async () => {
    const t = setup();
    t.session.edit("mine");
    t.onDisk.current = disk("theirs");
    await t.session.diskChanged();
    expect(t.banners).toEqual([{ kind: "conflict" }]);
    expect(t.texts).toEqual([]);
    expect(t.session.text).toBe("mine");
  });

  it("tells the caller what happened, so it can refresh the preview when the text is unchanged", async () => {
    const t = setup();
    expect(await t.session.diskChanged()).toBe("ignore");
    t.onDisk.current = disk("edited");
    expect(await t.session.diskChanged()).toBe("reload");
    t.session.edit("mine");
    t.onDisk.current = disk("theirs");
    expect(await t.session.diskChanged()).toBe("conflict");
    t.onDisk.current = null;
    expect(await t.session.diskChanged()).toBe("unreadable");
  });

  it("refreshPreview renders the current text again", async () => {
    const t = setup();
    t.session.edit("draft");
    await settle();
    t.rendered.length = 0;
    t.session.refreshPreview();
    await settle();
    expect(t.rendered).toEqual(["draft"]);
  });

  it("does nothing when the file cannot be read (deleted)", async () => {
    const t = setup();
    t.onDisk.current = null;
    await t.session.diskChanged();
    expect(t.texts).toEqual([]);
    expect(t.banners).toEqual([]);
  });
});

describe("EditSession: auto-save", () => {
  it("saves after typing pauses for the idle time", async () => {
    const t = setup({ autosave: true });
    t.session.edit("a");
    await t.clock.advance(AUTOSAVE_IDLE_MS - 1);
    expect(t.writes).toEqual([]);
    await t.clock.advance(2);
    expect(t.writes.map((w) => w.text)).toEqual(["a"]);
  });

  it("does not wait forever while typing continues: it saves at the maximum", async () => {
    const t = setup({ autosave: true });
    for (let i = 0; i < 12; i++) {
      t.session.edit(`text ${i}`);
      await t.clock.advance(600);
    }
    expect(t.writes.length).toBeGreaterThanOrEqual(1);
  });

  it("never saves on its own when auto-save is off", async () => {
    const t = setup({ autosave: false });
    t.session.edit("a");
    await t.clock.advance(AUTOSAVE_MAX_MS * 2);
    expect(t.writes).toEqual([]);
  });

  it("does not overwrite during a conflict", async () => {
    const t = setup({ autosave: true });
    t.nextWrite.current = () => ({ status: "conflict", disk_hash: "h:theirs" });
    t.onDisk.current = disk("theirs");
    t.session.edit("mine");
    await t.clock.advance(AUTOSAVE_IDLE_MS + 10);
    expect(t.writes).toHaveLength(1);
    t.session.edit("mine, more");
    await t.clock.advance(AUTOSAVE_MAX_MS * 2);
    expect(t.writes, "衝突している間は、自動保存で上書きを試みない").toHaveLength(1);
  });

  it("dispose cancels the pending auto-save", async () => {
    const t = setup({ autosave: true });
    t.session.edit("a");
    t.session.dispose();
    await t.clock.advance(AUTOSAVE_MAX_MS * 2);
    expect(t.writes).toEqual([]);
  });
});
