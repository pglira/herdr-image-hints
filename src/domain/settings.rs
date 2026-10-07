//! Everything the user can tune, already validated: the kernel never sees a
//! raw config file.

use super::alphabet::Alphabet;
use super::patterns::{DEFAULT_EXTENSIONS, PatternSet};
use super::style::{Color, TextStyle};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HintPosition {
    #[default]
    Left,
    Right,
}

/// Colors of the overlay. `backdrop` restyles everything that is not a
/// match (for instance dimming it); `None` keeps the pane's own colors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    pub hint: TextStyle,
    pub highlight: TextStyle,
    pub selected_hint: TextStyle,
    pub selected_highlight: TextStyle,
    pub backdrop: Option<TextStyle>,
    pub hint_position: HintPosition,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            hint: TextStyle::PLAIN
                .fg(Color::Indexed(0))
                .bg(Color::Indexed(3))
                .bold(),
            highlight: TextStyle::PLAIN.fg(Color::Indexed(3)),
            selected_hint: TextStyle::PLAIN
                .fg(Color::Indexed(0))
                .bg(Color::Indexed(12))
                .bold(),
            selected_highlight: TextStyle::PLAIN.fg(Color::Indexed(12)),
            backdrop: None,
            hint_position: HintPosition::Left,
        }
    }
}

/// The validated configuration of one run.
#[derive(Debug, Clone)]
pub struct Settings {
    pub alphabet: Alphabet,
    pub patterns: PatternSet,
    /// Lowercase image extensions: what the pattern matches and what the
    /// popup steps through in the image's directory.
    pub extensions: Vec<String>,
    pub theme: Theme,
    pub popup: PopupSize,
}

/// The size of the image popup: a cell count or a percentage ("85%") of the
/// terminal, per axis, as the Herdr API takes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PopupSize {
    pub width: String,
    pub height: String,
}

impl Default for PopupSize {
    fn default() -> Self {
        PopupSize {
            width: "85%".to_string(),
            height: "85%".to_string(),
        }
    }
}

impl PopupSize {
    /// Accepts a positive cell count or a percentage from 1% to 100%.
    pub fn is_valid(value: &str) -> bool {
        match value.strip_suffix('%') {
            Some(percent) => percent.parse::<u8>().is_ok_and(|n| (1..=100).contains(&n)),
            None => value.parse::<u16>().is_ok_and(|n| n > 0),
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            alphabet: Alphabet::default(),
            patterns: PatternSet::images(),
            extensions: DEFAULT_EXTENSIONS.iter().map(|e| e.to_string()).collect(),
            theme: Theme::default(),
            popup: PopupSize::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popup_sizes_are_cells_or_percentages() {
        assert!(PopupSize::is_valid("85%"));
        assert!(PopupSize::is_valid("100%"));
        assert!(PopupSize::is_valid("120"));
        assert!(!PopupSize::is_valid("0%"));
        assert!(!PopupSize::is_valid("101%"));
        assert!(!PopupSize::is_valid("0"));
        assert!(!PopupSize::is_valid("wide"));
    }
}
