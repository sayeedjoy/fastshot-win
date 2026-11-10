//! Win32 + GDI implementation of the native overlay.
//!
//! One thread per capture owns every overlay window (one per monitor) and runs
//! their message loop. Each window keeps two DIBs: the frozen frame (BGRA) and
//! a back buffer. A paint copies the frame, darkens everything outside the
//! selection with `AlphaBlend`, draws the border and labels, and presents only
//! the dirty rectangle. All of it is freed when the thread exits.
//!
//! Re-entrancy: Win32 calls such as `ShowWindow` or `ReleaseCapture` send
//! messages synchronously. State lives in a thread-local `RefCell`, borrowed
//! only while handling a message; any re-entrant message falls through to
//! `DefWindowProcW`, and side effects that send messages run after the borrow.

use std::cell::RefCell;
use std::sync::{Arc, Mutex, Once, mpsc};
use std::time::{Duration, Instant};

use image::RgbaImage;
use image::imageops::{self, FilterType};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_OVER, AlphaBlend, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, BeginPaint,
    BitBlt, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateCompatibleBitmap, CreateCompatibleDC,
    CreateDIBSection, CreateFontW, CreateSolidBrush, DEFAULT_CHARSET, DIB_RGB_COLORS,
    DT_CALCRECT, DT_NOPREFIX, DT_SINGLELINE, DeleteDC, DeleteObject, DrawTextW, EndPaint,
    FW_SEMIBOLD, FillRect, GetDC, HBITMAP, HDC, HFONT, HGDIOBJ, InvalidateRect,
    OUT_DEFAULT_PRECIS, PAINTSTRUCT, ReleaseDC, SRCCOPY, SelectObject, SetBkMode, SetTextColor,
    TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, SetFocus, VK_CONTROL, VK_ESCAPE, VK_RETURN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_DBLCLKS, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    IDC_CROSS, KillTimer, LoadCursorW, MSG, PostThreadMessageW, RegisterClassExW, SW_HIDE,
    SW_SHOWNOACTIVATE, SetForegroundWindow, SetTimer, ShowWindow, TranslateMessage, WM_APP,
    WM_DPICHANGED, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEMOVE, WM_PAINT, WM_QUIT, WM_RBUTTONUP, WM_TIMER, WNDCLASSEXW, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use super::{Action, Handler};
use crate::capture::WindowCandidate;
use crate::error::{AppError, AppResult};
use crate::geometry::PixelRect;
use crate::session::{CaptureMode, Session};

const CLASS_NAME: PCWSTR = w!("FastShotNativeOverlay");
/// Posted to the overlay thread when `notify` has left a message in the inbox.
const WM_APP_NOTICE: u32 = WM_APP + 1;
const NOTICE_TIMER: usize = 1;
const NOTICE_MS: u32 = 4000;
/// Below this pointer travel a press is a click, not a drag (in px).
const DRAG_THRESHOLD: i32 = 4;
/// Opacity of the black wash outside the selection (0-255).
const DIM_ALPHA: u8 = 115;

const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF(r as u32 | (g as u32) << 8 | (b as u32) << 16)
}
const ACCENT: COLORREF = rgb(56, 189, 248);
const PANEL: COLORREF = rgb(24, 24, 27);
const TEXT: COLORREF = rgb(250, 250, 250);

struct Active {
    session_id: String,
    thread_id: u32,
    inbox: Arc<Mutex<Option<String>>>,
}

static ACTIVE: Mutex<Option<Active>> = Mutex::new(None);

thread_local! {
    static OVERLAY: RefCell<Option<Overlay>> = const { RefCell::new(None) };
}

fn lock_active() -> std::sync::MutexGuard<'static, Option<Active>> {
    ACTIVE.lock().unwrap_or_else(|p| p.into_inner())
}

// ------------------------------------------------------------- public API ---

pub fn open(session: Arc<Session>, auto_select: bool, handler: Handler) -> AppResult<()> {
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
    let id = session.id.clone();
    std::thread::Builder::new()
        .name("fastshot-native-overlay".into())
        .spawn(move || run(session, auto_select, handler, ready_tx))
        .map_err(|e| AppError::Internal(e.to_string()))?;
    match ready_rx.recv_timeout(Duration::from_secs(3)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(AppError::Internal(format!("Native overlay failed: {e}"))),
        Err(_) => {
            close(&id);
            Err(AppError::Internal("Native overlay didn't start in time.".into()))
        }
    }
}

