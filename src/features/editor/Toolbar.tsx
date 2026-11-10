import type { ComponentType } from "react";
import { forwardRef, memo } from "react";
import {
  ArrowUpRight,
  Check,
  Circle,
  Copy,
  Droplets,
  Grid3x3,
  Highlighter,
  ListOrdered,
  LoaderCircle,
  Minus,
  MousePointer2,
  Pencil,
  Redo2,
  Save,
  ScanText,
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
type Side = "top" | "right" | "bottom" | "left";

/** Tools shown directly in the vertical column, top to bottom. */
const TOOLS: { tool: Tool; label: string; icon: Icon }[] = [
  { tool: "select", label: "Select & move", icon: MousePointer2 },
  { tool: "pen", label: "Pen", icon: Pencil },
  { tool: "line", label: "Line", icon: Minus },
  { tool: "arrow", label: "Arrow", icon: ArrowUpRight },
  { tool: "rect", label: "Rectangle", icon: Square },
  { tool: "ellipse", label: "Ellipse", icon: Circle },
  { tool: "highlight", label: "Highlighter", icon: Highlighter },
  { tool: "text", label: "Text", icon: Type },
  { tool: "step", label: "Steps", icon: ListOrdered },
  { tool: "redact", label: "Redact — permanently replaces pixels", icon: ShieldCheck },
];

const EFFECTS: { tool: Tool; label: string; icon: Icon }[] = [
  { tool: "blur", label: "Blur", icon: Droplets },
  { tool: "pixelate", label: "Pixelate", icon: Grid3x3 },
];

const mod = isMac ? "⌘" : "Ctrl";

const PANEL = "rounded-md border border-overlay-panel-border bg-overlay-panel p-1 text-overlay-panel-foreground shadow-[0_2px_8px_rgb(0_0_0/0.22)]";
const TOOL_CONTROL = "overlay-control";

function Hint({ label, keys, side, children }: { label: string; keys?: string; side: Side; children: React.ReactNode }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>{children}</TooltipTrigger>
      <TooltipContent side={side} className="flex items-center gap-2">
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

const stop = (e: React.PointerEvent) => e.stopPropagation();

/** Vertical drawing-tool column, docked beside the selection. */
// Each toolbar is a thin positioned shell around memoised contents: while
// the selection is dragged only the shell's transform changes per frame.
const ToolColumnBody = memo(function ToolColumnBody({ tipSide }: { tipSide: Side }) {
  const tool = useEditor((s) => s.tool);
  const setTool = useEditor((s) => s.setTool);
  const canUndo = useEditor((s) => s.past.length > 0);
  const canRedo = useEditor((s) => s.future.length > 0);
  const undo = useEditor((s) => s.undo);
  const redo = useEditor((s) => s.redo);
  const swatch = useEditor((s) => (s.tool === "redact" ? s.style.redactColor : s.tool === "highlight" ? s.style.highlightColor : s.style.color));

  const effectActive = EFFECTS.find((t) => t.tool === tool);
  const onTool = (v: string) => v && setTool(v as Tool);

  return (
    <>
      <ToggleGroup type="single" size="sm" spacing={0.5} orientation="vertical" value={effectActive ? "" : tool} onValueChange={onTool} aria-label="Tool">
        {TOOLS.map(({ tool: t, label, icon: I }) => (
          <Hint key={t} label={label} keys={TOOL_SHORTCUT[t]} side={tipSide}>
            <ToggleGroupItem value={t} aria-label={label} className={cn("size-7 min-w-0 p-0", TOOL_CONTROL)}>
              <I />
            </ToggleGroupItem>
          </Hint>
        ))}
      </ToggleGroup>

      <Popover>
        <Hint label="Blur & pixelate" side={tipSide}>
          <PopoverTrigger asChild>
            <Button variant="ghost" size="icon-sm" aria-label="Effects" aria-pressed={!!effectActive} className={TOOL_CONTROL}>
              {effectActive ? <effectActive.icon /> : <Droplets />}
            </Button>
          </PopoverTrigger>
        </Hint>
        <PopoverContent side={tipSide} align="center" className="w-60">
          <div className="flex flex-col gap-2">
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
        <Hint label="Colour & size" side={tipSide}>
          <PopoverTrigger asChild>
            <Button variant="ghost" size="icon-sm" aria-label="Colour and size" className={TOOL_CONTROL}>
              <span className="size-4 rounded-sm border border-overlay-panel-border" style={{ background: swatch }} />
            </Button>
          </PopoverTrigger>
        </Hint>
        <PopoverContent side={tipSide} align="center" className="w-64">
          <StylePanel />
        </PopoverContent>
      </Popover>

      <Separator className="my-0.5 w-5 bg-overlay-panel-separator" />

      <Hint label="Undo" keys={`${mod}+Z`} side={tipSide}>
        <Button variant="ghost" size="icon-sm" onClick={undo} disabled={!canUndo} aria-label="Undo" className={TOOL_CONTROL}>
          <Undo2 />
        </Button>
      </Hint>
      <Hint label="Redo" keys={isMac ? "⌘+⇧+Z" : "Ctrl+Y"} side={tipSide}>
        <Button variant="ghost" size="icon-sm" onClick={redo} disabled={!canRedo} aria-label="Redo" className={TOOL_CONTROL}>
          <Redo2 />
        </Button>
      </Hint>
    </>
  );
});

export const ToolColumn = forwardRef<HTMLDivElement, { style: React.CSSProperties; tipSide: Side }>(function ToolColumn({ style, tipSide }, ref) {
  return (
    <div ref={ref} role="toolbar" aria-orientation="vertical" aria-label="Drawing tools" className={cn("absolute flex flex-col items-center gap-0.5", PANEL)} style={style} onPointerDown={stop}>
      <ToolColumnBody tipSide={tipSide} />
    </div>
  );
});

interface ActionBarBodyProps {
  tipSide: Side;
  busy: boolean;
  /** OCR is running. */
  reading: boolean;
  onCopy: () => void;
  onCopyText: () => void;
  onSave: () => void;
  onQuickSave: () => void;
  onCancel: () => void;
}

interface ActionBarProps extends ActionBarBodyProps {
  style: React.CSSProperties;
}

const ActionBarBody = memo(function ActionBarBody({ tipSide, busy, reading, onCopy, onCopyText, onSave, onQuickSave, onCancel }: ActionBarBodyProps) {
  const tool = useEditor((s) => s.tool);
  const note =
    tool === "redact"
      ? "Redact permanently replaces these pixels in the copied or saved image."
      : tool === "blur" || tool === "pixelate"
        ? "Visual effect only — blurred or pixelated text can sometimes be recovered."
        : null;

  return (
    <>
      <div role="toolbar" aria-label="Screenshot actions" className={cn("flex items-center gap-0.5", PANEL)}>
        <Hint label="Quick save to your default folder" keys={isMac ? "⌘+⇧+S" : "Ctrl+Shift+S"} side={tipSide}>
          <Button variant="ghost" size="icon-sm" onClick={onQuickSave} disabled={busy} aria-label="Quick save" className={TOOL_CONTROL}>
            <Zap />
          </Button>
        </Hint>
        <Hint label="Copy text (OCR)" keys={isMac ? "⌘+⇧+C" : "Ctrl+Shift+C"} side={tipSide}>
          <Button variant="ghost" size="icon-sm" onClick={onCopyText} disabled={busy} aria-label="Copy text" className={TOOL_CONTROL}>
            {reading ? <LoaderCircle className="animate-spin" /> : <ScanText />}
          </Button>
        </Hint>
        <Hint label="Save as…" keys={`${mod}+S`} side={tipSide}>
          <Button variant="ghost" size="icon-sm" onClick={onSave} disabled={busy} aria-label="Save as" className={TOOL_CONTROL}>
            <Save />
          </Button>
        </Hint>
        <Hint label="Copy to clipboard" keys={`${mod}+C`} side={tipSide}>
          <Button variant="ghost" size="icon-sm" onClick={onCopy} disabled={busy} aria-label="Copy" className={TOOL_CONTROL}>
            {busy && !reading ? <Check /> : <Copy />}
          </Button>
        </Hint>
        <Separator orientation="vertical" className="mx-0.5 h-5 bg-overlay-panel-separator" />
        <Hint label="Close" keys="Esc" side={tipSide}>
          <Button variant="ghost" size="icon-sm" onClick={onCancel} aria-label="Close" className={TOOL_CONTROL}>
            <X />
          </Button>
        </Hint>
      </div>
      {note ? (
        <p className="absolute top-full right-0 mt-1.5 w-max max-w-80 rounded-md bg-popover/95 px-2 py-1 text-xs text-popover-foreground shadow">{note}</p>
      ) : null}
    </>
  );
});

/** Horizontal output-action bar, docked under the selection's right edge. */
export const ActionBar = forwardRef<HTMLDivElement, ActionBarProps>(function ActionBar({ style, ...body }, ref) {
  return (
    <div ref={ref} className="absolute" style={style} onPointerDown={stop}>
      <ActionBarBody {...body} />
    </div>
  );
});
