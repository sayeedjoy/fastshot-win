# Product Requirements Document

## Cross-Platform Screenshot & Redaction Tool

**Working name:** TBD  
**Version:** 0.1 — implementation-ready draft  
**Date:** 9 November 2026  
**Platforms:** Windows and macOS  
**Primary stack:** Tauri 2, Rust, React, TypeScript  
**Status:** Proposed; decisions marked *TBD* require validation

> **Product promise:** Take a screenshot, mark it up, permanently hide sensitive information, and share it — quickly, without an account or cloud upload.

---

## 1. Executive Summary

Build a lightweight screenshot-taking desktop application inspired by Lightshot's quick area-selection workflow but offering a more capable editor, especially **secure, permanent redaction**. The product should remain simple enough for casual use and reliable enough for developers, support teams, creators, and privacy-conscious users.

The MVP must support region, full-display, and visible-window capture; global shortcuts; a fast selection overlay; core annotations; solid-fill redaction; blur/pixelation; undo/redo; image copy/save; multi-monitor environments; and a system-tray/menu-bar presence. All core operations work locally and offline. Cloud upload, OCR, scrolling capture, recording, and screenshot history are deliberately excluded from MVP.

**Recommended delivery strategy:** Prove capture and overlay behavior on actual Windows and macOS devices first. Then implement editing and export, with redaction correctness as a release blocker.

## 2. Problem, Opportunity, and Audience

### 2.1 Problem statement

Users often need to show a small portion of their screen, annotate a bug, or share a UI example while concealing credentials, email addresses, customer data, or other private information. Existing workflows can feel slow, bloated, platform-specific, or unclear about whether a blur/black rectangle actually destroys underlying pixels.

### 2.2 Target personas

| Persona | Main job | Essential capability |
|---|---|---|
| Developer / QA engineer | Report bugs and document flows | Fast region capture; arrows; numbered steps later |
| Support / operations | Explain processes without exposing user data | Permanent redaction; clipboard export |
| Designer / creator | Give visual feedback | Shapes; text; clean saved images |
| Everyday user | Share a small screen portion | One shortcut; select; copy; close |

### 2.3 Jobs to be done

- When I see a bug, I want to select part of my screen and copy an annotated screenshot in seconds.
- When a screenshot contains sensitive information, I want to erase that information irreversibly from the image I share.
- When I use multiple displays, I want the selection box and final screenshot to align correctly even at different scaling settings.
- When I do not need advanced tools, I want the app to stay out of my way.

## 3. Product Goals and Non-Goals

### 3.1 Goals

1. **Fast:** Minimize steps from hotkey to clipboard.
2. **Trustworthy:** Make secure redaction clear and technically verifiable.
3. **Lightweight:** Run unobtrusively with a small background footprint.
4. **Cross-platform:** Deliver materially consistent behavior on Windows and macOS.
5. **Offline-first:** No sign-in or networking required for core use.
6. **Approachable:** A first-time user can capture and copy without reading documentation.

### 3.2 Explicit MVP non-goals

- Video / GIF recording and webcam capture.
- Cloud storage, sharing links, collaboration, or accounts.
- OCR and searchable screenshot history.
- Scrolling screenshots or full-page browser capture.
- Rich image editing such as layers panel, filters, and retouching.
- Linux support in the first release.
- Guaranteed capture of protected, DRM-controlled, minimized, or secure system windows.

### 3.3 Design principles

- **Capture first, edit second.** Avoid forcing users into a full editor when copy is enough.
- **One-click common actions.** Copy, save, and cancel must be visible and keyboard-accessible.
- **Secure by default.** Redaction is an irreversible raster operation on export, not a cosmetic annotation.
- **Never upload silently.** Core functionality needs no server.
- **Native where it matters.** Rust handles capture, coordinates, pixel processing, and file export.

## 4. MVP Scope and Priority

**Priority definitions:** P0 = required for public MVP; P1 = high-value follow-up; P2 = later exploration.

