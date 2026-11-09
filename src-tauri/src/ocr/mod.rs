//! On-device text recognition (PRD §4: "OCR / text extraction", P2).
//!
//! The recognizer only ever sees the output of `render_export`, the same
//! raster that Copy and Save produce. Redactions are painted into it before
//! OCR runs, so redacted text can never be extracted (SEC-01). Recognition is
//! done by the OS (Windows.Media.Ocr / Apple Vision): nothing leaves the
//! machine and no pixels are written to disk.

use image::RgbaImage;

#[cfg(target_os = "macos")]
mod vision;
#[cfg(windows)]
mod winrt;

/// A recognised run of text in image pixels, origin at the top-left.
#[derive(Debug, Clone, PartialEq)]
pub struct TextBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub text: String,
}

/// Recognise the text in `image` and return it in reading order. An empty
/// string means no text was found.
pub fn recognize(image: &RgbaImage) -> Result<String, String> {
    if image.width() == 0 || image.height() == 0 {
        return Ok(String::new());
    }
    #[cfg(windows)]
    let boxes = winrt::recognize(image)?;
    #[cfg(target_os = "macos")]
    let boxes = vision::recognize(image)?;
    #[cfg(not(any(windows, target_os = "macos")))]
    let boxes: Vec<TextBox> = return Err("Text recognition isn't available on this platform.".into());

    Ok(assemble(boxes))
}

/// Arrange recognised boxes into lines of text.
///
/// Both OCR engines report one box per line *fragment*: a table row or a
/// label/value pair comes back as several boxes. Boxes whose vertical centres
/// fall within the same band are joined left-to-right into one line, and a
/// blank line is kept where the vertical gap suggests a new paragraph.
pub fn assemble(mut boxes: Vec<TextBox>) -> String {
    boxes.retain(|b| !b.text.trim().is_empty() && b.height > 0.0);
    if boxes.is_empty() {
        return String::new();
    }
    boxes.sort_by(|a, b| center(a).total_cmp(&center(b)));

    struct Row {
        top: f32,
        bottom: f32,
        boxes: Vec<TextBox>,
    }
    let mut rows: Vec<Row> = Vec::new();
    for b in boxes {
        let c = center(&b);
        match rows.last_mut() {
            // Same row when this box's centre is inside the row's band and the
            // row's centre is inside this box (tolerates mixed font sizes).
            Some(row)
                if c >= row.top
                    && c <= row.bottom
                    && (row.top + row.bottom) / 2.0 >= b.y
                    && (row.top + row.bottom) / 2.0 <= b.y + b.height =>
            {
                row.top = row.top.min(b.y);
                row.bottom = row.bottom.max(b.y + b.height);
                row.boxes.push(b);
            }
            _ => rows.push(Row {
                top: b.y,
                bottom: b.y + b.height,
                boxes: vec![b],
            }),
        }
    }

    let mut out = String::new();
    let mut prev: Option<(f32, f32)> = None; // (bottom, height) of the last row
    for mut row in rows {
        row.boxes.sort_by(|a, b| a.x.total_cmp(&b.x));
        let line = row
            .boxes
            .iter()
            .map(|b| b.text.trim())
            .collect::<Vec<_>>()
            .join(" ");
        let height = row.bottom - row.top;
        if let Some((bottom, prev_height)) = prev {
            out.push('\n');
            if row.top - bottom > 1.2 * height.max(prev_height) {
                out.push('\n');
            }
        }
        out.push_str(&line);
        prev = Some((row.bottom, height));
    }
    out
}

