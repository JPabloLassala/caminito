//! Apple's dynamic wallpapers (217): a HEIC file of several pictures of one scene, with XMP
//! metadata that says when each shows. Three variants:
//!
//! - `apple_desktop:solar`, each picture at a position of the sun, its elevation and
//!   azimuth, which Caminito publishes (215);
//! - `apple_desktop:h24`, the older, each picture at a time of the day;
//! - `apple_desktop:apr`, a light and a dark picture, as a Plasma pair.
//!
//! Each is a base64 binary property list. They're read here from the file's container,
//! with no decoder: the metadata is an item of its own, found by the `meta` box's `iinf` and
//! `iloc`. The pictures are another matter, HEVC behind a grid of tiles, and libheif decodes
//! them ([`decode`]): its `heif-dec` writes each to a JPEG in the user's cache, once per
//! file, and the drawer draws those as any picture. libheif is optional (spec): without
//! `heif-dec` a HEIC is read but not shown, and the dialog greys it out.
//!
//! The decoder is a program rather than the library linked or opened, so the crate stays
//! free of `unsafe` and builds without libheif, and a file that crashes the decoder doesn't
//! take the drawer with it.

use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gio::glib;

/// The most of a `meta` box read: Apple's are a few kilobytes.
const META_LIMIT: u64 = 16 << 20;

/// The most of an XMP item read.
const XMP_LIMIT: u64 = 4 << 20;

/// How deep a property list's arrays and dictionaries nest at most: Apple's nest three deep.
const PLIST_DEPTH: usize = 8;

const DAY_SECONDS: i64 = 24 * 3600;

/// How far either side of north or south, in degrees of azimuth, a sun in the middle band
/// of the sky cross-fades from one half's picture to the other's: an hour of the sun's
/// path at the equinox.
const MERIDIAN: f64 = 15.0;

/// The programs of libheif's that decode a HEIC: `heif-dec`, called `heif-convert` before
/// libheif 1.16. Both write each of a file's pictures as `<name>-<n>.<extension>`.
const DECODERS: [&str; 2] = ["heif-dec", "heif-convert"];

/// Why a HEIC file isn't a dynamic wallpaper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// It can't be read.
    Read(String),
    /// It isn't a HEIF container, or its `meta` box can't be made sense of.
    NotHeif(&'static str),
    /// It has no XMP saying when its pictures show: one picture, a photo.
    NoSchedule,
    /// Its XMP says something this doesn't understand.
    BadSchedule(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Read(err) => write!(f, "can't read it: {err}"),
            Error::NotHeif(why) => write!(f, "it isn't a HEIC file: {why}"),
            Error::NoSchedule => write!(f, "it isn't a dynamic wallpaper, only a picture"),
            Error::BadSchedule(why) => write!(f, "its dynamic wallpaper metadata is broken: {why}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        if err.kind() == io::ErrorKind::UnexpectedEof {
            Error::NotHeif("it ends too soon")
        } else {
            Error::Read(err.to_string())
        }
    }
}

/// Whether `path` is named as a HEIC file.
pub fn is_heic(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("heic") || extension.eq_ignore_ascii_case("heif")
        })
}

/// Which picture shows at a moment: `from`, with `to` over it `blend` of the way, the two
/// counted from 0 as the file has them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pick {
    /// The picture under.
    pub from: usize,
    /// The picture over it.
    pub to: usize,
    /// How far `to` is over `from`, 0 to 1.
    pub blend: f64,
}

impl Pick {
    fn still(picture: usize) -> Self {
        Pick {
            from: picture,
            to: picture,
            blend: 0.0,
        }
    }

    fn between(from: usize, to: usize, blend: f64) -> Self {
        Pick {
            from,
            to,
            blend: blend.clamp(0.0, 1.0),
        }
    }
    /// The picture that shows most.
    pub fn nearer(&self) -> usize {
        if self.blend < 0.5 { self.from } else { self.to }
    }

    /// The pick `t` of the way, 0 to 1, from itself to `picture` alone, one of its two.
    fn lean(self, picture: usize, t: f64) -> Self {
        let blend = if picture == self.to && picture != self.from {
            self.blend + (1.0 - self.blend) * t
        } else {
            self.blend * (1.0 - t)
        };
        Pick::between(self.from, self.to, blend)
    }
}

/// A picture at a position of the sun, in degrees: `solar`'s `si`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    /// The sun's elevation, −90° to 90°.
    pub elevation: f64,
    /// The sun's azimuth, from north through east.
    pub azimuth: f64,
    /// The picture, from 0.
    pub picture: usize,
}

impl Position {
    /// Whether it's in the eastern half of the sky, where the sun rises and climbs, as it
    /// is in either hemisphere.
    fn east(&self) -> bool {
        is_east(self.azimuth)
    }
}

fn is_east(azimuth: f64) -> bool {
    azimuth.rem_euclid(360.0) < 180.0
}

/// A picture at a time of the day, in seconds from midnight: `h24`'s `ti`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Time {
    /// The seconds from local midnight, 0 to a day.
    pub seconds: f64,
    /// The picture, from 0.
    pub picture: usize,
}

/// The light and the dark picture: `apr`, or the `ap` of the others.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Appearance {
    /// The picture by day.
    pub light: usize,
    /// The picture by night.
    pub dark: usize,
}

