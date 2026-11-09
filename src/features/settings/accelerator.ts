/**
 * Convert a keydown into a Tauri global-shortcut accelerator string such as
 * `CommandOrControl+Shift+9`. Returns `null` until a non-modifier key is
 * pressed, and rejects combinations without a modifier (they'd hijack typing).
 */
export interface AcceleratorKey {
  code: string;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}

const MODIFIER_CODES = new Set(["ControlLeft", "ControlRight", "MetaLeft", "MetaRight", "ShiftLeft", "ShiftRight", "AltLeft", "AltRight", "OSLeft", "OSRight"]);

const NAMED: Record<string, string> = {
  Space: "Space",
  Enter: "Enter",
  Tab: "Tab",
  Backquote: "`",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
  PrintScreen: "PrintScreen",
  Insert: "Insert",
  Delete: "Delete",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
};

function keyName(code: string): string | null {
  let m = /^Key([A-Z])$/.exec(code);
  if (m) return m[1];
  m = /^Digit([0-9])$/.exec(code);
  if (m) return m[1];
  m = /^Numpad([0-9])$/.exec(code);
  if (m) return `Num${m[1]}`;
  m = /^F([1-9]|1[0-9]|2[0-4])$/.exec(code);
  if (m) return `F${m[1]}`;
  return NAMED[code] ?? null;
}

export type AcceleratorResult = { ok: true; accelerator: string } | { ok: false; reason: "incomplete" | "no-modifier" | "unsupported" };

export function acceleratorFromEvent(e: AcceleratorKey, isMac: boolean): AcceleratorResult {
  if (MODIFIER_CODES.has(e.code)) return { ok: false, reason: "incomplete" };
  const key = keyName(e.code);
  if (!key) return { ok: false, reason: "unsupported" };
  const parts: string[] = [];
  const primary = isMac ? e.metaKey : e.ctrlKey;
  const secondary = isMac ? e.ctrlKey : e.metaKey;
  if (primary) parts.push("CommandOrControl");
  if (secondary) parts.push(isMac ? "Control" : "Super");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  const isFKey = /^F\d+$/.test(key) || key === "PrintScreen";
  if (!parts.length && !isFKey) return { ok: false, reason: "no-modifier" };
  parts.push(key);
  return { ok: true, accelerator: parts.join("+") };
}

/** Human-readable label for an accelerator on the current platform. */
export function formatAccelerator(accel: string, isMac: boolean): string[] {
  return accel.split("+").map((p) => {
    switch (p) {
      case "CommandOrControl":
      case "CmdOrCtrl":
        return isMac ? "⌘" : "Ctrl";
      case "Control":
        return isMac ? "⌃" : "Ctrl";
      case "Shift":
        return isMac ? "⇧" : "Shift";
      case "Alt":
        return isMac ? "⌥" : "Alt";
      case "Super":
        return isMac ? "⌘" : "Win";
      default:
        return p;
    }
  });
}
