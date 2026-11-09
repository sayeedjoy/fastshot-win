//! macOS application menu (Apple HIG: "The menu bar").
//!
//! FastShot is a menu-bar app, so it normally runs with the `Accessory`
//! activation policy: no Dock icon and no app menu. While the Settings window
//! is open it becomes a regular app, so it gets a Dock icon, ⌘-Tab presence
//! and the standard menus (FastShot, Edit, Window). Settings… is ⌘, and Quit
//! is ⌘Q, as Apple's guidelines require. Closing the window returns it to the
//! menu bar only.
//!
//! On other platforms these functions do nothing.

use tauri::{AppHandle, Runtime};

#[cfg(target_os = "macos")]
const SETTINGS_ID: &str = "app_settings";
#[cfg(target_os = "macos")]
const QUIT_ID: &str = "app_quit";

/// Build the app menu and route its custom items. Call once during setup.
#[cfg(target_os = "macos")]
pub fn install<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    use tauri::menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu};

    use super::windows::{MainView, show_main};

    let pkg = app.package_info();
    let about = PredefinedMenuItem::about(
        app,
        Some("About FastShot"),
        Some(AboutMetadata {
            name: Some("FastShot".into()),
            version: Some(pkg.version.to_string()),
            icon: app.default_window_icon().cloned(),
            ..Default::default()
        }),
    )?;
    let settings = MenuItem::with_id(app, SETTINGS_ID, "Settings…", true, Some("Cmd+,"))?;
    // Not the predefined Quit: that asks the OS to terminate, which the run
    // loop vetoes to keep the menu-bar app alive. This exits for real.
    let quit = MenuItem::with_id(app, QUIT_ID, "Quit FastShot", true, Some("Cmd+Q"))?;

    let app_menu = Submenu::with_items(
        app,
        "FastShot",
        true,
        &[
            &about,
            &PredefinedMenuItem::separator(app)?,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::services(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, Some("Hide FastShot"))?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;
    // Edit menu keeps ⌘C/⌘V/⌘A/⌘Z working in text fields such as the
    // shortcut and filename inputs.
    let edit_menu = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;
    let window_menu = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, Some("Zoom"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;

    let menu = Menu::with_items(app, &[&app_menu, &edit_menu, &window_menu])?;
    app.set_menu(menu)?;

    // Menu handlers are app-wide (the tray's handler sees these events too),
    // which is why the ids above are distinct from the tray's ids.
    app.on_menu_event(|app, event| match event.id().as_ref() {
        SETTINGS_ID => show_main(app, MainView::Settings, None),
        QUIT_ID => {
            super::end_capture(app, None);
            app.exit(0);
        }
        _ => {}
    });
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn install<R: Runtime>(_app: &AppHandle<R>) -> tauri::Result<()> {
    Ok(())
}

/// `true` while the Settings window is on screen: show the Dock icon and the
/// app menu. `false` once it is hidden: back to the menu bar only.
#[cfg(target_os = "macos")]
pub fn set_foreground<R: Runtime>(app: &AppHandle<R>, foreground: bool) {
    use tauri::ActivationPolicy;
    let policy = if foreground {
        ActivationPolicy::Regular
    } else {
        ActivationPolicy::Accessory
    };
    let _ = app.set_activation_policy(policy);
}

#[cfg(not(target_os = "macos"))]
pub fn set_foreground<R: Runtime>(_app: &AppHandle<R>, _foreground: bool) {}
