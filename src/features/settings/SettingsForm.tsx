import { FolderOpen, RotateCcw } from "lucide-react";
import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { Field, FieldContent, FieldDescription, FieldError, FieldGroup, FieldLabel, FieldSet, FieldLegend, FieldSeparator } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { cn } from "@/lib/utils";
import { PALETTE } from "@/features/editor/palette";
import { type CaptureMode, type ImageFormat, type Settings, type Theme, api } from "@/shared/ipc";
import { isMac } from "@/shared/theme";

import { ShortcutRecorder } from "./ShortcutRecorder";

interface Props {
  settings: Settings;
  save: (patch: Partial<Settings>) => void;
  shortcutError: string | null;
}

/** Slider that shows its value while dragging and saves on release. */
function CommitSlider({ value, onCommit, label, ...rest }: { value: number; onCommit: (v: number) => void; label: (v: number) => string; id: string; min: number; max: number; disabled?: boolean }) {
  const [local, setLocal] = useState(value);
  useEffect(() => setLocal(value), [value]);
  return (
    <Field data-disabled={rest.disabled || undefined}>
      <FieldLabel htmlFor={rest.id}>{label(local)}</FieldLabel>
      <Slider {...rest} step={1} value={[local]} onValueChange={([v]) => setLocal(v)} onValueCommit={([v]) => onCommit(v)} aria-label={label(local)} />
    </Field>
  );
}

const TEMPLATE_TOKENS = ["{date}", "{time}", "{n}", "{YYYY}", "{MM}", "{DD}", "{hh}", "{mm}", "{ss}", "{width}", "{height}"];

/**
 * File name template with a live preview rendered by Rust (the same renderer
 * real saves use). Saves on blur / Enter once the template is valid.
 */
function FilenameTemplateField({ value, onCommit }: { value: string; onCommit: (v: string) => void }) {
  const [draft, setDraft] = useState(value);
  const [preview, setPreview] = useState<{ example: string | null; error: string | null }>({ example: null, error: null });
  useEffect(() => setDraft(value), [value]);
  useEffect(() => {
    let alive = true;
    const t = window.setTimeout(() => {
      api
        .previewFilename(draft)
        .then((p) => alive && setPreview(p))
        .catch(() => undefined);
    }, 120);
    return () => {
      alive = false;
      window.clearTimeout(t);
    };
  }, [draft]);

  const commit = () => {
    if (draft !== value && !preview.error) onCommit(draft);
  };
  const insert = (token: string) => setDraft((d) => d + token);

  return (
    <Field data-invalid={preview.error ? true : undefined}>
      <FieldLabel htmlFor="filename-template">File name</FieldLabel>
      <div className="flex gap-2">
        <Input
          id="filename-template"
          value={draft}
          spellCheck={false}
          aria-invalid={!!preview.error}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => e.key === "Enter" && commit()}
          className="flex-1 font-mono"
        />
        {value !== DEFAULT_TEMPLATE ? (
          <Button variant="ghost" size="icon" aria-label="Use default file name" onClick={() => onCommit(DEFAULT_TEMPLATE)}>
            <RotateCcw />
          </Button>
        ) : null}
      </div>
      <div className="flex flex-wrap gap-1" aria-label="Insert token">
        {TEMPLATE_TOKENS.map((t) => (
          <Button key={t} type="button" variant="outline" size="xs" className="font-mono" onMouseDown={(e) => e.preventDefault()} onClick={() => insert(t)}>
            {t}
          </Button>
        ))}
      </div>
      {preview.error ? (
        <FieldError>{preview.error}</FieldError>
      ) : (
        <FieldDescription>
          Example: <span className="font-mono">{preview.example ?? "…"}</span>. Used by Save and Quick save; existing files are never overwritten by Quick save.
        </FieldDescription>
      )}
    </Field>
  );
}

const DEFAULT_TEMPLATE = "Screenshot-{date}-{time}";

function Swatches({ value, onChange, label }: { value: string; onChange: (c: string) => void; label: string }) {
  return (
    <div className="flex flex-wrap gap-2" role="radiogroup" aria-label={label}>
      {PALETTE.map((c) => (
        <button
          key={c}
          type="button"
          role="radio"
          aria-checked={value.toLowerCase() === c}
          aria-label={c}
          onClick={() => onChange(c)}
          className={cn("size-7 rounded-full border border-border outline-none focus-visible:ring-2 focus-visible:ring-ring", value.toLowerCase() === c && "ring-2 ring-ring ring-offset-2 ring-offset-background")}
          style={{ background: c }}
        />
      ))}
    </div>
  );
}

