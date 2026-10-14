import { describe, expect, it } from "vitest";
import { Scheduler, type Timers } from "./scheduler";

class FakeClock implements Timers {
  time = 1000;
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
  advance(ms: number) {
    const end = this.time + ms;
    for (;;) {
      const due = [...this.jobs.entries()].filter(([, j]) => j.at <= end).sort((a, b) => a[1].at - b[1].at)[0];
      if (!due) break;
      this.jobs.delete(due[0]);
      this.time = Math.max(this.time, due[1].at);
      due[1].fn();
    }
    this.time = end;
  }
  get waiting() {
    return this.jobs.size;
  }
}

const settle = () => new Promise((r) => setTimeout(r, 0));

function make(strategy: "throttle" | "debounce", delay: number, duration = 0) {
  const clock = new FakeClock();
  const runs: number[] = [];
  let release: (() => void) | undefined;
  const scheduler = new Scheduler({
    strategy,
    delay,
    timers: clock,
    run: () => {
      runs.push(clock.time);
      if (duration === 0) return Promise.resolve();
      return new Promise<void>((resolve) => {
        release = resolve;
      });
    },
  });
  return { clock, runs, scheduler, finish: () => release?.() };
}

describe("Scheduler (throttle)", () => {
  it("runs the first request immediately", async () => {
    const t = make("throttle", 150);
    t.scheduler.trigger();
    expect(t.runs).toEqual([1000]);
  });

  it("collapses a burst into the first run and one trailing run with the latest state", async () => {
    const t = make("throttle", 150);
    t.scheduler.trigger();
    await settle();
    for (let i = 0; i < 5; i++) {
      t.clock.advance(20);
      t.scheduler.trigger();
    }
    expect(t.runs).toEqual([1000]);
    t.clock.advance(150);
    await settle();
    expect(t.runs).toEqual([1000, 1150]);
  });

  it("does not run twice at once: a request during a run waits for it and then runs once", async () => {
    const t = make("throttle", 100, 1);
    t.scheduler.trigger();
    t.clock.advance(300);
    t.scheduler.trigger();
    t.scheduler.trigger();
    t.clock.advance(300);
    expect(t.runs).toHaveLength(1);
    t.finish();
    await settle();
    expect(t.runs).toHaveLength(2);
    t.finish();
    await settle();
    expect(t.runs).toHaveLength(2);
  });

  it("runs the next request immediately once the quiet period has passed", async () => {
    const t = make("throttle", 150);
    t.scheduler.trigger();
    await settle();
    t.clock.advance(500);
    t.scheduler.trigger();
    expect(t.runs).toEqual([1000, 1500]);
  });
});

describe("Scheduler (debounce)", () => {
  it("waits until the requests stop for the delay", async () => {
    const t = make("debounce", 800);
    t.scheduler.trigger();
    t.clock.advance(500);
    t.scheduler.trigger();
    t.clock.advance(500);
    expect(t.runs).toEqual([]);
    t.clock.advance(300);
    expect(t.runs).toEqual([2300]);
  });

  it("flush runs right away and cancels the wait", async () => {
    const t = make("debounce", 800);
    t.scheduler.trigger();
    t.scheduler.flush();
    expect(t.runs).toEqual([1000]);
    expect(t.clock.waiting).toBe(0);
    t.clock.advance(2000);
    expect(t.runs).toHaveLength(1);
  });

  it("cancel drops a waiting run", async () => {
    const t = make("debounce", 800);
    t.scheduler.trigger();
    t.scheduler.cancel();
    t.clock.advance(2000);
    expect(t.runs).toEqual([]);
  });

  it("keeps going after a run that fails", async () => {
    const clock = new FakeClock();
    let calls = 0;
    const scheduler = new Scheduler({
      strategy: "debounce",
      delay: 10,
      timers: clock,
      run: async () => {
        calls++;
        throw new Error("boom");
      },
    });
    scheduler.trigger();
    clock.advance(10);
    await settle();
    scheduler.trigger();
    clock.advance(10);
    await settle();
    expect(calls).toBe(2);
  });
});
