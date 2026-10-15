//! Dynamic wallpapers (216): a set of pictures of one scene, and which of them shows at a
//! moment. Two formats so far, each found where its desktop installs it:
//!
//! - a Plasma package with `contents/images/` and `contents/images_dark/`, as under
//!   `/usr/share/wallpapers`: the light picture by day and the dark by night, cross-fading
//!   through twilight by the sun's elevation, which Caminito publishes (215);
//! - GNOME's timed XML, as `/usr/share/backgrounds/cosmos/background-1.xml`: its pictures at
//!   its clock times, with its transitions.
//!
//! Nothing here reads a clock or the bus: a moment is a [`Sky`], which the drawer and the
//! dialog make, so the tests are deterministic. Nothing here names a toolkit.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::events::Event;

use crate::grid::{is_picture, package_picture};

/// The sun's elevation, in degrees, at and below which a pair shows its dark picture alone:
/// civil twilight's, where Night Light's sun schedule is at its night temperature (138).
const NIGHT: f64 = -6.0;

/// The sun's elevation, in degrees, at and above which a pair shows its light picture alone.
/// Between this and [`NIGHT`] it cross-fades, linearly in elevation, as Night Light warms.
const DAY: f64 = 6.0;

/// The fixed times that stand in for the sun without Caminito to ask (spec): night until
/// 07:00 and from 19:00, as Night Light's with no location.
const FIXED_RISE: i64 = 7 * HOUR;
const FIXED_SET: i64 = 19 * HOUR;

const HOUR: i64 = 3600;
const DAY_SECONDS: i64 = 24 * HOUR;

/// A moment, as a dynamic wallpaper needs it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sky {
    /// The sun's elevation in degrees, as Caminito publishes it, or `None` without Caminito.
    pub elevation: Option<f64>,
    /// The local wall-clock time, in seconds since 1970-01-01 00:00 local: the Unix time
    /// plus the zone's offset, so that a day of a timed XML is a day of the clock across a
    /// change of daylight saving.
    pub local: i64,
}

impl Sky {
    /// How dark it is, 0 by day and 1 by night: linear in the sun's elevation between
    /// [`DAY`] and [`NIGHT`], or the fixed times' without an elevation.
    pub fn darkness(&self) -> f64 {
        match self.elevation {
            Some(elevation) => ((DAY - elevation) / (DAY - NIGHT)).clamp(0.0, 1.0),
            None => {
                let time = self.local.rem_euclid(DAY_SECONDS);
                if (FIXED_RISE..FIXED_SET).contains(&time) {
                    0.0
                } else {
                    1.0
                }
            }
        }
    }
}

/// What a dynamic wallpaper shows at a moment: `from`, with `to` over it `blend` of the way,
/// 0 to 1. A moment between transitions has the same picture for both, and a blend of 0.
#[derive(Debug, Clone, PartialEq)]
pub struct Moment {
    /// The picture under.
    pub from: PathBuf,
    /// The picture over it.
    pub to: PathBuf,
    /// How far `to` is over `from`, 0 to 1.
    pub blend: f64,
}

impl Moment {
    fn still(file: &Path) -> Self {
        Moment {
            from: file.to_owned(),
            to: file.to_owned(),
            blend: 0.0,
        }
    }

    /// The picture that shows most: what a single picture stands for the moment by, as
    /// `org.mate.background` and the dialog's sidebar have it.
    pub fn nearer(&self) -> &Path {
        if self.blend < 0.5 {
            &self.from
        } else {
            &self.to
        }
    }
}

/// Why a file or folder isn't a dynamic wallpaper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// It can't be read.
    Read(String),
    /// It isn't well-formed XML.
    Xml(String),
    /// Its XML isn't a timed wallpaper: the root isn't `background`, or an element it needs
    /// is missing or says something that isn't a number or a file.
    NotTimed(&'static str),
    /// It's a folder without both `contents/images/` and `contents/images_dark/` pictures.
    NotPair,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Read(err) => write!(f, "can't read it: {err}"),
            Error::Xml(err) => write!(f, "it isn't well-formed XML: {err}"),
            Error::NotTimed(why) => write!(f, "it isn't a timed wallpaper: {why}"),
            Error::NotPair => write!(f, "it has no light and dark pictures"),
        }
    }
}

