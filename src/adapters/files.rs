//! The real file system behind the [`Files`] port.

use std::path::{Path, PathBuf};

use crate::usecases::ports::Files;

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemFiles;

impl Files for SystemFiles {
    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn home(&self) -> Option<PathBuf> {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}