| Capability | Priority | Release |
|---|---|---|
| Region selection screenshot | P0 | MVP |
| Full-display screenshot | P0 | MVP |
| Visible-window screenshot | P0 | MVP |
| Configurable global region shortcut | P0 | MVP |
| Frozen-screen selection overlay | P0 | MVP |
| Rectangle, ellipse, line, arrow, text, pen | P0 | MVP |
| Solid-fill secure redaction | P0 | MVP |
| Blur and pixelation (visual effects) | P0 | MVP |
| Undo and redo | P0 | MVP |
| Copy PNG image to clipboard | P0 | MVP |
| Save PNG / JPEG | P0 | MVP |
| Multi-monitor and mixed-DPI support | P0 | MVP |
| Tray/menu-bar controls and quit | P0 | MVP |
| Settings for shortcut, default save folder, format | P0 | MVP |
| Selection magnifier, resize handles | P1 | v1.1 |
| Numbered step markers, highlighter | P1 | v1.1 |
| Quick-save with custom naming templates | P1 | v1.1 |
| Launch at login and optional capture delay | P1 | v1.1 |
| Screenshot history (opt-in, local only) | P2 | Later |
| OCR / text extraction (on-device; Windows.Media.Ocr / Apple Vision) | P2 | Implemented |
| Scrolling capture | P2 | Later |
| Cloud sharing / integrations | P2 | Later, opt-in |

**MVP trade-off:** A simple inline editor is acceptable. A complex editing workspace is not required. Redaction safety and capture reliability take precedence over visual polish.

## 5. Key User Flows

### 5.1 Flow A — Region capture and copy (golden path)

1. App is running in tray/menu bar.
2. User presses the configured hotkey (proposed default: **Command/Ctrl + Shift + 9**, subject to collision testing).
3. App obtains a frozen screen image **before** showing its overlay so the overlay cannot appear in the capture.
4. An overlay appears on each available monitor; pointer becomes a crosshair.
5. User drags a rectangle; on release, a compact editing toolbar appears near the selection.
6. User can copy immediately or annotate first.
7. User chooses **Copy**; app creates a fresh flattened image, writes it to the system clipboard, and dismisses the overlay/editor.
8. No screenshot is saved to disk unless the user explicitly chooses Save.

### 5.2 Flow B — Securely redact and save

1. User captures a region or window.
2. User selects **Redact** and draws over sensitive content.
3. Redaction area is rendered opaque in the preview, and is visually distinct from cosmetic blur/pixelate.
4. User selects **Save** and chooses PNG (default) or JPEG.
5. The Rust pipeline overwrites redacted pixels in a new output raster **before** it encodes the file; it strips image metadata and does not include the source image as a hidden layer or attachment.
6. User receives a success state with the final location; the original unredacted image is not automatically written to disk.

### 5.3 Flow C — Permission denial and recovery (macOS)

1. User initiates the first capture.
2. If required screen-capture access is not granted, show a brief explanation and a button to open the appropriate macOS privacy settings.
3. After the user grants access, present clear retry/relaunch guidance when required by the OS/API; never loop on permission prompts.
4. If the user declines, the app remains usable for settings, documentation, and quit.

### 5.4 Capture lifecycle

`Idle → PreparingCapture → Selecting → Editing → Exporting → Idle`

Cancellation can return from Selecting or Editing to Idle. Failures show a recoverable error, release overlays, and return to Idle. Only one capture session may be active at a time.

## 6. Functional Requirements and Acceptance Criteria

The IDs below are intended to map directly to GitHub issues and tests.

### 6.1 Capture and selection

**CAP-01 · Global shortcut (P0)**  
Register one configurable global region-capture shortcut while the app runs. Detect registration conflicts and allow a different key combination.

- **Accept when:** shortcut launches capture from another app; duplicate keydown does not open concurrent sessions; conflict is communicated without crashing.

**CAP-02 · Region capture (P0)**  
Render a frozen monitor image behind an interactive rectangle selection. Support dragging in all directions, moving/restarting a selection, visible pixel dimensions, Escape to cancel.

- **Accept when:** output exactly matches the selected physical-pixel rectangle; zero-area selections are rejected; cancel leaves no stale overlay.

**CAP-03 · Full-display capture (P0)**  
Capture one selected monitor at native resolution. Expose access via tray and capture-mode menu.

- **Accept when:** selected display dimensions and image pixels match, including Retina / high-DPI displays.

**CAP-04 · Visible-window capture (P0)**  
Allow selecting an on-screen application window and capturing its visible content, within underlying OS/API limits. Exclude the screenshot app's own windows where feasible.

- **Accept when:** a regular, visible, non-protected window is captured correctly on each OS; unsupported/minimized/protected windows produce a clear fallback message rather than a blank success.