pub fn close(session_id: &str) {
    let mut active = lock_active();
    if active.as_ref().is_some_and(|a| a.session_id == session_id)
        && let Some(a) = active.take()
    {
        // SAFETY: plain message post to a thread id we created.
        let _ = unsafe { PostThreadMessageW(a.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
    }
}

pub fn notify(session_id: &str, message: &str) {
    let active = lock_active();
    if let Some(a) = active.as_ref().filter(|a| a.session_id == session_id) {
        *a.inbox.lock().unwrap_or_else(|p| p.into_inner()) = Some(message.to_string());
        // SAFETY: plain message post to a thread id we created.
        let _ = unsafe { PostThreadMessageW(a.thread_id, WM_APP_NOTICE, WPARAM(0), LPARAM(0)) };
    }
}

// ------------------------------------------------------------ thread body ---

fn run(
    session: Arc<Session>,
    auto_select: bool,
    handler: Handler,
    ready: mpsc::Sender<Result<(), String>>,
) {
    let started = Instant::now();
    // SAFETY: affects only this thread; the windows it creates use physical
    // pixels, matching the monitor geometry and frame sizes on Windows.
    unsafe {
        let _ = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let inbox = Arc::new(Mutex::new(None));
    // SAFETY: GDI/USER calls on this thread with handles it owns.
    let overlay = match unsafe { Overlay::create(&session, auto_select, handler, inbox.clone()) } {
        Ok(o) => o,
        Err(e) => {
            let _ = ready.send(Err(e.message().to_string()));
            return;
        }
    };
    let hwnds: Vec<HWND> = overlay.surfaces.iter().map(|s| s.hwnd).collect();
    let focus = hwnds[overlay.focus];
    let thread_id = unsafe { GetCurrentThreadId() };
    *lock_active() = Some(Active {
        session_id: session.id.clone(),
        thread_id,
        inbox,
    });
    drop(session); // the handler keeps its own reference
    OVERLAY.with(|cell| *cell.borrow_mut() = Some(overlay));

    // If startup timed out, do not reveal a late selector over the fallback.
    let accepted = ready.send(Ok(())).is_ok();
    // SAFETY: windows created on this thread; no state borrow is held, so
    // the messages these calls send can be handled normally.
    unsafe {
        if accepted {
            for &hwnd in &hwnds {
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            }
            let _ = SetForegroundWindow(focus);
            let _ = SetFocus(Some(focus));
        }
    }
    eprintln!("native overlay shown in {} ms", started.elapsed().as_millis());

    let mut msg = MSG::default();
    // SAFETY: standard message loop for windows owned by this thread.
    unsafe {
        while accepted && GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            if msg.hwnd.0.is_null() && msg.message == WM_APP_NOTICE {
                OVERLAY.with(|cell| {
                    if let Ok(mut guard) = cell.try_borrow_mut()
                        && let Some(ov) = guard.as_mut()
                    {
                        ov.take_notice();
                    }
                });
                continue;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    // Tear down: take the state first so messages sent by DestroyWindow see
    // no overlay and fall through to the default handler.
    if let Some(overlay) = OVERLAY.with(|cell| cell.borrow_mut().take()) {
        // SAFETY: handles created by this thread, destroyed exactly once.
        unsafe { overlay.destroy() };
    }
    let mut active = lock_active();
    if active.as_ref().is_some_and(|a| a.thread_id == thread_id) {
        *active = None;
    }
}

// ------------------------------------------------------------------ state ---

/// One monitor's window and its GDI surfaces. Client coordinates are the
/// window's physical pixels; `img_scale` maps them to frame pixels.
struct Surface {
    hwnd: HWND,
    width: i32,
    height: i32,
    img_width: u32,
    img_height: u32,
    frame_dc: HDC,
    frame_bmp: HBITMAP,
    frame_old: HGDIOBJ,
    back_dc: HDC,
    back_bmp: HBITMAP,
    back_old: HGDIOBJ,
    font: HFONT,
    /// UI scale (DPI / 96) for borders and text.
    ui: f32,
}

impl Surface {
    fn client(&self) -> RECT {
        RECT {
            left: 0,
            top: 0,
            right: self.width,
            bottom: self.height,
        }
    }

    /// Client rect → frame pixel crop.
    fn to_image(&self, r: RECT) -> PixelRect {
        let sx = self.img_width as f64 / self.width.max(1) as f64;
        let sy = self.img_height as f64 / self.height.max(1) as f64;
        let x0 = ((r.left as f64 * sx).round() as i64).clamp(0, self.img_width as i64);
        let y0 = ((r.top as f64 * sy).round() as i64).clamp(0, self.img_height as i64);
        let x1 = ((r.right as f64 * sx).round() as i64).clamp(0, self.img_width as i64);
        let y1 = ((r.bottom as f64 * sy).round() as i64).clamp(0, self.img_height as i64);
        PixelRect::new(
            x0 as i32,
            y0 as i32,
            (x1 - x0).max(1) as u32,
            (y1 - y0).max(1) as u32,
        )
    }

    /// Frame pixel rect → client rect.
    fn to_client(&self, r: PixelRect) -> RECT {
        let sx = self.width as f64 / self.img_width.max(1) as f64;
        let sy = self.height as f64 / self.img_height.max(1) as f64;
        RECT {
            left: (r.x as f64 * sx).round() as i32,
            top: (r.y as f64 * sy).round() as i32,
            right: ((r.x as f64 + r.width as f64) * sx).round() as i32,
            bottom: ((r.y as f64 + r.height as f64) * sy).round() as i32,
        }
    }

    fn px(&self, v: f32) -> i32 {
        (v * self.ui).round().max(1.0) as i32
    }

    /// Bottom band reserved for the hint pill (for invalidation).
    fn hint_band(&self) -> RECT {
        RECT {
            left: 0,
            top: self.height - self.px(120.0),
            right: self.width,
            bottom: self.height,
        }
    }
}

struct Drag {
    monitor: usize,
    start: POINT,
    moved: bool,
}

struct Overlay {
    surfaces: Vec<Surface>,
    windows: Vec<WindowCandidate>,
    mode: CaptureMode,
    auto_select: bool,
    focus: usize,
    /// Monitor under the pointer: shows the hint.
    pointer: usize,
    drag: Option<Drag>,
    /// Committed or in-progress selection: (monitor, client rect).
    selection: Option<(usize, RECT)>,
    /// Pick candidate under the pointer while nothing is selected.
    hover: Option<(usize, RECT)>,
    notice: Option<String>,
    busy: bool,
    inbox: Arc<Mutex<Option<String>>>,
    /// 1×1 black bitmap stretched by `AlphaBlend` to dim the frame.
    black_dc: HDC,
    black_bmp: HBITMAP,
    black_old: HGDIOBJ,
    handler: Handler,
    first_paint: Option<Instant>,
}

/// Work to do after the state borrow is released.
#[derive(Default)]
struct Effects {
    capture: Option<HWND>,
    release: bool,
    hide: Vec<HWND>,
    action: Option<Action>,
}

impl Overlay {
    unsafe fn create(
        session: &Session,
        auto_select: bool,
        handler: Handler,
        inbox: Arc<Mutex<Option<String>>>,
    ) -> windows::core::Result<Overlay> {
        unsafe {
            register_class()?;
            let instance = GetModuleHandleW(None)?;
            let screen = GetDC(None);
            let (black_dc, black_bmp, black_old) = dib_from_bgra(screen, 1, 1, &[0, 0, 0, 255])?;
            let mut overlay = Overlay {
                surfaces: Vec::with_capacity(session.frames.len()),
                windows: session.windows.clone(),
                mode: session.mode,
                auto_select,
                focus: session.focus_index.min(session.frames.len().saturating_sub(1)),
                pointer: session.focus_index,
                drag: None,
                selection: None,
                hover: None,
                notice: None,
                busy: false,
                inbox,
                black_dc,
                black_bmp,
                black_old,
                handler,
                first_paint: Some(Instant::now()),
            };
            for frame in &session.frames {
                let g = frame.geometry;
                let (width, height) = (g.width as i32, g.height as i32);
                // Windows geometry is physical, so this is normally a no-op.
                let resized;
                let image: &RgbaImage = if frame.image.dimensions() == (g.width, g.height) {
                    &frame.image
                } else {
                    resized = imageops::resize(&frame.image, g.width, g.height, FilterType::Triangle);
                    &resized
                };
                let mut bgra = image.as_raw().clone();
                for px in bgra.as_chunks_mut::<4>().0 {
                    px.swap(0, 2);
                }
                let result = (|| -> windows::core::Result<Surface> {
                    let (frame_dc, frame_bmp, frame_old) =
                        dib_from_bgra(screen, width, height, &bgra)?;
                    let back_dc = CreateCompatibleDC(Some(screen));
                    let back_bmp = CreateCompatibleBitmap(screen, width, height);
                    let back_old = SelectObject(back_dc, back_bmp.into());
                    let ui = frame.scale_factor.max(1.0);
                    let font = CreateFontW(
                        -(14.0 * ui).round() as i32,
                        0,
                        0,
                        0,
                        FW_SEMIBOLD.0 as i32,
                        0,
                        0,
                        0,
                        DEFAULT_CHARSET,
                        OUT_DEFAULT_PRECIS,
                        CLIP_DEFAULT_PRECIS,
                        CLEARTYPE_QUALITY,
                        0,
                        w!("Segoe UI"),
                    );
                    let hwnd = CreateWindowExW(
                        WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                        CLASS_NAME,
                        w!("FastShot capture"),
                        WS_POPUP,
                        g.x,
                        g.y,
                        width,
                        height,
                        None,
                        None,
                        Some(instance.into()),
                        None,
                    )?;
                    Ok(Surface {
                        hwnd,
                        width,
                        height,
                        img_width: frame.image.width(),
                        img_height: frame.image.height(),
                        frame_dc,
                        frame_bmp,
                        frame_old,
                        back_dc,
                        back_bmp,
                        back_old,
                        font,
                        ui,
                    })
                })();
                match result {
                    Ok(s) => overlay.surfaces.push(s),
                    Err(e) => {
                        ReleaseDC(None, screen);
                        overlay.destroy();
                        return Err(e);
                    }
                }
            }
            ReleaseDC(None, screen);
            Ok(overlay)
        }
    }

    unsafe fn destroy(self) {
        unsafe {
            for s in &self.surfaces {
                let _ = DestroyWindow(s.hwnd);
                SelectObject(s.frame_dc, s.frame_old);
                let _ = DeleteObject(s.frame_bmp.into());
                let _ = DeleteDC(s.frame_dc);
                SelectObject(s.back_dc, s.back_old);
                let _ = DeleteObject(s.back_bmp.into());
                let _ = DeleteDC(s.back_dc);
                let _ = DeleteObject(s.font.into());
            }
            SelectObject(self.black_dc, self.black_old);
            let _ = DeleteObject(self.black_bmp.into());
            let _ = DeleteDC(self.black_dc);
        }
    }

    fn surface_of(&self, hwnd: HWND) -> Option<usize> {
        self.surfaces.iter().position(|s| s.hwnd == hwnd)
    }

    /// Pick target at `p` on monitor `i`, by capture mode.
    fn pick(&self, i: usize, p: POINT) -> Option<RECT> {
        let s = &self.surfaces[i];
        match self.mode {
            CaptureMode::Display => Some(s.client()),
            CaptureMode::Window => self.window_at(i, p),
            CaptureMode::Region if self.auto_select => self.window_at(i, p),
            CaptureMode::Region => None,
        }
    }

    fn window_at(&self, i: usize, p: POINT) -> Option<RECT> {
        let s = &self.surfaces[i];
        let q = s.to_image(RECT {
            left: p.x,
            top: p.y,
            right: p.x + 1,
            bottom: p.y + 1,
        });
        // Sorted topmost first.
        self.windows
            .iter()
            .filter(|w| w.monitor_index == i)
            .find(|w| {
                let r = w.rect;
                q.x >= r.x
                    && q.x < r.x + r.width as i32
                    && q.y >= r.y
                    && q.y < r.y + r.height as i32
            })
            .map(|w| intersect(s.to_client(w.rect), s.client()))
    }

    /// Everything currently drawn over the frame, per monitor, so a state
    /// change can repaint exactly what moved.
    fn dirty(&self) -> Vec<(usize, RECT)> {
        let mut out = Vec::new();
        let mut decor = |i: usize, r: RECT, label: bool| {
            let s = &self.surfaces[i];
            let m = s.px(4.0);
            let mut d = inflate(r, m);
            if label {
                d.top -= s.px(40.0);
                d.right = d.right.max(d.left + s.px(200.0));
            }
            out.push((i, d));
        };
        if let Some((i, r)) = self.selection {
            decor(i, r, true);
        }
        if let Some((i, r)) = self.hover {
            decor(i, r, false);
        }
        if let Some((i, r)) = self.edit_button() {
            out.push((i, inflate(r, self.surfaces[i].px(1.0))));
        }
        if let Some(s) = self.surfaces.get(self.pointer) {
            out.push((self.pointer, s.hint_band()));
        }
        out
    }

    fn invalidate(&self, rects: &[(usize, RECT)]) {
        for (i, r) in rects {
            if let Some(s) = self.surfaces.get(*i) {
                // SAFETY: window owned by this thread; does not send messages.
                let _ = unsafe { InvalidateRect(Some(s.hwnd), Some(r), false) };
            }
        }
    }

    /// Apply `change`, then repaint the union of what was and is now drawn.
    fn update(&mut self, change: impl FnOnce(&mut Self)) {
        let before = self.dirty();
        change(self);
        let after = self.dirty();
        self.invalidate(&before);
        self.invalidate(&after);
    }

    fn take_notice(&mut self) {
        let message = self.inbox.lock().unwrap_or_else(|p| p.into_inner()).take();
        let Some(message) = message else { return };
        let hwnd = self.surfaces[self.pointer.min(self.surfaces.len() - 1)].hwnd;
        self.update(|ov| {
            ov.busy = false;
            ov.notice = Some(message);
        });
        // SAFETY: timer on a window owned by this thread.
        unsafe { SetTimer(Some(hwnd), NOTICE_TIMER, NOTICE_MS, None) };
    }

    fn selection_crop(&self) -> Option<(usize, PixelRect)> {
        let (i, r) = self.selection?;
        (r.right - r.left >= 1 && r.bottom - r.top >= 1).then(|| (i, self.surfaces[i].to_image(r)))
    }

    fn edit_button(&self) -> Option<(usize, RECT)> {
        let (i, _) = self.selection?;
        let s = &self.surfaces[i];
        let width = s.px(120.0).min(s.width);
        let left = ((s.width - width) / 2).max(0);
        let top = (s.height - s.px(106.0)).max(0);
        Some((i, RECT { left, top, right: left + width, bottom: (top + s.px(36.0)).min(s.height) }))
    }

    fn edit(&mut self, fx: &mut Effects) {
        if let Some((monitor, crop)) = self.selection_crop() {
            self.update(|ov| {
                ov.busy = true;
                ov.notice = Some("Opening editor... Esc to cancel".into());
            });
            fx.action = Some(Action::Edit { monitor, crop });
        }
    }

    fn handle(&mut self, i: usize, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<Effects> {
        let mut fx = Effects::default();
        match msg {
            WM_PAINT => {
                // SAFETY: paint of a window owned by this thread.
                unsafe { self.paint(i) };
            }
            WM_MOUSEMOVE => {
                let p = clamp_point(point(lparam), self.surfaces[i].client());
                let pick = if self.selection.is_none() && self.drag.is_none() && !self.busy {
                    self.pick(i, p).map(|r| (i, r))
                } else {
                    None
                };
                self.update(|ov| {
                    ov.pointer = i;
                    if pick != ov.hover {
                        ov.hover = pick;
                    }
                    if let Some(d) = &mut ov.drag
                        && d.monitor == i
                    {
                        if !d.moved
                            && ((p.x - d.start.x).abs() > DRAG_THRESHOLD
                                || (p.y - d.start.y).abs() > DRAG_THRESHOLD)
                        {
                            d.moved = true;
                        }
                        if d.moved {
                            ov.selection = Some((i, rect_from(d.start, p)));
                            ov.hover = None;
                        }
                    }
                });
            }
            WM_LBUTTONDOWN if !self.busy => {
                let p = clamp_point(point(lparam), self.surfaces[i].client());
                if self.edit_button().is_some_and(|(m, r)| m == i && contains(r, p)) {
                    self.edit(&mut fx);
                    return Some(fx);
                }
                self.drag = Some(Drag {
                    monitor: i,
                    start: p,
                    moved: false,
                });
                fx.capture = Some(self.surfaces[i].hwnd);
            }
            WM_LBUTTONUP => {
                fx.release = true;
                let Some(drag) = self.drag.take() else {
                    return Some(fx);
                };
                if !drag.moved {
                    // A click picks the window/display under the pointer, or
                    // keeps the current selection when there's nothing to pick.
                    if let Some(r) = self.pick(drag.monitor, drag.start) {
                        self.update(|ov| {
                            ov.selection = Some((drag.monitor, r));
                            ov.hover = None;
                        });
                    }
                } else if let Some((_, r)) = self.selection
                    && (r.right - r.left < 2 || r.bottom - r.top < 2)
                {
                    self.update(|ov| ov.selection = None);
                }
            }
            WM_LBUTTONDBLCLK if !self.busy => {
                let p = point(lparam);
                if let Some((sel_i, r)) = self.selection
                    && sel_i == i
                    && contains(r, p)
                {
                    self.copy(&mut fx);
                }
            }
            WM_RBUTTONUP => fx.action = Some(Action::Cancel),
            WM_KEYDOWN => {
                let key = wparam.0 as u16;
                // SAFETY: reads this thread's keyboard state.
                let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
                let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
                if key == VK_ESCAPE.0 {
                    fx.action = Some(Action::Cancel);
                } else if self.busy {
                } else if !ctrl && key == b'E' as u16 {
                    self.edit(&mut fx);
                } else if !ctrl && !shift && (b'1' as u16..=b'3' as u16).contains(&key) {
                    self.update(|ov| {
                        ov.mode = match key as u8 {
                            b'2' => CaptureMode::Window,
                            b'3' => CaptureMode::Display,
                            _ => CaptureMode::Region,
                        };
                        ov.selection = None;
                        ov.hover = None;
                        ov.drag = None;
                    });
                    fx.release = true;
                } else if key == VK_RETURN.0 || (ctrl && !shift && key == b'C' as u16) {
                    self.copy(&mut fx);
                } else if ctrl && shift && key == b'C' as u16 {
                    if let Some((monitor, crop)) = self.selection_crop() {
                        self.update(|ov| {
                            ov.busy = true;
                            ov.notice = Some("Reading text…".into());
                        });
                        fx.action = Some(Action::CopyText { monitor, crop });
                    }
                } else {
                    return None;
                }
            }
            WM_TIMER if wparam.0 == NOTICE_TIMER => {
                // SAFETY: timer on a window owned by this thread.
                let _ = unsafe { KillTimer(Some(self.surfaces[i].hwnd), NOTICE_TIMER) };
                if !self.busy {
                    self.update(|ov| ov.notice = None);
                }
            }
            _ => return None,
        }
        Some(fx)
    }

    fn copy(&mut self, fx: &mut Effects) {
        if let Some((monitor, crop)) = self.selection_crop() {
            // Dismiss at once; the copy finishes in the background.
            fx.hide = self.surfaces.iter().map(|s| s.hwnd).collect();
            fx.action = Some(Action::Copy { monitor, crop });
        }
    }

    unsafe fn paint(&mut self, i: usize) {
        unsafe {
            let s = &self.surfaces[i];
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(s.hwnd, &mut ps);
            let r = intersect(ps.rcPaint, s.client());
            let (w, h) = (r.right - r.left, r.bottom - r.top);
            if w > 0 && h > 0 {
                let dc = s.back_dc;
                let _ = BitBlt(dc, r.left, r.top, w, h, Some(s.frame_dc), r.left, r.top, SRCCOPY);

                let selection = self.selection.filter(|(m, _)| *m == i).map(|(_, r)| r);
                let lit = selection.or(self.hover.filter(|(m, _)| *m == i).map(|(_, r)| r));
                for part in subtract(r, lit) {
                    self.dim(dc, part);
                }
                if let Some(l) = lit {
                    border(dc, l, s.px(2.0));
                }
                if let Some(sel) = selection {
                    let crop = s.to_image(sel);
                    let label = format!("{} × {}", crop.width, crop.height);
                    let below = sel.top < s.px(36.0);
                    pill(dc, s, &label, sel.left, if below { sel.top + s.px(8.0) } else { sel.top - s.px(8.0) }, below);
                }
                if i == self.pointer {
                    let text = self.hint_text();
                    let mut size = measure(dc, s, &text);
                    let pad = s.px(10.0);
                    size.0 += 2 * pad;
                    let x = (s.width - size.0) / 2;
                    let y = s.height - s.px(56.0);
                    pill_at(dc, s, &text, x, y, pad);
                }
                if let Some((monitor, button)) = self.edit_button()
                    && monitor == i
                {
                    let brush = CreateSolidBrush(PANEL);
                    FillRect(dc, &button, brush);
                    let _ = DeleteObject(brush.into());
                    border(dc, button, s.px(1.0));
                    let text = "Edit (E)";
                    let size = measure(dc, s, text);
                    pill_at(dc, s, text, button.left + ((button.right - button.left - size.0) / 2).max(0), button.top, 0);
                }
                let _ = BitBlt(hdc, r.left, r.top, w, h, Some(dc), r.left, r.top, SRCCOPY);
            }
            let _ = EndPaint(s.hwnd, &ps);
            if i == self.focus
                && let Some(t) = self.first_paint.take()
            {
                eprintln!("native overlay first paint {} ms after windows were created", t.elapsed().as_millis());
            }
        }
    }

    unsafe fn dim(&self, dc: HDC, r: RECT) {
        let (w, h) = (r.right - r.left, r.bottom - r.top);
        if w <= 0 || h <= 0 {
            return;
        }
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: DIM_ALPHA,
            AlphaFormat: 0,
        };
        let _ = unsafe { AlphaBlend(dc, r.left, r.top, w, h, self.black_dc, 0, 0, 1, 1, blend) };
    }

    fn hint_text(&self) -> String {
        if let Some(n) = &self.notice {
            return n.clone();
        }
        if self.selection.is_some() {
            return "E edit / save  ·  Enter copy  ·  Ctrl+Shift+C copy text  ·  Esc cancel".into();
        }
        match self.mode {
            CaptureMode::Display => "Click a display  ·  Esc to cancel".into(),
            CaptureMode::Window => "Click a window  ·  Esc to cancel".into(),
            CaptureMode::Region if self.auto_select => {
                "Drag to select or click a window  ·  Right-click or Esc to cancel".into()
            }
            CaptureMode::Region => "Drag to select  ·  Right-click or Esc to cancel".into(),
        }
    }
}

// ---------------------------------------------------------------- wndproc ---

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        // Every pixel is painted from the back buffer; no erase, no flicker.
        WM_ERASEBKGND => return LRESULT(1),
        // Keep the exact physical size we were given when crossing DPIs.
        WM_DPICHANGED => return LRESULT(0),
        _ => {}
    }
    let outcome = OVERLAY.with(|cell| {
        let mut guard = cell.try_borrow_mut().ok()?;
        let ov = guard.as_mut()?;
        let i = ov.surface_of(hwnd)?;
        ov.handle(i, msg, wparam, lparam)
    });
    let Some(fx) = outcome else {
        // SAFETY: default processing for our own window.
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    };
    // SAFETY: windows owned by this thread; no state borrow is held now.
    unsafe {
        if let Some(h) = fx.capture {
            SetCapture(h);
        }
        if fx.release {
            let _ = ReleaseCapture();
        }
        for h in fx.hide {
            let _ = ShowWindow(h, SW_HIDE);
        }
    }
    if let Some(action) = fx.action {
        OVERLAY.with(|cell| {
            if let Ok(guard) = cell.try_borrow()
                && let Some(ov) = guard.as_ref()
            {
                (ov.handler)(action);
            }
        });
    }
    LRESULT(0)
}

fn register_class() -> windows::core::Result<()> {
    static REGISTER: Once = Once::new();
    let mut result = Ok(());
    REGISTER.call_once(|| {
        // SAFETY: registers a class whose window procedure lives for the
        // whole process.
        result = (|| unsafe {
            let instance = GetModuleHandleW(None)?;
            let class = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                style: CS_DBLCLKS,
                lpfnWndProc: Some(wndproc),
                hInstance: instance.into(),
                hCursor: LoadCursorW(None, IDC_CROSS)?,
                lpszClassName: CLASS_NAME,
                ..Default::default()
            };
            if RegisterClassExW(&class) == 0 {
                Err(windows::core::Error::from_thread())
            } else {
                Ok(())
            }
        })();
    });
    result
}

// ------------------------------------------------------------ GDI helpers ---

/// A 32-bit top-down DIB selected into a new memory DC, filled with `bgra`.
unsafe fn dib_from_bgra(
    screen: HDC,
    width: i32,
    height: i32,
    bgra: &[u8],
) -> windows::core::Result<(HDC, HBITMAP, HGDIOBJ)> {
    unsafe {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let bmp = CreateDIBSection(Some(screen), &info, DIB_RGB_COLORS, &mut bits, None, 0)?;
        let len = width as usize * height as usize * 4;
        if bits.is_null() || bgra.len() < len {
            let _ = DeleteObject(bmp.into());
            return Err(windows::core::Error::from_thread());
        }
        std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits as *mut u8, len);
        let dc = CreateCompatibleDC(Some(screen));
        let old = SelectObject(dc, bmp.into());
        Ok((dc, bmp, old))
    }
}

/// 2-px accent border drawn just outside `r`, so it never covers pixels that
/// will be exported.
fn border(dc: HDC, r: RECT, t: i32) {
    // SAFETY: draws into a memory DC owned by the caller.
    unsafe {
        let brush = CreateSolidBrush(ACCENT);
        for edge in [
            RECT { left: r.left - t, top: r.top - t, right: r.right + t, bottom: r.top },
            RECT { left: r.left - t, top: r.bottom, right: r.right + t, bottom: r.bottom + t },
            RECT { left: r.left - t, top: r.top, right: r.left, bottom: r.bottom },
            RECT { left: r.right, top: r.top, right: r.right + t, bottom: r.bottom },
        ] {
            FillRect(dc, &edge, brush);
        }
        let _ = DeleteObject(brush.into());
    }
}

fn measure(dc: HDC, s: &Surface, text: &str) -> (i32, i32) {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    let mut r = RECT::default();
    // SAFETY: measures text in a memory DC owned by the caller.
    unsafe {
        let old = SelectObject(dc, s.font.into());
        DrawTextW(dc, &mut wide, &mut r, DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX);
        SelectObject(dc, old);
    }
    (r.right - r.left, r.bottom - r.top)
}

/// Size label anchored to the selection's top-left (above it, or inside when
/// there's no room above).
fn pill(dc: HDC, s: &Surface, text: &str, x: i32, y: i32, below: bool) {
    let pad = s.px(6.0);
    let (_, th) = measure(dc, s, text);
    let h = th + pad;
    let y = if below { y } else { y - h };
    pill_at(dc, s, text, x.max(0), y.max(0), pad);
}

fn pill_at(dc: HDC, s: &Surface, text: &str, x: i32, y: i32, pad: i32) {
    let (tw, th) = measure(dc, s, text);
    let vpad = (pad / 2).max(2);
    let mut r = RECT {
        left: x,
        top: y,
        right: x + tw + 2 * pad,
        bottom: y + th + 2 * vpad,
    };
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    // SAFETY: draws into a memory DC owned by the caller.
    unsafe {
        let brush = CreateSolidBrush(PANEL);
        FillRect(dc, &r, brush);
        let _ = DeleteObject(brush.into());
        let old = SelectObject(dc, s.font.into());
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, TEXT);
        r.left += pad;
        r.top += vpad;
        DrawTextW(dc, &mut wide, &mut r, DT_SINGLELINE | DT_NOPREFIX);
        SelectObject(dc, old);
    }
}

