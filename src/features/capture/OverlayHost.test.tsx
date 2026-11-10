import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useEditor } from "@/features/editor/store";
import type { OverlayBoot, SessionView } from "@/shared/ipc";

import OverlayHost from "./OverlayHost";
import { useCapture } from "./store";

const api = vi.hoisted(() => ({
  getSession: vi.fn(), getSettings: vi.fn(), overlayReady: vi.fn(), cancelCapture: vi.fn(),
}));
vi.mock("@/shared/ipc", () => ({ api, errorMessage: String }));
vi.mock("./Stage", () => ({ Stage: () => <div data-testid="editor-tools" /> }));
vi.mock("@/shared/theme", () => ({ isMac: false, setThemePreference: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => undefined), emit: vi.fn(async () => undefined),
}));

const boot: OverlayBoot = { sessionId: "first", monitorIndex: 0, mode: "region" };
const session: SessionView = {
  ...boot, monitorCount: 1, windows: [], previewUrl: "http://fastshot.localhost/first/0",
  monitor: { name: "Display", isPrimary: true, scaleFactor: 1, pixelWidth: 800, pixelHeight: 600 },
};

let root: Root;
let container: HTMLDivElement;
let fontLoad: ReturnType<typeof vi.fn>;

async function start(next = boot) {
  window.__FASTSHOT_OVERLAY__ = next;
  await act(async () => {
    window.dispatchEvent(new CustomEvent("fastshot:capture-start", { detail: next }));
  });
}

beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  delete window.__FASTSHOT_OVERLAY__;
  useCapture.getState().reset();
  useEditor.getState().reset();
  api.getSession.mockImplementation(async (id: string) => ({
    ...session, sessionId: id, previewUrl: `http://fastshot.localhost/${id}/0`,
  }));
  api.getSettings.mockResolvedValue({ theme: "light" });
  api.overlayReady.mockResolvedValue(undefined);
  fontLoad = vi.fn(async () => []);
  Object.defineProperty(document, "fonts", { configurable: true, value: { load: fontLoad } });
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
});

afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
  delete window.__FASTSHOT_OVERLAY__;
  vi.clearAllMocks();
  vi.unstubAllGlobals();
});

describe("warm capture host", () => {
  it("loads the annotation font while idle without retaining a screenshot", async () => {
    await act(async () => root.render(<OverlayHost />));
    expect(fontLoad).toHaveBeenCalled();
    expect(api.getSession).not.toHaveBeenCalled();
    expect(container.querySelector("img")).toBeNull();
    expect(useCapture.getState().session).toBeNull();
  });

  it("clears the image, selection, annotations and undo history when capture ends", async () => {
    await act(async () => root.render(<OverlayHost />));
    await start();
    expect(container.querySelector("img")).not.toBeNull();
    act(() => {
      useCapture.getState().setCrop({ x: 20, y: 30, width: 80, height: 60 });
      useEditor.getState().addItem({ id: "private-note", type: "text", x: 0, y: 0, text: "private", fontSize: 24, color: "#000000" });
    });
    act(() => {
      delete window.__FASTSHOT_OVERLAY__;
      window.dispatchEvent(new CustomEvent("fastshot:capture-end", { detail: { sessionId: "first" } }));
      // Cleanup is synchronous; it does not depend on another animation frame.
      expect(container.querySelector("img")).toBeNull();
      expect(useCapture.getState()).toMatchObject({ session: null, crop: null });
      expect(useEditor.getState()).toMatchObject({ items: [], past: [], future: [], editingTextId: null });
    });
    await start({ ...boot, sessionId: "second" });
    expect(useCapture.getState().session?.sessionId).toBe("second");
    expect(container.querySelector("img")?.getAttribute("src")).toBe("http://fastshot.localhost/second/0");
  });

  it("ignores a late capture-end from a previous session", async () => {
    await act(async () => root.render(<OverlayHost />));
    await start();
    await start({ ...boot, sessionId: "second" });
    act(() => {
      window.dispatchEvent(new CustomEvent("fastshot:capture-end", { detail: { sessionId: "first" } }));
    });
    expect(useCapture.getState().session?.sessionId).toBe("second");
    expect(container.querySelector("img")).not.toBeNull();
  });
});