/// When a HEIC's pictures show.
#[derive(Debug, Clone, PartialEq)]
pub enum Schedule {
    /// Each at a position of the sun, with the light and dark ones, if it says.
    Solar {
        positions: Vec<Position>,
        appearance: Appearance,
    },
    /// Each at a time of the day, in order.
    Clock {
        times: Vec<Time>,
        appearance: Appearance,
    },
    /// A light and a dark picture.
    Appearance(Appearance),
}

impl Schedule {
    /// The schedule an XMP packet says.
    pub fn from_xmp(xmp: &str) -> Result<Self, Error> {
        let (name, encoded) = apple_desktop(xmp).ok_or(Error::NoSchedule)?;
        let plist = base64(&encoded).ok_or(Error::BadSchedule("it isn't base64"))?;
        let plist = Plist::parse(&plist)?;
        Self::from_plist(&name, &plist)
    }

    /// The schedule of `apple_desktop:<name>`, whose property list is `plist`.
    fn from_plist(name: &str, plist: &Plist) -> Result<Self, Error> {
        let appearance = |plist: &Plist| -> Option<Appearance> {
            Some(Appearance {
                light: plist.get("l")?.index()?,
                dark: plist.get("d")?.index()?,
            })
        };
        match name {
            "solar" => {
                let mut positions = Vec::new();
                for entry in plist
                    .get("si")
                    .and_then(Plist::array)
                    .ok_or(Error::BadSchedule("it has no positions of the sun"))?
                {
                    let position = (|| {
                        Some(Position {
                            elevation: entry.get("a")?.number()?,
                            azimuth: entry.get("z")?.number()?,
                            picture: entry.get("i")?.index()?,
                        })
                    })()
                    .filter(|p| p.elevation.abs() <= 90.0 && p.azimuth.is_finite())
                    .ok_or(Error::BadSchedule("a position of the sun isn't one"))?;
                    positions.push(position);
                }
                if positions.is_empty() {
                    return Err(Error::BadSchedule("it has no positions of the sun"));
                }
                let appearance = plist.get("ap").and_then(appearance).unwrap_or_else(|| {
                    let by = |cmp: fn(f64, f64) -> bool| {
                        positions
                            .iter()
                            .copied()
                            .reduce(|a, b| if cmp(b.elevation, a.elevation) { b } else { a })
                            .map_or(0, |p| p.picture)
                    };
                    Appearance {
                        light: by(|b, a| b > a),
                        dark: by(|b, a| b < a),
                    }
                });
                Ok(Schedule::Solar {
                    positions,
                    appearance,
                })
            }
            "h24" => {
                let mut times = Vec::new();
                for entry in plist
                    .get("ti")
                    .and_then(Plist::array)
                    .ok_or(Error::BadSchedule("it has no times"))?
                {
                    let time = (|| {
                        Some(Time {
                            seconds: entry.get("t")?.number()?,
                            picture: entry.get("i")?.index()?,
                        })
                    })()
                    .filter(|t| (0.0..=1.0).contains(&t.seconds))
                    .ok_or(Error::BadSchedule("a time isn't one"))?;
                    // A fraction of the day.
                    times.push(Time {
                        seconds: time.seconds * DAY_SECONDS as f64,
                        ..time
                    });
                }
                if times.is_empty() {
                    return Err(Error::BadSchedule("it has no times"));
                }
                times.sort_by(|a, b| a.seconds.total_cmp(&b.seconds));
                let at = |seconds: f64| Schedule::clock_at(&times, seconds).nearer();
                let appearance = plist.get("ap").and_then(appearance).unwrap_or(Appearance {
                    light: at(12.0 * 3600.0),
                    dark: at(0.0),
                });
                Ok(Schedule::Clock { times, appearance })
            }
            "apr" => appearance(plist)
                .map(Schedule::Appearance)
                .ok_or(Error::BadSchedule("it has no light and dark pictures")),
            _ => Err(Error::NoSchedule),
        }
    }

    /// The pictures it names, each once.
    pub fn pictures(&self) -> Vec<usize> {
        let mut pictures: Vec<usize> = match self {
            Schedule::Solar {
                positions,
                appearance,
            } => positions
                .iter()
                .map(|p| p.picture)
                .chain([appearance.light, appearance.dark])
                .collect(),
            Schedule::Clock { times, appearance } => times
                .iter()
                .map(|t| t.picture)
                .chain([appearance.light, appearance.dark])
                .collect(),
            Schedule::Appearance(appearance) => vec![appearance.light, appearance.dark],
        };
        pictures.sort_unstable();
        pictures.dedup();
        pictures
    }

    /// Whether it follows the sun, rather than the clock.
    pub fn follows_sun(&self) -> bool {
        !matches!(self, Schedule::Clock { .. })
    }

    /// Its light and dark pictures, by which the dialog shows it.
    pub fn appearance(&self) -> Appearance {
        match self {
            Schedule::Solar { appearance, .. }
            | Schedule::Clock { appearance, .. }
            | Schedule::Appearance(appearance) => *appearance,
        }
    }

