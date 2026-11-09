//! Fresh encoders. The output contains only the pixels we pass in: no EXIF,
//! text chunks, thumbnails, colour profiles, or source payloads (PRD §9.2).

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder, RgbImage, RgbaImage};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    #[default]
    Png,
    Jpeg,
}

impl ImageFormat {
    pub fn extension(self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Jpeg => "jpg",
        }
    }

    pub fn from_extension(ext: &str) -> Option<ImageFormat> {
        match ext.to_ascii_lowercase().as_str() {
            "png" => Some(ImageFormat::Png),
            "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
            _ => None,
        }
    }
}

/// Composite onto a solid background (white), dropping alpha. Exported
/// screenshots are always opaque; this defines JPEG's transparency behaviour.
fn flatten(img: &RgbaImage) -> RgbImage {
    RgbImage::from_fn(img.width(), img.height(), |x, y| {
        let [r, g, b, a] = img.get_pixel(x, y).0;
        let a = a as u16;
        let mix = |c: u8| ((c as u16 * a + 255 * (255 - a)) / 255) as u8;
        image::Rgb([mix(r), mix(g), mix(b)])
    })
}

pub fn encode(img: &RgbaImage, format: ImageFormat, jpeg_quality: u8) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    match format {
        // The compositor already forces every export pixel opaque. Keeping the
        // RGBA representation avoids another full-size RGB allocation and copy.
        ImageFormat::Png => {
            PngEncoder::new_with_quality(&mut buf, CompressionType::Default, FilterType::Adaptive)
                .write_image(
                    img.as_raw(),
                    img.width(),
                    img.height(),
                    ExtendedColorType::Rgba8,
                )
        }
        ImageFormat::Jpeg => {
            let rgb = flatten(img);
            JpegEncoder::new_with_quality(&mut buf, jpeg_quality.clamp(1, 100)).write_image(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                ExtendedColorType::Rgb8,
            )
        }
    }
    .map_err(|e| e.to_string())?;
    Ok(buf)
}

/// Fast PNG for the in-memory overlay preview. Never written to disk.
pub fn encode_preview(img: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut buf = Vec::with_capacity(img.as_raw().len() / 2);
    PngEncoder::new_with_quality(&mut buf, CompressionType::Fast, FilterType::Sub)
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(|e| e.to_string())?;
    Ok(buf)
}

/// List the chunk types of a PNG stream (used by tests and self-checks).
pub fn png_chunk_types(bytes: &[u8]) -> Result<Vec<[u8; 4]>, String> {
    const SIG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if bytes.len() < 8 || bytes[..8] != SIG {
        return Err("not a PNG".into());
    }
    let mut out = Vec::new();
    let mut i = 8;
    while i + 8 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
        let ty: [u8; 4] = bytes[i + 4..i + 8].try_into().unwrap();
        out.push(ty);
        i += 12 + len;
    }
    Ok(out)
}

/// List JPEG marker bytes up to start-of-scan.
pub fn jpeg_markers(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return Err("not a JPEG".into());
    }
    let mut out = Vec::new();
    let mut i = 2;
    while i + 4 <= bytes.len() {
        if bytes[i] != 0xFF {
            return Err("corrupt JPEG marker".into());
        }
        let m = bytes[i + 1];
        out.push(m);
        if m == 0xDA {
            break;
        }
        let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
        i += 2 + len;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_has_only_critical_chunks() {
        let img = RgbaImage::from_pixel(16, 9, image::Rgba([1, 2, 3, 255]));
        let png = encode(&img, ImageFormat::Png, 90).unwrap();
        for ty in png_chunk_types(&png).unwrap() {
            assert!(
                [*b"IHDR", *b"IDAT", *b"IEND"].contains(&ty),
                "unexpected chunk {}",
                String::from_utf8_lossy(&ty)
            );
        }
        let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(decoded.dimensions(), (16, 9));
        assert_eq!(decoded.get_pixel(0, 0).0, [1, 2, 3, 255]);
    }

    #[test]
    fn jpeg_has_no_exif_or_comments() {
        let img = RgbaImage::from_pixel(32, 32, image::Rgba([200, 100, 50, 255]));
        let jpg = encode(&img, ImageFormat::Jpeg, 85).unwrap();
        let markers = jpeg_markers(&jpg).unwrap();
        assert!(!markers.contains(&0xE1), "APP1/EXIF must not be present");
        assert!(!markers.contains(&0xFE), "COM must not be present");
        let decoded = image::load_from_memory(&jpg).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (32, 32));
    }

    #[test]
    fn jpeg_flattens_transparency_onto_white() {
        let img = RgbaImage::from_pixel(8, 8, image::Rgba([0, 0, 0, 0]));
        let jpg = encode(&img, ImageFormat::Jpeg, 95).unwrap();
        let p = image::load_from_memory(&jpg)
            .unwrap()
            .to_rgb8()
            .get_pixel(4, 4)
            .0;
        assert!(p.iter().all(|c| *c > 245), "expected white, got {p:?}");
    }

    #[test]
    fn format_from_extension() {
        assert_eq!(ImageFormat::from_extension("JPEG"), Some(ImageFormat::Jpeg));
        assert_eq!(ImageFormat::from_extension("png"), Some(ImageFormat::Png));
        assert_eq!(ImageFormat::from_extension("gif"), None);
    }
}
