//! Coordinate model.
//!
//! * **OS space**: what the capture backend reports for monitors and windows.
//!   On Windows these are physical pixels in the virtual desktop (may be
//!   negative). On macOS these are points (logical units).
//! * **Monitor pixel space**: pixels of one frozen monitor image, origin at its
//!   top-left. Every rect the UI sends is in this space. The UI derives it from
//!   `image_px / css_px`, so it never needs to know the OS convention.
//!
//! Converting OS → pixel space uses the per-monitor ratio `pixel_width / width`,
//! which is 1.0 on Windows and the backing scale factor on macOS.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PixelRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl PixelRect {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(&self) -> i64 {
        self.x as i64 + self.width as i64
    }

    pub fn bottom(&self) -> i64 {
        self.y as i64 + self.height as i64
    }

    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub fn intersect(&self, other: &PixelRect) -> Option<PixelRect> {
        let x0 = (self.x as i64).max(other.x as i64);
        let y0 = (self.y as i64).max(other.y as i64);
        let x1 = self.right().min(other.right());
        let y1 = self.bottom().min(other.bottom());
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        Some(PixelRect::new(
            x0 as i32,
            y0 as i32,
            (x1 - x0) as u32,
            (y1 - y0) as u32,
        ))
    }
}

/// Validate a crop rectangle requested by the UI against an image of
/// `width × height`. Rejects empty selections; clamps to the image so a
/// 1px rounding overshoot from the UI never reads out of bounds.
pub fn validate_crop(rect: PixelRect, width: u32, height: u32) -> Result<PixelRect, String> {
    if rect.is_empty() {
        return Err("The selection is empty. Drag to select an area.".into());
    }
    rect.intersect(&PixelRect::new(0, 0, width, height))
        .ok_or_else(|| "The selection is outside the captured screen.".into())
}

/// Floating-point rect in crop-relative image pixels (annotations, effects,
/// redactions). Width/height may be negative while the user is dragging; we
/// normalise before use.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct FRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl FRect {
    pub fn normalized(&self) -> FRect {
        let (x, width) = if self.width < 0.0 {
            (self.x + self.width, -self.width)
        } else {
            (self.x, self.width)
        };
        let (y, height) = if self.height < 0.0 {
            (self.y + self.height, -self.height)
        } else {
            (self.y, self.height)
        };
        FRect {
            x,
            y,
            width,
            height,
        }
    }

    pub fn is_finite(&self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
    }

    /// Smallest integer pixel rect that fully covers this rect, clamped to
    /// `width × height`. Rounding **outward** is what makes redaction safe:
    /// a partially covered edge pixel is always overwritten.
    pub fn covering_pixels(&self, width: u32, height: u32) -> Option<PixelRect> {
        let r = self.normalized();
        if !r.is_finite() {
            return None;
        }
        let x0 = (r.x.floor() as i64).max(0);
        let y0 = (r.y.floor() as i64).max(0);
        let x1 = ((r.x + r.width).ceil() as i64).min(width as i64);
        let y1 = ((r.y + r.height).ceil() as i64).min(height as i64);
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        Some(PixelRect::new(
            x0 as i32,
            y0 as i32,
            (x1 - x0) as u32,
            (y1 - y0) as u32,
        ))
    }
}

/// One monitor's OS-space bounds and the pixel size of its frozen image.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub pixel_width: u32,
    pub pixel_height: u32,
}

impl MonitorGeometry {
    fn scale(&self) -> (f64, f64) {
        (
            self.pixel_width as f64 / self.width.max(1) as f64,
            self.pixel_height as f64 / self.height.max(1) as f64,
        )
    }

