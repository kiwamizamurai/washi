import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";

import { menuActions } from "./actions";
import {
  initialFile,
  latestRelease,
  listDocuments,
  openInTab,
  openSource,
  print,
  render,
  renderBuffer,
  renderText,
  setDirty,
  supportedExtensions,
  watch,
} from "./api";
import { interpretPaste } from "./clipboard";
import { EditingController } from "./editor/controller";
import { listenHere } from "./events";
import { sourceAt } from "./editor/sync";
import { FindBar } from "./find";
import { FileTreePanel } from "./filetree/panel";
import { OutlinePanel } from "./outline";
import { availableCommands } from "./palette/commands";
import type { Item } from "./palette/items";
import { CommandPalette } from "./palette/palette";
import { ReadingProgress } from "./progress";
import { domSearchSource } from "./search-source";
import { basename, dirname, extensionOf, resolveLink } from "./paths";
import { loadPrefs, savePrefs, type Prefs } from "./prefs";
import { addRecent, clearRecent, describe, loadRecent, removeRecent } from "./recent";
import { Scroll } from "./scroll";
import { applyPrefs } from "./theme";
import { createBusy } from "./busy";
import { createToast } from "./toast";
import { checkForUpdate, loadUpdateState, updateState } from "./update";
import { UpdateNotice } from "./update-notice";
import { Viewer, type Host } from "./viewer";
import { MarkdownView } from "./views/markdown";

const CHANGED_EVENT = "washi://changed";
const OPEN_EVENT = "washi://open";
const MENU_EVENT = "washi://menu";
const QUIT_EVENT = "washi://quit-requested";
const RELOAD_DELAY_MS = 150;
const FILES_REFRESH_DELAY_MS = 300;
const WHEEL_ZOOM_INTERVAL_MS = 90;
const UPDATE_CHECK_DELAY_MS = 3000;

const byId = (id: string) => document.getElementById(id)!;

function debounce(fn: () => void, ms: number) {
  let timer: number | undefined;
  return () => {
    clearTimeout(timer);
    timer = window.setTimeout(fn, ms);
  };
}

const recentChanged = { current: () => {} };
const filesChanged = { current: () => {} };

const trackBusy = createBusy({
  show(message) {
    const el = byId("busy");
    el.replaceChildren();
    const text = document.createElement("span");
    text.className = "text";
    text.textContent = message;
    el.append(text);
    el.hidden = false;
  },
  hide() {
    byId("busy").hidden = true;
  },
});

const host: Host = {
  async render(path) {
    try {
      return await trackBusy(`Rendering ${basename(path)}…`, render(path));
    } catch (e) {
      removeRecent(path);
      recentChanged.current();
      throw e;
    }
  },
  renderBuffer,
  renderText,
  async opened(path) {
    addRecent(path);
    recentChanged.current();
    await getCurrentWindow().setTitle(`${basename(path)} — Washi`);
    await watch(path);
    filesChanged.current();
  },
  async pasted() {
    await getCurrentWindow().setTitle("Pasted text — Washi");
  },
};

