export interface SourcePoint {
  line: number;
  column: number;
}

export interface SourceSpot extends SourcePoint {
  file: string | null;
}

export function parseSourcepos(value: string | undefined | null): SourcePoint | null {
  const m = /^(\d+):(\d+)-\d+:\d+$/.exec(value ?? "");
  return m ? { line: Number(m[1]), column: Number(m[2]) } : null;
}

export function sourceAt(target: Pick<Element, "closest"> | null): SourceSpot | null {
  const el = target?.closest<HTMLElement>("[data-sourcepos]");
  const point = parseSourcepos(el?.dataset.sourcepos);
  return el && point ? { ...point, file: el.dataset.sourceFile ?? null } : null;
}

export function elementForLine(root: Pick<ParentNode, "querySelectorAll">, line: number): HTMLElement | null {
  let found: HTMLElement | null = null;
  let foundLine = 0;
  for (const el of root.querySelectorAll<HTMLElement>("[data-sourcepos]")) {
    if (el.dataset.sourceFile) continue;
    const at = parseSourcepos(el.dataset.sourcepos);
    if (!at || at.line > line || at.line < foundLine) continue;
    found = el;
    foundLine = at.line;
  }
  return found;
}
