//! Reads the plugin's `config.toml` into validated [`Settings`].

use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

use crate::domain::alphabet::{Alphabet, AlphabetError};
use crate::domain::patterns::{self, DEFAULT_EXTENSIONS, PatternError, PatternSet, PatternSpec};
use crate::domain::settings::{HintPosition, PopupSize, Settings, Theme};
use crate::domain::style::{Color, ColorParseError, TextStyle};

/// The commented default configuration, written on first run.
pub const DEFAULT_CONFIG: &str = include_str!("../../examples/config.toml");

pub const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("config.toml is not valid: {0}")]
    Parse(#[from] toml::de::Error),
    #[error(transparent)]
    Alphabet(#[from] AlphabetError),
    #[error(transparent)]
    Pattern(#[from] PatternError),
    #[error("in [style]: {0}")]
    Color(#[from] ColorParseError),
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    keyboard_layout: Option<String>,
    alphabet: Option<String>,
    hint_position: Option<String>,
    extensions: Option<Vec<String>>,
    #[serde(default)]
    patterns: Vec<PatternEntry>,
    popup_width: Option<String>,
    popup_height: Option<String>,
    #[serde(default)]
    style: StyleConfig,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PatternEntry {
    name: String,
    regex: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct StyleConfig {
    hint: Option<StyleEntry>,
    highlight: Option<StyleEntry>,
    selected_hint: Option<StyleEntry>,
    selected_highlight: Option<StyleEntry>,
    backdrop: Option<StyleEntry>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct StyleEntry {
    fg: Option<String>,
    bg: Option<String>,
    #[serde(default)]
    bold: bool,
    #[serde(default)]
    dim: bool,
    #[serde(default)]
    italic: bool,
    #[serde(default)]
    underline: bool,
    #[serde(default)]
    reverse: bool,
    #[serde(default)]
    strikethrough: bool,
}

impl StyleEntry {
    fn to_style(&self) -> Result<TextStyle, ColorParseError> {
        Ok(TextStyle {
            fg: self.fg.as_deref().map(Color::parse).transpose()?,
            bg: self.bg.as_deref().map(Color::parse).transpose()?,
            bold: self.bold,
            dim: self.dim,
            italic: self.italic,
            underline: self.underline,
            reverse: self.reverse,
            strikethrough: self.strikethrough,
        })
    }
}

/// Loads `config.toml` from `dir`. A missing file means defaults, and the
/// commented default file is written there so the user can find the knobs.
pub fn load(dir: Option<&Path>) -> Result<Settings, ConfigError> {
    let Some(dir) = dir else {
        return Ok(Settings::default());
    };
    let path = dir.join(CONFIG_FILE_NAME);
    match std::fs::read_to_string(&path) {
        Ok(text) => parse(&text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            write_default(&path);
            Ok(Settings::default())
        }
        Err(source) => Err(ConfigError::Read { path, source }),
    }
}

fn write_default(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, DEFAULT_CONFIG);
}

/// Parses and validates a configuration file's text.
pub fn parse(text: &str) -> Result<Settings, ConfigError> {
    let file: FileConfig = toml::from_str(text)?;
    let defaults = Settings::default();

    let alphabet = match (&file.alphabet, &file.keyboard_layout) {
        (Some(keys), _) => Alphabet::custom(keys)?,
        (None, Some(layout)) => Alphabet::layout(layout)?,
        (None, None) => defaults.alphabet,
    };

    let extensions: Vec<String> = match &file.extensions {
        Some(list) => list.iter().map(|e| e.trim().to_ascii_lowercase()).collect(),
        None => DEFAULT_EXTENSIONS.iter().map(|e| e.to_string()).collect(),
    };
    let mut specs = vec![patterns::image_spec(&extensions)?];
    let custom: Vec<PatternSpec> = file
        .patterns
        .iter()
        .map(|entry| PatternSpec::new(entry.name.clone(), entry.regex.clone()))
        .collect();
    specs.splice(0..0, custom);
    let pattern_set = PatternSet::compile(&specs)?;

    let hint_position = match file.hint_position.as_deref().map(str::trim) {
        None | Some("left") => HintPosition::Left,
        Some("right") => HintPosition::Right,
        Some(other) => {
            return Err(ConfigError::Invalid(format!(
                "hint_position must be \"left\" or \"right\", got \"{other}\""
            )));
        }
    };

    let popup_size = |value: &Option<String>, key: &str, fallback: String| match value {
        None => Ok(fallback),
        Some(value) if PopupSize::is_valid(value.trim()) => Ok(value.trim().to_string()),
        Some(value) => Err(ConfigError::Invalid(format!(
            "{key} must be a cell count or a percentage such as \"85%\", got \"{value}\""
        ))),
    };
    let popup = PopupSize {
        width: popup_size(
            &file.popup_width,
            "popup_width",
            defaults.popup.width.clone(),
        )?,
        height: popup_size(
            &file.popup_height,
            "popup_height",
            defaults.popup.height.clone(),
        )?,
    };

    let default_theme = Theme::default();
    let style =
        |entry: &Option<StyleEntry>, fallback: TextStyle| -> Result<TextStyle, ConfigError> {
            Ok(entry
                .as_ref()
                .map(StyleEntry::to_style)
                .transpose()?
                .unwrap_or(fallback))
        };
    let theme = Theme {
        hint: style(&file.style.hint, default_theme.hint)?,
        highlight: style(&file.style.highlight, default_theme.highlight)?,
        selected_hint: style(&file.style.selected_hint, default_theme.selected_hint)?,
        selected_highlight: style(
            &file.style.selected_highlight,
            default_theme.selected_highlight,
        )?,
        backdrop: file
            .style
            .backdrop
            .as_ref()
            .map(StyleEntry::to_style)
            .transpose()?,
        hint_position,
    };

    Ok(Settings {
        alphabet,
        patterns: pattern_set,
        extensions,
        theme,
        popup,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_default_file_parses_to_the_default_settings() {
        let parsed = parse(DEFAULT_CONFIG).unwrap();
        let defaults = Settings::default();
        assert_eq!(parsed.alphabet, defaults.alphabet);
        assert_eq!(parsed.patterns.names(), defaults.patterns.names());
        assert_eq!(parsed.theme, defaults.theme);
        assert_eq!(parsed.popup, defaults.popup);
    }

    #[test]
    fn an_empty_file_is_the_defaults() {
        let parsed = parse("").unwrap();
        assert_eq!(parsed.patterns.names(), vec![patterns::IMAGE_PATTERN]);
        assert_eq!(parsed.popup, PopupSize::default());
    }

    #[test]
    fn custom_patterns_come_before_the_image_pattern() {
        let parsed = parse(
            r#"
[[patterns]]
name = "plot"
regex = "plot-[0-9]+"
"#,
        )
        .unwrap();
        assert_eq!(parsed.patterns.names(), vec!["plot", "image"]);
    }

    #[test]
    fn extensions_are_lowercased_and_replace_the_defaults() {
        let parsed = parse("extensions = [\"SVG\"]").unwrap();
        let found = parsed.patterns.find("a.svg b.png");
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].capture_start, found[0].capture_end), (0, 5));
    }

    #[test]
    fn styles_popup_and_hints_are_read() {
        let parsed = parse(
            r##"
alphabet = "jkl"
hint_position = "right"
popup_width = "60%"
popup_height = "40"
[style]
hint = { fg = "#ff0000", bg = "0", bold = true, underline = true }
backdrop = { dim = true }
"##,
        )
        .unwrap();
        assert_eq!(parsed.alphabet.keys(), &['j', 'k', 'l']);
        assert_eq!(parsed.theme.hint_position, HintPosition::Right);
        assert_eq!(
            parsed.popup,
            PopupSize {
                width: "60%".into(),
                height: "40".into()
            }
        );
        assert_eq!(parsed.theme.hint.fg, Some(Color::Rgb(255, 0, 0)));
        assert!(parsed.theme.hint.underline);
        assert_eq!(parsed.theme.backdrop, Some(TextStyle::PLAIN.dim()));
    }

    #[test]
    fn mistakes_are_reported_precisely() {
        assert!(matches!(
            parse("hint_position = \"middle\""),
            Err(ConfigError::Invalid(_))
        ));
        assert!(matches!(
            parse("popup_width = \"wide\""),
            Err(ConfigError::Invalid(_))
        ));
        assert!(matches!(
            parse("keyboard_layout = \"bepo\""),
            Err(ConfigError::Alphabet(_))
        ));
        assert!(matches!(
            parse("extensions = []"),
            Err(ConfigError::Pattern(_))
        ));
        assert!(matches!(
            parse("[style]\nhint = { fg = \"chartreuse\" }"),
            Err(ConfigError::Color(_))
        ));
        assert!(matches!(parse("typo = 1"), Err(ConfigError::Parse(_))));
    }

    #[test]
    fn a_missing_file_is_written_with_the_defaults() {
        let dir = std::env::temp_dir().join(format!(
            "herdr-image-hints-config-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let settings = load(Some(&dir)).unwrap();
        assert_eq!(settings.popup, PopupSize::default());
        assert_eq!(
            std::fs::read_to_string(dir.join(CONFIG_FILE_NAME)).unwrap(),
            DEFAULT_CONFIG
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
