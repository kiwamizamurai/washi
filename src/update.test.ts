import { describe, expect, it, vi } from "vitest";
import {
  CHECK_INTERVAL_MS,
  checkForUpdate,
  DEFAULT_UPDATE_STATE,
  isNewer,
  loadUpdateState,
  updateState,
  type Release,
} from "./update";

const memory = (initial?: string) => {
  let value = initial;
  return {
    getItem: () => value ?? null,
    setItem: (_: string, v: string) => void (value = v),
  };
};

const release = (latest: string, current = "2.0.0"): Release => ({
  current,
  latest,
  url: `https://github.com/kiwamizamurai/washi/releases/tag/v${latest}`,
});

const DAY = CHECK_INTERVAL_MS;

describe("isNewer", () => {
  it("compares each number, not the text", () => {
    expect(isNewer("2.0.1", "2.0.0")).toBe(true);
    expect(isNewer("2.10.0", "2.9.9")).toBe(true);
    expect(isNewer("10.0.0", "9.0.0")).toBe(true);
    expect(isNewer("v3.0.0", "2.0.0")).toBe(true);
  });

  it("is false for the same or an older version and for anything that is not x.y.z", () => {
    expect(isNewer("2.0.0", "2.0.0")).toBe(false);
    expect(isNewer("1.9.9", "2.0.0")).toBe(false);
    expect(isNewer("3.0.0-beta.1", "2.0.0")).toBe(false);
    expect(isNewer("nightly", "2.0.0")).toBe(false);
    expect(isNewer("3.0.0", "dev")).toBe(false);
  });
});

describe("update state", () => {
  it("starts with automatic checks on", () => {
    expect(loadUpdateState(memory())).toEqual(DEFAULT_UPDATE_STATE);
    expect(loadUpdateState(null)).toEqual(DEFAULT_UPDATE_STATE);
  });

  it("drops broken values field by field", () => {
    const store = memory(JSON.stringify({ lastCheck: "soon", automatic: "no" }));
    expect(loadUpdateState(store)).toEqual(DEFAULT_UPDATE_STATE);
    expect(loadUpdateState(memory("{not json"))).toEqual(DEFAULT_UPDATE_STATE);
    expect(loadUpdateState(memory("null"))).toEqual(DEFAULT_UPDATE_STATE);
  });

  it("merges a patch into what is stored", () => {
    const store = memory();
    updateState({ lastCheck: 7 }, store);
    updateState({ automatic: false }, store);
    expect(loadUpdateState(store)).toEqual({ lastCheck: 7, automatic: false });
  });
});

describe("checkForUpdate", () => {
  it("reports a newer release and remembers when it looked", async () => {
    const store = memory();
    const outcome = await checkForUpdate({ fetchLatest: async () => release("2.1.0"), manual: false, now: 5 * DAY, store });
    expect(outcome).toEqual({ kind: "available", release: release("2.1.0") });
    expect(loadUpdateState(store).lastCheck).toBe(5 * DAY);
  });

  it("asks GitHub at most once a day on its own, so other windows and tabs stay quiet", async () => {
    const store = memory();
    const fetchLatest = vi.fn(async () => release("2.1.0"));
    await checkForUpdate({ fetchLatest, manual: false, now: 5 * DAY, store });
    expect(await checkForUpdate({ fetchLatest, manual: false, now: 5 * DAY + 1000, store })).toEqual({ kind: "not-due" });
    expect(await checkForUpdate({ fetchLatest, manual: false, now: 6 * DAY - 1, store })).toEqual({ kind: "not-due" });
    expect((await checkForUpdate({ fetchLatest, manual: false, now: 6 * DAY, store })).kind).toBe("available");
    expect(fetchLatest).toHaveBeenCalledTimes(2);
  });

  it("records the time before asking, so a window opened during the request does not ask again", async () => {
    const store = memory();
    let finish!: (r: Release) => void;
    const slow = checkForUpdate({ fetchLatest: () => new Promise((r) => (finish = r)), manual: false, now: DAY, store });
    expect(await checkForUpdate({ fetchLatest: async () => release("2.1.0"), manual: false, now: DAY + 10, store })).toEqual({ kind: "not-due" });
    finish(release("2.1.0"));
    expect((await slow).kind).toBe("available");
  });

  it("checks again when the clock went backwards", async () => {
    const store = memory(JSON.stringify({ lastCheck: 10 * DAY }));
    expect((await checkForUpdate({ fetchLatest: async () => release("2.1.0"), manual: false, now: 2 * DAY, store })).kind).toBe("available");
  });


  it("does not ask on its own when automatic checks are off, but a manual check still works", async () => {
    const store = memory(JSON.stringify({ automatic: false }));
    const fetchLatest = vi.fn(async () => release("2.0.0"));
    expect(await checkForUpdate({ fetchLatest, manual: false, now: DAY, store })).toEqual({ kind: "off" });
    expect(fetchLatest).not.toHaveBeenCalled();
    expect(await checkForUpdate({ fetchLatest, manual: true, now: DAY, store })).toEqual({ kind: "current", release: release("2.0.0") });
  });

  it("turns an error into a failed outcome", async () => {
    const outcome = await checkForUpdate({
      fetchLatest: async () => Promise.reject("Could not reach GitHub: offline"),
      manual: true,
      now: DAY,
      store: memory(),
    });
    expect(outcome).toEqual({ kind: "failed", error: "Could not reach GitHub: offline" });
  });
});
