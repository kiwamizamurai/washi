export interface Folder {
  kind: "folder";
  name: string;
  path: string;
  children: Entry[];
}

export interface Document {
  kind: "document";
  name: string;
  path: string;
}

export type Entry = Folder | Document;

export interface Row {
  entry: Entry;
  depth: number;
  expanded: boolean;
}

export type NavKey = "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight" | "Home" | "End";

export interface Move {
  index: number;
  toggle?: "open" | "close";
}

const byName = (a: Entry, b: Entry) =>
  a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: "base" });

const rank = (entry: Entry) => (entry.kind === "folder" ? 0 : 1);

function sortEntries(entries: Entry[]) {
  entries.sort((a, b) => rank(a) - rank(b) || byName(a, b));
  for (const entry of entries) if (entry.kind === "folder") sortEntries(entry.children);
}

export function buildTree(files: readonly string[]): Folder {
  const root: Folder = { kind: "folder", name: "", path: "", children: [] };
  for (const file of files) {
    const parts = file.split("/").filter(Boolean);
    let folder = root;
    parts.slice(0, -1).forEach((name, i) => {
      const path = parts.slice(0, i + 1).join("/");
      let next = folder.children.find((e): e is Folder => e.kind === "folder" && e.name === name);
      if (!next) {
        next = { kind: "folder", name, path, children: [] };
        folder.children.push(next);
      }
      folder = next;
    });
    const name = parts[parts.length - 1];
    if (name) folder.children.push({ kind: "document", name, path: parts.join("/") });
  }
  sortEntries(root.children);
  return root;
}

export function visibleRows(root: Folder, expanded: ReadonlySet<string>): Row[] {
  const rows: Row[] = [];
  const walk = (folder: Folder, depth: number) => {
    for (const entry of folder.children) {
      const open = entry.kind === "folder" && expanded.has(entry.path);
      rows.push({ entry, depth, expanded: open });
      if (entry.kind === "folder" && open) walk(entry, depth + 1);
    }
  };
  walk(root, 0);
  return rows;
}

export function ancestorsOf(path: string): string[] {
  const parts = path.split("/").filter(Boolean);
  return parts.slice(0, -1).map((_, i) => parts.slice(0, i + 1).join("/"));
}

export function relativeTo(root: string, path: string | null): string | null {
  if (!root || !path) return null;
  const prefix = root.endsWith("/") ? root : `${root}/`;
  return path.startsWith(prefix) ? path.slice(prefix.length) : null;
}

export function moveFocus(rows: readonly Row[], index: number, key: NavKey): Move {
  const last = rows.length - 1;
  const row = rows[index];
  switch (key) {
    case "ArrowDown":
      return { index: Math.min(last, index + 1) };
    case "ArrowUp":
      return { index: Math.max(0, index - 1) };
    case "Home":
      return { index: 0 };
    case "End":
      return { index: Math.max(0, last) };
    case "ArrowRight": {
      if (!row || row.entry.kind !== "folder") return { index };
      if (!row.expanded) return { index, toggle: "open" };
      const next = rows[index + 1];
      return { index: next && next.depth > row.depth ? index + 1 : index };
    }
    case "ArrowLeft": {
      if (!row) return { index };
      if (row.entry.kind === "folder" && row.expanded) return { index, toggle: "close" };
      for (let i = index - 1; i >= 0; i--) if (rows[i].depth < row.depth) return { index: i };
      return { index };
    }
  }
}
