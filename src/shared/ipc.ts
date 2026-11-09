import { invoke } from "@tauri-apps/api/core";

import type { PixelRect, RenderRequest } from "@/features/editor/model";

export type CaptureMode = "region" | "window" | "display";
export type ImageFormat = "png" | "jpeg";
export type Theme = "system" | "light" | "dark";

export interface Settings {
  shortcut: string;
  defaultMode: CaptureMode;
  format: ImageFormat;
  jpegQuality: number;
  saveDirectory: string | null;
  theme: Theme;
  annotationColor: string;
  strokeWidth: number;
  fontSize: number;
  redactColor: string;
  quitOnClose: boolean;
  onboardingComplete: boolean;
  filenameTemplate: string;
  showMagnifier: boolean;
  autoSelectWindow: boolean;
  launchAtLogin: boolean;
  /** Experimental, Windows only: native capture overlay (selection + copy). */
  nativeOverlay: boolean;
}

export interface FilenamePreview {
  example: string | null;
  error: string | null;
}

export interface WindowCandidate {
  monitorIndex: number;
  rect: PixelRect;
  z: number;
  appName: string;
}

export interface SessionView {
  sessionId: string;
  monitorIndex: number;
  monitorCount: number;
  mode: CaptureMode;
  monitor: {
    name: string;
    isPrimary: boolean;
    scaleFactor: number;
    pixelWidth: number;
    pixelHeight: number;
  };
  windows: WindowCandidate[];
  previewUrl: string;
}

export type Destination = "clipboard" | "file" | "quicksave";

export interface ExportResult {
  completed: boolean;
  destination: Destination;
  path: string | null;
  width: number;
  height: number;
}

export interface ExtractResult {
  /** `false` when no text was recognised; the capture stays open. */
  found: boolean;
  lines: number;
}

export interface AppInfo {
  version: string;
  platform: string;
  permission: "granted" | "denied";
  canOpenPermissionSettings: boolean;
  shortcutError: string | null;
  /** `Documents/FastShot`, used when no save folder is chosen. */
  defaultSaveDirectory: string | null;
}

export type MainView = "settings" | "onboarding" | "permission";
export interface NavigatePayload {
  view: MainView;
  error: string | null;
}

/** Overlay bootstrap data injected by Rust before the page loads. */
export interface OverlayBoot {
  sessionId: string;
  monitorIndex: number;
  mode: CaptureMode;
}

declare global {
  interface Window {
    /** Initial route for a settings window that Rust created on demand. */
    __FASTSHOT_MAIN__?: NavigatePayload;
    /** Marks a pre-warmed capture WebView, even before it has a session. */
    __FASTSHOT_OVERLAY_WINDOW__?: boolean;
    __FASTSHOT_OVERLAY__?: OverlayBoot;
  }
}

/** Rust returns `{ code, message }`; anything else is unexpected. */
export function errorMessage(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) return String((e as { message: unknown }).message);
  if (typeof e === "string") return e;
  return "Something went wrong.";
}

export const api = {
  getSession: (sessionId: string, monitorIndex: number) =>
    invoke<SessionView>("get_session", { sessionId, monitorIndex }),
  overlayReady: (sessionId: string, monitorIndex: number) =>
    invoke<void>("overlay_ready", { sessionId, monitorIndex }),
  cancelCapture: (sessionId: string) => invoke<void>("cancel_capture", { sessionId }),
  exportCapture: (args: { sessionId: string; monitorIndex: number; destination: Destination } & RenderRequest) =>
    invoke<ExportResult>("export_capture", { args }),
  /** OCR the rendered selection (redactions applied) and copy the text. */
  extractText: (args: { sessionId: string; monitorIndex: number } & RenderRequest) =>
    invoke<ExtractResult>("extract_text", { args }),
  getSettings: () => invoke<Settings>("get_settings"),
  updateSettings: (settings: Settings) => invoke<Settings>("update_settings", { settings }),
  browseSaveDirectory: () => invoke<string | null>("browse_save_directory"),
  getAppInfo: () => invoke<AppInfo>("get_app_info"),
  beginCapture: (mode?: CaptureMode) => invoke<void>("begin_capture", { mode: mode ?? null }),
  previewFilename: (template: string) => invoke<FilenamePreview>("preview_filename", { template }),
  requestPermission: () => invoke<"granted" | "denied">("request_permission"),
  openPermissionSettings: () => invoke<void>("open_permission_settings"),
  quitApp: () => invoke<void>("quit_app"),
};
