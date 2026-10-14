import { describe, expect, it } from "vitest";
import { MAX_RECENT, addRecent, clearRecent, describe as describePath, loadRecent, removeRecent } from "./recent";

const memory = (initial?: string) => {
  let value = initial;
  return {
    getItem: () => value ?? null,
    setItem: (_: string, v: string) => void (value = v),
  };
};

describe("recent files", () => {
  it("starts empty, and survives broken storage", () => {
    expect(loadRecent(memory())).toEqual([]);
    expect(loadRecent(memory("not json"))).toEqual([]);
    expect(loadRecent(memory('{"a":1}'))).toEqual([]);
    expect(loadRecent(memory('["/a.md", 3, "relative.md", null]'))).toEqual(["/a.md"]);
    expect(loadRecent(null)).toEqual([]);
  });

  it("puts the newest first and moves repeats to the front", () => {
    const store = memory();
    addRecent("/a.md", store);
    addRecent("/b.md", store);
    expect(addRecent("/a.md", store)).toEqual(["/a.md", "/b.md"]);
    expect(loadRecent(store)).toEqual(["/a.md", "/b.md"]);
  });

  it("keeps at most MAX_RECENT", () => {
    const store = memory();
    for (let i = 0; i < MAX_RECENT + 3; i++) addRecent(`/f${i}.md`, store);
    const list = loadRecent(store);
    expect(list).toHaveLength(MAX_RECENT);
    expect(list[0]).toBe(`/f${MAX_RECENT + 2}.md`);
  });

  it("removes and clears", () => {
    const store = memory();
    addRecent("/a.md", store);
    addRecent("/b.md", store);
    expect(removeRecent("/a.md", store)).toEqual(["/b.md"]);
    clearRecent(store);
    expect(loadRecent(store)).toEqual([]);
  });

  it("does not throw when storage fails", () => {
    const broken = { getItem: () => null, setItem: () => { throw new Error("full"); } };
    expect(() => addRecent("/a.md", broken)).not.toThrow();
  });
});

describe("describe", () => {
  it("shows the name and the last two folders", () => {
    expect(describePath("/Users/me/work/notes/plan.md")).toEqual({ name: "plan.md", folder: "…/work/notes" });
    expect(describePath("/Users/me/plan.md")).toEqual({ name: "plan.md", folder: "/Users/me" });
    expect(describePath("/plan.md")).toEqual({ name: "plan.md", folder: "/" });
  });
});