**CAP-05 · Multi-monitor behavior (P0)**  
Discover displays, handle negative virtual desktop coordinates and mixed scale factors, and show selection UI on the monitor being targeted.

- **Accept when:** one region or one monitor is captured correctly on a two-display mixed-DPI setup; coordinate round-tripping has no meaningful visible offset.

**CAP-06 · Overlay management (P0)**  
Capture underlying pixels before overlay visibility, preserve display geometry, avoid self-capture, hide cleanly on cancel, copy, save, monitor disconnect, and failure.

- **Accept when:** selection chrome never appears in the exported image, and the overlay does not remain stuck after failure.

### 6.2 Editor and annotations

**EDT-01 · Core annotation tools (P0)**  
Provide pen, rectangle, ellipse, straight line, arrow, and text. Each new item has position, size/geometry, color, and stroke width, with sensible defaults.

- **Accept when:** drawing, moving, and editing text work with mouse/trackpad; all annotations appear at their intended exported positions.

**EDT-02 · Selection and manipulation (P0)**  
Users can select an annotation, drag it, delete it, and change supported properties. The tool toolbar does not cover the active selection where reasonably avoidable.

- **Accept when:** an existing item can be repositioned without modifying the screenshot background.

**EDT-03 · Undo/redo (P0)**  
Maintain an ordered command history for annotation addition, deletion, movement, and redaction selection changes in the **current editing session**.

- **Accept when:** Ctrl/Cmd+Z reverses an edit; Ctrl/Cmd+Shift+Z or Ctrl/Cmd+Y redoes as appropriate; a new edit clears the redo branch.

**EDT-04 · Inline editing (P0)**  
Offer a compact toolbar near the capture, keyboard Escape to cancel/close active tool, Enter to confirm text, and obvious Copy/Save actions. Avoid a separate full editor window unless needed for OS/windowing reliability.

- **Accept when:** a user can capture, draw one arrow, and copy without entering a complex multi-panel interface.

### 6.3 Redaction and image effects

**SEC-01 · Secure solid redaction (P0; release blocker)**  
Redact selected rectangular regions with an opaque, user-chosen solid color, default black. At export, permanently replace those pixels in a fresh output raster. Redaction regions must be applied **after all normal annotations and effects**, preventing their contents from being revealed by layer ordering.

- **Accept when:** the output image contains no source detail in redacted coordinates; exported PNG/JPEG is freshly encoded without original image payloads or edit layers; reopening the exported image cannot recover hidden content.

**SEC-02 · Blur and pixelation (P0)**  
Provide adjustable blur and pixelate regions for aesthetic obscuring and visual focus. Label these as visual effects, **not a privacy guarantee**.

- **Accept when:** effects are visibly distinct from secure redaction and can be combined with annotations; UI does not claim blur/pixelate permanently protects secrets.

**SEC-03 · Private capture handling (P0; release blocker)**  
Core capture/edit/export flows run offline, use in-memory images where practical, create no automatic unredacted screenshots/history, and avoid logging image bytes or sensitive text.

- **Accept when:** no source screenshot is written to a persistent temp/history folder by default; no screenshot leaves the device; generated outputs contain no accidental image metadata or hidden source data.

**SEC-04 · Export-safe transformations (P0; release blocker)**  
Apply crop, annotations and cosmetic effects, then overwrite redaction regions before final image encoding. The clipboard and saved file must use the same sanitized compositor path.

- **Accept when:** same-session clipboard copy and save have equivalent sanitized visible pixels; test fixtures with high-contrast secret patterns remain inaccessible under redaction.

### 6.4 Export, settings, and desktop integration

**OUT-01 · Clipboard copy (P0)**  
Copy the final rendered image as an image payload compatible with common apps. Default clipboard content is PNG-compatible raster data; do not copy file paths unless specifically requested in a future feature.

- **Accept when:** paste works in common document/chat/image applications on Windows and macOS; success is signaled; failure is actionable.

**OUT-02 · Save screenshot (P0)**  
Use a native Save dialog; default to PNG, optional JPEG quality settings; support a configurable default folder and collision-safe suggested filenames such as `Screenshot-YYYY-MM-DD-HHMMSS.png`.

