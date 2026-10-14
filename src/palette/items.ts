import { fuzzyMatch } from "./match";

export type Group = "Command" | "File" | "Heading";

export interface Item {
  key: string;
  group: Group;
  title: string;
  detail?: string;
  shortcut?: string;
  run(): unknown;
}

export interface Source {
  commands: Item[];
  files: Item[];
  headings: Item[];
}

export interface Ranked {
  item: Item;
  indices: number[];
}

export const MAX_RESULTS = 50;
const GROUP_ORDER: Record<Group, number> = { Command: 0, File: 1, Heading: 2 };

export function rank(source: Source, raw: string, limit = MAX_RESULTS): Ranked[] {
  const headingsOnly = raw.trimStart().startsWith("#");
  const query = headingsOnly ? raw.trimStart().slice(1) : raw;
  const pool = headingsOnly
    ? source.headings
    : query.trim() === ""
      ? [...source.commands, ...source.files]
      : [...source.commands, ...source.files, ...source.headings];

  const scored: { ranked: Ranked; score: number; order: number }[] = [];
  pool.forEach((item, order) => {
    const byTitle = fuzzyMatch(query, item.title);
    if (byTitle) {
      scored.push({ ranked: { item, indices: byTitle.indices }, score: byTitle.score, order });
      return;
    }
    const byDetail = item.group === "File" && item.detail ? fuzzyMatch(query, item.detail) : null;
    if (byDetail) scored.push({ ranked: { item, indices: [] }, score: byDetail.score - 40, order });
  });

  if (query.trim() !== "") {
    scored.sort(
      (a, b) =>
        b.score - a.score || GROUP_ORDER[a.ranked.item.group] - GROUP_ORDER[b.ranked.item.group] || a.order - b.order,
    );
  }
  return scored.slice(0, limit).map((s) => s.ranked);
}
