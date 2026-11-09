import { type PointerEvent as ReactPointerEvent, useCallback, useLayoutEffect, useMemo, useRef, useState } from "react";

import { HitArea, ItemShape, type PreviewSource } from "@/features/editor/ItemShape";
import { TextEditor } from "@/features/editor/TextEditor";
import { ActionBar, ToolColumn } from "@/features/editor/Toolbar";
import { measureText } from "@/features/editor/measure";
import {
  type DrawTool,
  type Item,
  type ItemOf,
  type PixelRect,
  type Point,
  isMeaningful,
  itemBounds,
  newId,
  nextStepNumber,
  rectFromPoints,
  translateItem,
} from "@/features/editor/model";
import { type ToolStyle, useEditor } from "@/features/editor/store";
import type { SessionView } from "@/shared/ipc";

import { Magnifier } from "./Magnifier";
import { ModeBar } from "./ModeBar";
import { snapCrop, useCapture, windowAt } from "./store";

type Gesture =
  // `prev`: the selection that was active when a new drag began outside it.
  // It is kept until the drag is big enough to count, and restored otherwise.
  | { kind: "region"; start: Point; prev?: PixelRect }
  | { kind: "pick"; start: Point }
  | { kind: "move-crop"; start: Point; orig: PixelRect }
  | { kind: "resize-crop"; handle: Handle; orig: PixelRect }
  | { kind: "draw"; start: Point }
  | { kind: "move-item"; id: string; start: Point };

type Handle = "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw";

const HANDLE_CURSOR: Record<Handle, string> = {
  n: "ns-resize",
  s: "ns-resize",
  e: "ew-resize",
  w: "ew-resize",
  ne: "nesw-resize",
  sw: "nesw-resize",
  nw: "nwse-resize",
  se: "nwse-resize",
};

/** Drag one edge/corner of `orig` to `p`; edges may cross (the rect flips). */
function resizeRect(orig: PixelRect, h: Handle, p: Point): PixelRect {
  let x0 = orig.x;
  let y0 = orig.y;
  let x1 = orig.x + orig.width;
  let y1 = orig.y + orig.height;
  if (h.includes("w")) x0 = p[0];
  if (h.includes("e")) x1 = p[0];
  if (h.includes("n")) y0 = p[1];
  if (h.includes("s")) y1 = p[1];
  return rectFromPoints([x0, y0], [x1, y1]);
}

const MIN_REGION = 4;

function startDraft(tool: DrawTool, p: Point, s: ToolStyle, step: number): Item | null {
  const id = newId();
  const box = { id, x: p[0], y: p[1], width: 0, height: 0 };
  switch (tool) {
    case "arrow":
    case "line":
      return { id, type: tool, x1: p[0], y1: p[1], x2: p[0], y2: p[1], color: s.color, strokeWidth: s.strokeWidth };
    case "rect":
    case "ellipse":
      return { ...box, type: tool, color: s.color, strokeWidth: s.strokeWidth };
    case "pen":
      return { id, type: "pen", points: [p], color: s.color, strokeWidth: s.strokeWidth };
    case "highlight":
      return { id, type: "highlight", points: [p], color: s.highlightColor, strokeWidth: s.highlightWidth };
    case "step":
      return { id, type: "step", x: p[0], y: p[1], radius: s.stepRadius, number: step, color: s.color };
    case "redact":
      return { ...box, type: "redact", color: s.redactColor };
    case "blur":
      return { ...box, type: "blur", strength: s.blurStrength };
    case "pixelate":
      return { ...box, type: "pixelate", strength: s.pixelSize };
    case "text":
      return null;
  }
}

function updateDraft(d: Item, start: Point, p: Point): Item {
  switch (d.type) {
    case "arrow":
    case "line":
      return { ...d, x2: p[0], y2: p[1] };
    case "step":
      // Dragging a new marker positions it.
      return { ...d, x: p[0], y: p[1] };
    case "pen":
    case "highlight": {
      const last = d.points[d.points.length - 1];
      return Math.hypot(p[0] - last[0], p[1] - last[1]) < 1 ? d : { ...d, points: [...d.points, p] };
    }
    case "text":
      return d;
    default:
      return { ...d, ...rectFromPoints(start, p) };
  }
}