- **Accept when:** save creates a readable image in the chosen location; cancel does not write a file; JPEG output handles transparency on a defined solid background.

**OUT-03 · Export consistency (P0)**  
The saved/copy image resolution must match the captured pixel region (unless explicitly scaled in a later feature). Editor chrome, cursor selection lines, and handles must never be exported.

- **Accept when:** pixel dimensions remain stable before and after editing; image output contains only requested edits.

**SYS-01 · Tray/menu-bar app (P0)**  
When the main window closes, remain active in tray/menu bar by default; expose Capture Region, Capture Display, Capture Window, Settings, and Quit. Provide a configurable Quit-on-close choice if practical.

- **Accept when:** app remains accessible without a persistent large window; Quit fully unregisters hotkeys and exits.

**SYS-02 · Preferences (P0)**  
Persist global shortcut, capture mode default, image format, JPEG quality, default save directory, UI theme (system/light/dark), and annotation defaults.

- **Accept when:** valid settings survive restart; shortcut conflicts or inaccessible folders are surfaced and recoverable.

**SYS-03 · First-run setup (P0)**  
Explain the shortcut, capture permissions, redaction-vs-blur difference, and tray behavior in a lightweight, skippable onboarding sequence.

- **Accept when:** first capture remains reachable quickly; denied permissions never cause silent failures.

**SYS-04 · Single instance and lifecycle (P0)**  
A second app launch should focus/open settings or trigger the existing app rather than spawning a duplicate tray process. Clean up overlays, native resources, and hotkeys on exit.

- **Accept when:** repeated launches do not create multiple tray icons or competing shortcut handlers.

## 7. UX and Interaction Specification

### 7.1 Main interface model

**Default state:** No persistent main window; app lives in the system tray or macOS menu bar. Settings opens as a small, conventional window. **Capture state:** fullscreen, borderless overlays based on frozen screen content. **Edit state:** selected region with a floating annotation toolbar and small Copy / Save / Close controls. Colors and icons should be subtle, high contrast, and consistent with OS conventions.

### 7.2 Toolbar hierarchy

- **Primary:** Select, Arrow, Rectangle, Pen, Text, Redact.
- **Secondary/overflow:** Ellipse, Line, Blur, Pixelate, Undo, Redo, tool options.
- **Always prominent:** Copy, Save, Cancel.

Make **Redact** visually and linguistically distinct: e.g., `Redact (permanent)` with a short description. Blur and pixelate appear under `Effects`, never as secure-redaction alternatives.

### 7.3 Keyboard and pointer behavior

| Action | Proposed behavior |
|---|---|
| Capture region | Configurable global shortcut; initial candidate Cmd/Ctrl+Shift+9 |
| Cancel / close tool | Esc |
| Copy final screenshot | Cmd/Ctrl+C when not typing in text editor |
| Save | Cmd/Ctrl+S |
| Undo | Cmd/Ctrl+Z |
| Redo | Cmd/Ctrl+Shift+Z; optionally Ctrl+Y on Windows |
| Delete selected annotation | Delete / Backspace outside text editing |
| Draw region | Click and drag in any direction |
| Select an annotation | Click annotation / selection tool |

All shortcut assignments are proposals and must be checked for collisions on both OSes. Provide accessible focus indicators, tooltips, labels, and a non-shortcut workflow.

### 7.4 Empty, error, and recovery states

Show useful, nontechnical messaging for: denied macOS screen-recording permission, shortcut already registered elsewhere, monitor disconnected mid-capture, capture API failure, protected-window limitation, clipboard blocked, invalid save path, and low-memory export failure. Never falsely report success.

## 8. Technical Architecture

### 8.1 Proposed technology decisions

| Concern | Choice | Reason / caveat |
|---|---|---|
| App shell | Tauri 2 | Native integration and compact distribution |
| UI | React + TypeScript + Vite | Efficient component workflow |
| Styling | Tailwind CSS + shadcn/ui | Consistent controls, customizable styling |
| Editor canvas | Konva.js + react-konva | Interactive vector annotations; validate memory/performance |
| UI state | Zustand | Small editor/session stores |
| Native capture | Rust + `xcap` adapter | Monitor/window capture available; platform behavior needs spikes |
| Image pipeline | Rust `image` crate | Cropping, raster compositing, image encoding |
| Shortcuts | Tauri Global Shortcut plugin | Register configurable desktop shortcut |
| Clipboard | Tauri Clipboard Manager plugin | Copy image data |
| Settings | Tauri Store plugin | Persist non-sensitive preferences |
| Save dialog | Tauri Dialog plugin | Native save interaction |
| Updates | Tauri Updater plugin (post-MVP or release-hardening) | Signed updates require release infrastructure |

