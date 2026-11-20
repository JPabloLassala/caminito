//! The Windows page's keys (ADR 0057, CMN-305): `org.mate.Caminito`'s
//! `fullscreen-workspaces`, which Caminito follows as it changes, on windows already
//! fullscreen too.

use gio::prelude::*;

pub use crate::animations::Error;

#[cfg(test)]
mod tests;

const FULLSCREEN_WORKSPACES: &str = "fullscreen-workspaces";

/// The Windows page's key.
pub struct Windows {
    settings: gio::Settings,
}

impl Windows {
    /// The key of `settings`, which are `org.mate.Caminito`'s.
    pub fn new(settings: gio::Settings) -> Self {
        Windows { settings }
    }

    /// Opens the installed `org.mate.Caminito`.
    pub fn open() -> Result<Self, Error> {
        crate::installed().map(Windows::new).ok_or(Error::NoSchema)
    }

    /// Whether each fullscreen window has a Fullscreen workspace of its own.
    pub fn fullscreen_workspaces(&self) -> bool {
        self.settings.boolean(FULLSCREEN_WORKSPACES)
    }

    /// Writes the switch at once: there's no Apply.
    pub fn set_fullscreen_workspaces(&self, on: bool) -> Result<(), Error> {
        self.settings
            .set_boolean(FULLSCREEN_WORKSPACES, on)
            .map_err(|err| Error::Key(FULLSCREEN_WORKSPACES, err))
    }

    /// Calls `f` with the key each time it changes, as by `gsettings` or by
    /// [`set_fullscreen_workspaces`](Self::set_fullscreen_workspaces).
    pub fn connect_changed(&self, f: impl Fn(bool) + 'static) {
        self.settings
            .connect_changed(Some(FULLSCREEN_WORKSPACES), move |settings, _| {
                f(settings.boolean(FULLSCREEN_WORKSPACES));
            });
    }
}
