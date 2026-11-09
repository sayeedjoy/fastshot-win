import { useLayoutEffect, useRef, useState } from "react";

import type { Point } from "@/features/editor/model";

/** Image pixels shown across the loupe (odd, so one pixel sits in the centre). */
const SAMPLE = 15;
/** Loupe size in CSS px. */
const SIZE = 120;
const CELL = SIZE / SAMPLE;
const GAP = 20;

let probe: CanvasRenderingContext2D | null = null;

function pixelHex(img: HTMLImageElement, x: number, y: number): string | null {
  probe ??= (() => {
    const c = document.createElement("canvas");
    c.width = 1;
    c.height = 1;
    return c.getContext("2d", { willReadFrequently: true });
  })();
  if (!probe) return null;
  try {
    probe.clearRect(0, 0, 1, 1);
    probe.drawImage(img, x, y, 1, 1, 0, 0, 1, 1);
    const [r, g, b] = probe.getImageData(0, 0, 1, 1).data;
    return `#${[r, g, b].map((v) => v.toString(16).padStart(2, "0")).join("")}`;
  } catch {
    return null; // tainted canvas; the loupe still works without the readout
  }
}

interface Props {
  img: HTMLImageElement;
  /** Pointer in image px. */
  point: Point;
  /** Image px per CSS px. */
  sx: number;
  sy: number;
  viewport: { w: number; h: number };
}

/**
 * Pixel loupe for precise selection (P1). Reads the frozen preview image, so
 * it shows exactly the pixels that will be cropped; it is UI chrome only and
 * never part of the export.
 */
export function Magnifier({ img, point, sx, sy, viewport }: Props) {
  const ref = useRef<HTMLCanvasElement>(null);
  const [hex, setHex] = useState<string | null>(null);
  const px = Math.min(img.naturalWidth - 1, Math.floor(point[0]));
  const py = Math.min(img.naturalHeight - 1, Math.floor(point[1]));

  useLayoutEffect(() => {
    const c = ref.current;
    const ctx = c?.getContext("2d");
    if (!c || !ctx) return;
    const dpr = window.devicePixelRatio || 1;
    if (c.width !== SIZE * dpr) {
      c.width = SIZE * dpr;
      c.height = SIZE * dpr;
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.imageSmoothingEnabled = false;
    ctx.fillStyle = "#000";
    ctx.fillRect(0, 0, SIZE, SIZE);
    const half = (SAMPLE - 1) / 2;
    ctx.drawImage(img, px - half, py - half, SAMPLE, SAMPLE, 0, 0, SIZE, SIZE);

    // Crosshair through the centre pixel, drawn twice for contrast on any colour.
    const c0 = half * CELL;
    for (const [color, w] of [["rgba(0,0,0,0.6)", 3], ["rgba(255,255,255,0.9)", 1]] as const) {
      ctx.strokeStyle = color;
      ctx.lineWidth = w;
      ctx.strokeRect(c0 + 0.5, c0 + 0.5, CELL - 1, CELL - 1);
    }
    setHex(pixelHex(img, px, py));
  }, [img, px, py]);

  // Below-right of the cursor; flip to the other side near the edges.
  const cx = point[0] / sx;
  const cy = point[1] / sy;
  const boxH = SIZE + 28;
  const left = cx + GAP + SIZE + 8 > viewport.w ? cx - GAP - SIZE : cx + GAP;
  const top = cy + GAP + boxH + 8 > viewport.h ? cy - GAP - boxH : cy + GAP;

  return (
    <div className="pointer-events-none absolute flex flex-col overflow-hidden rounded-lg border border-white/30 bg-black/85 shadow-xl" style={{ left, top, width: SIZE }} aria-hidden>
      <canvas ref={ref} style={{ width: SIZE, height: SIZE, imageRendering: "pixelated" }} />
      <div className="flex items-center justify-between gap-1 px-1.5 py-1 font-mono text-[11px] text-white tabular-nums">
        <span>
          {px}, {py}
        </span>
        {hex ? (
          <span className="flex items-center gap-1">
            <span className="size-2.5 rounded-sm border border-white/40" style={{ background: hex }} />
            {hex}
          </span>
        ) : null}
      </div>
    </div>
  );
}