These are architectural proposals, not commitments to particular dependency versions. Pin and validate current compatible releases during implementation.

### 8.2 Component responsibilities

**Rust domain layer:** monitor enumeration; capture engine abstraction; OS permission/status checks; logical-to-physical coordinate conversion; crop and image buffer ownership; export compositing; safe file encoding; clipboard bridge; hotkey/tray integration.

**React UI layer:** overlay selection interaction; annotation toolbar and canvas; text editing; tool properties; undo/redo operations; settings and onboarding. UI may render a low-cost display preview, but the export pipeline must not trust UI overlay rendering for security.

**Tauri integration layer:** commands/events, window management, scoped plugin capabilities, application lifecycle. Keep privileged commands accessible only from the required application windows.

### 8.3 Capture and edit data flow

1. **Native capture:** Rust snapshots selected monitor(s)/window(s), records pixel dimensions, display ID, scale factor, bounds, and capture session ID.
2. **Overlay preview:** UI receives an appropriate preview via a controlled local transport; capture remains in memory. Avoid repeatedly serializing large buffers over the IPC boundary.
3. **Region selection:** UI reports selection in logical coordinates with source display/session identifiers; Rust validates and converts it to physical pixels.
4. **Edit state:** React stores annotation operations as structured objects (`type`, `bounds`, `color`, `stroke`, `content`, `zIndex`) and redaction regions separately.
5. **Native export:** Rust reads validated crop/operations, renders cosmetic edits and annotations, and applies secure redaction last to a fresh RGBA output buffer.
6. **Output:** Rust encodes a fresh PNG/JPEG or writes a sanitized image to clipboard; sensitive buffers are released when the session ends.

**Avoid:** Encoding the raw screenshot as a base64 JSON string for every move/resize; storing a source image in the app's persistent state; assuming the browser CSS pixel grid equals image pixels.

### 8.4 Suggested Rust commands (conceptual contract)

```ts
// Conceptual request/response shapes, not final generated bindings.
type CaptureMode = 'region' | 'display' | 'window';
type PixelRect = { x: number; y: number; width: number; height: number };

type CaptureSession = {
  id: string;
  sourceId: string;
  pixelWidth: number;
  pixelHeight: number;
  scaleFactor: number;
  previewUrl: string; // controlled local / asset transport
};

list_displays(): Promise<DisplayInfo[]>
list_capture_windows(): Promise<WindowInfo[]>
begin_capture(mode: CaptureMode, sourceId?: string): Promise<CaptureSession>
export_capture(args: {
  sessionId: string;
  crop: PixelRect;
  annotations: AnnotationOperation[];
  redactions: PixelRect[];
  effectRegions: EffectOperation[];
  format: 'png' | 'jpeg';
  destination: 'clipboard' | 'file';
  outputPath?: string;
}): Promise<ExportResult>
cancel_capture(sessionId: string): Promise<void>
```

**Security note:** Verify that a file path was explicitly selected via a native Save dialog (or allowlisted policy). Do not expose unrestricted arbitrary file writes through frontend commands. Validate image bounds and operations in Rust even if TypeScript has already checked them.

### 8.5 Proposed repository layout

```text
src/
  app/             # routes, app bootstrap, global UI
  features/
    capture/       # region overlay and selection
    editor/        # canvas, tools, undo stack
    settings/      # preferences and onboarding
  shared/          # UI controls, types, utilities
src-tauri/
  src/
    capture/       # backend adapter and platform helpers
    imaging/       # crop, effects, redact, encode
    commands/      # Tauri invocations and validation
    desktop/       # tray, hotkeys, windows, permissions
    lib.rs
  capabilities/    # scoped Tauri capabilities
  tauri.conf.json
.github/workflows/  # checks, multi-OS release builds
```

### 8.6 Platform-specific design requirements

