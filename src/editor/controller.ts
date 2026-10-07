import type { EditorState } from "@codemirror/state";
import { autocomplete, forwardLocate, locateSource, readText, writeFile, type Diagnostic } from "../api";
import { basename } from "../paths";
import type { Prefs } from "../prefs";
import type { Scroll } from "../scroll";
import type { Viewer } from "../viewer";
import type { CursorPosition, EditorHandle } from "./editor";
import { DIRTY_MARK, kindOf, type Kind } from "./kinds";
import { EditSession, type Banner, type SessionHost } from "./session";
import { applyEditorWidth, attachResizer } from "./split";
import { countChars, nextDiagnostic, summarize } from "./status";
import { elementForLine, sourceAt } from "./sync";

const REVEAL_DELAY_MS = 150;
const OWN_WRITE_WINDOW_MS = 800;
const FLASH_MS = 1200;
const COUNT_DELAY_MS = 150;

export interface Elements {
  pane: HTMLElement;
  editor: HTMLElement;
  banner: HTMLElement;
  status: HTMLElement;
  resizer: HTMLElement;
  confirm: HTMLDialogElement;
  scroller: HTMLElement;
  markdown: HTMLElement;
  pdf: HTMLElement;
}

export interface Deps {
  viewer: Viewer;
  scroll: Scroll;
  elements: Elements;
  prefs(): Prefs;
  update(patch: Partial<Prefs>): void;
  toast(message: string, ms?: number): void;
  setTitle(title: string): Promise<void> | void;
  setDirty(dirty: boolean): Promise<void> | void;
}

type Choice = "save" | "discard" | "cancel";

function button(label: string, onClick: () => void) {
  const b = document.createElement("button");
  b.type = "button";
  b.textContent = label;
  b.addEventListener("click", onClick);
  return b;
}

export class EditingController {
  private session: EditSession | null = null;
  private handle: EditorHandle | null = null;
  private kind: Kind | null = null;
  private lastWrite = 0;
  private revealTimer: number | undefined;
  private lastRevealLine = 0;
  private failure: string | null = null;
  private entering = false;
  private retained: { path: string; state: EditorState } | null = null;
  private diagnostics: Diagnostic[] = [];
  private chars = 0;
  private countTimer: number | undefined;

  constructor(private readonly deps: Deps) {
    const { elements: el } = deps;
    attachResizer(
      el.resizer,
      el.pane,
      () => deps.prefs().editorWidth,
      (percent) => {
        deps.update({ editorWidth: percent });
        applyEditorWidth(el.pane, percent);
      },
    );
    el.markdown.addEventListener(
      "click",
      (e) => {
        if (!e.metaKey || !this.session) return;
        const at = sourceAt(e.target as Element);
        if (!at) return;
        e.preventDefault();
        e.stopPropagation();
        this.handle?.goTo(at.line, at.column);
      },
      true,
    );
  }

  get active() {
    return this.session !== null;
  }

  get dirty() {
    return this.session?.dirty ?? false;
  }

  async toggle() {
    if (this.entering) return;
    const session = this.session;
    if (session) {
      const state = this.handle?.state();
      if (!(await this.release())) return;
      if (state && !session.dirty) this.retained = { path: session.path, state };
      await this.deps.viewer.leaveBuffer();
      return;
    }
    await this.enter();
  }

  private async enter() {
    const { viewer, elements: el } = this.deps;
    const path = viewer.currentPath;
    const kind = path ? kindOf(path) : null;
    if (!path || !kind) {
      this.deps.toast(path ? "This kind of file cannot be edited" : "Open a file before editing");
      return;
    }
    this.entering = true;
    try {
      const [{ createEditor }, { languageFor }, disk] = await Promise.all([
        import("./editor"),
        import("./languages"),
        readText(path),
      ]);
      const restore =
        this.retained?.path === path && this.retained.state.doc.toString() === disk.text ? this.retained.state : undefined;
      this.retained = null;
      const [language, extra] = restore
        ? [[], []]
        : await Promise.all([languageFor(kind), this.extensionsFor(kind, path)]);
      const session = new EditSession(this.sessionHost(path), path, disk, {
        kind,
        autosave: () => this.deps.prefs().autosave,
      });
      this.session = session;
      this.kind = kind;
      this.failure = null;
      this.diagnostics = [];
      applyEditorWidth(el.pane, this.deps.prefs().editorWidth);
      el.pane.hidden = false;
      el.resizer.hidden = false;
      this.handle = createEditor(el.editor, {
        doc: disk.text,
        language,
        extra,
        restore,
        onChange: (text) => {
          this.session?.edit(text);
          this.countSoon();
        },
        onCursor: (position) => {
          this.showStatus(position);
          this.revealSoon();
        },
      });
      this.chars = countChars(disk.text);
      this.lastPosition = this.handle.cursor();
      viewer.setBufferHooks({
        diagnostics: (list) => {
          this.diagnostics = this.ownDiagnostics(list);
          this.handle?.setDiagnostics(this.diagnostics);
          this.refreshStatus();
        },
        failed: (message) => {
          this.failure = message;
          this.refreshStatus();
        },
      });
      this.refreshStatus();
      session.start();
      this.handle.focus();
    } catch (e) {
      this.teardown();
      this.deps.toast(`Could not start editing: ${String(e)}`, 5000);
    } finally {
      this.entering = false;
    }
  }

