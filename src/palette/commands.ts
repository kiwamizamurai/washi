export interface Context {
  editing: boolean;
}

export interface CommandDef {
  id: string;
  title(ctx: Context): string;
  shortcut?: string;
  editingOnly?: boolean;
}

const fixed = (id: string, title: string, shortcut?: string, editingOnly = false): CommandDef => ({
  id,
  title: () => title,
  shortcut,
  editingOnly,
});

export const COMMANDS: CommandDef[] = [
  fixed("open", "Open File…", "⌘O"),
  fixed("reload", "Reload", "⌘R"),
  { id: "edit", title: (ctx) => (ctx.editing ? "Back to Reading" : "Edit (Split View)"), shortcut: "⌘E" },
  fixed("save", "Save", "⌘S", true),
  fixed("undo", "Undo", "⌘Z", true),
  fixed("redo", "Redo", "⇧⌘Z", true),
  fixed("find", "Find…", "⌘F"),
  fixed("paste", "Paste and Open", "⌘V"),
  fixed("print", "Print…", "⌘P"),
  fixed("outline", "Toggle Outline", "⇧⌘O"),
  fixed("files", "Toggle Files", "⌘B"),
  fixed("zoom-in", "Zoom In", "⌘+"),
  fixed("zoom-out", "Zoom Out", "⌘-"),
  fixed("zoom-reset", "Actual Size", "⌘0"),
  fixed("theme-system", "Theme: Match System"),
  fixed("theme-light", "Theme: Light"),
  fixed("theme-dark", "Theme: Dark"),
  fixed("width-narrow", "Text Width: Standard"),
  fixed("width-wide", "Text Width: Wide"),
  fixed("width-full", "Text Width: Full Window"),
  fixed("autosave", "Toggle Autosave"),
  fixed("sync-cursor", "Toggle Cursor Line in Preview", undefined, true),
  fixed("check-updates", "Check for Updates…"),
  fixed("auto-updates", "Toggle Automatic Update Checks"),
];

export const availableCommands = (ctx: Context) => COMMANDS.filter((c) => ctx.editing || !c.editingOnly);
