import { describe, expect, it } from "vitest";
import { clampPercent, percentFromPointer } from "./split";

describe("clampPercent", () => {
  it("keeps the editor between a quarter and three quarters of the window", () => {
    expect(clampPercent(50)).toBe(50);
    expect(clampPercent(5)).toBe(25);
    expect(clampPercent(99)).toBe(75);
  });

  it("falls back to half for nonsense", () => {
    expect(clampPercent(Number.NaN)).toBe(50);
    expect(clampPercent(Number.POSITIVE_INFINITY)).toBe(50);
  });
});

describe("percentFromPointer", () => {
  it("is the distance from the editor's left edge, as a share of the window", () => {
    expect(percentFromPointer(500, 0, 1000)).toBe(50);
    expect(percentFromPointer(756, 256, 1000)).toBe(50);
  });

  it("stays inside the allowed range", () => {
    expect(percentFromPointer(0, 0, 1000)).toBe(25);
    expect(percentFromPointer(990, 0, 1000)).toBe(75);
  });

  it("copes with a window of no width", () => {
    expect(percentFromPointer(10, 0, 0)).toBe(50);
  });
});
