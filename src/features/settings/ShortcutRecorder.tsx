import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Kbd, KbdGroup } from "@/components/ui/kbd";
import { isMac } from "@/shared/theme";

import { acceleratorFromEvent, formatAccelerator } from "./accelerator";

interface Props {
  id?: string;
  value: string;
  onChange: (accelerator: string) => void;
  invalid?: boolean;
}

/** Click, then press the new combination. Esc cancels recording. */
export function ShortcutRecorder({ id, value, onChange, invalid }: Props) {
  const [recording, setRecording] = useState(false);
  const [hint, setHint] = useState<string | null>(null);

  return (
    <div className="flex flex-col gap-1.5">
      <Button
        id={id}
        type="button"
        variant="outline"
        aria-invalid={invalid || undefined}
        aria-live="polite"
        className="w-fit min-w-44 justify-start"
        onClick={() => {
          setRecording(true);
          setHint("Press the new shortcut…");
        }}
        onBlur={() => {
          setRecording(false);
          setHint(null);
        }}
        onKeyDown={(e) => {
          if (!recording) return;
          e.preventDefault();
          e.stopPropagation();
          if (e.key === "Escape") {
            setRecording(false);
            setHint(null);
            return;
          }
          const r = acceleratorFromEvent(e.nativeEvent, isMac);
          if (r.ok) {
            setRecording(false);
            setHint(null);
            onChange(r.accelerator);
          } else if (r.reason === "no-modifier") {
            setHint(`Include ${isMac ? "⌘, ⌥, ⌃" : "Ctrl, Alt"} or Shift.`);
          } else if (r.reason === "unsupported") {
            setHint("That key can't be used. Try another.");
          }
        }}
      >
        {recording ? (
          <span className="text-muted-foreground">Recording…</span>
        ) : (
          <KbdGroup>
            {formatAccelerator(value, isMac).map((k) => (
              <Kbd key={k}>{k}</Kbd>
            ))}
          </KbdGroup>
        )}
      </Button>
      {hint ? <span className="text-xs text-muted-foreground">{hint}</span> : null}
    </div>
  );
}
