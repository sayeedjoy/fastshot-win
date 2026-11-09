//! Tauri command boundary. Every input is validated here or in the module it
//! is passed to, even though the UI validates too (PRD §8.4 security note).
//! Which window may call which command is scoped in `capabilities/`.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, WebviewWindow};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::capture::WindowCandidate;
use crate::capture::permission::{self, PermissionStatus};
use crate::desktop::{self, autostart, protocol, shortcut, windows};
use crate::error::{AppError, AppResult};
use crate::imaging::encode::{ImageFormat, encode};
use crate::imaging::model::RenderRequest;
use crate::imaging::render::render_export;
use crate::naming::{self, NameContext};
use crate::session::{CaptureMode, SessionManager};
use crate::settings::{self, Settings};

// ---------------------------------------------------------------- overlay ---

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorView {
    name: String,
    is_primary: bool,
    scale_factor: f32,
    pixel_width: u32,
    pixel_height: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    session_id: String,
    monitor_index: usize,
    monitor_count: usize,
    mode: CaptureMode,
    monitor: MonitorView,
    windows: Vec<WindowCandidate>,
    preview_url: String,
}

#[tauri::command]
pub fn get_session(
    app: AppHandle,
    session_id: String,
    monitor_index: usize,
) -> AppResult<SessionView> {
    let session = app.state::<SessionManager>().get(&session_id)?;
    let frame = session.frame(monitor_index)?;
    Ok(SessionView {
        session_id: session.id.clone(),
        monitor_index,
        monitor_count: session.frames.len(),
        mode: session.mode,
        monitor: MonitorView {
            name: frame.name.clone(),
            is_primary: frame.is_primary,
            scale_factor: frame.scale_factor,
            pixel_width: frame.image.width(),
            pixel_height: frame.image.height(),
        },
        windows: session
            .windows
            .iter()
            .filter(|w| w.monitor_index == monitor_index)
            .cloned()
            .collect(),
        preview_url: protocol::preview_url(&session.id, monitor_index),
    })
}

#[tauri::command]
pub fn overlay_ready(app: AppHandle, session_id: String, monitor_index: usize) -> AppResult<()> {
    let session = app.state::<SessionManager>().get(&session_id)?;
    session.frame(monitor_index)?;
    windows::reveal_overlay(&app, &session, monitor_index);
    Ok(())
}

