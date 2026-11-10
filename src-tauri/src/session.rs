//! Capture session lifecycle (PRD §5.4):
//! `Idle → PreparingCapture → Selecting/Editing → Exporting → Idle`.
//!
//! Frozen frames live only in memory, inside the active `Session`. Ending the
//! session drops the last strong reference and frees the buffers. Nothing is
//! written to disk (SEC-03).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use image::RgbaImage;
use serde::{Deserialize, Serialize};

use crate::capture::{FrozenMonitor, WindowCandidate};
use crate::error::{AppError, AppResult};
use crate::geometry::{MonitorGeometry, PixelRect, validate_crop};
use crate::imaging::encode::encode_preview;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CaptureMode {
    #[default]
    Region,
    Window,
    Display,
}

pub struct Frame {
    pub name: String,
    pub is_primary: bool,
    pub scale_factor: f32,
    pub geometry: MonitorGeometry,
    pub image: RgbaImage,
    preview: OnceLock<Result<Arc<Vec<u8>>, String>>,
}

impl Frame {
    /// PNG for the overlay. Encoded once; concurrent callers wait for it.
    pub fn preview_png(&self) -> Result<Arc<Vec<u8>>, String> {
        self.preview
            .get_or_init(|| encode_preview(&self.image).map(Arc::new))
            .clone()
    }
}

pub struct Session {
    pub id: String,
    pub mode: CaptureMode,
    pub frames: Vec<Frame>,
    pub windows: Vec<WindowCandidate>,
    /// Overlay that receives keyboard focus (the monitor under the cursor).
    pub focus_index: usize,
    editor: Mutex<Option<EditorSelection>>,
    pub editor_ready: AtomicBool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorSelection {
    pub monitor: usize,
    pub crop: PixelRect,
}

impl Session {
    pub fn new(
        mode: CaptureMode,
        monitors: Vec<FrozenMonitor>,
        windows: Vec<WindowCandidate>,
        focus_index: usize,
    ) -> Session {
        let frames = monitors
            .into_iter()
            .map(|m| Frame {
                name: m.name,
                is_primary: m.is_primary,
                scale_factor: m.scale_factor,
                geometry: m.geometry,
                image: m.image,
                preview: OnceLock::new(),
            })
            .collect();
        Session {
            id: uuid::Uuid::new_v4().simple().to_string(),
            mode,
            frames,
            windows,
            focus_index,
            editor: Mutex::new(None),
            editor_ready: AtomicBool::new(false),
        }
    }

    /// Preserve the native selection in this frozen session; never recapture.
    pub fn request_editor(&self, monitor: usize, crop: PixelRect) -> AppResult<()> {
        let frame = self.frame(monitor)?;
        let crop = validate_crop(crop, frame.image.width(), frame.image.height())
            .map_err(AppError::InvalidInput)?;
        let mut editor = self.editor.lock().unwrap();
        if editor.is_some() {
            return Err(AppError::Busy);
        }
        *editor = Some(EditorSelection { monitor, crop });
        Ok(())
    }

    pub fn editor_selection(&self) -> Option<EditorSelection> {
        *self.editor.lock().unwrap()
    }

    pub fn frame(&self, index: usize) -> AppResult<&Frame> {
        self.frames
            .get(index)
            .ok_or_else(|| AppError::InvalidInput("Unknown display.".into()))
    }
}

#[derive(Default)]
pub struct SessionManager {
    preparing: AtomicBool,
    current: Mutex<Option<Arc<Session>>>,
}

/// Held while a capture is being prepared; releases the slot on drop so a
/// failure can never wedge the app in `PreparingCapture`.
pub struct PrepareGuard<'a>(&'a SessionManager);

impl Drop for PrepareGuard<'_> {
    fn drop(&mut self) {
        self.0.preparing.store(false, Ordering::SeqCst);
    }
}

