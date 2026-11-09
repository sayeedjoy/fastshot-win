pub mod capture;
pub mod commands;
pub mod desktop;
pub mod error;
pub mod geometry;
pub mod imaging;
pub mod naming;
pub mod ocr;
pub mod session;
pub mod settings;

use tauri::{Manager, RunEvent, WindowEvent};

use desktop::windows::{self, MainView, show_main};
use desktop::{app_menu, autostart, protocol, shortcut, tray};
use session::SessionManager;
use settings::SettingsState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        // Must be first: a second launch focuses this instance instead (SYS-04).
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app, MainView::Settings, None);
        }))
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(shortcut::plugin())
        .plugin(autostart::plugin())
        .manage(SessionManager::default())
        .manage(SettingsState::default())
        .manage(shortcut::ShortcutStatus::default())
        .register_asynchronous_uri_scheme_protocol(protocol::SCHEME, |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            std::thread::spawn(move || responder.respond(protocol::handle(&app, &request)));
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_session,
            commands::overlay_ready,
            commands::cancel_capture,
            commands::export_capture,
            commands::extract_text,
            commands::get_settings,
            commands::update_settings,
            commands::preview_filename,
            commands::browse_save_directory,
            commands::get_app_info,
            commands::begin_capture,
            commands::request_permission,
            commands::open_permission_settings,
            commands::quit_app,
        ])
        .setup(|app| {
            // Menu-bar app on macOS: no Dock icon.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle().clone();
            let prefs = settings::load(&handle);
            // A conflict is recorded and shown in Settings; startup continues.
            let _ = shortcut::apply(&handle, &prefs.shortcut, None);
            tray::create(&handle)?;
            app_menu::install(&handle)?;

            // Launched by the OS at login: stay quiet in the tray unless the
            // shortcut is broken (onboarding waits for a manual launch).
            if autostart::launched_at_login() {
                if let Some(err) = shortcut::last_error(&handle) {
                    show_main(&handle, MainView::Settings, Some(err));
                }
            } else if !prefs.onboarding_complete {
                show_main(&handle, MainView::Onboarding, None);
            } else if let Some(err) = shortcut::last_error(&handle) {
                show_main(&handle, MainView::Settings, Some(err));
            }

            // Pay WebView startup cost while idle. Captures can then reuse and
            // reveal these hidden overlays instead of creating them on demand.
            if let Err(e) = windows::prewarm_overlays(&handle) {
                eprintln!("FastShot could not pre-warm capture overlays: {e}");
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { .. } if window.label() == windows::MAIN => {
                if settings::current(window.app_handle()).quit_on_close {
                    window.app_handle().exit(0);
                } else {
                    // Allow the settings WebView to be destroyed. The tray and
                    // shortcut keep running, and show_main recreates it.
                    app_menu::set_foreground(window.app_handle(), false);
                }
            }
            WindowEvent::Destroyed if windows::is_overlay(window.label()) => {
                // An overlay vanished without us closing it (e.g. a display was
                // disconnected): end the whole session so nothing is left stuck.
                desktop::end_capture(window.app_handle(), None);
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("error while building FastShot");

    app.run(|_app, event| {
        // Closing the last window must not quit a tray app; only `exit(code)` does.
        if let RunEvent::ExitRequested {
            code: None, api, ..
        } = event
        {
            api.prevent_exit();
        }
    });
}
