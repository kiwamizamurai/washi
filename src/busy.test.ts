import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createBusy } from "./busy";

function sink() {
  const log: string[] = [];
  return { log, show: (m: string) => log.push(`show:${m}`), hide: () => log.push("hide") };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("busy", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("stays silent when the work finishes quickly", async () => {
    const s = sink();
    const track = createBusy(s, 400);
    const work = deferred<number>();
    const done = track("Rendering a.md", work.promise);
    vi.advanceTimersByTime(399);
    work.resolve(1);
    expect(await done).toBe(1);
    vi.advanceTimersByTime(1000);
    expect(s.log).toEqual([]);
  });

  it("shows the message after the delay and hides it when the work ends", async () => {
    const s = sink();
    const track = createBusy(s, 400);
    const work = deferred<void>();
    const done = track("Rendering a.md", work.promise);
    vi.advanceTimersByTime(400);
    expect(s.log).toEqual(["show:Rendering a.md"]);
    work.resolve();
    await done;
    expect(s.log).toEqual(["show:Rendering a.md", "hide"]);
  });

  it("hides the message and rethrows when the work fails", async () => {
    const s = sink();
    const track = createBusy(s, 400);
    const work = deferred<void>();
    const done = track("Rendering a.md", work.promise);
    vi.advanceTimersByTime(400);
    work.reject(new Error("boom"));
    await expect(done).rejects.toThrow("boom");
    expect(s.log).toEqual(["show:Rendering a.md", "hide"]);
  });

  it("stays visible until every overlapping job has ended, showing the latest message", async () => {
    const s = sink();
    const track = createBusy(s, 400);
    const first = deferred<void>();
    const second = deferred<void>();
    const a = track("Rendering a.md", first.promise);
    vi.advanceTimersByTime(400);
    const b = track("Rendering b.md", second.promise);
    first.resolve();
    await a;
    expect(s.log).toEqual(["show:Rendering a.md", "show:Rendering b.md"]);
    second.resolve();
    await b;
    expect(s.log).toEqual(["show:Rendering a.md", "show:Rendering b.md", "hide"]);
  });

  it("can show again for a later job", async () => {
    const s = sink();
    const track = createBusy(s, 400);
    for (const name of ["a.md", "b.md"]) {
      const work = deferred<void>();
      const done = track(`Rendering ${name}`, work.promise);
      vi.advanceTimersByTime(400);
      work.resolve();
      await done;
    }
    expect(s.log).toEqual(["show:Rendering a.md", "hide", "show:Rendering b.md", "hide"]);
  });
});
