import { create } from "zustand";

import { type Item, type Tool, translateItem } from "./model";

const HISTORY_LIMIT = 200;

export interface ToolStyle {
  color: string;
  strokeWidth: number;
  fontSize: number;
  redactColor: string;
  blurStrength: number;
  pixelSize: number;
  /** Step marker radius in image px. */
  stepRadius: number;
  highlightColor: string;
  highlightWidth: number;
}

export const DEFAULT_STYLE: ToolStyle = {
  color: "#e11d48",
  strokeWidth: 4,
  fontSize: 24,
  redactColor: "#000000",
  blurStrength: 8,
  pixelSize: 12,
  stepRadius: 16,
  highlightColor: "#facc15",
  highlightWidth: 20,
};

export interface EditorState {
  tool: Tool;
  style: ToolStyle;
  items: Item[];
  selectedId: string | null;
  /** Id of the text item currently open in the inline text editor. */
  editingTextId: string | null;
  past: Item[][];
  future: Item[][];

  setTool: (tool: Tool) => void;
  setStyle: (patch: Partial<ToolStyle>) => void;
  select: (id: string | null) => void;
  addItem: (item: Item) => void;
  /** Replace one item and record history (e.g. after a move or edit). */
  replaceItem: (item: Item) => void;
  removeItem: (id: string) => void;
  /** Translate every item (keeps annotations pinned when the crop origin moves). */
  shiftItems: (dx: number, dy: number) => void;
  deleteSelected: () => void;
  setEditingText: (id: string | null) => void;
  undo: () => void;
  redo: () => void;
  reset: (style?: Partial<ToolStyle>) => void;
}

/** Push the current items to history and clear the redo branch (EDT-03). */
function withHistory(s: EditorState, items: Item[]): Partial<EditorState> {
  const past = [...s.past, s.items];
  if (past.length > HISTORY_LIMIT) past.shift();
  return { items, past, future: [] };
}

function applyStyle(item: Item, patch: Partial<ToolStyle>): Item {
  switch (item.type) {
    case "redact":
      return patch.redactColor ? { ...item, color: patch.redactColor } : item;
    case "blur":
      return patch.blurStrength ? { ...item, strength: patch.blurStrength } : item;
    case "pixelate":
      return patch.pixelSize ? { ...item, strength: patch.pixelSize } : item;
    case "step":
      if (patch.color === undefined && patch.stepRadius === undefined) return item;
      return { ...item, color: patch.color ?? item.color, radius: patch.stepRadius ?? item.radius };
    case "highlight":
      if (patch.highlightColor === undefined && patch.highlightWidth === undefined) return item;
      return { ...item, color: patch.highlightColor ?? item.color, strokeWidth: patch.highlightWidth ?? item.strokeWidth };
    case "text":
      if (patch.color === undefined && patch.fontSize === undefined) return item;
      return { ...item, color: patch.color ?? item.color, fontSize: patch.fontSize ?? item.fontSize };
    default:
      if (patch.color === undefined && patch.strokeWidth === undefined) return item;
      return { ...item, color: patch.color ?? item.color, strokeWidth: patch.strokeWidth ?? item.strokeWidth };
  }
}

export const useEditor = create<EditorState>((set, get) => ({
  tool: "select",
  style: DEFAULT_STYLE,
  items: [],
  selectedId: null,
  editingTextId: null,
  past: [],
  future: [],

  setTool: (tool) => set({ tool, selectedId: tool === "select" ? get().selectedId : null }),

  setStyle: (patch) =>
    set((s) => {
      const style = { ...s.style, ...patch };
      const sel = s.items.find((i) => i.id === s.selectedId);
      if (!sel) return { style };
      // Editing style while an item is selected restyles that item (EDT-02).
      const next = applyStyle(sel, patch);
      if (next === sel) return { style };
      return { style, ...withHistory(s, s.items.map((i) => (i.id === sel.id ? next : i))) };
    }),

  select: (id) => set({ selectedId: id }),

  addItem: (item) => set((s) => ({ ...withHistory(s, [...s.items, item]), selectedId: item.id })),

  replaceItem: (item) =>
    set((s) => {
      const idx = s.items.findIndex((i) => i.id === item.id);
      if (idx < 0) return {};
      const items = s.items.slice();
      items[idx] = item;
      return withHistory(s, items);
    }),

  removeItem: (id) =>
    set((s) => {
      if (!s.items.some((i) => i.id === id)) return {};
      return { ...withHistory(s, s.items.filter((i) => i.id !== id)), selectedId: s.selectedId === id ? null : s.selectedId };
    }),

  shiftItems: (dx, dy) =>
    set((s) => {
      if (s.items.length === 0 || (dx === 0 && dy === 0)) return {};
      return withHistory(s, s.items.map((i) => translateItem(i, dx, dy)));
    }),

  deleteSelected: () => {
    const id = get().selectedId;
    if (id) get().removeItem(id);
  },

  setEditingText: (id) => set({ editingTextId: id }),

  undo: () =>
    set((s) => {
      if (!s.past.length) return {};
      const past = s.past.slice();
      const items = past.pop()!;
      return { items, past, future: [s.items, ...s.future], selectedId: null, editingTextId: null };
    }),

  redo: () =>
    set((s) => {
      if (!s.future.length) return {};
      const [items, ...future] = s.future;
      return { items, past: [...s.past, s.items], future, selectedId: null, editingTextId: null };
    }),

  reset: (style) =>
    set({
      tool: "select",
      style: { ...DEFAULT_STYLE, ...style },
      items: [],
      selectedId: null,
      editingTextId: null,
      past: [],
      future: [],
    }),
}));
