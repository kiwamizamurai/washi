export interface Timers {
  now(): number;
  set(fn: () => void, ms: number): unknown;
  clear(handle: unknown): void;
}

const realTimers: Timers = {
  now: () => performance.now(),
  set: (fn, ms) => window.setTimeout(fn, ms),
  clear: (handle) => window.clearTimeout(handle as number),
};

export interface SchedulerOptions {
  strategy: "throttle" | "debounce";
  delay: number;
  run(): Promise<unknown>;
  timers?: Timers;
}

export class Scheduler {
  private timer: unknown;
  private running = false;
  private pending = false;
  private lastRun = Number.NEGATIVE_INFINITY;
  private readonly timers: Timers;

  constructor(private readonly options: SchedulerOptions) {
    this.timers = options.timers ?? realTimers;
  }

  trigger() {
    if (this.options.strategy === "debounce") {
      this.clearTimer();
      this.timer = this.timers.set(() => {
        this.timer = undefined;
        this.fire();
      }, this.options.delay);
      return;
    }
    const wait = this.lastRun + this.options.delay - this.timers.now();
    if (!this.running && wait <= 0 && this.timer === undefined) {
      this.fire();
      return;
    }
    this.pending = true;
    if (this.timer === undefined) {
      this.timer = this.timers.set(() => {
        this.timer = undefined;
        if (this.pending) {
          this.pending = false;
          this.fire();
        }
      }, Math.max(0, wait));
    }
  }

  flush() {
    this.clearTimer();
    this.pending = false;
    this.fire();
  }

  cancel() {
    this.clearTimer();
    this.pending = false;
  }

  private clearTimer() {
    if (this.timer !== undefined) this.timers.clear(this.timer);
    this.timer = undefined;
  }

  private fire() {
    if (this.running) {
      this.pending = true;
      return;
    }
    this.running = true;
    this.lastRun = this.timers.now();
    void Promise.resolve(this.options.run())
      .catch(() => {})
      .finally(() => {
        this.running = false;
        if (this.pending) {
          this.pending = false;
          this.trigger();
        }
      });
  }
}