impl std::error::Error for Error {}

/// A dynamic wallpaper, read from its package or file.
#[derive(Debug, Clone, PartialEq)]
pub enum Dynamic {
    /// A Plasma package's light and dark pictures, which follow the sun.
    Pair { light: PathBuf, dark: PathBuf },
    /// GNOME's timed XML, which follows the clock.
    Timed(Timed),
}

impl Dynamic {
    /// The dynamic wallpaper at `path`: a Plasma package's folder, or a timed XML.
    pub fn read(path: &Path) -> Result<Self, Error> {
        if path.is_dir() {
            return pair(path).ok_or(Error::NotPair);
        }
        let text = fs::read_to_string(path).map_err(|err| Error::Read(err.to_string()))?;
        Timed::parse(&text, path.parent().unwrap_or(Path::new("/"))).map(Dynamic::Timed)
    }

    /// Whether it follows the sun, rather than fixed times.
    pub fn follows_sun(&self) -> bool {
        matches!(self, Dynamic::Pair { .. })
    }

    /// What it shows at `sky`.
    pub fn at(&self, sky: &Sky) -> Moment {
        match self {
            Dynamic::Pair { light, dark } => Moment {
                from: light.clone(),
                to: dark.clone(),
                blend: sky.darkness(),
            },
            Dynamic::Timed(timed) => timed.at(sky.local),
        }
    }

    /// How many seconds after `sky` it next shows something else by the clock alone: the
    /// end of a timed XML's picture, or a second during its transition, so that a
    /// transition of a few seconds is drawn as its file gives it. `None` for a pair, which
    /// changes as the sun moves.
    pub fn next_change(&self, sky: &Sky) -> Option<u64> {
        match self {
            Dynamic::Pair { .. } => None,
            Dynamic::Timed(timed) => Some(timed.next_change(sky.local)),
        }
    }

    /// The pictures it's shown by in the dialog: by day and by night. A pair's are its own;
    /// a timed XML's are what it shows at noon and at midnight of the day it starts.
    pub fn day_and_night(&self) -> (PathBuf, PathBuf) {
        match self {
            Dynamic::Pair { light, dark } => (light.clone(), dark.clone()),
            Dynamic::Timed(timed) => {
                let midnight = timed.start - timed.start.rem_euclid(DAY_SECONDS);
                let at = |time: i64| timed.at(time).nearer().to_owned();
                (at(midnight + 12 * HOUR), at(midnight))
            }
        }
    }
}

/// The pair of the Plasma package at `folder`, or `None` if it hasn't both kinds of picture.
/// Each is the package's largest landscape picture of its kind, as the Picture grid's is.
pub(crate) fn pair(folder: &Path) -> Option<Dynamic> {
    let contents = folder.join("contents");
    let picture = |images: &str| {
        let images = contents.join(images);
        let names: Vec<String> = fs::read_dir(&images)
            .ok()?
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| is_picture(path))
            .filter_map(|path| path.file_name()?.to_str().map(str::to_owned))
            .collect();
        package_picture(&names).map(|name| images.join(name))
    };
    Some(Dynamic::Pair {
        light: picture("images")?,
        dark: picture("images_dark")?,
    })
}

/// One part of a timed XML's cycle: a `<static>` or a `<transition>`.
#[derive(Debug, Clone, PartialEq)]
enum Part {
    /// A picture, for `duration` seconds.
    Still { duration: f64, file: PathBuf },
    /// A cross-fade from one picture to another, over `duration` seconds.
    Transition {
        duration: f64,
        from: PathBuf,
        to: PathBuf,
    },
}

impl Part {
    fn duration(&self) -> f64 {
        match self {
            Part::Still { duration, .. } | Part::Transition { duration, .. } => *duration,
        }
    }
}

/// GNOME's timed XML: from its start time, its parts one after the other, over and over.
#[derive(Debug, Clone, PartialEq)]
pub struct Timed {
    /// The start time, in the local seconds of [`Sky::local`].
    start: i64,
    parts: Vec<Part>,
    /// The parts' durations added up, more than 0.
    cycle: f64,
}