    /// What it shows with the sun at `elevation` and `azimuth`, `local` seconds into the
    /// day, and `darkness` from 0 by day to 1 by night as a Plasma pair has it.
    ///
    /// Without the sun's position a solar schedule is its light and dark pictures, as a
    /// pair.
    pub fn at(&self, sun: Option<(f64, f64)>, local: i64, darkness: f64) -> Pick {
        let pair =
            |appearance: &Appearance| Pick::between(appearance.light, appearance.dark, darkness);
        match self {
            Schedule::Solar {
                positions,
                appearance,
            } => match sun {
                Some((elevation, azimuth)) => solar_at(positions, elevation, azimuth),
                None => pair(appearance),
            },
            Schedule::Clock { times, .. } => {
                Schedule::clock_at(times, local.rem_euclid(DAY_SECONDS) as f64)
            }
            Schedule::Appearance(appearance) => pair(appearance),
        }
    }

    /// What `times`, in order, show at `seconds` into the day: each picture cross-fading to
    /// the next from its time to the next's, across midnight from the last to the first.
    fn clock_at(times: &[Time], seconds: f64) -> Pick {
        let next = times
            .iter()
            .position(|time| time.seconds > seconds)
            .unwrap_or(0);
        let previous = (next + times.len() - 1) % times.len();
        let (from, to) = (times[previous], times[next]);
        let span = (to.seconds - from.seconds).rem_euclid(DAY_SECONDS as f64);
        if span == 0.0 {
            return Pick::still(from.picture);
        }
        let into = (seconds - from.seconds).rem_euclid(DAY_SECONDS as f64);
        Pick::between(from.picture, to.picture, into / span)
    }
}

/// What `positions` show with the sun at `elevation` and `azimuth`.
///
/// A day goes round the positions in a cycle: up the eastern half of the sky, over the
/// top, down the western half and round under the horizon. The sky is cut in three bands
/// for it. Between the lower of the two halves' highest positions and the higher of their
/// lowest, the sun is placed among the positions of its own half by its elevation, and
/// cross-fades between the two on either side of it, linearly in elevation. Above, where
/// the sun turns from east to west and its elevation no longer says which way it's going,
/// it's placed among the positions in that band by the east-west part of its azimuth, 1
/// due east, −1 due west, and 0 north and south, where the halves meet; below, the same
/// through the night.
///
/// So a file made at one latitude shows its morning in the morning and its dusk at dusk
/// at another, in either hemisphere, where the nearest position by azimuth alone would
/// show a northern noon at a southern midnight. A sun that crosses north or south in the
/// middle band, as a winter noon's does below the top or a summer midnight's above the
/// bottom, cross-fades from its half's picture to the other's as it crosses. With every
/// position in one half of the
/// sky, that half stands for both.
fn solar_at(positions: &[Position], elevation: f64, azimuth: f64) -> Pick {
    let mut east: Vec<Position> = positions.iter().copied().filter(Position::east).collect();
    let mut west: Vec<Position> = positions.iter().copied().filter(|p| !p.east()).collect();
    if east.is_empty() {
        east = west.clone();
    } else if west.is_empty() {
        west = east.clone();
    }
    let highest = |half: &[Position]| half.iter().map(|p| p.elevation).fold(f64::MIN, f64::max);
    let lowest = |half: &[Position]| half.iter().map(|p| p.elevation).fold(f64::MAX, f64::min);
    let top = highest(&east).min(highest(&west));
    let bottom = lowest(&east).max(lowest(&west));
    let eastness = |azimuth: f64| azimuth.to_radians().sin();

    if elevation >= top || elevation <= bottom {
        // Across the top, the sun goes from east to west; across the bottom, back.
        let over = elevation >= top;
        let mut band: Vec<(f64, usize)> = east
            .iter()
            .chain(&west)
            .filter(|p| {
                if over {
                    p.elevation >= top
                } else {
                    p.elevation <= bottom
                }
            })
            .map(|p| {
                let e = eastness(p.azimuth);
                (if over { -e } else { e }, p.picture)
            })
            .collect();
        band.sort_by(|a, b| a.0.total_cmp(&b.0));
        band.dedup();
        let sun = if over {
            -eastness(azimuth)
        } else {
            eastness(azimuth)
        };
        return along(&band, sun);
    }
    let in_half = |half: &[Position], rising: bool| {
        let mut stops: Vec<(f64, usize)> = half.iter().map(|p| (p.elevation, p.picture)).collect();
        stops.sort_by(|a, b| a.0.total_cmp(&b.0));
        let pick = along(&stops, elevation);
        if rising {
            pick
        } else {
            // Falling, so the cross-fade is from the higher to the lower.
            Pick::between(pick.to, pick.from, 1.0 - pick.blend)
        }
    };
    let (rising, falling) = (in_half(&east, true), in_half(&west, false));
    // A sun that crosses north or south in this band, as a winter noon's below the top,
    // goes from one half's picture to the other's within [`MERIDIAN`] of the crossing: its
    // half's pick leans to its picture on the crossing's side, the higher at noon and the
    // lower at midnight, that cross-fades to the other half's, and that leans back into
    // the other half's pick. Noon is above the middle of the band.
    let across = (0.5 - eastness(azimuth) / (2.0 * MERIDIAN.to_radians().sin())).clamp(0.0, 1.0);
    let noon = elevation > (top + bottom) / 2.0;
    let (rising_side, falling_side) = if noon {
        (rising.to, falling.from)
    } else {
        (rising.from, falling.to)
    };
    if across <= 1.0 / 3.0 {
        rising.lean(rising_side, across * 3.0)
    } else if across >= 2.0 / 3.0 {
        falling.lean(falling_side, (1.0 - across) * 3.0)
    } else {
        Pick::between(rising_side, falling_side, across * 3.0 - 1.0)
    }
}