  private async extensionsFor(kind: Kind, path: string) {
    if (kind !== "typst") return [];
    const { typstCompletion } = await import("./complete");
    return [typstCompletion((text, offset, explicit) => autocomplete(path, text, offset, explicit))];
  }

  private sessionHost(path: string): SessionHost {
    const { viewer } = this.deps;
    return {
      readText,
      writeFile: async (p, text, base, force) => {
        try {
          return await writeFile(p, text, base, force);
        } finally {
          this.lastWrite = performance.now();
        }
      },
      render: (p, text) => viewer.renderBuffer(p, text),
      setText: (text) => this.handle?.setText(text),
      dirtyChanged: (dirty) => {
        void this.deps.setTitle(`${dirty ? DIRTY_MARK : ""}${basename(path)} — Washi`);
        void this.deps.setDirty(dirty);
        this.refreshStatus();
      },
      banner: (banner) => this.showBanner(banner),
      notify: (message) => this.deps.toast(message, 5000),
    };
  }

  private ownDiagnostics(list: readonly Diagnostic[]) {
    return list.filter((d) => d.file === null);
  }

  async release(): Promise<boolean> {
    const session = this.session;
    if (!session) return true;
    if (!(await this.confirmDiscardOrSave())) return false;
    this.teardown();
    return true;
  }

  async confirmDiscardOrSave(): Promise<boolean> {
    const session = this.session;
    if (!session?.dirty) return true;
    const choice = await this.ask(`Do you want to save the changes to "${basename(session.path)}"?`);
    if (choice === "cancel") return false;
    if (choice === "discard") return true;
    const outcome = await session.save();
    return outcome === "saved" || outcome === "clean";
  }

  private ask(message: string): Promise<Choice> {
    const dialog = this.deps.elements.confirm;
    dialog.querySelector("p")!.textContent = message;
    return new Promise((resolve) => {
      dialog.addEventListener(
        "close",
        () => resolve(dialog.returnValue === "save" || dialog.returnValue === "discard" ? dialog.returnValue : "cancel"),
        { once: true },
      );
      dialog.returnValue = "";
      dialog.showModal();
    });
  }

  private teardown() {
    const { elements: el, viewer } = this.deps;
    clearTimeout(this.revealTimer);
    clearTimeout(this.countTimer);
    this.countTimer = undefined;
    this.session?.dispose();
    this.handle?.destroy();
    this.session = null;
    this.handle = null;
    this.kind = null;
    this.failure = null;
    viewer.setBufferHooks(null);
    el.pane.hidden = true;
    el.resizer.hidden = true;
    el.editor.replaceChildren();
    el.banner.hidden = true;
    void this.deps.setDirty(false);
  }

  async save() {
    const session = this.session;
    if (!session) return;
    const outcome = await session.save();
    if (outcome === "saved") this.deps.toast("Saved", 1200);
  }

  async diskChanged(): Promise<boolean> {
    const session = this.session;
    if (!session) return false;
    if (performance.now() - this.lastWrite < OWN_WRITE_WINDOW_MS) return true;
    const change = await session.diskChanged();
    if (change === "ignore") session.refreshPreview();
    return true;
  }

  find() {
    if (!this.handle?.hasFocus()) return false;
    this.handle.openSearch();
    return true;
  }

  undo() {
    if (this.handle?.hasFocus()) this.handle.undo();
    else document.execCommand("undo");
  }

  redo() {
    if (this.handle?.hasFocus()) this.handle.redo();
    else document.execCommand("redo");
  }

  paste(text: string) {
    if (!this.handle?.hasFocus()) return false;
    this.handle.replaceSelection(text);
    return true;
  }

  async revealSource(page: number, x: number, y: number): Promise<boolean> {
    const path = this.session?.path;
    if (!path || !this.handle) return false;
    try {
      const found = await locateSource(path, page, x, y);
      if (!found || found.file !== path) return false;
      this.handle.goTo(found.line, found.column);
      return true;
    } catch {
      return false;
    }
  }

  toggleAutosave() {
    const autosave = !this.deps.prefs().autosave;
    this.deps.update({ autosave });
    this.deps.toast(`Autosave: ${autosave ? "on" : "off"}`);
  }

  toggleSyncCursor() {
    const syncCursor = !this.deps.prefs().syncCursor;
    this.deps.update({ syncCursor });
    this.deps.toast(`Show the cursor line in the preview: ${syncCursor ? "on" : "off"}`);
  }

