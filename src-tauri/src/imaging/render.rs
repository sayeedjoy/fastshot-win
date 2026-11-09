//! The single export compositor used by **both** clipboard and file output
//! (PRD SEC-04). Order of operations:
//!
//! 1. Copy the crop into a fresh, fully opaque buffer.
//! 2. Cosmetic effects (blur / pixelate).
//! 3. Annotations, in the order given (array order == z-order).
//! 4. Secure redactions — always last, so nothing can be drawn over them and
//!    no layer can reveal what was underneath (PRD SEC-01, §9.2).

use image::RgbaImage;
use tiny_skia::{FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

use super::effects;
use super::model::{Annotation, Color, EffectKind, RenderRequest};
use super::text;
use crate::geometry::validate_crop;

/// Arrow-head geometry shared with the UI (`arrowGeometry` in TS).
/// `(shaft_end, head_left, tip, head_right)`.
pub type ArrowGeometry = ([f32; 2], [f32; 2], [f32; 2], [f32; 2]);

pub fn arrow_head(x1: f32, y1: f32, x2: f32, y2: f32, stroke: f32) -> Option<ArrowGeometry> {
    let (dx, dy) = (x2 - x1, y2 - y1);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 0.5 {
        return None;
    }
    let (ux, uy) = (dx / len, dy / len);
    let head_len = (stroke * 4.0).max(12.0).min(len);
    let half_w = head_len * 0.45;
    let base = [x2 - ux * head_len, y2 - uy * head_len];
    let left = [base[0] - uy * half_w, base[1] + ux * half_w];
    let right = [base[0] + uy * half_w, base[1] - ux * half_w];
    // Shaft ends inside the head so the round cap never pokes out of the tip.
    let shaft_end = [x2 - ux * head_len * 0.6, y2 - uy * head_len * 0.6];
    Some((shaft_end, left, [x2, y2], right))
}

/// Highlighter opacity. Keep in sync with `HIGHLIGHT_ALPHA` in TS.
pub const HIGHLIGHT_ALPHA: f32 = 0.4;
/// Step label baseline sits this many font-sizes below the disc centre
/// (half the digit height). Keep in sync with `STEP_BASELINE` in TS.
pub const STEP_BASELINE: f32 = 0.36;

/// Font size for a step label; more digits get a smaller size so they fit.
/// Mirrors `stepFontSize` in TS.
pub fn step_font_size(radius: f32, number: u32) -> f32 {
    radius
        * match number {
            0..=9 => 1.15,
            10..=99 => 0.95,
            _ => 0.72,
        }
}

/// Black label on light discs, white otherwise. Mirrors `stepLabelColor` in TS.
pub fn step_label_color(fill: Color) -> Color {
    let lum = (0.299 * fill.r as f32 + 0.587 * fill.g as f32 + 0.114 * fill.b as f32) / 255.0;
    if lum > 0.6 {
        Color::BLACK
    } else {
        Color {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        }
    }
}

fn polyline(points: &[[f32; 2]]) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    match points {
        [] => return None,
        [p] => {
            // A click draws a dot.
            pb.move_to(p[0], p[1]);
            pb.line_to(p[0] + 0.01, p[1]);
        }
        [first, rest @ ..] => {
            pb.move_to(first[0], first[1]);
            for p in rest {
                pb.line_to(p[0], p[1]);
            }
        }
    }
    pb.finish()
}

fn paint(color: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(color.r, color.g, color.b, color.a);
    p.anti_alias = true;
    p
}

fn stroke(width: f32) -> Stroke {
    Stroke {
        width,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Stroke::default()
    }
}

fn rect_of(x: f32, y: f32, w: f32, h: f32) -> Option<Rect> {
    let (x, w) = if w < 0.0 { (x + w, -w) } else { (x, w) };
    let (y, h) = if h < 0.0 { (y + h, -h) } else { (y, h) };
    Rect::from_xywh(x, y, w.max(0.01), h.max(0.01))
}

