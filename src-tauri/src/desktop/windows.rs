//! Window management: the settings window (`main`) and one borderless
//! overlay per monitor (`overlay-{index}`).

use std::sync::atomic::Ordering;
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

// Standard capture windows remain ready for the lifetime of the app.
// Only per-session pixels and editor state are released between captures.

pub fn is_overlay(label: &str) -> bool {
    label.starts_with(OVERLAY_PREFIX)
}

pub fn overlay_label(index: usize) -> String {
    format!("{OVERLAY_PREFIX}{index}")
}

fn editor_label(session_id: &str, index: usize) -> String {
    format!("{OVERLAY_PREFIX}editor-{session_id}-{index}")
}

/// Only the selected monitor gets an editor. Its unique label prevents late
/// destruction from an earlier capture from interfering with the next one.
pub fn open_editor<R: Runtime>(app: &AppHandle<R>, session: &std::sync::Arc<Session>, index: usize) -> AppResult<()> {
    session.frame(index)?;
    let app2 = app.clone();
    let id = session.id.clone();
    let session = session.clone();
    app.run_on_main_thread(move || {
        if app2.state::<SessionManager>().get(&session.id).is_err() {
            return;
        }
        let result = (|| -> AppResult<()> {
            let boot = OverlayBoot {
                session_id: &session.id,
                monitor_index: index,
                mode: session.mode,
            };
            let window = build_overlay(&app2, &editor_label(&session.id, index), Some(&boot))?;
            let g = session.frames[index].geometry;
            window.set_position(PhysicalPosition::new(g.x, g.y))
                .map_err(|e| AppError::Internal(e.to_string()))?;
            window.set_size(PhysicalSize::new(g.width, g.height))
                .map_err(|e| AppError::Internal(e.to_string()))?;
            Ok(())
        })();
        if let Err(e) = result {
            super::end_capture(&app2, Some(&session.id));
            show_main(&app2, MainView::Settings, Some(e.to_string()));
        }
    }).map_err(|e| AppError::Internal(e.to_string()))?;

    let app = app.clone();
    // A page that fails to decode or start must never leave capture busy.
    std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(10));
            if let Ok(session) = app.state::<SessionManager>().get(&id)
                && !session.editor_ready.load(Ordering::SeqCst)
            {
                super::end_capture(&app, Some(&id));
                show_main(&app, MainView::Settings, Some("The screenshot editor could not start. Please try capturing again.".into()));
            }
    });
    Ok(())
}

pub fn overlay_destroyed<R: Runtime>(app: &AppHandle<R>, label: &str) {
    if let Some(session) = app.state::<SessionManager>().current() {
        let owns_window = match session.editor_selection() {
            Some(editor) => label == editor_label(&session.id, editor.monitor),
            None => (0..session.frames.len()).any(|i| label == overlay_label(i)),
        };
        if owns_window {
            super::end_capture(app, Some(&session.id));
        }
    }
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

fn main_script(payload: &NavigatePayload) -> AppResult<String> {
    let json = serde_json::to_string(payload).map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(format!("window.__FASTSHOT_MAIN__ = {json};"))
}

fn build_main<R: Runtime>(
    app: &AppHandle<R>,
    payload: &NavigatePayload,
) -> AppResult<WebviewWindow<R>> {
    WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("index.html".into()))
        .title("FastShot")
        .initialization_script(main_script(payload)?)
        .inner_size(760.0, 640.0)
        .min_inner_size(560.0, 480.0)
        .center()
        .visible(false)
        .build()
        .map_err(|e| AppError::Internal(e.to_string()))
}

pub fn show_main<R: Runtime>(app: &AppHandle<R>, view: MainView, error: Option<String>) {
    let payload = NavigatePayload { view, error };
    let (window, created) = match app.get_webview_window(MAIN) {
        Some(window) => (window, false),
        None => match build_main(app, &payload) {
            Ok(window) => (window, true),
            Err(error) => {
                eprintln!("FastShot could not open Settings: {error}");
                return;
            }
        },
    };

    // macOS: become a regular app (Dock icon + app menu) while Settings is open.
    super::app_menu::set_foreground(app, true);
    if !created {
        // Keep the bootstrap value current too. If this arrives while the new
        // page is mounting and before its event listener exists, its initial
        // state still observes the latest route.
        if let Ok(script) = main_script(&payload) {
            let _ = window.eval(script);
        }
        let _ = window.emit("navigate", payload);
    }
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
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
    let editor = session.editor_selection();
    let label = if editor.is_some() { editor_label(&session.id, index) } else { overlay_label(index) };
    let Some(w) = app.get_webview_window(&label) else {
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
    if index == editor.map_or(session.focus_index, |e| e.monitor) {
        let _ = w.set_focus();
    }
}

/// Hide standard overlays while keeping the UI ready. The session-end event
/// unmounts its editor and releases screenshot data. Native on-demand editors
/// are destroyed instead because they are specific to a single session.
pub fn close_overlays<R: Runtime>(app: &AppHandle<R>, session_id: &str) {
    let script = capture_end_script(session_id);
    for (label, w) in app.webview_windows() {
        if label.starts_with(&format!("{OVERLAY_PREFIX}editor-{session_id}-")) {
            let _ = w.hide();
            let _ = w.destroy();
        } else if is_overlay(&label) && !label.starts_with("overlay-editor-") {
            let _ = w.hide();
            let _ = w.eval(&script);
        }
    }
}

fn capture_end_script(session_id: &str) -> String {
    // A JSON string is a valid, fully escaped JS string literal.
    let id = serde_json::Value::from(session_id).to_string();
    format!(
        "if(window.__FASTSHOT_OVERLAY__&&window.__FASTSHOT_OVERLAY__.sessionId==={id})window.__FASTSHOT_OVERLAY__=undefined;window.dispatchEvent(new CustomEvent('fastshot:capture-end',{{detail:{{sessionId:{id}}}}}));"
    )
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

#[cfg(test)]
mod tests {
    use super::{MainView, NavigatePayload, capture_end_script, main_script};

    #[test]
    fn main_script_preserves_the_initial_route_and_escapes_errors() {
        let script = main_script(&NavigatePayload {
            view: MainView::Permission,
            error: Some(r#"x"};alert(1);//"#.into()),
        })
        .unwrap();
        assert!(script.starts_with(r#"window.__FASTSHOT_MAIN__ = {"view":"permission""#));
        assert!(script.contains(r#""error":"x\"};alert(1);//""#));
        assert!(!script.contains(r#""x"};"#));
    }

    #[test]
    fn capture_end_script_targets_only_its_session() {
        let s = capture_end_script("abc-123");
        assert!(s.contains(r#"__FASTSHOT_OVERLAY__.sessionId==="abc-123""#));
        assert!(s.contains(r#"'fastshot:capture-end',{detail:{sessionId:"abc-123"}}"#));
    }

    #[test]
    fn capture_end_script_escapes_the_id() {
        // Ids are UUIDs, but the script must stay well-formed for any string.
        let s = capture_end_script(r#"x"};alert(1);//"#);
        assert!(s.contains(r#""x\"};alert(1);//""#));
        assert!(!s.contains(r#""x"};"#));
    }
}
