//! Filename templates for saved screenshots (P1 "custom naming templates").
//!
//! A template is literal text plus `{token}`s. Tokens only ever expand to
//! digits and dashes, and literal text is validated up front, so a rendered
//! name can never contain path separators or characters the OS rejects. The
//! template only names the file: the folder always comes from Settings or the
//! native Save dialog, never from the UI.

use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;

pub const DEFAULT_TEMPLATE: &str = "Screenshot-{date}-{time}";
const MAX_TEMPLATE_LEN: usize = 120;
const MAX_SEQUENCE: u32 = 9_999;

/// Supported tokens, shown in Settings.
pub const TOKENS: &[(&str, &str)] = &[
    ("date", "2026-11-09"),
    ("time", "142501"),
    ("YYYY", "2026"),
    ("YY", "26"),
    ("MM", "11"),
    ("DD", "09"),
    ("hh", "14"),
    ("mm", "25"),
    ("ss", "01"),
    ("width", "1280"),
    ("height", "720"),
    ("n", "1, 2, 3… (first unused number)"),
];

#[derive(Debug, Clone, Copy)]
pub struct NameContext {
    pub now: NaiveDateTime,
    pub width: u32,
    pub height: u32,
}

impl NameContext {
    pub fn now(width: u32, height: u32) -> Self {
        NameContext {
            now: chrono::Local::now().naive_local(),
            width,
            height,
        }
    }
}

enum Part<'a> {
    Lit(&'a str),
    Token(&'a str),
}

fn parse(template: &str) -> Result<Vec<Part<'_>>, String> {
    let mut parts = Vec::new();
    let mut rest = template;
    while !rest.is_empty() {
        match rest.find(['{', '}']) {
            None => {
                parts.push(Part::Lit(rest));
                break;
            }
            Some(i) if rest.as_bytes()[i] == b'}' => {
                return Err("The file name template has a \"}\" without a matching \"{\".".into());
            }
            Some(i) => {
                if i > 0 {
                    parts.push(Part::Lit(&rest[..i]));
                }
                let after = &rest[i + 1..];
                let end = after
                    .find('}')
                    .ok_or("The file name template has a \"{\" that is never closed.")?;
                let name = &after[..end];
                if !TOKENS.iter().any(|(t, _)| *t == name) {
                    return Err(format!(
                        "\"{{{name}}}\" isn't a known template token. Try {{date}}, {{time}} or {{n}}."
                    ));
                }
                parts.push(Part::Token(name));
                rest = &after[end + 1..];
            }
        }
    }
    Ok(parts)
}

fn bad_char(c: char) -> bool {
    c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
}

