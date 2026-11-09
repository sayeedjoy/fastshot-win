import { memo, useEffect, useState } from "react";

import { measureText } from "./measure";
import {
  ANNOTATION_FONT,
  HIGHLIGHT_ALPHA,
  type Item,
  type ItemOf,
  type PixelRect,
  STEP_BASELINE,
  arrowGeometry,
  normalizeRect,
  stepFontSize,
  stepLabelColor,
  textBaseline,
  textLines,
} from "./model";

/** Everything the effect previews need to sample the frozen image. */
export interface PreviewSource {
  img: HTMLImageElement | null;
  url: string;
  imageWidth: number;
  imageHeight: number;
  crop: PixelRect;
}

const pts = (p: [number, number][]) => p.map(([x, y]) => `${x.toFixed(1)},${y.toFixed(1)}`).join(" ");

function PenShape({ item }: { item: ItemOf<"pen"> }) {
  if (item.points.length === 1) {
    const [x, y] = item.points[0];
    return <circle cx={x} cy={y} r={item.strokeWidth / 2} fill={item.color} />;
  }
  return <polyline points={pts(item.points)} fill="none" stroke={item.color} strokeWidth={item.strokeWidth} strokeLinecap="round" strokeLinejoin="round" />;
}

/** Highlighter: one translucent polyline, so overlaps don't stack (matches Rust). */
function HighlightShape({ item }: { item: ItemOf<"highlight"> }) {
  if (item.points.length === 1) {
    const [x, y] = item.points[0];
    return <circle cx={x} cy={y} r={item.strokeWidth / 2} fill={item.color} fillOpacity={HIGHLIGHT_ALPHA} />;
  }
  return <polyline points={pts(item.points)} fill="none" stroke={item.color} strokeOpacity={HIGHLIGHT_ALPHA} strokeWidth={item.strokeWidth} strokeLinecap="round" strokeLinejoin="round" />;
}

/** Step marker: same centring maths as `render.rs` (advance width, fixed baseline offset). */
function StepShape({ item }: { item: ItemOf<"step"> }) {
  const label = String(item.number);
  const size = stepFontSize(item.radius, item.number);
  return (
    <>
      <circle cx={item.x} cy={item.y} r={item.radius} fill={item.color} />
      <text className="annotation-text" x={item.x - measureText(label, size) / 2} y={item.y + STEP_BASELINE * size} fontFamily={ANNOTATION_FONT} fontSize={size} fill={stepLabelColor(item.color)}>
        {label}
      </text>
    </>
  );
}

function ArrowShape({ item }: { item: ItemOf<"arrow"> }) {
  const g = arrowGeometry(item.x1, item.y1, item.x2, item.y2, item.strokeWidth);
  if (!g) return null;
  return (
    <>
      <line x1={item.x1} y1={item.y1} x2={g.shaftEnd[0]} y2={g.shaftEnd[1]} stroke={item.color} strokeWidth={item.strokeWidth} strokeLinecap="round" />
      <polygon points={pts([g.left, g.tip, g.right])} fill={item.color} />
    </>
  );
}

function TextShape({ item }: { item: ItemOf<"text"> }) {
  return (
    <>
      {textLines(item.text).map((line, i) => (
        <text key={i} className="annotation-text" x={item.x} y={textBaseline(item.y, item.fontSize, i)} fontFamily={ANNOTATION_FONT} fontSize={item.fontSize} fill={item.color}>
          {line}
        </text>
      ))}
    </>
  );
}

/** Blur preview: the frozen image, Gaussian-blurred and clipped to the item. */
function BlurShape({ item, src }: { item: ItemOf<"blur">; src: PreviewSource }) {
  const r = normalizeRect(item);
  const fid = `blur-${item.id}`;
  const cid = `clip-${item.id}`;
  return (
    <>
      <defs>
        <filter id={fid} x={r.x - item.strength * 3} y={r.y - item.strength * 3} width={r.width + item.strength * 6} height={r.height + item.strength * 6} filterUnits="userSpaceOnUse">
          <feGaussianBlur stdDeviation={item.strength} edgeMode="duplicate" />
        </filter>
        <clipPath id={cid}>
          <rect x={r.x} y={r.y} width={r.width} height={r.height} />
        </clipPath>
      </defs>
      <g clipPath={`url(#${cid})`}>
        <image href={src.url} x={-src.crop.x} y={-src.crop.y} width={src.imageWidth} height={src.imageHeight} filter={`url(#${fid})`} preserveAspectRatio="none" />
      </g>
    </>
  );
}

/**
 * Pixelate preview: average blocks by downscaling the region onto a tiny
 * canvas, then upscale with nearest-neighbour. Blocks are aligned to the
 * region's top-left, like the Rust implementation.
 */
