import type { Tool } from "./model";

export type KeyAction =
  | { type: "cancel" }
  | { type: "copy" }
  | { type: "save" }
  | { type: "quicksave" }
  | { type: "undo" }
  | { type: "redo" }
  | { type: "delete" }
  | { type: "tool"; tool: Tool };

export interface KeyLike {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}

export interface KeyContext {
  isMac: boolean;
  /** True while a text field has focus — editing keys belong to it. */
  typing: boolean;
  /** Editing actions only make sense once a region is selected. */
  editing: boolean;
}

export const TOOL_KEYS: Record<string, Tool> = {
  v: "select",
  a: "arrow",
  r: "rect",
  e: "ellipse",
  l: "line",
  p: "pen",
  h: "highlight",
  n: "step",
  t: "text",
  x: "redact",
  b: "blur",
  m: "pixelate",
};

export const TOOL_SHORTCUT: Partial<Record<Tool, string>> = Object.fromEntries(
  Object.entries(TOOL_KEYS).map(([k, t]) => [t, k.toUpperCase()]),
);

/** Map a keydown to an editor action (PRD §7.3). */
export function keyAction(e: KeyLike, ctx: KeyContext): KeyAction | null {
  if (ctx.typing) return null;
  const mod = ctx.isMac ? e.metaKey : e.ctrlKey;
  const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;

  if (key === "Escape") return { type: "cancel" };
  if (!ctx.editing) return null;

  if (mod && !e.altKey) {
    if (key === "c" && !e.shiftKey) return { type: "copy" };
    if (key === "s") return e.shiftKey ? { type: "quicksave" } : { type: "save" };
    if (key === "z") return e.shiftKey ? { type: "redo" } : { type: "undo" };
    if (key === "y" && !ctx.isMac) return { type: "redo" };
    return null;
  }
  if (e.ctrlKey || e.metaKey || e.altKey) return null;
  if (key === "Delete" || key === "Backspace") return { type: "delete" };
  const tool = TOOL_KEYS[key];
  return tool ? { type: "tool", tool } : null;
}