/// The pick among `stops`, each a value and a picture in rising order, for `value`: the
/// two stops either side of it, cross-fading linearly, or the first or the last alone
/// outside them.
fn along(stops: &[(f64, usize)], value: f64) -> Pick {
    let next = stops.iter().position(|&(at, _)| at > value);
    match next {
        None => Pick::still(stops[stops.len() - 1].1),
        Some(0) => Pick::still(stops[0].1),
        Some(next) => {
            let ((a, from), (b, to)) = (stops[next - 1], stops[next]);
            Pick::between(from, to, (value - a) / (b - a))
        }
    }
}

/// A HEIC dynamic wallpaper: its schedule, and where its pictures are decoded to.
#[derive(Debug, Clone, PartialEq)]
pub struct Heic {
    file: PathBuf,
    /// The folder of its pictures, `picture-1.jpg` its first, in the cache.
    folder: PathBuf,
    schedule: Schedule,
}

impl Heic {
    /// The HEIC `file`, its pictures decoded to the user's cache.
    pub fn read(file: &Path) -> Result<Self, Error> {
        Self::read_in(file, &user_cache())
    }

    /// The HEIC `file`, its pictures decoded to `cache`.
    pub fn read_in(file: &Path, cache: &Path) -> Result<Self, Error> {
        let mut reader = fs::File::open(file).map_err(|err| Error::Read(err.to_string()))?;
        let xmp = xmp(&mut reader)?;
        let schedule = Schedule::from_xmp(&xmp)?;
        let folder = cache.join(cache_folder_name(file, &reader.metadata()?));
        Ok(Heic {
            file: file.to_owned(),
            folder,
            schedule,
        })
    }

    /// The HEIC file itself.
    pub fn file(&self) -> &Path {
        &self.file
    }

    /// When its pictures show.
    pub fn schedule(&self) -> &Schedule {
        &self.schedule
    }

    /// The file of its picture `index`, from 0, decoded or not.
    pub fn picture(&self, index: usize) -> PathBuf {
        self.folder.join(format!("picture-{}.jpg", index + 1))
    }

    /// Whether its pictures are decoded, every one it names.
    pub fn decoded(&self) -> bool {
        self.schedule
            .pictures()
            .into_iter()
            .all(|index| self.picture(index).is_file())
    }
}

/// Where HEICs' pictures are decoded: `~/.cache/caminito-wallpaper/heic`.
pub fn user_cache() -> PathBuf {
    glib::user_cache_dir()
        .join("caminito-wallpaper")
        .join("heic")
}

/// The name of the folder of `file`'s pictures in the cache: its name and a hash of its
/// path, size and time, so that a file changed or replaced is decoded again.
fn cache_folder_name(file: &Path, metadata: &fs::Metadata) -> String {
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .unwrap_or_default();
    let identity = format!(
        "{}\n{}\n{}",
        file.display(),
        metadata.len(),
        modified.as_nanos()
    );
    let hash = glib::compute_checksum_for_string(glib::ChecksumType::Sha256, &identity)
        .map(|hash| hash.to_string())
        .unwrap_or_default();
    let stem = file
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    format!("{stem}-{}", &hash[..hash.len().min(16)])
}

/// libheif's decoder, if it's installed.
pub fn decoder() -> Option<PathBuf> {
    DECODERS.iter().find_map(glib::find_program_in_path)
}

