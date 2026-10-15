//! The two bridges to `org.mate.background` (211), which isn't the store any more (spec):
//! out, the primary output's picture, positioning and colour, for whatever still reads
//! MATE's keys; and in, a picture another program writes to `picture-filename`, caja's "Set
//! as wallpaper" among them, as the Picture of every output.
//!
//! The drawer hears its own writes to `picture-filename` as it hears anyone's, so each is
//! noted before it's written and passed over when it comes back. A guard on the value alone
//! wouldn't do: another program writing the file the primary output already shows means
//! every other output is to show it too, wherever GSettings signals that write. The keyfile
//! backend doesn't, and dconf may not either, since it can drop a write that changes nothing.

use crate::settings::{Colour, Drawn, Kind, MateBackground, Set, Store};

/// What the bridges remember between the writes and the changes heard.
#[derive(Debug, Default)]
pub struct Bridge {
    /// The `picture-filename` the drawer wrote and hasn't heard back yet.
    own: Option<String>,
    /// The `picture-filename` another program wrote, not yet every output's.
    heard: Option<String>,
}

/// The keys of `org.mate.background` to write, each `None` where it says so already.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Writes {
    /// `picture-filename`.
    pub picture: Option<String>,
    /// `picture-options`.
    pub options: Option<&'static str>,
    /// `primary-color`, which is solid colour.
    pub colour: Option<Colour>,
}

impl Writes {
    /// Whether there's nothing to write.
    pub fn is_empty(&self) -> bool {
        *self == Writes::default()
    }
}

impl Bridge {
    /// What to write so that MATE's keys, `now`, say what the primary output shows: its
    /// picture and positioning, or for a colour an empty filename, and its colour either way.
    /// A new `picture-filename` is noted as the drawer's own, to be passed over when heard.
    pub fn out(&mut self, now: &MateBackground, primary: &Drawn) -> Writes {
        // MATE has one picture: for a dynamic wallpaper part way, the one that shows most
        // (216). It has no options for no picture, so they stay as they are.
        let (picture, options) = match primary {
            Drawn::Picture { positioning, .. } | Drawn::Blend { positioning, .. } => (
                primary.nearer().unwrap_or_default(),
                Some(positioning.mate()),
            ),
            Drawn::Colour(_) => ("", None),
        };
        let colour = primary.colour();
        let writes = Writes {
            picture: (now.picture != picture).then(|| picture.to_owned()),
            options: options.filter(|options| now.options != *options),
            colour: (Colour::parse(&now.colour) != Some(colour)).then_some(colour),
        };
        if let Some(picture) = &writes.picture {
            self.own = Some(picture.clone());
        }
        writes
    }

    /// The writes `out` gave failed, so the drawer won't hear its picture back.
    pub fn not_written(&mut self) {
        self.own = None;
    }

    /// `picture-filename` changed to `file`: the drawer's own write coming back, or
    /// another program's, to be brought in. Returns whether it's another's.
    pub fn heard(&mut self, file: &str) -> bool {
        if self.own.as_deref() == Some(file) {
            self.own = None;
            false
        } else {
            self.heard = Some(file.to_owned());
            true
        }
    }

    /// Makes the picture another program wrote last the Picture of every output: every
    /// output's set and each monitor's seen, each keeping its positioning and colour.
    /// Returns whether there was one. An empty filename, MATE's "no picture", isn't one,
    /// since Caminito's Colour has its own colour to choose.
    pub fn bring_in(&mut self, store: &mut impl Store) -> bool {
        let Some(file) = self.heard.take().filter(|file| !file.is_empty()) else {
            return false;
        };
        let seen = store.seen().into_iter().map(Set::Seen);
        for set in std::iter::once(Set::All).chain(seen) {
            let mut settings = store.read(&set);
            settings.kind = Kind::Picture;
            settings.picture = file.clone();
            store.write(&set, &settings);
        }
        true
    }
}

#[cfg(test)]
mod tests;
