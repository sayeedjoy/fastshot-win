//! Capture adapter around `xcap`. Kept behind this module so a native backend
//! (ScreenCaptureKit, Windows.Graphics.Capture) can replace it (PRD §14).

pub mod permission;

use image::RgbaImage;
use serde::Serialize;
use xcap::{Monitor, Window};

use crate::error::{AppError, AppResult};
use crate::geometry::{MonitorGeometry, PixelRect};

pub struct FrozenMonitor {
    pub name: String,
    pub is_primary: bool,
    pub scale_factor: f32,
    pub geometry: MonitorGeometry,
    pub image: RgbaImage,
}

/// A visible top-level window, already mapped into one monitor's pixel space.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowCandidate {
    pub monitor_index: usize,
    pub rect: PixelRect,
    pub z: i32,
    pub app_name: String,
}

fn capture_one(m: &Monitor) -> AppResult<FrozenMonitor> {
    let err = |e: xcap::XCapError| AppError::Capture(e.to_string());
    let mut image = m.capture_image().map_err(err)?;
    // Some backends leave alpha at 0; the preview and export expect opaque.
    for p in image.pixels_mut() {
        p.0[3] = 255;
    }
    let geometry = MonitorGeometry {
        x: m.x().map_err(err)?,
        y: m.y().map_err(err)?,
        width: m.width().map_err(err)?,
        height: m.height().map_err(err)?,
        pixel_width: image.width(),
        pixel_height: image.height(),
    };
    Ok(FrozenMonitor {
        name: m
            .friendly_name()
            .or_else(|_| m.name())
            .unwrap_or_else(|_| "Display".into()),
        is_primary: m.is_primary().unwrap_or(false),
        scale_factor: m.scale_factor().unwrap_or(1.0),
        geometry,
        image,
    })
}

/// Snapshot every monitor in parallel. Monitors that fail are skipped; the
/// call fails only if none could be captured.
pub fn freeze_all_monitors() -> AppResult<Vec<FrozenMonitor>> {
    let count = Monitor::all()
        .map_err(|e| AppError::Capture(e.to_string()))?
        .len();
    if count == 0 {
        return Err(AppError::Capture("No displays were found.".into()));
    }
    // `xcap::Monitor` isn't `Send`, so each worker enumerates and picks its own.
    let results: Vec<AppResult<FrozenMonitor>> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..count)
            .map(|i| {
                s.spawn(move || {
                    let monitors = Monitor::all().map_err(|e| AppError::Capture(e.to_string()))?;
                    let m = monitors
                        .get(i)
                        .ok_or_else(|| AppError::Capture("A display was disconnected.".into()))?;
                    capture_one(m)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err(AppError::Capture("capture thread panicked".into())))
            })
            .collect()
    });
    let mut frozen = Vec::new();
    let mut last_err = None;
    for r in results {
        match r {
            Ok(f) => frozen.push(f),
            Err(e) => last_err = Some(e),
        }
    }
    if frozen.is_empty() {
        return Err(
            last_err.unwrap_or_else(|| AppError::Capture("No displays were captured.".into()))
        );
    }
    Ok(frozen)
}

/// OS chrome that shows up as "windows" but isn't something users mean to pick.
const IGNORED_APPS: &[&str] = &[
    "Window Server",
    "Dock",
    "Control Centre",
    "Control Center",
    "Notification Center",
    "Spotlight",
];

/// Visible top-level windows, excluding our own process, mapped to each
/// monitor they overlap. Sorted topmost first.
pub fn list_windows(geometries: &[MonitorGeometry]) -> Vec<WindowCandidate> {
    let own_pid = std::process::id();
    let Ok(windows) = Window::all() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for w in windows {
        if w.pid().map(|p| p == own_pid).unwrap_or(true) || w.is_minimized().unwrap_or(true) {
            continue;
        }
        let app_name = w.app_name().unwrap_or_default();
        if IGNORED_APPS.contains(&app_name.as_str()) {
            continue;
        }
        let (Ok(x), Ok(y), Ok(width), Ok(height)) = (w.x(), w.y(), w.width(), w.height()) else {
            continue;
        };
        if width < 24 || height < 24 {
            continue;
        }
        let z = w.z().unwrap_or(0);
        let os_rect = PixelRect::new(x, y, width, height);
        for (monitor_index, g) in geometries.iter().enumerate() {
            if let Some(rect) = g.to_local_pixels(os_rect) {
                out.push(WindowCandidate {
                    monitor_index,
                    rect,
                    z,
                    app_name: app_name.clone(),
                });
            }
        }
    }
    out.sort_by_key(|w| std::cmp::Reverse(w.z));
    out
}
