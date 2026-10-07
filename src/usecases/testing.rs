//! In-memory stand-ins for the ports, shared by the use-case tests.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::ports::{Files, PaneHost, PickView, Picker, PortError};
use crate::domain::geometry::{Layout, OverlayGeometry, PanePlacement, Rect};
use crate::domain::session::{Key, Outcome, Session};
use crate::domain::settings::PopupSize;

/// One `open_overlay` call: plugin id, entrypoint, environment.
pub(crate) type OpenedOverlay = (String, String, BTreeMap<String, String>);

/// One `open_popup` call: plugin id, entrypoint, environment, size.
pub(crate) type OpenedPopup = (String, String, BTreeMap<String, String>, PopupSize);

/// A host showing a fixed screen and recording everything asked of it.
pub(crate) struct FakeHost {
    pub screen: String,
    pub layout: Option<Layout>,
    pub label: Option<String>,
    pub cwd: Option<PathBuf>,
    pub reads_fail: bool,
    pub layout_calls: RefCell<Vec<String>>,
    pub popups: RefCell<Vec<OpenedPopup>>,
    pub notified: RefCell<Vec<(String, String)>>,
    pub opened: RefCell<Vec<OpenedOverlay>>,
}

impl FakeHost {
    pub fn showing(screen: &str) -> Self {
        FakeHost {
            screen: screen.to_string(),
            layout: None,
            label: None,
            cwd: None,
            reads_fail: false,
            layout_calls: RefCell::new(Vec::new()),
            popups: RefCell::new(Vec::new()),
            notified: RefCell::new(Vec::new()),
            opened: RefCell::new(Vec::new()),
        }
    }

    /// A 100×40 tab split in two, `pane_id` being the right half.
    pub fn split_layout(pane_id: &str) -> Layout {
        Layout {
            area: Rect::new(0, 0, 100, 40),
            zoomed: false,
            panes: vec![
                PanePlacement {
                    pane_id: "w1:p1".into(),
                    rect: Rect::new(0, 0, 50, 40),
                },
                PanePlacement {
                    pane_id: pane_id.into(),
                    rect: Rect::new(51, 0, 49, 40),
                },
            ],
        }
    }

    pub fn with_split_layout(pane_id: &str) -> Self {
        FakeHost::showing("").with_layout(FakeHost::split_layout(pane_id))
    }

    pub fn with_layout(mut self, layout: Layout) -> Self {
        self.layout = Some(layout);
        self
    }

    pub fn labelled(mut self, label: &str) -> Self {
        self.label = Some(label.to_string());
        self
    }

    pub fn in_directory(mut self, cwd: &str) -> Self {
        self.cwd = Some(PathBuf::from(cwd));
        self
    }

    pub fn failing_reads(mut self) -> Self {
        self.reads_fail = true;
        self
    }
}

impl PaneHost for FakeHost {
    fn layout(&self, pane_id: &str) -> Result<Layout, PortError> {
        self.layout_calls.borrow_mut().push(pane_id.to_string());
        self.layout
            .clone()
            .ok_or_else(|| PortError::new("no layout"))
    }

    fn read_visible(&self, _pane_id: &str) -> Result<String, PortError> {
        if self.reads_fail {
            return Err(PortError::new("pane not found"));
        }
        Ok(self.screen.clone())
    }

    fn pane_label(&self, _pane_id: &str) -> Result<Option<String>, PortError> {
        Ok(self.label.clone())
    }

    fn pane_cwd(&self, _pane_id: &str) -> Result<Option<PathBuf>, PortError> {
        Ok(self.cwd.clone())
    }

    fn open_overlay(
        &self,
        plugin_id: &str,
        entrypoint: &str,
        env: BTreeMap<String, String>,
    ) -> Result<String, PortError> {
        self.opened
            .borrow_mut()
            .push((plugin_id.to_string(), entrypoint.to_string(), env));
        Ok("w1:p9".to_string())
    }

    fn open_popup(
        &self,
        plugin_id: &str,
        entrypoint: &str,
        env: BTreeMap<String, String>,
        size: &PopupSize,
    ) -> Result<(), PortError> {
        self.popups.borrow_mut().push((
            plugin_id.to_string(),
            entrypoint.to_string(),
            env,
            size.clone(),
        ));
        Ok(())
    }

    fn notify(&self, title: &str, body: &str) -> Result<(), PortError> {
        self.notified
            .borrow_mut()
            .push((title.to_string(), body.to_string()));
        Ok(())
    }
}

/// A file system holding a fixed set of files under a fixed home.
pub(crate) struct FakeFiles {
    pub files: Vec<PathBuf>,
    pub home: Option<PathBuf>,
}

impl FakeFiles {
    pub fn with(files: &[&str]) -> Self {
        FakeFiles {
            files: files.iter().map(PathBuf::from).collect(),
            home: Some(PathBuf::from("/home/u")),
        }
    }
}

impl Files for FakeFiles {
    fn is_file(&self, path: &Path) -> bool {
        self.files.iter().any(|f| f == path)
    }

    fn home(&self) -> Option<PathBuf> {
        self.home.clone()
    }
}

/// Feeds a fixed key sequence to the real session; cancels if it runs out.
pub(crate) struct ScriptedPicker {
    keys: Vec<Key>,
    pub seen_notice: Option<String>,
    pub seen_geometry: Option<OverlayGeometry>,
}

impl ScriptedPicker {
    pub fn new(keys: &[Key]) -> Self {
        ScriptedPicker {
            keys: keys.to_vec(),
            seen_notice: None,
            seen_geometry: None,
        }
    }
}

impl Picker for ScriptedPicker {
    fn pick(&mut self, view: &PickView<'_>, session: &mut Session) -> Result<Outcome, PortError> {
        self.seen_notice = view.notice.map(str::to_string);
        self.seen_geometry = view.geometry.cloned();
        for key in &self.keys {
            match session.press(*key) {
                Outcome::Continue => {}
                other => return Ok(other),
            }
        }
        Ok(Outcome::Cancelled)
    }
}