/// Check a template without rendering it for a particular file.
pub fn validate(template: &str) -> Result<(), String> {
    if template.trim().is_empty() {
        return Err("The file name template can't be empty.".into());
    }
    if template.chars().count() > MAX_TEMPLATE_LEN {
        return Err(format!(
            "The file name template is too long (max {MAX_TEMPLATE_LEN} characters)."
        ));
    }
    let parts = parse(template)?;
    for p in &parts {
        if let Part::Lit(s) = p
            && let Some(c) = s.chars().find(|c| bad_char(*c))
        {
            return Err(format!(
                "File names can't contain \"{}\". Remove it from the template.",
                c.escape_default()
            ));
        }
    }
    if template.ends_with(['.', ' ']) || template.starts_with(' ') {
        return Err(
            "The file name template can't start with a space or end with a space or dot.".into(),
        );
    }
    // Windows refuses these as file names regardless of extension.
    let stem = render_stem(&parts, &NameContext::now(1, 1), 1);
    let upper = stem.to_ascii_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL"].contains(&upper.as_str())
        || ((upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.len() == 4
            && upper.as_bytes()[3].is_ascii_digit());
    if reserved {
        return Err(format!(
            "\"{stem}\" is reserved by Windows and can't be used as a file name."
        ));
    }
    Ok(())
}

fn render_stem(parts: &[Part<'_>], ctx: &NameContext, n: u32) -> String {
    let mut out = String::new();
    for p in parts {
        match p {
            Part::Lit(s) => out.push_str(s),
            Part::Token(t) => {
                let f = |fmt: &str| ctx.now.format(fmt).to_string();
                out.push_str(&match *t {
                    "date" => f("%Y-%m-%d"),
                    "time" => f("%H%M%S"),
                    "YYYY" => f("%Y"),
                    "YY" => f("%y"),
                    "MM" => f("%m"),
                    "DD" => f("%d"),
                    "hh" => f("%H"),
                    "mm" => f("%M"),
                    "ss" => f("%S"),
                    "width" => ctx.width.to_string(),
                    "height" => ctx.height.to_string(),
                    "n" => n.to_string(),
                    _ => String::new(),
                });
            }
        }
    }
    out
}

/// Render a template to `stem.ext` for display (Settings preview).
pub fn preview(template: &str, ctx: &NameContext, ext: &str) -> Result<String, String> {
    validate(template)?;
    Ok(format!("{}.{ext}", render_stem(&parse(template)?, ctx, 1)))
}

/// A file name that doesn't exist yet in `dir` (when given). With `{n}` the
/// first free number is used; otherwise `-2`, `-3`… is appended on collision.
pub fn unique_name(
    template: &str,
    ctx: &NameContext,
    ext: &str,
    dir: Option<&Path>,
) -> Result<String, String> {
    validate(template)?;
    let parts = parse(template)?;
    let free = |name: &str| dir.is_none_or(|d| !d.join(name).exists());
    let has_seq = parts.iter().any(|p| matches!(p, Part::Token("n")));

    if has_seq {
        return (1..=MAX_SEQUENCE)
            .map(|n| format!("{}.{ext}", render_stem(&parts, ctx, n)))
            .find(|name| free(name))
            .ok_or_else(|| "Too many files with this name pattern in the folder.".into());
    }
    let stem = render_stem(&parts, ctx, 1);
    let first = format!("{stem}.{ext}");
    if free(&first) {
        return Ok(first);
    }
    (2..=MAX_SEQUENCE)
        .map(|n| format!("{stem}-{n}.{ext}"))
        .find(|name| free(name))
        .ok_or_else(|| "Too many files with this name in the folder.".into())
}

/// Full path for a quick save into `dir`.
pub fn unique_path(
    template: &str,
    ctx: &NameContext,
    ext: &str,
    dir: &Path,
) -> Result<PathBuf, String> {
    Ok(dir.join(unique_name(template, ctx, ext, Some(dir))?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> NameContext {
        NameContext {
            now: chrono::NaiveDate::from_ymd_opt(2026, 11, 9)
                .unwrap()
                .and_hms_opt(14, 5, 1)
                .unwrap(),
            width: 640,
            height: 480,
        }
    }

    fn temp_dir() -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("fastshot-naming-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn default_matches_legacy_format() {
        assert_eq!(
            preview(DEFAULT_TEMPLATE, &ctx(), "png").unwrap(),
            "Screenshot-2026-11-09-140501.png"
        );
    }

    #[test]
    fn expands_every_token() {
        let t = "{YYYY}{YY}{MM}{DD}_{hh}{mm}{ss}_{width}x{height}_{n}";
        assert_eq!(
            preview(t, &ctx(), "jpg").unwrap(),
            "2026261109_140501_640x480_1.jpg"
        );
    }

    #[test]
    fn rejects_bad_templates() {
        for t in [
            "", "   ", "a/b", "a\\b", "x:y", "{nope}", "{date", "date}", "a.", " a", "CON",
            "{date}?",
        ] {
            assert!(validate(t).is_err(), "{t:?} should be rejected");
        }
        assert!(validate(&"a".repeat(200)).is_err());
        validate("bug-report {date} #{n}").unwrap();
    }

    #[test]
    fn appends_suffix_on_collision() {
        let dir = temp_dir();
        let a = unique_name("shot", &ctx(), "png", Some(&dir)).unwrap();
        assert_eq!(a, "shot.png");
        std::fs::write(dir.join(&a), b"x").unwrap();
        assert_eq!(
            unique_name("shot", &ctx(), "png", Some(&dir)).unwrap(),
            "shot-2.png"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn sequence_token_finds_first_free_number() {
        let dir = temp_dir();
        for n in [1, 2, 4] {
            std::fs::write(dir.join(format!("step-{n}.png")), b"x").unwrap();
        }
        let p = unique_path("step-{n}", &ctx(), "png", &dir).unwrap();
        assert_eq!(p, dir.join("step-3.png"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
