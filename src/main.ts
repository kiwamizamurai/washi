import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";

import { menuActions } from "./actions";
import {
  initialFile,
  jumpToSource,
  latexmkrcStatus,
  print,
  readText as readFileText,
  render,
  renderBuffer,
  renderText,
  setDirty,
  supportedExtensions,
  trustLatexmkrc,
  watch,
} from "./api";
import { interpretPaste } from "./clipboard";
import { EditingController } from "./editor/controller";
import { FindBar } from "./find";
import { hasJumpableSource, pageClick } from "./jump";
import { OutlinePanel } from "./outline";
import { availableCommands } from "./palette/commands";
import type { Item } from "./palette/items";
import { CommandPalette } from "./palette/palette";
import { ReadingProgress } from "./progress";
import { domSearchSource } from "./search-source";
import { basename, extensionOf, resolveLink } from "./paths";
import { loadPrefs, savePrefs, type Prefs } from "./prefs";
import { addRecent, clearRecent, describe, loadRecent, removeRecent } from "./recent";
import { Scroll } from "./scroll";
import { applyPrefs } from "./theme";
import { createToast } from "./toast";
import { Viewer, type Host } from "./viewer";
import { MarkdownView } from "./views/markdown";
import { PdfView } from "./views/pdf";

const CHANGED_EVENT = "washi://changed";
const OPEN_EVENT = "washi://open";
const MENU_EVENT = "washi://menu";
const QUIT_EVENT = "washi://quit-requested";
const RELOAD_DELAY_MS = 150;
const WHEEL_ZOOM_INTERVAL_MS = 90;

const byId = (id: string) => document.getElementById(id)!;

function debounce(fn: () => void, ms: number) {
  let timer: number | undefined;
  return () => {
    clearTimeout(timer);
    timer = window.setTimeout(fn, ms);
  };
}

const recentChanged = { current: () => {} };
const rcTrusted = { current: () => {} };
const declinedRc = new Set<string>();

async function askAboutLatexmkrc(path: string) {
  if (extensionOf(path) !== "tex" && extensionOf(path) !== "latex") return;
  const status = await latexmkrcStatus(path).catch(() => null);
  if (!status || status.trusted || declinedRc.has(status.file)) return;
  const dialog = byId("trust") as HTMLDialogElement;
  const head = await readFileText(status.file).then((d) => d.text.split("\n").slice(0, 14).join("\n")).catch(() => "");
  dialog.querySelector("p")!.textContent = `This folder has a ${basename(status.file)}, which can run commands on your computer. Washi has not used it.`;
  dialog.querySelector("code")!.textContent = status.file;
  dialog.querySelector("pre")!.textContent = head;
  const choice = await new Promise<string>((resolve) => {
    dialog.addEventListener("close", () => resolve(dialog.returnValue), { once: true });
    dialog.returnValue = "";
    dialog.showModal();
  });
  if (choice === "allow") {
    await trustLatexmkrc(path);
    rcTrusted.current();
  } else {
    declinedRc.add(status.file);
  }
}

const host: Host = {
  async render(path) {
    try {
      return await render(path);
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
    void askAboutLatexmkrc(path);
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
    [new MarkdownView(byId("markdown"), scroll), new PdfView(byId("pdf"), scroll)],
    outline,
  );
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
  rcTrusted.current = () => void viewer.reload();
  byId("clear-recent").addEventListener("click", () => {
    clearRecent();
    showRecent();
  });
  showRecent();

  const finder = new FindBar(byId("find") as HTMLFormElement, domSearchSource(byId("scroller")));
  new ReadingProgress(byId("progress").firstElementChild as HTMLElement, byId("scroller"), [
    byId("markdown"),
    byId("pdf"),
  ]);
  const toast = createToast(byId("toast"));

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
      pdf: byId("pdf"),
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

  byId("pdf").addEventListener("click", async (e) => {
    const path = viewer.currentPath;
    const wrapper = (e.target as HTMLElement).closest<HTMLElement>(".page");
    if (!e.metaKey || !wrapper || !hasJumpableSource(path)) return;
    const click = pageClick(wrapper, e.clientX, e.clientY);
    if (!click) return;
    e.preventDefault();
    if (editing.active && (await editing.revealSource(click.page, click.x, click.y))) return;
    try {
      const where = await jumpToSource(path, click.page, click.x, click.y);
      toast(where ? `Opened ${where}` : "No source found for this position");
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

  await listen<string>(MENU_EVENT, (e) => void actions[e.payload]?.());
  await listen(CHANGED_EVENT, async () => {
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
  await listen(QUIT_EVENT, () => void confirmThenDestroy());
  await listen(OPEN_EVENT, () => void openPending());
  await getCurrentWebview().onDragDropEvent((e) => {
    if (e.payload.type !== "drop") return;
    const dropped = e.payload.paths.find(isSupported);
    if (dropped) void open_(dropped);
  });

  await openPending();
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