**macOS:** Test Apple Silicon and Intel; request screen-recording access with explanatory onboarding; detect denial; explain restart/retry if OS requires it. Window capture should be validated against macOS system restrictions, overlays, Space/fullscreen behavior, Retina scaling, and menu-bar lifecycle. Apple ScreenCaptureKit is a possible native fallback if a cross-platform adapter cannot deliver needed behavior.

**Windows:** Test Windows 11 x64 as the primary target; assess Windows 10 x64 compatibility as best effort rather than a launch promise. Validate WebView2 behavior, high-DPI scaling (100/125/150/200%), multiple monitors with negative coordinates, exclusive-fullscreen apps, and protected/accelerated content behavior. No requirement to bypass protected content.

**Primary browser/webview risk:** A visually transparent/borderless webview is not automatically guaranteed to have equivalent z-order and input focus on both OSes. Prototype fullscreen selection overlays early and maintain a native window-management fallback.

## 9. Privacy, Security, and Redaction Threat Model

### 9.1 What redaction guarantees

For the **exported image only**, any area marked with `Secure Redact` is replaced with new opaque pixel values. The saved file or image clipboard payload must contain no recoverable original pixel information within those coordinates. The output must be rasterized and freshly encoded without source image chunks, hidden layers, or editable redaction geometry.

This does **not** claim to erase data already present in other apps, original documents, system screenshots, clipboard history maintained by the OS, backups, or a user's separately saved source image. The editor may still hold the unredacted capture in memory until the capture session ends to enable undo/repositioning; do not misrepresent live preview redaction as immediate memory erasure.

### 9.2 Threats and mitigations

| Threat | Required mitigation |
|---|---|
| Black rectangle can be removed in exported image | Flatten the raster; overwrite pixel buffer; never export editable overlays |
| Blur/pixelation reveals text through reconstruction | Explicitly label as cosmetic effects, not secure redaction |
| Original image accidentally written to disk | No automatic source save, history, persistent draft, or debug dump by default |
| Hidden image data retained as metadata | Fresh encoder output; no source payloads, EXIF, thumbnails, ancillary chunks |
| Annotation draws sensitive content over blackout | Apply secure redaction as the final visual operation |
| UI sends out-of-bounds crop/region | Validate coordinates and format in Rust |
| Unintended clipboard exposure | Copy only after explicit action; warn that clipboard managers may retain copies |
| App telemetry leaks screen content | No screenshot bytes/text, selected titles, paths, or pixels in telemetry |
| Native command abuse | Scope Tauri capabilities; validate paths; no generic filesystem access |

### 9.3 Privacy defaults

No account. No auto-upload. No analytics required for app operation. No screenshots persisted by default. Diagnostics should be opt-in and redact file paths/process titles where feasible. If a history feature ships later, it must be explicitly opt-in and define retention/deletion controls.

## 10. Non-Functional Requirements

The following are **proposed acceptance targets**, to be measured on an agreed reference laptop/desktop; they are not measured claims.

| Category | Proposed target |
|---|---|
| Responsiveness | 95% of simple region captures reach visible selection overlay within 1.5 s after hotkey on reference hardware |
| Export latency | 95% of 1080p region images with ≤10 annotations export in ≤1 s, excluding Save dialog interaction |
| Idle resource use | Aim for ≤100 MB app-private RAM at idle; report process and helper-process measurements separately |
| Crash reliability | ≥99.5% of test capture/edit/export sessions complete without crash in release-candidate QA |
| Image correctness | Pixel alignment within 1 physical pixel for selection/crop in supported monitor layouts |
| Accessibility | Fully keyboard-operable main controls, visible focus, descriptive tooltips, high-contrast annotations |
| Localization readiness | UI copy externalized; English first; layouts accommodate translated strings |
| Network independence | All P0 features function with networking disabled |
| Packaging | Signed/notarized macOS artifacts and signed Windows releases targeted for public distribution, subject to credentials/cost |
| Dependency/security | No unnecessary broad plugins or permissions; review Rust/JS dependency advisories in CI |

Measure cold-start, idle memory, CPU while selecting, screenshot duration, large-image export and failure cleanup. Report results on both operating systems before final release.

## 11. QA and Acceptance Test Matrix

### 11.1 Must-pass scenarios

