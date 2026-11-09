//! Tray / menu-bar presence (SYS-01).

use std::time::Duration;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
#[cfg(not(target_os = "macos"))]
use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
use tauri::{AppHandle, Runtime};

use super::windows::{MainView, show_main};
use super::{end_capture, trigger_capture};
use crate::session::CaptureMode;

/// Give the native context menu a few compositor frames to disappear before
/// freezing pixels without adding a perceptible pause.
const MENU_CLOSE_DELAY: Duration = Duration::from_millis(50);
const TRAY_ID: &str = "fastshot";
/// Windows tray: the full-colour app logo.
#[cfg(not(target_os = "macos"))]
const TRAY_ICON: &[u8] = include_bytes!("../../icons/64x64.png");
/// macOS menu bar: a monochrome template glyph (22pt @2x) that the system
/// tints for light/dark menu bars and the highlighted state (Apple HIG).
#[cfg(target_os = "macos")]
const TRAY_ICON: &[u8] = include_bytes!("../../icons/tray-template.png");
const TOOLTIP: &str = "FastShot";

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
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit FastShot", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &region,
            &window,
            &display,
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
            "settings" => show_main(app, MainView::Settings, None),
            "quit" => {
                end_capture(app, None);
                // Global shortcuts are unregistered when the plugin is dropped on exit.
                app.exit(0);
            }
            _ => {}
        });

    // Embedded directly (not via `default_window_icon`) so a logo change is
    // picked up by cargo's change tracking on the next build.
    match tauri::image::Image::from_bytes(TRAY_ICON) {
        Ok(icon) => {
            builder = builder.icon(icon);
            #[cfg(target_os = "macos")]
            {
                builder = builder.icon_as_template(true);
            }
        }
        Err(_) => {
            if let Some(icon) = app.default_window_icon() {
                builder = builder.icon(icon.clone());
            }
        }
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
