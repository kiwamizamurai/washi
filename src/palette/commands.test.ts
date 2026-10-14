import { describe, expect, it, vi } from "vitest";
import { menuActions, type Controls } from "../actions";
import { availableCommands, COMMANDS } from "./commands";

describe("palette commands", () => {
  it("only use ids the menu handles", () => {
    const stub = new Proxy({}, { get: () => vi.fn() }) as Controls;
    const handled = Object.keys(menuActions(stub));
    for (const { id } of COMMANDS) expect(handled, id).toContain(id);
  });

  it("have unique ids", () => {
    const ids = COMMANDS.map((c) => c.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("hide editing-only commands while reading", () => {
    const reading = availableCommands({ editing: false }).map((c) => c.id);
    const editing = availableCommands({ editing: true }).map((c) => c.id);
    for (const id of ["save", "undo", "redo", "sync-cursor"]) {
      expect(reading).not.toContain(id);
      expect(editing).toContain(id);
    }
    expect(reading).toContain("edit");
  });

  it("names the edit command after what it will do", () => {
    const edit = COMMANDS.find((c) => c.id === "edit")!;
    expect(edit.title({ editing: false })).toBe("Edit (Split View)");
    expect(edit.title({ editing: true })).toBe("Back to Reading");
  });
});
