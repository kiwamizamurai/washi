import { describe, expect, it } from "vitest";
import { elementForLine, parseSourcepos, sourceAt } from "./sync";

const el = (sourcepos: string, name = "x", sourceFile?: string) =>
  ({ dataset: { sourcepos, sourceFile }, name }) as unknown as HTMLElement;

describe("parseSourcepos", () => {
  it("reads the start of the range", () => {
    expect(parseSourcepos("3:5-4:10")).toEqual({ line: 3, column: 5 });
  });
  it("rejects anything else", () => {
    expect(parseSourcepos("nope")).toBeNull();
    expect(parseSourcepos(undefined)).toBeNull();
  });
});

describe("sourceAt", () => {
  it("reads the nearest ancestor that has a position", () => {
    const target = { closest: () => el("3:1-4:8") };
    expect(sourceAt(target)).toEqual({ line: 3, column: 1, file: null });
  });
  it("names the file when the element comes from an included one", () => {
    const target = { closest: () => el("2:3-2:9", "x", "part.typ") };
    expect(sourceAt(target)).toEqual({ line: 2, column: 3, file: "part.typ" });
  });
  it("is null when nothing has a position", () => {
    expect(sourceAt({ closest: () => null })).toBeNull();
    expect(sourceAt(null)).toBeNull();
  });
});

describe("elementForLine", () => {
  const list = [el("1:1-1:6", "h1"), el("3:1-4:8", "p"), el("6:1-8:5", "ul"), el("7:1-8:5", "li")];
  const root = { querySelectorAll: () => list } as unknown as ParentNode;
  const name = (line: number) => (elementForLine(root, line) as unknown as { name: string } | null)?.name;

  it("finds the element at or just before a line", () => {
    expect(name(1)).toBe("h1");
    expect(name(2)).toBe("h1");
    expect(name(4)).toBe("p");
    expect(name(7)).toBe("li");
    expect(name(100)).toBe("li");
  });

  it("finds nothing before the first element", () => {
    expect(elementForLine({ querySelectorAll: () => [] } as unknown as ParentNode, 1)).toBeNull();
    expect(name(0)).toBeUndefined();
  });

  it("does not rely on the elements being in line order", () => {
    const shuffled = [el("6:1-6:4", "late"), el("1:1-1:6", "first"), el("3:1-3:9", "middle")];
    const unordered = { querySelectorAll: () => shuffled } as unknown as ParentNode;
    const pick = (line: number) => (elementForLine(unordered, line) as unknown as { name: string } | null)?.name;
    expect(pick(4)).toBe("middle");
    expect(pick(7)).toBe("late");
    expect(pick(2)).toBe("first");
  });

  it("ignores elements that belong to included files", () => {
    const mixed = [el("1:1-1:6", "main"), el("2:1-2:6", "included", "part.typ")];
    const root2 = { querySelectorAll: () => mixed } as unknown as ParentNode;
    expect((elementForLine(root2, 5) as unknown as { name: string }).name).toBe("main");
  });
});
