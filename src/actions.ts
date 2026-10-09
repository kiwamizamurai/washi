import type { Theme, Width } from "./prefs";
import type { ZoomDirection } from "./views/view";

export interface Controls {
  open(): unknown;
  reload(): unknown;
  print(): unknown;
  find(): unknown;
  paste(): unknown;
  toggleOutline(): unknown;
  setTheme(theme: Theme): unknown;
  setWidth(width: Width): unknown;
  zoom(direction: ZoomDirection): unknown;
  save(): unknown;
  toggleEdit(): unknown;
  undo(): unknown;
  redo(): unknown;
  toggleAutosave(): unknown;
  toggleSyncCursor(): unknown;
  openPalette(): unknown;
  clearRecent(): unknown;
}

export function menuActions(c: Controls): Record<string, () => unknown> {
  return {
    open: () => c.open(),
    reload: () => c.reload(),
    print: () => c.print(),
    paste: () => c.paste(),
    find: () => c.find(),
    outline: () => c.toggleOutline(),
    "theme-system": () => c.setTheme("system"),
    "theme-light": () => c.setTheme("light"),
    "theme-dark": () => c.setTheme("dark"),
    "width-narrow": () => c.setWidth("narrow"),
    "width-wide": () => c.setWidth("wide"),
    "width-full": () => c.setWidth("full"),
    "zoom-in": () => c.zoom("in"),
    "zoom-out": () => c.zoom("out"),
    "zoom-reset": () => c.zoom("reset"),
    save: () => c.save(),
    edit: () => c.toggleEdit(),
    undo: () => c.undo(),
    redo: () => c.redo(),
    autosave: () => c.toggleAutosave(),
    "sync-cursor": () => c.toggleSyncCursor(),
    palette: () => c.openPalette(),
    "recent-clear": () => c.clearRecent(),
  };
}
