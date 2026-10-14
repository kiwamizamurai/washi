import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { menuActions, type Controls } from "./actions";

function controls() {
  const calls: string[] = [];
  const log = (name: string) => (...args: unknown[]) => void calls.push([name, ...args].join(":"));
  const c: Controls = {
    open: log("open"),
    reload: log("reload"),
    print: log("print"),
    find: log("find"),
    paste: log("paste"),
    toggleOutline: log("outline"),
    toggleFiles: log("files"),
    setTheme: log("theme"),
    setWidth: log("width"),
    zoom: log("zoom"),
    save: log("save"),
    toggleEdit: log("edit"),
    undo: log("undo"),
    redo: log("redo"),
    toggleAutosave: log("autosave"),
    toggleSyncCursor: log("sync-cursor"),
    openPalette: log("palette"),
    checkForUpdates: log("check-updates"),
    toggleAutoUpdates: log("auto-updates"),
  };
  return { calls, actions: menuActions(c) };
}

describe("menuActions", () => {
  it("handles exactly the ids the native menu emits", () => {
    const rust = readFileSync(new URL("../src-tauri/src/menu.rs", import.meta.url), "utf8");
    const block = rust.slice(rust.indexOf("pub mod id"), rust.indexOf("fn item"));
    const rustIds = [...block.matchAll(/pub const \w+: &str = "([^"]+)";/g)].map((m) => m[1]);
    expect(Object.keys(controls().actions).sort()).toEqual([...rustIds].sort());
  });

  it("maps ids to controls with the right arguments", () => {
    const { calls, actions } = controls();
    for (const id of ["theme-dark", "width-full", "zoom-out", "outline", "find", "save", "edit", "undo", "redo"]) actions[id]();
    expect(calls).toEqual(["theme:dark", "width:full", "zoom:out", "outline", "find", "save", "edit", "undo", "redo"]);
  });
});