const inside = (p: Point, r: PixelRect) => p[0] >= r.x && p[0] <= r.x + r.width && p[1] >= r.y && p[1] <= r.y + r.height;

interface StageProps {
  session: SessionView;
  img: HTMLImageElement | null;
  onClaim: () => void;
  onCopy: () => void;
  onCopyText: () => void;
  onSave: () => void;
  onQuickSave: () => void;
  onCancel: () => void;
}

export function Stage({ session, img, onClaim, onCopy, onCopyText, onSave, onQuickSave, onCancel }: StageProps) {
  const W = session.monitor.pixelWidth;
  const H = session.monitor.pixelHeight;

  // Stable action callbacks so the memoised action bar does not re-render on
  // every frame of a drag (the parent passes fresh closures each render).
  const actions = useRef({ onCopy, onCopyText, onSave, onQuickSave, onCancel });
  useLayoutEffect(() => {
    actions.current = { onCopy, onCopyText, onSave, onQuickSave, onCancel };
  });
  const copy = useCallback(() => actions.current.onCopy(), []);
  const copyText = useCallback(() => actions.current.onCopyText(), []);
  const save = useCallback(() => actions.current.onSave(), []);
  const quickSave = useCallback(() => actions.current.onQuickSave(), []);
  const cancel = useCallback(() => actions.current.onCancel(), []);

  const phase = useCapture((s) => s.phase);
  const mode = useCapture((s) => s.mode);
  const crop = useCapture((s) => s.crop);
  const setCrop = useCapture((s) => s.setCrop);
  const setPhase = useCapture((s) => s.setPhase);

  const magnifierOn = useCapture((s) => s.magnifier);
  const tool = useEditor((s) => s.tool);
  const items = useEditor((s) => s.items);
  const selectedId = useEditor((s) => s.selectedId);

  const [viewport, setViewport] = useState({ w: window.innerWidth, h: window.innerHeight });
  const [regionDraft, setRegionDraft] = useState<PixelRect | null>(null);
  const [hover, setHover] = useState<PixelRect | null>(null);
  const [draft, setDraftState] = useState<Item | null>(null);
  // Mirror for event handlers; side effects must never run inside updaters.
  const draftRef = useRef<Item | null>(null);
  const setDraft = (d: Item | null) => {
    draftRef.current = d;
    setDraftState(d);
  };
  const [moveOffset, setMoveOffset] = useState<{ id: string; dx: number; dy: number } | null>(null);
  // Crop as it was when a resize began; annotations stay pinned to it on screen.
  const [resizeOrig, setResizeOrig] = useState<PixelRect | null>(null);
  // Crop as it was when a move began. Blur/pixelate previews keep sampling
  // from it during the drag (they are costly to recompute every frame) and
  // refresh once on release.
  const [moveOrig, setMoveOrig] = useState<PixelRect | null>(null);
  const [textEdit, setTextEdit] = useState<{ item: ItemOf<"text">; isNew: boolean } | null>(null);
  const gesture = useRef<Gesture | null>(null);
  const moveFrame = useRef<number | null>(null);
  const pendingMove = useRef<{ clientX: number; clientY: number } | null>(null);
  // Pointer in image px while the magnifier is relevant, else null.
  const [pointer, setPointer] = useState<Point | null>(null);
  const columnRef = useRef<HTMLDivElement>(null);
  const barRef = useRef<HTMLDivElement>(null);
  const [columnSize, setColumnSize] = useState({ w: 38, h: 460 });
  const [barSize, setBarSize] = useState({ w: 150, h: 38 });

  useLayoutEffect(() => {
    const onResize = () => setViewport({ w: window.innerWidth, h: window.innerHeight });
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("resize", onResize);
      if (moveFrame.current !== null) window.cancelAnimationFrame(moveFrame.current);
    };
  }, []);

  // Image px per CSS px. Derived from the live window size, never from the
  // OS scale factor, so preview and pixels can't disagree (PRD §8.3 "Avoid").
  const sx = W / viewport.w;
  const sy = H / viewport.h;
  const editing = phase === "editing" || phase === "exporting" || phase === "reading" || phase === "done";

  useLayoutEffect(() => {
    const c = columnRef.current;
    const b = barRef.current;
    if (c) setColumnSize((s) => (s.w === c.offsetWidth && s.h === c.offsetHeight ? s : { w: c.offsetWidth, h: c.offsetHeight }));
    if (b) setBarSize((s) => (s.w === b.offsetWidth && s.h === b.offsetHeight ? s : { w: b.offsetWidth, h: b.offsetHeight }));
  }, [editing, tool]);

  const toImage = (e: { clientX: number; clientY: number }): Point => [
    Math.min(W, Math.max(0, e.clientX * sx)),
    Math.min(H, Math.max(0, e.clientY * sy)),
  ];

  // While resizing, annotations stay pinned on screen, so the start crop is
  // exactly right; while moving, the previews ride along until release.
  const previewCrop = resizeOrig ?? moveOrig ?? crop;
  const src: PreviewSource = useMemo(
    () => ({ img, url: session.previewUrl, imageWidth: W, imageHeight: H, crop: previewCrop ?? { x: 0, y: 0, width: W, height: H } }),
    [img, session.previewUrl, W, H, previewCrop],
  );

  const pickAt = (p: Point): PixelRect | null => (mode === "display" ? { x: 0, y: 0, width: W, height: H } : windowAt(session.windows, p[0], p[1]));

  const beginEditing = (r: PixelRect) => {
    const snapped = snapCrop(r, W, H);
    if (snapped.width < 1 || snapped.height < 1) return;
    setCrop(snapped);
    setHover(null);
    setPhase("editing");
  };

  // A press outside the selection may start a new one, but the current
  // selection stays until the drag is at least MIN_REGION; a click never hides it.
  const restartRegion = (p: Point, e: ReactPointerEvent) => {
    gesture.current = { kind: "region", start: p, prev: crop ?? undefined };
    e.currentTarget.setPointerCapture(e.pointerId);
  };

  const openText = (p: Point) => {
    const s = useEditor.getState().style;
    setTextEdit({
      isNew: true,
      item: { id: newId(), type: "text", x: p[0], y: p[1] - s.fontSize * 0.6, text: "", color: s.color, fontSize: s.fontSize },
    });
  };

  const onPointerDown = (e: ReactPointerEvent<SVGSVGElement>) => {
    if (e.button !== 0) return;
    const p = toImage(e);

    if (phase === "selecting") {
      onClaim();
      if (mode === "region") {
        gesture.current = { kind: "region", start: p };
        setRegionDraft({ x: p[0], y: p[1], width: 0, height: 0 });
      } else {
        gesture.current = { kind: "pick", start: p };
      }
      e.currentTarget.setPointerCapture(e.pointerId);
      return;
    }
    if (phase !== "editing" || !crop) return;
    // A click while typing just confirms the text (the textarea's blur does it).
    if (textEdit) return;

    // Handles and edge lines resize the area with any tool active.
    const handle = (e.target as Element).closest("[data-handle]")?.getAttribute("data-handle") as Handle | null;
    if (handle) {
      gesture.current = { kind: "resize-crop", handle, orig: crop };
      setResizeOrig(crop);
      e.currentTarget.setPointerCapture(e.pointerId);
      return;
    }

    const ed = useEditor.getState();
    const local: Point = [p[0] - crop.x, p[1] - crop.y];
    const inCrop = inside(p, crop);
    const hitId = (e.target as Element).closest("[data-item-id]")?.getAttribute("data-item-id") ?? null;
    const empty = ed.items.length === 0;

    if (tool === "select") {
      if (hitId) {
        ed.select(hitId);
        gesture.current = { kind: "move-item", id: hitId, start: p };
      } else if (inCrop) {
        // Drag empty space inside the area to move it (annotations ride along).
        ed.select(null);
        gesture.current = { kind: "move-crop", start: p, orig: crop };
        setMoveOrig(crop);
      } else if (empty) {
        restartRegion(p, e);
        return;
      } else {
        ed.select(null);
      }
    } else if (tool === "text") {
      const hit = hitId ? ed.items.find((i) => i.id === hitId) : undefined;
      if (hit?.type === "text") {
        ed.select(hit.id);
        setTextEdit({ item: hit, isNew: false });
      } else if (inCrop) {
        openText(local);
      } else if (empty) {
        restartRegion(p, e);
      }
      return;
    } else {
      if (!inCrop && empty) {
        restartRegion(p, e);
        return;
      }
      const d = startDraft(tool, local, ed.style, nextStepNumber(ed.items));
      if (!d) return;
      ed.select(null);
      setDraft(d);
      gesture.current = { kind: "draw", start: local };
    }
    e.currentTarget.setPointerCapture(e.pointerId);
  };

  const processPointerMove = (e: { clientX: number; clientY: number }) => {
    const p = toImage(e);
    const g = gesture.current;
    if (magnifierOn) {
      const loupe = (phase === "selecting" && mode === "region") || g?.kind === "resize-crop";
      setPointer(loupe ? p : null);
    }
    if (!g) {
      if (phase === "selecting" && mode !== "region") setHover(pickAt(p));
      return;
    }
    switch (g.kind) {
      case "region": {
        const r = rectFromPoints(g.start, p);
        if (g.prev && useCapture.getState().crop) {
          if (r.width < MIN_REGION || r.height < MIN_REGION) break;
          setCrop(null);
          setPhase("selecting");
        }
        setRegionDraft(r);
        break;
      }
      case "pick":
        setHover(pickAt(p));
        break;
      case "move-crop": {
        const dx = Math.round(p[0] - g.start[0]);
        const dy = Math.round(p[1] - g.start[1]);
        setCrop({
          ...g.orig,
          x: Math.min(W - g.orig.width, Math.max(0, g.orig.x + dx)),
          y: Math.min(H - g.orig.height, Math.max(0, g.orig.y + dy)),
        });
        break;
      }
      case "resize-crop": {
        const r = snapCrop(resizeRect(g.orig, g.handle, p), W, H);
        if (r.width >= 1 && r.height >= 1) setCrop(r);
        break;
      }
      case "draw":
        if (crop && draftRef.current) setDraft(updateDraft(draftRef.current, g.start, [p[0] - crop.x, p[1] - crop.y]));
        break;
      case "move-item":
        setMoveOffset({ id: g.id, dx: p[0] - g.start[0], dy: p[1] - g.start[1] });
        break;
    }
  };

  // Pointer devices can deliver events faster than the display refreshes.
  // Coalesce them so the SVG and magnifier render at most once per frame.
  const onPointerMove = (e: ReactPointerEvent<SVGSVGElement>) => {
    pendingMove.current = { clientX: e.clientX, clientY: e.clientY };
    if (moveFrame.current !== null) return;
    moveFrame.current = window.requestAnimationFrame(() => {
      moveFrame.current = null;
      const pending = pendingMove.current;
      pendingMove.current = null;
      if (pending) processPointerMove(pending);
    });
  };

  const onPointerUp = (e: ReactPointerEvent<SVGSVGElement>) => {
    if (moveFrame.current !== null) {
      window.cancelAnimationFrame(moveFrame.current);
      moveFrame.current = null;
    }
    pendingMove.current = null;
    // Preserve the final coordinates even when the last move was coalesced.
    processPointerMove(e);
    const g = gesture.current;
    gesture.current = null;
    if (!g) return;
    const p = toImage(e);
    switch (g.kind) {
      case "region": {
        const r = rectFromPoints(g.start, p);
        setRegionDraft(null);
        if (r.width >= MIN_REGION && r.height >= MIN_REGION) {
          beginEditing(r);
        } else if (g.prev) {
          // Just a click (or a tiny drag) outside the selection: keep it.
          setCrop(g.prev);
          setPhase("editing");
        } else {
          // A plain click selects the window under the pointer (zero-area
          // drags are otherwise rejected, CAP-02), unless turned off in Settings.
          const w = useCapture.getState().autoSelectWindow ? windowAt(session.windows, p[0], p[1]) : null;
          if (w) beginEditing(w);
        }
        break;
      }
      case "pick": {
        const r = pickAt(p);
        if (r) beginEditing(r);
        break;
      }
      case "draw": {
        const d = draftRef.current;
        setDraft(null);
        if (d && isMeaningful(d)) useEditor.getState().addItem(d);
        break;
      }
      case "move-item": {
        const ed = useEditor.getState();
        const item = ed.items.find((i) => i.id === g.id);
        const dx = p[0] - g.start[0];
        const dy = p[1] - g.start[1];
        if (item && (Math.abs(dx) > 0.5 || Math.abs(dy) > 0.5)) ed.replaceItem(translateItem(item, dx, dy));
        setMoveOffset(null);
        break;
      }
      case "move-crop":
        setMoveOrig(null);
        break;
      case "resize-crop": {
        setPointer(null);
        const cur = useCapture.getState().crop;
        if (cur) useEditor.getState().shiftItems(g.orig.x - cur.x, g.orig.y - cur.y);
        setResizeOrig(null);
        break;
      }
    }
  };

  const onDoubleClick = (e: React.MouseEvent<SVGSVGElement>) => {
    if (tool !== "select" || phase !== "editing") return;
    const id = (e.target as Element).closest("[data-item-id]")?.getAttribute("data-item-id");
    const item = useEditor.getState().items.find((i) => i.id === id);
    if (item?.type === "text") setTextEdit({ item, isNew: false });
  };

  const commitText = (text: string) => {
    const t = textEdit;
    setTextEdit(null);
    if (!t) return;
    const ed = useEditor.getState();
    const next = { ...t.item, text: text.replace(/\s+$/, "") };
    if (t.isNew) {
      if (isMeaningful(next)) ed.addItem(next);
    } else if (!isMeaningful(next)) {
      ed.removeItem(next.id);
    } else if (next.text !== t.item.text) {
      ed.replaceItem(next);
    }
  };

  // ---------------------------------------------------------- rendering ---

  const active = regionDraft ?? crop ?? hover;
  const dimPath = active
    ? `M0 0H${W}V${H}H0Z M${active.x} ${active.y}h${active.width}v${active.height}h${-active.width}Z`
    : `M0 0H${W}V${H}H0Z`;

  const selected = items.find((i) => i.id === selectedId);
  const renderItem = (i: Item) => (moveOffset?.id === i.id ? translateItem(i, moveOffset.dx, moveOffset.dy) : i);
  const hitEnabled = tool === "select" || tool === "text";
  // Crop-relative, so a selection drag never needs to re-measure them.
  const hitAreas = useMemo(
    () => (
      <g>
        {items
          .filter((i) => tool === "select" || i.type === "text")
          .map((i) => {
            const it = moveOffset?.id === i.id ? translateItem(i, moveOffset.dx, moveOffset.dy) : i;
            return <HitArea key={i.id} item={it} bounds={itemBounds(it, measureText)} scale={sx} />;
          })}
      </g>
    ),
    [items, tool, moveOffset, sx],
  );
  const shift = resizeOrig && crop ? `translate(${resizeOrig.x - crop.x} ${resizeOrig.y - crop.y})` : undefined;

  const cursor =
    phase === "selecting"
      ? mode === "region"
        ? "crosshair"
        : "pointer"
      : tool === "select"
        ? "default"
        : tool === "text"
          ? "text"
          : "crosshair";

  // Tool column: right of the selection (bottom-aligned), else left, else
  // inside the right edge. Action bar: below the selection's right edge,
  // else above, else inside the bottom edge.
  const GAP = 6;
  const EDGE = 4;
  const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(v, hi));
  let columnStyle: React.CSSProperties = {};
  let barStyle: React.CSSProperties = {};
  let columnTip: "left" | "right" = "right";
  let barTip: "top" | "bottom" = "bottom";
  let badgeStyle: React.CSSProperties = {};
  if (crop) {
    const left = crop.x / sx;
    const top = crop.y / sy;
    const right = (crop.x + crop.width) / sx;
    const bottom = (crop.y + crop.height) / sy;

    let cLeft: number;
    if (right + GAP + columnSize.w <= viewport.w - EDGE) {
      cLeft = right + GAP;
      columnTip = "right";
    } else if (left - GAP - columnSize.w >= EDGE) {
      cLeft = left - GAP - columnSize.w;
      columnTip = "left";
    } else {
      cLeft = right - GAP - columnSize.w;
      columnTip = "left";
    }
    const cTop = clamp(bottom - columnSize.h, EDGE, viewport.h - columnSize.h - EDGE);
    // transform, not left/top: moving it needs no layout, only compositing.
    columnStyle = { left: 0, top: 0, transform: `translate3d(${Math.max(EDGE, cLeft)}px, ${Math.max(EDGE, cTop)}px, 0)` };

    // Leave room under the bar for the redact/blur note.
    const room = barSize.h + 32;
    let bTop: number;
    if (bottom + GAP + room <= viewport.h) {
      bTop = bottom + GAP;
      barTip = "bottom";
    } else if (top - GAP - barSize.h >= EDGE) {
      bTop = top - GAP - barSize.h;
      barTip = "top";
    } else {
      bTop = bottom - GAP - barSize.h;
      barTip = "top";
    }
    const bLeft = clamp(right - barSize.w, EDGE, viewport.w - barSize.w - EDGE);
    barStyle = { left: 0, top: 0, transform: `translate3d(${bLeft}px, ${bTop}px, 0)` };
    badgeStyle = top >= 28 ? { left, top: top - 26 } : { left: left + 6, top: top + 6 };
  }
  const sizeLabel = regionDraft ?? crop;

  return (
    <>
      <svg
        className="absolute inset-0 size-full touch-none"
        viewBox={`0 0 ${W} ${H}`}
        preserveAspectRatio="none"
        style={{ cursor }}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerUp}
        onPointerLeave={() => setPointer(null)}
        onDoubleClick={onDoubleClick}
        role="application"
        aria-label="Screen capture area"
      >
        <path d={dimPath} fillRule="evenodd" fill={active ? "rgba(0,0,0,0.5)" : "rgba(0,0,0,0.3)"} pointerEvents="none" />

        {crop ? (
          <g transform={`translate(${crop.x} ${crop.y})`}>
            <defs>
              <clipPath id="crop-clip">
                <rect x={0} y={0} width={crop.width} height={crop.height} />
              </clipPath>
            </defs>
            <g clipPath="url(#crop-clip)" pointerEvents="none">
              <g transform={shift}>
                {items.map((i) => (textEdit?.item.id === i.id ? null : <ItemShape key={i.id} item={renderItem(i)} src={src} />))}
                {draft ? <ItemShape item={draft} src={src} /> : null}
              </g>
            </g>
            {hitEnabled && !resizeOrig ? hitAreas : null}
            {selected && !textEdit && !resizeOrig ? (
              <SelectionOutline bounds={itemBounds(renderItem(selected), measureText)} pad={4 * sx} />
            ) : null}
          </g>
        ) : null}

        {active ? (
          <rect x={active.x} y={active.y} width={active.width} height={active.height} fill="none" stroke="#0ea5e9" strokeWidth={2} vectorEffect="non-scaling-stroke" pointerEvents="none" />
        ) : null}

        {crop && phase === "editing" && !textEdit ? <CropHandles crop={crop} sx={sx} sy={sy} /> : null}
      </svg>

      {sizeLabel && sizeLabel.width > 0 ? (
        <div
          className="pointer-events-none absolute rounded bg-black/75 px-1.5 py-0.5 font-mono text-xs text-white tabular-nums"
          style={regionDraft ? { left: regionDraft.x / sx, top: Math.max(4, regionDraft.y / sy - 24) } : badgeStyle}
        >
          {Math.round(sizeLabel.width)} × {Math.round(sizeLabel.height)}
        </div>
      ) : null}

      {phase === "selecting" && !regionDraft ? <ModeBar session={session} /> : null}

      {magnifierOn && pointer && img && ((phase === "selecting" && mode === "region") || resizeOrig) ? (
        <Magnifier img={img} point={pointer} sx={sx} sy={sy} viewport={viewport} />
      ) : null}

      {editing && crop ? (
        <>
          <ToolColumn ref={columnRef} style={columnStyle} tipSide={columnTip} />
          <ActionBar ref={barRef} style={barStyle} tipSide={barTip} busy={phase !== "editing"} reading={phase === "reading"} onCopy={copy} onCopyText={copyText} onSave={save} onQuickSave={quickSave} onCancel={cancel} />
        </>
      ) : null}

      {textEdit && crop ? (
        <TextEditor key={textEdit.item.id} item={textEdit.item} origin={crop} scale={sx} onCommit={commitText} onCancel={() => setTextEdit(null)} />
      ) : null}
    </>
  );
}

