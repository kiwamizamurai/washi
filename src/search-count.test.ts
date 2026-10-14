import { describe, expect, it } from "vitest";
import { formatCount, matchStarts, stepTo, summarize } from "./search-count";

describe("matchStarts", () => {
  it("ignores case and does not overlap", () => {
    expect(matchStarts("Foo foo FOO", "foo")).toEqual([0, 4, 8]);
    expect(matchStarts("aaaa", "aa")).toEqual([0, 2]);
  });

  it("finds Japanese text", () => {
    expect(matchStarts("和紙を読む。和紙", "和紙")).toEqual([0, 6]);
  });

  it("is empty for an empty query and for no match", () => {
    expect(matchStarts("abc", "")).toEqual([]);
    expect(matchStarts("abc", "z")).toEqual([]);
  });

  it("gives up when lower-casing changes the length", () => {
    expect(matchStarts("İstanbul", "i")).toBeNull();
  });
});

describe("summarize", () => {
  it("numbers the match at the selection", () => {
    expect(summarize("a b a b a", "a", 4)).toEqual({ total: 3, index: 2 });
    expect(summarize("a b a b a", "a", 0)).toEqual({ total: 3, index: 1 });
  });

  it("has no index when nothing is selected or the selection is elsewhere", () => {
    expect(summarize("a b a", "a", null)).toEqual({ total: 2, index: null });
    expect(summarize("a b a", "a", 1)).toEqual({ total: 2, index: null });
  });
});

describe("formatCount", () => {
  it("formats 'n / total'", () => {
    expect(formatCount({ total: 12, index: 3 }, false)).toBe("3 / 12");
  });
  it("shows a dash before the first jump", () => {
    expect(formatCount({ total: 4, index: null }, false)).toBe("– / 4");
  });
  it("marks an incomplete count with a plus", () => {
    expect(formatCount({ total: 12, index: 3 }, true)).toBe("3 / 12+");
  });
  it("says so when there is nothing", () => {
    expect(formatCount({ total: 0, index: null }, false)).toBe("No results");
    expect(formatCount(null, false)).toBe("");
  });
});

describe("stepTo", () => {
  const starts = [2, 10, 30];

  it("starts from the first (or last) match when nothing is selected", () => {
    expect(stepTo(starts, null, false)).toBe(2);
    expect(stepTo(starts, null, true)).toBe(30);
  });

  it("goes to the next match after the selection, and the previous one before it", () => {
    expect(stepTo(starts, 2, false)).toBe(10);
    expect(stepTo(starts, 10, true)).toBe(2);
    expect(stepTo(starts, 15, false)).toBe(30);
    expect(stepTo(starts, 15, true)).toBe(10);
  });

  it("wraps around at both ends", () => {
    expect(stepTo(starts, 30, false)).toBe(2);
    expect(stepTo(starts, 2, true)).toBe(30);
  });

  it("is null when there are no matches", () => {
    expect(stepTo([], null, false)).toBeNull();
    expect(stepTo([], 5, true)).toBeNull();
  });
});