async function main() {
  const extensions = await supportedExtensions();
  const isSupported = (path: string) => extensions.includes(extensionOf(path));

  const root = document.documentElement;
  let prefs = loadPrefs();
  applyPrefs(root, prefs);

  const scroll = new Scroll(byId("scroller"));
  const outline = new OutlinePanel(byId("outline"), byId("outline").querySelector("nav")!, scroll);
  outline.show(prefs.outline);

  const viewer = new Viewer(
    host,
    scroll,
    { empty: byId("empty"), error: byId("error") },
    [new MarkdownView(byId("markdown"), scroll)],
    outline,
  );
  const files = new FileTreePanel(byId("files"), {
    list: listDocuments,
    open: (path) => void openInTab(path).catch((e) => toast(String(e))),
    current: () => viewer.currentPath,
  });
  const refreshFilesSoon = debounce(() => void files.refresh(), FILES_REFRESH_DELAY_MS);
  files.show(prefs.fileTree);
  filesChanged.current = () => void files.refresh();
  window.addEventListener("focus", refreshFilesSoon);
  const recentList = byId("recent");
  const showRecent = () => {
    const list = loadRecent();
    recentList.hidden = list.length === 0;
    recentList.querySelector("ul")!.replaceChildren(
      ...list.map((path) => {
        const { name, folder } = describe(path);
        const li = document.createElement("li");
        const button = document.createElement("button");
        button.type = "button";
        button.title = path;
        const nameEl = document.createElement("span");
        nameEl.className = "name";
        nameEl.textContent = name;
        const folderEl = document.createElement("span");
        folderEl.className = "folder";
        folderEl.textContent = `\u200e${folder}\u200e`;
        button.append(nameEl, folderEl);
        button.addEventListener("click", () => void open_(path));
        li.append(button);
        return li;
      }),
    );
  };
  recentChanged.current = showRecent;
  byId("clear-recent").addEventListener("click", () => {
    clearRecent();
    showRecent();
  });
  showRecent();

  const finder = new FindBar(byId("find") as HTMLFormElement, domSearchSource(byId("scroller")));
  new ReadingProgress(byId("progress").firstElementChild as HTMLElement, byId("scroller"), [byId("markdown")]);
  const toast = createToast(byId("toast"));

  const notice = new UpdateNotice(byId("update"), {
    openNotes: (url) => void openUrl(url),
    copy: (text) => navigator.clipboard.writeText(text),
  });
  const checkUpdates = async (manual: boolean) => {
    const outcome = await checkForUpdate({ fetchLatest: latestRelease, manual, now: Date.now() });
    if (outcome.kind === "available") notice.show(outcome.release);
    else if (manual && outcome.kind === "current") toast(`Washi ${outcome.release.current} is the latest version`);
    else if (manual && outcome.kind === "failed") toast(outcome.error, 5000);
  };

  const editing = new EditingController({
    viewer,
    scroll,
    elements: {
      pane: byId("editor-pane"),
      editor: byId("editor"),
      banner: byId("banner"),
      status: byId("editor-status"),
      resizer: byId("resizer"),
      confirm: byId("confirm") as HTMLDialogElement,
      scroller: byId("scroller"),
      markdown: byId("markdown"),
    },
    prefs: () => prefs,
    update: (patch) => update(patch),
    toast,
    setTitle: (title) => getCurrentWindow().setTitle(title),
    setDirty,
  });

  const open_ = async (path: string) => {
    if (await editing.release()) await viewer.load(path);
  };

  const reloadSoon = debounce(() => void viewer.reload(), RELOAD_DELAY_MS);
  const relayoutSoon = debounce(() => void viewer.relayout(), RELOAD_DELAY_MS);

  const update = (patch: Partial<Prefs>) => {
    prefs = { ...prefs, ...patch };
    savePrefs(prefs);
    applyPrefs(root, prefs);
  };

  const pick = async () => {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Documents", extensions }],
    });
    if (typeof selected === "string") await open_(selected);
  };

  const paste = async () => {
    const text = await readText().catch(() => "");
    const field = document.activeElement;
    if (editing.paste(text)) return;
    if (field instanceof HTMLInputElement) {
      const end = field.value.length;
      field.setRangeText(text, field.selectionStart ?? end, field.selectionEnd ?? end, "end");
      return;
    }
    const pasted = interpretPaste(text, isSupported);
    if (pasted && !(await editing.release())) return;
    if (pasted?.kind === "file") await viewer.load(pasted.path);
    else if (pasted) await viewer.showText(pasted.text);
  };

  const palette = new CommandPalette(byId("palette") as HTMLDialogElement, () => {
    const ctx = { editing: editing.active };
    const current = viewer.currentPath;
    const commands: Item[] = availableCommands(ctx).map((c) => ({
      key: `command:${c.id}`,
      group: "Command",
      title: c.title(ctx),
      shortcut: c.shortcut,
      run: () => actions[c.id]?.(),
    }));
    const files: Item[] = loadRecent()
      .filter((path) => path !== current)
      .map((path) => ({
        key: `file:${path}`,
        group: "File",
        title: basename(path),
        detail: describe(path).folder,
        run: () => open_(path),
      }));
    const headings: Item[] = outline.headings.map((node, i) => ({
      key: `heading:${i}`,
      group: "Heading",
      title: node.title,
      detail: `H${node.level}`,
      run: () => node.go(),
    }));
    return { commands, files, headings };
  });

  const actions = menuActions({
    open: pick,
    reload: () => viewer.reload(),
    print,
    find: () => editing.find() || finder.open(),
    paste,
    toggleFiles: () => {
      update({ fileTree: !prefs.fileTree });
      files.show(prefs.fileTree);
      relayoutSoon();
    },
    toggleOutline: () => {
      update({ outline: !prefs.outline });
      outline.show(prefs.outline);
      relayoutSoon();
    },
    setTheme: (theme) => {
      update({ theme });
      return viewer.reload();
    },
    setWidth: (width) => update({ width }),
    zoom: (direction) => viewer.zoom(direction),
    save: () => editing.save(),
    toggleEdit: () => editing.toggle(),
    undo: () => editing.undo(),
    redo: () => editing.redo(),
    toggleAutosave: () => editing.toggleAutosave(),
    toggleSyncCursor: () => editing.toggleSyncCursor(),
    openPalette: () => palette.open(),
    checkForUpdates: () => checkUpdates(true),
    toggleAutoUpdates: () => {
      const { automatic } = updateState({ automatic: !loadUpdateState().automatic });
      toast(automatic ? "Automatic update checks are on" : "Automatic update checks are off");
    },
  });

  const openPending = async () => {
    const path = await initialFile();
    if (path) await open_(path);
  };

  byId("open").addEventListener("click", () => void pick());
  window.addEventListener("resize", relayoutSoon);

  let lastWheelZoom = 0;
  byId("scroller").addEventListener(
    "wheel",
    (e) => {
      if (!e.ctrlKey && !e.metaKey) return;
      e.preventDefault();
      const now = performance.now();
      if (now - lastWheelZoom < WHEEL_ZOOM_INTERVAL_MS) return;
      lastWheelZoom = now;
      void viewer.zoom(e.deltaY < 0 ? "in" : "out");
    },
    { passive: false },
  );

  byId("markdown").addEventListener("click", async (e) => {
    const target = e.target as Element;
    const path = viewer.currentPath;
    if (!e.metaKey || !path || target.closest("a")) return;
    const at = sourceAt(target);
    if (!at) return;
    e.preventDefault();
    const file = at.file ? `${dirname(path)}/${at.file}` : path;
    try {
      toast(`Opened ${await openSource(file, at.line, at.column)}`);
    } catch (error) {
      toast(String(error), 5000);
    }
  });

  document.addEventListener("click", (e) => {
    const href = (e.target as HTMLElement).closest("a")?.getAttribute("href");
    if (!href || href.startsWith("#")) return;
    e.preventDefault();
    if (/^https?:\/\//.test(href)) {
      void openUrl(href);
      return;
    }
    const base = viewer.currentPath;
    const target = base && resolveLink(base, href);
    if (target && isSupported(target)) void open_(target);
  });

  await listenHere<string>(MENU_EVENT, (e) => void actions[e.payload]?.());
  await listenHere(CHANGED_EVENT, async () => {
    if (!(await editing.diskChanged())) reloadSoon();
  });

  const confirmThenDestroy = async () => {
    if (await editing.confirmDiscardOrSave()) await getCurrentWindow().destroy();
  };
  await getCurrentWindow().onCloseRequested(async (e) => {
    if (!editing.dirty) return;
    e.preventDefault();
    await confirmThenDestroy();
  });
  await listenHere(QUIT_EVENT, () => void confirmThenDestroy());
  await listenHere(OPEN_EVENT, () => void openPending());
  await getCurrentWebview().onDragDropEvent((e) => {
    if (e.payload.type !== "drop") return;
    const dropped = e.payload.paths.find(isSupported);
    if (dropped) void open_(dropped);
  });

  await openPending();
  window.setTimeout(() => void checkUpdates(false), UPDATE_CHECK_DELAY_MS);
}

window.addEventListener("DOMContentLoaded", () => {
  main().catch((e) => {
    const error = document.getElementById("error");
    if (error) {
      error.textContent = `Failed to start: ${String(e)}`;
      error.hidden = false;
    }
    console.error(e);
  });
});