impl Timed {
    /// The timed wallpaper `text` says, its relative files taken from `folder`.
    pub fn parse(text: &str, folder: &Path) -> Result<Self, Error> {
        let root = Element::parse(text)?;
        if root.name != "background" {
            return Err(Error::NotTimed("its root isn't a background"));
        }
        let start = root
            .child("starttime")
            .ok_or(Error::NotTimed("it has no start time"))?;
        let field = |name: &'static str| -> Result<i64, Error> {
            match start.child(name) {
                Some(element) => element
                    .text
                    .trim()
                    .parse()
                    .map_err(|_| Error::NotTimed("its start time isn't numbers")),
                None => Ok(0),
            }
        };
        let (year, month, day) = (field("year")?, field("month")?, field("day")?);
        if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            return Err(Error::NotTimed("its start time isn't a date"));
        }
        let start = days_from_civil(year, month, day) * DAY_SECONDS
            + field("hour")? * HOUR
            + field("minute")? * 60
            + field("second")?;

        let file = |element: Option<&Element>| -> Result<PathBuf, Error> {
            let element = element.ok_or(Error::NotTimed("a part has no file"))?;
            let text = largest_size(element).unwrap_or(&element.text).trim();
            if text.is_empty() {
                return Err(Error::NotTimed("a part has no file"));
            }
            Ok(folder.join(text))
        };
        let mut parts = Vec::new();
        for element in &root.children {
            let duration = || -> Result<f64, Error> {
                let duration = element
                    .child("duration")
                    .and_then(|duration| duration.text.trim().parse::<f64>().ok())
                    .filter(|duration| duration.is_finite() && *duration >= 0.0);
                duration.ok_or(Error::NotTimed("a part's duration isn't a number"))
            };
            match element.name.as_str() {
                "static" => parts.push(Part::Still {
                    duration: duration()?,
                    file: file(element.child("file"))?,
                }),
                "transition" => parts.push(Part::Transition {
                    duration: duration()?,
                    from: file(element.child("from"))?,
                    to: file(element.child("to"))?,
                }),
                _ => {}
            }
        }
        let cycle: f64 = parts.iter().map(Part::duration).sum();
        if cycle <= 0.0 {
            return Err(Error::NotTimed("it has no parts that last"));
        }
        Ok(Timed {
            start,
            parts,
            cycle,
        })
    }

    /// What it shows at `local`, in the local seconds of [`Sky::local`]. Before its start
    /// time it's where it would be had it started a whole number of cycles sooner, as GNOME
    /// has it.
    /// Seconds from `local` to the end of the picture it shows then, or 1 in a transition.
    fn next_change(&self, local: i64) -> u64 {
        let mut into = ((local - self.start) as f64).rem_euclid(self.cycle);
        for part in &self.parts {
            let duration = part.duration();
            if into < duration {
                return match part {
                    Part::Still { .. } => ((duration - into).ceil() as u64).max(1),
                    Part::Transition { .. } => 1,
                };
            }
            into -= duration;
        }
        1
    }

    fn at(&self, local: i64) -> Moment {
        let mut into = ((local - self.start) as f64).rem_euclid(self.cycle);
        for part in &self.parts {
            let duration = part.duration();
            if into < duration {
                return match part {
                    Part::Still { file, .. } => Moment::still(file),
                    Part::Transition { from, to, .. } => Moment {
                        from: from.clone(),
                        to: to.clone(),
                        blend: into / duration,
                    },
                };
            }
            into -= duration;
        }
        // Only rounding gets here: the moment is the cycle's last.
        match self.parts.last().expect("a timed wallpaper has a part") {
            Part::Still { file, .. } => Moment::still(file),
            Part::Transition { to, .. } => Moment::still(to),
        }
    }
}

