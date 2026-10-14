export interface Match {
  score: number;
  indices: number[];
}

const isWordStart = (text: string, i: number) => i === 0 || !/[\p{L}\p{N}]/u.test(text[i - 1]);

export function fuzzyMatch(query: string, text: string): Match | null {
  const terms = query.trim().split(/\s+/).filter(Boolean);
  if (terms.length === 0) return { score: 0, indices: [] };
  let score = 0;
  const indices = new Set<number>();
  for (const term of terms) {
    const m = matchTerm(term.toLowerCase(), text.toLowerCase());
    if (!m) return null;
    score += m.score;
    m.indices.forEach((i) => indices.add(i));
  }
  return { score, indices: [...indices].sort((a, b) => a - b) };
}

function matchTerm(term: string, text: string): Match | null {
  const substring = bestSubstring(term, text);
  if (substring !== null) {
    const bonus = (substring === 0 ? 30 : 0) + (isWordStart(text, substring) ? 20 : 0);
    return {
      score: 100 + bonus - Math.min(substring, 50) - Math.max(0, text.length - term.length) * 0.1,
      indices: Array.from({ length: term.length }, (_, k) => substring + k),
    };
  }
  return subsequence(term, text);
}

function bestSubstring(term: string, text: string): number | null {
  let first: number | null = null;
  for (let at = text.indexOf(term); at !== -1; at = text.indexOf(term, at + 1)) {
    first ??= at;
    if (isWordStart(text, at)) return at;
  }
  return first;
}

function subsequence(term: string, text: string): Match | null {
  const indices: number[] = [];
  let score = 0;
  let from = 0;
  for (const ch of term) {
    let at = text.indexOf(ch, from);
    if (at === -1) return null;
    for (let j = at; j < Math.min(text.length, at + 8); j++) {
      if (text[j] === ch && isWordStart(text, j)) {
        at = j;
        break;
      }
    }
    const previous = indices[indices.length - 1];
    score += 1 + (previous !== undefined && at === previous + 1 ? 3 : 0) + (isWordStart(text, at) ? 4 : 0);
    indices.push(at);
    from = at + 1;
  }
  return { score: Math.min(score, 60), indices };
}
