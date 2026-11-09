import { AppWindow, Monitor, SquareDashed } from "lucide-react";

import { Kbd } from "@/components/ui/kbd";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import type { CaptureMode, SessionView } from "@/shared/ipc";

import { useCapture } from "./store";

const MODES: { mode: CaptureMode; label: string; key: string; icon: typeof Monitor }[] = [
  { mode: "region", label: "Region", key: "1", icon: SquareDashed },
  { mode: "window", label: "Window", key: "2", icon: AppWindow },
  { mode: "display", label: "Display", key: "3", icon: Monitor },
];

const HINT: Record<CaptureMode, string> = {
  region: "Drag to select an area, or click a window",
  window: "Click a window to capture it",
  display: "Click to capture this display",
};

export function ModeBar({ session }: { session: SessionView }) {
  const mode = useCapture((s) => s.mode);
  const setMode = useCapture((s) => s.setMode);
  return (
    <div className="absolute top-4 left-1/2 flex -translate-x-1/2 flex-col items-center gap-2" onPointerDown={(e) => e.stopPropagation()}>
      <div className="flex items-center gap-2 rounded-xl border bg-popover p-1 text-popover-foreground shadow-xl">
        <ToggleGroup type="single" size="sm" spacing={1} value={mode} onValueChange={(v) => v && setMode(v as CaptureMode)} aria-label="Capture mode">
          {MODES.map(({ mode: m, label, key, icon: I }) => (
            <ToggleGroupItem key={m} value={m} aria-label={`${label} (${key})`} className="gap-1.5 px-2.5">
              <I data-icon="inline-start" />
              {label}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      </div>
      <p className="flex items-center gap-2 rounded-md bg-black/70 px-2.5 py-1 text-xs text-white">
        {HINT[mode]}
        {session.monitorCount > 1 ? <span className="opacity-70">· {session.monitor.name}</span> : null}
        <span className="opacity-70">·</span>
        <Kbd>Esc</Kbd> to cancel
      </p>
    </div>
  );
}