/// Why a HEIC's pictures weren't decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// libheif's `heif-dec` isn't installed: the dialog greys HEICs out.
    NoDecoder,
    /// The folder to decode into can't be made.
    Folder(String),
    /// `heif-dec` can't be run.
    Run(String),
    /// `heif-dec` ran and failed, with what it said.
    Failed(String),
    /// The pictures can't be moved into place in the cache.
    Move(String),
    /// It wrote fewer pictures than the metadata names.
    TooFew,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::NoDecoder => write!(
                f,
                "libheif's heif-dec isn't installed (on Debian, it's in libheif-examples)"
            ),
            DecodeError::Folder(err) => write!(f, "can't make the folder for its pictures: {err}"),
            DecodeError::Run(err) => write!(f, "can't run heif-dec: {err}"),
            DecodeError::Failed(said) => write!(f, "heif-dec can't decode it: {said}"),
            DecodeError::Move(err) => write!(f, "can't move its pictures into the cache: {err}"),
            DecodeError::TooFew => write!(f, "it has fewer pictures than its metadata names"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Decodes `heic`'s pictures into the cache with libheif's `heif-dec`, off the main loop,
/// and calls `done` with how it went once they're there. They're written to a folder of
/// their own beside, and it's renamed into place, so the drawer and the dialog decoding
/// the same file at once each see all of its pictures or none.
pub fn decode(heic: &Heic, done: impl FnOnce(Result<(), DecodeError>) + 'static) {
    let Some(decoder) = decoder() else {
        done(Err(DecodeError::NoDecoder));
        return;
    };
    let folder = heic.folder.clone();
    // Beside it, named for this process, whatever dots the file's name has.
    let mut part = folder.clone().into_os_string();
    part.push(format!(".part-{}", std::process::id()));
    let part = PathBuf::from(part);
    let _ = fs::remove_dir_all(&part);
    if let Err(err) = fs::create_dir_all(&part) {
        done(Err(DecodeError::Folder(err.to_string())));
        return;
    }
    let output = part.join("picture.jpg");
    let argv = [
        decoder.as_os_str(),
        "-q".as_ref(),
        "92".as_ref(),
        heic.file.as_os_str(),
        output.as_os_str(),
    ];
    let process = match gio::Subprocess::newv(
        &argv,
        gio::SubprocessFlags::STDOUT_SILENCE | gio::SubprocessFlags::STDERR_PIPE,
    ) {
        Ok(process) => process,
        Err(err) => {
            let _ = fs::remove_dir_all(&part);
            done(Err(DecodeError::Run(err.to_string())));
            return;
        }
    };
    let (heic, waited) = (heic.clone(), process.clone());
    process.communicate_utf8_async(None, None::<&gio::Cancellable>, move |result| {
        let outcome = (|| {
            let (_, stderr) = result.map_err(|err| DecodeError::Run(err.to_string()))?;
            if !waited.is_successful() {
                let stderr = stderr.map(|s| s.trim().to_owned()).unwrap_or_default();
                return Err(DecodeError::Failed(stderr));
            }
            // One picture is written without a number.
            let single = part.join("picture.jpg");
            if single.is_file() {
                fs::rename(&single, part.join("picture-1.jpg"))
                    .map_err(|err| DecodeError::Move(err.to_string()))?;
            }
            // Another process may have put them there first.
            if let Err(err) = fs::rename(&part, &folder)
                && !folder.is_dir()
            {
                return Err(DecodeError::Move(err.to_string()));
            }
            if heic.decoded() {
                Ok(())
            } else {
                Err(DecodeError::TooFew)
            }
        })();
        let _ = fs::remove_dir_all(&part);
        done(outcome);
    });
}

/// The HEICs a program is decoding, and those that couldn't be decoded, which aren't tried
/// again while it runs: the drawer's, and the dialog's for its thumbnails. A HEIC met
/// without libheif isn't one of them, so that libheif installed later is used.
#[derive(Clone)]
pub struct Decodes {
    /// The log domain of the program's GLib messages.
    domain: &'static str,
    state: Rc<RefCell<DecodesState>>,
}

#[derive(Default)]
struct DecodesState {
    running: HashSet<PathBuf>,
    failed: HashSet<PathBuf>,
    /// Whether it said that libheif isn't installed: once, not once per HEIC.
    said_no_decoder: bool,
}

impl Decodes {
    /// None yet, logged under `domain`.
    pub fn new(domain: &'static str) -> Self {
        Decodes {
            domain,
            state: Rc::default(),
        }
    }

    /// Decodes `heic`'s pictures, unless they're being decoded or couldn't be, and calls
    /// `decoded` once they're in. A failure is logged once.
    pub fn start(&self, heic: &Heic, decoded: impl FnOnce() + 'static) {
        let file = heic.file().to_owned();
        {
            let mut state = self.state.borrow_mut();
            if state.failed.contains(&file) || state.running.contains(&file) {
                return;
            }
            if decoder().is_none() {
                if !std::mem::replace(&mut state.said_no_decoder, true) {
                    glib::g_warning!(self.domain, "can't show a HEIC: {}", DecodeError::NoDecoder);
                }
                return;
            }
            state.running.insert(file.clone());
        }
        glib::g_debug!(self.domain, "decoding {}", file.display());
        let decodes = self.clone();
        decode(heic, move |outcome| {
            let mut state = decodes.state.borrow_mut();
            state.running.remove(&file);
            match outcome {
                Ok(()) => {
                    glib::g_debug!(decodes.domain, "decoded {}", file.display());
                    drop(state);
                    decoded();
                }
                Err(err) => {
                    glib::g_warning!(
                        decodes.domain,
                        "can't show the dynamic wallpaper {}: {err}",
                        file.display()
                    );
                    state.failed.insert(file);
                }
            }
        });
    }
}

/// The XMP packet in a HEIF container `reader`: the `meta` box's item of the type `mime`
/// and the content type `application/rdf+xml` whose text names `apple_desktop`, or its
/// first such item if none does.
pub fn xmp(reader: &mut (impl Read + Seek)) -> Result<String, Error> {
    let meta = top_level_meta(reader)?;
    let mut xmp_ids = Vec::new();
    let mut locations = Vec::new();
    let mut idat: &[u8] = &[];
    for (kind, body) in boxes(&meta[4..]).map_err(Error::NotHeif)? {
        match &kind {
            b"iinf" => xmp_ids = xmp_items(body).map_err(Error::NotHeif)?,
            b"iloc" => locations = iloc(body).map_err(Error::NotHeif)?,
            b"idat" => idat = body,
            _ => {}
        }
    }
    let mut first = None;
    for id in xmp_ids {
        let Some(location) = locations.iter().find(|location| location.id == id) else {
            continue;
        };
        let mut bytes = Vec::new();
        for &(offset, length) in &location.extents {
            if bytes.len() as u64 + length > XMP_LIMIT {
                return Err(Error::NotHeif("its XMP is too long"));
            }
            match location.method {
                0 => {
                    reader.seek(SeekFrom::Start(location.base + offset))?;
                    let start = bytes.len();
                    bytes.resize(start + length as usize, 0);
                    reader.read_exact(&mut bytes[start..])?;
                }
                1 => {
                    let start = (location.base + offset) as usize;
                    let piece = start
                        .checked_add(length as usize)
                        .and_then(|end| idat.get(start..end))
                        .ok_or(Error::NotHeif("its XMP is outside its idat"))?;
                    bytes.extend_from_slice(piece);
                }
                _ => continue,
            }
        }
        let text = String::from_utf8_lossy(&bytes).into_owned();
        if text.contains("apple_desktop") {
            return Ok(text);
        }
        first.get_or_insert(text);
    }
    first.ok_or(Error::NoSchedule)
}

/// The body of the file's top-level `meta` box, after the `ftyp` that says it's HEIF.
fn top_level_meta(reader: &mut (impl Read + Seek)) -> Result<Vec<u8>, Error> {
    let mut first = true;
    loop {
        let mut header = [0u8; 8];
        match reader.read_exact(&mut header) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::UnexpectedEof && !first => {
                return Err(Error::NotHeif("it has no meta box"));
            }
            Err(err) => return Err(err.into()),
        }
        let size = u64::from(u32::from_be_bytes(
            header[..4]
                .try_into()
                .expect("a header's first four bytes are four"),
        ));
        let kind = &header[4..];
        let (size, header_size) = match size {
            1 => {
                let mut large = [0u8; 8];
                reader.read_exact(&mut large)?;
                (u64::from_be_bytes(large), 16)
            }
            0 => (u64::MAX, 8),
            _ => (size, 8),
        };
        if size < header_size {
            return Err(Error::NotHeif("a box is shorter than its header"));
        }
        if first && kind != b"ftyp" {
            return Err(Error::NotHeif("it doesn't start with ftyp"));
        }
        first = false;
        if kind == b"meta" {
            let body = size - header_size;
            if body > META_LIMIT {
                return Err(Error::NotHeif("its meta box is too long"));
            }
            let mut meta = vec![0u8; body as usize];
            reader.read_exact(&mut meta)?;
            if meta.len() < 4 {
                return Err(Error::NotHeif("its meta box is empty"));
            }
            return Ok(meta);
        }
        if size == u64::MAX {
            return Err(Error::NotHeif("it has no meta box"));
        }
        reader.seek(SeekFrom::Current((size - header_size) as i64))?;
    }
}

