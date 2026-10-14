import { describe, expect, it } from "vitest";
import { rank, type Group, type Item } from "./items";

const item = (group: Group, title: string, detail?: string): Item => ({
  key: `${group}:${title}`,
  group,
  title,
  detail,
  run: () => {},
});

const source = {
  commands: [item("Command", "Open File…"), item("Command", "Reload"), item("Command", "Toggle Outline"), item("Command", "Zoom In")],
  files: [item("File", "plan.md", "…/work/notes"), item("File", "report.typ", "/Users/me")],
  headings: [item("Heading", "Outline of the plan", "H2"), item("Heading", "Install", "H1")],
};
const titles = (raw: string) => rank(source, raw).map((r) => r.item.title);

describe("rank", () => {
  it("shows commands then recent files for an empty query, without headings", () => {
    expect(titles("")).toEqual(["Open File…", "Reload", "Toggle Outline", "Zoom In", "plan.md", "report.typ"]);
  });

  it("filters across commands, files and headings", () => {
    expect(titles("out")).toEqual(["Outline of the plan", "Toggle Outline"]);
  });

  it("puts the better match first, whatever the group", () => {
    expect(titles("plan")[0]).toBe("plan.md");
  });

  it("with # shows only headings", () => {
    expect(titles("#")).toEqual(["Outline of the plan", "Install"]);
    expect(titles("# inst")).toEqual(["Install"]);
    expect(titles("#nope")).toEqual([]);
  });

  it("also finds a file by its folder, without highlighting", () => {
    const hit = rank(source, "notes").find((r) => r.item.title === "plan.md");
    expect(hit).toBeDefined();
    expect(hit!.indices).toEqual([]);
  });

  it("returns the matched positions for highlighting", () => {
    expect(rank(source, "zoom")[0].indices).toEqual([0, 1, 2, 3]);
  });

  it("limits the number of results", () => {
    const many = { commands: Array.from({ length: 80 }, (_, i) => item("Command", `Command ${i}`)), files: [], headings: [] };
    expect(rank(many, "").length).toBe(50);
    expect(rank(many, "", 5).length).toBe(5);
  });

  it("returns nothing when nothing matches", () => {
    expect(titles("qqqzzz")).toEqual([]);
  });
});
