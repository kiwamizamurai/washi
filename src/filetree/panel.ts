import type { DocumentList } from "../api";
import { ancestorsOf, buildTree, moveFocus, relativeTo, visibleRows, type Folder, type NavKey, type Row } from "./model";

export interface FilePanelDeps {
  list(): Promise<DocumentList>;
  open(path: string): void;
  current(): string | null;
}

const NAV_KEYS: readonly string[] = ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End"];

export class FileTreePanel {
  private readonly title: HTMLElement;
  private readonly tree: HTMLElement;
  private readonly note: HTMLElement;
  private folder: Folder = buildTree([]);
  private root = "";
  private expanded = new Set<string>();
  private rows: Row[] = [];
  private focused = 0;
  private token = 0;
  private message: string | null = null;

  constructor(
    private readonly panel: HTMLElement,
    private readonly deps: FilePanelDeps,
  ) {
    this.title = panel.querySelector(".title")!;
    this.tree = panel.querySelector(".tree")!;
    this.note = panel.querySelector(".note")!;
    panel.querySelector(".refresh")!.addEventListener("click", () => void this.refresh());
    this.tree.addEventListener("keydown", (e) => this.onKey(e));
  }

  get visible() {
    return !this.panel.hidden;
  }

  show(visible: boolean) {
    this.panel.hidden = !visible;
    if (visible) void this.refresh();
  }

  async refresh() {
    if (!this.visible) return;
    const token = ++this.token;
    try {
      const list = await this.deps.list();
      if (token !== this.token) return;
      if (list.root !== this.root) this.expanded = new Set();
      this.root = list.root;
      this.folder = buildTree(list.files);
      this.message = list.root ? null : "Open a file to browse its folder";
      this.note.textContent = list.truncated ? "This folder is large, so the list is incomplete." : "";
      this.note.hidden = !list.truncated;
    } catch {
      if (token !== this.token) return;
      this.folder = buildTree([]);
      this.message = "Can't read this folder";
      this.note.hidden = true;
    }
    this.reveal();
    this.render();
  }

  private currentRelative() {
    return relativeTo(this.root, this.deps.current());
  }

  private reveal() {
    const current = this.currentRelative();
    if (current) for (const folder of ancestorsOf(current)) this.expanded.add(folder);
  }

  private render() {
    const label = this.root.split("/").filter(Boolean).pop();
    this.title.textContent = label ? `Files · ${label}` : "Files";
    this.rows = visibleRows(this.folder, this.expanded);
    const current = this.currentRelative();
    const currentIndex = this.rows.findIndex((r) => r.entry.path === current);
    this.focused = Math.min(Math.max(currentIndex, 0), Math.max(this.rows.length - 1, 0));
    if (this.rows.length === 0) {
      const empty = document.createElement("div");
      empty.className = "empty";
      empty.textContent = this.message ?? "No documents in this folder";
      this.tree.replaceChildren(empty);
      return;
    }
    this.tree.replaceChildren(...this.rows.map((row, i) => this.item(row, i, row.entry.path === current)));
    this.tree.children[currentIndex]?.scrollIntoView({ block: "nearest" });
  }

  private item(row: Row, index: number, current: boolean) {
    const { entry } = row;
    const el = document.createElement("div");
    el.setAttribute("role", "treeitem");
    el.setAttribute("aria-level", String(row.depth + 1));
    el.className = `item ${entry.kind}`;
    el.tabIndex = index === this.focused ? 0 : -1;
    el.style.paddingLeft = `${0.5 + row.depth * 0.9}rem`;
    el.title = entry.path;
    if (entry.kind === "folder") el.setAttribute("aria-expanded", String(row.expanded));
    if (current) {
      el.setAttribute("aria-current", "page");
      el.classList.add("active");
    }
    const mark = document.createElement("span");
    mark.className = "mark";
    mark.setAttribute("aria-hidden", "true");
    const name = document.createElement("span");
    name.className = "name";
    const dot = entry.kind === "document" ? entry.name.lastIndexOf(".") : -1;
    name.textContent = dot > 0 ? entry.name.slice(0, dot) : entry.name;
    el.append(mark, name);
    if (dot > 0) {
      const ext = document.createElement("span");
      ext.className = "ext";
      ext.textContent = entry.name.slice(dot);
      el.append(ext);
    }
    el.addEventListener("click", () => {
      this.focused = index;
      this.activate(row);
    });
    return el;
  }

  private activate(row: Row) {
    if (row.entry.kind === "folder") {
      this.toggle(row.entry.path);
      return;
    }
    if (row.entry.path === this.currentRelative()) return;
    this.deps.open(`${this.root.replace(/\/$/, "")}/${row.entry.path}`);
  }

  private toggle(path: string, to?: "open" | "close") {
    const open = to ? to === "open" : !this.expanded.has(path);
    if (open) this.expanded.add(path);
    else this.expanded.delete(path);
    this.render();
    this.focusRow(this.rows.findIndex((r) => r.entry.path === path));
  }

  private focusRow(index: number) {
    const el = this.tree.children[index] as HTMLElement | undefined;
    if (!el) return;
    this.tree.querySelectorAll<HTMLElement>('[tabindex="0"]').forEach((other) => (other.tabIndex = -1));
    el.tabIndex = 0;
    el.focus();
    this.focused = index;
  }

  private onKey(e: KeyboardEvent) {
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    const row = this.rows[this.focused];
    if (NAV_KEYS.includes(e.key)) {
      e.preventDefault();
      const move = moveFocus(this.rows, this.focused, e.key as NavKey);
      if (move.toggle && row) this.toggle(row.entry.path, move.toggle);
      else this.focusRow(move.index);
    } else if ((e.key === "Enter" || e.key === " ") && row) {
      e.preventDefault();
      this.activate(row);
    }
  }
}
