use serde::{Serialize, Serializer, ser::SerializeStruct};

/// Errors surfaced to the UI. Messages are user-facing and must never contain
/// pixel data, window titles, or file paths (PRD §9.2 telemetry/logging rules).
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("FastShot needs screen recording permission to capture your screen.")]
    PermissionDenied,
    #[error("A capture is already in progress.")]
    Busy,
    #[error("This capture has already ended.")]
    SessionNotFound,
    #[error("{0}")]
    InvalidInput(String),
    #[error("Couldn't capture the screen. {0}")]
    Capture(String),
    #[error("Couldn't copy the image to the clipboard. {0}")]
    Clipboard(String),
    #[error("Couldn't save the image. {0}")]
    Io(String),
    #[error("Couldn't create the image. {0}")]
    Encode(String),
    #[error("{0}")]
    Shortcut(String),
    #[error("{0}")]
    Ocr(String),
    #[error("{0}")]
    Internal(String),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            AppError::PermissionDenied => "permission_denied",
            AppError::Busy => "busy",
            AppError::SessionNotFound => "session_not_found",
            AppError::InvalidInput(_) => "invalid_input",
            AppError::Capture(_) => "capture_failed",
            AppError::Clipboard(_) => "clipboard_failed",
            AppError::Io(_) => "io_failed",
            AppError::Encode(_) => "encode_failed",
            AppError::Shortcut(_) => "shortcut_failed",
            AppError::Ocr(_) => "ocr_failed",
            AppError::Internal(_) => "internal",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("code", self.code())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;
