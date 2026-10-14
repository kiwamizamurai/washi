import { describe, expect, it } from "vitest";
import { fuzzyMatch } from "./match";

describe("fuzzyMatch", () => {
  it("matches everything for an empty query", () => {
    expect(fuzzyMatch("", "Anything")).toEqual({ score: 0, indices: [] });
    expect(fuzzyMatch("   ", "Anything")).toEqual({ score: 0, indices: [] });
  });

  it("ignores case and reports where it matched", () => {
    expect(fuzzyMatch("zoom", "Zoom In")?.indices).toEqual([0, 1, 2, 3]);
    expect(fuzzyMatch("IN", "Zoom In")?.indices).toEqual([5, 6]);
  });

  it("matches letters that are not next to each other", () => {
    expect(fuzzyMatch("tgl", "Toggle Outline")?.indices).toEqual([0, 2, 4]);
    expect(fuzzyMatch("zzz", "Zoom In")).toBeNull();
  });

  it("needs every word, in any order", () => {
    expect(fuzzyMatch("out tog", "Toggle Outline")).not.toBeNull();
    expect(fuzzyMatch("out xyz", "Toggle Outline")).toBeNull();
  });

  it("ranks a whole-word hit above a scattered one", () => {
    const whole = fuzzyMatch("save", "Save")!;
    const scattered = fuzzyMatch("save", "Show a vivid even")!;
    expect(whole.score).toBeGreaterThan(scattered.score);
  });

  it("prefers a match at the start, and at a word start", () => {
    const start = fuzzyMatch("out", "Outline")!;
    const inside = fuzzyMatch("out", "Layout")!;
    const word = fuzzyMatch("out", "Toggle Outline")!;
    expect(start.score).toBeGreaterThan(word.score);
    expect(word.score).toBeGreaterThan(inside.score);
  });

  it("works with Japanese", () => {
    expect(fuzzyMatch("読書", "はじめに 読書 の進み")?.indices).toEqual([5, 6]);
  });
});
