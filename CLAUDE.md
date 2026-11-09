# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

FastShot is a Tauri 2 desktop screenshot tool for Windows and macOS. It has a React 19 + TypeScript frontend and a Rust backend. Its central promise is **permanent redaction**: the final image is always rendered natively in Rust, and the frontend never produces exported pixels. Product and security requirements are in [docs/prd.md](docs/prd.md). Code comments cite requirement IDs from it (e.g. `SEC-01`, `CAP-06`, `SYS-04`, `PRD §8.2`). The plan and task list are in [tasks/](tasks/).

## Commands

Package manager is **pnpm** (Node 22+, stable Rust).

| Command | Purpose |
| --- | --- |
| `pnpm tauri dev` | Run the full desktop app (starts Vite on port 1420, strict) |
| `pnpm dev` | Frontend only. Most features need Tauri IPC, so this is of limited use |
| `pnpm build` | `tsc -b` + Vite build |
| `pnpm typecheck` | TypeScript project check |
| `pnpm test` | Vitest, run once (jsdom; files `src/**/*.test.{ts,tsx}`) |
| `pnpm test:rust` | `cargo test --manifest-path src-tauri/Cargo.toml` |
| `pnpm tauri build` | Production bundle → `src-tauri/target/release/bundle/` |

Run a single test:
- Frontend: `pnpm vitest run src/features/editor/store.test.ts` or `pnpm vitest run -t "test name"`
- Rust: `cargo test --manifest-path src-tauri/Cargo.toml <name_substring>` (e.g. `redaction_wins_over_annotations_and_effects`)

Before finishing a change, run `pnpm test`, `pnpm test:rust`, and `pnpm build`.

On Windows, if Tauri reports `cargo metadata: program not found`, prepend `$env:USERPROFILE\.cargo\bin` to `PATH` in the session.

## Architecture

### One frontend bundle, two kinds of window
[src/main.tsx](src/main.tsx) chooses the UI at runtime, and each UI is lazy-loaded:
- **`main` window**: `SettingsApp` (settings, onboarding, permission recovery). Rust creates it on demand and destroys it when closed, then sends a `navigate` event (`{view, error}`) to switch an existing window (`show_main` in [src-tauri/src/desktop/windows.rs](src-tauri/src/desktop/windows.rs)).
- **`overlay-N` windows**: one per monitor, running `OverlayApp` (capture selection plus the editor). They are detected through `window.__FASTSHOT_OVERLAY_WINDOW__` / `__FASTSHOT_OVERLAY__`, which Rust injects with an initialization script.

Overlays are **pre-warmed** at startup and reused for fast captures. To start a capture, Rust resumes them, `eval`s a new `__FASTSHOT_OVERLAY__` boot payload and dispatches a `fastshot:capture-start` DOM event. `OverlayHost` remounts `OverlayApp` keyed on `sessionId:monitorIndex`. When a capture ends, overlays are hidden, kept fully warm briefly, then suspended with a low WebView2 memory target on Windows. A long idle timeout destroys them; the next capture rebuilds them on demand.

**Native overlay (experimental, Windows, `nativeOverlay` setting):** [desktop/native_overlay/](src-tauri/src/desktop/native_overlay/) replaces the WebView overlay with Win32/GDI windows created per capture on a dedicated thread and destroyed when it ends; overlay WebViews are then never pre-warmed. Scope is selection, window/display picking, copy and copy text, which go through the shared `render_to_clipboard` / `recognize_selection` / `finish_text_copy` helpers in `commands.rs`. If it fails to start, `run_capture` falls back to the WebView overlay. The real-window test is opt-in: `cargo test --manifest-path src-tauri/Cargo.toml -- --ignored native_overlay_drag_and_copy`.

### Capture flow (Rust, [src-tauri/src/desktop/mod.rs](src-tauri/src/desktop/mod.rs))
`trigger_capture` (from the tray, the global shortcut, or the `begin_capture` command) → destroy the settings window → check permission (macOS asks at most once per run) → `capture::freeze_all_monitors()` freezes the pixels **before any overlay is shown** → build the `Session` → `windows::open_overlays`. Each overlay calls `overlay_ready` when it has painted. A 2.5 s fallback reveals overlays regardless, so the user can always press Esc.

### Sessions and frame transport
- [session.rs](src-tauri/src/session.rs): there is only ever one active session (`SessionManager::try_prepare` returns `Busy` otherwise). Frozen frames live **only in memory** and are freed when the session ends. Nothing is written to disk.
- [desktop/protocol.rs](src-tauri/src/desktop/protocol.rs): the `fastshot://` custom protocol (on Windows, `http://fastshot.localhost`) serves session frames as PNG from memory. Their URLs stop working once the session ends. The CSP in [tauri.conf.json](src-tauri/tauri.conf.json) allows exactly this origin.

