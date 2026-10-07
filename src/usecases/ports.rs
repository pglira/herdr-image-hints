//! What the use cases need from the world, as traits the adapters implement.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::domain::geometry::{Layout, OverlayGeometry};
use crate::domain::screen::Screen;
use crate::domain::session::{Outcome, Session};
use crate::domain::settings::{PopupSize, Theme};

/// An adapter failed; the message is already fit for a human.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{0}")]
pub struct PortError(pub String);

impl PortError {
    pub fn new(message: impl std::fmt::Display) -> Self {
        PortError(message.to_string())
    }
}

/// The multiplexer the panes live in.
pub trait PaneHost {
    /// The tab layout around `pane_id`.
    fn layout(&self, pane_id: &str) -> Result<Layout, PortError>;
    /// The pane's visible screen, ANSI styling included.
    fn read_visible(&self, pane_id: &str) -> Result<String, PortError>;
    /// The label the host shows for the pane, if any.
    fn pane_label(&self, pane_id: &str) -> Result<Option<String>, PortError>;
    /// The directory the pane's foreground process runs in.
    fn pane_cwd(&self, pane_id: &str) -> Result<Option<PathBuf>, PortError>;
    /// Opens one of this plugin's declared panes; returns the new pane id.
    fn open_overlay(
        &self,
        plugin_id: &str,
        entrypoint: &str,
        env: BTreeMap<String, String>,
    ) -> Result<String, PortError>;
    /// Opens one of this plugin's declared panes as a centered popup.
    fn open_popup(
        &self,
        plugin_id: &str,
        entrypoint: &str,
        env: BTreeMap<String, String>,
        size: &PopupSize,
    ) -> Result<(), PortError>;
    /// Shows a toast.
    fn notify(&self, title: &str, body: &str) -> Result<(), PortError>;
}

/// The bits of the file system a pick looks at.
pub trait Files {
    /// True when `path` names an existing regular file.
    fn is_file(&self, path: &Path) -> bool;
    /// The user's home directory, for `~/` paths.
    fn home(&self) -> Option<PathBuf>;
}

/// Everything the picker shows besides the session it drives.
pub struct PickView<'a> {
    pub screen: &'a Screen,
    pub theme: &'a Theme,
    pub geometry: Option<&'a OverlayGeometry>,
    /// A one-line notice (configuration problem…).
    pub notice: Option<&'a str>,
}

/// The interactive part: shows the hints, feeds keys to the session, and
/// returns when the user has picked or given up.
pub trait Picker {
    fn pick(&mut self, view: &PickView<'_>, session: &mut Session) -> Result<Outcome, PortError>;
}
