import { useEffect, useState } from "react";
import { flushSync } from "react-dom";

import { loadAnnotationFont } from "@/features/editor/measure";
import type { OverlayBoot } from "@/shared/ipc";

// Eager within the overlay chunk: prewarming loads the full editor, even while
// no capture is mounted. Settings windows never import this chunk.
import OverlayApp from "./OverlayApp";

const CAPTURE_START_EVENT = "fastshot:capture-start";
const CAPTURE_END_EVENT = "fastshot:capture-end";

/**
 * Overlay WebViews stay alive while hidden so subsequent captures do not pay
 * WebView startup cost. Rust updates the session with a small DOM event.
 */
export default function OverlayHost() {
  const [boot, setBoot] = useState<OverlayBoot | null>(window.__FASTSHOT_OVERLAY__ ?? null);

  useEffect(() => {
    // Load reusable assets while hidden; no screenshot session is requested.
    void loadAnnotationFont();
    const onCaptureStart = (event: Event) => {
      setBoot((event as CustomEvent<OverlayBoot>).detail);
    };
    // Unmount the editor when its session ends so the hidden WebView releases
    // the decoded frame. Ignore a late end event for an older session.
    const onCaptureEnd = (event: Event) => {
      const sessionId = (event as CustomEvent<{ sessionId?: string } | null>).detail?.sessionId;
      if (!sessionId) return;
      // Release the decoded image and edits now, even in a hidden window.
      flushSync(() => setBoot((current) => (current?.sessionId === sessionId ? null : current)));
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

