//! Window management: the settings window (`main`) and one borderless
//! overlay per monitor (`overlay-{index}`).

use std::time::Duration;

use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

use crate::error::{AppError, AppResult};
use crate::geometry::MonitorGeometry;
use crate::session::{CaptureMode, Session, SessionManager};

pub const MAIN: &str = "main";
pub const OVERLAY_PREFIX: &str = "overlay-";

pub fn is_overlay(label: &str) -> bool {
    label.starts_with(OVERLAY_PREFIX)
}

pub fn overlay_label(index: usize) -> String {
    format!("{OVERLAY_PREFIX}{index}")
}

/// Which screen of the settings window to open.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MainView {
    Settings,
    Onboarding,
    Permission,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigatePayload {
    pub view: MainView,
    pub error: Option<String>,
}

pub fn show_main<R: Runtime>(app: &AppHandle<R>, view: MainView, error: Option<String>) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.emit("navigate", NavigatePayload { view, error });
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Index of the monitor under the mouse pointer, else the primary, else 0.
pub fn cursor_monitor<R: Runtime>(
    app: &AppHandle<R>,
    monitors: &[(MonitorGeometry, bool, f32)],
) -> usize {
    if let Ok(c) = app.cursor_position() {
        for (i, (g, _, scale)) in monitors.iter().enumerate() {
            // Cursor is physical; macOS geometry is in points.
            let (cx, cy) = if cfg!(target_os = "macos") {
                (c.x / *scale as f64, c.y / *scale as f64)
            } else {
                (c.x, c.y)
            };
            if cx >= g.x as f64
                && cx < g.x as f64 + g.width as f64
                && cy >= g.y as f64
                && cy < g.y as f64 + g.height as f64
            {
                return i;
            }
        }
    }
    monitors
        .iter()
        .position(|(_, primary, _)| *primary)
        .unwrap_or(0)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OverlayBoot<'a> {
    session_id: &'a str,
    monitor_index: usize,
    mode: CaptureMode,
}

fn overlay_script(boot: Option<&OverlayBoot<'_>>) -> AppResult<String> {
    let mut script = "window.__FASTSHOT_OVERLAY_WINDOW__ = true;".to_string();
    if let Some(boot) = boot {
        let json = serde_json::to_string(boot).map_err(|e| AppError::Internal(e.to_string()))?;
        script.push_str(&format!("window.__FASTSHOT_OVERLAY__ = {json};"));
    }
    Ok(script)
}

