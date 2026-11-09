//! Global region-capture shortcut (CAP-01).

use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::error::{AppError, AppResult};

/// Last registration error, shown in Settings until resolved.
#[derive(Default)]
pub struct ShortcutStatus(pub Mutex<Option<String>>);

pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            // Only key-down; auto-repeat and key-up can't open a second session
            // because `SessionManager::try_prepare` rejects concurrent captures.
            if event.state() == ShortcutState::Pressed {
                super::trigger_capture(app, None, Duration::ZERO);
            }
        })
        .build()
}

pub fn parse(accelerator: &str) -> AppResult<Shortcut> {
    accelerator.parse::<Shortcut>().map_err(|_| {
        AppError::Shortcut(format!(
            "\"{accelerator}\" isn't a valid shortcut. Use a combination like Ctrl+Shift+9."
        ))
    })
}

/// Replace the registered shortcut. On failure the previous one (if any) is
/// restored so the app is never left without a working hotkey.
pub fn apply<R: Runtime>(
    app: &AppHandle<R>,
    accelerator: &str,
    previous: Option<&str>,
) -> AppResult<()> {
    let shortcut = parse(accelerator)?;
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let status = app.state::<ShortcutStatus>();
    match gs.register(shortcut) {
        Ok(()) => {
            *status.0.lock().unwrap() = None;
            Ok(())
        }
        Err(_) => {
            let msg = format!(
                "{accelerator} is already used by another app or by the system. Choose a different shortcut."
            );
            if let Some(prev) = previous.and_then(|p| parse(p).ok()) {
                let _ = gs.register(prev);
            }
            *status.0.lock().unwrap() = Some(msg.clone());
            Err(AppError::Shortcut(msg))
        }
    }
}

pub fn last_error<R: Runtime>(app: &AppHandle<R>) -> Option<String> {
    app.state::<ShortcutStatus>().0.lock().unwrap().clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_and_rejects_garbage() {
        parse(crate::settings::DEFAULT_SHORTCUT).unwrap();
        parse("Alt+Shift+S").unwrap();
        assert!(parse("Ctrl+Banana").is_err());
        assert!(parse("").is_err());
    }
}
