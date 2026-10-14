import { clamp } from "../zoom";

const WHEEL_STEP = 1.1;
const SCALE_RANGE = { min: 0.2, max: 8 } as const;
const DRAG_THRESHOLD_PX = 4;

export function enableDiagramZoom(root: HTMLElement) {
  for (const diagram of root.querySelectorAll<HTMLElement>(".mermaid")) {
    diagram.addEventListener("click", () => openOverlay(diagram));
  }
}

function openOverlay(source: HTMLElement) {
  const svg = source.querySelector("svg");
  if (!svg) return;

  const overlay = document.createElement("div");
  overlay.className = "diagram-overlay";
  const stage = document.createElement("div");
  stage.className = "diagram-stage";
  const clone = svg.cloneNode(true) as SVGElement;
  clone.removeAttribute("style");
  clone.removeAttribute("width");
  clone.removeAttribute("height");
  stage.append(clone);
  overlay.append(stage);

  let scale = 1;
  let x = 0;
  let y = 0;
  let travelled = 0;
  let dragging = false;
  const apply = () => {
    stage.style.transform = `translate(${x}px, ${y}px) scale(${scale})`;
  };

  const close = () => {
    overlay.remove();
    document.removeEventListener("keydown", onKey);
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") close();
  };

  overlay.addEventListener(
    "wheel",
    (e) => {
      e.preventDefault();
      const factor = e.deltaY < 0 ? WHEEL_STEP : 1 / WHEEL_STEP;
      scale = clamp(scale * factor, SCALE_RANGE.min, SCALE_RANGE.max);
      apply();
    },
    { passive: false },
  );
  overlay.addEventListener("pointerdown", (e) => {
    dragging = true;
    travelled = 0;
    overlay.setPointerCapture(e.pointerId);
  });
  overlay.addEventListener("pointermove", (e) => {
    if (!dragging) return;
    x += e.movementX;
    y += e.movementY;
    travelled += Math.abs(e.movementX) + Math.abs(e.movementY);
    apply();
  });
  overlay.addEventListener("pointerup", () => {
    dragging = false;
    if (travelled < DRAG_THRESHOLD_PX) close();
  });

  document.addEventListener("keydown", onKey);
  document.body.append(overlay);
  apply();
}
