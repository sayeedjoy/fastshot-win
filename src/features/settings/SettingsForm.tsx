import { AppWindow, Brush, Check, EyeOff, FolderOpen, Monitor, Moon, Palette, Power, RotateCcw, SquareDashed, Sun, type LucideIcon } from "lucide-react";
import { type ReactNode, useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { Field, FieldContent, FieldDescription, FieldError, FieldLabel, FieldSeparator } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
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

const trayName = isMac ? "menu bar" : "system tray";

/** A titled group of related settings on one quiet, shared surface. */
export function SettingsSection({ icon: Icon, title, description, children }: { icon: LucideIcon; title: string; description?: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-2.5">
      <div className="flex items-start gap-2.5 px-0.5">
        <Icon className="mt-0.5 size-4 shrink-0 text-capture" />
        <div className="flex flex-col gap-0.5">
          <h3 className="font-heading text-sm font-semibold">{title}</h3>
          {description ? <p className="text-xs text-muted-foreground">{description}</p> : null}
        </div>
      </div>
      <div className="flex flex-col gap-5 rounded-xl border bg-card p-5 max-[560px]:p-4">{children}</div>
    </section>
  );
}

/** Horizontal row: label + description on the left, control on the right. */
function SettingRow({ id, label, description, children }: { id?: string; label: string; description?: ReactNode; children: ReactNode }) {
  return (
    <Field orientation="horizontal" className="flex-wrap items-center justify-between gap-x-4 gap-y-2 max-[560px]:items-start">
      <FieldContent>
        <FieldLabel htmlFor={id}>{label}</FieldLabel>
        {description ? <FieldDescription>{description}</FieldDescription> : null}
      </FieldContent>
      <div className="shrink-0 max-[560px]:max-w-full">{children}</div>
    </Field>
  );
}

/** Slider that shows its value while dragging and saves on release. */
function CommitSlider({
  value,
  onCommit,
  label,
  unit = "",
  description,
  ...rest
}: {
  value: number;
  onCommit: (v: number) => void;
  label: string;
  unit?: string;
  description?: string;
  id: string;
  min: number;
  max: number;
  disabled?: boolean;
}) {
  const [local, setLocal] = useState(value);
  useEffect(() => setLocal(value), [value]);
  return (
    <Field data-disabled={rest.disabled || undefined} className="group-data-[disabled=true]/field:opacity-50">
      <div className="flex items-center justify-between">
        <FieldLabel htmlFor={rest.id}>{label}</FieldLabel>
        <span className="rounded-md bg-muted px-1.5 py-0.5 font-mono text-xs tabular-nums text-muted-foreground">
          {local}
          {unit}
        </span>
      </div>
      <Slider {...rest} step={1} value={[local]} onValueChange={([v]) => setLocal(v)} onValueCommit={([v]) => onCommit(v)} aria-label={label} aria-valuetext={`${local}${unit}`} />
      {description ? <FieldDescription>{description}</FieldDescription> : null}
    </Field>
  );
}

const TEMPLATE_TOKENS = ["{date}", "{time}", "{n}", "{YYYY}", "{MM}", "{DD}", "{hh}", "{mm}", "{ss}", "{width}", "{height}"];
const DEFAULT_TEMPLATE = "Screenshot-{date}-{time}";

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
      <div className="flex gap-2 max-[560px]:flex-wrap">
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
          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="ghost" size="icon" aria-label="Use default file name" onClick={() => onCommit(DEFAULT_TEMPLATE)}>
                <RotateCcw />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Reset to default</TooltipContent>
          </Tooltip>
        ) : null}
      </div>
      <div className="flex flex-wrap gap-1" aria-label="Insert token">
        {TEMPLATE_TOKENS.map((t) => (
          <Button key={t} type="button" variant="outline" size="xs" className="font-mono text-muted-foreground hover:text-foreground" onMouseDown={(e) => e.preventDefault()} onClick={() => insert(t)}>
            {t}
          </Button>
        ))}
      </div>
      {preview.error ? (
        <FieldError>{preview.error}</FieldError>
      ) : (
        <FieldDescription>
          Example: <span className="font-mono text-foreground">{preview.example ?? "…"}</span>
          <br />
          Existing files are never overwritten by Quick save.
        </FieldDescription>
      )}
    </Field>
  );
}

