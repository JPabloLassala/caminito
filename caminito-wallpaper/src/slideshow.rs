//! The Slideshow (213): a folder's pictures one after another, changing at a whole interval
//! of the local clock, in order of their names or at random.
//!
//! What shows is worked out from the clock alone, with no state kept: the time is cut into
//! slots of the interval, counted from 1970 in local wall-clock seconds as a timed XML's are
//! (216), so a change of a day is at local midnight, and a restarted drawer shows what it
//! showed. At random, the slots are taken in rounds as long as the folder: each round is a
//! shuffle of every picture, seeded by the folder and the round, so every picture shows
//! once before any shows again. A round doesn't know the one before it, so where one ends
//! and the next begins a picture can come twice in a row, one time in as many as the folder
//! has pictures: the price of an order that follows from the clock alone.
//!
//! Each output on the same folder has a rank, and shows the picture `rank` places further
//! on in the same order, so two outputs never show the same picture while the folder has
//! enough. Nothing here reads a clock or names a toolkit.

use std::path::{Path, PathBuf};

use crate::grid::pictures_in;
use crate::settings::{Kind, OutputSettings};

const HOUR: i64 = 3600;
const DAY: i64 = 24 * HOUR;

/// How long each picture of a slideshow shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Interval {
    /// Every 5 minutes of the clock.
    FiveMinutes,
    /// Every 15 minutes, the default: neither restless nor stale.
    #[default]
    FifteenMinutes,
    /// Every hour, on the hour.
    Hour,
    /// Every day, at local midnight.
    Day,
}

impl Interval {
    /// Each interval, in the schema's order.
    pub const ALL: [Self; 4] = [
        Self::FiveMinutes,
        Self::FifteenMinutes,
        Self::Hour,
        Self::Day,
    ];

    /// Its nick in the schema's `org.mate.Caminito.WallpaperInterval`.
    pub fn nick(self) -> &'static str {
        match self {
            Self::FiveMinutes => "5-minutes",
            Self::FifteenMinutes => "15-minutes",
            Self::Hour => "hour",
            Self::Day => "day",
        }
    }

    /// The interval of a nick, or the default for one the schema doesn't have.
    pub fn from_nick(nick: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|interval| interval.nick() == nick)
            .unwrap_or_default()
    }

    /// Its length in seconds.
    pub fn seconds(self) -> i64 {
        match self {
            Self::FiveMinutes => 5 * 60,
            Self::FifteenMinutes => 15 * 60,
            Self::Hour => HOUR,
            Self::Day => DAY,
        }
    }
}

/// A slideshow's pictures, as its folder has them now, and how they're shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slideshow {
    /// By name.
    pictures: Vec<PathBuf>,
    interval: Interval,
    in_order: bool,
    /// The random order's seed, the folder's, so the drawer and the dialog agree.
    seed: u64,
}

impl Slideshow {
    /// The slideshow of the pictures in `folder`'s own level, by name. A folder that's
    /// missing or can't be read has none.
    pub fn read(folder: &Path, interval: Interval, in_order: bool) -> Self {
        Self::new(folder, pictures_in(folder, 1), interval, in_order)
    }

    /// The slideshow of `pictures`, by name, from `folder`.
    pub(crate) fn new(
        folder: &Path,
        mut pictures: Vec<PathBuf>,
        interval: Interval,
        in_order: bool,
    ) -> Self {
        pictures.sort();
        Slideshow {
            pictures,
            interval,
            in_order,
            seed: fnv(folder.as_os_str().as_encoded_bytes()),
        }
    }

    /// How many pictures it has.
    pub fn len(&self) -> usize {
        self.pictures.len()
    }

    /// Whether it has no picture, so the output shows its colour.
    pub fn is_empty(&self) -> bool {
        self.pictures.is_empty()
    }

    /// The picture the output of rank `rank` shows at `local`, the local wall-clock time in
    /// seconds since 1970-01-01 00:00 local, or `None` with no picture.
    pub fn picture(&self, local: i64, rank: usize) -> Option<&Path> {
        let count = self.pictures.len() as i64;
        if count == 0 {
            return None;
        }
        let slot = local.div_euclid(self.interval.seconds());
        let place = (slot.rem_euclid(count) + rank as i64) % count;
        let index = if self.in_order {
            place
        } else {
            let round = slot.div_euclid(count);
            shuffled(self.pictures.len(), self.seed ^ splitmix(round as u64))[place as usize] as i64
        };
        self.pictures.get(index as usize).map(PathBuf::as_path)
    }

    /// How many seconds after `local` it next shows another picture: at the next whole
    /// interval. `None` with fewer than two pictures, which never change.
    pub fn next_change(&self, local: i64) -> Option<u64> {
        if self.pictures.len() < 2 {
            return None;
        }
        let interval = self.interval.seconds();
        Some((interval - local.rem_euclid(interval)) as u64)
    }
}

/// Each output's rank among the outputs whose Slideshow shows the same folder, or `None` for
/// an output that isn't a Slideshow. Each output is given as the path element of its
/// monitor's identity and its settings, in any order, and the ranks come in that order.
/// Ranked by identity, a monitor keeps its rank whatever order the outputs come in, and the
/// drawer and the dialog's sidebar agree.
pub(crate) fn ranks(shown: &[(&str, &OutputSettings)]) -> Vec<Option<usize>> {
    shown
        .iter()
        .map(|&(element, settings)| {
            let folder = folder_of(settings)?;
            let before = shown
                .iter()
                .filter(|&&(other, others)| other < element && folder_of(others) == Some(folder))
                .count();
            Some(before)
        })
        .collect()
}

/// The folder of a Slideshow, or `None` for another kind.
fn folder_of(settings: &OutputSettings) -> Option<&str> {
    (settings.kind == Kind::Slideshow).then_some(settings.slideshow.as_str())
}

/// `0..count` shuffled by Fisher and Yates, with `seed`'s draws.
fn shuffled(count: usize, seed: u64) -> Vec<usize> {
    let mut order: Vec<usize> = (0..count).collect();
    let mut state = seed;
    for last in (1..count).rev() {
        state = splitmix(state);
        let pick = (state % (last as u64 + 1)) as usize;
        order.swap(last, pick);
    }
    order
}

/// SplitMix64's step: a well-mixed 64 bits of any 64, so neighbouring rounds and folders
/// shuffle unalike. No crate for it, as the order needs only to look random and be the same
/// in the drawer and the dialog.
fn splitmix(value: u64) -> u64 {
    let mut z = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// FNV-1a's 64-bit hash, which, unlike std's, is the same from one build to the next.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests;
