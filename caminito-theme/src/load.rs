// SPDX-FileCopyrightText: 2001 Havoc Pennington
// SPDX-FileCopyrightText: 2025-2026 Juan Pablo Lassala
// SPDX-License-Identifier: GPL-3.0-or-later AND GPL-2.0-or-later

//! Finding and loading a theme by name, as Marco's `meta_theme_load` does
//! (`theme-parser.c:4338`), over the XDG data directories (09).

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use crate::error::ThemeError;
use crate::model::Theme;

/// The data directories themes are looked up in, in order: each has a
/// `themes/<name>/metacity-1/` for a theme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeDirs(Vec<PathBuf>);

impl ThemeDirs {
    pub fn new(dirs: Vec<PathBuf>) -> Self {
        ThemeDirs(dirs)
    }

    /// `$XDG_DATA_HOME`, then each of `$XDG_DATA_DIRS`, with the spec's defaults.
    pub fn from_env() -> Self {
        ThemeDirs::from_vars(
            env::var_os("XDG_DATA_HOME"),
            env::var_os("HOME"),
            env::var_os("XDG_DATA_DIRS"),
        )
    }

    /// [`ThemeDirs::from_env`] from the variables' values. An empty variable counts as
    /// unset.
    pub fn from_vars(
        xdg_data_home: Option<OsString>,
        home: Option<OsString>,
        xdg_data_dirs: Option<OsString>,
    ) -> Self {
        let set = |value: Option<OsString>| value.filter(|value| !value.is_empty());
        let mut dirs = Vec::new();
        match set(xdg_data_home) {
            Some(data_home) => dirs.push(PathBuf::from(data_home)),
            None => {
                if let Some(home) = set(home) {
                    dirs.push(PathBuf::from(home).join(".local/share"));
                }
            }
        }
        let data_dirs =
            set(xdg_data_dirs).unwrap_or_else(|| OsString::from("/usr/local/share:/usr/share"));
        dirs.extend(env::split_paths(&data_dirs).filter(|dir| !dir.as_os_str().is_empty()));
        ThemeDirs(dirs)
    }

    pub fn dirs(&self) -> &[PathBuf] {
        &self.0
    }
}

impl Theme {
    /// Loads a theme as Marco does: the newest `metacity-theme-<N>.xml` it ships, up to
    /// format 3, looking in each directory in turn. A file that can't be read, or whose
    /// root needs a newer Marco, is passed over. Any other error ends the search.
    pub fn load(name: &str, dirs: &ThemeDirs) -> Result<Theme, ThemeError> {
        for major in (1..=3).rev() {
            for dir in dirs.dirs() {
                let path = dir
                    .join("themes")
                    .join(name)
                    .join("metacity-1")
                    .join(format!("metacity-theme-{major}.xml"));
                let Ok(text) = std::fs::read(&path) else {
                    continue;
                };
                match Theme::parse(&text, name, &path, major) {
                    Err(ThemeError::UnsupportedVersion { .. }) => continue,
                    result => return result,
                }
            }
        }
        Err(ThemeError::NotFound {
            name: name.to_owned(),
        })
    }
}
