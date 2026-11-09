import { StrictMode, Suspense, lazy, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";

import type { OverlayBoot } from "@/shared/ipc";

import "./index.css";

// Each window loads only its own UI: overlays must open fast, and the
// settings window never needs the editor code (bundle-conditional).
const OverlayApp = lazy(() => import("./features/capture/OverlayApp"));
const SettingsApp = lazy(() => import("./features/settings/SettingsApp"));

const CAPTURE_START_EVENT = "fastshot:capture-start";

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
    window.addEventListener(CAPTURE_START_EVENT, onCaptureStart);
    return () => window.removeEventListener(CAPTURE_START_EVENT, onCaptureStart);
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
