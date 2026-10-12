//! The store in GSettings (210): `org.mate.Caminito.wallpaper`, and one relocatable
//! `org.mate.Caminito.wallpaper.output` per monitor under its path, and one more for every
//! output's. MATE's `org.mate.background` is read for the first run, and bridged to after
//! (211).

use std::cell::RefCell;
use std::collections::HashMap;

use gio::glib;
use gio::prelude::*;

use crate::bridge::Writes;
use crate::positioning::Positioning;
use crate::settings::{Colour, Kind, MateBackground, OutputSettings, Set, Store};

/// The schema of the keys that aren't an output's.
pub const SCHEMA: &str = "org.mate.Caminito.wallpaper";
/// The relocatable schema of an output's keys.
pub const OUTPUT_SCHEMA: &str = "org.mate.Caminito.wallpaper.output";
/// The path every output's keys are at, and below which each monitor's are.
pub const PATH: &str = "/org/mate/caminito/wallpaper/";
/// MATE's background, which the first run reads and the bridges write and hear.
pub const MATE_SCHEMA: &str = "org.mate.background";

/// The path of a set of an output's keys.
pub fn path(set: &Set) -> String {
    match set {
        Set::All => format!("{PATH}all/"),
        Set::Output(identity) => format!("{PATH}outputs/{}/", identity.path_element()),
        Set::Seen(element) => format!("{PATH}outputs/{element}/"),
    }
}

/// Why the store can't be opened, or MATE's keys can't be written.
#[derive(Debug)]
pub enum Error {
    /// A schema isn't installed, or isn't compiled.
    NoSchema(&'static str),
    /// A key of `org.mate.background` can't be written: it's locked down, or the backend
    /// failed (211).
    MateKey(&'static str, glib::BoolError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NoSchema(id) => write!(f, "the schema {id} isn't installed"),
            Error::MateKey(key, err) => write!(f, "can't write org.mate.background's {key}: {err}"),
        }
    }
}

impl std::error::Error for Error {}

/// The store in GSettings. Each set's `Settings` is kept once made, so that its changes
/// are signalled.
pub struct GioStore {
    /// The log domain of the program that opened it.
    domain: &'static str,
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
    /// Opens the store, which logs under `domain`. `watch` is given each `Settings` the
    /// store reads, the main one first, so that the drawer and the dialog hear of every
    /// change.
    pub fn open(
        domain: &'static str,
        watch: impl Fn(&gio::Settings) + 'static,
    ) -> Result<Self, Error> {
        for id in [SCHEMA, OUTPUT_SCHEMA] {
            if !has_schema(id) {
                return Err(Error::NoSchema(id));
            }
        }
        let main = gio::Settings::new(SCHEMA);
        watch(&main);
        Ok(GioStore {
            domain,
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

    /// A list of the main schema's.
    fn list(&self, key: &str) -> Vec<String> {
        self.main
            .strv(key)
            .iter()
            .map(|element| element.as_str().to_owned())
            .collect()
    }

    fn set_list(&self, key: &str, list: &[String]) {
        let list: Vec<&str> = list.iter().map(String::as_str).collect();
        if let Err(err) = self.main.set_strv(key, list.as_slice()) {
            glib::g_warning!(self.domain, "can't write {key}: {err}");
        }
    }
}

impl Store for GioStore {
    fn same_on_all_outputs(&self) -> bool {
        self.main.boolean("same-on-all-outputs")
    }

    fn set_same_on_all_outputs(&mut self, same: bool) {
        if let Err(err) = self.main.set_boolean("same-on-all-outputs", same) {
            glib::g_warning!(self.domain, "can't set same-on-all-outputs: {err}");
        }
    }

    fn seen(&self) -> Vec<String> {
        self.list("outputs")
    }

    fn set_seen(&mut self, seen: &[String]) {
        self.set_list("outputs", seen);
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
            glib::g_warning!(self.domain, "can't write {}: {err}", path(set));
        }
    }

    fn took_mate_background(&self) -> bool {
        self.main.boolean("took-mate-background")
    }

    fn set_took_mate_background(&mut self) {
        if let Err(err) = self.main.set_boolean("took-mate-background", true) {
            glib::g_warning!(self.domain, "can't note the first run: {err}");
        }
    }

    fn pictures(&self) -> Vec<String> {
        self.list("pictures")
    }

    fn set_pictures(&mut self, pictures: &[String]) {
        self.set_list("pictures", pictures);
    }

    fn folders(&self) -> Vec<String> {
        self.list("folders")
    }

    fn set_folders(&mut self, folders: &[String]) {
        self.set_list("folders", folders);
    }
}

/// MATE's background, `org.mate.background`, as the first run and the bridges see it (211).
pub struct Mate {
    settings: gio::Settings,
}

impl Mate {
    /// Opens MATE's keys, or `None` if their schema isn't installed. `heard` is given
    /// `picture-filename` each time it changes, whoever wrote it, the drawer included.
    pub fn open(heard: impl Fn(&str) + 'static) -> Option<Self> {
        if !has_schema(MATE_SCHEMA) {
            return None;
        }
        let settings = gio::Settings::new(MATE_SCHEMA);
        settings.connect_changed(Some("picture-filename"), move |settings, key| {
            heard(&settings.string(key));
        });
        Some(Mate { settings })
    }

    /// The keys the first run and the bridges read.
    pub fn read(&self) -> MateBackground {
        MateBackground {
            picture: self.settings.string("picture-filename").into(),
            options: self.settings.string("picture-options").into(),
            colour: self.settings.string("primary-color").into(),
        }
    }

    /// Writes the keys in `writes`, and no other, since a key written signals a change even
    /// when its value is the same. A colour is solid, as Caminito's are.
    pub fn write(&self, writes: &Writes) -> Result<(), Error> {
        let set = |key: &'static str, value: &str| {
            self.settings
                .set_string(key, value)
                .map_err(|err| Error::MateKey(key, err))
        };
        if let Some(options) = writes.options {
            set("picture-options", options)?;
        }
        if let Some(colour) = writes.colour {
            set("primary-color", &colour.text())?;
            if self.settings.string("color-shading-type") != "solid" {
                set("color-shading-type", "solid")?;
            }
        }
        // Last, so that a reader hearing it finds the options and colour already its.
        if let Some(picture) = &writes.picture {
            set("picture-filename", picture)?;
        }
        Ok(())
    }
}
