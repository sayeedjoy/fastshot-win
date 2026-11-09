//! Text rasterisation with the bundled annotation font. The UI preview uses the
//! same font file and the same layout constants (see `src/features/editor/
//! geometry.ts`), with kerning and ligatures disabled, so glyphs land in the
//! same place.

use std::sync::OnceLock;

use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};
use tiny_skia::Pixmap;

use super::model::Color;

static FONT_BYTES: &[u8] = include_bytes!("../../assets/AnnotationFont.ttf");

/// Line height as a multiple of font size. Keep in sync with `TEXT_LINE_HEIGHT` in TS.
pub const LINE_HEIGHT: f32 = 1.25;

pub fn font() -> &'static FontRef<'static> {
    static FONT: OnceLock<FontRef<'static>> = OnceLock::new();
    FONT.get_or_init(|| FontRef::try_from_slice(FONT_BYTES).expect("bundled font is valid"))
}

/// Ascender as a fraction of the font size. Keep in sync with `TEXT_ASCENT` in TS.
pub fn ascent_ratio() -> f32 {
    let f = font();
    f.ascent_unscaled() / f.units_per_em().unwrap_or(1000.0)
}

/// Advance width of a single line (no kerning), matching the UI's
/// `measureText`.
pub fn line_width(line: &str, size: f32) -> f32 {
    let font = font();
    let scale =
        PxScale::from(size * font.height_unscaled() / font.units_per_em().unwrap_or(1000.0));
    let scaled = font.as_scaled(scale);
    line.chars()
        .map(|c| scaled.h_advance(font.glyph_id(c)))
        .sum()
}

/// Draw `text` with its first line's top at `(x, y)`; each line's baseline is
/// `y + ascent + i * LINE_HEIGHT * size`. Pixmap pixels are assumed opaque.
pub fn draw_text(pixmap: &mut Pixmap, x: f32, y: f32, text: &str, size: f32, color: Color) {
    let font = font();
    let scale =
        PxScale::from(size * font.height_unscaled() / font.units_per_em().unwrap_or(1000.0));
    let scaled = font.as_scaled(scale);
    let ascent = ascent_ratio() * size;
    let (pw, ph) = (pixmap.width() as i32, pixmap.height() as i32);
    let data = pixmap.data_mut();
    let alpha = color.a as f32 / 255.0;

    for (i, line) in text.split('\n').enumerate() {
        let baseline = y + ascent + i as f32 * LINE_HEIGHT * size;
        let mut caret = x;
        for ch in line.chars() {
            let id = font.glyph_id(ch);
            let glyph = id.with_scale_and_position(scale, point(caret, baseline));
            caret += scaled.h_advance(id);
            let Some(outlined) = font.outline_glyph(glyph) else {
                continue;
            };
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, coverage| {
                let px = bounds.min.x as i32 + gx as i32;
                let py = bounds.min.y as i32 + gy as i32;
                if px < 0 || py < 0 || px >= pw || py >= ph {
                    return;
                }
                let a = (coverage.clamp(0.0, 1.0) * alpha).clamp(0.0, 1.0);
                if a <= 0.0 {
                    return;
                }
                let idx = (py as usize * pw as usize + px as usize) * 4;
                let blend = |dst: u8, src: u8| -> u8 {
                    (src as f32 * a + dst as f32 * (1.0 - a)).round() as u8
                };
                data[idx] = blend(data[idx], color.r);
                data[idx + 1] = blend(data[idx + 1], color.g);
                data[idx + 2] = blend(data[idx + 2], color.b);
                data[idx + 3] = 255;
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_loads_and_reports_metrics() {
        let r = ascent_ratio();
        // Printed so the TS constant can be checked against it: `cargo test -- --nocapture`.
        println!("TEXT_ASCENT = {r}");
        assert!(r > 0.5 && r < 1.5);
    }

    #[test]
    fn draws_some_pixels() {
        let mut pm = Pixmap::new(200, 60).unwrap();
        pm.fill(tiny_skia::Color::WHITE);
        draw_text(&mut pm, 4.0, 4.0, "Hello", 32.0, Color::BLACK);
        let dark = pm.data().chunks(4).filter(|p| p[0] < 128).count();
        assert!(dark > 50, "expected glyph pixels, got {dark}");
    }
}