fn draw_annotation(pm: &mut Pixmap, a: &Annotation) {
    let id = Transform::identity();
    match a {
        Annotation::Rect {
            x,
            y,
            width,
            height,
            color,
            stroke_width,
        } => {
            if let Some(r) = rect_of(*x, *y, *width, *height) {
                let path = PathBuilder::from_rect(r);
                pm.stroke_path(
                    &path,
                    &paint(*color),
                    &Stroke {
                        line_join: LineJoin::Miter,
                        ..stroke(*stroke_width)
                    },
                    id,
                    None,
                );
            }
        }
        Annotation::Ellipse {
            x,
            y,
            width,
            height,
            color,
            stroke_width,
        } => {
            if let Some(path) = rect_of(*x, *y, *width, *height).and_then(PathBuilder::from_oval) {
                pm.stroke_path(&path, &paint(*color), &stroke(*stroke_width), id, None);
            }
        }
        Annotation::Line {
            x1,
            y1,
            x2,
            y2,
            color,
            stroke_width,
        } => {
            let mut pb = PathBuilder::new();
            pb.move_to(*x1, *y1);
            pb.line_to(*x2, *y2);
            if let Some(path) = pb.finish() {
                pm.stroke_path(&path, &paint(*color), &stroke(*stroke_width), id, None);
            }
        }
        Annotation::Arrow {
            x1,
            y1,
            x2,
            y2,
            color,
            stroke_width,
        } => {
            let Some((shaft_end, left, tip, right)) = arrow_head(*x1, *y1, *x2, *y2, *stroke_width)
            else {
                return;
            };
            let mut pb = PathBuilder::new();
            pb.move_to(*x1, *y1);
            pb.line_to(shaft_end[0], shaft_end[1]);
            if let Some(path) = pb.finish() {
                pm.stroke_path(&path, &paint(*color), &stroke(*stroke_width), id, None);
            }
            let mut hb = PathBuilder::new();
            hb.move_to(left[0], left[1]);
            hb.line_to(tip[0], tip[1]);
            hb.line_to(right[0], right[1]);
            hb.close();
            if let Some(path) = hb.finish() {
                pm.fill_path(&path, &paint(*color), FillRule::Winding, id, None);
            }
        }
        Annotation::Pen {
            points,
            color,
            stroke_width,
        } => {
            if let Some(path) = polyline(points) {
                pm.stroke_path(&path, &paint(*color), &stroke(*stroke_width), id, None);
            }
        }
        Annotation::Highlight {
            points,
            color,
            stroke_width,
        } => {
            // One stroke path, so overlapping segments don't stack alpha
            // (same as the SVG preview's single polyline).
            let c = Color {
                a: (HIGHLIGHT_ALPHA * 255.0).round() as u8,
                ..color.opaque()
            };
            if let Some(path) = polyline(points) {
                pm.stroke_path(&path, &paint(c), &stroke(*stroke_width), id, None);
            }
        }
        Annotation::Step {
            x,
            y,
            radius,
            number,
            color,
        } => {
            let fill = color.opaque();
            if let Some(path) = PathBuilder::from_circle(*x, *y, *radius) {
                pm.fill_path(&path, &paint(fill), FillRule::Winding, id, None);
            }
            let label = number.to_string();
            let size = step_font_size(*radius, *number);
            let left = x - text::line_width(&label, size) / 2.0;
            let top = y + STEP_BASELINE * size - text::ascent_ratio() * size;
            text::draw_text(pm, left, top, &label, size, step_label_color(fill));
        }
        Annotation::Text {
            x,
            y,
            text,
            color,
            font_size,
        } => {
            text::draw_text(pm, *x, *y, text, *font_size, *color);
        }
    }
}

