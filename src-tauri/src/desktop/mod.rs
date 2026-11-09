pub mod autostart;
pub mod protocol;
pub mod shortcut;
pub mod tray;
pub mod windows;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Manager, Runtime};

use crate::capture::{self, permission};
use crate::error::{AppError, AppResult};
use crate::session::{CaptureMode, Session, SessionManager};
use crate::settings;
use windows::{MainView, show_main};

/// macOS shows its permission prompt at most once; we ask at most once per run
/// so a denial never turns into a prompt loop (PRD Flow C).
static PERMISSION_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Longest user-selectable capture delay.
pub const MAX_DELAY_SECS: u32 = 30;

/// Start a capture on a background thread. Safe to call repeatedly: extra
/// calls while a capture is active are ignored.
pub fn trigger_capture<R: Runtime>(app: &AppHandle<R>, mode: Option<CaptureMode>, delay: Duration) {
    trigger_delayed_capture(app, mode, delay, 0);
}

/// Like `trigger_capture`, but counts down `countdown_secs` first (P1 capture
/// delay) so the user can open a menu or hover state before pixels freeze.
/// The countdown is shown on the tray icon.
pub fn trigger_delayed_capture<R: Runtime>(
    app: &AppHandle<R>,
    mode: Option<CaptureMode>,
    delay: Duration,
    countdown_secs: u32,
) {
    let app = app.clone();
    let countdown = countdown_secs.min(MAX_DELAY_SECS);
    std::thread::spawn(move || match run_capture(&app, mode, delay, countdown) {
        Ok(()) | Err(AppError::Busy) => {}
        Err(AppError::PermissionDenied) => show_main(&app, MainView::Permission, None),
        Err(e) => show_main(&app, MainView::Settings, Some(e.to_string())),
    });
}

fn run_capture<R: Runtime>(
    app: &AppHandle<R>,
    mode: Option<CaptureMode>,
    mut delay: Duration,
    countdown: u32,
) -> AppResult<()> {
    let manager = app.state::<SessionManager>();
    let guard = manager.try_prepare().ok_or(AppError::Busy)?;

    // Keep our own settings window out of the screenshot.
    if let Some(main) = app.get_webview_window(windows::MAIN)
        && main.is_visible().unwrap_or(false)
    {
        let _ = main.hide();
        // One or two compositor frames are enough for the hidden window to
        // disappear. The previous 250 ms pause made capture feel unresponsive.
        delay = delay.max(Duration::from_millis(40));
    }
    if !delay.is_zero() {
        std::thread::sleep(delay);
    }
    if countdown > 0 {
        for left in (1..=countdown).rev() {
            tray::set_status(
                app,
                Some(&format!("Capturing in {left}…")),
                Some(&left.to_string()),
            );
            std::thread::sleep(Duration::from_secs(1));
        }
        tray::set_status(app, None, None);
    }

    if permission::status() == permission::PermissionStatus::Denied {
        let granted = !PERMISSION_REQUESTED.swap(true, Ordering::SeqCst)
            && permission::request() == permission::PermissionStatus::Granted;
        if !granted {
            return Err(AppError::PermissionDenied);
        }
    }

    // Pixels are frozen here, before any overlay exists (CAP-06).
    let monitors = capture::freeze_all_monitors()?;
    let geometries: Vec<_> = monitors.iter().map(|m| m.geometry).collect();
    let window_list = capture::list_windows(&geometries);
    let focus_meta: Vec<_> = monitors
        .iter()
        .map(|m| (m.geometry, m.is_primary, m.scale_factor))
        .collect();
    let focus_index = windows::cursor_monitor(app, &focus_meta);

    let mode = mode.unwrap_or_else(|| settings::current(app).default_mode);
    let session = Arc::new(Session::new(mode, monitors, window_list, focus_index));

    // Encode previews in the background so they're ready when the pages ask.
    for i in 0..session.frames.len() {
        let s = session.clone();
        std::thread::spawn(move || {
            let _ = s.frames[i].preview_png();
        });
    }

    manager.install(&guard, session.clone());
    drop(guard);
    if let Err(e) = windows::open_overlays(app, &session) {
        end_capture(app, Some(&session.id));
        return Err(e);
    }
    Ok(())
}

/// End the active session: close overlays and release frozen frames.
pub fn end_capture<R: Runtime>(app: &AppHandle<R>, session_id: Option<&str>) {
    if app.state::<SessionManager>().take(session_id).is_some() {
        windows::close_overlays(app);
    }
}
