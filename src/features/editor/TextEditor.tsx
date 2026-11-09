import { useEffect, useRef, useState } from "react";

import { ANNOTATION_FONT, TEXT_LINE_HEIGHT, type ItemOf } from "./model";

interface Props {
  item: ItemOf<"text">;
  /** Crop origin in image px, and image px per CSS px. */
  origin: { x: number; y: number };
  scale: number;
  onCommit: (text: string) => void;
  onCancel: () => void;
}

/**
 * Inline text entry (EDT-04): Enter confirms, Shift+Enter adds a line, Esc
 * cancels, clicking elsewhere confirms.
 */
export function TextEditor({ item, origin, scale, onCommit, onCancel }: Props) {
  const ref = useRef<HTMLTextAreaElement>(null);
  const [text, setText] = useState(item.text);
  const done = useRef(false);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.focus();
    el.select();
  }, []);

  const finish = (commit: boolean) => {
    if (done.current) return;
    done.current = true;
    if (commit) onCommit(text);
    else onCancel();
  };

  const fontPx = item.fontSize / scale;
  const lines = text.split("\n");
  const longest = Math.max(1, ...lines.map((l) => l.length));

  return (
    <textarea
      ref={ref}
      aria-label="Annotation text"
      value={text}
      spellCheck={false}
      rows={lines.length}
      onChange={(e) => setText(e.target.value)}
      onBlur={() => finish(true)}
      onPointerDown={(e) => e.stopPropagation()}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === "Escape") {
          e.preventDefault();
          finish(false);
        } else if (e.key === "Enter" && !e.shiftKey) {
          e.preventDefault();
          finish(true);
        }
      }}
      className="annotation-text absolute resize-none overflow-hidden rounded-none border border-dashed border-white/80 bg-transparent p-0 outline-none [box-shadow:0_0_0_1px_rgba(0,0,0,0.6)]"
      style={{
        left: (origin.x + item.x) / scale,
        top: (origin.y + item.y) / scale,
        fontFamily: ANNOTATION_FONT,
        fontSize: fontPx,
        lineHeight: TEXT_LINE_HEIGHT,
        color: item.color,
        caretColor: item.color,
        width: `${longest + 1.5}ch`,
        minWidth: fontPx * 2,
      }}
    />
  );
}