fn center(b: &TextBox) -> f32 {
    b.y + b.height / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tb(x: f32, y: f32, w: f32, h: f32, text: &str) -> TextBox {
        TextBox {
            x,
            y,
            width: w,
            height: h,
            text: text.into(),
        }
    }

    #[test]
    fn empty_input_is_empty_text() {
        assert_eq!(assemble(vec![]), "");
        assert_eq!(assemble(vec![tb(0.0, 0.0, 10.0, 10.0, "  ")]), "");
    }

    #[test]
    fn lines_are_ordered_top_to_bottom() {
        let text = assemble(vec![
            tb(0.0, 20.0, 50.0, 12.0, "second"),
            tb(0.0, 4.0, 50.0, 12.0, "first"),
        ]);
        assert_eq!(text, "first\nsecond");
    }

    #[test]
    fn fragments_on_one_row_join_left_to_right() {
        let text = assemble(vec![
            tb(200.0, 11.0, 40.0, 12.0, "$12.00"),
            tb(0.0, 10.0, 60.0, 14.0, "Total"),
            tb(0.0, 30.0, 60.0, 12.0, "Paid"),
        ]);
        assert_eq!(text, "Total $12.00\nPaid");
    }

    #[test]
    fn large_gap_keeps_a_paragraph_break() {
        let text = assemble(vec![
            tb(0.0, 0.0, 50.0, 10.0, "Title"),
            tb(0.0, 12.0, 50.0, 10.0, "line"),
            tb(0.0, 60.0, 50.0, 10.0, "Next"),
        ]);
        assert_eq!(text, "Title\nline\n\nNext");
    }

    #[test]
    fn text_is_trimmed() {
        assert_eq!(assemble(vec![tb(0.0, 0.0, 9.0, 9.0, "  hi \n")]), "hi");
    }

    /// Redaction is applied before OCR: the recogniser is handed the export
    /// raster, so a fully redacted crop is a solid block with nothing to read.
    #[test]
    fn recognizer_input_is_the_redacted_export() {
        use crate::imaging::model::RenderRequest;
        use crate::imaging::render::render_export;

        let mut src = RgbaImage::from_pixel(40, 20, image::Rgba([255, 255, 255, 255]));
        for x in 5..35 {
            src.put_pixel(x, 10, image::Rgba([0, 0, 0, 255]));
        }
        let json = r##"{"crop":{"x":0,"y":0,"width":40,"height":20},"annotations":[],"effects":[],
            "redactions":[{"rect":{"x":0,"y":0,"width":40,"height":20},"color":"#000000"}]}"##;
        let request: RenderRequest = serde_json::from_str(json).unwrap();
        let out = render_export(&src, &request).unwrap();
        assert!(out.pixels().all(|p| p.0 == [0, 0, 0, 255]));
    }

    /// End to end against the real OS engine: visible text is read, redacted
    /// text is not. Needs Windows OCR language support or macOS Vision, so it
    /// is opt-in: `cargo test -- --ignored ocr_reads_text_but_not_redactions`.
    #[test]
    #[ignore = "needs the OS OCR engine (and a language pack on Windows)"]
    #[cfg(any(windows, target_os = "macos"))]
    fn ocr_reads_text_but_not_redactions() {
        use crate::imaging::model::RenderRequest;
        use crate::imaging::render::render_export;

        let src = RgbaImage::from_pixel(520, 170, image::Rgba([255, 255, 255, 255]));
        let json = r##"{"crop":{"x":0,"y":0,"width":520,"height":170},"effects":[],
            "annotations":[
                {"type":"text","x":20,"y":20,"text":"Invoice 4821","color":"#111111","fontSize":36},
                {"type":"text","x":20,"y":66,"text":"Total due 1250","color":"#333333","fontSize":15},
                {"type":"text","x":20,"y":100,"text":"Password hunter2","color":"#111111","fontSize":36}],
            "redactions":[{"rect":{"x":0,"y":90,"width":520,"height":80},"color":"#ffffff"}]}"##;
        let request: RenderRequest = serde_json::from_str(json).unwrap();
        let image = render_export(&src, &request).unwrap();

        let text = recognize(&image).unwrap();
        assert!(text.contains("Invoice"), "got {text:?}");
        assert!(text.contains("4821"), "got {text:?}");
        // Screen-sized (15 px) text must survive too.
        assert!(text.contains("Total due 1250"), "got {text:?}");
        assert!(!text.contains("hunter2") && !text.contains("Password"), "got {text:?}");
    }
}
