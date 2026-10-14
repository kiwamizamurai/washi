export type Kind = "markdown" | "typst" | "latex" | "mermaid";

const KINDS: Record<string, Kind> = {
  md: "markdown",
  markdown: "markdown",
  mdown: "markdown",
  typ: "typst",
  tex: "latex",
  latex: "latex",
  mmd: "mermaid",
  mermaid: "mermaid",
};

export function kindOf(path: string): Kind | null {
  const dot = path.lastIndexOf(".");
  if (dot < 0 || path.lastIndexOf("/") > dot) return null;
  return KINDS[path.slice(dot + 1).toLowerCase()] ?? null;
}

export interface RenderPolicy {
  strategy: "throttle" | "debounce";
  delay: number;
}

export function renderPolicy(kind: Kind): RenderPolicy {
  switch (kind) {
    case "markdown":
    case "mermaid":
      return { strategy: "throttle", delay: 150 };
    case "typst":
      return { strategy: "throttle", delay: 300 };
    case "latex":
      return { strategy: "throttle", delay: 500 };
  }
}

export const DIRTY_MARK = "● ";
