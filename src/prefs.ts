import { EDITOR_FONT_SIZE_DEFAULT, EDITOR_FONT_SIZE_RANGE } from "./zoom";

export const THEMES = ["system", "light", "dark"] as const;
export const WIDTHS = ["narrow", "wide", "full"] as const;

export type Theme = (typeof THEMES)[number];
export type Width = (typeof WIDTHS)[number];

export interface Prefs {
  theme: Theme;
  width: Width;
  outline: boolean;
  autosave: boolean;
  syncCursor: boolean;
  editorWidth: number;
  editorFontSize: number;
}

export const EDITOR_WIDTH_RANGE = { min: 25, max: 75 } as const;


export const DEFAULT_PREFS: Prefs = {
  theme: "system",
  width: "narrow",
  outline: false,
  autosave: false,
  syncCursor: true,
  editorWidth: 50,
  editorFontSize: EDITOR_FONT_SIZE_DEFAULT,
};

const KEY = "washi:prefs";

type Store = Pick<Storage, "getItem" | "setItem">;

function defaultStore(): Store | null {
  try {
    return localStorage;
  } catch {
    return null;
  }
}

const pick = <T extends string>(allowed: readonly T[], value: unknown, fallback: T): T =>
  allowed.includes(value as T) ? (value as T) : fallback;

const flag = (value: unknown, fallback: boolean) => (typeof value === "boolean" ? value : fallback);

const percent = (value: unknown, fallback: number) =>
  typeof value === "number" && Number.isFinite(value)
    ? Math.min(EDITOR_WIDTH_RANGE.max, Math.max(EDITOR_WIDTH_RANGE.min, value))
    : fallback;

const px = (value: unknown, fallback: number) =>
  typeof value === "number" && Number.isFinite(value)
    ? Math.min(EDITOR_FONT_SIZE_RANGE.max, Math.max(EDITOR_FONT_SIZE_RANGE.min, Math.round(value)))
    : fallback;

export function loadPrefs(store: Store | null = defaultStore()): Prefs {
  try {
    const raw = store?.getItem(KEY);
    const parsed: Record<string, unknown> = raw ? JSON.parse(raw) : {};
    return {
      theme: pick(THEMES, parsed.theme, DEFAULT_PREFS.theme),
      width: pick(WIDTHS, parsed.width, DEFAULT_PREFS.width),
      outline: flag(parsed.outline, DEFAULT_PREFS.outline),
      autosave: flag(parsed.autosave, DEFAULT_PREFS.autosave),
      syncCursor: flag(parsed.syncCursor, DEFAULT_PREFS.syncCursor),
      editorWidth: percent(parsed.editorWidth, DEFAULT_PREFS.editorWidth),
      editorFontSize: px(parsed.editorFontSize, DEFAULT_PREFS.editorFontSize),
    };
  } catch {
    return { ...DEFAULT_PREFS };
  }
}

export function savePrefs(prefs: Prefs, store: Store | null = defaultStore()) {
  try {
    store?.setItem(KEY, JSON.stringify(prefs));
  } catch {
    return;
  }
}