#[tauri::command]
pub fn cancel_capture(app: AppHandle, session_id: String) {
    desktop::end_capture(&app, Some(&session_id));
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Destination {
    Clipboard,
    /// Native Save dialog.
    File,
    /// Straight into the default save folder with the naming template, no
    /// dialog (P1). The folder was chosen by the user via a native folder
    /// picker (or is `Documents/FastShot`); the UI never supplies a path.
    #[serde(rename = "quicksave")]
    QuickSave,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportArgs {
    session_id: String,
    monitor_index: usize,
    destination: Destination,
    #[serde(flatten)]
    request: RenderRequest,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    /// `false` when the user dismissed the Save dialog; nothing was written.
    completed: bool,
    destination: Destination,
    path: Option<String>,
    width: u32,
    height: u32,
}

/// Name of the folder FastShot creates inside the user's Documents folder.
const APP_FOLDER: &str = "FastShot";

/// `Documents/FastShot` (like Lightshot's `Documents/LightShot`). Not created.
fn app_folder(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .document_dir()
        .ok()
        .or_else(|| app.path().picture_dir().ok())
        .map(|d| d.join(APP_FOLDER))
}

/// The user's chosen folder if it still exists, otherwise `Documents/FastShot`
/// (created on demand), otherwise the OS Pictures folder.
fn default_save_dir(app: &AppHandle, settings: &Settings) -> Option<PathBuf> {
    settings
        .save_directory
        .as_ref()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .or_else(|| {
            app_folder(app).filter(|p| p.is_dir() || std::fs::create_dir_all(p).is_ok())
        })
        .or_else(|| app.path().picture_dir().ok().filter(|p| p.is_dir()))
}

/// Resolve the output format from what the user typed in the Save dialog,
/// appending the default extension when they left it off.
fn resolve_output(path: PathBuf, default: ImageFormat) -> (PathBuf, ImageFormat) {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .and_then(ImageFormat::from_extension)
    {
        Some(fmt) => (path, fmt),
        None => {
            let mut p = path.into_os_string();
            p.push(".");
            p.push(default.extension());
            (PathBuf::from(p), default)
        }
    }
}

async fn write_image(
    image: image::RgbaImage,
    path: PathBuf,
    format: ImageFormat,
    quality: u8,
    overwrite: bool,
) -> AppResult<()> {
    use std::io::Write;
    tauri::async_runtime::spawn_blocking(move || -> AppResult<()> {
        let bytes = encode(&image, format, quality).map_err(AppError::Encode)?;
        // Quick save uses `create_new` so it can never overwrite a file, even
        // one that appeared after the name was chosen. The Save dialog has
        // already confirmed any overwrite with the user.
        let io = |e: std::io::Error| AppError::Io(e.kind().to_string());
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .create_new(!overwrite)
            .open(&path)
            .map_err(io)?;
        file.write_all(&bytes).map_err(io)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

/// Copy is meant to feel instant: the overlay is hidden first, then the image
/// is composited and written to the clipboard off the async runtime. The
/// session `Arc` keeps the frozen frame alive after the session is ended.
/// Failures surface in the main window because the overlay is already gone.
async fn copy_to_clipboard(
    app: AppHandle,
    session: std::sync::Arc<crate::session::Session>,
    args: ExportArgs,
) -> AppResult<ExportResult> {
    desktop::end_capture(&app, Some(&args.session_id));

    let index = args.monitor_index;
    let request = args.request;
    let worker = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || -> AppResult<(u32, u32)> {
        let image = render_export(&session.frames[index].image, &request)
            .map_err(AppError::InvalidInput)?;
        let (width, height) = image.dimensions();
        let img = tauri::image::Image::new_owned(image.into_raw(), width, height);
        worker
            .clipboard()
            .write_image(&img)
            .map_err(|e| AppError::Clipboard(e.to_string()))?;
        Ok((width, height))
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))
    .and_then(|r| r);

    match result {
        Ok((width, height)) => Ok(ExportResult {
            completed: true,
            destination: Destination::Clipboard,
            path: None,
            width,
            height,
        }),
        Err(e) => {
            windows::show_main(&app, windows::MainView::Settings, Some(e.to_string()));
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn export_capture(
    app: AppHandle,
    window: WebviewWindow,
    args: ExportArgs,
) -> AppResult<ExportResult> {
    let session = app.state::<SessionManager>().get(&args.session_id)?;
    session.frame(args.monitor_index)?;

    if args.destination == Destination::Clipboard {
        return copy_to_clipboard(app, session, args).await;
    }
    let prefs = settings::current(&app);

    // Same compositor for clipboard and file (SEC-04).
    let request = args.request;
    let index = args.monitor_index;
    let render_session = session.clone();
    let image = tauri::async_runtime::spawn_blocking(move || {
        render_export(&render_session.frames[index].image, &request)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
    .map_err(AppError::InvalidInput)?;
    let (width, height) = image.dimensions();

    match args.destination {
        Destination::Clipboard => unreachable!("handled by copy_to_clipboard"),
        Destination::QuickSave => {
            let dir = default_save_dir(&app, &prefs).ok_or_else(|| {
                AppError::Io("There's no default save folder. Choose one in Settings.".into())
            })?;
            let format = prefs.format;
            let path = naming::unique_path(
                &prefs.filename_template,
                &NameContext::now(width, height),
                format.extension(),
                &dir,
            )
            .map_err(AppError::InvalidInput)?;
            write_image(image, path.clone(), format, prefs.jpeg_quality, false).await?;
            Ok(ExportResult {
                completed: true,
                destination: Destination::QuickSave,
                path: Some(path.to_string_lossy().into_owned()),
                width,
                height,
            })
        }
        Destination::File => {
            // The path comes only from the native dialog, never from the UI.
            let dir = default_save_dir(&app, &prefs);
            let name = naming::unique_name(
                &prefs.filename_template,
                &NameContext::now(width, height),
                prefs.format.extension(),
                dir.as_deref(),
            )
            .map_err(AppError::InvalidInput)?;
            let mut dialog = app
                .dialog()
                .file()
                .set_title("Save screenshot")
                .set_file_name(&name)
                .set_parent(&window);
            dialog = match prefs.format {
                ImageFormat::Png => dialog
                    .add_filter("PNG image", &["png"])
                    .add_filter("JPEG image", &["jpg", "jpeg"]),
                ImageFormat::Jpeg => dialog
                    .add_filter("JPEG image", &["jpg", "jpeg"])
                    .add_filter("PNG image", &["png"]),
            };
            if let Some(dir) = dir {
                dialog = dialog.set_directory(dir);
            }
            let chosen = tauri::async_runtime::spawn_blocking(move || dialog.blocking_save_file())
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;
            let _ = window.set_focus();
            let Some(chosen) = chosen else {
                return Ok(ExportResult {
                    completed: false,
                    destination: Destination::File,
                    path: None,
                    width,
                    height,
                });
            };
            let path = chosen
                .into_path()
                .map_err(|_| AppError::Io("That location isn't a local folder.".into()))?;
            let (path, format) = resolve_output(path, prefs.format);
            write_image(image, path.clone(), format, prefs.jpeg_quality, true).await?;
            Ok(ExportResult {
                completed: true,
                destination: Destination::File,
                path: Some(path.to_string_lossy().into_owned()),
                width,
                height,
            })
        }
    }
}

// -------------------------------------------------------------------- ocr ---

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractArgs {
    session_id: String,
    monitor_index: usize,
    #[serde(flatten)]
    request: RenderRequest,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractResult {
    /// `false` when no text was recognised; the capture stays open.
    found: bool,
    /// Number of lines copied.
    lines: usize,
}

/// Recognise the text in the selection and copy it to the clipboard.
///
/// OCR reads the same composited raster as Copy/Save, so anything under a
/// redaction is already gone before recognition (SEC-01). The overlay stays
/// up while the OS recogniser runs so "no text found" or an error can be
/// shown in place; on success the capture ends like an image copy.
#[tauri::command]
pub async fn extract_text(app: AppHandle, args: ExtractArgs) -> AppResult<ExtractResult> {
    let session = app.state::<SessionManager>().get(&args.session_id)?;
    session.frame(args.monitor_index)?;

    let index = args.monitor_index;
    let request = args.request;
    let text = tauri::async_runtime::spawn_blocking(move || -> AppResult<String> {
        let image = render_export(&session.frames[index].image, &request)
            .map_err(AppError::InvalidInput)?;
        crate::ocr::recognize(&image).map_err(AppError::Ocr)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    if text.is_empty() {
        return Ok(ExtractResult {
            found: false,
            lines: 0,
        });
    }
    let lines = text.lines().count();
    app.state::<SessionManager>()
        .complete_if_active(&args.session_id, || {
            app.clipboard()
                .write_text(text)
                .map_err(|e| AppError::Clipboard(e.to_string()))
        })?;
    desktop::end_capture(&app, Some(&args.session_id));
    Ok(ExtractResult { found: true, lines })
}

// --------------------------------------------------------------- settings ---

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Settings {
    settings::current(&app)
}

#[tauri::command]
pub fn update_settings(app: AppHandle, settings: Settings) -> AppResult<Settings> {
    settings.validate()?;
    let previous = settings::current(&app);
    let shortcut_changed =
        settings.shortcut != previous.shortcut || shortcut::last_error(&app).is_some();
    let autostart_changed = settings.launch_at_login != previous.launch_at_login;

    if shortcut_changed {
        shortcut::apply(&app, &settings.shortcut, Some(&previous.shortcut))?;
    }
    if autostart_changed && let Err(error) = autostart::set_enabled(&app, settings.launch_at_login)
    {
        let _ = autostart::set_enabled(&app, previous.launch_at_login);
        if shortcut_changed {
            let _ = shortcut::apply(&app, &previous.shortcut, Some(&settings.shortcut));
        }
        return Err(error);
    }
    if let Err(error) = settings::save(&app, &settings) {
        // `Store::set` precedes its disk write, so restore both the persisted
        // value and the plugin's in-memory value when that write fails.
        let _ = settings::save(&app, &previous);
        if autostart_changed {
            let _ = autostart::set_enabled(&app, previous.launch_at_login);
        }
        if shortcut_changed {
            let _ = shortcut::apply(&app, &previous.shortcut, Some(&settings.shortcut));
        }
        return Err(error);
    }
    Ok(settings)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilenamePreview {
    /// Rendered example, or `None` when the template is invalid.
    example: Option<String>,
    error: Option<String>,
}

/// Live preview for the naming template field in Settings. The same renderer
/// is used for real saves, so the preview can't drift from the output.
#[tauri::command]
pub fn preview_filename(app: AppHandle, template: String) -> FilenamePreview {
    let ext = settings::current(&app).format.extension();
    match naming::preview(&template, &NameContext::now(1280, 720), ext) {
        Ok(example) => FilenamePreview {
            example: Some(example),
            error: None,
        },
        Err(error) => FilenamePreview {
            example: None,
            error: Some(error),
        },
    }
}

#[tauri::command]
pub async fn browse_save_directory(app: AppHandle, window: WebviewWindow) -> Option<String> {
    let mut dialog = app
        .dialog()
        .file()
        .set_title("Default save folder")
        .set_parent(&window);
    if let Some(dir) = default_save_dir(&app, &settings::current(&app)) {
        dialog = dialog.set_directory(dir);
    }
    tauri::async_runtime::spawn_blocking(move || dialog.blocking_pick_folder())
        .await
        .ok()
        .flatten()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}

// ------------------------------------------------------------------- app ---

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    platform: &'static str,
    permission: PermissionStatus,
    can_open_permission_settings: bool,
    shortcut_error: Option<String>,
    /// Where saves go when no folder is chosen: `Documents/FastShot`.
    default_save_directory: Option<String>,
}

#[tauri::command]
pub fn get_app_info(app: AppHandle) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        platform: std::env::consts::OS,
        permission: permission::status(),
        can_open_permission_settings: permission::settings_url().is_some(),
        shortcut_error: shortcut::last_error(&app),
        default_save_directory: app_folder(&app).map(|p| p.to_string_lossy().into_owned()),
    }
}

#[tauri::command]
pub fn begin_capture(app: AppHandle, mode: Option<CaptureMode>) -> AppResult<()> {
    desktop::trigger_capture(&app, mode, Duration::ZERO);
    Ok(())
}

#[tauri::command]
pub fn request_permission() -> PermissionStatus {
    permission::request()
}

#[tauri::command]
pub fn open_permission_settings(app: AppHandle) -> AppResult<()> {
    let url = permission::settings_url()
        .ok_or_else(|| AppError::InvalidInput("Not needed on this platform.".into()))?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    desktop::end_capture(&app, None);
    app.exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_format_follows_extension() {
        assert_eq!(
            resolve_output(PathBuf::from("a.jpg"), ImageFormat::Png),
            (PathBuf::from("a.jpg"), ImageFormat::Jpeg)
        );
        assert_eq!(
            resolve_output(PathBuf::from("a"), ImageFormat::Png),
            (PathBuf::from("a.png"), ImageFormat::Png)
        );
        assert_eq!(
            resolve_output(PathBuf::from("a.gif"), ImageFormat::Jpeg),
            (PathBuf::from("a.gif.jpg"), ImageFormat::Jpeg)
        );
    }

    #[test]
    fn export_args_parse_flattened_request() {
        let json = r##"{"sessionId":"s","monitorIndex":0,"destination":"clipboard","crop":{"x":0,"y":0,"width":4,"height":4},"annotations":[],"effects":[],"redactions":[]}"##;
        let a: ExportArgs = serde_json::from_str(json).unwrap();
        assert_eq!(a.destination, Destination::Clipboard);
        assert_eq!(a.request.crop.width, 4);
        let q: ExportArgs = serde_json::from_str(&json.replace("clipboard", "quicksave")).unwrap();
        assert_eq!(q.destination, Destination::QuickSave);
    }
}