impl SessionManager {
    /// Enter `PreparingCapture` unless another capture is active or starting.
    /// Repeated hotkey presses fall through here and are ignored (CAP-01).
    pub fn try_prepare(&self) -> Option<PrepareGuard<'_>> {
        if self.current.lock().unwrap().is_some() {
            return None;
        }
        self.preparing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()?;
        Some(PrepareGuard(self))
    }

    pub fn install(&self, _guard: &PrepareGuard<'_>, session: Arc<Session>) {
        *self.current.lock().unwrap() = Some(session);
    }

    pub fn get(&self, id: &str) -> AppResult<Arc<Session>> {
        match &*self.current.lock().unwrap() {
            Some(s) if s.id == id => Ok(s.clone()),
            _ => Err(AppError::SessionNotFound),
        }
    }

    pub fn current(&self) -> Option<Arc<Session>> {
        self.current.lock().unwrap().clone()
    }

    /// Run a short completion step only while `id` is still the active
    /// session. Holding the session lock makes the check atomic with respect
    /// to cancellation, which removes the session through [`Self::take`].
    pub fn complete_if_active<T>(
        &self,
        id: &str,
        complete: impl FnOnce() -> AppResult<T>,
    ) -> AppResult<T> {
        let current = self.current.lock().unwrap();
        match &*current {
            Some(session) if session.id == id => complete(),
            _ => Err(AppError::SessionNotFound),
        }
    }

    /// End the session (if `id` matches, or any session if `None`).
    pub fn take(&self, id: Option<&str>) -> Option<Arc<Session>> {
        let mut cur = self.current.lock().unwrap();
        match (&*cur, id) {
            (Some(s), Some(id)) if s.id != id => None,
            (Some(_), _) => cur.take(),
            (None, _) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Arc<Session> {
        let m = FrozenMonitor {
            name: "Test".into(),
            is_primary: true,
            scale_factor: 1.0,
            geometry: MonitorGeometry {
                x: 0,
                y: 0,
                width: 4,
                height: 4,
                pixel_width: 4,
                pixel_height: 4,
            },
            image: RgbaImage::new(4, 4),
        };
        Arc::new(Session::new(CaptureMode::Region, vec![m], vec![], 0))
    }

    #[test]
    fn only_one_session_at_a_time() {
        let mgr = SessionManager::default();
        let guard = mgr.try_prepare().expect("idle manager can prepare");
        assert!(
            mgr.try_prepare().is_none(),
            "second prepare while preparing must fail"
        );
        mgr.install(&guard, session());
        drop(guard);
        assert!(
            mgr.try_prepare().is_none(),
            "prepare while a session is active must fail"
        );
        assert!(mgr.take(None).is_some());
        assert!(
            mgr.try_prepare().is_some(),
            "idle again after the session ends"
        );
    }

    #[test]
    fn failed_prepare_releases_slot() {
        let mgr = SessionManager::default();
        {
            let _g = mgr.try_prepare().unwrap();
            // capture fails here; guard dropped without install
        }
        assert!(mgr.try_prepare().is_some());
    }

    #[test]
    fn editor_handoff_preserves_the_frozen_frame_and_selection() {
        let s = session();
        let pixels = s.frames[0].image.as_ptr();
        let crop = PixelRect::new(1, 1, 2, 3);
        s.request_editor(0, crop).unwrap();
        assert_eq!(s.editor_selection(), Some(EditorSelection { monitor: 0, crop }));
        assert_eq!(s.frames[0].image.as_ptr(), pixels);
        assert!(matches!(s.request_editor(0, crop), Err(AppError::Busy)));
        assert!(!s.editor_ready.load(Ordering::SeqCst));
    }

    #[test]
    fn editor_handoff_rejects_invalid_selections_without_claiming_the_editor() {
        let s = session();
        assert!(s.request_editor(1, PixelRect::new(0, 0, 1, 1)).is_err());
        assert!(s.request_editor(0, PixelRect::new(0, 0, 0, 1)).is_err());
        assert!(s.request_editor(0, PixelRect::new(9, 9, 1, 1)).is_err());
        assert!(s.editor_selection().is_none());
        s.request_editor(0, PixelRect::new(3, 3, 2, 2)).unwrap();
        assert_eq!(s.editor_selection().unwrap().crop, PixelRect::new(3, 3, 1, 1));
    }

    #[test]
    fn take_with_wrong_id_keeps_session() {
        let mgr = SessionManager::default();
        let g = mgr.try_prepare().unwrap();
        let s = session();
        let id = s.id.clone();
        mgr.install(&g, s);
        assert!(mgr.take(Some("nope")).is_none());
        assert!(mgr.get(&id).is_ok());
        assert!(mgr.take(Some(&id)).is_some());
        assert!(matches!(mgr.get(&id), Err(AppError::SessionNotFound)));
    }

    #[test]
    fn completion_only_runs_for_the_active_session() {
        use std::cell::Cell;

        let mgr = SessionManager::default();
        let guard = mgr.try_prepare().unwrap();
        let active = session();
        let id = active.id.clone();
        mgr.install(&guard, active);
        drop(guard);

        assert_eq!(mgr.complete_if_active(&id, || Ok(7)).unwrap(), 7);
        assert!(mgr.take(Some(&id)).is_some());

        let called = Cell::new(false);
        let result = mgr.complete_if_active(&id, || {
            called.set(true);
            Ok(())
        });
        assert!(matches!(result, Err(AppError::SessionNotFound)));
        assert!(!called.get(), "completion ran after the session ended");
    }

    #[test]
    fn preview_is_png_and_cached() {
        let s = session();
        let a = s.frames[0].preview_png().unwrap();
        let b = s.frames[0].preview_png().unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(&a[1..4], b"PNG");
    }
}
