import { ANNOTATION_FONT, type TextMeasurer, roughMeasure } from "./model";

let ctx: CanvasRenderingContext2D | null = null;
const cache = new Map<string, number>();

/** Width of one line in the annotation font (kerning off, like the exporter). */
export const measureText: TextMeasurer = (line, fontSize) => {
  const key = `${fontSize}|${line}`;
  const hit = cache.get(key);
  if (hit !== undefined) return hit;
  ctx ??= document.createElement("canvas").getContext("2d");
  if (!ctx) return roughMeasure(line, fontSize);
  ctx.font = `${fontSize}px ${ANNOTATION_FONT}`;
  if ("fontKerning" in ctx) ctx.fontKerning = "none";
  const w = ctx.measureText(line).width;
  if (cache.size > 2000) cache.clear();
  cache.set(key, w);
  return w;
};

/** Forget measured lines. Keys contain typed text, so clear them when a capture ends. */
export function clearMeasureCache(): void {
  cache.clear();
}

export async function loadAnnotationFont(): Promise<void> {
  try {
    await document.fonts.load(`24px ${ANNOTATION_FONT}`);
  } catch {
    // Falls back to rough measurement; export is unaffected.
  }
}
