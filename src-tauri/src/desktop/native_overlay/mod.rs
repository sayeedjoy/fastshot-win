//! Native capture overlay with an on-demand editor (Windows).
//!
//! The WebView overlay must be pre-warmed to open instantly, which keeps a
//! renderer process alive while FastShot idles in the tray. This overlay is a
//! plain OS window drawn with GDI instead: it is created when a capture starts
//! and destroyed when it ends, so nothing is retained between captures.
//!
//! Region drag, window/display picking, copy image, copy text (OCR), cancel.
//! Edit opens the full WebView editor using the same frozen session and crop.
//! Output goes through the same `render_export` compositor (SEC-04).

use std::sync::Arc;

use tauri::{AppHandle, Runtime};

use crate::commands;
use crate::desktop::{self, windows};
use crate::error::AppResult;
use crate::geometry::PixelRect;
use crate::imaging::model::RenderRequest;
use crate::session::Session;
use crate::settings;

#[cfg(windows)]
mod win32;

/// What the user asked for. `crop` is in the monitor's image pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Edit { monitor: usize, crop: PixelRect },
    Copy { monitor: usize, crop: PixelRect },
    CopyText { monitor: usize, crop: PixelRect },
    Cancel,
}

/// Called on the overlay's UI thread; must not block it.
pub type Handler = Box<dyn Fn(Action) + Send>;

pub const fn is_supported() -> bool {
    cfg!(windows)
}

/// The user opted in and this platform has a native overlay.
pub fn enabled<R: Runtime>(app: &AppHandle<R>) -> bool {
    is_supported() && settings::current(app).native_overlay
}

/// Create and show one overlay window per frozen monitor. Returns once the
/// windows exist, so the caller can fall back to the WebView overlay.
pub fn open<R: Runtime>(app: &AppHandle<R>, session: &Arc<Session>) -> AppResult<()> {
    #[cfg(windows)]
    {
        let auto_select = settings::current(app).auto_select_window;
        let app = app.clone();
        let handler_session = session.clone();
        let handler: Handler = Box::new(move |action| handle(&app, &handler_session, action));
        win32::open(session.clone(), auto_select, handler)
    }
    #[cfg(not(windows))]
    {
        let _ = (app, session);
        Err(crate::error::AppError::Internal(
            "The native overlay isn't available on this platform.".into(),
        ))
    }
}

/// Destroy the overlay windows for `session_id`, if they are ours.
pub fn close(session_id: &str) {
    #[cfg(windows)]
    win32::close(session_id);
    #[cfg(not(windows))]
    let _ = session_id;
}

fn crop_only(crop: PixelRect) -> RenderRequest {
    RenderRequest {
        crop,
        annotations: Vec::new(),
        effects: Vec::new(),
        redactions: Vec::new(),
    }
}

/// Work runs on its own thread so the overlay's message loop stays responsive.
fn handle<R: Runtime>(app: &AppHandle<R>, session: &Arc<Session>, action: Action) {
    let app = app.clone();
    let session = session.clone();
    std::thread::spawn(move || match action {
        Action::Edit { monitor, crop } => {
            let result = session.request_editor(monitor, crop)
                .and_then(|()| windows::open_editor(&app, &session, monitor));
            if let Err(e) = result {
                desktop::end_capture(&app, Some(&session.id));
                windows::show_main(&app, windows::MainView::Settings, Some(e.to_string()));
            }
        }
        Action::Cancel => desktop::end_capture(&app, Some(&session.id)),
        Action::Copy { monitor, crop } => {
            // Like the WebView copy: dismiss first so it feels instant. The
            // session `Arc` keeps the frozen frame alive for the render.
            desktop::end_capture(&app, Some(&session.id));
            if let Err(e) = commands::render_to_clipboard(&app, &session, monitor, &crop_only(crop))
            {
                windows::show_main(&app, windows::MainView::Settings, Some(e.to_string()));
            }
        }
        Action::CopyText { monitor, crop } => {
            let result = commands::recognize_selection(&session, monitor, &crop_only(crop))
                .and_then(|text| {
                    if text.is_empty() {
                        Ok(false)
                    } else {
                        commands::finish_text_copy(&app, &session.id, text).map(|()| true)
                    }
                });
            match result {
                Ok(true) => {}
                Ok(false) => notify(&session.id, "No text found in the selection."),
                Err(e) => notify(&session.id, &e.to_string()),
            }
        }
    });
}

/// Show a short message in the overlay (and end its busy state).
fn notify(session_id: &str, message: &str) {
    #[cfg(windows)]
    win32::notify(session_id, message);
    #[cfg(not(windows))]
    let _ = (session_id, message);
}
