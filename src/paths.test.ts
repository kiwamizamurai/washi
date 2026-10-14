import { describe, expect, it } from "vitest";
import { basename, dirname, extensionOf, resolveLink } from "./paths";

describe("resolveLink", () => {
  const base = "/docs/guide/index.md";

  it("resolves relative links against the current file", () => {
    expect(resolveLink(base, "intro.md")).toBe("/docs/guide/intro.md");
    expect(resolveLink(base, "./a/b.typ")).toBe("/docs/guide/a/b.typ");
    expect(resolveLink(base, "../top.md")).toBe("/docs/top.md");
  });

  it("drops fragments and queries, and decodes percent-encoding", () => {
    expect(resolveLink(base, "intro.md#sec")).toBe("/docs/guide/intro.md");
    expect(resolveLink(base, "%E6%97%A5%E6%9C%AC.md")).toBe("/docs/guide/日本.md");
  });

  it("treats leading slash as absolute", () => {
    expect(resolveLink(base, "/etc/x.md")).toBe("/etc/x.md");
  });

  it("ignores anchors, URLs and schemes", () => {
    for (const href of ["#top", "https://example.com/a.md", "mailto:a@b.c", "//cdn/x.md", ""]) {
      expect(resolveLink(base, href), href).toBeNull();
    }
  });

  it("does not escape above the root", () => {
    expect(resolveLink("/a.md", "../../x.md")).toBe("/x.md");
  });
});

describe("basename / extensionOf", () => {
  it("handles typical paths", () => {
    expect(basename("/a/b/c.md")).toBe("c.md");
    expect(extensionOf("/a/b/C.MD")).toBe("md");
    expect(extensionOf("/a/b/README")).toBe("");
    expect(extensionOf("/a.b/README")).toBe("");
  });
});

describe("dirname", () => {
  it("drops the last segment", () => {
    expect(dirname("/a/b/c.md")).toBe("/a/b");
    expect(dirname("/c.md")).toBe("/");
    expect(dirname("c.md")).toBe("/");
  });
});
