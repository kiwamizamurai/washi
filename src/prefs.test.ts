import { describe, expect, it } from "vitest";
import { DEFAULT_PREFS, loadPrefs, savePrefs } from "./prefs";

const memory = (initial?: string) => {
  let value = initial;
  return {
    getItem: () => value ?? null,
    setItem: (_: string, v: string) => void (value = v),
  };
};

describe("prefs", () => {
  it("returns defaults when nothing is stored", () => {
    expect(loadPrefs(memory())).toEqual(DEFAULT_PREFS);
  });

  it("round-trips valid values", () => {
    const store = memory();
    const prefs = { theme: "dark", width: "wide", outline: true, autosave: true, syncCursor: false, editorWidth: 40, editorFontSize: 16 } as const;
    savePrefs(prefs, store);
    expect(loadPrefs(store)).toEqual(prefs);
  });

  it("drops invalid values field by field", () => {
    const store = memory(JSON.stringify({ theme: "neon", width: "full", outline: "yes" }));
    expect(loadPrefs(store)).toEqual({ ...DEFAULT_PREFS, width: "full" });
  });

  it("clamps the editor width and rejects non-numbers and non-booleans", () => {
    const wide = memory(JSON.stringify({ editorWidth: 400, autosave: "yes", syncCursor: 1 }));
    expect(loadPrefs(wide)).toEqual({ ...DEFAULT_PREFS, editorWidth: 75 });
    expect(loadPrefs(memory(JSON.stringify({ editorWidth: -3 }))).editorWidth).toBe(25);
    expect(loadPrefs(memory(JSON.stringify({ editorWidth: "50" }))).editorWidth).toBe(50);
    expect(loadPrefs(memory(JSON.stringify({ editorWidth: null }))).editorWidth).toBe(50);
  });

  it("survives corrupt JSON and missing or throwing storage", () => {
    expect(loadPrefs(memory("{not json"))).toEqual(DEFAULT_PREFS);
    expect(loadPrefs(null)).toEqual(DEFAULT_PREFS);
    const throwing = {
      getItem: () => {
        throw new Error("denied");
      },
      setItem: () => {
        throw new Error("denied");
      },
    };
    expect(loadPrefs(throwing)).toEqual(DEFAULT_PREFS);
    expect(() => savePrefs(DEFAULT_PREFS, throwing)).not.toThrow();
  });
});

  it("clamps the editor font size and rejects non-numbers", () => {
    const big = memory(JSON.stringify({ editorFontSize: 99 }));
    expect(loadPrefs(big).editorFontSize).toBe(24);
    expect(loadPrefs(memory(JSON.stringify({ editorFontSize: 1 }))).editorFontSize).toBe(10);
    expect(loadPrefs(memory(JSON.stringify({ editorFontSize: "14" }))).editorFontSize).toBe(14);
    expect(loadPrefs(memory(JSON.stringify({ editorFontSize: null }))).editorFontSize).toBe(14);
  });

  it("rounds fractional editor font sizes", () => {
    expect(loadPrefs(memory(JSON.stringify({ editorFontSize: 15.6 }))).editorFontSize).toBe(16);
  });
