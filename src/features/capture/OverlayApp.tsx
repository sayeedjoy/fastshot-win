import { emit, listen } from "@tauri-apps/api/event";
import { CircleCheck, Info, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import { TooltipProvider } from "@/components/ui/tooltip";
import { keyAction } from "@/features/editor/keyboard";
import { clearMeasureCache, loadAnnotationFont } from "@/features/editor/measure";
import { toRenderRequest } from "@/features/editor/model";
import { useEditor } from "@/features/editor/store";
import { type CaptureMode, type Destination, type OverlayBoot, api, errorMessage } from "@/shared/ipc";
import { isMac, setThemePreference } from "@/shared/theme";

import { Stage } from "./Stage";
import { useCapture } from "./store";

const CLAIM_EVENT = "overlay-claim";
const MODE_KEYS: Record<string, CaptureMode> = { "1": "region", "2": "window", "3": "display" };

function isTyping(el: Element | null): boolean {
  return !!el && (el.tagName === "TEXTAREA" || el.tagName === "INPUT" || (el as HTMLElement).isContentEditable);
}

export default function OverlayApp({ boot }: { boot: OverlayBoot }) {
  const session = useCapture((s) => s.session);
  const phase = useCapture((s) => s.phase);
  const notice = useCapture((s) => s.notice);
  const [img, setImg] = useState<HTMLImageElement | null>(null);
  const readySent = useRef(false);

  const cancel = useCallback(() => {
    void api.cancelCapture(boot.sessionId);
  }, [boot.sessionId]);

  // Fetch the session first so the preview can begin loading immediately.
  // Preferences and the annotation font are not needed during selection and
  // must never delay the overlay becoming visible.
  useEffect(() => {
    let alive = true;
    useCapture.getState().reset(boot.mode);
    useEditor.getState().reset();

    api
      .getSession(boot.sessionId, boot.monitorIndex)
      .then((s) => {
        if (!alive) return;
        useCapture.getState().setSession(s);
        useCapture.getState().setPhase("selecting");
      })
      .catch((e) => {
        if (!alive) return;
        useCapture.getState().setNotice({ kind: "error", message: errorMessage(e) });
        useCapture.getState().setPhase("error");
        void api.overlayReady(boot.sessionId, boot.monitorIndex).catch(() => cancel());
      });

    void api
      .getSettings()
      .then((prefs) => {
        if (!alive) return;
        setThemePreference(prefs.theme);
        useCapture.getState().setMagnifier(prefs.showMagnifier);
        useCapture.getState().setAutoSelectWindow(prefs.autoSelectWindow);
        // Settings load independently of the frozen frame. Merge the defaults
        // without resetting items that the user may already have drawn.
        useEditor.setState((state) => ({
          style: {
            ...state.style,
            color: prefs.annotationColor,
            strokeWidth: prefs.strokeWidth,
            fontSize: prefs.fontSize,
            redactColor: prefs.redactColor,
          },
        }));
      })
      .catch(() => undefined);
    void loadAnnotationFont();

    // OverlayApp is keyed by session, so this runs only on unmount. The
    // stores and the text-measure cache outlive the component: clear them so
    // the selection and edits (including typed text) don't linger.
    return () => {
      alive = false;
      useCapture.getState().reset();
      useEditor.getState().reset();
      clearMeasureCache();
    };
  }, [boot.sessionId, boot.monitorIndex, cancel]);

  /** False once this session's editor is gone (ended or replaced). */
  const isCurrent = useCallback(() => useCapture.getState().session?.sessionId === boot.sessionId, [boot.sessionId]);

  // Only one monitor can hold a selection; the others reset when it's claimed.
  useEffect(() => {
    const un = listen<{ sessionId: string; monitorIndex: number }>(CLAIM_EVENT, (e) => {
      if (e.payload.sessionId !== boot.sessionId || e.payload.monitorIndex === boot.monitorIndex) return;
      const c = useCapture.getState();
      if (c.crop || c.phase === "editing") {
        c.setCrop(null);
        c.setPhase("selecting");
        useEditor.getState().reset(useEditor.getState().style);
      }
    });
    return () => {
      void un.then((f) => f());
    };
  }, [boot.sessionId, boot.monitorIndex]);

  const claim = useCallback(() => {
    void emit(CLAIM_EVENT, { sessionId: boot.sessionId, monitorIndex: boot.monitorIndex });
  }, [boot.sessionId, boot.monitorIndex]);

  const doExport = useCallback(
    async (destination: Destination) => {
      // Commit any open text field first; its blur handler adds the item.
      if (isTyping(document.activeElement)) (document.activeElement as HTMLElement).blur();
      const c = useCapture.getState();
      if (!c.crop || !c.session || c.phase !== "editing") return;
      c.setPhase("exporting");
      c.setNotice(null);
      const request = {
        sessionId: c.session.sessionId,
        monitorIndex: c.session.monitorIndex,
        destination,
        ...toRenderRequest(c.crop, useEditor.getState().items),
      };
      if (destination === "clipboard") {
        // Rust hides the overlay immediately and copies in the background;
        // any failure is reported in the main window.
        void api.exportCapture(request).catch(() => undefined);
        return;
      }
      try {
        const res = await api.exportCapture(request);
        // The session may have ended (Esc, tray) while the dialog was open.
        if (!isCurrent()) return;
        if (!res.completed) {
          c.setPhase("editing");
          return;
        }
        c.setPhase("done");
        c.setNotice({
          kind: "success",
          message: `Saved to ${res.path}`,
        });
        window.setTimeout(cancel, 1600);
      } catch (e) {
        if (!isCurrent()) return;
        c.setPhase("editing");
        c.setNotice({ kind: "error", message: errorMessage(e) });
      }
    },
    [cancel, isCurrent],
  );

  // OCR runs on the same rendered raster as Copy/Save, so redacted text is
  // never read. The overlay stays up until the text is on the clipboard.
  const copyText = useCallback(async () => {
    if (isTyping(document.activeElement)) (document.activeElement as HTMLElement).blur();
    const c = useCapture.getState();
    if (!c.crop || !c.session || c.phase !== "editing") return;
    c.setPhase("reading");
    c.setNotice(null);
    try {
      const res = await api.extractText({
        sessionId: c.session.sessionId,
        monitorIndex: c.session.monitorIndex,
        ...toRenderRequest(c.crop, useEditor.getState().items),
      });
      // On success Rust has already ended the capture and hidden the overlay.
      if (!isCurrent()) return;
      if (!res.found) {
        c.setPhase("editing");
        c.setNotice({ kind: "info", message: "No text found in the selection." });
      }
    } catch (e) {
      if (!isCurrent()) return;
      c.setPhase("editing");
      c.setNotice({ kind: "error", message: errorMessage(e) });
    }
  }, [isCurrent]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const c = useCapture.getState();
      // Let an open popover handle its own Escape.
      if (e.key === "Escape" && document.querySelector('[data-slot="popover-content"]')) return;
      if (c.phase === "selecting" && !e.ctrlKey && !e.metaKey && !e.altKey && MODE_KEYS[e.key]) {
        c.setMode(MODE_KEYS[e.key]);
        return;
      }
      const action = keyAction(e, { isMac, typing: isTyping(document.activeElement), editing: c.phase === "editing" });
      if (!action) return;
      e.preventDefault();
      const ed = useEditor.getState();
      switch (action.type) {
        case "cancel":
          cancel();
          break;
        case "copy":
          void doExport("clipboard");
          break;
        case "copytext":
          void copyText();
          break;
        case "save":
          void doExport("file");
          break;
        case "quicksave":
          void doExport("quicksave");
          break;
        case "undo":
          ed.undo();
          break;
        case "redo":
          ed.redo();
          break;
        case "delete":
          ed.deleteSelected();
          break;
        case "tool":
          ed.setTool(action.tool);
          break;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [cancel, doExport, copyText]);

  // Auto-dismiss error and info notices.
  useEffect(() => {
    if (!notice || notice.kind === "success" || phase === "error") return;
    const t = window.setTimeout(() => useCapture.getState().setNotice(null), 5000);
    return () => window.clearTimeout(t);
  }, [notice, phase]);

  const onImageLoad = (el: HTMLImageElement) => {
    setImg(el);
    if (!readySent.current) {
      readySent.current = true;
      void api.overlayReady(boot.sessionId, boot.monitorIndex);
    }
  };

  const activeSession =
    session?.sessionId === boot.sessionId && session.monitorIndex === boot.monitorIndex ? session : null;

  return (
    <TooltipProvider delayDuration={400}>
      <div className="fixed inset-0 overflow-hidden bg-black">
        {activeSession ? (
          <>
            <img
              src={activeSession.previewUrl}
              crossOrigin="anonymous"
              alt=""
              draggable={false}
              decoding="sync"
              fetchPriority="high"
              className="pointer-events-none absolute inset-0 size-full select-none"
              onLoad={(e) => onImageLoad(e.currentTarget)}
              onError={() => {
                useCapture.getState().setNotice({ kind: "error", message: "Couldn't load the captured screen." });
                useCapture.getState().setPhase("error");
                void api.overlayReady(boot.sessionId, boot.monitorIndex);
              }}
            />
            {img ? <Stage session={activeSession} img={img} onClaim={claim} onCopy={() => void doExport("clipboard")} onCopyText={() => void copyText()} onSave={() => void doExport("file")} onQuickSave={() => void doExport("quicksave")} onCancel={cancel} /> : null}
          </>
        ) : null}

        {notice ? (
          <div className="pointer-events-none absolute inset-x-0 bottom-10 flex justify-center" role="status" aria-live="polite">
            <div className="pointer-events-auto flex max-w-[min(90vw,40rem)] items-center gap-2 rounded-lg border bg-popover px-3 py-2 text-sm text-popover-foreground shadow-xl">
              {notice.kind === "success" ? (
                <CircleCheck className="text-primary" />
              ) : notice.kind === "info" ? (
                <Info className="text-muted-foreground" />
              ) : (
                <TriangleAlert className="text-destructive" />
              )}
              <span className="truncate">{notice.message}</span>
              {phase === "error" ? (
                <Button size="sm" variant="outline" onClick={cancel}>
                  Close
                </Button>
              ) : null}
            </div>
          </div>
        ) : null}
      </div>
    </TooltipProvider>
  );
}