function PixelateShape({ item, src }: { item: ItemOf<"pixelate">; src: PreviewSource }) {
  const r = normalizeRect(item);
  const block = Math.max(2, Math.round(item.strength));
  const cols = Math.max(1, Math.ceil(r.width / block));
  const rows = Math.max(1, Math.ceil(r.height / block));
  const [dataUrl, setDataUrl] = useState<string | null>(null);
  const sx = src.crop.x + r.x;
  const sy = src.crop.y + r.y;

  useEffect(() => {
    if (!src.img?.complete) return;
    const c = document.createElement("canvas");
    c.width = cols;
    c.height = rows;
    const ctx = c.getContext("2d");
    if (!ctx) return;
    ctx.imageSmoothingEnabled = true;
    ctx.imageSmoothingQuality = "high";
    ctx.drawImage(src.img, sx, sy, cols * block, rows * block, 0, 0, cols, rows);
    try {
      setDataUrl(c.toDataURL());
    } catch {
      setDataUrl(null);
    }
  }, [src.img, sx, sy, cols, rows, block]);

  const cid = `clip-${item.id}`;
  return (
    <>
      <defs>
        <clipPath id={cid}>
          <rect x={r.x} y={r.y} width={r.width} height={r.height} />
        </clipPath>
      </defs>
      {dataUrl ? (
        <image href={dataUrl} x={r.x} y={r.y} width={cols * block} height={rows * block} clipPath={`url(#${cid})`} preserveAspectRatio="none" style={{ imageRendering: "pixelated" }} />
      ) : (
        <rect x={r.x} y={r.y} width={r.width} height={r.height} fill="rgba(127,127,127,0.6)" />
      )}
    </>
  );
}

export const ItemShape = memo(function ItemShape({ item, src }: { item: Item; src: PreviewSource }) {
  switch (item.type) {
    case "rect": {
      const r = normalizeRect(item);
      return <rect {...r} fill="none" stroke={item.color} strokeWidth={item.strokeWidth} strokeLinejoin="miter" />;
    }
    case "ellipse": {
      const r = normalizeRect(item);
      return <ellipse cx={r.x + r.width / 2} cy={r.y + r.height / 2} rx={r.width / 2} ry={r.height / 2} fill="none" stroke={item.color} strokeWidth={item.strokeWidth} />;
    }
    case "line":
      return <line x1={item.x1} y1={item.y1} x2={item.x2} y2={item.y2} stroke={item.color} strokeWidth={item.strokeWidth} strokeLinecap="round" />;
    case "arrow":
      return <ArrowShape item={item} />;
    case "pen":
      return <PenShape item={item} />;
    case "highlight":
      return <HighlightShape item={item} />;
    case "step":
      return <StepShape item={item} />;
    case "text":
      return <TextShape item={item} />;
    case "redact":
      // Opaque in preview, exactly like the export (SEC-01).
      return <rect {...normalizeRect(item)} fill={item.color} />;
    case "blur":
      return <BlurShape item={item} src={src} />;
    case "pixelate":
      return <PixelateShape item={item} src={src} />;
  }
});

/** Invisible, generous hit target so thin strokes are easy to grab. */
export function HitArea({ item, bounds, scale }: { item: Item; bounds: PixelRect; scale: number }) {
  const pad = 8 * scale;
  const common = { "data-item-id": item.id, style: { cursor: "move" } } as const;
  switch (item.type) {
    case "rect":
    case "ellipse": {
      const r = normalizeRect(item);
      const w = item.strokeWidth + pad * 2;
      return item.type === "rect" ? (
        <rect {...common} {...r} fill="none" stroke="transparent" strokeWidth={w} pointerEvents="stroke" />
      ) : (
        <ellipse {...common} cx={r.x + r.width / 2} cy={r.y + r.height / 2} rx={r.width / 2} ry={r.height / 2} fill="none" stroke="transparent" strokeWidth={w} pointerEvents="stroke" />
      );
    }
    case "line":
    case "arrow":
      return <line {...common} x1={item.x1} y1={item.y1} x2={item.x2} y2={item.y2} stroke="transparent" strokeWidth={item.strokeWidth + pad * 2} strokeLinecap="round" pointerEvents="stroke" />;
    case "step":
      return <circle {...common} cx={item.x} cy={item.y} r={item.radius + pad / 2} fill="transparent" />;
    case "pen":
    case "highlight":
      return item.points.length === 1 ? (
        <circle {...common} cx={item.points[0][0]} cy={item.points[0][1]} r={item.strokeWidth / 2 + pad} fill="transparent" />
      ) : (
        <polyline {...common} points={pts(item.points)} fill="none" stroke="transparent" strokeWidth={item.strokeWidth + pad * 2} strokeLinecap="round" strokeLinejoin="round" pointerEvents="stroke" />
      );
    default:
      return <rect {...common} x={bounds.x} y={bounds.y} width={bounds.width} height={bounds.height} fill="transparent" />;
  }
}
