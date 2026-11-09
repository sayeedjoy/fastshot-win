//! Screen-capture permission. Only macOS gates capture behind a user grant
//! (Screen & System Audio Recording). Windows has no equivalent prompt.

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionStatus {
    Granted,
    Denied,
}

#[cfg(target_os = "macos")]
mod mac {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        pub fn CGPreflightScreenCaptureAccess() -> bool;
        pub fn CGRequestScreenCaptureAccess() -> bool;
    }
}

pub fn status() -> PermissionStatus {
    #[cfg(target_os = "macos")]
    {
        // SAFETY: argument-less CoreGraphics query, safe from any thread.
        if unsafe { mac::CGPreflightScreenCaptureAccess() } {
            PermissionStatus::Granted
        } else {
            PermissionStatus::Denied
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        PermissionStatus::Granted
    }
}

/// Ask the OS to show its permission prompt. macOS only shows it once per
/// app install; later calls return immediately. The caller must not loop.
pub fn request() -> PermissionStatus {
    #[cfg(target_os = "macos")]
    {
        // SAFETY: as above.
        if unsafe { mac::CGRequestScreenCaptureAccess() } {
            PermissionStatus::Granted
        } else {
            PermissionStatus::Denied
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        PermissionStatus::Granted
    }
}

/// Deep link to the relevant privacy pane, if the platform has one.
pub fn settings_url() -> Option<&'static str> {
    if cfg!(target_os = "macos") {
        Some("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
    } else {
        None
    }
}