/**
 * Eight dots on the crop outline plus invisible grab strips along each edge,
 * so the area resizes from anywhere on its border, not just the dots.
 */
function CropHandles({ crop, sx, sy }: { crop: PixelRect; sx: number; sy: number }) {
  const { x, y, width: w, height: h } = crop;
  const band = 10; // CSS px grab thickness
  const bx = band * sx;
  const by = band * sy;
  const half = 4; // square handle is 8x8 CSS px, centred on the outline

  const edges: { h: Handle; x: number; y: number; w: number; h2: number }[] = [
    { h: "n", x, y: y - by / 2, w, h2: by },
    { h: "s", x, y: y + h - by / 2, w, h2: by },
    { h: "w", x: x - bx / 2, y, w: bx, h2: h },
    { h: "e", x: x + w - bx / 2, y, w: bx, h2: h },
  ];
  const dots: { h: Handle; cx: number; cy: number }[] = [
    { h: "nw", cx: x, cy: y },
    { h: "n", cx: x + w / 2, cy: y },
    { h: "ne", cx: x + w, cy: y },
    { h: "e", cx: x + w, cy: y + h / 2 },
    { h: "se", cx: x + w, cy: y + h },
    { h: "s", cx: x + w / 2, cy: y + h },
    { h: "sw", cx: x, cy: y + h },
    { h: "w", cx: x, cy: y + h / 2 },
  ];

  return (
    <g>
      {edges.map((e) => (
        <rect key={e.h} data-handle={e.h} x={e.x} y={e.y} width={e.w} height={e.h2} fill="transparent" style={{ cursor: HANDLE_CURSOR[e.h] }} />
      ))}
      {dots.map((d) => (
        <g key={d.h} data-handle={d.h} style={{ cursor: HANDLE_CURSOR[d.h] }} transform={`translate(${d.cx} ${d.cy}) scale(${sx} ${sy})`}>
          {/* invisible grab target, larger than the square */}
          <rect x={-band} y={-band} width={band * 2} height={band * 2} fill="transparent" />
          <rect x={-half} y={-half} width={half * 2} height={half * 2} fill="#fff" stroke="#0ea5e9" strokeWidth={1.5} vectorEffect="non-scaling-stroke" />
        </g>
      ))}
    </g>
  );
}

function SelectionOutline({ bounds, pad }: { bounds: PixelRect; pad: number }) {
  return (
    <rect
      x={bounds.x - pad}
      y={bounds.y - pad}
      width={bounds.width + pad * 2}
      height={bounds.height + pad * 2}
      fill="none"
      stroke="#0ea5e9"
      strokeWidth={1.5}
      strokeDasharray="4 3"
      vectorEffect="non-scaling-stroke"
      pointerEvents="none"
    />
  );
}
