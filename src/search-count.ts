export interface MatchSummary {
  total: number;
  index: number | null;
}

export function matchStarts(text: string, query: string): number[] | null {
  if (!query) return [];
  const haystack = text.toLowerCase();
  const needle = query.toLowerCase();
  if (haystack.length !== text.length || needle.length !== query.length) return null;
  const starts: number[] = [];
  for (let at = haystack.indexOf(needle); at !== -1; at = haystack.indexOf(needle, at + needle.length)) {
    starts.push(at);
  }
  return starts;
}

export function summarize(text: string, query: string, selectionStart: number | null): MatchSummary | null {
  const starts = matchStarts(text, query);
  if (!starts) return null;
  const at = selectionStart === null ? -1 : starts.indexOf(selectionStart);
  return { total: starts.length, index: at === -1 ? null : at + 1 };
}

export function formatCount(summary: MatchSummary | null, partial: boolean): string {
  if (!summary) return "";
  if (summary.total === 0) return "No results";
  const total = `${summary.total}${partial ? "+" : ""}`;
  return `${summary.index ?? "–"} / ${total}`;
}

export function stepTo(starts: readonly number[], from: number | null, backwards: boolean): number | null {
  if (starts.length === 0) return null;
  if (from === null) return backwards ? starts[starts.length - 1] : starts[0];
  if (backwards) return [...starts].reverse().find((s) => s < from) ?? starts[starts.length - 1];
  return starts.find((s) => s > from) ?? starts[0];
}
