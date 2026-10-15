//! The lock screen's file (218): what each output shows, for caminito-locker, which reads it
//! and never asks the drawer, so that the lock comes up when the drawer has crashed or hung.
//!
//! It's a GLib key file at `$XDG_RUNTIME_DIR/caminito-wallpaper/lock-screen`, with a group
//! per output named by its connector. GTK 3, the locker's, tells a monitor by its make,
//! model and place, not its connector, so each group has those, then the picture to show
//! and how, under `org.mate.background`'s names:
//!
//! ```ini
//! [DP-1]
//! make=Dell Inc.
//! model=DELL U2720Q
//! x=0
//! y=0
//! picture-filename=/usr/share/backgrounds/cosmos/comet.jpg
//! picture-options=zoom
//! primary-color=#2c001e
//! ```
//!
//! `picture-filename` is the picture that shows most, so a dynamic wallpaper part way is
//! one of its two (216), and empty for a colour. The file is written whole to a temporary
//! file and renamed over the last, so a reader has the old one or the new one.

use std::path::{Path, PathBuf};

use gio::glib;

use crate::settings::Drawn;

/// An output as the locker tells it, and what it shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    pub connector: String,
    pub make: String,
    pub model: String,
    /// Its place in the layout, in logical pixels.
    pub x: i32,
    pub y: i32,
    pub drawn: Drawn,
}

/// Where the file is: in the session's runtime directory, gone with the session.
pub fn path() -> PathBuf {
    glib::user_runtime_dir()
        .join("caminito-wallpaper")
        .join("lock-screen")
}

/// The file's text for `outputs`.
pub fn text(outputs: &[Shown]) -> String {
    let file = glib::KeyFile::new();
    for output in outputs {
        let group = output.connector.as_str();
        file.set_string(group, "make", &output.make);
        file.set_string(group, "model", &output.model);
        file.set_integer(group, "x", output.x);
        file.set_integer(group, "y", output.y);
        let (picture, options) = match &output.drawn {
            Drawn::Picture { positioning, .. } | Drawn::Blend { positioning, .. } => (
                output.drawn.nearer().unwrap_or_default(),
                positioning.mate(),
            ),
            // MATE's, with no picture to place.
            Drawn::Colour(_) => ("", "zoom"),
        };
        file.set_string(group, "picture-filename", picture);
        file.set_string(group, "picture-options", options);
        file.set_string(group, "primary-color", &output.drawn.colour().text());
    }
    file.to_data().into()
}

/// Writes `text` to `path` whole, or not at all, making its directory if need be.
pub fn write(path: &Path, text: &str) -> Result<(), glib::Error> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|err| {
            glib::Error::new(
                glib::FileError::Failed,
                &format!("can't make {}: {err}", dir.display()),
            )
        })?;
    }
    // GLib's writes a temporary file beside it, then renames it over the last.
    glib::file_set_contents(path, text.as_bytes())
}

#[cfg(test)]
mod tests;