// ----------------------------------------------------------- rect helpers ---

fn point(lparam: LPARAM) -> POINT {
    POINT {
        x: (lparam.0 & 0xFFFF) as u16 as i16 as i32,
        y: ((lparam.0 >> 16) & 0xFFFF) as u16 as i16 as i32,
    }
}

fn clamp_point(p: POINT, r: RECT) -> POINT {
    POINT {
        x: p.x.clamp(r.left, r.right),
        y: p.y.clamp(r.top, r.bottom),
    }
}

fn rect_from(a: POINT, b: POINT) -> RECT {
    RECT {
        left: a.x.min(b.x),
        top: a.y.min(b.y),
        right: a.x.max(b.x),
        bottom: a.y.max(b.y),
    }
}

fn contains(r: RECT, p: POINT) -> bool {
    p.x >= r.left && p.x < r.right && p.y >= r.top && p.y < r.bottom
}

fn inflate(r: RECT, m: i32) -> RECT {
    RECT {
        left: r.left - m,
        top: r.top - m,
        right: r.right + m,
        bottom: r.bottom + m,
    }
}

fn intersect(a: RECT, b: RECT) -> RECT {
    let r = RECT {
        left: a.left.max(b.left),
        top: a.top.max(b.top),
        right: a.right.min(b.right),
        bottom: a.bottom.min(b.bottom),
    };
    if r.right <= r.left || r.bottom <= r.top {
        RECT::default()
    } else {
        r
    }
}

