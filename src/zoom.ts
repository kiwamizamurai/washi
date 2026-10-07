import type { ZoomDirection } from "./views/view";

export const ZOOM_STEP = 1.1;
export const ZOOM_RANGE = { min: 0.4, max: 4 } as const;

export const clamp = (value: number, min: number, max: number) =>
  Math.min(max, Math.max(min, value));

export function stepZoom(level: number, direction: ZoomDirection) {
  if (direction === "reset") return 1;
  const next = direction === "in" ? level * ZOOM_STEP : level / ZOOM_STEP;
  return clamp(next, ZOOM_RANGE.min, ZOOM_RANGE.max);
}

export const EDITOR_FONT_SIZE_RANGE = { min: 10, max: 24 } as const;
export const EDITOR_FONT_SIZE_DEFAULT = 14;

export function stepEditorFontSize(size: number, direction: ZoomDirection) {
  if (direction === "reset") return EDITOR_FONT_SIZE_DEFAULT;
  const next = direction === "in" ? size + 1 : size - 1;
  return clamp(next, EDITOR_FONT_SIZE_RANGE.min, EDITOR_FONT_SIZE_RANGE.max);
}
