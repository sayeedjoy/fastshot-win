import { describe, expect, it } from "vitest";

import { snapCrop, useCapture, windowAt } from "./store";
import type { SessionView } from "@/shared/ipc";

describe("capture lifecycle", () => {
  const session: SessionView = {
    sessionId: "native-session", monitorIndex: 1, monitorCount: 2, mode: "region",
    monitor: { name: "Second display", isPrimary: false, scaleFactor: 1.5, pixelWidth: 2560, pixelHeight: 1440 },
    windows: [], previewUrl: "fastshot://preview/native-session/1",
  };

  it("opens the full editor at the native selection without selecting again", () => {
    const crop = { x: 143, y: 82, width: 713, height: 411 };
    useCapture.getState().reset();
    useCapture.getState().setSession({ ...session, initialCrop: crop });
    expect(useCapture.getState()).toMatchObject({ crop, phase: "editing" });
    useCapture.getState().reset();
    expect(useCapture.getState()).toMatchObject({ crop: null, session: null });
  });

  it("starts ordinary captures in selection mode without a stale native crop", () => {
    useCapture.getState().setCrop({ x: 1, y: 2, width: 3, height: 4 });
    useCapture.getState().setSession(session);
    expect(useCapture.getState()).toMatchObject({ crop: null, phase: "selecting" });
  });
  it("clears stale UI state when a warm overlay starts a new session", () => {
    const state = useCapture.getState();
    state.setMode("window");
    state.setCrop({ x: 10, y: 20, width: 30, height: 40 });
    state.setNotice({ kind: "error", message: "old session" });
    state.setPhase("editing");

    useCapture.getState().reset("display");

    expect(useCapture.getState()).toMatchObject({
      session: null,
      phase: "loading",
      mode: "display",
      crop: null,
      notice: null,
    });
  });
});

describe("snapCrop", () => {
  it("rounds outward and clamps to the monitor", () => {
    expect(snapCrop({ x: -3.2, y: 10.6, width: 50.1, height: 2000 }, 1920, 1080)).toEqual({ x: 0, y: 10, width: 47, height: 1070 });
  });
  it("keeps whole-pixel rects unchanged", () => {
    expect(snapCrop({ x: 10, y: 20, width: 30, height: 40 }, 100, 100)).toEqual({ x: 10, y: 20, width: 30, height: 40 });
  });
});

describe("windowAt", () => {
  const windows = [
    { monitorIndex: 0, rect: { x: 100, y: 100, width: 50, height: 50 }, z: 9, appName: "Top" },
    { monitorIndex: 0, rect: { x: 0, y: 0, width: 500, height: 500 }, z: 1, appName: "Below" },
  ];
  it("returns the topmost window under the point", () => {
    expect(windowAt(windows, 120, 120)).toEqual(windows[0].rect);
    expect(windowAt(windows, 10, 10)).toEqual(windows[1].rect);
    expect(windowAt(windows, 900, 900)).toBeNull();
  });
});