const COLOR_NAMES: Record<string, string> = {
  "#e11d48": "Red",
  "#f97316": "Orange",
  "#facc15": "Yellow",
  "#22c55e": "Green", // Functional annotation choice. taste-skill-ignore
  "#0ea5e9": "Blue",
  "#6366f1": "Indigo", // Functional annotation choice. taste-skill-ignore
  "#ffffff": "White",
  "#000000": "Black",
};

/** Pick a check-mark colour that stays readable on the swatch. */
function contrastOn(hex: string) {
  const n = parseInt(hex.slice(1), 16);
  const [r, g, b] = [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  return 0.299 * r + 0.587 * g + 0.114 * b > 150 ? "#000" : "#fff";
}

function Swatches({ value, onChange, label }: { value: string; onChange: (c: string) => void; label: string }) {
  return (
    <div className="flex flex-wrap gap-2" role="radiogroup" aria-label={label}>
      {PALETTE.map((c) => {
        const selected = value.toLowerCase() === c;
        const name = COLOR_NAMES[c] ?? c;
        return (
          <Tooltip key={c}>
            <TooltipTrigger asChild>
              <button
                type="button"
                role="radio"
                aria-checked={selected}
                aria-label={name}
                onClick={() => onChange(c)}
                className={cn(
                  "flex size-8 items-center justify-center rounded-full border border-foreground/15 transition-transform outline-none hover:scale-110 focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-card",
                  selected && "scale-110 ring-2 ring-primary ring-offset-2 ring-offset-card",
                )}
                style={{ background: c }}
              >
                {selected ? <Check className="size-4" strokeWidth={3} style={{ color: contrastOn(c) }} /> : null}
              </button>
            </TooltipTrigger>
            <TooltipContent>{name}</TooltipContent>
          </Tooltip>
        );
      })}
    </div>
  );
}

export function GeneralSettings({ settings, save, shortcutError, defaultSaveDirectory }: Props & { defaultSaveDirectory?: string | null }) {
  return (
    <div className="flex flex-col gap-7">
      <SettingsSection icon={SquareDashed} title="Capture" description="How screenshots are started.">
        <Field data-invalid={shortcutError ? true : undefined} orientation="horizontal" className="flex-wrap items-center justify-between gap-x-4 gap-y-2">
          <FieldContent>
            <FieldLabel htmlFor="shortcut">Capture shortcut</FieldLabel>
            {shortcutError ? <FieldError>{shortcutError}</FieldError> : <FieldDescription>Works from any app while FastShot is running.</FieldDescription>}
          </FieldContent>
          <div className="shrink-0 max-[560px]:max-w-full">
            <ShortcutRecorder id="shortcut" value={settings.shortcut} onChange={(shortcut) => save({ shortcut })} invalid={!!shortcutError} />
          </div>
        </Field>

        <FieldSeparator />

        <SettingRow label="Default capture mode" description="Used by the shortcut and tray icon.">
          <ToggleGroup type="single" variant="outline" value={settings.defaultMode} onValueChange={(v) => v && save({ defaultMode: v as CaptureMode })} aria-label="Default capture mode">
            <ToggleGroupItem value="region">
              <SquareDashed data-icon="inline-start" />
              Region
            </ToggleGroupItem>
            <ToggleGroupItem value="window">
              <AppWindow data-icon="inline-start" />
              Window
            </ToggleGroupItem>
            <ToggleGroupItem value="display">
              <Monitor data-icon="inline-start" />
              Display
            </ToggleGroupItem>
          </ToggleGroup>
        </SettingRow>

        <FieldSeparator />

        <SettingRow id="magnifier" label="Show magnifier while selecting" description="A zoomed view with pixel coordinates and colour, for precise edges.">
          <Switch id="magnifier" checked={settings.showMagnifier} onCheckedChange={(showMagnifier) => save({ showMagnifier })} />
        </SettingRow>

        <FieldSeparator />

        <SettingRow id="auto-select-window" label="Click to select a window" description="In Region mode, clicking without dragging captures the window under the pointer. Turn off to select only by dragging.">
          <Switch id="auto-select-window" checked={settings.autoSelectWindow} onCheckedChange={(autoSelectWindow) => save({ autoSelectWindow })} />
        </SettingRow>

        {!isMac ? (
          <>
            <FieldSeparator />
            <SettingRow
              id="native-overlay"
              label="Native capture (experimental)"
              description="Uses less memory while idle, with a separate Edit step for annotations and saving. Leave off for the full editor to stay ready and appear directly after selection. Applies to the next capture; restart to release unused windows."
            >
              <Switch id="native-overlay" checked={settings.nativeOverlay} onCheckedChange={(nativeOverlay) => save({ nativeOverlay })} />
            </SettingRow>
          </>
        ) : null}
      </SettingsSection>

      <SettingsSection icon={FolderOpen} title="Saving" description={`Used by Save and Quick save (${isMac ? "⌘⇧S" : "Ctrl+Shift+S"}).`}>
        <SettingRow label="Format" description="You can still pick either format in the Save dialog.">
          <ToggleGroup type="single" variant="outline" value={settings.format} onValueChange={(v) => v && save({ format: v as ImageFormat })} aria-label="Save format">
            <ToggleGroupItem value="png">PNG</ToggleGroupItem>
            <ToggleGroupItem value="jpeg">JPEG</ToggleGroupItem>
          </ToggleGroup>
        </SettingRow>

        {settings.format === "jpeg" ? (
          <CommitSlider id="jpeg-quality" min={40} max={100} value={settings.jpegQuality} label="JPEG quality" description="Higher is sharper but produces larger files." onCommit={(jpegQuality) => save({ jpegQuality })} />
        ) : null}

        <FieldSeparator />

        <Field>
          <FieldLabel htmlFor="save-dir">Save folder</FieldLabel>
          <div className="flex gap-2 max-[560px]:flex-wrap">
            <Input
              id="save-dir"
              readOnly
              value={settings.saveDirectory ?? (defaultSaveDirectory ? `${defaultSaveDirectory} (default)` : "Documents/FastShot (default)")}
              title={settings.saveDirectory ?? defaultSaveDirectory ?? undefined}
              className="min-w-48 flex-1 truncate text-muted-foreground"
            />
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
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button variant="ghost" size="icon" aria-label="Use default folder" onClick={() => save({ saveDirectory: null })}>
                    <RotateCcw />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>Reset to Documents/FastShot</TooltipContent>
              </Tooltip>
            ) : null}
          </div>
          <FieldDescription>Quick save writes here without asking.</FieldDescription>
        </Field>

        <FilenameTemplateField value={settings.filenameTemplate} onCommit={(filenameTemplate) => save({ filenameTemplate })} />
      </SettingsSection>

      <SettingsSection icon={Palette} title="Appearance">
        <SettingRow label="Theme">
          <ToggleGroup type="single" variant="outline" value={settings.theme} onValueChange={(v) => v && save({ theme: v as Theme })} aria-label="Theme">
            <ToggleGroupItem value="system">
              <Monitor data-icon="inline-start" />
              System
            </ToggleGroupItem>
            <ToggleGroupItem value="light">
              <Sun data-icon="inline-start" />
              Light
            </ToggleGroupItem>
            <ToggleGroupItem value="dark">
              <Moon data-icon="inline-start" />
              Dark
            </ToggleGroupItem>
          </ToggleGroup>
        </SettingRow>
      </SettingsSection>

      <SettingsSection icon={Power} title="Startup & background">
        <SettingRow id="keep-running" label="Keep running when closed" description={`FastShot stays in the ${trayName} so the shortcut keeps working.`}>
          <Switch id="keep-running" checked={!settings.quitOnClose} onCheckedChange={(on) => save({ quitOnClose: !on })} />
        </SettingRow>
        <FieldSeparator />
        <SettingRow id="launch-at-login" label="Launch at login" description={`Start quietly in the ${trayName} when you sign in.`}>
          <Switch id="launch-at-login" checked={settings.launchAtLogin} onCheckedChange={(launchAtLogin) => save({ launchAtLogin })} />
        </SettingRow>
      </SettingsSection>
    </div>
  );
}

