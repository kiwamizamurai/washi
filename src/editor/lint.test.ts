import { describe, expect, it } from "vitest";
import type { Diagnostic } from "../api";
import { columnToOffset, toCmDiagnostics, type DocLike } from "./lint";

function docOf(text: string): DocLike {
  const lines = text.split("\n");
  const starts: number[] = [];
  let at = 0;
  for (const l of lines) {
    starts.push(at);
    at += l.length + 1;
  }
  return {
    lines: lines.length,
    length: text.length,
    line: (n) => ({ from: starts[n - 1], to: starts[n - 1] + lines[n - 1].length, text: lines[n - 1] }),
  };
}

const diagnostic = (patch: Partial<Diagnostic>): Diagnostic => ({
  file: null,
  line: 1,
  column: 1,
  endLine: 1,
  endColumn: 2,
  severity: "error",
  message: "m",
  hints: [],
  ...patch,
});

describe("columnToOffset", () => {
  it("counts code points: a BMP character is 1 unit, an emoji is 2", () => {
    expect(columnToOffset("abc", 1)).toBe(0);
    expect(columnToOffset("abc", 3)).toBe(2);
    expect(columnToOffset("日本語x", 4)).toBe(3);
    expect(columnToOffset("日本😀x", 4)).toBe(4);
    expect(columnToOffset("日本😀x", 5)).toBe(5);
  });

  it("clamps to the end of the line", () => {
    expect(columnToOffset("ab", 99)).toBe(2);
    expect(columnToOffset("", 5)).toBe(0);
  });
});

describe("toCmDiagnostics", () => {
  it("turns line and column into document offsets", () => {
    const doc = docOf("= A\n#undefined-fn()\nend");
    const [d] = toCmDiagnostics(doc, [diagnostic({ line: 2, column: 2, endLine: 2, endColumn: 14, message: "unknown variable" })]);
    expect(d).toMatchObject({ from: 5, to: 17, severity: "error", message: "unknown variable" });
  });

  it("uses code-point columns after Japanese and emoji", () => {
    const doc = docOf("日本😀 #bad()");
    const [d] = toCmDiagnostics(doc, [diagnostic({ line: 1, column: 5, endLine: 1, endColumn: 9 })]);
    expect(doc.line(1).text.slice(d.from, d.to)).toBe("#bad");
  });

  it("widens an empty range to one character and keeps it inside the document", () => {
    const doc = docOf("abc");
    const [a] = toCmDiagnostics(doc, [diagnostic({ line: 1, column: 2, endLine: 1, endColumn: 2 })]);
    expect([a.from, a.to]).toEqual([1, 2]);
    const [b] = toCmDiagnostics(doc, [diagnostic({ line: 1, column: 4, endLine: 1, endColumn: 4 })]);
    expect(b.from).toBeLessThanOrEqual(3);
    expect(b.to).toBeLessThanOrEqual(3);
  });

  it("tolerates a document that got shorter since the compile (stale positions)", () => {
    const doc = docOf("x");
    const [d] = toCmDiagnostics(doc, [diagnostic({ line: 40, column: 9, endLine: 41, endColumn: 3 })]);
    expect(d.from).toBeLessThanOrEqual(1);
    expect(d.to).toBeLessThanOrEqual(1);
    expect(d.to).toBeGreaterThanOrEqual(d.from);
  });

  it("shows only diagnostics of the open file and appends the hints", () => {
    const doc = docOf("a\nb");
    const list = [
      diagnostic({ file: "other.typ", message: "elsewhere" }),
      diagnostic({ message: "here", hints: ["try x", "or y"] }),
    ];
    const out = toCmDiagnostics(doc, list);
    expect(out).toHaveLength(1);
    expect(out[0].message).toBe("here\nHint: try x\nHint: or y");
  });

  it("maps warnings to the warning severity", () => {
    const [d] = toCmDiagnostics(docOf("abc"), [diagnostic({ severity: "warning" })]);
    expect(d.severity).toBe("warning");
  });
});