### Export pipeline: the security core
The frontend sends a declarative `RenderRequest` (crop + items) through `export_capture`. Rust treats it as untrusted: `RenderRequest::validate` in [imaging/model.rs](src-tauri/src/imaging/model.rs) enforces limits. Then [imaging/render.rs](src-tauri/src/imaging/render.rs) `render_export` builds a **fresh** raster in a fixed order: crop → blur/pixelate effects → annotations (array order = z-order) → **opaque redactions, always last**. The same compositor handles clipboard, save-dialog, and quick-save output, and encoding strips metadata. [imaging/security_tests.rs](src-tauri/src/imaging/security_tests.rs) holds the regression fixtures for these guarantees, so keep them passing.

Blur and pixelate are cosmetic and do not count as redaction. Only the `redact` item type is secure.

### OCR (copy text)
`extract_text` ([commands.rs](src-tauri/src/commands.rs)) runs `render_export` and hands that raster to [ocr/](src-tauri/src/ocr/), so redacted text is never read. Recognition is on-device: `Windows.Media.Ocr` on Windows ([ocr/winrt.rs](src-tauri/src/ocr/winrt.rs), needs an OCR language pack; small images are upscaled 2x) and Apple Vision on macOS ([ocr/vision.rs](src-tauri/src/ocr/vision.rs)). Both backends return boxes that `ocr::assemble` orders into lines. Text goes to the clipboard and the capture ends; with no text found the overlay stays open. The real-engine test is opt-in: `cargo test --manifest-path src-tauri/Cargo.toml -- --ignored ocr_reads_text_but_not_redactions`. The macOS backend can be type-checked from Windows only in a standalone crate, because a Tauri dependency needs a C compiler for the Apple target.

### Editor model and TS/Rust parity
Editor item coordinates are **image pixels relative to the crop's top-left**, the same space the Rust renderer uses. The SVG preview in the overlay has to match the Rust render, so several constants and functions are duplicated across both languages and must stay in sync:
- [src/features/editor/model.ts](src/features/editor/model.ts) ↔ [render.rs](src-tauri/src/imaging/render.rs) / [text.rs](src-tauri/src/imaging/text.rs): `TEXT_LINE_HEIGHT`, `TEXT_ASCENT`, `HIGHLIGHT_ALPHA`, `STEP_BASELINE`, `stepFontSize`, `stepLabelColor`, `MAX_STEP_NUMBER`
- The annotation font is shipped twice: `public/fonts/AnnotationFont.ttf` (preview) and `src-tauri/assets/AnnotationFont.ttf` (render).

When you add an item type or field, update the TS `Item` union, the serde model and validation in `imaging/model.rs`, and the drawing code in `render.rs`.

Frontend state is Zustand: [features/editor/store.ts](src/features/editor/store.ts) (tools, items, undo/redo) and [features/capture/store.ts](src/features/capture/store.ts) (selection, hover-picking of windows).

### IPC and permissions: adding a command
Tauri commands are scoped per window. To add one:
1. Implement it in [commands.rs](src-tauri/src/commands.rs) and register it in `generate_handler!` in [lib.rs](src-tauri/src/lib.rs).
2. Add its name to the `AppManifest::commands` list in [build.rs](src-tauri/build.rs). This generates the `allow-<name>` permission under `permissions/autogenerated/`.
3. Grant it only to the window that needs it: [capabilities/default.json](src-tauri/capabilities/default.json) (main/settings: no pixel access, no export) or [capabilities/overlay.json](src-tauri/capabilities/overlay.json) (overlays: session and export, no settings writes, no filesystem).
4. Add a typed wrapper to `api` in [src/shared/ipc.ts](src/shared/ipc.ts). Rust errors arrive as `{ code, message }`; use `errorMessage()`.

### Settings and desktop integration
- [settings.rs](src-tauri/src/settings.rs): preferences are persisted through the Store plugin **from Rust only**, and the UI never gets generic store access. `validate()` runs on update. The TS `Settings` interface in `ipc.ts` mirrors the Rust struct (camelCase).
- `desktop/`: tray, macOS app menu (`app_menu.rs`), global shortcut (default `Ctrl/Cmd+Shift+9`; conflicts are recorded and shown in Settings rather than failing startup), autostart, and window management.
- This is a tray app. Closing the main window hides it unless `quitOnClose` is set, and `RunEvent::ExitRequested` without a code is prevented. On macOS the activation policy is `Accessory` (no Dock icon). Platform-specific code uses `cfg!(target_os = ...)`. Geometry is in physical pixels on Windows and in points on macOS (see [geometry.rs](src-tauri/src/geometry.rs)).

### UI conventions
shadcn/ui primitives (style `radix-nova`, lucide icons) live in [src/components/ui/](src/components/ui/), with Tailwind v4 configured in [src/index.css](src/index.css). The path alias is `@/` → `src/`.
