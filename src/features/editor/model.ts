/**
 * Editor data model. All item coordinates are **image pixels relative to the
 * crop's top-left** — the same space the Rust renderer uses, so preview and
 * export can't drift apart.
 */

export interface PixelRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export type Point = [number, number];

export type DrawTool = "arrow" | "rect" | "ellipse" | "line" | "pen" | "highlight" | "step" | "text" | "redact" | "blur" | "pixelate";
export type Tool = "select" | DrawTool;

interface BoxBase {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

type Stroked = { color: string; strokeWidth: number };
type Segment = { id: string; x1: number; y1: number; x2: number; y2: number } & Stroked;

// One union member per `type` so `Extract<Item, { type: T }>` narrows cleanly.
export type Item =
  | (BoxBase & Stroked & { type: "rect" })
  | (BoxBase & Stroked & { type: "ellipse" })
  | (Segment & { type: "line" })
  | (Segment & { type: "arrow" })
  | ({ id: string; type: "pen"; points: Point[] } & Stroked)
  | ({ id: string; type: "highlight"; points: Point[] } & Stroked)
  /** Numbered step marker centred on (x, y). */
  | { id: string; type: "step"; x: number; y: number; radius: number; number: number; color: string }
  | { id: string; type: "text"; x: number; y: number; text: string; color: string; fontSize: number }
  | (BoxBase & { type: "redact"; color: string })
  | (BoxBase & { type: "blur"; strength: number })
  | (BoxBase & { type: "pixelate"; strength: number });

export type ItemType = Item["type"];
export type ItemOf<T extends ItemType> = Extract<Item, { type: T }>;

/** Keep in sync with `src-tauri/src/imaging/text.rs`. */
export const TEXT_LINE_HEIGHT = 1.25;
/** Font ascender / unitsPerEm of the bundled annotation font (Geist SemiBold). */
export const TEXT_ASCENT = 1.005;
export const ANNOTATION_FONT = "FastShotAnnotation";
/** Keep in sync with `HIGHLIGHT_ALPHA` in `render.rs`. */
export const HIGHLIGHT_ALPHA = 0.4;
/** Keep in sync with `STEP_BASELINE` in `render.rs`. */
export const STEP_BASELINE = 0.36;
export const MAX_STEP_NUMBER = 999;

/** Mirrors `step_font_size` in `render.rs`. */
export function stepFontSize(radius: number, n: number): number {
  return radius * (n <= 9 ? 1.15 : n <= 99 ? 0.95 : 0.72);
}

/** Black label on light discs, white otherwise. Mirrors `step_label_color`. */
export function stepLabelColor(fill: string): string {
  const hex = fill.replace("#", "");
  const full = hex.length === 3 ? hex.split("").map((c) => c + c).join("") : hex.slice(0, 6);
  const [r, g, b] = [0, 2, 4].map((i) => parseInt(full.slice(i, i + 2), 16));
  const lum = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
  return lum > 0.6 ? "#000000" : "#ffffff";
}

/** Next step number: one past the highest existing marker. */
export function nextStepNumber(items: Item[]): number {
  let max = 0;
  for (const i of items) if (i.type === "step" && i.number > max) max = i.number;
  return Math.min(MAX_STEP_NUMBER, max + 1);
}

let idCounter = 0;
export function newId(): string {
  idCounter += 1;
  return `i${Date.now().toString(36)}${idCounter}`;
}

export function normalizeRect(r: { x: number; y: number; width: number; height: number }): PixelRect {
  const x = r.width < 0 ? r.x + r.width : r.x;
  const y = r.height < 0 ? r.y + r.height : r.y;
  return { x, y, width: Math.abs(r.width), height: Math.abs(r.height) };
}

export function rectFromPoints(a: Point, b: Point): PixelRect {
  return normalizeRect({ x: a[0], y: a[1], width: b[0] - a[0], height: b[1] - a[1] });
}

/** Arrow geometry — mirrors `arrow_head` in `src-tauri/src/imaging/render.rs`. */
export function arrowGeometry(x1: number, y1: number, x2: number, y2: number, stroke: number) {
  const dx = x2 - x1;
  const dy = y2 - y1;
  const len = Math.hypot(dx, dy);
  if (len < 0.5) return null;
  const ux = dx / len;
  const uy = dy / len;
  const headLen = Math.min(Math.max(stroke * 4, 12), len);
  const halfW = headLen * 0.45;
  const bx = x2 - ux * headLen;
  const by = y2 - uy * headLen;
  return {
    shaftEnd: [x2 - ux * headLen * 0.6, y2 - uy * headLen * 0.6] as Point,
    left: [bx - uy * halfW, by + ux * halfW] as Point,
    tip: [x2, y2] as Point,
    right: [bx + uy * halfW, by - ux * halfW] as Point,
  };
}

export function textLines(text: string): string[] {
  return text.split("\n");
}

/** Baseline of line `i` for a text item whose box top is `y`. */
export function textBaseline(y: number, fontSize: number, i: number): number {
  return y + TEXT_ASCENT * fontSize + i * TEXT_LINE_HEIGHT * fontSize;
}

export type TextMeasurer = (line: string, fontSize: number) => number;

/** Approximate fallback (tests / before fonts load). */
export const roughMeasure: TextMeasurer = (line, fontSize) => line.length * fontSize * 0.58;

export function itemBounds(item: Item, measure: TextMeasurer = roughMeasure): PixelRect {
  switch (item.type) {
    case "rect":
    case "ellipse":
    case "redact":
    case "blur":
    case "pixelate":
      return normalizeRect(item);
    case "line":
    case "arrow":
      return rectFromPoints([item.x1, item.y1], [item.x2, item.y2]);
    case "step":
      return { x: item.x - item.radius, y: item.y - item.radius, width: item.radius * 2, height: item.radius * 2 };
    case "pen":
    case "highlight": {
      let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
      for (const [x, y] of item.points) {
        if (x < minX) minX = x;
        if (y < minY) minY = y;
        if (x > maxX) maxX = x;
        if (y > maxY) maxY = y;
      }
      return { x: minX, y: minY, width: maxX - minX, height: maxY - minY };
    }
    case "text": {
      const lines = textLines(item.text);
      const width = Math.max(item.fontSize * 0.5, ...lines.map((l) => measure(l, item.fontSize)));
      return { x: item.x, y: item.y, width, height: lines.length * TEXT_LINE_HEIGHT * item.fontSize };
    }
  }
}

export function translateItem(item: Item, dx: number, dy: number): Item {
  switch (item.type) {
    case "line":
    case "arrow":
      return { ...item, x1: item.x1 + dx, y1: item.y1 + dy, x2: item.x2 + dx, y2: item.y2 + dy };
    case "pen":
    case "highlight":
      return { ...item, points: item.points.map(([x, y]) => [x + dx, y + dy] as Point) };
    default:
      return { ...item, x: item.x + dx, y: item.y + dy };
  }
}

/** Is an item big enough to keep? Prevents accidental zero-size clicks. */
export function isMeaningful(item: Item): boolean {
  switch (item.type) {
    case "pen":
    case "highlight":
      return item.points.length > 0;
    case "step":
      return true;
    case "text":
      return item.text.trim().length > 0;
    case "line":
    case "arrow":
      return Math.hypot(item.x2 - item.x1, item.y2 - item.y1) >= 3;
    default: {
      const r = normalizeRect(item);
      return r.width >= 2 && r.height >= 2;
    }
  }
}

/** Items that can change colour, and the property that holds it. */
export function supportsColor(item: Item): boolean {
  return item.type !== "blur" && item.type !== "pixelate";
}

// ---------------------------------------------------------------- export ---

export type AnnotationOp =
  | { type: "rect" | "ellipse"; x: number; y: number; width: number; height: number; color: string; strokeWidth: number }
  | { type: "line" | "arrow"; x1: number; y1: number; x2: number; y2: number; color: string; strokeWidth: number }
  | { type: "pen" | "highlight"; points: Point[]; color: string; strokeWidth: number }
  | { type: "step"; x: number; y: number; radius: number; number: number; color: string }
  | { type: "text"; x: number; y: number; text: string; color: string; fontSize: number };

export interface RenderRequest {
  crop: PixelRect;
  annotations: AnnotationOp[];
  effects: { kind: "blur" | "pixelate"; rect: PixelRect; strength: number }[];
  redactions: { rect: PixelRect; color: string }[];
}

/**
 * Split editor items into the Rust render contract. Array order is z-order for
 * annotations; Rust always applies effects first and redactions last
 * regardless of where they sit in the editor's stack.
 */
export function toRenderRequest(crop: PixelRect, items: Item[]): RenderRequest {
  const req: RenderRequest = {
    crop: {
      x: Math.round(crop.x),
      y: Math.round(crop.y),
      width: Math.round(crop.width),
      height: Math.round(crop.height),
    },
    annotations: [],
    effects: [],
    redactions: [],
  };
  for (const item of items) {
    switch (item.type) {
      case "redact":
        req.redactions.push({ rect: normalizeRect(item), color: item.color });
        break;
      case "blur":
      case "pixelate":
        req.effects.push({ kind: item.type, rect: normalizeRect(item), strength: item.strength });
        break;
      case "rect":
      case "ellipse": {
        const r = normalizeRect(item);
        req.annotations.push({ type: item.type, ...r, color: item.color, strokeWidth: item.strokeWidth });
        break;
      }
      case "line":
      case "arrow":
        req.annotations.push({ type: item.type, x1: item.x1, y1: item.y1, x2: item.x2, y2: item.y2, color: item.color, strokeWidth: item.strokeWidth });
        break;
      case "pen":
      case "highlight":
        req.annotations.push({ type: item.type, points: item.points, color: item.color, strokeWidth: item.strokeWidth });
        break;
      case "step":
        req.annotations.push({ type: "step", x: item.x, y: item.y, radius: item.radius, number: item.number, color: item.color });
        break;
      case "text":
        if (item.text.trim()) {
          req.annotations.push({ type: "text", x: item.x, y: item.y, text: item.text, color: item.color, fontSize: item.fontSize });
        }
        break;
    }
  }
  return req;
}
