<div align="center">
  <img src="public/favicon.png" alt="FastShot logo" width="96" height="96" />

  # FastShot

  **Capture fast. Redact permanently. Share confidently.**

  A privacy-first screenshot tool for Windows and macOS.<br />
  Offline by design. No account required.

  [![Version](https://img.shields.io/badge/version-0.1.0-18181b?style=flat-square)](src-tauri/tauri.conf.json)
  [![Status](https://img.shields.io/badge/status-early%20MVP-f59e0b?style=flat-square)](#development-status)
  [![Tauri](https://img.shields.io/badge/Tauri-2-24c8db?style=flat-square&logo=tauri&logoColor=white)](https://v2.tauri.app/)
  [![Rust](https://img.shields.io/badge/Rust-2024-b7410e?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
  [![License](https://img.shields.io/badge/license-MIT-22c55e?style=flat-square)](#license)
</div>

---

FastShot is a native desktop screenshot workflow built for speed and privacy.
Capture a region, window, or display, add clear annotations, permanently remove
sensitive information, then copy or save a clean image—all without uploading
your screen to a server.

## Why FastShot?

Most screenshot tools make blur and pixelation look private. FastShot treats
privacy as an export guarantee: secure redactions are rendered by the native
Rust pipeline after every other edit, replacing the original pixels in the
final image.

| | |
| --- | --- |
| **Private by default** | Captures and edits stay on your device. No cloud service or account. |
| **Permanent redaction** | Opaque redactions overwrite pixels in a newly rendered image. |
| **Built for real desktops** | Multi-monitor capture, mixed-DPI coordinates, tray access, and a global shortcut. |
| **Consistent export** | Clipboard and file output use the same validated native compositor. |

## Product highlights

### Capture your way

- Select a custom region, a visible app window, or an entire display
- Freeze connected displays into one in-memory capture session
- Work across multiple monitors and mixed DPI scaling
- Start a capture from the tray or the global `Ctrl/Cmd+Shift+9` shortcut

### Explain visually

- Draw rectangles, ellipses, lines, arrows, and freehand strokes
- Add text using the bundled annotation font
- Apply blur or pixelation for visual emphasis
- Undo and redo editor changes through the local Zustand state model

### Export with confidence

- Permanently cover sensitive areas with opaque redaction
- Copy the finished result directly to the clipboard
- Save a fresh PNG or JPEG through the native file dialog
- Avoid source-image metadata and hidden editable layers

> [!NOTE]
> Blur and pixelation are visual effects, not secure redaction. Use an opaque
> redaction when the underlying content must not be recoverable.

## How permanent redaction works

```text
Captured frame
      │
      ▼
Validated crop → Visual effects → Annotations → Opaque redactions
                                                       │
                                                       ▼
                                            Fresh PNG / JPEG / clipboard
```

The frontend sends a declarative render request. Rust validates its crop and
operations, creates a new raster, applies effects and annotations, and then
writes redacted regions last with fully opaque pixels. The same compositor is
used for clipboard and file exports.

Automated security fixtures verify layer ordering, bounds handling,
clipboard/file pixel parity, metadata-free encoding, and the absence of source
payload data in redacted output.

## Quick start

### Prerequisites

- [Node.js](https://nodejs.org/) 22 or newer
- [pnpm](https://pnpm.io/) 12
- The stable [Rust toolchain](https://rustup.rs/), including Cargo
- Platform dependencies:
  - **Windows:** Microsoft C++ Build Tools and the WebView2 Runtime
  - **macOS:** Xcode Command Line Tools and macOS 12.3 or newer

See the official [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)
for platform-specific installation details.

### Run locally

```sh
pnpm install
pnpm tauri dev
```

This starts the Vite development server and opens FastShot as a native desktop
application.

### Build a desktop bundle

```sh
pnpm tauri build
```

Tauri writes the platform-specific artifacts beneath
`src-tauri/target/release/bundle/`.

## Development status

FastShot is currently an **early MVP**. Its native capture, session, geometry,
rendering, secure export, settings, tray, shortcut, and permission foundations
are implemented. The React capture, editor, onboarding, and settings experiences
are still being integrated.

The app is not yet recommended for production use. Follow the
[implementation plan](tasks/plan.md) and [MVP task list](tasks/todo.md) for
current scope. Detailed product and security requirements are documented in the
[PRD](docs/prd.md).

## Architecture

FastShot keeps privileged and security-sensitive work in Rust while the React
frontend owns interaction and presentation.

| Layer | Responsibility |
| --- | --- |
| **React + TypeScript** | Capture overlay, editor interaction, settings, typed IPC requests |
| **Tauri** | Window lifecycle, command permissions, tray, shortcuts, dialogs, clipboard |
| **Rust domain layer** | Capture sessions, coordinate validation, compositing, redaction, encoding |
| **Operating system** | Display enumeration, screen permission, native windows, file destinations |

```text
fastshot/
├── src/
│   ├── components/ui/       Shared React UI primitives
│   ├── features/editor/     Editor model, state, undo/redo, and tests
│   ├── shared/ipc.ts        Typed frontend-to-Rust API
│   └── main.tsx             Window-aware React entry point
├── src-tauri/
│   ├── capabilities/        Per-window Tauri permissions
│   └── src/
│       ├── capture/         Display capture and OS permissions
│       ├── desktop/         Windows, tray, shortcuts, preview protocol
│       ├── imaging/         Effects, annotations, redaction, encoding
│       ├── commands.rs      Validated Tauri command boundary
│       ├── geometry.rs      DPI conversion and bounds validation
│       ├── session.rs       In-memory capture lifecycle
│       └── settings.rs      Local preference persistence
├── docs/prd.md              Product requirements
└── tasks/                   Implementation plan and task tracking
```

## Development

| Command | Description |
| --- | --- |
| `pnpm tauri dev` | Run the complete desktop application |
| `pnpm dev` | Run only the Vite frontend |
| `pnpm build` | Type-check and build the frontend |
| `pnpm typecheck` | Run TypeScript project checks |
| `pnpm test` | Run frontend tests once |
| `pnpm test:watch` | Run frontend tests in watch mode |
| `pnpm test:rust` | Run the Rust test suite |
| `pnpm tauri build` | Build production desktop bundles |

Before submitting a change, run:

```sh
pnpm test
pnpm test:rust
pnpm build
```

Native capture behavior should also be tested on physical Windows and macOS
devices, particularly with multiple displays, mixed DPI scaling, clipboard
targets, and macOS screen-recording permissions.

## Platform support

| Platform | Status | Notes |
| --- | --- | --- |
| Windows 11 x64 | Primary target | No screen-capture permission prompt required |
| Windows 10 x64 | Best effort | Compatibility remains under evaluation |
| macOS 12.3+ | Targeted | Screen Recording permission is required |

Protected, DRM-controlled, minimized, or secure system windows may not be
capturable because of operating-system restrictions.

## Troubleshooting

### `cargo metadata`: program not found

If Rust is installed but Tauri cannot find Cargo on Windows, restart the IDE or
terminal so it inherits the updated `PATH`. To repair the current PowerShell
session immediately:

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
cargo --version
pnpm tauri dev
```

You can verify the full toolchain with:

```sh
node --version
pnpm --version
rustc --version
cargo --version
```

## License

FastShot is available under the MIT License.
