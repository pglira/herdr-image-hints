//! What counts as an image path: the built-in pattern over a list of image
//! extensions plus whatever the user adds, compiled once per session.

use regex::Regex;
use thiserror::Error;

/// A named regular expression. A `match` capture group narrows the copied
/// text to that group (the rest of the match is only context).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternSpec {
    pub name: String,
    pub regex: String,
}

impl PatternSpec {
    pub fn new(name: impl Into<String>, regex: impl Into<String>) -> Self {
        PatternSpec {
            name: name.into(),
            regex: regex.into(),
        }
    }
}

/// The image file extensions recognised by default, lowercase.
pub const DEFAULT_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff", "ico", "qoi", "tga", "pnm", "pbm",
    "pgm", "ppm", "exr", "hdr",
];

/// The name of the built-in image path pattern.
pub const IMAGE_PATTERN: &str = "image";

/// A path to a file with one of `extensions`: absolute, relative or starting
/// with `~/`, made of word characters and `.@+%=-` per segment. Case is
/// ignored, so `shot.PNG` matches `png`.
pub fn image_spec(extensions: &[String]) -> Result<PatternSpec, PatternError> {
    if extensions.is_empty() {
        return Err(PatternError::NoExtensions);
    }
    if let Some(bad) = extensions
        .iter()
        .find(|ext| ext.is_empty() || !ext.chars().all(|c| c.is_ascii_alphanumeric()))
    {
        return Err(PatternError::InvalidExtension(bad.clone()));
    }
    let alternatives: Vec<String> = extensions.iter().map(|e| regex::escape(e)).collect();
    Ok(PatternSpec::new(
        IMAGE_PATTERN,
        format!(
            r"(?i)(?:~/|/)?(?:[\w.@+%=-]+/)*[\w.@+%=-]+\.(?:{})\b",
            alternatives.join("|")
        ),
    ))
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PatternError {
    #[error("the extension list is empty")]
    NoExtensions,
    #[error("extension `{0}` must be letters and digits only")]
    InvalidExtension(String),
    #[error("pattern `{name}` is not a valid regular expression: {reason}")]
    InvalidRegex { name: String, reason: String },
    #[error("pattern names must be unique; `{0}` appears twice")]
    DuplicateName(String),
}

#[derive(Debug, Clone)]
struct Pattern {
    name: String,
    regex: Regex,
}

/// A match found in a logical line, in byte offsets of that line's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextMatch {
    pub pattern: String,
    /// The whole regular expression match; nothing else may overlap it.
    pub start: usize,
    pub end: usize,
    /// The part worth copying: the `match` group when present, else the whole.
    pub capture_start: usize,
    pub capture_end: usize,
}

/// The compiled, ordered patterns of one session.
#[derive(Debug, Clone, Default)]
pub struct PatternSet {
    patterns: Vec<Pattern>,
}

impl PatternSet {
    /// The image path pattern over the default extensions.
    pub fn images() -> Self {
        let extensions: Vec<String> = DEFAULT_EXTENSIONS.iter().map(|e| e.to_string()).collect();
        Self::compile(&[image_spec(&extensions).expect("the defaults are valid")])
            .expect("the image pattern compiles")
    }

    /// Compiles `specs` in order; earlier patterns win ties at a column.
    pub fn compile(specs: &[PatternSpec]) -> Result<Self, PatternError> {
        let mut patterns: Vec<Pattern> = Vec::with_capacity(specs.len());
        for spec in specs {
            if patterns.iter().any(|existing| existing.name == spec.name) {
                return Err(PatternError::DuplicateName(spec.name.clone()));
            }
            let regex = Regex::new(&spec.regex).map_err(|error| PatternError::InvalidRegex {
                name: spec.name.clone(),
                reason: error.to_string(),
            })?;
            patterns.push(Pattern {
                name: spec.name.clone(),
                regex,
            });
        }
        Ok(PatternSet { patterns })
    }

    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    pub fn names(&self) -> Vec<&str> {
        self.patterns.iter().map(|p| p.name.as_str()).collect()
    }

