import { describe, expect, it } from "vitest";
import { clamp, stepZoom, ZOOM_RANGE } from "./zoom";

describe("zoom", () => {
  it("steps in and out multiplicatively", () => {
    expect(stepZoom(1, "in")).toBeCloseTo(1.1);
    expect(stepZoom(1.1, "out")).toBeCloseTo(1);
  });

  it("resets to 1", () => {
    expect(stepZoom(3, "reset")).toBe(1);
  });

  it("stays within range", () => {
    expect(stepZoom(ZOOM_RANGE.max, "in")).toBe(ZOOM_RANGE.max);
    expect(stepZoom(ZOOM_RANGE.min, "out")).toBe(ZOOM_RANGE.min);
    expect(clamp(5, 0, 3)).toBe(3);
  });
});
