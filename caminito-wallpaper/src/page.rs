//! The dialog's state (212), with no toolkit in it: the outputs in the layout's left-to-right
//! order, the one selected, the kind whose controls show, and every change a click makes,
//! each written to the store at once. There's no Apply.
//!
//! While "same on all outputs" is on, a change is every output's, at `all/`; while it's
//! off, it's the selected output's own.

use crate::daily::Source;
use crate::identity::Identity;
use crate::positioning::Positioning;
use crate::settings::{Colour, Kind, OutputSettings, Set, Store, settings_for};
use crate::slideshow::{self, Interval};

#[cfg(test)]
mod tests;

/// One connected output, as output management describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    /// Its connector, as `DP-1`.
    pub connector: String,
    /// What the user reads for it.
    pub title: String,
    /// What the store knows its monitor by.
    pub identity: Identity,
    /// Its left edge in the layout.
    pub x: i32,
    /// Its top edge in the layout.
    pub y: i32,
    /// Its mode's size, in pixels.
    pub size: (i32, i32),
}

impl Output {
    /// The output at `connector`, whose monitor reports `make`, `model` and `serial`, at
    /// `x`, `y` in the layout, showing a mode of `size` pixels.
    pub fn new(
        connector: &str,
        make: &str,
        model: &str,
        serial: &str,
        (x, y): (i32, i32),
        size: (i32, i32),
    ) -> Self {
        Output {
            connector: connector.to_owned(),
            title: title(connector, make, model),
            identity: Identity::of(make, model, serial, connector),
            x,
            y,
            size,
        }
    }
}

/// What the user reads for a monitor, as caminito-displays has it: a built-in panel as that,
/// another by its make and model, and one whose EDID says neither by its connector.
fn title(connector: &str, make: &str, model: &str) -> String {
    if ["eDP-", "LVDS-", "DSI-"]
        .iter()
        .any(|kind| connector.starts_with(kind))
    {
        return "Built-in display".to_owned();
    }
    let said = |text: &str| {
        let text = text.trim();
        (!text.is_empty() && text != "Unknown").then(|| text.to_owned())
    };
    match (said(make), said(model)) {
        (Some(make), Some(model)) => format!("{make} {model}"),
        (None, Some(model)) => model,
        (Some(make), None) => make,
        (None, None) => connector.to_owned(),
    }
}

/// The dialog's state.
#[derive(Debug, Clone)]
pub struct Page {
    outputs: Vec<Output>,
    selected: usize,
    /// The kind whose controls show.
    kind: Kind,
}

impl Page {
    /// The page of `outputs`, with the output at the connector `primary` selected, or the
    /// leftmost. Each output is given its settings as the drawer gives them, so a monitor
    /// the drawer hasn't seen yet shows what it will.
    pub fn new(
        store: &mut (impl Store + ?Sized),
        mut outputs: Vec<Output>,
        primary: Option<&str>,
    ) -> Self {
        outputs.sort_by_key(|output| (output.x, output.y));
        let primary_identity = primary.and_then(|connector| {
            outputs
                .iter()
                .find(|output| output.connector == connector)
                .map(|output| output.identity.clone())
        });
        for output in &outputs {
            settings_for(store, &output.identity, primary_identity.as_ref());
        }
        let selected = primary
            .and_then(|connector| {
                outputs
                    .iter()
                    .position(|output| output.connector == connector)
            })
            .unwrap_or(0);
        let mut page = Page {
            outputs,
            selected,
            kind: Kind::default(),
        };
        page.kind = page.current(store).kind;
        page
    }

    /// The outputs, left to right.
    pub fn outputs(&self) -> &[Output] {
        &self.outputs
    }

    /// The index of the selected output.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// The kind whose controls show.
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// Whether "Same on all outputs" shows: not with one output, or none.
    pub fn has_switch(&self) -> bool {
        self.outputs.len() > 1
    }

