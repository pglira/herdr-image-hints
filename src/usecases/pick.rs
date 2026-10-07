//! `pick`: read the pane, label the image paths, let the user choose, and
//! show the chosen image in a popup.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use thiserror::Error;

use super::ports::{Files, PaneHost, PickView, Picker, PortError};
use crate::domain::geometry::OverlayGeometry;
use crate::domain::matcher::find_candidates;
use crate::domain::screen::Screen;
use crate::domain::session::{Outcome, Session};
use crate::domain::settings::Settings;

/// The popup pane declared in `herdr-plugin.toml` that draws the image.
pub const VIEWER_ENTRYPOINT: &str = "viewer";
/// Environment variable carrying the image path from `ui` to `view`.
pub const IMAGE_ENV: &str = "HERDR_IMAGE_HINTS_FILE";

/// What one run of the overlay works on.
pub struct PickRequest<'a> {
    pub plugin_id: &'a str,
    pub pane_id: &'a str,
    /// Where the pane sits in the overlay; `None` when launched by hand.
    pub geometry: Option<&'a OverlayGeometry>,
    pub settings: &'a Settings,
    /// A one-line notice to show (for instance a configuration problem).
    pub notice: Option<&'a str>,
}

/// The adapters one run of the overlay goes through.
pub struct Deps<'a> {
    pub host: &'a dyn PaneHost,
    pub files: &'a dyn Files,
    pub picker: &'a mut dyn Picker,
}

#[derive(Debug, Error)]
pub enum PickError {
    #[error(transparent)]
    Port(#[from] PortError),
}

/// How the run ended, with a line for the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Picked {
    Cancelled,
    Done(String),
}

pub fn pick(request: &PickRequest<'_>, deps: &mut Deps<'_>) -> Result<Picked, PickError> {
    let width = match request.geometry {
        Some(geometry) => geometry.pane.width,
        None => deps
            .host
            .layout(request.pane_id)
            .ok()
            .and_then(|layout| {
                layout
                    .panes
                    .into_iter()
                    .find(|p| p.pane_id == request.pane_id)
            })
            .map_or(0, |p| p.rect.width),
    };
    let screen = Screen::from_ansi(&deps.host.read_visible(request.pane_id)?, width);
    let candidates = find_candidates(&screen, &request.settings.patterns);
    let mut session = Session::new(candidates, &request.settings.alphabet);
    let view = PickView {
        screen: &screen,
        theme: &request.settings.theme,
        geometry: request.geometry,
        notice: request.notice,
    };
    let selection = match deps.picker.pick(&view, &mut session)? {
        Outcome::Picked(selection) => selection,
        Outcome::Cancelled | Outcome::Continue => return Ok(Picked::Cancelled),
    };
    let Some(text) = selection.texts.first() else {
        return Ok(Picked::Cancelled);
    };
    let cwd = deps.host.pane_cwd(request.pane_id).unwrap_or(None);
    let path = resolve(text, cwd.as_deref(), deps.files.home().as_deref());
    if !deps.files.is_file(&path) {
        deps.host
            .notify("Image not found", &path.display().to_string())?;
        return Ok(Picked::Done(format!("not found: {}", path.display())));
    }
    let env = BTreeMap::from([(IMAGE_ENV.to_string(), path.display().to_string())]);
    deps.host.open_popup(
        request.plugin_id,
        VIEWER_ENTRYPOINT,
        env,
        &request.settings.popup,
    )?;
    Ok(Picked::Done(format!("showed {}", path.display())))
}

