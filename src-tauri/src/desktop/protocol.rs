//! `fastshot://` serves the active session's frozen frames to the overlays
//! from memory (PRD §8.3 step 2). No file paths are involved, and URLs stop
//! working the moment the session ends.

use tauri::http::{Request, Response, StatusCode, header};
use tauri::{AppHandle, Manager, Runtime};

use crate::session::SessionManager;

pub const SCHEME: &str = "fastshot";

/// URL the webview should use for `/{session}/{index}` on this platform.
pub fn preview_url(session_id: &str, index: usize) -> String {
    if cfg!(any(target_os = "windows", target_os = "android")) {
        format!("http://{SCHEME}.localhost/{session_id}/{index}")
    } else {
        format!("{SCHEME}://localhost/{session_id}/{index}")
    }
}

fn not_found() -> Response<Vec<u8>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Vec::new())
        .unwrap()
}

pub fn handle<R: Runtime>(app: &AppHandle<R>, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let mut parts = request.uri().path().trim_start_matches('/').split('/');
    let (Some(id), Some(index), None) = (parts.next(), parts.next(), parts.next()) else {
        return not_found();
    };
    let Ok(index) = index.parse::<usize>() else {
        return not_found();
    };
    let Ok(session) = app.state::<SessionManager>().get(id) else {
        return not_found();
    };
    let Ok(frame) = session.frame(index) else {
        return not_found();
    };
    match frame.preview_png() {
        Ok(png) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/png")
            .header(header::CACHE_CONTROL, "no-store")
            // Lets the overlay read pixels into a canvas for the pixelate
            // preview. Only this app's webviews can reach a custom protocol.
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(png.as_ref().clone())
            .unwrap(),
        Err(_) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Vec::new())
            .unwrap(),
    }
}
