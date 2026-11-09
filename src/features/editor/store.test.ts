import { beforeEach, describe, expect, it } from "vitest";

import type { Item } from "./model";
import { translateItem } from "./model";
import { useEditor } from "./store";

const rect = (id: string): Item => ({ id, type: "rect", x: 0, y: 0, width: 10, height: 10, color: "#ff0000", strokeWidth: 2 });

describe("editor store", () => {
  beforeEach(() => useEditor.getState().reset());

  it("undo reverses add, redo reapplies", () => {
    const s = useEditor.getState();
    s.addItem(rect("a"));
    s.addItem(rect("b"));
    expect(useEditor.getState().items.map((i) => i.id)).toEqual(["a", "b"]);
    useEditor.getState().undo();
    expect(useEditor.getState().items.map((i) => i.id)).toEqual(["a"]);
    useEditor.getState().redo();
    expect(useEditor.getState().items.map((i) => i.id)).toEqual(["a", "b"]);
  });

  it("a new edit clears the redo branch", () => {
    const s = useEditor.getState();
    s.addItem(rect("a"));
    s.addItem(rect("b"));
    useEditor.getState().undo();
    useEditor.getState().addItem(rect("c"));
    expect(useEditor.getState().future).toHaveLength(0);
    useEditor.getState().redo();
    expect(useEditor.getState().items.map((i) => i.id)).toEqual(["a", "c"]);
  });

  it("undoes moves and deletes", () => {
    const s = useEditor.getState();
    s.addItem(rect("a"));
    const moved = translateItem(useEditor.getState().items[0], 5, 7);
    useEditor.getState().replaceItem(moved);
    expect(useEditor.getState().items[0]).toMatchObject({ x: 5, y: 7 });
    useEditor.getState().select("a");
    useEditor.getState().deleteSelected();
    expect(useEditor.getState().items).toHaveLength(0);
    useEditor.getState().undo();
    expect(useEditor.getState().items[0]).toMatchObject({ x: 5, y: 7 });
    useEditor.getState().undo();
    expect(useEditor.getState().items[0]).toMatchObject({ x: 0, y: 0 });
  });

  it("undo on empty history is a no-op", () => {
    useEditor.getState().undo();
    useEditor.getState().redo();
    expect(useEditor.getState().items).toEqual([]);
  });

  it("changing style restyles the selected item with history", () => {
    useEditor.getState().addItem(rect("a"));
    useEditor.getState().setStyle({ color: "#00ff00" });
    expect(useEditor.getState().items[0]).toMatchObject({ color: "#00ff00" });
    useEditor.getState().undo();
    expect(useEditor.getState().items[0]).toMatchObject({ color: "#ff0000" });
  });

  it("changing style with nothing selected only changes the default", () => {
    useEditor.getState().addItem(rect("a"));
    useEditor.getState().select(null);
    const pastLen = useEditor.getState().past.length;
    useEditor.getState().setStyle({ color: "#0000ff" });
    expect(useEditor.getState().style.color).toBe("#0000ff");
    expect(useEditor.getState().items[0]).toMatchObject({ color: "#ff0000" });
    expect(useEditor.getState().past.length).toBe(pastLen);
  });

  it("redaction colour applies only to redactions", () => {
    useEditor.getState().addItem({ id: "r", type: "redact", x: 0, y: 0, width: 5, height: 5, color: "#000000" });
    useEditor.getState().setStyle({ redactColor: "#ffffff" });
    expect(useEditor.getState().items[0]).toMatchObject({ color: "#ffffff" });
    useEditor.getState().setStyle({ color: "#ff0000" });
    expect(useEditor.getState().items[0]).toMatchObject({ color: "#ffffff" });
  });

  it("step and highlighter styles apply only to their own kind", () => {
    useEditor.getState().addItem({ id: "s", type: "step", x: 0, y: 0, radius: 16, number: 1, color: "#000000" });
    useEditor.getState().setStyle({ stepRadius: 24, color: "#ff0000" });
    expect(useEditor.getState().items[0]).toMatchObject({ radius: 24, color: "#ff0000" });
    useEditor.getState().addItem({ id: "h", type: "highlight", points: [[0, 0]], color: "#facc15", strokeWidth: 20 });
    useEditor.getState().setStyle({ color: "#00ff00" });
    expect(useEditor.getState().items[1]).toMatchObject({ color: "#facc15" });
    useEditor.getState().setStyle({ highlightColor: "#22c55e", highlightWidth: 30 });
    expect(useEditor.getState().items[1]).toMatchObject({ color: "#22c55e", strokeWidth: 30 });
  });
});