    /// What the output at `index` shows.
    pub fn wallpaper(&self, store: &(impl Store + ?Sized), index: usize) -> OutputSettings {
        match self.outputs.get(index) {
            Some(output) if !store.same_on_all_outputs() => {
                store.read(&Set::Output(output.identity.clone()))
            }
            _ => store.read(&Set::All),
        }
    }

    /// What the selected output shows.
    pub fn current(&self, store: &(impl Store + ?Sized)) -> OutputSettings {
        self.wallpaper(store, self.selected)
    }

    /// The set a change is written to.
    fn set(&self, store: &(impl Store + ?Sized)) -> Set {
        match self.outputs.get(self.selected) {
            Some(output) if !store.same_on_all_outputs() => Set::Output(output.identity.clone()),
            _ => Set::All,
        }
    }

    /// Changes what the selected output shows, and through it every output's while the
    /// switch is on.
    fn change(&self, store: &mut (impl Store + ?Sized), change: impl FnOnce(&mut OutputSettings)) {
        let set = self.set(store);
        let mut settings = store.read(&set);
        change(&mut settings);
        store.write(&set, &settings);
    }

    /// Selects the output at `index`, and shows its kind's controls.
    pub fn select(&mut self, store: &(impl Store + ?Sized), index: usize) {
        if index < self.outputs.len() {
            self.selected = index;
            self.kind = self.current(store).kind;
        }
    }

    /// Turns "Same on all outputs" on or off. On, every output takes the selected one's
    /// wallpaper; off, each keeps the one it shows, to change by itself from then on.
    pub fn set_same_on_all_outputs(&mut self, store: &mut (impl Store + ?Sized), same: bool) {
        if same == store.same_on_all_outputs() {
            return;
        }
        if same {
            let selected = self.current(store);
            store.write(&Set::All, &selected);
        } else {
            let all = store.read(&Set::All);
            for output in &self.outputs {
                store.write(&Set::Output(output.identity.clone()), &all);
            }
        }
        store.set_same_on_all_outputs(same);
    }

    /// Shows `kind`'s controls. A Picture, a Slideshow or a Dynamic with something to show,
    /// a Colour and a Picture of the day, whose source has a default, are the selected
    /// output's at once.
    pub fn show_kind(&mut self, store: &mut (impl Store + ?Sized), kind: Kind) {
        self.kind = kind;
        let applies = match kind {
            Kind::Picture => !self.current(store).picture.is_empty(),
            Kind::Colour | Kind::PictureOfTheDay => true,
            Kind::Slideshow => !self.current(store).slideshow.is_empty(),
            Kind::Dynamic => !self.current(store).dynamic.is_empty(),
        };
        if applies {
            self.change(store, |settings| settings.kind = kind);
        }
    }

    /// Makes `file` the selected output's picture.
    pub fn choose_picture(&mut self, store: &mut (impl Store + ?Sized), file: &str) {
        self.kind = Kind::Picture;
        self.change(store, |settings| {
            settings.kind = Kind::Picture;
            settings.picture = file.to_owned();
        });
    }

    /// Makes `colour` the selected output's, as a Colour.
    pub fn choose_colour(&mut self, store: &mut (impl Store + ?Sized), colour: Colour) {
        self.kind = Kind::Colour;
        self.change(store, |settings| {
            settings.kind = Kind::Colour;
            settings.colour = colour;
        });
    }

    /// Sets how the selected output's picture fits it.
    pub fn choose_positioning(
        &mut self,
        store: &mut (impl Store + ?Sized),
        positioning: Positioning,
    ) {
        self.change(store, |settings| settings.positioning = positioning);
    }

    /// Adds `file` to the pictures the user added, once, and makes it the selected
    /// output's picture.
    pub fn add_picture(&mut self, store: &mut (impl Store + ?Sized), file: &str) {
        let mut pictures = store.pictures();
        if !pictures.iter().any(|picture| picture == file) {
            pictures.push(file.to_owned());
            store.set_pictures(&pictures);
        }
        self.choose_picture(store, file);
    }