export function GeneralSettings({ settings, save, shortcutError }: Props) {
  return (
    <FieldGroup>
      <Field data-invalid={shortcutError ? true : undefined}>
        <FieldLabel htmlFor="shortcut">Capture shortcut</FieldLabel>
        <ShortcutRecorder id="shortcut" value={settings.shortcut} onChange={(shortcut) => save({ shortcut })} invalid={!!shortcutError} />
        {shortcutError ? <FieldError>{shortcutError}</FieldError> : <FieldDescription>Works from any app while FastShot is running.</FieldDescription>}
      </Field>

      <Field>
        <FieldLabel>Default capture mode</FieldLabel>
        <ToggleGroup type="single" variant="outline" value={settings.defaultMode} onValueChange={(v) => v && save({ defaultMode: v as CaptureMode })} aria-label="Default capture mode">
          <ToggleGroupItem value="region">Region</ToggleGroupItem>
          <ToggleGroupItem value="window">Window</ToggleGroupItem>
          <ToggleGroupItem value="display">Display</ToggleGroupItem>
        </ToggleGroup>
      </Field>

      <FieldSeparator />

      <Field>
        <FieldLabel>Save format</FieldLabel>
        <ToggleGroup type="single" variant="outline" value={settings.format} onValueChange={(v) => v && save({ format: v as ImageFormat })} aria-label="Save format">
          <ToggleGroupItem value="png">PNG</ToggleGroupItem>
          <ToggleGroupItem value="jpeg">JPEG</ToggleGroupItem>
        </ToggleGroup>
        <FieldDescription>You can still pick either format in the Save dialog.</FieldDescription>
      </Field>

      <CommitSlider id="jpeg-quality" min={40} max={100} value={settings.jpegQuality} disabled={settings.format !== "jpeg"} label={(v) => `JPEG quality · ${v}`} onCommit={(jpegQuality) => save({ jpegQuality })} />

      <Field>
        <FieldLabel htmlFor="save-dir">Default save folder</FieldLabel>
        <div className="flex gap-2">
          <Input id="save-dir" readOnly value={settings.saveDirectory ?? "Pictures (default)"} className="flex-1" />
          <Button
            variant="outline"
            onClick={async () => {
              const dir = await api.browseSaveDirectory();
              if (dir) save({ saveDirectory: dir });
            }}
          >
            <FolderOpen data-icon="inline-start" />
            Browse…
          </Button>
          {settings.saveDirectory ? (
            <Button variant="ghost" size="icon" aria-label="Use default folder" onClick={() => save({ saveDirectory: null })}>
              <RotateCcw />
            </Button>
          ) : null}
        </div>
        <FieldDescription>Quick save ({isMac ? "⌘⇧S" : "Ctrl+Shift+S"}) writes here without asking.</FieldDescription>
      </Field>

      <FilenameTemplateField value={settings.filenameTemplate} onCommit={(filenameTemplate) => save({ filenameTemplate })} />

      <FieldSeparator />

      <Field orientation="horizontal">
        <FieldContent>
          <FieldLabel htmlFor="magnifier">Show magnifier while selecting</FieldLabel>
          <FieldDescription>A zoomed view with pixel coordinates and colour, for precise edges.</FieldDescription>
        </FieldContent>
        <Switch id="magnifier" checked={settings.showMagnifier} onCheckedChange={(showMagnifier) => save({ showMagnifier })} />
      </Field>

      <FieldSeparator />

      <Field>
        <FieldLabel>Appearance</FieldLabel>
        <ToggleGroup type="single" variant="outline" value={settings.theme} onValueChange={(v) => v && save({ theme: v as Theme })} aria-label="Appearance">
          <ToggleGroupItem value="system">System</ToggleGroupItem>
          <ToggleGroupItem value="light">Light</ToggleGroupItem>
          <ToggleGroupItem value="dark">Dark</ToggleGroupItem>
        </ToggleGroup>
      </Field>

      <Field orientation="horizontal">
        <FieldContent>
          <FieldLabel htmlFor="keep-running">Keep running when this window is closed</FieldLabel>
          <FieldDescription>FastShot stays in the {isMac ? "menu bar" : "system tray"} so the shortcut keeps working.</FieldDescription>
        </FieldContent>
        <Switch id="keep-running" checked={!settings.quitOnClose} onCheckedChange={(on) => save({ quitOnClose: !on })} />
      </Field>

      <Field orientation="horizontal">
        <FieldContent>
          <FieldLabel htmlFor="launch-at-login">Launch at login</FieldLabel>
          <FieldDescription>Start FastShot quietly in the {isMac ? "menu bar" : "system tray"} when you sign in.</FieldDescription>
        </FieldContent>
        <Switch id="launch-at-login" checked={settings.launchAtLogin} onCheckedChange={(launchAtLogin) => save({ launchAtLogin })} />
      </Field>
    </FieldGroup>
  );
}

export function AnnotationSettings({ settings, save }: Props) {
  return (
    <FieldGroup>
      <FieldSet>
        <FieldLegend variant="label">Default annotation colour</FieldLegend>
        <Swatches label="Default annotation colour" value={settings.annotationColor} onChange={(annotationColor) => save({ annotationColor })} />
      </FieldSet>
      <CommitSlider id="stroke" min={1} max={30} value={settings.strokeWidth} label={(v) => `Stroke width · ${v}px`} onCommit={(strokeWidth) => save({ strokeWidth })} />
      <CommitSlider id="font-size" min={10} max={120} value={settings.fontSize} label={(v) => `Text size · ${v}px`} onCommit={(fontSize) => save({ fontSize })} />
      <FieldSeparator />
      <FieldSet>
        <FieldLegend variant="label">Redaction colour</FieldLegend>
        <FieldDescription>Redactions are always fully opaque.</FieldDescription>
        <Swatches label="Redaction colour" value={settings.redactColor} onChange={(redactColor) => save({ redactColor })} />
      </FieldSet>
    </FieldGroup>
  );
}