1. **Single 1080p monitor:** drag region in both directions; copy; verify dimensions and exact pixels.
2. **Mixed DPI:** two monitors with different scaling, including one left of the primary with negative coordinates; validate selection and crop alignment.
3. **Retina / HiDPI:** export pixel dimensions reflect native image resolution, not CSS dimensions.
4. **Window capture:** test a normal visible window on each OS; verify graceful handling of protected/minimized windows.
5. **Permission denial:** macOS first-run denial, settings recovery, and capture retry behavior.
6. **Shortcut collision:** another app registers proposed shortcut; alternate shortcut registration works.
7. **Cancel paths:** Escape during capture, during text edit, and before Save; no overlay stuck, no output written.
8. **Redaction security fixture:** redact a region containing a known high-contrast secret pattern; decode output pixels, inspect file structure, and verify no original bytes/embedded data are retained.
9. **Layer ordering:** annotation under/over redaction still results in fully opaque redaction in output.
10. **Clipboard parity:** clipboard image and saved PNG present equivalent sanitized pixels from same edit state.
11. **Large image:** full 4K display with annotations; handle memory pressure and provide failure recovery rather than crash.
12. **Lifecycle:** closing settings, switching monitors, system sleep/wake, and repeated hotkey use do not leak overlays or duplicate tray processes.
13. **Offline:** disconnect network and exercise every P0 workflow.
14. **Export formats:** PNG and JPEG decode correctly; JPEG quality and alpha-background behavior are defined.

### 11.2 Automated test plan

- **Rust unit tests:** crop bounds, mixed-DPI coordinate conversion, color fills, redaction-last ordering, PNG/JPEG fresh encoding, invalid-path handling.
- **Frontend tests:** tool selection, annotation state transitions, undo/redo, shortcut keyboard rules, settings validation.
- **Integration tests:** Tauri command schema/authorization, session cleanup, clipboard and Save dialog error handling.
- **OS end-to-end manual tests:** capture APIs, permissions, window focus, tray lifecycle, multiple displays.
- **Security regression fixture:** compare decoded output across redacted region to the chosen opaque fill; assert source secret patterns are absent from output bytes and image metadata.

## 12. Release Plan and Milestones

**Milestone 0 — Feasibility spike:** Tauri app launches on Windows/macOS; xcap can enumerate displays and capture a display and visible window; frozen full-screen overlay aligns correctly with at least one mixed-DPI setup; macOS permissions are understood. **Exit gate:** platform-specific blockers documented, fallback path chosen.

**Milestone 1 — Capture experience:** Tray, hotkeys, display/window choices, selection overlay, cancel/cleanup, screen preview, coordinate tests. **Exit gate:** CAP-01 through CAP-06 pass on both OSes.

**Milestone 2 — Editor:** Core annotation controls, text, move/select, undo/redo, visible Copy/Save actions. **Exit gate:** golden-path annotated screenshot can be copied.

**Milestone 3 — Privacy and export:** Solid redaction, cosmetic blur/pixelation, native fresh-image rendering, clipboard/file output, secure image tests. **Exit gate:** SEC-01 through SEC-04 pass; no source screenshot persistence by default.

**Milestone 4 — Product hardening:** First-run guidance, settings, permissions, UI refinement, multi-monitor QA, performance benchmarks, release builds, signing/notarization, documentation. **Exit gate:** P0 acceptance suite passes on supported OSes and no unresolved critical data-leak issues.

### 12.1 Definition of Done for MVP

- Every P0 feature and its acceptance criteria is implemented or explicitly removed from scope via approved change.
- Windows and macOS installs, shortcut capture, region/window selection, annotation, secure redaction, copy, and save are tested on physical devices.
- Exported redacted pixels are irrecoverable from the output image by known tests; no metadata/layers contain original pixels.
- First-run permissions and error states are understandable.
- No cloud service or account is needed for the core workflow.
- Installers and release notes are published, with a documented support matrix.

## 13. Success Metrics

**Primary metric:** `Capture-to-Copy Completion` — percentage of initiated, user-intended region captures that successfully reach a clipboard image.

**Secondary metrics:** median and P95 hotkey-to-selection latency, percentage of captures successfully exported, number of recoverable errors, user-reported redaction trust, crash-free session rate, and first-run successful capture rate.

**Collection policy:** Instrument performance locally during development. Production telemetry, if adopted, is opt-in and restricted to anonymous event counts/timings. It must not collect screenshots, text, titles, selected app names, file paths, or clipboard content.

## 14. Risks, Dependencies, and Mitigations