  private showBanner(banner: Banner) {
    const el = this.deps.elements.banner;
    el.replaceChildren();
    if (!banner) {
      el.hidden = true;
      return;
    }
    el.hidden = false;
    el.dataset.kind = banner.kind;
    const text = document.createElement("span");
    el.append(text);
    if (banner.kind === "conflict") {
      text.textContent = "The file on disk was changed elsewhere.";
      el.append(
        button("Keep mine", () => this.session?.keepMine()),
        button("Load the disk version", () => void this.session?.loadDisk()),
      );
    } else {
      text.textContent = `Could not save: ${banner.message}`;
      el.append(button("Dismiss", () => this.showBanner(null)));
    }
  }

  private lastPosition: CursorPosition = { line: 1, column: 1, selected: 0 };

  private showStatus(position: CursorPosition) {
    this.lastPosition = position;
    this.refreshStatus();
  }

  private countSoon() {
    if (this.countTimer !== undefined) return;
    this.countTimer = window.setTimeout(() => {
      this.countTimer = undefined;
      if (!this.handle) return;
      this.chars = countChars(this.handle.getText());
      this.refreshStatus();
    }, COUNT_DELAY_MS);
  }

  private refreshStatus() {
    const status = this.deps.elements.status;
    status.replaceChildren();
    if (!this.session) return;
    const left = document.createElement("span");
    left.className = "left";
    const dirty = this.session.dirty;
    const saved = dirty ? button(`${DIRTY_MARK}Unsaved`, () => void this.save()) : document.createElement("span");
    saved.className = dirty ? "unsaved" : "saved";
    if (dirty) saved.title = "Save (⌘S)";
    else saved.textContent = "Saved";
    const { line, column, selected } = this.lastPosition;
    const where = document.createElement("span");
    where.textContent = `${line}:${column}`;
    const size = document.createElement("span");
    size.textContent = `${this.chars.toLocaleString()} chars${selected ? ` (${selected.toLocaleString()} selected)` : ""}`;
    left.append(saved, where, size);

    const right = document.createElement("span");
    right.className = "right";
    const { errors, warnings } = summarize(this.diagnostics);
    if (errors) right.append(this.diagnosticButton("error", errors));
    if (warnings) right.append(this.diagnosticButton("warning", warnings));
    if (this.failure) {
      const fail = document.createElement("span");
      fail.className = "failure";
      fail.textContent = `Cannot update the preview: ${this.failure.split("\n")[0]}`;
      fail.title = this.failure;
      right.append(fail);
    }
    status.append(left, right);
  }

  private diagnosticButton(severity: "error" | "warning", count: number) {
    const noun = severity === "error" ? "error" : "warning";
    const b = document.createElement("button");
    b.type = "button";
    b.className = severity;
    b.textContent = `${severity === "error" ? "✕" : "⚠"} ${count}`;
    b.title = `${count} ${noun}${count === 1 ? "" : "s"}. Click to go to the next`;
    b.addEventListener("click", () => {
      const handle = this.handle;
      if (!handle) return;
      const next = nextDiagnostic(this.diagnostics, severity, handle.cursor());
      if (next) handle.goTo(next.line, next.column);
    });
    return b;
  }

  private revealSoon() {
    if (!this.deps.prefs().syncCursor) return;
    clearTimeout(this.revealTimer);
    this.revealTimer = window.setTimeout(() => void this.reveal(), REVEAL_DELAY_MS);
  }

  private async reveal() {
    const handle = this.handle;
    const path = this.session?.path;
    if (!handle || !path || !this.kind) return;
    const { line, column } = handle.cursor();
    if (line === this.lastRevealLine) return;
    this.lastRevealLine = line;
    const { scroll, elements: el } = this.deps;
    if (this.kind === "markdown") {
      const target = elementForLine(el.markdown, line);
      if (!target) return;
      scroll.scrollTo(scroll.offsetOf(target) - el.scroller.clientHeight / 3);
      target.classList.add("sync-flash");
      window.setTimeout(() => target.classList.remove("sync-flash"), FLASH_MS);
      return;
    }
    if (this.kind === "mermaid") return;
    try {
      const at = await forwardLocate(path, line, column);
      if (!at || !this.session) return;
      const wrapper = el.pdf.querySelector<HTMLElement>(`.page[data-page="${at.page}"]`);
      const scale = Number(wrapper?.dataset.scale);
      if (!wrapper || !scale) return;
      scroll.scrollTo(scroll.offsetOf(wrapper) + at.y * scale - el.scroller.clientHeight / 3);
      const marker = document.createElement("div");
      marker.className = "sync-marker";
      marker.style.left = `${at.x * scale}px`;
      marker.style.top = `${at.y * scale}px`;
      wrapper.append(marker);
      window.setTimeout(() => marker.remove(), FLASH_MS);
    } catch {
    }
  }
}