/// The file of an element that lists one per size, as GNOME's newer XMLs do, as
/// `<size width="1920" height="1080">…</size>`: the largest.
fn largest_size(element: &Element) -> Option<&String> {
    let size = |child: &Element| {
        let dimension = |name: &str| child.attribute(name)?.parse::<u64>().ok();
        dimension("width").unwrap_or(0) * dimension("height").unwrap_or(0)
    };
    element
        .children
        .iter()
        .filter(|child| child.name == "size")
        .max_by_key(|child| size(child))
        .map(|child| &child.text)
}

/// The days from 1970-01-01 to the date, in the proleptic Gregorian calendar (Howard
/// Hinnant's `days_from_civil`, from https://howardhinnant.github.io/date_algorithms.html).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// An element of a timed XML, with its text and the elements in it: as much of XML as the
/// format needs.
#[derive(Debug, Default)]
struct Element {
    name: String,
    attributes: Vec<(String, String)>,
    text: String,
    children: Vec<Element>,
}

impl Element {
    /// The root element of `text`.
    fn parse(text: &str) -> Result<Self, Error> {
        let mut reader = quick_xml::Reader::from_str(text);
        let xml = |err: &dyn fmt::Display| Error::Xml(err.to_string());
        // The elements open, the root first.
        let mut open: Vec<Element> = Vec::new();
        let mut root = None;
        loop {
            let event = reader.read_event().map_err(|err| xml(&err))?;
            match event {
                Event::Start(start) => open.push(Element::of(&start)?),
                Event::Empty(start) => {
                    let element = Element::of(&start)?;
                    if open.is_empty() && root.is_some() {
                        return Err(Error::Xml("more than one root".into()));
                    }
                    close(&mut open, &mut root, element);
                }
                Event::End(_) => {
                    let element = open
                        .pop()
                        .ok_or(Error::Xml("an end with no start".into()))?;
                    if open.is_empty() && root.is_some() {
                        return Err(Error::Xml("more than one root".into()));
                    }
                    close(&mut open, &mut root, element);
                }
                Event::Text(text) => {
                    let text = text.decode().map_err(|err| xml(&err))?;
                    push_text(&mut open, &text);
                }
                Event::CData(text) => {
                    let text = text.decode().map_err(|err| xml(&err))?;
                    push_text(&mut open, &text);
                }
                Event::GeneralRef(reference) => {
                    let resolved = match reference.resolve_char_ref().map_err(|err| xml(&err))? {
                        Some(character) => character.to_string(),
                        None => {
                            let name = reference.decode().map_err(|err| xml(&err))?;
                            quick_xml::escape::resolve_predefined_entity(&name)
                                .ok_or_else(|| Error::Xml(format!("an unknown entity, {name}")))?
                                .to_owned()
                        }
                    };
                    push_text(&mut open, &resolved);
                }
                Event::Eof => break,
                _ => {}
            }
        }
        if !open.is_empty() {
            return Err(Error::Xml("it ends inside an element".into()));
        }
        root.ok_or(Error::Xml("it has no root".into()))
    }

    /// The element `start` opens, with its attributes and nothing in it yet.
    fn of(start: &quick_xml::events::BytesStart<'_>) -> Result<Self, Error> {
        let mut element = Element {
            name: String::from_utf8_lossy(start.name().as_ref()).into_owned(),
            ..Element::default()
        };
        for attribute in start.attributes() {
            let attribute = attribute.map_err(|err| Error::Xml(err.to_string()))?;
            let value = attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|err| Error::Xml(err.to_string()))?;
            let key = String::from_utf8_lossy(attribute.key.as_ref()).into_owned();
            element.attributes.push((key, value.into_owned()));
        }
        Ok(element)
    }

    /// Its first child named `name`.
    fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|child| child.name == name)
    }

    fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// Adds `text` to the element open innermost. Text outside the root is ignored.
fn push_text(open: &mut [Element], text: &str) {
    if let Some(element) = open.last_mut() {
        element.text.push_str(text);
    }
}

/// Puts a finished `element` in the one it's in, or makes it the root.
fn close(open: &mut [Element], root: &mut Option<Element>, element: Element) {
    match open.last_mut() {
        Some(parent) => parent.children.push(element),
        None => *root = Some(element),
    }
}

#[cfg(test)]
mod tests;
