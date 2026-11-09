//! Launch at login (P1). The OS login-item registry is the source of truth;
//! the setting only mirrors it so Settings can show a switch.

use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

use crate::error::{AppError, AppResult};

/// Passed when the OS launches us at login, so startup can stay quiet.
pub const AUTOSTART_ARG: &str = "--autostart";

pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![AUTOSTART_ARG]))
}

pub fn is_enabled<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.try_state::<tauri_plugin_autostart::AutoLaunchManager>()
        .is_some()
        && app.autolaunch().is_enabled().unwrap_or(false)
}

pub fn set_enabled<R: Runtime>(app: &AppHandle<R>, on: bool) -> AppResult<()> {
    let al = app.autolaunch();
    let result = if on { al.enable() } else { al.disable() };
    result.map_err(|_| {
        AppError::Internal(if on {
            "Couldn't add FastShot to your login items. Check your system settings.".into()
        } else {
            "Couldn't remove FastShot from your login items. Check your system settings.".into()
        })
    })
}

pub fn launched_at_login() -> bool {
    std::env::args().any(|a| a == AUTOSTART_ARG)
}
