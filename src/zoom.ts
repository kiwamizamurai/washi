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
