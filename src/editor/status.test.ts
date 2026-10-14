import { describe, expect, it } from "vitest";
import type { Diagnostic } from "../api";
import { countChars, nextDiagnostic, summarize } from "./status";

const diag = (severity: "error" | "warning", line: number, column = 1): Diagnostic => ({
  file: null,
  line,
  column,
  endLine: line,
  endColumn: column + 1,
  severity,
  message: "m",
  hints: [],
});

describe("countChars", () => {
  it("counts characters, not UTF-16 units", () => {
    expect(countChars("")).toBe(0);
    expect(countChars("abc")).toBe(3);
    expect(countChars("日本語")).toBe(3);
    expect(countChars("a😀b")).toBe(3);
    expect(countChars("😀😀")).toBe(2);
  });

  it("counts a lone surrogate as one", () => {
    expect(countChars("\ud83d")).toBe(1);
    expect(countChars("\ud83dx")).toBe(2);
  });
});

describe("summarize", () => {
  it("counts errors and warnings", () => {
    expect(summarize([])).toEqual({ errors: 0, warnings: 0 });
    expect(summarize([diag("error", 1), diag("warning", 2), diag("error", 3)])).toEqual({ errors: 2, warnings: 1 });
  });
});

describe("nextDiagnostic", () => {
  const list = [diag("error", 9), diag("warning", 4), diag("error", 2), diag("error", 2, 7)];

  it("goes to the next one after the cursor, in source order", () => {
    expect(nextDiagnostic(list, "error", { line: 1, column: 1 })).toMatchObject({ line: 2, column: 1 });
    expect(nextDiagnostic(list, "error", { line: 2, column: 1 })).toMatchObject({ line: 2, column: 7 });
    expect(nextDiagnostic(list, "error", { line: 2, column: 7 })).toMatchObject({ line: 9 });
  });

  it("wraps around to the first", () => {
    expect(nextDiagnostic(list, "error", { line: 9, column: 1 })).toMatchObject({ line: 2, column: 1 });
  });

  it("only looks at the asked severity", () => {
    expect(nextDiagnostic(list, "warning", { line: 1, column: 1 })).toMatchObject({ line: 4 });
  });

  it("is null when there are none", () => {
    expect(nextDiagnostic([], "error", { line: 1, column: 1 })).toBeNull();
    expect(nextDiagnostic([diag("warning", 1)], "error", { line: 1, column: 1 })).toBeNull();
  });
});