| Risk | Impact | Mitigation / decision checkpoint |
|---|---|---|
| `xcap` behavior differs by OS or window type | High | Complete native capture spike before editor investment; isolate behind adapter; consider ScreenCaptureKit or Windows-native backend fallback |
| Overlay focus/transparency differs on macOS vs Windows | High | Prototype actual always-on-top/input interactions early; use native platform window controls if needed |
| Multi-monitor DPI errors | High | Use physical pixel model; central coordinate mapping with automated tests and mixed-DPI devices |
| Accidentally reversible redaction | Critical | Make native redaction final-pass and block release on security regression failures |
| Editor causes high RAM usage with 4K images | Medium | Keep base pixels in Rust; use low-resolution preview where suitable; profile memory and IPC copies |
| Shortcut conflicts with OS/other apps | Medium | Configurable shortcut, conflict messaging, fallback menu actions |
| Window capture cannot capture protected/minimized content | Medium | Document limits; choose visible-window MVP contract, never bypass OS protection |
| Signing/notarization and updater infrastructure | Medium | Plan certificates and CI secrets early; unsigned development builds remain separate from release policy |

## 15. Open Decisions for Product Owner

1. **Product name and brand:** TBD; verify trademark/domain availability before publishing.
2. **Licensing:** Private, MIT, Apache-2.0, or dual-licensed open source? Decide before public repository launch.
3. **Public OS minimums:** Finalize Windows 10 support policy and minimum macOS version after feasibility spike.
4. **MVP window capture:** Confirm whether `visible windows only` is sufficient; minimized/occluded capture is intentionally not guaranteed.
5. **Default shortcut:** Validate Cmd/Ctrl+Shift+9 and provide nonconflicting presets.
6. **Redaction behavior:** MVP uses rectangular solid-fill redaction; freeform/redact-text detection is out of scope.
7. **Release channel:** GitHub Releases, website downloads, stores, or multiple channels; changes signing and update logistics.
8. **Analytics:** Default off recommended; clarify whether any opt-in anonymous diagnostics will exist.

## 16. Proposed First Development Tickets

| Ticket | Scope | Dependencies |
|---|---|---|
| SPK-01 | Scaffold Tauri 2 + React/TS app and CI | None |
| SPK-02 | Rust capture adapter: list displays/capture | SPK-01 |
| SPK-03 | Windows/macOS screen-permission handling | SPK-02 |
| SPK-04 | Single-monitor frozen selection overlay | SPK-02 |
| SPK-05 | Mixed-DPI coordinate translation + tests | SPK-04 |
| SPK-06 | Window enumeration and visible-window capture | SPK-02 |
| SPK-07 | Tray, hotkey, single-instance lifecycle | SPK-01 |
| SPK-08 | React annotation canvas + undo/redo | SPK-04 |
| SPK-09 | Secure native redaction + security tests | SPK-02, SPK-08 |
| SPK-10 | Fresh native PNG/JPEG export and clipboard | SPK-08, SPK-09 |
| SPK-11 | Settings, native Save dialog, onboarding | SPK-07, SPK-10 |
| SPK-12 | Dual-platform regression matrix and release packaging | All P0 tickets |

## 17. Implementation References

The following primary sources informed feasibility and integration choices (review again before pinning dependencies):

- Tauri 2 Global Shortcut plugin: https://v2.tauri.app/plugin/global-shortcut/
- Tauri 2 Clipboard Manager / permission reference: https://v2.tauri.app/plugin/clipboard/
- Tauri 2 configuration / window options: https://v2.tauri.app/reference/config/
- Tauri 2 plugin catalog (Store, Dialog, Single Instance): https://v2.tauri.app/plugin/
- Tauri 2 Updater signing requirements: https://v2.tauri.app/plugin/updater/
- `xcap` Rust screenshot crate: https://docs.rs/crate/xcap/latest
- Apple ScreenCaptureKit sample / permissions: https://developer.apple.com/documentation/screencapturekit/capturing-screen-content-in-macos
- Apple user-facing screen-capture permission management: https://support.apple.com/guide/mac-help/control-access-screen-system-audio-recording-mchld6aa7d23/mac

---

**PRD status:** Ready for design and engineering review. Final hardware support, shortcut, legal licensing, and distribution decisions remain open. This document defines a target product and is not a claim that the implementation or performance targets have been validated.