    /// Makes the dynamic wallpaper at `path`, a Plasma package's folder or a timed XML, the
    /// selected output's.
    pub fn choose_dynamic(&mut self, store: &mut (impl Store + ?Sized), path: &str) {
        self.kind = Kind::Dynamic;
        self.change(store, |settings| {
            settings.kind = Kind::Dynamic;
            settings.dynamic = path.to_owned();
        });
    }

    /// Adds the timed XML or HEIC `file` to the dynamic wallpapers the user added, once, and
    /// makes it the selected output's. The dialog reads it first, so as to add only one that is.
    pub fn add_dynamic(&mut self, store: &mut (impl Store + ?Sized), file: &str) {
        self.keep_dynamic(store, file);
        self.choose_dynamic(store, file);
    }

    /// Adds `file` to the dynamic wallpapers the user added, once, without making it the
    /// selected output's: a HEIC added without libheif to show it (217).
    pub fn keep_dynamic(&mut self, store: &mut (impl Store + ?Sized), file: &str) {
        let mut dynamics = store.dynamics();
        if !dynamics.iter().any(|added| added == file) {
            dynamics.push(file.to_owned());
            store.set_dynamics(&dynamics);
        }
    }

    /// Makes the pictures of `folder` the selected output's, as a Slideshow (213).
    pub fn choose_slideshow(&mut self, store: &mut (impl Store + ?Sized), folder: &str) {
        self.kind = Kind::Slideshow;
        self.change(store, |settings| {
            settings.kind = Kind::Slideshow;
            settings.slideshow = folder.to_owned();
        });
    }

    /// Sets how long each of the selected output's Slideshow pictures shows.
    pub fn choose_interval(&mut self, store: &mut (impl Store + ?Sized), interval: Interval) {
        self.change(store, |settings| settings.interval = interval);
    }

    /// Sets whether the selected output's Slideshow goes by name rather than at random.
    pub fn choose_in_order(&mut self, store: &mut (impl Store + ?Sized), in_order: bool) {
        self.change(store, |settings| settings.in_order = in_order);
    }

    /// Each output's rank among the outputs on the same Slideshow's folder, as the drawer
    /// ranks them, so the sidebar shows each the picture the drawer does; `None` for one
    /// that isn't a Slideshow.
    pub fn slideshow_ranks(&self, store: &(impl Store + ?Sized)) -> Vec<Option<usize>> {
        let shown: Vec<(String, OutputSettings)> = (0..self.outputs.len())
            .map(|index| {
                let element = self.outputs[index].identity.path_element();
                (element, self.wallpaper(store, index))
            })
            .collect();
        let shown: Vec<(&str, &OutputSettings)> = shown
            .iter()
            .map(|(element, settings)| (element.as_str(), settings))
            .collect();
        slideshow::ranks(&shown)
    }
    /// Makes today's picture from `source` the selected output's (214).
    pub fn choose_source(&mut self, store: &mut (impl Store + ?Sized), source: Source) {
        self.kind = Kind::PictureOfTheDay;
        self.change(store, |settings| {
            settings.kind = Kind::PictureOfTheDay;
            settings.source = source;
        });
    }

    /// Adds `file`, a picture of the day the user kept, to the pictures they added, once.
    /// What the output shows doesn't change: it's the same picture.
    pub fn keep_picture(&mut self, store: &mut (impl Store + ?Sized), file: &str) {
        let mut pictures = store.pictures();
        if !pictures.iter().any(|picture| picture == file) {
            pictures.push(file.to_owned());
            store.set_pictures(&pictures);
        }
    }

    /// Adds `folder` to the folders the user added, once.
    pub fn add_folder(&mut self, store: &mut (impl Store + ?Sized), folder: &str) {
        let mut folders = store.folders();
        if !folders.iter().any(|added| added == folder) {
            folders.push(folder.to_owned());
            store.set_folders(&folders);
        }
    }
}