/** Live sample of how new annotations will look with the current defaults. */
function AnnotationPreview({ color, stroke, fontSize, redact }: { color: string; stroke: number; fontSize: number; redact: string }) {
  const text = Math.min(fontSize, 40);
  return (
    <div aria-hidden className="relative flex min-h-36 items-center gap-6 overflow-hidden rounded-xl border bg-muted/25 px-5 pt-6 max-[560px]:gap-4 max-[560px]:px-4">
      <div className="absolute inset-x-0 top-0 flex h-7 items-center gap-1.5 border-b bg-background/75 px-3">
        <span className="size-1.5 rounded-full bg-muted-foreground/30" />
        <span className="size-1.5 rounded-full bg-muted-foreground/30" />
        <span className="ml-2 h-1.5 w-20 rounded-full bg-muted-foreground/15" />
      </div>
      <svg width="96" height="72" viewBox="0 0 96 72" className="shrink-0 overflow-visible max-[560px]:w-20">
        <rect x={stroke / 2 + 2} y={stroke / 2 + 2} width={92 - stroke} height={68 - stroke} rx="4" fill="none" stroke={color} strokeWidth={Math.min(stroke, 16)} />
      </svg>
      <span className="truncate font-semibold leading-none" style={{ color, fontSize: text }}>
        Aa
      </span>
      <div className="flex flex-col gap-1.5 max-[560px]:hidden">
        <span className="text-xs text-muted-foreground">Redacted</span>
        <span className="h-4 w-24 rounded-sm" style={{ background: redact }} />
      </div>
      {fontSize > 40 ? <span className="absolute right-2 bottom-1.5 text-[10px] text-muted-foreground">Text preview capped at 40px</span> : null}
    </div>
  );
}