    /// Map an OS-space rect (e.g. a window) into this monitor's pixel space,
    /// clipped to the monitor. Returns `None` if they don't overlap.
    pub fn to_local_pixels(&self, os_rect: PixelRect) -> Option<PixelRect> {
        let monitor = PixelRect::new(self.x, self.y, self.width, self.height);
        let clipped = os_rect.intersect(&monitor)?;
        let (sx, sy) = self.scale();
        let lx0 = ((clipped.x as i64 - self.x as i64) as f64 * sx).floor();
        let ly0 = ((clipped.y as i64 - self.y as i64) as f64 * sy).floor();
        let lx1 = ((clipped.right() - self.x as i64) as f64 * sx)
            .ceil()
            .min(self.pixel_width as f64);
        let ly1 = ((clipped.bottom() - self.y as i64) as f64 * sy)
            .ceil()
            .min(self.pixel_height as f64);
        if lx1 <= lx0 || ly1 <= ly0 {
            return None;
        }
        Some(PixelRect::new(
            lx0 as i32,
            ly0 as i32,
            (lx1 - lx0) as u32,
            (ly1 - ly0) as u32,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_rejects_zero_area() {
        assert!(validate_crop(PixelRect::new(10, 10, 0, 5), 100, 100).is_err());
        assert!(validate_crop(PixelRect::new(10, 10, 5, 0), 100, 100).is_err());
    }

    #[test]
    fn crop_clamps_overshoot() {
        let r = validate_crop(PixelRect::new(90, -2, 20, 10), 100, 100).unwrap();
        assert_eq!(r, PixelRect::new(90, 0, 10, 8));
    }

    #[test]
    fn crop_rejects_fully_outside() {
        assert!(validate_crop(PixelRect::new(200, 200, 10, 10), 100, 100).is_err());
        assert!(validate_crop(PixelRect::new(-50, 0, 50, 10), 100, 100).is_err());
    }

    #[test]
    fn crop_exact_is_unchanged() {
        let r = PixelRect::new(12, 34, 56, 78);
        assert_eq!(validate_crop(r, 1920, 1080).unwrap(), r);
    }

    #[test]
    fn covering_pixels_rounds_outward() {
        let r = FRect {
            x: 10.4,
            y: 5.6,
            width: 3.2,
            height: 1.1,
        };
        // 10.4..13.6 -> 10..14 ; 5.6..6.7 -> 5..7
        assert_eq!(
            r.covering_pixels(100, 100),
            Some(PixelRect::new(10, 5, 4, 2))
        );
    }

    #[test]
    fn covering_pixels_normalizes_negative_drag() {
        let r = FRect {
            x: 20.0,
            y: 20.0,
            width: -10.0,
            height: -5.0,
        };
        assert_eq!(
            r.covering_pixels(100, 100),
            Some(PixelRect::new(10, 15, 10, 5))
        );
    }

    #[test]
    fn covering_pixels_rejects_nan_and_outside() {
        assert_eq!(
            FRect {
                x: f32::NAN,
                y: 0.0,
                width: 1.0,
                height: 1.0
            }
            .covering_pixels(10, 10),
            None
        );
        assert_eq!(
            FRect {
                x: 20.0,
                y: 0.0,
                width: 1.0,
                height: 1.0
            }
            .covering_pixels(10, 10),
            None
        );
    }

    /// Windows: OS space == physical pixels. A monitor left of the primary
    /// has negative coordinates.
    #[test]
    fn window_on_negative_monitor_maps_1_to_1() {
        let m = MonitorGeometry {
            x: -1920,
            y: 0,
            width: 1920,
            height: 1080,
            pixel_width: 1920,
            pixel_height: 1080,
        };
        let w = PixelRect::new(-1800, 100, 800, 600);
        assert_eq!(
            m.to_local_pixels(w),
            Some(PixelRect::new(120, 100, 800, 600))
        );
    }

    /// macOS Retina: OS space is points; image is 2x.
    #[test]
    fn window_on_retina_monitor_scales() {
        let m = MonitorGeometry {
            x: 0,
            y: 0,
            width: 1440,
            height: 900,
            pixel_width: 2880,
            pixel_height: 1800,
        };
        let w = PixelRect::new(100, 50, 400, 300);
        assert_eq!(
            m.to_local_pixels(w),
            Some(PixelRect::new(200, 100, 800, 600))
        );
    }

    /// Mixed DPI: secondary 4K display at 150% on Windows still reports physical
    /// coordinates; a window straddling both monitors is clipped per monitor.
    #[test]
    fn window_straddling_monitors_is_clipped() {
        let primary = MonitorGeometry {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            pixel_width: 1920,
            pixel_height: 1080,
        };
        let right = MonitorGeometry {
            x: 1920,
            y: 0,
            width: 3840,
            height: 2160,
            pixel_width: 3840,
            pixel_height: 2160,
        };
        let w = PixelRect::new(1800, 200, 400, 300);
        assert_eq!(
            primary.to_local_pixels(w),
            Some(PixelRect::new(1800, 200, 120, 300))
        );
        assert_eq!(
            right.to_local_pixels(w),
            Some(PixelRect::new(0, 200, 280, 300))
        );
    }

    #[test]
    fn non_overlapping_window_is_none() {
        let m = MonitorGeometry {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
            pixel_width: 100,
            pixel_height: 100,
        };
        assert_eq!(m.to_local_pixels(PixelRect::new(100, 0, 10, 10)), None);
    }
}
