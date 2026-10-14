import { describe, expect, it } from "vitest";
import { kindOf, renderPolicy } from "./kinds";

describe("kindOf", () => {
  it("maps the editable extensions, case-insensitively", () => {
    expect(kindOf("/a/notes.md")).toBe("markdown");
    expect(kindOf("/a/NOTES.MARKDOWN")).toBe("markdown");
    expect(kindOf("/a/b.typ")).toBe("typst");
    expect(kindOf("/a/p.tex")).toBe("latex");
    expect(kindOf("/a/p.latex")).toBe("latex");
    expect(kindOf("/a/d.mmd")).toBe("mermaid");
    expect(kindOf("/a/d.mermaid")).toBe("mermaid");
  });

  it("returns null for PDF, unknown extensions and extension-less names", () => {
    expect(kindOf("/a/x.pdf")).toBeNull();
    expect(kindOf("/a/x.txt")).toBeNull();
    expect(kindOf("/a/Makefile")).toBeNull();
    expect(kindOf("/a.d/Makefile")).toBeNull();
  });
});

describe("renderPolicy", () => {
  it("renders cheap formats often, Typst less often, and LaTeX the least often", () => {
    expect(renderPolicy("markdown")).toEqual({ strategy: "throttle", delay: 150 });
    expect(renderPolicy("mermaid")).toEqual({ strategy: "throttle", delay: 150 });
    expect(renderPolicy("typst")).toEqual({ strategy: "throttle", delay: 300 });
    expect(renderPolicy("latex")).toEqual({ strategy: "throttle", delay: 500 });
  });
});
