import { StrictMode, Suspense, lazy, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";

import type { OverlayBoot } from "@/shared/ipc";

import "./index.css";

// Each window loads only its own UI: overlays must open fast, and the
// settings window never needs the editor code (bundle-conditional).
const OverlayApp = lazy(() => import("./features/capture/OverlayApp"));
const SettingsApp = lazy(() => import("./features/settings/SettingsApp"));

const CAPTURE_START_EVENT = "fastshot:capture-start";
const CAPTURE_END_EVENT = "fastshot:capture-end";

/**
 * Overlay WebViews stay alive while hidden so subsequent captures do not pay
 * WebView startup cost. Rust updates the session with a small DOM event.
 */
function OverlayHost() {
  const [boot, setBoot] = useState<OverlayBoot | null>(window.__FASTSHOT_OVERLAY__ ?? null);

  useEffect(() => {
    const onCaptureStart = (event: Event) => {
      setBoot((event as CustomEvent<OverlayBoot>).detail);
    };
    // Unmount the editor when its session ends so the hidden WebView releases
    // the decoded frame. Ignore a late end event for an older session.
    const onCaptureEnd = (event: Event) => {
      const sessionId = (event as CustomEvent<{ sessionId?: string } | null>).detail?.sessionId;
      if (!sessionId) return;
      setBoot((current) => (current?.sessionId === sessionId ? null : current));
    };
    window.addEventListener(CAPTURE_START_EVENT, onCaptureStart);
    window.addEventListener(CAPTURE_END_EVENT, onCaptureEnd);
    // An event may have fired between the first render and subscribing. Rust
    // updates the global before every event, so it holds the latest state.
    const latest = window.__FASTSHOT_OVERLAY__ ?? null;
    setBoot((current) =>
      current?.sessionId === latest?.sessionId && current?.monitorIndex === latest?.monitorIndex ? current : latest,
    );
    return () => {
      window.removeEventListener(CAPTURE_START_EVENT, onCaptureStart);
      window.removeEventListener(CAPTURE_END_EVENT, onCaptureEnd);
    };
  }, []);

  return boot ? <OverlayApp key={`${boot.sessionId}:${boot.monitorIndex}`} boot={boot} /> : null;
}

const isOverlay = window.__FASTSHOT_OVERLAY_WINDOW__ === true || window.__FASTSHOT_OVERLAY__ != null;
document.documentElement.dataset.view = isOverlay ? "overlay" : "settings";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Suspense fallback={null}>{isOverlay ? <OverlayHost /> : <SettingsApp />}</Suspense>
  </StrictMode>,
);
