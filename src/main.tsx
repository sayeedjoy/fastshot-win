import { StrictMode, Suspense, lazy } from "react";
import { createRoot } from "react-dom/client";

import "./index.css";

// Each window loads only its own UI: overlays must open fast, and the
// settings window never needs the editor code (bundle-conditional).
const OverlayHost = lazy(() => import("./features/capture/OverlayHost"));
const SettingsApp = lazy(() => import("./features/settings/SettingsApp"));

const isOverlay = window.__FASTSHOT_OVERLAY_WINDOW__ === true || window.__FASTSHOT_OVERLAY__ != null;
document.documentElement.dataset.view = isOverlay ? "overlay" : "settings";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Suspense fallback={null}>{isOverlay ? <OverlayHost /> : <SettingsApp />}</Suspense>
  </StrictMode>,
);
