//! The Effects page's keys (ADR 0048): `org.mate.Caminito`'s `esmeril`, `esmeril-strength`
//! and `esmeril-applications`, which Caminito follows as they change, and Marco's
//! `reduced-resources`, which turns Esmeril off and which the page only reads.

use gio::prelude::*;

pub use crate::animations::Error;

#[cfg(test)]
mod tests;

const ON: &str = "esmeril";
const STRENGTH: &str = "esmeril-strength";
const APPLICATIONS: &str = "esmeril-applications";
const REDUCED_RESOURCES: &str = "reduced-resources";

/// The weakest and the strongest blur, as the schema's range has them.
pub const STRENGTHS: std::ops::RangeInclusive<u32> = 1..=15;

/// One of Esmeril's keys in `org.mate.Caminito`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// The `esmeril` switch.
    On,
    /// `esmeril-strength`.
    Strength,
    /// `esmeril-applications`.
    Applications,
}

/// Esmeril's keys, and Marco's `reduced-resources`, whose schema may be missing: Caminito
/// takes its default then, which is off.
pub struct Esmeril {
    settings: gio::Settings,
    marco: Option<gio::Settings>,
}

impl Esmeril {
    /// The keys of `settings`, which are `org.mate.Caminito`'s, with `org.mate.Marco.general`
    /// if it's there.
    pub fn new(settings: gio::Settings, marco: Option<gio::Settings>) -> Self {
        Esmeril { settings, marco }
    }

    /// Opens the installed schemas.
    pub fn open() -> Result<Self, Error> {
        let settings = crate::installed().ok_or(Error::NoSchema)?;
        Ok(Esmeril::new(
            settings,
            crate::installed_schema("org.mate.Marco.general"),
        ))
    }

    /// Whether Esmeril is on.
    pub fn is_on(&self) -> bool {
        self.settings.boolean(ON)
    }

    /// Writes the switch at once: there's no Apply.
    pub fn set_on(&self, on: bool) -> Result<(), Error> {
        self.settings
            .set_boolean(ON, on)
            .map_err(|err| Error::Key(ON, err))
    }

    /// How strong the blur is, from 1 to 15.
    pub fn strength(&self) -> u32 {
        self.settings.uint(STRENGTH)
    }

    /// Writes the strength at once. One outside 1 to 15 is refused, as the schema's range
    /// would, and GSettings aborts on it.
    pub fn set_strength(&self, strength: u32) -> Result<(), Error> {
        if !STRENGTHS.contains(&strength) {
            return Err(Error::Key(
                STRENGTH,
                gio::glib::bool_error!("{strength} is outside 1 to 15"),
            ));
        }
        self.settings
            .set_uint(STRENGTH, strength)
            .map_err(|err| Error::Key(STRENGTH, err))
    }

    /// The app ids of the listed applications, in the key's order.
    pub fn applications(&self) -> Vec<String> {
        self.settings
            .strv(APPLICATIONS)
            .iter()
            .map(|app_id| app_id.to_string())
            .collect()
    }

    /// Writes the list at once.
    pub fn set_applications(&self, app_ids: &[String]) -> Result<(), Error> {
        let app_ids: Vec<&str> = app_ids.iter().map(String::as_str).collect();
        self.settings
            .set_strv(APPLICATIONS, app_ids.as_slice())
            .map_err(|err| Error::Key(APPLICATIONS, err))
    }

    /// Whether `app_id` is listed.
    pub fn is_listed(&self, app_id: &str) -> bool {
        self.applications().iter().any(|each| each == app_id)
    }

    /// Lists `app_id` or takes it off the list, and writes the list.
    pub fn set_listed(&self, app_id: &str, listed: bool) -> Result<(), Error> {
        let mut app_ids = self.applications();
        match (app_ids.iter().position(|each| each == app_id), listed) {
            (None, true) => app_ids.push(app_id.to_owned()),
            (Some(index), false) => {
                app_ids.remove(index);
            }
            _ => return Ok(()),
        }
        self.set_applications(&app_ids)
    }

    /// Calls `f` with the key that changed each time one of Esmeril's keys changes.
    pub fn connect_changed(&self, f: impl Fn(Key) + 'static) {
        self.settings.connect_changed(None, move |_, key| {
            match key {
                ON => f(Key::On),
                STRENGTH => f(Key::Strength),
                APPLICATIONS => f(Key::Applications),
                _ => {}
            };
        });
    }

    /// Whether `reduced-resources` is on, which turns Esmeril off.
    pub fn reduced_resources(&self) -> bool {
        self.marco
            .as_ref()
            .is_some_and(|marco| marco.boolean(REDUCED_RESOURCES))
    }

    /// Calls `f` with `reduced-resources` each time it changes.
    pub fn connect_reduced_resources_changed(&self, f: impl Fn(bool) + 'static) {
        if let Some(marco) = &self.marco {
            marco.connect_changed(Some(REDUCED_RESOURCES), move |marco, _| {
                f(marco.boolean(REDUCED_RESOURCES));
            });
        }
    }
}
