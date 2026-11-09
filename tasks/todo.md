# FastShot MVP — Task List

## Phase 1: Foundation
- [ ] T1 Replace SvelteKit with React+TS+Vite+Tailwind+shadcn; add Tauri plugins (SPK-01)
  - Accept: `pnpm build` and `cargo check` succeed; main window renders React.
- [ ] T2 Rust capture adapter: list displays, freeze all monitors, session store (SPK-02)
  - Accept: `begin_capture` returns per-monitor geometry; frames held only in memory.
- [ ] T3 Geometry + validation module with unit tests (SPK-05)
  - Accept: CSS→px, window→monitor mapping, crop clamping, zero-area rejection tested.

### Checkpoint A: cargo test + pnpm build green

## Phase 2: Capture experience
- [ ] T4 Overlay windows per monitor + `fastshot://` preview protocol (SPK-04, CAP-06)
- [ ] T5 Region / window / display selection UI, Esc cancel, single session (CAP-02..05)
- [ ] T6 Tray, global shortcut w/ conflict reporting, single instance (SPK-07, CAP-01, SYS-01, SYS-04)
- [ ] T7 macOS permission preflight + recovery UI (SPK-03, Flow C)

### Checkpoint B: hotkey → overlay → cancel works with no stuck windows

## Phase 3: Editor
- [ ] T8 Editor store (Zustand): tools, annotations, selection, undo/redo + tests (EDT-03)
- [ ] T9 SVG canvas: pen/rect/ellipse/line/arrow/text, select/move/delete (EDT-01, EDT-02)
- [ ] T10 Inline toolbar, properties, keyboard shortcuts (EDT-04, §7.2–7.3)

## Phase 4: Privacy & export
- [ ] T11 Rust renderer: effects, annotations, redaction-last (SEC-01, SEC-02, SEC-04)
- [ ] T12 Fresh PNG/JPEG encode, clipboard, native save dialog in Rust (OUT-01..03, SEC-03)
- [ ] T13 Security regression fixtures: secret pattern, layer ordering, parity, no metadata (QA #8–10, #14)

### Checkpoint C: golden path: hotkey → arrow → redact → copy and save

## Phase 5: Hardening
- [ ] T14 Settings window + persisted preferences (SYS-02)
- [ ] T15 First-run onboarding (SYS-03)
- [ ] T16 Accessibility pass, README, support matrix

### Checkpoint D: all automated tests pass; manual matrix documented