fn build_overlay<R: Runtime>(
    app: &AppHandle<R>,
    label: &str,
    boot: Option<&OverlayBoot<'_>>,
) -> AppResult<WebviewWindow<R>> {
    WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
        .title("FastShot capture")
        .initialization_script(overlay_script(boot)?)
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .shadow(false)
        .visible(false)
        .visible_on_all_workspaces(true)
        .background_color(tauri::window::Color(0, 0, 0, 255))
        .build()
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Start hidden overlay WebViews while the app is idle. Creating a WebView is
/// much slower than moving and revealing an existing one, so captures reuse
/// these windows instead of rebuilding one for every monitor.
pub fn prewarm_overlays<R: Runtime>(app: &AppHandle<R>) -> AppResult<()> {
    let count = app
        .available_monitors()
        .map_err(|e| AppError::Internal(e.to_string()))?
        .len();
    for index in 0..count {
        let label = overlay_label(index);
        if app.get_webview_window(&label).is_none() {
            build_overlay(app, &label, None)?;
        }
    }
    Ok(())
}

/// Create one hidden overlay per frozen monitor, positioned exactly over it.
/// The overlay is opaque (it paints the frozen image), so it doesn't rely on
/// webview transparency behaving the same on both OSes (PRD §8.6).
pub fn open_overlays<R: Runtime>(app: &AppHandle<R>, session: &Session) -> AppResult<()> {
    for (index, frame) in session.frames.iter().enumerate() {
        let label = overlay_label(index);
        let boot = serde_json::to_string(&OverlayBoot {
            session_id: &session.id,
            monitor_index: index,
            mode: session.mode,
        })
        .map_err(|e| AppError::Internal(e.to_string()))?;
        let window = if let Some(window) = app.get_webview_window(&label) {
            let _ = window.hide();
            // Store the payload before dispatching so a listener that mounts
            // during this eval can still read the latest session from window.
            window
                .eval(format!(
                    "window.__FASTSHOT_OVERLAY__ = {boot};window.dispatchEvent(new CustomEvent('fastshot:capture-start', {{ detail: window.__FASTSHOT_OVERLAY__ }}));"
                ))
                .map_err(|e| AppError::Internal(e.to_string()))?;
            window
        } else {
            let boot_data = OverlayBoot {
                session_id: &session.id,
                monitor_index: index,
                mode: session.mode,
            };
            build_overlay(app, &label, Some(&boot_data))?
        };

        let g = frame.geometry;
        // OS space is physical on Windows and points on macOS (see geometry.rs).
        if cfg!(target_os = "macos") {
            let _ = window.set_position(tauri::LogicalPosition::new(g.x as f64, g.y as f64));
            let _ = window.set_size(tauri::LogicalSize::new(g.width as f64, g.height as f64));
        } else {
            let _ = window.set_position(PhysicalPosition::new(g.x, g.y));
            // Size after position: crossing a DPI boundary may rescale the window.
            let _ = window.set_size(PhysicalSize::new(g.width, g.height));
        }
        raise_above_menu_bar(&window);
    }

    // Fallback: if a page never reports ready (e.g. decode failure), show it
    // anyway so the user can always press Esc; an invisible capture is worse.
    let app2 = app.clone();
    let id = session.id.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(2500));
        if let Some(s) = app2
            .state::<SessionManager>()
            .current()
            .filter(|s| s.id == id)
        {
            for i in 0..s.frames.len() {
                reveal_overlay(&app2, &s, i);
            }
        }
    });
    Ok(())
}

/// Show an overlay; focus it if it's the one under the cursor.
pub fn reveal_overlay<R: Runtime>(app: &AppHandle<R>, session: &Session, index: usize) {
    let Some(w) = app.get_webview_window(&overlay_label(index)) else {
        return;
    };
    if w.is_visible().unwrap_or(false) {
        return;
    }
    let g = session.frames[index].geometry;
    if !cfg!(target_os = "macos") {
        // Re-assert geometry: Windows may have adjusted it on a DPI change.
        let _ = w.set_position(PhysicalPosition::new(g.x, g.y));
        let _ = w.set_size(PhysicalSize::new(g.width, g.height));
    }
    let _ = w.show();
    if index == session.focus_index {
        let _ = w.set_focus();
    }
}

/// Hide overlays but keep their WebViews warm for the next capture.
pub fn close_overlays<R: Runtime>(app: &AppHandle<R>) {
    for (label, w) in app.webview_windows() {
        if is_overlay(&label) {
            let _ = w.hide();
        }
    }
}

#[cfg(target_os = "macos")]
fn raise_above_menu_bar<R: Runtime>(window: &tauri::WebviewWindow<R>) {
    // NSScreenSaverWindowLevel (1000): above the menu bar and the Dock.
    let Ok(ptr) = window.ns_window() else { return };
    let ptr = ptr as usize;
    let _ = window.run_on_main_thread(move || {
        // SAFETY: `ns_window` returns a live NSWindow*; we're on the main thread.
        let ns_window: &objc2_app_kit::NSWindow =
            unsafe { &*(ptr as *const objc2_app_kit::NSWindow) };
        ns_window.setLevel(1000);
    });
}

#[cfg(not(target_os = "macos"))]
fn raise_above_menu_bar<R: Runtime>(_window: &tauri::WebviewWindow<R>) {}
