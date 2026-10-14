import type { Output } from "../api";
import type { OutlineNode } from "../outline";

export type ZoomDirection = "in" | "out" | "reset";

export interface RenderContext {
  readonly width: number;
  isCurrent(): boolean;
  restoreScroll(): void;
}

export interface View {
  readonly accepts: Output["kind"];
  readonly el: HTMLElement;
  show(output: Output, ctx: RenderContext): Promise<void>;
  relayout?(ctx: RenderContext): Promise<void>;
  zoom?(direction: ZoomDirection, ctx: RenderContext): Promise<void>;
  resetZoom?(): void;
  outline?(): Promise<OutlineNode[]>;
}