/// Render the final, sanitised image. `base` is the frozen monitor image.
pub fn render_export(base: &RgbaImage, req: &RenderRequest) -> Result<RgbaImage, String> {
    req.validate()?;
    let crop = validate_crop(req.crop, base.width(), base.height())?;
    let (w, h) = (crop.width, crop.height);

    // 1. Fresh buffer: copied pixel-by-pixel from the crop, alpha forced opaque.
    let mut out = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let mut p = *base.get_pixel(crop.x as u32 + x, crop.y as u32 + y);
            p.0[3] = 255;
            out.put_pixel(x, y, p);
        }
    }

    // 2. Effects.
    for e in &req.effects {
        let Some(r) = e.rect.covering_pixels(w, h) else {
            continue;
        };
        match e.kind {
            EffectKind::Blur => effects::blur(&mut out, r, e.strength),
            EffectKind::Pixelate => effects::pixelate(&mut out, r, e.strength.round() as u32),
        }
    }

    // 3. Annotations. All pixels are opaque, so premultiplied == straight RGBA
    //    and the buffer can be handed to tiny-skia without conversion.
    if !req.annotations.is_empty() {
        let size = tiny_skia::IntSize::from_wh(w, h).ok_or("Invalid image size")?;
        let mut pm = Pixmap::from_vec(out.into_raw(), size).ok_or("Invalid image buffer")?;
        for a in &req.annotations {
            draw_annotation(&mut pm, a);
        }
        out = RgbaImage::from_raw(w, h, pm.take()).ok_or("Invalid image buffer")?;
    }

    // 4. Secure redaction — final pass, opaque, rounded outward.
    for r in &req.redactions {
        let Some(px) = r.rect.covering_pixels(w, h) else {
            continue;
        };
        let c = r.color.opaque();
        let fill = image::Rgba([c.r, c.g, c.b, 255]);
        for y in px.y as u32..px.y as u32 + px.height {
            for x in px.x as u32..px.x as u32 + px.width {
                out.put_pixel(x, y, fill);
            }
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{FRect, PixelRect};
    use crate::imaging::model::{Effect, Redaction};

    fn grey(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_pixel(w, h, image::Rgba([128, 128, 128, 255]))
    }

    fn req(crop: PixelRect) -> RenderRequest {
        RenderRequest {
            crop,
            annotations: vec![],
            effects: vec![],
            redactions: vec![],
        }
    }

    #[test]
    fn output_matches_crop_size() {
        let out = render_export(&grey(100, 80), &req(PixelRect::new(10, 20, 30, 40))).unwrap();
        assert_eq!(out.dimensions(), (30, 40));
    }

    #[test]
    fn crop_copies_exact_pixels() {
        let base = RgbaImage::from_fn(50, 50, |x, y| image::Rgba([x as u8, y as u8, 7, 255]));
        let out = render_export(&base, &req(PixelRect::new(5, 6, 10, 10))).unwrap();
        assert_eq!(out.get_pixel(0, 0).0, [5, 6, 7, 255]);
        assert_eq!(out.get_pixel(9, 9).0, [14, 15, 7, 255]);
    }

    #[test]
    fn transparent_source_becomes_opaque() {
        let base = RgbaImage::from_pixel(4, 4, image::Rgba([10, 20, 30, 0]));
        let out = render_export(&base, &req(PixelRect::new(0, 0, 4, 4))).unwrap();
        assert!(out.pixels().all(|p| p.0[3] == 255));
    }

    #[test]
    fn annotation_draws_inside_crop_coordinates() {
        let mut r = req(PixelRect::new(50, 50, 40, 40));
        r.annotations.push(Annotation::Rect {
            x: 10.0,
            y: 10.0,
            width: 20.0,
            height: 20.0,
            color: Color::parse("#ff0000").unwrap(),
            stroke_width: 2.0,
        });
        let out = render_export(&grey(200, 200), &r).unwrap();
        assert_eq!(
            out.get_pixel(10, 20).0,
            [255, 0, 0, 255],
            "left edge of the rect stroke"
        );
        assert_eq!(
            out.get_pixel(20, 20).0,
            [128, 128, 128, 255],
            "rect interior untouched"
        );
    }

    #[test]
    fn redaction_wins_over_annotations_and_effects() {
        let mut r = req(PixelRect::new(0, 0, 60, 60));
        let white = Color::parse("#ffffff").unwrap();
        r.effects.push(Effect {
            kind: EffectKind::Blur,
            rect: FRect {
                x: 0.0,
                y: 0.0,
                width: 60.0,
                height: 60.0,
            },
            strength: 4.0,
        });
        r.annotations.push(Annotation::Line {
            x1: 0.0,
            y1: 30.0,
            x2: 60.0,
            y2: 30.0,
            color: white,
            stroke_width: 20.0,
        });
        r.annotations.push(Annotation::Text {
            x: 10.0,
            y: 10.0,
            text: "SECRET".into(),
            color: white,
            font_size: 30.0,
        });
        r.redactions.push(Redaction {
            rect: FRect {
                x: 10.0,
                y: 10.0,
                width: 40.0,
                height: 40.0,
            },
            color: Color::BLACK,
        });
        let out = render_export(&grey(60, 60), &r).unwrap();
        for y in 10..50 {
            for x in 10..50 {
                assert_eq!(
                    out.get_pixel(x, y).0,
                    [0, 0, 0, 255],
                    "pixel ({x},{y}) must be fully redacted"
                );
            }
        }
    }

    #[test]
    fn redaction_ignores_alpha_in_requested_color() {
        let mut r = req(PixelRect::new(0, 0, 10, 10));
        r.redactions.push(Redaction {
            rect: FRect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            color: Color::parse("#ff000000").unwrap(),
        });
        let out = render_export(&grey(10, 10), &r).unwrap();
        assert!(out.pixels().all(|p| p.0 == [255, 0, 0, 255]));
    }

    #[test]
    fn redaction_rounds_outward_on_fractional_rects() {
        let mut r = req(PixelRect::new(0, 0, 20, 20));
        r.redactions.push(Redaction {
            rect: FRect {
                x: 4.7,
                y: 4.7,
                width: 2.6,
                height: 2.6,
            },
            color: Color::BLACK,
        });
        let out = render_export(&grey(20, 20), &r).unwrap();
        for (x, y) in [(4, 4), (7, 7), (4, 7), (7, 4)] {
            assert_eq!(out.get_pixel(x, y).0, [0, 0, 0, 255]);
        }
        assert_eq!(out.get_pixel(3, 3).0, [128, 128, 128, 255]);
        assert_eq!(out.get_pixel(8, 8).0, [128, 128, 128, 255]);
    }

    #[test]
    fn rejects_invalid_crop_and_ops() {
        assert!(render_export(&grey(10, 10), &req(PixelRect::new(0, 0, 0, 0))).is_err());
        let mut r = req(PixelRect::new(0, 0, 10, 10));
        r.effects.push(Effect {
            kind: EffectKind::Pixelate,
            rect: FRect {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            strength: 0.0,
        });
        assert!(render_export(&grey(10, 10), &r).is_err());
    }

    #[test]
    fn step_marker_fills_disc_with_contrasting_label() {
        let mut r = req(PixelRect::new(0, 0, 80, 80));
        r.annotations.push(Annotation::Step {
            x: 40.0,
            y: 40.0,
            radius: 20.0,
            number: 7,
            color: Color::parse("#000000").unwrap(),
        });
        let out = render_export(&grey(80, 80), &r).unwrap();
        // Inside the disc but off the glyph: fill colour.
        assert_eq!(out.get_pixel(40, 24).0, [0, 0, 0, 255]);
        // Outside the disc: untouched.
        assert_eq!(out.get_pixel(5, 5).0, [128, 128, 128, 255]);
        // The label adds white pixels near the centre.
        let white = (30..50)
            .flat_map(|y| (30..50).map(move |x| (x, y)))
            .filter(|&(x, y)| out.get_pixel(x, y).0[0] > 200)
            .count();
        assert!(white > 10, "expected label pixels, got {white}");
    }

    #[test]
    fn step_label_contrast() {
        assert_eq!(
            step_label_color(Color::parse("#facc15").unwrap()),
            Color::BLACK
        );
        assert_eq!(step_label_color(Color::parse("#e11d48").unwrap()).r, 255);
    }

    #[test]
    fn highlighter_is_translucent() {
        let mut r = req(PixelRect::new(0, 0, 40, 40));
        r.annotations.push(Annotation::Highlight {
            points: vec![[0.0, 20.0], [40.0, 20.0], [0.0, 20.0]],
            color: Color::parse("#ffff00").unwrap(),
            stroke_width: 10.0,
        });
        let out = render_export(
            &RgbaImage::from_pixel(40, 40, image::Rgba([0, 0, 0, 255])),
            &r,
        )
        .unwrap();
        let p = out.get_pixel(20, 20).0;
        // 40 % yellow over black, and overlapping segments don't double up.
        assert!(
            (95..=110).contains(&p[0]) && p[0] == p[1] && p[2] == 0,
            "got {p:?}"
        );
        assert_eq!(out.get_pixel(20, 2).0, [0, 0, 0, 255]);
    }

    #[test]
    fn redaction_covers_step_markers_and_highlights() {
        let mut r = req(PixelRect::new(0, 0, 40, 40));
        let white = Color::parse("#ffffff").unwrap();
        r.annotations.push(Annotation::Step {
            x: 20.0,
            y: 20.0,
            radius: 15.0,
            number: 1,
            color: white,
        });
        r.annotations.push(Annotation::Highlight {
            points: vec![[0.0, 20.0], [40.0, 20.0]],
            color: white,
            stroke_width: 30.0,
        });
        r.redactions.push(Redaction {
            rect: FRect {
                x: 0.0,
                y: 0.0,
                width: 40.0,
                height: 40.0,
            },
            color: Color::BLACK,
        });
        let out = render_export(&grey(40, 40), &r).unwrap();
        assert!(out.pixels().all(|p| p.0 == [0, 0, 0, 255]));
    }

    #[test]
    fn arrow_head_points_at_tip() {
        let (_, l, tip, rr) = arrow_head(0.0, 0.0, 100.0, 0.0, 4.0).unwrap();
        assert_eq!(tip, [100.0, 0.0]);
        assert!((l[0] - 84.0).abs() < 1e-3 && (rr[0] - 84.0).abs() < 1e-3);
        assert!(arrow_head(1.0, 1.0, 1.0, 1.0, 4.0).is_none());
    }
}