/// Boxes, each its type and its body.
type Boxes<'a> = Vec<([u8; 4], &'a [u8])>;

/// The boxes one after another in `bytes`.
fn boxes(mut bytes: &[u8]) -> Result<Boxes<'_>, &'static str> {
    let mut found = Vec::new();
    while !bytes.is_empty() {
        let mut cursor = Cursor(bytes);
        let size = cursor.uint(4)?;
        let kind: [u8; 4] = cursor
            .take(4)?
            .try_into()
            .expect("four bytes taken are four");
        let (size, header) = match size {
            1 => (cursor.uint(8)?, 16),
            0 => (bytes.len() as u64, 8),
            _ => (size, 8),
        };
        if size < header || size > bytes.len() as u64 {
            return Err("a box's size is wrong");
        }
        found.push((kind, &bytes[header as usize..size as usize]));
        bytes = &bytes[size as usize..];
    }
    Ok(found)
}

/// The IDs of `iinf`'s items of the type `mime` and the content type of XMP.
fn xmp_items(iinf: &[u8]) -> Result<Vec<u32>, &'static str> {
    let mut cursor = Cursor(iinf);
    let version = cursor.uint(1)?;
    cursor.take(3)?;
    let count = cursor.uint(if version == 0 { 2 } else { 4 })?;
    let mut ids = Vec::new();
    for (kind, body) in boxes(cursor.0)?.into_iter().take(count as usize) {
        if &kind != b"infe" {
            continue;
        }
        let mut infe = Cursor(body);
        let version = infe.uint(1)?;
        infe.take(3)?;
        if version < 2 {
            continue;
        }
        let id = infe.uint(if version == 2 { 2 } else { 4 })? as u32;
        infe.take(2)?;
        let item_type = infe.take(4)?;
        infe.string()?;
        if item_type == b"mime" && infe.string()?.trim() == "application/rdf+xml" {
            ids.push(id);
        }
    }
    Ok(ids)
}

/// Where an item is: in the file from `base`, or in `idat`, by `method`, 0 or 1.
struct Location {
    id: u32,
    method: u64,
    base: u64,
    /// Each extent's offset from `base` and its length.
    extents: Vec<(u64, u64)>,
}

