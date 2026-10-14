import { describe, expect, it } from "vitest";
import { ancestorsOf, buildTree, moveFocus, relativeTo, visibleRows, type Row } from "./model";

const names = (rows: Row[]) => rows.map((r) => `${" ".repeat(r.depth)}${r.entry.name}`);

describe("buildTree", () => {
  it("nests files under their folders", () => {
    const tree = buildTree(["a.md", "docs/guide.md", "docs/api/ref.typ"]);
    const docs = tree.children.find((e) => e.name === "docs");
    expect(docs?.kind).toBe("folder");
    expect(docs?.kind === "folder" && docs.children.map((e) => e.path)).toEqual(["docs/api", "docs/guide.md"]);
  });

  it("puts folders first and sorts names naturally, ignoring case", () => {
    const tree = buildTree(["b.md", "A.md", "10.md", "2.md", "z/x.md", "Y/x.md"]);
    expect(tree.children.map((e) => e.name)).toEqual(["Y", "z", "2.md", "10.md", "A.md", "b.md"]);
  });

  it("returns an empty root for no files and ignores empty segments", () => {
    expect(buildTree([]).children).toEqual([]);
    expect(buildTree(["//a.md"]).children.map((e) => e.path)).toEqual(["a.md"]);
  });
});

describe("visibleRows", () => {
  const tree = buildTree(["top.md", "docs/guide.md", "docs/api/ref.typ"]);

  it("shows only the top level while everything is collapsed", () => {
    expect(names(visibleRows(tree, new Set()))).toEqual(["docs", "top.md"]);
  });

  it("shows the children of open folders, indented by depth", () => {
    expect(names(visibleRows(tree, new Set(["docs"])))).toEqual(["docs", " api", " guide.md", "top.md"]);
    expect(names(visibleRows(tree, new Set(["docs", "docs/api"])))).toEqual([
      "docs",
      " api",
      "  ref.typ",
      " guide.md",
      "top.md",
    ]);
  });

  it("does not show children of a closed folder inside an open one", () => {
    expect(names(visibleRows(tree, new Set(["docs/api"])))).toEqual(["docs", "top.md"]);
  });
});

describe("paths", () => {
  it("lists the folders above a file, outermost first", () => {
    expect(ancestorsOf("a/b/c.md")).toEqual(["a", "a/b"]);
    expect(ancestorsOf("c.md")).toEqual([]);
  });

  it("makes a path relative to the root only when it is inside", () => {
    expect(relativeTo("/notes", "/notes/a/b.md")).toBe("a/b.md");
    expect(relativeTo("/notes/", "/notes/a.md")).toBe("a.md");
    expect(relativeTo("/notes", "/notes-old/a.md")).toBeNull();
    expect(relativeTo("/notes", "/other/a.md")).toBeNull();
    expect(relativeTo("", "/a.md")).toBeNull();
    expect(relativeTo("/notes", null)).toBeNull();
  });
});

describe("moveFocus", () => {
  const tree = buildTree(["top.md", "docs/guide.md", "docs/api/ref.typ"]);
  const rows = visibleRows(tree, new Set(["docs", "docs/api"]));
  const at = (name: string) => rows.findIndex((r) => r.entry.name === name);

  it("moves up and down, stopping at both ends", () => {
    expect(moveFocus(rows, 0, "ArrowUp")).toEqual({ index: 0 });
    expect(moveFocus(rows, 0, "ArrowDown")).toEqual({ index: 1 });
    expect(moveFocus(rows, rows.length - 1, "ArrowDown")).toEqual({ index: rows.length - 1 });
  });

  it("jumps to the first and last row", () => {
    expect(moveFocus(rows, 2, "Home")).toEqual({ index: 0 });
    expect(moveFocus(rows, 2, "End")).toEqual({ index: rows.length - 1 });
    expect(moveFocus([], 0, "End")).toEqual({ index: 0 });
  });

  it("opens a closed folder, then enters it, with the right arrow", () => {
    const closed = visibleRows(tree, new Set());
    expect(moveFocus(closed, 0, "ArrowRight")).toEqual({ index: 0, toggle: "open" });
    expect(moveFocus(rows, at("docs"), "ArrowRight")).toEqual({ index: at("docs") + 1 });
  });

  it("does nothing on the right arrow for a document", () => {
    expect(moveFocus(rows, at("guide.md"), "ArrowRight")).toEqual({ index: at("guide.md") });
  });

  it("closes an open folder, then goes to the parent, with the left arrow", () => {
    expect(moveFocus(rows, at("api"), "ArrowLeft")).toEqual({ index: at("api"), toggle: "close" });
    expect(moveFocus(rows, at("ref.typ"), "ArrowLeft")).toEqual({ index: at("api") });
    expect(moveFocus(rows, at("guide.md"), "ArrowLeft")).toEqual({ index: at("docs") });
  });

  it("stays on a top-level document on the left arrow", () => {
    expect(moveFocus(rows, at("top.md"), "ArrowLeft")).toEqual({ index: at("top.md") });
  });
});
