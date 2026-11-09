import { describe, expect, it } from "vitest";

import { acceleratorFromEvent, formatAccelerator } from "./accelerator";

const ev = (code: string, m: { ctrl?: boolean; meta?: boolean; shift?: boolean; alt?: boolean } = {}) => ({
  code,
  ctrlKey: !!m.ctrl,
  metaKey: !!m.meta,
  shiftKey: !!m.shift,
  altKey: !!m.alt,
});

describe("acceleratorFromEvent", () => {
  it("builds the default shortcut on both platforms", () => {
    expect(acceleratorFromEvent(ev("Digit9", { ctrl: true, shift: true }), false)).toEqual({ ok: true, accelerator: "CommandOrControl+Shift+9" });
    expect(acceleratorFromEvent(ev("Digit9", { meta: true, shift: true }), true)).toEqual({ ok: true, accelerator: "CommandOrControl+Shift+9" });
  });

  it("waits while only modifiers are held", () => {
    expect(acceleratorFromEvent(ev("ShiftLeft", { shift: true }), false)).toEqual({ ok: false, reason: "incomplete" });
  });

  it("requires a modifier except for function keys", () => {
    expect(acceleratorFromEvent(ev("KeyA"), false)).toEqual({ ok: false, reason: "no-modifier" });
    expect(acceleratorFromEvent(ev("F8"), false)).toEqual({ ok: true, accelerator: "F8" });
    expect(acceleratorFromEvent(ev("PrintScreen"), false)).toEqual({ ok: true, accelerator: "PrintScreen" });
  });

  it("maps letters, arrows and the secondary modifier", () => {
    expect(acceleratorFromEvent(ev("KeyS", { alt: true, shift: true }), false)).toEqual({ ok: true, accelerator: "Alt+Shift+S" });
    expect(acceleratorFromEvent(ev("ArrowUp", { ctrl: true }), true)).toEqual({ ok: true, accelerator: "Control+Up" });
  });

  it("rejects unsupported keys", () => {
    expect(acceleratorFromEvent(ev("IntlRo", { ctrl: true }), false)).toEqual({ ok: false, reason: "unsupported" });
  });
});

describe("formatAccelerator", () => {
  it("uses platform labels", () => {
    expect(formatAccelerator("CommandOrControl+Shift+9", false)).toEqual(["Ctrl", "Shift", "9"]);
    expect(formatAccelerator("CommandOrControl+Shift+9", true)).toEqual(["⌘", "⇧", "9"]);
  });
});