/// `r` minus `hole`, as up to four non-overlapping rects.
fn subtract(r: RECT, hole: Option<RECT>) -> Vec<RECT> {
    let Some(hole) = hole else { return vec![r] };
    let h = intersect(r, hole);
    if h.right <= h.left || h.bottom <= h.top {
        return vec![r];
    }
    [
        RECT { left: r.left, top: r.top, right: r.right, bottom: h.top },
        RECT { left: r.left, top: h.bottom, right: r.right, bottom: r.bottom },
        RECT { left: r.left, top: h.top, right: h.left, bottom: h.bottom },
        RECT { left: h.right, top: h.top, right: r.right, bottom: h.bottom },
    ]
    .into_iter()
    .filter(|p| p.right > p.left && p.bottom > p.top)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(l: i32, t: i32, r: i32, b: i32) -> RECT {
        RECT { left: l, top: t, right: r, bottom: b }
    }
    fn area(rs: &[RECT]) -> i32 {
        rs.iter().map(|r| (r.right - r.left) * (r.bottom - r.top)).sum()
    }

    #[test]
    fn subtract_leaves_the_frame_around_a_hole() {
        let parts = subtract(rect(0, 0, 100, 100), Some(rect(20, 30, 60, 70)));
        assert_eq!(parts.len(), 4);
        assert_eq!(area(&parts), 100 * 100 - 40 * 40);
    }

    #[test]
    fn subtract_without_overlap_is_identity() {
        assert_eq!(subtract(rect(0, 0, 10, 10), Some(rect(50, 50, 60, 60))), vec![rect(0, 0, 10, 10)]);
        assert_eq!(subtract(rect(0, 0, 10, 10), None), vec![rect(0, 0, 10, 10)]);
        assert!(subtract(rect(0, 0, 10, 10), Some(rect(-5, -5, 20, 20))).is_empty());
    }

    #[test]
    fn drag_rect_is_normalised() {
        assert_eq!(rect_from(POINT { x: 50, y: 10 }, POINT { x: 5, y: 40 }), rect(5, 10, 50, 40));
    }

    /// Real windows on the real desktop: opens the overlay over a frozen
    /// capture, drives a drag + Enter with posted messages, and checks the
    /// resulting action and teardown. Opt-in because it flashes the screen:
    /// `cargo test -- --ignored native_overlay_drag_and_copy --nocapture`
    #[test]
    #[ignore = "opens real overlay windows on the desktop"]
    fn native_overlay_drag_and_copy() {
        drive_native_selection(false, false);
    }

    #[test]
    #[ignore = "opens real overlay windows on the desktop"]
    fn native_overlay_edit_after_idle() {
        drive_native_selection(true, false);
        std::thread::sleep(Duration::from_secs(31));
        drive_native_selection(true, true);
    }

    fn drive_native_selection(edit: bool, click: bool) {
        use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW};

        let monitors = crate::capture::freeze_all_monitors().unwrap();
        let session = Arc::new(Session::new(CaptureMode::Region, monitors, Vec::new(), 0));
        let (tx, rx) = mpsc::channel();
        let handler: Handler = Box::new(move |a| {
            let _ = tx.send(a);
        });
        let t = Instant::now();
        open(session.clone(), false, handler).unwrap();
        println!("open() returned in {} ms", t.elapsed().as_millis());
        std::thread::sleep(Duration::from_millis(400));

        // Any of our overlay windows will do; drag within its client area.
        let hwnd = unsafe { FindWindowW(CLASS_NAME, None) }.unwrap();
        let at = |x: i32, y: i32| LPARAM(((y as u32) << 16 | (x as u32 & 0xFFFF)) as isize);
        unsafe {
            PostMessageW(Some(hwnd), WM_LBUTTONDOWN, WPARAM(1), at(100, 120)).unwrap();
            PostMessageW(Some(hwnd), WM_MOUSEMOVE, WPARAM(1), at(250, 200)).unwrap();
            PostMessageW(Some(hwnd), WM_MOUSEMOVE, WPARAM(1), at(400, 320)).unwrap();
            PostMessageW(Some(hwnd), WM_LBUTTONUP, WPARAM(0), at(400, 320)).unwrap();
            if click {
                let mut bounds = RECT::default();
                windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut bounds).unwrap();
                let scale = session.frames[0].scale_factor.max(1.0);
                let y = bounds.bottom - (90.0 * scale).round() as i32;
                PostMessageW(Some(hwnd), WM_LBUTTONDOWN, WPARAM(1), at(bounds.right / 2, y)).unwrap();
                PostMessageW(Some(hwnd), WM_LBUTTONUP, WPARAM(0), at(bounds.right / 2, y)).unwrap();
            } else {
                let key = if edit { b'E' as usize } else { VK_RETURN.0 as usize };
                PostMessageW(Some(hwnd), WM_KEYDOWN, WPARAM(key), LPARAM(0)).unwrap();
            }
        }
        let action = rx.recv_timeout(Duration::from_secs(3)).expect("no action");
        match action {
            Action::Copy { crop, .. } if !edit => {
                assert_eq!((crop.width, crop.height), (300, 200), "{crop:?}");
            }
            Action::Edit { crop, .. } if edit => {
                assert_eq!((crop.width, crop.height), (300, 200), "{crop:?}");
            }
            other => panic!("unexpected {other:?}"),
        }

        close(&session.id);
        let deadline = Instant::now() + Duration::from_secs(3);
        while unsafe { FindWindowW(CLASS_NAME, None) }.is_ok() {
            assert!(Instant::now() < deadline, "overlay windows were not destroyed");
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(lock_active().is_none());
    }

    #[test]
    fn lparam_coordinates_are_signed() {
        // (-3, 7): negative x arrives during mouse capture past the left edge.
        let lp = LPARAM(((7u32 << 16) | (-3i16 as u16 as u32)) as isize);
        let p = point(lp);
        assert_eq!((p.x, p.y), (-3, 7));
    }
}
