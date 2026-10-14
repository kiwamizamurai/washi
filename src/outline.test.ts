import { describe, expect, it } from "vitest";
import { activeIndex } from "./outline";

describe("activeIndex", () => {
  const offsets = [0, 400, 900, 1500];

  it("is -1 before the first heading", () => {
    expect(activeIndex([200, 500], 0)).toBe(-1);
  });

  it("picks the last heading at or above the viewport top", () => {
    expect(activeIndex(offsets, 0)).toBe(0);
    expect(activeIndex(offsets, 399)).toBe(1);
    expect(activeIndex(offsets, 1000)).toBe(2);
    expect(activeIndex(offsets, 99999)).toBe(3);
  });

  it("selects the last heading when scrolled to the bottom", () => {
    expect(activeIndex(offsets, 1000, true)).toBe(3);
    expect(activeIndex([], 1000, true)).toBe(-1);
  });

  it("handles an empty outline", () => {
    expect(activeIndex([], 100)).toBe(-1);
  });
});