    /// Non-overlapping matches in `text`, leftmost first; at equal starts the
    /// earlier pattern wins, then the longer match. Trailing whitespace is
    /// trimmed from the captured part so a `.+` group never copies padding.
    pub fn find(&self, text: &str) -> Vec<TextMatch> {
        let mut candidates: Vec<(usize, TextMatch)> = Vec::new();
        for (priority, pattern) in self.patterns.iter().enumerate() {
            for captures in pattern.regex.captures_iter(text) {
                let whole = captures.get(0).expect("group 0 always exists");
                let capture = captures.name("match").unwrap_or(whole);
                let (capture_start, capture_end) =
                    trim_trailing_whitespace(text, capture.start(), capture.end());
                if capture_start >= capture_end {
                    continue;
                }
                candidates.push((
                    priority,
                    TextMatch {
                        pattern: pattern.name.clone(),
                        start: whole.start(),
                        end: whole.end(),
                        capture_start,
                        capture_end,
                    },
                ));
            }
        }
        candidates.sort_by(|(priority_a, a), (priority_b, b)| {
            a.start
                .cmp(&b.start)
                .then(priority_a.cmp(priority_b))
                .then(b.end.cmp(&a.end))
        });
        let mut accepted: Vec<TextMatch> = Vec::new();
        let mut cursor = 0;
        for (_, candidate) in candidates {
            if candidate.start < cursor {
                continue;
            }
            cursor = candidate.end;
            accepted.push(candidate);
        }
        accepted
    }
}

fn trim_trailing_whitespace(text: &str, start: usize, end: usize) -> (usize, usize) {
    let trimmed = text[start..end].trim_end();
    (start, start + trimmed.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn captured<'a>(set: &PatternSet, text: &'a str) -> Vec<&'a str> {
        set.find(text)
            .into_iter()
            .map(|m| &text[m.capture_start..m.capture_end])
            .collect()
    }

    #[test]
    fn absolute_relative_and_home_paths_are_found() {
        let set = PatternSet::images();
        assert_eq!(
            captured(&set, "see /tmp/a.png, ./b.jpg and ~/pics/e.webp"),
            vec!["/tmp/a.png", "./b.jpg", "~/pics/e.webp"]
        );
        assert_eq!(captured(&set, "plot.PNG"), vec!["plot.PNG"]);
        assert_eq!(
            captured(&set, "../out/x-1_2.tiff"),
            vec!["../out/x-1_2.tiff"]
        );
    }

    #[test]
    fn surrounding_punctuation_and_box_drawing_stay_outside_the_match() {
        let set = PatternSet::images();
        assert_eq!(
            captured(&set, "│ saved to `out/fig.png`. │"),
            vec!["out/fig.png"]
        );
        assert_eq!(captured(&set, "(fig.gif)"), vec!["fig.gif"]);
        assert_eq!(captured(&set, "fig.png:12"), vec!["fig.png"]);
    }

    #[test]
    fn other_files_and_partial_extensions_are_ignored() {
        let set = PatternSet::images();
        assert!(captured(&set, "src/main.rs notes.md a.pngx").is_empty());
    }

    #[test]
    fn custom_extensions_replace_the_defaults() {
        let spec = image_spec(&["svg".to_string()]).unwrap();
        let set = PatternSet::compile(&[spec]).unwrap();
        assert_eq!(captured(&set, "a.svg b.png"), vec!["a.svg"]);
    }

    #[test]
    fn bad_extensions_and_regexes_are_reported() {
        assert_eq!(image_spec(&[]), Err(PatternError::NoExtensions));
        assert_eq!(
            image_spec(&["p.ng".to_string()]),
            Err(PatternError::InvalidExtension("p.ng".into()))
        );
        let error = PatternSet::compile(&[PatternSpec::new("bad", "(")]).unwrap_err();
        assert!(matches!(error, PatternError::InvalidRegex { name, .. } if name == "bad"));
        let error =
            PatternSet::compile(&[PatternSpec::new("dup", "a"), PatternSpec::new("dup", "b")])
                .unwrap_err();
        assert_eq!(error, PatternError::DuplicateName("dup".into()));
    }
}
