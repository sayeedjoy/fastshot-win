//! Tray / menu-bar presence (SYS-01).

use std::time::Duration;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
#[cfg(not(target_os = "macos"))]
use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
use tauri::{AppHandle, Runtime};

use super::windows::{MainView, show_main};
use super::{end_capture, trigger_capture, trigger_delayed_capture};
use crate::session::CaptureMode;

/// Give the native context menu a few compositor frames to disappear before
/// freezing pixels without adding a perceptible pause.
const MENU_CLOSE_DELAY: Duration = Duration::from_millis(50);
const TRAY_ID: &str = "fastshot";
const TOOLTIP: &str = "FastShot";
const DELAYS: [u32; 3] = [3, 5, 10];

/// Show a transient status (e.g. a capture countdown) on the tray icon;
/// `None` restores the default. `title` is shown beside the macOS menu-bar icon.
pub fn set_status<R: Runtime>(app: &AppHandle<R>, tooltip: Option<&str>, title: Option<&str>) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    let _ = tray.set_tooltip(Some(tooltip.unwrap_or(TOOLTIP)));
    #[cfg(target_os = "macos")]
    let _ = tray.set_title(title);
    #[cfg(not(target_os = "macos"))]
    let _ = title;
}

pub fn create<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let region = MenuItem::with_id(app, "capture_region", "Capture Region", true, None::<&str>)?;
    let window = MenuItem::with_id(app, "capture_window", "Capture Window", true, None::<&str>)?;
    let display = MenuItem::with_id(
        app,
        "capture_display",
        "Capture Display",
        true,
        None::<&str>,
    )?;
    let delay_items = DELAYS
        .iter()
        .map(|s| {
            MenuItem::with_id(
                app,
                format!("delay_{s}"),
                format!("Region in {s} seconds"),
                true,
                None::<&str>,
            )
        })
        .collect::<tauri::Result<Vec<_>>>()?;
    let delay_refs: Vec<&dyn tauri::menu::IsMenuItem<R>> = delay_items
        .iter()
        .map(|i| i as &dyn tauri::menu::IsMenuItem<R>)
        .collect();
    let delayed = Submenu::with_items(app, "Delayed Capture", true, &delay_refs)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit FastShot", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &region,
            &window,
            &display,
            &delayed,
            &PredefinedMenuItem::separator(app)?,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip(TOOLTIP)
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "capture_region" => trigger_capture(app, Some(CaptureMode::Region), MENU_CLOSE_DELAY),
            "capture_window" => trigger_capture(app, Some(CaptureMode::Window), MENU_CLOSE_DELAY),
            "capture_display" => trigger_capture(app, Some(CaptureMode::Display), MENU_CLOSE_DELAY),
            id if id.starts_with("delay_") => {
                if let Ok(secs) = id["delay_".len()..].parse::<u32>() {
                    trigger_delayed_capture(app, Some(CaptureMode::Region), MENU_CLOSE_DELAY, secs);
                }
            }
            "settings" => show_main(app, MainView::Settings, None),
            "quit" => {
                end_capture(app, None);
                // Global shortcuts are unregistered when the plugin is dropped on exit.
                app.exit(0);
            }
            _ => {}
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    // Windows convention: left click acts, right click opens the menu.
    // macOS convention: any click opens the menu-bar menu.
    #[cfg(not(target_os = "macos"))]
    {
        builder = builder
            .show_menu_on_left_click(false)
            .on_tray_icon_event(|tray, event| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
                {
                    trigger_capture(tray.app_handle(), Some(CaptureMode::Region), Duration::ZERO);
                }
            });
    }
    builder.build(app)?;
    Ok(())
}
