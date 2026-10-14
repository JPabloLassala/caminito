//! An output's wallpaper settings, and which ones it gets from the store (210): its own,
//! by its monitor's identity, or every output's while "same on all outputs" is on. A
//! monitor never seen before starts with the primary output's, and the first run starts
//! every output with the picture `org.mate.background` names.

use crate::dynamic::Moment;
use crate::identity::Identity;
use crate::positioning::Positioning;

/// Which of the five things an output's wallpaper is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kind {
    /// One picture file.
    Picture,
    /// One plain colour.
    #[default]
    Colour,
    /// A folder of pictures on a timer (213).
    Slideshow,
    /// Today's picture from an online source (214).
    PictureOfTheDay,
    /// Pictures that follow the time of day (216, 217).
    Dynamic,
}

impl Kind {
    /// Each kind, in the schema's order.
    pub const ALL: [Self; 5] = [
        Self::Picture,
        Self::Colour,
        Self::Slideshow,
        Self::PictureOfTheDay,
        Self::Dynamic,
    ];

    /// Its nick in the schema's `org.mate.Caminito.WallpaperKind`.
    pub fn nick(self) -> &'static str {
        match self {
            Self::Picture => "picture",
            Self::Colour => "colour",
            Self::Slideshow => "slideshow",
            Self::PictureOfTheDay => "picture-of-the-day",
            Self::Dynamic => "dynamic",
        }
    }

    /// The kind of a nick, or Colour for one the schema doesn't have.
    pub fn from_nick(nick: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|kind| kind.nick() == nick)
            .unwrap_or_default()
    }
}

/// A colour, from its `#rrggbb` text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Colour {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Colour {
    /// The colour of `#rgb`, `#rrggbb` or `#rrrrggggbbbb`, the last as GdkColor writes it
    /// and older MATE settings have it, or `None` for any other text.
    pub fn parse(text: &str) -> Option<Self> {
        let digits = text.strip_prefix('#')?;
        if !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let per_channel = match digits.len() {
            3 => 1,
            6 => 2,
            12 => 4,
            _ => return None,
        };
        let channel = |index: usize| {
            let start = index * per_channel;
            let value = u16::from_str_radix(&digits[start..start + per_channel], 16).ok()?;
            Some(match per_channel {
                1 => (value * 0x11) as u8,
                2 => value as u8,
                _ => (value >> 8) as u8,
            })
        };
        Some(Colour {
            red: channel(0)?,
            green: channel(1)?,
            blue: channel(2)?,
        })
    }

    /// The colour as `#rrggbb`.
    pub fn text(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.red, self.green, self.blue)
    }
}

/// One output's wallpaper, or every output's.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OutputSettings {
    pub kind: Kind,
    /// The Picture's file.
    pub picture: String,
    /// The Dynamic's: a Plasma package's folder or a timed XML (216).
    pub dynamic: String,
    pub positioning: Positioning,
    /// The Colour's, and what shows where a picture doesn't.
    pub colour: Colour,
}

/// What is drawn on an output, of its settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Drawn {
    /// A picture, placed by the positioning over the colour.
    Picture {
        file: String,
        positioning: Positioning,
        colour: Colour,
    },
    /// Two pictures, each placed by the positioning over the colour, the second over the
    /// first `percent` of the way: a dynamic wallpaper between two of its pictures (216).
    Blend {
        from: String,
        to: String,
        percent: u8,
        positioning: Positioning,
        colour: Colour,
    },
    /// The colour alone.
    Colour(Colour),
}

impl Drawn {
    /// The colour shown where no picture is.
    pub fn colour(&self) -> Colour {
        match self {
            Drawn::Picture { colour, .. } | Drawn::Blend { colour, .. } | Drawn::Colour(colour) => {
                *colour
            }
        }
    }

    /// The picture that shows most, if any: as `org.mate.background` has it (211, 216).
    pub fn nearer(&self) -> Option<&str> {
        match self {
            Drawn::Picture { file, .. } => Some(file),
            Drawn::Blend {
                from, to, percent, ..
            } => Some(if *percent < 50 { from } else { to }),
            Drawn::Colour(_) => None,
        }
    }

    /// The picture files it shows.
    pub fn files(&self) -> Vec<&str> {
        match self {
            Drawn::Picture { file, .. } => vec![file],
            Drawn::Blend { from, to, .. } => vec![from, to],
            Drawn::Colour(_) => Vec::new(),
        }
    }
}

impl OutputSettings {
    /// What's drawn, with `moment` what the Dynamic shows now. A Picture with no file, and
    /// a Dynamic with no moment, are their colour, and the two kinds that later tickets draw
    /// are their colour until then. A Dynamic's blend is in whole percent, so a frame is
    /// drawn again only when it shows.
    pub fn drawn(&self, moment: Option<&Moment>) -> Drawn {
        let picture = |file: &str| Drawn::Picture {
            file: file.to_owned(),
            positioning: self.positioning,
            colour: self.colour,
        };
        match (self.kind, moment) {
            (Kind::Picture, _) if !self.picture.is_empty() => picture(&self.picture),
            (Kind::Dynamic, Some(moment)) => {
                let percent = (moment.blend.clamp(0.0, 1.0) * 100.0).round() as u8;
                let (from, to) = (moment.from.to_string_lossy(), moment.to.to_string_lossy());
                match percent {
                    _ if from == to => picture(&from),
                    0 => picture(&from),
                    100 => picture(&to),
                    _ => Drawn::Blend {
                        from: from.into_owned(),
                        to: to.into_owned(),
                        percent,
                        positioning: self.positioning,
                        colour: self.colour,
                    },
                }
            }
            _ => Drawn::Colour(self.colour),
        }
    }
}

