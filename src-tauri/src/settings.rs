//! Persisted, non-sensitive preferences (SYS-02). Stored with the Store plugin
//! from Rust only; the UI never gets generic store access.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_store::StoreExt;

use crate::error::{AppError, AppResult};
use crate::imaging::encode::ImageFormat;
use crate::imaging::model::Color;
use crate::naming;
use crate::session::CaptureMode;

const STORE_FILE: &str = "settings.json";
const KEY: &str = "settings";
pub const DEFAULT_SHORTCUT: &str = "CommandOrControl+Shift+9";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub shortcut: String,
    pub default_mode: CaptureMode,
    pub format: ImageFormat,
    pub jpeg_quality: u8,
    pub save_directory: Option<String>,
    pub theme: Theme,
    pub annotation_color: String,
    pub stroke_width: f32,
    pub font_size: f32,
    pub redact_color: String,
    pub quit_on_close: bool,
    pub onboarding_complete: bool,
    /// File name template for Save and Quick save (see `naming`).
    pub filename_template: String,
    /// Pixel loupe while selecting / resizing a region.
    pub show_magnifier: bool,
    /// In Region mode, a click without dragging selects the window under the
    /// pointer. Off: only a drag makes a selection.
    pub auto_select_window: bool,
    /// Mirrors the OS login-item state; the OS is the source of truth.
    pub launch_at_login: bool,
    /// Experimental (Windows): capture with a native overlay instead of a
    /// pre-warmed WebView. Lighter while idle; selection and copy only.
    pub native_overlay: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            shortcut: DEFAULT_SHORTCUT.into(),
            default_mode: CaptureMode::Region,
            format: ImageFormat::Png,
            jpeg_quality: 90,
            save_directory: None,
            theme: Theme::System,
            annotation_color: "#e11d48".into(),
            stroke_width: 4.0,
            font_size: 24.0,
            redact_color: "#000000".into(),
            quit_on_close: false,
            onboarding_complete: false,
            filename_template: naming::DEFAULT_TEMPLATE.into(),
            show_magnifier: true,
            auto_select_window: true,
            launch_at_login: false,
            native_overlay: false,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> AppResult<()> {
        let bad = |m: &str| Err(AppError::InvalidInput(m.into()));
        if !(1..=100).contains(&self.jpeg_quality) {
            return bad("JPEG quality must be between 1 and 100.");
        }
        if !(1.0..=50.0).contains(&self.stroke_width) {
            return bad("Stroke width must be between 1 and 50.");
        }
        if !(8.0..=200.0).contains(&self.font_size) {
            return bad("Font size must be between 8 and 200.");
        }
        Color::parse(&self.annotation_color).map_err(AppError::InvalidInput)?;
        Color::parse(&self.redact_color).map_err(AppError::InvalidInput)?;
        naming::validate(&self.filename_template).map_err(AppError::InvalidInput)?;
        if let Some(dir) = &self.save_directory
            && !std::path::Path::new(dir).is_dir()
        {
            return bad(
                "The default save folder doesn't exist or can't be accessed. Choose another folder.",
            );
        }
        Ok(())
    }

    /// Fields that fail validation fall back to defaults, so a corrupt or
    /// stale file never blocks startup.
    fn sanitized(mut self) -> Settings {
        let d = Settings::default();
        if !(1..=100).contains(&self.jpeg_quality) {
            self.jpeg_quality = d.jpeg_quality;
        }
        if !(1.0..=50.0).contains(&self.stroke_width) {
            self.stroke_width = d.stroke_width;
        }
        if !(8.0..=200.0).contains(&self.font_size) {
            self.font_size = d.font_size;
        }
        if Color::parse(&self.annotation_color).is_err() {
            self.annotation_color = d.annotation_color;
        }
        if Color::parse(&self.redact_color).is_err() {
            self.redact_color = d.redact_color;
        }
        if self
            .save_directory
            .as_deref()
            .is_some_and(|p| !std::path::Path::new(p).is_dir())
        {
            self.save_directory = None;
        }
        if self.shortcut.trim().is_empty() {
            self.shortcut = d.shortcut;
        }
        if naming::validate(&self.filename_template).is_err() {
            self.filename_template = d.filename_template;
        }
        self
    }
}

#[derive(Default)]
pub struct SettingsState(pub Mutex<Settings>);

pub fn load<R: Runtime>(app: &AppHandle<R>) -> Settings {
    let loaded = app
        .store(STORE_FILE)
        .ok()
        .and_then(|s| s.get(KEY))
        .and_then(|v| serde_json::from_value::<Settings>(v).ok())
        .unwrap_or_default()
        .sanitized();
    let loaded = Settings {
        launch_at_login: crate::desktop::autostart::is_enabled(app),
        ..loaded
    };
    *app.state::<SettingsState>().0.lock().unwrap() = loaded.clone();
    loaded
}

pub fn current<R: Runtime>(app: &AppHandle<R>) -> Settings {
    app.state::<SettingsState>().0.lock().unwrap().clone()
}

pub fn save<R: Runtime>(app: &AppHandle<R>, settings: &Settings) -> AppResult<()> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| AppError::Io(e.to_string()))?;
    store.set(
        KEY,
        serde_json::to_value(settings).map_err(|e| AppError::Internal(e.to_string()))?,
    );
    store.save().map_err(|e| AppError::Io(e.to_string()))?;
    *app.state::<SettingsState>().0.lock().unwrap() = settings.clone();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        Settings::default().validate().unwrap();
    }

    #[test]
    fn partial_json_fills_defaults() {
        let s: Settings = serde_json::from_str(r#"{"format":"jpeg"}"#).unwrap();
        assert_eq!(s.format, ImageFormat::Jpeg);
        assert_eq!(s.shortcut, DEFAULT_SHORTCUT);
    }

    #[test]
    fn sanitize_repairs_bad_values() {
        let s = Settings {
            jpeg_quality: 0,
            annotation_color: "nope".into(),
            save_directory: Some("Z:/does/not/exist/xyz".into()),
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(s.jpeg_quality, 90);
        assert_eq!(s.annotation_color, "#e11d48");
        assert_eq!(s.save_directory, None);
        let s = Settings {
            filename_template: "a/b".into(),
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(s.filename_template, naming::DEFAULT_TEMPLATE);
    }

    #[test]
    fn validate_rejects_bad_template() {
        let s = Settings {
            filename_template: "{bogus}".into(),
            ..Settings::default()
        };
        assert!(s.validate().is_err());
    }

    #[test]
    fn validate_rejects_missing_folder() {
        let s = Settings {
            save_directory: Some("Z:/does/not/exist/xyz".into()),
            ..Settings::default()
        };
        assert!(s.validate().is_err());
    }
}
