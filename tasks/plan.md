# Implementation Plan: FastShot MVP

Source spec: [docs/prd.md](../docs/prd.md) (v0.1, 2026-11-09).

## Overview
A tray-resident Tauri 2 app: a global hotkey freezes every display, shows a
full-screen overlay per monitor, lets the user pick a region / window / display,
annotate inline, and Copy or Save. All pixels stay in Rust; the UI gets a
preview over a scoped custom protocol and sends back structured operations.
Rust re-renders the final image and applies secure redaction **last**.

## Architecture Decisions
- **Frontend: React 19 + TS + Vite + Tailwind v4 + shadcn/ui + Zustand** (per
  PRD §8.1). The repo's SvelteKit template is replaced.
- **Annotation preview: SVG layer over the frozen `<img>`** instead of Konva.
  SVG gives hit-testing, crisp scaling and zero extra runtime. Coordinates are
  stored in *image pixels relative to the crop*, so preview and export share
  one coordinate system.
- **Export renderer: Rust `tiny-skia` (shapes) + `ab_glyph` (text) + `image`
  (encode)**. A bundled font is used by both CSS and Rust so text positions match.
- **Order of operations:** crop → effects (blur / pixelate) → annotations
  (by z-order) → secure redactions → fresh encode. Clipboard and file share
  the same `render_export()` function (SEC-04).
- **Preview transport:** `fastshot://` custom URI protocol serving the
  session's monitor frames as fast-compressed PNG from memory. No temp files,
  and the URL is only valid for the live session id.
- **Coordinates:** Each overlay window covers exactly one monitor. The UI
  converts CSS px to image px with `imageWidth / innerWidth` and sends
  monitor-local pixel rects. Rust validates and clamps them. Window rects from
  `xcap` are mapped with `img_px / monitor_bounds` ratios, which handles both
  Windows (physical) and macOS (points) conventions.
- **Window capture = crop of the frozen frame** at the hovered window's
  visible rect (the PRD's "visible-window" contract). Our own windows are
  excluded by PID.
- **Save path is chosen in Rust** via the native dialog. The frontend can never
  name a file path (PRD §8.4 security note).
- **Settings persisted by Rust** (Store plugin). The UI gets narrow commands
  only, not generic store or fs permissions.

## Task List
Tracked in [tasks/todo.md](todo.md).

## Risks and Mitigations
| Risk | Impact | Mitigation |
|------|--------|------------|
| Overlay z-order / focus differs by OS | High | Opaque frozen-image overlay (no transparency); macOS raises window level above the menu bar; explicit focus on show |
| Mixed-DPI drift | High | Per-monitor windows; ratio-based mapping; unit tests on geometry |
| macOS not testable on this machine | High | cfg-gated code; must be verified on hardware before release |
| Preview vs export text mismatch | Medium | Same embedded font in CSS and Rust; same baseline math |
| 4K preview latency | Medium | Capture monitors in parallel; encode PNG with fast settings on background threads |

## Open Questions (from PRD §15, still open)
- Product name / licence / release channel. The scaffold says MIT and "fastshot"; kept as a working name.
- Default shortcut collision testing on real machines.
