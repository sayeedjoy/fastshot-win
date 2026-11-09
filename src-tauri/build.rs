fn main() {
    // Declare app commands so each gets an `allow-*` permission and can be
    // granted per window in `capabilities/` (PRD §8.2: privileged commands
    // only reachable from the windows that need them).
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_session",
            "overlay_ready",
            "cancel_capture",
            "export_capture",
            "extract_text",
            "get_settings",
            "update_settings",
            "preview_filename",
            "browse_save_directory",
            "get_app_info",
            "begin_capture",
            "request_permission",
            "open_permission_settings",
            "quit_app",
        ]),
    ))
    .expect("failed to run tauri-build");
}
