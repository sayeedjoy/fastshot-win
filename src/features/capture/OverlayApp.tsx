import { emit, listen } from "@tauri-apps/api/event";
import { CircleCheck, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import { TooltipProvider } from "@/components/ui/tooltip";
import { keyAction } from "@/features/editor/keyboard";
import { loadAnnotationFont } from "@/features/editor/measure";
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
        useEditor.getState().reset({
          color: prefs.annotationColor,
          strokeWidth: prefs.strokeWidth,
          fontSize: prefs.fontSize,
          redactColor: prefs.redactColor,
        });
      })
      .catch(() => undefined);
    void loadAnnotationFont();

    return () => {
      alive = false;
    };
  }, [boot.sessionId, boot.monitorIndex, cancel]);

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
      try {
        const res = await api.exportCapture({
          sessionId: c.session.sessionId,
          monitorIndex: c.session.monitorIndex,
          destination,
          ...toRenderRequest(c.crop, useEditor.getState().items),
        });
        if (!res.completed) {
          c.setPhase("editing");
          return;
        }
        c.setPhase("done");
        c.setNotice({
          kind: "success",
          message: destination === "clipboard" ? `Copied ${res.width} × ${res.height} image` : `Saved to ${res.path}`,
        });
        window.setTimeout(cancel, destination === "clipboard" ? 650 : 1600);
      } catch (e) {
        c.setPhase("editing");
        c.setNotice({ kind: "error", message: errorMessage(e) });
      }
    },
    [cancel],
  );

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
  }, [cancel, doExport]);

  // Auto-dismiss error notices.
  useEffect(() => {
    if (notice?.kind !== "error" || phase === "error") return;
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
            {img ? <Stage session={activeSession} img={img} onClaim={claim} onCopy={() => void doExport("clipboard")} onSave={() => void doExport("file")} onQuickSave={() => void doExport("quicksave")} onCancel={cancel} /> : null}
          </>
        ) : null}

        {notice ? (
          <div className="pointer-events-none absolute inset-x-0 bottom-10 flex justify-center" role="status" aria-live="polite">
            <div className="pointer-events-auto flex max-w-[min(90vw,40rem)] items-center gap-2 rounded-lg border bg-popover px-3 py-2 text-sm text-popover-foreground shadow-xl">
              {notice.kind === "success" ? <CircleCheck className="text-primary" /> : <TriangleAlert className="text-destructive" />}
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