/// Turns the text on screen into a path: `~/` goes under `home`, a relative
/// path under the pane's working directory, an absolute path stays.
pub fn resolve(text: &str, cwd: Option<&Path>, home: Option<&Path>) -> PathBuf {
    if let (Some(rest), Some(home)) = (text.strip_prefix("~/"), home) {
        return home.join(rest);
    }
    let path = Path::new(text);
    match cwd {
        Some(cwd) if path.is_relative() => cwd.join(path),
        _ => path.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::alphabet::Alphabet;
    use crate::domain::geometry::Rect;
    use crate::domain::session::{Key, Modifier};
    use crate::domain::settings::PopupSize;
    use crate::usecases::testing::{FakeFiles, FakeHost, ScriptedPicker};

    const SCREEN: &str = "wrote plots/loss.png\nsaved /tmp/shot.jpg\n";

    fn settings() -> Settings {
        Settings {
            alphabet: Alphabet::custom("asdf").unwrap(),
            ..Settings::default()
        }
    }

    fn geometry() -> OverlayGeometry {
        OverlayGeometry {
            pane_id: "w1:p1".into(),
            area: Rect::new(0, 0, 80, 24),
            pane: Rect::new(0, 0, 80, 24),
        }
    }

    struct World {
        host: FakeHost,
        files: FakeFiles,
        picker: ScriptedPicker,
    }

    impl World {
        fn new(keys: &[Key]) -> Self {
            World {
                host: FakeHost::showing(SCREEN).in_directory("/work/repo"),
                files: FakeFiles::with(&["/work/repo/plots/loss.png", "/tmp/shot.jpg"]),
                picker: ScriptedPicker::new(keys),
            }
        }

        fn run(&mut self, settings: &Settings, notice: Option<&str>) -> Picked {
            let geometry = geometry();
            let request = PickRequest {
                plugin_id: "pglira.herdr-image-hints",
                pane_id: "w1:p1",
                geometry: Some(&geometry),
                settings,
                notice,
            };
            let mut deps = Deps {
                host: &self.host,
                files: &self.files,
                picker: &mut self.picker,
            };
            pick(&request, &mut deps).unwrap()
        }

        fn shown(&self) -> Vec<String> {
            self.host
                .popups
                .borrow()
                .iter()
                .map(|(_, _, env, _)| env[IMAGE_ENV].clone())
                .collect()
        }
    }

    // On SCREEN with alphabet "asdf": the bottom match (/tmp/shot.jpg) is
    // "a", the relative path above it is "s".

    #[test]
    fn typing_a_hint_opens_the_viewer_popup_on_that_image() {
        let mut world = World::new(&[Key::Hint('a', Modifier::Main)]);
        let picked = world.run(&settings(), None);
        assert_eq!(picked, Picked::Done("showed /tmp/shot.jpg".into()));
        let popups = world.host.popups.borrow();
        assert_eq!(popups.len(), 1);
        let (plugin, entrypoint, _, size) = &popups[0];
        assert_eq!(plugin, "pglira.herdr-image-hints");
        assert_eq!(entrypoint, VIEWER_ENTRYPOINT);
        assert_eq!(size, &PopupSize::default());
    }

    #[test]
    fn a_relative_path_resolves_against_the_pane_working_directory() {
        let mut world = World::new(&[Key::Hint('s', Modifier::Main)]);
        world.run(&settings(), None);
        assert_eq!(world.shown(), vec!["/work/repo/plots/loss.png"]);
    }

    #[test]
    fn a_modifier_held_with_the_hint_changes_nothing() {
        let mut world = World::new(&[Key::Hint('s', Modifier::Ctrl)]);
        world.run(&settings(), None);
        assert_eq!(world.shown(), vec!["/work/repo/plots/loss.png"]);
    }

    #[test]
    fn a_missing_file_is_reported_with_a_toast_and_opens_nothing() {
        let mut world = World::new(&[Key::Hint('a', Modifier::Main)]);
        world.files = FakeFiles::with(&[]);
        let picked = world.run(&settings(), None);
        assert_eq!(picked, Picked::Done("not found: /tmp/shot.jpg".into()));
        assert!(world.host.popups.borrow().is_empty());
        assert_eq!(
            *world.host.notified.borrow(),
            vec![("Image not found".to_string(), "/tmp/shot.jpg".to_string())]
        );
    }

    #[test]
    fn cancelling_opens_nothing() {
        let mut world = World::new(&[Key::Escape]);
        assert_eq!(world.run(&settings(), None), Picked::Cancelled);
        assert!(world.host.popups.borrow().is_empty());
    }

    #[test]
    fn the_notice_and_geometry_reach_the_picker() {
        let mut world = World::new(&[Key::Escape]);
        world.run(&settings(), Some("config.toml ignored"));
        assert_eq!(world.picker.seen_notice, Some("config.toml ignored".into()));
        assert_eq!(world.picker.seen_geometry, Some(geometry()));
        assert!(world.host.layout_calls.borrow().is_empty());
    }

    #[test]
    fn a_host_that_cannot_read_the_pane_is_an_error() {
        let mut world = World::new(&[Key::Escape]);
        world.host = FakeHost::showing(SCREEN).failing_reads();
        let settings = settings();
        let request = PickRequest {
            plugin_id: "id",
            pane_id: "w1:p1",
            geometry: None,
            settings: &settings,
            notice: None,
        };
        let mut deps = Deps {
            host: &world.host,
            files: &world.files,
            picker: &mut world.picker,
        };
        assert!(matches!(pick(&request, &mut deps), Err(PickError::Port(_))));
    }

    #[test]
    fn paths_resolve_against_home_and_the_working_directory() {
        let home = Some(Path::new("/home/u"));
        let cwd = Some(Path::new("/w"));
        assert_eq!(
            resolve("~/a.png", cwd, home),
            PathBuf::from("/home/u/a.png")
        );
        assert_eq!(resolve("b/c.png", cwd, home), PathBuf::from("/w/b/c.png"));
        assert_eq!(resolve("/abs.png", cwd, home), PathBuf::from("/abs.png"));
        assert_eq!(resolve("rel.png", None, home), PathBuf::from("rel.png"));
    }
}
