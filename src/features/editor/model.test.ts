import { describe, expect, it } from "vitest";

import { arrowGeometry, isMeaningful, itemBounds, nextStepNumber, normalizeRect, stepFontSize, stepLabelColor, textBaseline, toRenderRequest, translateItem, type Item } from "./model";

describe("geometry", () => {
  it("normalizes rects dragged up-left", () => {
    expect(normalizeRect({ x: 20, y: 20, width: -10, height: -5 })).toEqual({ x: 10, y: 15, width: 10, height: 5 });
  });

  it("matches the Rust arrow-head test", () => {
    const g = arrowGeometry(0, 0, 100, 0, 4)!;
    expect(g.tip).toEqual([100, 0]);
    expect(g.left[0]).toBeCloseTo(84);
    expect(g.right[0]).toBeCloseTo(84);
    expect(arrowGeometry(1, 1, 1, 1, 4)).toBeNull();
  });

  it("uses the shared text baseline formula", () => {
    expect(textBaseline(10, 20, 0)).toBeCloseTo(10 + 1.005 * 20);
    expect(textBaseline(10, 20, 2)).toBeCloseTo(10 + 1.005 * 20 + 2 * 1.25 * 20);
  });

  it("translates every item kind", () => {
    const items: Item[] = [
      { id: "1", type: "line", x1: 0, y1: 0, x2: 1, y2: 1, color: "#000", strokeWidth: 1 },
      { id: "2", type: "pen", points: [[0, 0], [2, 2]], color: "#000", strokeWidth: 1 },
      { id: "3", type: "text", x: 1, y: 1, text: "a", color: "#000", fontSize: 10 },
      { id: "4", type: "redact", x: 0, y: 0, width: 4, height: 4, color: "#000" },
    ];
    const moved = items.map((i) => translateItem(i, 3, 4));
    expect(moved[0]).toMatchObject({ x1: 3, y1: 4, x2: 4, y2: 5 });
    expect((moved[1] as Extract<Item, { type: "pen" }>).points).toEqual([[3, 4], [5, 6]]);
    expect(moved[2]).toMatchObject({ x: 4, y: 5 });
    expect(moved[3]).toMatchObject({ x: 3, y: 4 });
  });

  it("computes pen bounds", () => {
    expect(itemBounds({ id: "p", type: "pen", points: [[5, 1], [1, 9], [3, 3]], color: "#000", strokeWidth: 1 })).toEqual({ x: 1, y: 1, width: 4, height: 8 });
  });

  it("drops accidental zero-size items", () => {
    expect(isMeaningful({ id: "a", type: "rect", x: 0, y: 0, width: 1, height: 40, color: "#000", strokeWidth: 1 })).toBe(false);
    expect(isMeaningful({ id: "a", type: "arrow", x1: 0, y1: 0, x2: 1, y2: 1, color: "#000", strokeWidth: 1 })).toBe(false);
    expect(isMeaningful({ id: "a", type: "text", x: 0, y: 0, text: "  ", color: "#000", fontSize: 10 })).toBe(false);
    expect(isMeaningful({ id: "a", type: "pen", points: [[1, 1]], color: "#000", strokeWidth: 1 })).toBe(true);
  });
});

describe("toRenderRequest", () => {
  it("separates redactions and effects and keeps annotation order", () => {
    const items: Item[] = [
      { id: "r", type: "redact", x: 10, y: 10, width: -5, height: 5, color: "#000000" },
      { id: "a", type: "arrow", x1: 0, y1: 0, x2: 9, y2: 9, color: "#e11d48", strokeWidth: 4 },
      { id: "b", type: "blur", x: 0, y: 0, width: 5, height: 5, strength: 8 },
      { id: "t", type: "text", x: 0, y: 0, text: "hi", color: "#000", fontSize: 20 },
      { id: "e", type: "text", x: 0, y: 0, text: "", color: "#000", fontSize: 20 },
    ];
    const req = toRenderRequest({ x: 10.4, y: 20.6, width: 100.2, height: 50.5 }, items);
    expect(req.crop).toEqual({ x: 10, y: 21, width: 100, height: 51 });
    expect(req.redactions).toEqual([{ rect: { x: 5, y: 10, width: 5, height: 5 }, color: "#000000" }]);
    expect(req.effects).toEqual([{ kind: "blur", rect: { x: 0, y: 0, width: 5, height: 5 }, strength: 8 }]);
    expect(req.annotations.map((a) => a.type)).toEqual(["arrow", "text"]);
  });
});

describe("step markers and highlighter", () => {
  it("numbers steps one past the highest, even with gaps", () => {
    const step = (id: string, number: number): Item => ({ id, type: "step", x: 0, y: 0, radius: 16, number, color: "#e11d48" });
    expect(nextStepNumber([])).toBe(1);
    expect(nextStepNumber([step("a", 1), step("b", 4)])).toBe(5);
    expect(nextStepNumber([step("a", 999)])).toBe(999);
  });

  it("step bounds are the disc's box and steps translate by their centre", () => {
    const s: Item = { id: "s", type: "step", x: 50, y: 40, radius: 10, number: 1, color: "#000" };
    expect(itemBounds(s)).toEqual({ x: 40, y: 30, width: 20, height: 20 });
    expect(translateItem(s, 5, -5)).toMatchObject({ x: 55, y: 35 });
  });

  it("label contrast and font size mirror the Rust renderer", () => {
    expect(stepLabelColor("#facc15")).toBe("#000000");
    expect(stepLabelColor("#e11d48")).toBe("#ffffff");
    expect(stepLabelColor("#fff")).toBe("#000000");
    expect(stepFontSize(20, 7)).toBeCloseTo(23);
    expect(stepFontSize(20, 42)).toBeCloseTo(19);
  });

  it("exports steps and highlights as annotations in z-order", () => {
    const items: Item[] = [
      { id: "h", type: "highlight", points: [[0, 0], [10, 0]], color: "#facc15", strokeWidth: 20 },
      { id: "s", type: "step", x: 5, y: 5, radius: 16, number: 2, color: "#e11d48" },
    ];
    const req = toRenderRequest({ x: 0, y: 0, width: 20, height: 20 }, items);
    expect(req.annotations).toEqual([
      { type: "highlight", points: [[0, 0], [10, 0]], color: "#facc15", strokeWidth: 20 },
      { type: "step", x: 5, y: 5, radius: 16, number: 2, color: "#e11d48" },
    ]);
  });
});
