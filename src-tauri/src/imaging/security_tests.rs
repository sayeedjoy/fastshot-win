//! Redaction security regression fixtures (PRD §11.1 #8–#10, §11.2).
//! A failure here is a release blocker.

use image::RgbaImage;

use super::encode::{ImageFormat, encode, png_chunk_types};
use super::model::{Annotation, Color, Effect, EffectKind, Redaction, RenderRequest};
use super::render::render_export;
use crate::geometry::{FRect, PixelRect};

/// High-contrast "secret": a pseudo-random black/white noise block with a
/// unique byte signature, placed at (40,40)-(120,72) of a 200×120 image.
fn secret_fixture() -> (RgbaImage, PixelRect) {
    let secret = PixelRect::new(40, 40, 80, 32);
    let mut seed: u32 = 0xC0FF_EE11;
    let img = RgbaImage::from_fn(200, 120, |x, y| {
        let inside = (40..120).contains(&x) && (40..72).contains(&y);
        if inside {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            // Distinct colours that never appear elsewhere in the fixture.
            let v = (seed & 0xFF) as u8;
            image::Rgba([v, 255 - v, (seed >> 8) as u8 | 1, 255])
        } else {
            image::Rgba([240, 240, 240, 255])
        }
    });
    (img, secret)
}

fn redaction_request(crop: PixelRect, secret_in_crop: FRect) -> RenderRequest {
    RenderRequest {
        crop,
        annotations: vec![],
        effects: vec![],
        redactions: vec![Redaction {
            rect: secret_in_crop,
            color: Color::BLACK,
        }],
    }
}

fn secret_pixels(img: &RgbaImage, r: PixelRect) -> Vec<[u8; 4]> {
    let mut v = Vec::new();
    for y in r.y as u32..r.y as u32 + r.height {
        for x in r.x as u32..r.x as u32 + r.width {
            v.push(img.get_pixel(x, y).0);
        }
    }
    v
}

#[test]
fn redacted_region_decodes_to_fill_only_png() {
    let (base, secret) = secret_fixture();
    let crop = PixelRect::new(20, 20, 160, 80);
    let rect = FRect {
        x: 20.0,
        y: 20.0,
        width: 80.0,
        height: 32.0,
    };
    let out = render_export(&base, &redaction_request(crop, rect)).unwrap();
    let png = encode(&out, ImageFormat::Png, 90).unwrap();
    let decoded = image::load_from_memory(&png).unwrap().to_rgba8();

    for y in 20..52 {
        for x in 20..100 {
            assert_eq!(
                decoded.get_pixel(x, y).0,
                [0, 0, 0, 255],
                "secret visible at ({x},{y})"
            );
        }
    }
    // No secret colour may survive anywhere in the output.
    let secret_colors = secret_pixels(&base, secret);
    for p in decoded.pixels() {
        if p.0 != [0, 0, 0, 255] {
            assert!(
                !secret_colors.contains(&p.0),
                "secret colour {:?} leaked",
                p.0
            );
        }
    }
}

#[test]
fn redacted_file_bytes_contain_no_source_payload() {
    let (base, secret) = secret_fixture();
    let crop = PixelRect::new(0, 0, 200, 120);
    let rect = FRect {
        x: 40.0,
        y: 40.0,
        width: 80.0,
        height: 32.0,
    };
    let out = render_export(&base, &redaction_request(crop, rect)).unwrap();

    for fmt in [ImageFormat::Png, ImageFormat::Jpeg] {
        let bytes = encode(&out, fmt, 95).unwrap();
        // Raw secret rows must not appear verbatim (e.g. a stored source layer).
        let row: Vec<u8> =
            secret_pixels(&base, PixelRect::new(secret.x, secret.y, secret.width, 1))
                .iter()
                .flat_map(|p| [p[0], p[1], p[2]])
                .collect();
        assert!(
            !bytes.windows(24).any(|w| row.windows(24).any(|r| r == w)),
            "{fmt:?} output contains source bytes"
        );
        if fmt == ImageFormat::Png {
            let chunks = png_chunk_types(&bytes).unwrap();
            assert!(
                chunks
                    .iter()
                    .all(|c| [*b"IHDR", *b"IDAT", *b"IEND"].contains(c))
            );
        }
    }
}

#[test]
fn layer_ordering_cannot_reveal_redacted_content() {
    // An annotation that tries to paint the secret back on top (a bright
    // stroke and text) plus a blur under it — the redaction must still win.
    let (base, _) = secret_fixture();
    let crop = PixelRect::new(0, 0, 200, 120);
    let rect = FRect {
        x: 40.0,
        y: 40.0,
        width: 80.0,
        height: 32.0,
    };
    let mut req = redaction_request(crop, rect);
    req.effects.push(Effect {
        kind: EffectKind::Pixelate,
        rect,
        strength: 4.0,
    });
    req.annotations.push(Annotation::Rect {
        x: 30.0,
        y: 30.0,
        width: 100.0,
        height: 52.0,
        color: Color::parse("#ff00ff").unwrap(),
        stroke_width: 30.0,
    });
    req.annotations.push(Annotation::Text {
        x: 40.0,
        y: 40.0,
        text: "LEAK".into(),
        color: Color::parse("#00ff00").unwrap(),
        font_size: 30.0,
    });
    let out = render_export(&base, &req).unwrap();
    for y in 40..72 {
        for x in 40..120 {
            assert_eq!(out.get_pixel(x, y).0, [0, 0, 0, 255]);
        }
    }
}

#[test]
fn clipboard_and_file_share_pixels() {
    // Clipboard receives `render_export` RGBA directly; file receives the
    // encoded PNG of the same buffer. Decoding the PNG must give identical RGB.
    let (base, _) = secret_fixture();
    let crop = PixelRect::new(10, 10, 150, 90);
    let mut req = redaction_request(
        crop,
        FRect {
            x: 30.0,
            y: 30.0,
            width: 80.0,
            height: 32.0,
        },
    );
    req.annotations.push(Annotation::Arrow {
        x1: 0.0,
        y1: 0.0,
        x2: 100.0,
        y2: 80.0,
        color: Color::parse("#e11d48").unwrap(),
        stroke_width: 5.0,
    });
    let clipboard = render_export(&base, &req).unwrap();
    let file = render_export(&base, &req).unwrap();
    let png = encode(&file, ImageFormat::Png, 90).unwrap();
    let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
    assert_eq!(clipboard.as_raw(), decoded.as_raw());
}

#[test]
fn source_image_is_not_modified() {
    let (base, _) = secret_fixture();
    let before = base.clone();
    let _ = render_export(
        &base,
        &redaction_request(
            PixelRect::new(0, 0, 200, 120),
            FRect {
                x: 0.0,
                y: 0.0,
                width: 200.0,
                height: 120.0,
            },
        ),
    )
    .unwrap();
    assert_eq!(base.as_raw(), before.as_raw());
}
