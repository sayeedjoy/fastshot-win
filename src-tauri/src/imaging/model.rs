//! Structured edit operations sent by the UI. Everything here is untrusted
//! input: `RenderRequest::validate` must pass before rendering.

use serde::Deserialize;

use crate::geometry::{FRect, PixelRect};

pub const MAX_ITEMS: usize = 2_000;
pub const MAX_PEN_POINTS: usize = 20_000;
pub const MAX_TEXT_CHARS: usize = 5_000;
pub const MAX_STEP_NUMBER: u32 = 999;
const MAX_COORD: f32 = 100_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const BLACK: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };

    /// Parse `#rgb`, `#rrggbb` or `#rrggbbaa`.
    pub fn parse(s: &str) -> Result<Color, String> {
        let hex = s
            .strip_prefix('#')
            .ok_or_else(|| format!("Invalid colour '{s}'"))?;
        let nib = |c: u8| -> Result<u8, String> {
            (c as char)
                .to_digit(16)
                .map(|d| d as u8)
                .ok_or_else(|| format!("Invalid colour '{s}'"))
        };
        let b = hex.as_bytes();
        let byte = |i: usize| -> Result<u8, String> { Ok(nib(b[i])? << 4 | nib(b[i + 1])?) };
        match b.len() {
            3 => Ok(Color {
                r: nib(b[0])? * 17,
                g: nib(b[1])? * 17,
                b: nib(b[2])? * 17,
                a: 255,
            }),
            6 => Ok(Color {
                r: byte(0)?,
                g: byte(2)?,
                b: byte(4)?,
                a: 255,
            }),
            8 => Ok(Color {
                r: byte(0)?,
                g: byte(2)?,
                b: byte(4)?,
                a: byte(6)?,
            }),
            _ => Err(format!("Invalid colour '{s}'")),
        }
    }

    pub fn opaque(self) -> Color {
        Color { a: 255, ..self }
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Color::parse(&s).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Annotation {
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: Color,
        stroke_width: f32,
    },
    Ellipse {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: Color,
        stroke_width: f32,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: Color,
        stroke_width: f32,
    },
    Arrow {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: Color,
        stroke_width: f32,
    },
    Pen {
        points: Vec<[f32; 2]>,
        color: Color,
        stroke_width: f32,
    },
    Text {
        x: f32,
        y: f32,
        text: String,
        color: Color,
        font_size: f32,
    },
    /// Numbered step marker: a filled disc centred on `(x, y)`.
    Step {
        x: f32,
        y: f32,
        radius: f32,
        number: u32,
        color: Color,
    },
    /// Freehand highlighter: a wide stroke drawn at `HIGHLIGHT_ALPHA`.
    Highlight {
        points: Vec<[f32; 2]>,
        color: Color,
        stroke_width: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EffectKind {
    Blur,
    Pixelate,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Effect {
    pub kind: EffectKind,
    pub rect: FRect,
    /// Blur: Gaussian sigma in px. Pixelate: block size in px.
    pub strength: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Redaction {
    pub rect: FRect,
    pub color: Color,
}

/// All coordinates in `annotations`, `effects`, `redactions` are relative to
/// the crop's top-left, in image pixels.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderRequest {
    pub crop: PixelRect,
    #[serde(default)]
    pub annotations: Vec<Annotation>,
    #[serde(default)]
    pub effects: Vec<Effect>,
    #[serde(default)]
    pub redactions: Vec<Redaction>,
}

fn coord_ok(v: f32) -> bool {
    v.is_finite() && v.abs() <= MAX_COORD
}

fn check_coords(values: &[f32]) -> Result<(), String> {
    if values.iter().all(|v| coord_ok(*v)) {
        Ok(())
    } else {
        Err("An annotation has invalid coordinates.".into())
    }
}

fn check_stroke(w: f32) -> Result<(), String> {
    if w.is_finite() && (0.5..=200.0).contains(&w) {
        Ok(())
    } else {
        Err("Stroke width must be between 0.5 and 200 px.".into())
    }
}

fn check_rect(r: &FRect) -> Result<(), String> {
    check_coords(&[r.x, r.y, r.width, r.height])
}

impl RenderRequest {
    pub fn validate(&self) -> Result<(), String> {
        let total = self.annotations.len() + self.effects.len() + self.redactions.len();
        if total > MAX_ITEMS {
            return Err(format!(
                "Too many edits ({total}); the limit is {MAX_ITEMS}."
            ));
        }
        for a in &self.annotations {
            match a {
                Annotation::Rect {
                    x,
                    y,
                    width,
                    height,
                    stroke_width,
                    ..
                }
                | Annotation::Ellipse {
                    x,
                    y,
                    width,
                    height,
                    stroke_width,
                    ..
                } => {
                    check_coords(&[*x, *y, *width, *height])?;
                    check_stroke(*stroke_width)?;
                }
                Annotation::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    stroke_width,
                    ..
                }
                | Annotation::Arrow {
                    x1,
                    y1,
                    x2,
                    y2,
                    stroke_width,
                    ..
                } => {
                    check_coords(&[*x1, *y1, *x2, *y2])?;
                    check_stroke(*stroke_width)?;
                }
                Annotation::Pen {
                    points,
                    stroke_width,
                    ..
                }
                | Annotation::Highlight {
                    points,
                    stroke_width,
                    ..
                } => {
                    if points.len() > MAX_PEN_POINTS {
                        return Err("A pen stroke has too many points.".into());
                    }
                    for p in points {
                        check_coords(p)?;
                    }
                    check_stroke(*stroke_width)?;
                }
                Annotation::Text {
                    x,
                    y,
                    text,
                    font_size,
                    ..
                } => {
                    check_coords(&[*x, *y])?;
                    if text.chars().count() > MAX_TEXT_CHARS {
                        return Err("A text annotation is too long.".into());
                    }
                    if !(font_size.is_finite() && (4.0..=500.0).contains(font_size)) {
                        return Err("Font size must be between 4 and 500 px.".into());
                    }
                }
                Annotation::Step {
                    x,
                    y,
                    radius,
                    number,
                    ..
                } => {
                    check_coords(&[*x, *y])?;
                    if !(radius.is_finite() && (4.0..=200.0).contains(radius)) {
                        return Err("Step marker size must be between 4 and 200 px.".into());
                    }
                    if !(1..=MAX_STEP_NUMBER).contains(number) {
                        return Err(format!(
                            "Step numbers must be between 1 and {MAX_STEP_NUMBER}."
                        ));
                    }
                }
            }
        }
        for e in &self.effects {
            check_rect(&e.rect)?;
            if !(e.strength.is_finite() && (1.0..=200.0).contains(&e.strength)) {
                return Err("Effect strength must be between 1 and 200.".into());
            }
        }
        for r in &self.redactions {
            check_rect(&r.rect)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_colors() {
        assert_eq!(
            Color::parse("#fff").unwrap(),
            Color {
                r: 255,
                g: 255,
                b: 255,
                a: 255
            }
        );
        assert_eq!(
            Color::parse("#FF0000").unwrap(),
            Color {
                r: 255,
                g: 0,
                b: 0,
                a: 255
            }
        );
        assert_eq!(
            Color::parse("#00ff0080").unwrap(),
            Color {
                r: 0,
                g: 255,
                b: 0,
                a: 128
            }
        );
        assert!(Color::parse("red").is_err());
        assert!(Color::parse("#ggg").is_err());
        assert!(Color::parse("#12345").is_err());
    }

    #[test]
    fn deserializes_request_from_ui_json() {
        let json = r##"{
            "crop": {"x": 1, "y": 2, "width": 30, "height": 40},
            "annotations": [
                {"type": "arrow", "x1": 0, "y1": 0, "x2": 10, "y2": 10, "color": "#e11d48", "strokeWidth": 4},
                {"type": "pen", "points": [[0,0],[1,1]], "color": "#000", "strokeWidth": 2},
                {"type": "text", "x": 5, "y": 5, "text": "hi", "color": "#000000", "fontSize": 24}
            ],
            "effects": [{"kind": "pixelate", "rect": {"x":0,"y":0,"width":5,"height":5}, "strength": 8}],
            "redactions": [{"rect": {"x":0,"y":0,"width":5,"height":5}, "color": "#000"}]
        }"##;
        let req: RenderRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.annotations.len(), 3);
        req.validate().unwrap();
    }

    #[test]
    fn rejects_bad_numbers() {
        let mut req = RenderRequest {
            crop: PixelRect::new(0, 0, 10, 10),
            annotations: vec![Annotation::Line {
                x1: 0.0,
                y1: 0.0,
                x2: f32::INFINITY,
                y2: 0.0,
                color: Color::BLACK,
                stroke_width: 2.0,
            }],
            effects: vec![],
            redactions: vec![],
        };
        assert!(req.validate().is_err());
        req.annotations = vec![Annotation::Line {
            x1: 0.0,
            y1: 0.0,
            x2: 1.0,
            y2: 0.0,
            color: Color::BLACK,
            stroke_width: 0.0,
        }];
        assert!(req.validate().is_err());
    }

    #[test]
    fn parses_and_validates_step_and_highlight() {
        let json = r##"{"crop":{"x":0,"y":0,"width":10,"height":10},"annotations":[
            {"type":"step","x":5,"y":5,"radius":16,"number":3,"color":"#e11d48"},
            {"type":"highlight","points":[[0,0],[9,0]],"color":"#facc15","strokeWidth":20}
        ]}"##;
        let mut req: RenderRequest = serde_json::from_str(json).unwrap();
        req.validate().unwrap();
        req.annotations[0] = Annotation::Step {
            x: 0.0,
            y: 0.0,
            radius: 16.0,
            number: 0,
            color: Color::BLACK,
        };
        assert!(req.validate().is_err(), "step 0 is rejected");
        req.annotations[0] = Annotation::Step {
            x: 0.0,
            y: 0.0,
            radius: 1.0,
            number: 1,
            color: Color::BLACK,
        };
        assert!(req.validate().is_err(), "tiny radius is rejected");
    }

    #[test]
    fn rejects_unknown_annotation_type() {
        let json = r##"{"crop":{"x":0,"y":0,"width":1,"height":1},"annotations":[{"type":"image","src":"x"}]}"##;
        assert!(serde_json::from_str::<RenderRequest>(json).is_err());
    }
}