/// `iloc`'s items.
fn iloc(body: &[u8]) -> Result<Vec<Location>, &'static str> {
    let mut cursor = Cursor(body);
    let version = cursor.uint(1)?;
    cursor.take(3)?;
    if version > 2 {
        return Err("its iloc is of an unknown version");
    }
    let sizes = cursor.uint(1)?;
    let (offset_size, length_size) = ((sizes >> 4) as usize, (sizes & 15) as usize);
    let sizes = cursor.uint(1)?;
    let base_size = (sizes >> 4) as usize;
    let index_size = if version > 0 {
        (sizes & 15) as usize
    } else {
        0
    };
    let count = cursor.uint(if version < 2 { 2 } else { 4 })?;
    let mut locations = Vec::new();
    for _ in 0..count {
        let id = cursor.uint(if version < 2 { 2 } else { 4 })? as u32;
        let method = if version > 0 { cursor.uint(2)? & 15 } else { 0 };
        cursor.take(2)?;
        let base = cursor.uint(base_size)?;
        let extents = cursor.uint(2)?;
        let mut location = Location {
            id,
            method,
            base,
            extents: Vec::new(),
        };
        for _ in 0..extents {
            cursor.uint(index_size)?;
            let offset = cursor.uint(offset_size)?;
            let length = cursor.uint(length_size)?;
            location.extents.push((offset, length));
        }
        locations.push(location);
    }
    Ok(locations)
}

/// Bytes read from the front.
struct Cursor<'a>(&'a [u8]);

impl<'a> Cursor<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], &'static str> {
        if count > self.0.len() {
            return Err("a box ends too soon");
        }
        let (taken, rest) = self.0.split_at(count);
        self.0 = rest;
        Ok(taken)
    }

    /// A big-endian unsigned number of `size` bytes, 0 to 8.
    fn uint(&mut self, size: usize) -> Result<u64, &'static str> {
        if size > 8 {
            return Err("a number is too long");
        }
        Ok(self
            .take(size)?
            .iter()
            .fold(0, |n, &byte| (n << 8) | u64::from(byte)))
    }

    /// A string ended by a NUL.
    fn string(&mut self) -> Result<String, &'static str> {
        let end = self
            .0
            .iter()
            .position(|&byte| byte == 0)
            .ok_or("a string isn't ended")?;
        let text = String::from_utf8_lossy(&self.0[..end]).into_owned();
        self.0 = &self.0[end + 1..];
        Ok(text)
    }
}

/// The variant, `solar`, `h24` or `apr`, and the base64 an XMP packet gives it, as an
/// attribute of an element, as Apple writes it, or as an element's text.
fn apple_desktop(xmp: &str) -> Option<(String, String)> {
    const NAMES: [&str; 3] = ["solar", "h24", "apr"];
    let mut reader = quick_xml::Reader::from_str(xmp);
    let mut inside: Option<String> = None;
    let mut text = String::new();
    loop {
        match reader.read_event().ok()? {
            quick_xml::events::Event::Start(start) | quick_xml::events::Event::Empty(start) => {
                for attribute in start.attributes().flatten() {
                    let key = String::from_utf8_lossy(attribute.key.as_ref()).into_owned();
                    if let Some(name) = key.strip_prefix("apple_desktop:")
                        && NAMES.contains(&name)
                    {
                        let value = attribute
                            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                            .ok()?
                            .into_owned();
                        return Some((name.to_owned(), value));
                    }
                }
                let name = String::from_utf8_lossy(start.name().as_ref()).into_owned();
                if let Some(name) = name.strip_prefix("apple_desktop:")
                    && NAMES.contains(&name)
                {
                    inside = Some(name.to_owned());
                    text.clear();
                }
            }
            quick_xml::events::Event::Text(t) if inside.is_some() => {
                text.push_str(&t.decode().ok()?);
            }
            quick_xml::events::Event::End(_) => {
                if let Some(name) = inside.take() {
                    return Some((name, text));
                }
            }
            quick_xml::events::Event::Eof => return None,
            _ => {}
        }
    }
}

