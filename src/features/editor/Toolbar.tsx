import type { ComponentType, CSSProperties } from "react";
import {
  ArrowUpRight,
  Check,
  Circle,
  Copy,
  Download,
  Droplets,
  Ellipsis,
  Grid3x3,
  Highlighter,
  ListOrdered,
  Minus,
  MousePointer2,
  Pencil,
  Redo2,
  ShieldCheck,
  Square,
  Type,
  Undo2,
  X,
  Zap,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/kbd";
import { Popover, PopoverContent, PopoverDescription, PopoverHeader, PopoverTitle, PopoverTrigger } from "@/components/ui/popover";
import { Separator } from "@/components/ui/separator";
import { Slider } from "@/components/ui/slider";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import { isMac } from "@/shared/theme";

import { TOOL_SHORTCUT } from "./keyboard";
import type { Tool } from "./model";
import { PALETTE } from "./palette";
import { useEditor } from "./store";

type Icon = ComponentType<{ className?: string; "data-icon"?: string }>;

const PRIMARY: { tool: Tool; label: string; icon: Icon }[] = [
  { tool: "select", label: "Select & move", icon: MousePointer2 },
  { tool: "arrow", label: "Arrow", icon: ArrowUpRight },
  { tool: "rect", label: "Rectangle", icon: Square },
  { tool: "pen", label: "Pen", icon: Pencil },
  { tool: "text", label: "Text", icon: Type },
];

const SHAPES: { tool: Tool; label: string; icon: Icon }[] = [
  { tool: "ellipse", label: "Ellipse", icon: Circle },
  { tool: "line", label: "Line", icon: Minus },
];

const MARKERS: { tool: Tool; label: string; icon: Icon }[] = [
  { tool: "step", label: "Steps", icon: ListOrdered },
  { tool: "highlight", label: "Highlighter", icon: Highlighter },
];

const EFFECTS: { tool: Tool; label: string; icon: Icon }[] = [
  { tool: "blur", label: "Blur", icon: Droplets },
  { tool: "pixelate", label: "Pixelate", icon: Grid3x3 },
];


const mod = isMac ? "⌘" : "Ctrl";

function Hint({ label, keys, children }: { label: string; keys?: string; children: React.ReactNode }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>{children}</TooltipTrigger>
      <TooltipContent side="bottom" className="flex items-center gap-2">
        {label}
        {keys ? <Kbd>{keys}</Kbd> : null}
      </TooltipContent>
    </Tooltip>
  );
}

function StylePanel() {
  const tool = useEditor((s) => s.tool);
  const style = useEditor((s) => s.style);
  const setStyle = useEditor((s) => s.setStyle);
  const selectedType = useEditor((s) => s.items.find((i) => i.id === s.selectedId)?.type);
  const kind = selectedType ?? tool;

  if (kind === "blur" || kind === "pixelate") {
    const isBlur = kind === "blur";
    const value = isBlur ? style.blurStrength : style.pixelSize;
    return (
      <div className="flex flex-col gap-2">
        <span className="text-sm font-medium">{isBlur ? "Blur strength" : "Block size"}</span>
        <Slider min={2} max={isBlur ? 30 : 40} step={1} value={[value]} onValueChange={([v]) => setStyle(isBlur ? { blurStrength: v } : { pixelSize: v })} aria-label={isBlur ? "Blur strength" : "Block size"} />
      </div>
    );
  }

  const isRedact = kind === "redact";
  const isHighlight = kind === "highlight";
  const color = isRedact ? style.redactColor : isHighlight ? style.highlightColor : style.color;
  const pick = (c: string) => setStyle(isRedact ? { redactColor: c } : isHighlight ? { highlightColor: c } : { color: c });
  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-col gap-2">
        <span className="text-sm font-medium">{isRedact ? "Redaction colour" : "Colour"}</span>
        <div className="grid grid-cols-8 gap-1.5" role="radiogroup" aria-label="Colour">
          {PALETTE.map((c) => (
            <button
              key={c}
              type="button"
              role="radio"
              aria-checked={color === c}
              aria-label={c}
              onClick={() => pick(c)}
              className={cn("size-6 rounded-full border border-border outline-none focus-visible:ring-2 focus-visible:ring-ring", color === c && "ring-2 ring-ring ring-offset-2 ring-offset-popover")}
              style={{ background: c }}
            />
          ))}
        </div>
      </div>
      {isRedact ? null : kind === "step" ? (
        <div className="flex flex-col gap-2">
          <span className="text-sm font-medium">Marker size · {style.stepRadius * 2}px</span>
          <Slider min={8} max={60} step={1} value={[style.stepRadius]} onValueChange={([v]) => setStyle({ stepRadius: v })} aria-label="Marker size" />
        </div>
      ) : isHighlight ? (
        <div className="flex flex-col gap-2">
          <span className="text-sm font-medium">Width · {style.highlightWidth}px</span>
          <Slider min={6} max={60} step={1} value={[style.highlightWidth]} onValueChange={([v]) => setStyle({ highlightWidth: v })} aria-label="Highlighter width" />
        </div>
      ) : kind === "text" ? (
        <div className="flex flex-col gap-2">
          <span className="text-sm font-medium">Font size · {style.fontSize}px</span>
          <Slider min={10} max={120} step={1} value={[style.fontSize]} onValueChange={([v]) => setStyle({ fontSize: v })} aria-label="Font size" />
        </div>
      ) : (
        <div className="flex flex-col gap-2">
          <span className="text-sm font-medium">Stroke · {style.strokeWidth}px</span>
          <Slider min={1} max={30} step={1} value={[style.strokeWidth]} onValueChange={([v]) => setStyle({ strokeWidth: v })} aria-label="Stroke width" />
        </div>
      )}
    </div>
  );
}