/// One set of an output's keys in the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Set {
    /// Every output's, while "same on all outputs" is on.
    All,
    /// The monitor's own.
    Output(Identity),
    /// A monitor's own, by its path element as `outputs` lists it, for a monitor that may
    /// not be plugged in (211).
    Seen(String),
}

/// Where the settings are kept: GSettings for the drawer, memory for the tests.
pub trait Store {
    /// Whether every output has the same settings.
    fn same_on_all_outputs(&self) -> bool;
    /// Turns "same on all outputs" on or off, and nothing else: what each output shows is
    /// the page's to keep.
    fn set_same_on_all_outputs(&mut self, same: bool);
    /// The monitors seen so far, by their path elements.
    fn seen(&self) -> Vec<String>;
    fn set_seen(&mut self, seen: &[String]);
    fn read(&self, set: &Set) -> OutputSettings;
    fn write(&mut self, set: &Set, settings: &OutputSettings);
    /// Whether the first run took `org.mate.background`'s picture already.
    fn took_mate_background(&self) -> bool;
    fn set_took_mate_background(&mut self);
    /// The pictures the user added, each a file.
    fn pictures(&self) -> Vec<String>;
    fn set_pictures(&mut self, pictures: &[String]);
    /// The folders the user added.
    fn folders(&self) -> Vec<String>;
    fn set_folders(&mut self, folders: &[String]);
    /// The dynamic wallpapers the user added, each a timed XML (216).
    fn dynamics(&self) -> Vec<String>;
    fn set_dynamics(&mut self, dynamics: &[String]);
}

/// The store in memory, as GSettings has it with nothing written: "same on all outputs" on,
/// and every set at its defaults. The tests' store, and the dialog's `--demo`.
#[derive(Debug, Default)]
pub struct Memory {
    pub(crate) different: bool,
    pub(crate) seen: Vec<String>,
    sets: std::collections::HashMap<String, OutputSettings>,
    took: bool,
    pictures: Vec<String>,
    folders: Vec<String>,
    dynamics: Vec<String>,
}

impl Memory {
    fn key(set: &Set) -> String {
        match set {
            Set::All => "all".into(),
            Set::Output(identity) => identity.path_element(),
            Set::Seen(element) => element.clone(),
        }
    }
}

impl Store for Memory {
    fn same_on_all_outputs(&self) -> bool {
        !self.different
    }
    fn set_same_on_all_outputs(&mut self, same: bool) {
        self.different = !same;
    }
    fn seen(&self) -> Vec<String> {
        self.seen.clone()
    }
    fn set_seen(&mut self, seen: &[String]) {
        self.seen = seen.to_vec();
    }
    fn read(&self, set: &Set) -> OutputSettings {
        self.sets.get(&Self::key(set)).cloned().unwrap_or_default()
    }
    fn write(&mut self, set: &Set, settings: &OutputSettings) {
        self.sets.insert(Self::key(set), settings.clone());
    }
    fn took_mate_background(&self) -> bool {
        self.took
    }
    fn set_took_mate_background(&mut self) {
        self.took = true;
    }
    fn pictures(&self) -> Vec<String> {
        self.pictures.clone()
    }
    fn set_pictures(&mut self, pictures: &[String]) {
        self.pictures = pictures.to_vec();
    }
    fn folders(&self) -> Vec<String> {
        self.folders.clone()
    }
    fn set_folders(&mut self, folders: &[String]) {
        self.folders = folders.to_vec();
    }
    fn dynamics(&self) -> Vec<String> {
        self.dynamics.clone()
    }
    fn set_dynamics(&mut self, dynamics: &[String]) {
        self.dynamics = dynamics.to_vec();
    }
}

/// The settings of the output that shows `identity`, with `primary` the primary output's.
/// A monitor seen for the first time is given the primary output's wallpaper, or every
/// output's if the primary output's is new too, and is seen from then on.
pub fn settings_for(
    store: &mut (impl Store + ?Sized),
    identity: &Identity,
    primary: Option<&Identity>,
) -> OutputSettings {
    let mut seen = store.seen();
    let element = identity.path_element();
    if !seen.contains(&element) {
        let source = match primary {
            Some(primary)
                if !store.same_on_all_outputs()
                    && primary != identity
                    && seen.contains(&primary.path_element()) =>
            {
                Set::Output(primary.clone())
            }
            _ => Set::All,
        };
        let settings = store.read(&source);
        store.write(&Set::Output(identity.clone()), &settings);
        seen.push(element);
        store.set_seen(&seen);
    }
    if store.same_on_all_outputs() {
        store.read(&Set::All)
    } else {
        store.read(&Set::Output(identity.clone()))
    }
}

/// `org.mate.background`'s keys that the first run reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MateBackground {
    /// `picture-filename`, empty for none.
    pub picture: String,
    /// `picture-options`.
    pub options: String,
    /// `primary-color`.
    pub colour: String,
}

/// On the first run, makes MATE's picture every output's wallpaper, and returns `true`.
/// Every run after does nothing and returns `false`, whatever MATE's picture is by then.
pub fn take_mate_background(store: &mut (impl Store + ?Sized), mate: &MateBackground) -> bool {
    if store.took_mate_background() {
        return false;
    }
    let colour = Colour::parse(&mate.colour).unwrap_or(store.read(&Set::All).colour);
    let kind = if mate.picture.is_empty() {
        Kind::Colour
    } else {
        Kind::Picture
    };
    let settings = OutputSettings {
        kind,
        picture: mate.picture.clone(),
        dynamic: String::new(),
        positioning: Positioning::from_mate(&mate.options),
        colour,
    };
    store.write(&Set::All, &settings);
    store.set_took_mate_background();
    true
}

#[cfg(test)]
pub(crate) mod tests;
