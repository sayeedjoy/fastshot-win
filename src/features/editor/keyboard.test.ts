import { describe, expect, it } from "vitest";

import { keyAction, type KeyLike } from "./keyboard";

const k = (key: string, mods: Partial<KeyLike> = {}): KeyLike => ({ key, ctrlKey: false, metaKey: false, shiftKey: false, altKey: false, ...mods });
const win = { isMac: false, typing: false, editing: true };
const mac = { isMac: true, typing: false, editing: true };

describe("keyAction", () => {
  it("Esc cancels in every phase", () => {
    expect(keyAction(k("Escape"), { ...win, editing: false })).toEqual({ type: "cancel" });
    expect(keyAction(k("Escape"), win)).toEqual({ type: "cancel" });
  });

  it("uses Ctrl on Windows and Cmd on macOS", () => {
    expect(keyAction(k("c", { ctrlKey: true }), win)).toEqual({ type: "copy" });
    expect(keyAction(k("c", { metaKey: true }), win)).toBeNull();
    expect(keyAction(k("c", { metaKey: true }), mac)).toEqual({ type: "copy" });
    expect(keyAction(k("s", { metaKey: true }), mac)).toEqual({ type: "save" });
  });

  it("undo / redo variants", () => {
    expect(keyAction(k("z", { ctrlKey: true }), win)).toEqual({ type: "undo" });
    expect(keyAction(k("Z", { ctrlKey: true, shiftKey: true }), win)).toEqual({ type: "redo" });
    expect(keyAction(k("y", { ctrlKey: true }), win)).toEqual({ type: "redo" });
    expect(keyAction(k("y", { metaKey: true }), mac)).toBeNull();
  });

  it("ignores everything while typing, including copy and delete", () => {
    const typing = { ...win, typing: true };
    expect(keyAction(k("c", { ctrlKey: true }), typing)).toBeNull();
    expect(keyAction(k("Backspace"), typing)).toBeNull();
    expect(keyAction(k("Escape"), typing)).toBeNull();
  });

  it("editing keys are inactive while still selecting", () => {
    const selecting = { ...win, editing: false };
    expect(keyAction(k("c", { ctrlKey: true }), selecting)).toBeNull();
    expect(keyAction(k("a"), selecting)).toBeNull();
  });

  it("delete and tool letters", () => {
    expect(keyAction(k("Delete"), win)).toEqual({ type: "delete" });
    expect(keyAction(k("Backspace"), mac)).toEqual({ type: "delete" });
    expect(keyAction(k("a"), win)).toEqual({ type: "tool", tool: "arrow" });
    expect(keyAction(k("X"), win)).toEqual({ type: "tool", tool: "redact" });
    expect(keyAction(k("a", { altKey: true }), win)).toBeNull();
  });

  it("quick save and marker tools", () => {
    expect(keyAction(k("S", { ctrlKey: true, shiftKey: true }), win)).toEqual({ type: "quicksave" });
    expect(keyAction(k("s", { metaKey: true, shiftKey: true }), mac)).toEqual({ type: "quicksave" });
    expect(keyAction(k("n"), win)).toEqual({ type: "tool", tool: "step" });
    expect(keyAction(k("h"), win)).toEqual({ type: "tool", tool: "highlight" });
  });
});