/// The bytes of base64 `text`, whitespace in it ignored.
fn base64(text: &str) -> Option<Vec<u8>> {
    let value = |byte: u8| -> Option<u32> {
        Some(match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    };
    let digits: Vec<u8> = text
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    let digits = digits
        .strip_suffix(b"==")
        .or_else(|| digits.strip_suffix(b"="))
        .unwrap_or(&digits);
    if digits.len() % 4 == 1 {
        return None;
    }
    let mut bytes = Vec::with_capacity(digits.len() * 3 / 4);
    for chunk in digits.chunks(4) {
        let mut n = 0u32;
        for &digit in chunk {
            n = (n << 6) | value(digit)?;
        }
        n <<= 6 * (4 - chunk.len() as u32);
        let three = n.to_be_bytes();
        bytes.extend_from_slice(&three[1..chunk.len()]);
    }
    Some(bytes)
}

/// A value of a binary property list, as much of the format as Apple's metadata uses.
#[derive(Debug, Clone, PartialEq)]
enum Plist {
    Bool(bool),
    Integer(i64),
    Real(f64),
    String(String),
    Data(Vec<u8>),
    Array(Vec<Plist>),
    Dictionary(Vec<(String, Plist)>),
}

impl Plist {
    /// The value of a `bplist00`.
    fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let bad = Error::BadSchedule;
        if bytes.len() < 8 + 32 || &bytes[..8] != b"bplist00" {
            return Err(bad("it isn't a binary property list"));
        }
        let trailer = &bytes[bytes.len() - 32..];
        let offset_size = trailer[6] as usize;
        let reference_size = trailer[7] as usize;
        let field = |at: usize| {
            u64::from_be_bytes(
                trailer[at..at + 8]
                    .try_into()
                    .expect("the trailer's fields are eight bytes"),
            )
        };
        let (count, top, table) = (field(8), field(16), field(24));
        if !(1..=8).contains(&offset_size) || !(1..=8).contains(&reference_size) || top >= count {
            return Err(bad("its property list's trailer is wrong"));
        }
        let parser = PlistParser {
            bytes,
            offset_size,
            reference_size,
            count,
            table: table as usize,
        };
        parser
            .object(top, 0)
            .ok_or(bad("its property list is broken"))
    }

    fn get(&self, key: &str) -> Option<&Plist> {
        match self {
            Plist::Dictionary(entries) => entries
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    fn array(&self) -> Option<&[Plist]> {
        match self {
            Plist::Array(values) => Some(values),
            _ => None,
        }
    }

    fn number(&self) -> Option<f64> {
        match *self {
            Plist::Integer(n) => Some(n as f64),
            Plist::Real(n) if n.is_finite() => Some(n),
            _ => None,
        }
    }

    /// A picture's index: a whole number from 0.
    fn index(&self) -> Option<usize> {
        match *self {
            Plist::Integer(n) => usize::try_from(n).ok(),
            Plist::Real(n) if n >= 0.0 && n.fract() == 0.0 && n < 1e9 => Some(n as usize),
            _ => None,
        }
    }
}

struct PlistParser<'a> {
    bytes: &'a [u8],
    offset_size: usize,
    reference_size: usize,
    count: u64,
    table: usize,
}

impl PlistParser<'_> {
    fn uint(&self, at: usize, size: usize) -> Option<u64> {
        let bytes = self.bytes.get(at..at.checked_add(size)?)?;
        Some(bytes.iter().fold(0, |n, &byte| (n << 8) | u64::from(byte)))
    }

    /// Object `index`, `depth` arrays and dictionaries in.
    fn object(&self, index: u64, depth: usize) -> Option<Plist> {
        if index >= self.count || depth > PLIST_DEPTH {
            return None;
        }
        let at = self.uint(
            self.table.checked_add(index as usize * self.offset_size)?,
            self.offset_size,
        )? as usize;
        let marker = *self.bytes.get(at)?;
        let (kind, low) = (marker >> 4, marker & 15);
        // The length of a string, data, an array or a dictionary, and where its content is.
        let length = || -> Option<(usize, usize)> {
            if low != 15 {
                return Some((low as usize, at + 1));
            }
            let marker = *self.bytes.get(at + 1)?;
            if marker >> 4 != 1 {
                return None;
            }
            let size = 1usize << (marker & 15);
            Some((self.uint(at + 2, size)? as usize, at + 2 + size))
        };
        let references = |count: usize, start: usize| -> Option<Vec<u64>> {
            (0..count)
                .map(|n| self.uint(start + n * self.reference_size, self.reference_size))
                .collect()
        };
        Some(match kind {
            0 => match low {
                8 => Plist::Bool(false),
                9 => Plist::Bool(true),
                _ => return None,
            },
            1 => {
                let size = 1usize << low;
                if size > 8 {
                    return None;
                }
                let n = self.uint(at + 1, size)?;
                // 8 bytes are signed; fewer are unsigned.
                Plist::Integer(if size == 8 {
                    n as i64
                } else {
                    i64::try_from(n).ok()?
                })
            }
            2 => match low {
                2 => Plist::Real(f64::from(f32::from_bits(self.uint(at + 1, 4)? as u32))),
                3 => Plist::Real(f64::from_bits(self.uint(at + 1, 8)?)),
                _ => return None,
            },
            4 => {
                let (length, start) = length()?;
                Plist::Data(self.bytes.get(start..start.checked_add(length)?)?.to_vec())
            }
            5 => {
                let (length, start) = length()?;
                let bytes = self.bytes.get(start..start.checked_add(length)?)?;
                Plist::String(String::from_utf8_lossy(bytes).into_owned())
            }
            6 => {
                let (length, start) = length()?;
                let bytes = self
                    .bytes
                    .get(start..start.checked_add(length.checked_mul(2)?)?)?;
                let units: Vec<u16> = bytes
                    .chunks(2)
                    .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
                    .collect();
                Plist::String(String::from_utf16_lossy(&units))
            }
            10 => {
                let (length, start) = length()?;
                Plist::Array(
                    references(length, start)?
                        .into_iter()
                        .map(|reference| self.object(reference, depth + 1))
                        .collect::<Option<_>>()?,
                )
            }
            13 => {
                let (length, start) = length()?;
                let keys = references(length, start)?;
                let values = references(length, start + length * self.reference_size)?;
                let mut entries = Vec::new();
                for (key, value) in keys.into_iter().zip(values) {
                    let Plist::String(key) = self.object(key, depth + 1)? else {
                        return None;
                    };
                    entries.push((key, self.object(value, depth + 1)?));
                }
                Plist::Dictionary(entries)
            }
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests;
