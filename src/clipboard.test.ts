import { describe, expect, it } from "vitest";
import { interpretPaste } from "./clipboard";

const supported = (p: string) => /\.(md|typ|tex)$/.test(p);

describe("interpretPaste", () => {
  it("ignores empty clipboard", () => {
    expect(interpretPaste("", supported)).toBeNull();
    expect(interpretPaste("  \n\t", supported)).toBeNull();
  });

  it("treats a single supported path as a file", () => {
    expect(interpretPaste("/Users/me/a.md\n", supported)).toEqual({ kind: "file", path: "/Users/me/a.md" });
    expect(interpretPaste("file:///Users/me/%E6%97%A5.typ", supported)).toEqual({
      kind: "file",
      path: "/Users/me/日.typ",
    });
  });

  it("treats unsupported paths and prose as text", () => {
    expect(interpretPaste("/Users/me/a.docx", supported)).toEqual({ kind: "text", text: "/Users/me/a.docx" });
    expect(interpretPaste("# Hello\n\nworld", supported)).toEqual({ kind: "text", text: "# Hello\n\nworld" });
  });

  it("normalises CRLF", () => {
    expect(interpretPaste("a\r\nb", supported)).toEqual({ kind: "text", text: "a\nb" });
  });
});