interface ToolbarProps {
  style: CSSProperties;
  busy: boolean;
  onCopy: () => void;
  onSave: () => void;
  onQuickSave: () => void;
  onCancel: () => void;
}

export function Toolbar({ style: position, busy, onCopy, onSave, onQuickSave, onCancel }: ToolbarProps) {
  const tool = useEditor((s) => s.tool);
  const setTool = useEditor((s) => s.setTool);
  const canUndo = useEditor((s) => s.past.length > 0);
  const canRedo = useEditor((s) => s.future.length > 0);
  const undo = useEditor((s) => s.undo);
  const redo = useEditor((s) => s.redo);
  const swatch = useEditor((s) => (s.tool === "redact" ? s.style.redactColor : s.tool === "highlight" ? s.style.highlightColor : s.style.color));

  const secondaryActive = [...SHAPES, ...MARKERS, ...EFFECTS].find((t) => t.tool === tool);
  const onTool = (v: string) => v && setTool(v as Tool);

  return (
    <div
      role="toolbar"
      aria-label="Screenshot tools"
      className="absolute flex flex-col items-end gap-1.5"
      style={position}
      onPointerDown={(e) => e.stopPropagation()}
    >
      <div className="flex items-center gap-1 rounded-xl border bg-popover p-1 text-popover-foreground shadow-xl">
        <ToggleGroup type="single" size="sm" spacing={1} value={secondaryActive ? "" : tool} onValueChange={onTool} aria-label="Tool">
          {PRIMARY.map(({ tool: t, label, icon: I }) => (
            <Hint key={t} label={label} keys={TOOL_SHORTCUT[t]}>
              <ToggleGroupItem value={t} aria-label={label}>
                <I />
              </ToggleGroupItem>
            </Hint>
          ))}
          <Hint label="Redact — permanently replaces pixels" keys={TOOL_SHORTCUT.redact}>
            <ToggleGroupItem value="redact" aria-label="Redact (permanent)" className="gap-1 px-2 font-medium">
              <ShieldCheck data-icon="inline-start" />
              Redact
            </ToggleGroupItem>
          </Hint>
        </ToggleGroup>

        <Popover>
          <Hint label="More shapes and effects">
            <PopoverTrigger asChild>
              <Button variant={secondaryActive ? "secondary" : "ghost"} size="icon-sm" aria-label="More tools">
                {secondaryActive ? <secondaryActive.icon /> : <Ellipsis />}
              </Button>
            </PopoverTrigger>
          </Hint>
          <PopoverContent side="bottom" align="start" className="w-64">
            <div className="flex flex-col gap-3">
              <div className="flex flex-col gap-1">
                <span className="text-xs font-medium text-muted-foreground">Shapes</span>
                <ToggleGroup type="single" size="sm" spacing={1} value={tool} onValueChange={onTool} aria-label="Shapes">
                  {SHAPES.map(({ tool: t, label, icon: I }) => (
                    <ToggleGroupItem key={t} value={t} aria-label={label} className="gap-1.5 px-2">
                      <I data-icon="inline-start" />
                      {label}
                    </ToggleGroupItem>
                  ))}
                </ToggleGroup>
              </div>
              <div className="flex flex-col gap-1">
                <span className="text-xs font-medium text-muted-foreground">Markers</span>
                <ToggleGroup type="single" size="sm" spacing={1} value={tool} onValueChange={onTool} aria-label="Markers">
                  {MARKERS.map(({ tool: t, label, icon: I }) => (
                    <Hint key={t} label={label} keys={TOOL_SHORTCUT[t]}>
                      <ToggleGroupItem value={t} aria-label={label} className="gap-1.5 px-2">
                        <I data-icon="inline-start" />
                        {label}
                      </ToggleGroupItem>
                    </Hint>
                  ))}
                </ToggleGroup>
              </div>
              <Separator />
              <PopoverHeader>
                <PopoverTitle className="text-xs font-medium text-muted-foreground">Effects</PopoverTitle>
                <PopoverDescription className="text-xs">Visual only — not a privacy guarantee. Use Redact to hide secrets.</PopoverDescription>
              </PopoverHeader>
              <ToggleGroup type="single" size="sm" spacing={1} value={tool} onValueChange={onTool} aria-label="Effects">
                {EFFECTS.map(({ tool: t, label, icon: I }) => (
                  <ToggleGroupItem key={t} value={t} aria-label={label} className="gap-1.5 px-2">
                    <I data-icon="inline-start" />
                    {label}
                  </ToggleGroupItem>
                ))}
              </ToggleGroup>
            </div>
          </PopoverContent>
        </Popover>

        <Popover>
          <Hint label="Colour & size">
            <PopoverTrigger asChild>
              <Button variant="ghost" size="icon-sm" aria-label="Colour and size">
                <span className="size-4 rounded-full border border-border" style={{ background: swatch }} />
              </Button>
            </PopoverTrigger>
          </Hint>
          <PopoverContent side="bottom" align="center" className="w-64">
            <StylePanel />
          </PopoverContent>
        </Popover>

        <Separator orientation="vertical" className="mx-0.5 h-5" />

        <Hint label="Undo" keys={`${mod}+Z`}>
          <Button variant="ghost" size="icon-sm" onClick={undo} disabled={!canUndo} aria-label="Undo">
            <Undo2 />
          </Button>
        </Hint>
        <Hint label="Redo" keys={isMac ? "⌘+⇧+Z" : "Ctrl+Y"}>
          <Button variant="ghost" size="icon-sm" onClick={redo} disabled={!canRedo} aria-label="Redo">
            <Redo2 />
          </Button>
        </Hint>

        <Separator orientation="vertical" className="mx-0.5 h-5" />

        <Hint label="Cancel" keys="Esc">
          <Button variant="ghost" size="icon-sm" onClick={onCancel} aria-label="Cancel">
            <X />
          </Button>
        </Hint>
        <Hint label="Quick save to your default folder" keys={isMac ? "⌘+⇧+S" : "Ctrl+Shift+S"}>
          <Button variant="ghost" size="icon-sm" onClick={onQuickSave} disabled={busy} aria-label="Quick save">
            <Zap />
          </Button>
        </Hint>
        <Hint label="Save as…" keys={`${mod}+S`}>
          <Button variant="outline" size="sm" onClick={onSave} disabled={busy}>
            <Download data-icon="inline-start" />
            Save
          </Button>
        </Hint>
        <Hint label="Copy to clipboard" keys={`${mod}+C`}>
          <Button size="sm" onClick={onCopy} disabled={busy}>
            {busy ? <Check data-icon="inline-start" /> : <Copy data-icon="inline-start" />}
            Copy
          </Button>
        </Hint>
      </div>
      {tool === "redact" ? (
        <p className="rounded-md bg-popover/95 px-2 py-1 text-xs text-popover-foreground shadow">Redact permanently replaces these pixels in the copied or saved image.</p>
      ) : tool === "blur" || tool === "pixelate" ? (
        <p className="rounded-md bg-popover/95 px-2 py-1 text-xs text-popover-foreground shadow">Visual effect only — blurred or pixelated text can sometimes be recovered.</p>
      ) : null}
    </div>
  );
}

export const TOOLBAR_HEIGHT = 44;
export const TOOLBAR_WIDTH = 560;
