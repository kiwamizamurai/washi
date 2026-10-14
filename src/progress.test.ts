import { describe, expect, it } from "vitest";
import { progressOf } from "./progress";

describe("progressOf", () => {
  it("is the share of the scrollable distance already scrolled", () => {
    expect(progressOf(0, 2000, 1000)).toBe(0);
    expect(progressOf(500, 2000, 1000)).toBe(0.5);
    expect(progressOf(1000, 2000, 1000)).toBe(1);
  });

  it("stays within 0 and 1 (overscroll)", () => {
    expect(progressOf(-30, 2000, 1000)).toBe(0);
    expect(progressOf(1200, 2000, 1000)).toBe(1);
  });

  it("is null when there is nothing to scroll", () => {
    expect(progressOf(0, 800, 800)).toBeNull();
    expect(progressOf(0, 600, 800)).toBeNull();
    expect(progressOf(0, 801, 800)).toBeNull();
  });
});
