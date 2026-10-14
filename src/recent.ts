import { basename } from "./paths";

const KEY = "washi:recent";
export const MAX_RECENT = 8;

type Store = Pick<Storage, "getItem" | "setItem">;

function defaultStore(): Store | null {
  try {
    return localStorage;
  } catch {
    return null;
  }
}

export function loadRecent(store: Store | null = defaultStore()): string[] {
  try {
    const parsed: unknown = JSON.parse(store?.getItem(KEY) ?? "[]");
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((p): p is string => typeof p === "string" && p.startsWith("/")).slice(0, MAX_RECENT);
  } catch {
    return [];
  }
}

function save(list: string[], store: Store | null) {
  try {
    store?.setItem(KEY, JSON.stringify(list));
  } catch {
    return;
  }
}

export function addRecent(path: string, store: Store | null = defaultStore()): string[] {
  const list = [path, ...loadRecent(store).filter((p) => p !== path)].slice(0, MAX_RECENT);
  save(list, store);
  return list;
}

export function removeRecent(path: string, store: Store | null = defaultStore()): string[] {
  const list = loadRecent(store).filter((p) => p !== path);
  save(list, store);
  return list;
}

export function clearRecent(store: Store | null = defaultStore()) {
  save([], store);
}

export function describe(path: string): { name: string; folder: string } {
  const parts = path.split("/").filter(Boolean);
  const name = basename(path);
  const parents = parts.slice(0, -1);
  const shown = parents.slice(-2).join("/");
  return { name, folder: parents.length > 2 ? `…/${shown}` : parents.length ? `/${shown}` : "/" };
}
