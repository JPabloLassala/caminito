//! The store in GSettings (210): `org.mate.Caminito.wallpaper`, and one relocatable
//! `org.mate.Caminito.wallpaper.output` per monitor under its path, and one more for every
//! output's. MATE's `org.mate.background` is read for the first run.

use std::cell::RefCell;
use std::collections::HashMap;

use gio::glib;
use gio::prelude::*;

use crate::positioning::Positioning;
use crate::settings::{Colour, Kind, MateBackground, OutputSettings, Set, Store};

/// The schema of the keys that aren't an output's.
pub const SCHEMA: &str = "org.mate.Caminito.wallpaper";
/// The relocatable schema of an output's keys.
pub const OUTPUT_SCHEMA: &str = "org.mate.Caminito.wallpaper.output";
/// The path every output's keys are at, and below which each monitor's are.
pub const PATH: &str = "/org/mate/caminito/wallpaper/";
/// MATE's background, which the first run reads.
pub const MATE_SCHEMA: &str = "org.mate.background";

/// The path of a set of an output's keys.
pub fn path(set: &Set) -> String {
    match set {
        Set::All => format!("{PATH}all/"),
        Set::Output(identity) => format!("{PATH}outputs/{}/", identity.path_element()),
    }
}

/// Why the store can't be opened.
#[derive(Debug)]
pub enum Error {
    /// A schema isn't installed, or isn't compiled.
    NoSchema(&'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NoSchema(id) => write!(f, "the schema {id} isn't installed"),
        }
    }
}

impl std::error::Error for Error {}

/// The store in GSettings. Each set's `Settings` is kept once made, so that its changes
/// are signalled.
pub struct GioStore {
    main: gio::Settings,
    sets: RefCell<HashMap<String, gio::Settings>>,
    /// Called with every set made, for its `changed` signal.
    watch: Box<dyn Fn(&gio::Settings)>,
}

/// Whether the schema `id` is installed.
fn has_schema(id: &str) -> bool {
    gio::SettingsSchemaSource::default().is_some_and(|source| source.lookup(id, true).is_some())
}

impl GioStore {
    /// Opens the store. `watch` is given each `Settings` the store reads, the main one
    /// first, so that the drawer hears of every change.
    pub fn open(watch: impl Fn(&gio::Settings) + 'static) -> Result<Self, Error> {
        for id in [SCHEMA, OUTPUT_SCHEMA] {
            if !has_schema(id) {
                return Err(Error::NoSchema(id));
            }
        }
        let main = gio::Settings::new(SCHEMA);
        watch(&main);
        Ok(GioStore {
            main,
            sets: RefCell::new(HashMap::new()),
            watch: Box::new(watch),
        })
    }

    /// The `Settings` of `set`, made and watched the first time.
    fn settings(&self, set: &Set) -> gio::Settings {
        let path = path(set);
        let mut sets = self.sets.borrow_mut();
        if let Some(settings) = sets.get(&path) {
            return settings.clone();
        }
        let settings = gio::Settings::with_path(OUTPUT_SCHEMA, &path);
        (self.watch)(&settings);
        sets.insert(path, settings.clone());
        settings
    }

    /// MATE's background, or `None` if its schema isn't installed.
    pub fn mate_background() -> Option<MateBackground> {
        if !has_schema(MATE_SCHEMA) {
            return None;
        }
        let mate = gio::Settings::new(MATE_SCHEMA);
        Some(MateBackground {
            picture: mate.string("picture-filename").into(),
            options: mate.string("picture-options").into(),
            colour: mate.string("primary-color").into(),
        })
    }
}

impl Store for GioStore {
    fn same_on_all_outputs(&self) -> bool {
        self.main.boolean("same-on-all-outputs")
    }

    fn seen(&self) -> Vec<String> {
        self.main
            .strv("outputs")
            .iter()
            .map(|element| element.as_str().to_owned())
            .collect()
    }

    fn set_seen(&mut self, seen: &[String]) {
        let seen: Vec<&str> = seen.iter().map(String::as_str).collect();
        if let Err(err) = self.main.set_strv("outputs", seen.as_slice()) {
            glib::g_warning!("caminito-wallpaperd", "can't note the monitors seen: {err}");
        }
    }

    fn read(&self, set: &Set) -> OutputSettings {
        let settings = self.settings(set);
        let colour: String = settings.string("colour").into();
        OutputSettings {
            kind: Kind::from_nick(&settings.string("kind")),
            picture: settings.string("picture").into(),
            positioning: Positioning::from_nick(&settings.string("positioning")),
            colour: Colour::parse(&colour).unwrap_or_default(),
        }
    }

    fn write(&mut self, set: &Set, written: &OutputSettings) {
        let settings = self.settings(set);
        settings.delay();
        let results = [
            settings.set_string("kind", written.kind.nick()),
            settings.set_string("picture", &written.picture),
            settings.set_string("positioning", written.positioning.nick()),
            settings.set_string("colour", &written.colour.text()),
        ];
        settings.apply();
        if let Some(Err(err)) = results.into_iter().find(Result::is_err) {
            glib::g_warning!("caminito-wallpaperd", "can't write {}: {err}", path(set));
        }
    }

    fn took_mate_background(&self) -> bool {
        self.main.boolean("took-mate-background")
    }

    fn set_took_mate_background(&mut self) {
        if let Err(err) = self.main.set_boolean("took-mate-background", true) {
            glib::g_warning!("caminito-wallpaperd", "can't note the first run: {err}");
        }
    }
}