export function AnnotationSettings({ settings, save }: Props) {
  return (
    <div className="flex flex-col gap-7">
      <AnnotationPreview color={settings.annotationColor} stroke={settings.strokeWidth} fontSize={settings.fontSize} redact={settings.redactColor} />

      <SettingsSection icon={Brush} title="Annotations" description="Defaults for new shapes, arrows and text. You can change them per item in the editor.">
        <Field>
          <FieldLabel>Colour</FieldLabel>
          <Swatches label="Default annotation colour" value={settings.annotationColor} onChange={(annotationColor) => save({ annotationColor })} />
        </Field>
        <FieldSeparator />
        <CommitSlider id="stroke" min={1} max={30} value={settings.strokeWidth} unit="px" label="Stroke width" onCommit={(strokeWidth) => save({ strokeWidth })} />
        <CommitSlider id="font-size" min={10} max={120} value={settings.fontSize} unit="px" label="Text size" onCommit={(fontSize) => save({ fontSize })} />
      </SettingsSection>

      <SettingsSection icon={EyeOff} title="Redaction" description="Redactions are always fully opaque and permanently replace pixels.">
        <Field>
          <FieldLabel>Colour</FieldLabel>
          <Swatches label="Redaction colour" value={settings.redactColor} onChange={(redactColor) => save({ redactColor })} />
        </Field>
      </SettingsSection>
    </div>
  );
}
