//! Windows backend: `Windows.Media.Ocr` (WinRT, Windows 10+). Uses the OCR
//! languages installed for the user's profile languages.

use image::RgbaImage;
use image::imageops::{self, FilterType};
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::DataWriter;
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};

use super::TextBox;

/// Screen text is small; the engine reads it much more reliably at 2x.
const UPSCALE_BELOW_PIXELS: u64 = 2_000_000;

pub fn recognize(image: &RgbaImage) -> Result<Vec<TextBox>, String> {
    // Called on a blocking-pool thread. Joining the MTA is idempotent; an
    // error only means the thread already has an apartment, which is fine.
    let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
    run(image).map_err(|e| format!("Text recognition failed ({}).", e.message()))?
}

fn run(image: &RgbaImage) -> windows::core::Result<Result<Vec<TextBox>, String>> {
    let Ok(engine) = OcrEngine::TryCreateFromUserProfileLanguages() else {
        return Ok(Err(NO_LANGUAGE.into()));
    };
    let max_dim = OcrEngine::MaxImageDimension().unwrap_or(4096).max(1);

    let (w, h) = image.dimensions();
    let long = w.max(h);
    let scale = if long > max_dim {
        max_dim as f32 / long as f32
    } else if u64::from(w) * u64::from(h) <= UPSCALE_BELOW_PIXELS && long * 2 <= max_dim {
        2.0
    } else {
        1.0
    };
    let scaled;
    let input = if scale == 1.0 {
        image
    } else {
        let sw = ((w as f32 * scale).round() as u32).clamp(1, max_dim);
        let sh = ((h as f32 * scale).round() as u32).clamp(1, max_dim);
        scaled = imageops::resize(image, sw, sh, FilterType::CatmullRom);
        &scaled
    };

    let bitmap = to_software_bitmap(input)?;
    let result = engine.RecognizeAsync(&bitmap)?.join()?;

    // Chinese and Japanese are written without spaces, but the engine reports
    // each character as a word; join those words directly.
    let tag = engine
        .RecognizerLanguage()
        .and_then(|l| l.LanguageTag())
        .map(|t| t.to_string_lossy())
        .unwrap_or_default();
    let separator = if tag.starts_with("zh") || tag.starts_with("ja") { "" } else { " " };

    let mut boxes = Vec::new();
    for line in result.Lines()? {
        let mut words = Vec::new();
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for word in line.Words()? {
            let r = word.BoundingRect()?;
            x0 = x0.min(r.X);
            y0 = y0.min(r.Y);
            x1 = x1.max(r.X + r.Width);
            y1 = y1.max(r.Y + r.Height);
            words.push(word.Text()?.to_string_lossy());
        }
        if words.is_empty() {
            continue;
        }
        boxes.push(TextBox {
            x: x0 / scale,
            y: y0 / scale,
            width: (x1 - x0) / scale,
            height: (y1 - y0) / scale,
            text: words.join(separator),
        });
    }
    Ok(Ok(boxes))
}

fn to_software_bitmap(image: &RgbaImage) -> windows::core::Result<SoftwareBitmap> {
    // RGBA -> BGRA, the engine's native layout.
    let mut bgra = image.as_raw().clone();
    for px in bgra.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let writer = DataWriter::new()?;
    writer.WriteBytes(&bgra)?;
    let buffer = writer.DetachBuffer()?;
    SoftwareBitmap::CreateCopyFromBuffer(
        &buffer,
        BitmapPixelFormat::Bgra8,
        image.width() as i32,
        image.height() as i32,
    )
}

const NO_LANGUAGE: &str = "Text recognition needs an OCR language pack. Add one in Windows Settings > Time & language > Language & region.";
