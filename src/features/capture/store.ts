import { create } from "zustand";

import type { PixelRect } from "@/features/editor/model";
import type { CaptureMode, SessionView } from "@/shared/ipc";

/** Overlay lifecycle (PRD §5.4) as seen by one monitor's overlay. */
export type Phase = "loading" | "selecting" | "editing" | "exporting" | "done" | "error";

export interface Notice {
  kind: "success" | "error" | "info";
  message: string;
}

interface CaptureState {
  session: SessionView | null;
  phase: Phase;
  mode: CaptureMode;
  /** Selected region in this monitor's pixel space. */
  crop: PixelRect | null;
  notice: Notice | null;
  /** Pixel loupe preference (Settings → showMagnifier). */
  magnifier: boolean;
  setMagnifier: (on: boolean) => void;
  /** Click-to-select the window under the pointer (Settings → autoSelectWindow). */
  autoSelectWindow: boolean;
  setAutoSelectWindow: (on: boolean) => void;
  setSession: (s: SessionView) => void;
  setPhase: (p: Phase) => void;
  setMode: (m: CaptureMode) => void;
  setCrop: (c: PixelRect | null) => void;
  setNotice: (n: Notice | null) => void;
  reset: (mode?: CaptureMode) => void;
}

export const useCapture = create<CaptureState>((set) => ({
  session: null,
  phase: "loading",
  mode: "region",
  crop: null,
  notice: null,
  magnifier: true,
  setMagnifier: (magnifier) => set({ magnifier }),
  autoSelectWindow: true,
  setAutoSelectWindow: (autoSelectWindow) => set({ autoSelectWindow }),
  setSession: (session) => set({ session, mode: session.mode }),
  setPhase: (phase) => set({ phase }),
  setMode: (mode) => set({ mode }),
  setCrop: (crop) => set({ crop }),
  setNotice: (notice) => set({ notice }),
  reset: (mode = "region") => set({ session: null, phase: "loading", mode, crop: null, notice: null }),
}));

/** Round a float rect to whole pixels and clamp it to the monitor. */
export function snapCrop(r: PixelRect, maxW: number, maxH: number): PixelRect {
  const x0 = Math.max(0, Math.floor(r.x));
  const y0 = Math.max(0, Math.floor(r.y));
  const x1 = Math.min(maxW, Math.ceil(r.x + r.width));
  const y1 = Math.min(maxH, Math.ceil(r.y + r.height));
  return { x: x0, y: y0, width: Math.max(0, x1 - x0), height: Math.max(0, y1 - y0) };
}

/** Topmost window rect containing a pixel point (list is sorted topmost-first). */
export function windowAt(windows: SessionView["windows"], x: number, y: number): PixelRect | null {
  for (const w of windows) {
    const r = w.rect;
    if (x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height) return r;
  }
  return null;
}
