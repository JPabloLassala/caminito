//! The Notification history (CMN-259): the notifications the user hasn't dismissed, newest
//! first, kept in a file under `$XDG_STATE_HOME/caminito/` so that a restart of the daemon or
//! a new Session keeps them. It holds at most [`MAX_AGE`] and [`MAX_ENTRIES`], and takes the
//! time as an argument, so tests don't wait for a week.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gio::glib::{self, Variant};
use gio::prelude::*;

use crate::DOMAIN;
use crate::notification::{Notification, icon, pixels};
use crate::server::{Action, Bubble, Icon, Image};

/// How long an entry stays, in seconds: a week.
pub(crate) const MAX_AGE: u64 = 7 * 24 * 60 * 60;

/// How many entries it holds. The oldest go first.
pub(crate) const MAX_ENTRIES: usize = 200;

/// The most bytes of an `image-data` image the file keeps. A bigger one is in the entry
/// until the daemon stops, so that rewriting the file with every notification stays cheap.
const MAX_PERSISTED_PIXELS: usize = 64 * 1024;

/// The file's layout: a version, and the entries. A file of another version is not read.
const FILE_VERSION: u32 = 1;
const FILE_TYPE: &str = "(ua(usssssva(ss)bbbt))";

/// The type of an entry on the wire and in the file: id, app key, app name, app icon,
/// summary, body, image (a string, or `(iiibiiay)` pixels), actions, has a default action,
/// resident, has a reply field, and its time in seconds since the epoch.
pub(crate) const ENTRY_TYPE: &str = "(usssssva(ss)bbbt)";

/// A notification in the history.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HistoryEntry {
    pub(crate) id: u32,
    /// The Notifying app's key: its `desktop-entry`, or else its `app_name`.
    pub(crate) app: String,
    /// What a bubble shows of it.
    pub(crate) bubble: Bubble,
    /// Whether a click invokes the default action.
    pub(crate) has_default: bool,
    /// Whether it stays after one of its actions is used.
    pub(crate) resident: bool,
    /// Whether it has a reply field.
    pub(crate) reply: bool,
    /// When it arrived, in seconds since the epoch.
    pub(crate) time: u64,
}

/// The history, and the file it keeps itself in.
pub(crate) struct History {
    path: Option<PathBuf>,
    /// Oldest first.
    entries: Vec<HistoryEntry>,
    /// Counts the changes, for the server to see whether a call changed anything.
    generation: u64,
}

impl History {
    /// An empty history, kept in `path` if there's one.
    fn new(path: Option<PathBuf>) -> History {
        History {
            path,
            entries: Vec::new(),
            generation: 0,
        }
    }

    /// The history `path` holds at `now`. A file that's missing is an empty history, and one
    /// that can't be read is too, with a warning.
    pub(crate) fn load(path: PathBuf, now: u64) -> History {
        let mut history = History::new(None);
        match std::fs::read(&path) {
            Ok(bytes) => match decode(&bytes) {
                Some(entries) => history.entries = entries,
                None => glib::g_warning!(DOMAIN, "can't read the history {}", path.display()),
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                glib::g_warning!(DOMAIN, "can't read the history {}: {err}", path.display())
            }
        }
        history.path = Some(path);
        history.entries.sort_by_key(|entry| entry.time);
        history.prune(now);
        history
    }

    /// Writes the file.
    pub(crate) fn save(&self) {
        let Some(path) = &self.path else {
            return;
        };
        if let Err(err) = write(path, &encode(&self.entries)) {
            glib::g_warning!(DOMAIN, "can't write the history {}: {err}", path.display());
        }
    }

    /// How many changes the history has had, which only grows, so that a caller can tell
    /// whether a call changed anything.
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    /// The entries, newest first.
    pub(crate) fn entries(&self) -> impl Iterator<Item = &HistoryEntry> {
        self.entries.iter().rev()
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// The entry `id`, if the history has it.
    pub(crate) fn get(&self, id: u32) -> Option<&HistoryEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// The highest id it holds, or 0.
    pub(crate) fn max_id(&self) -> u32 {
        self.entries.iter().map(|entry| entry.id).max().unwrap_or(0)
    }

    /// The ids of an app's entries, newest first.
    pub(crate) fn ids_of(&self, app: &str) -> Vec<u32> {
        self.entries()
            .filter(|entry| entry.app == app)
            .map(|entry| entry.id)
            .collect()
    }

    /// How many entries each app has.
    pub(crate) fn counts(&self) -> BTreeMap<String, u32> {
        let mut counts = BTreeMap::new();
        for entry in &self.entries {
            *counts.entry(entry.app.clone()).or_insert(0) += 1;
        }
        counts
    }

    /// Adds `entry` as the newest, in place of the one with its id, and drops what the limits
    /// no longer allow at `now`.
    pub(crate) fn insert(&mut self, entry: HistoryEntry, now: u64) {
        self.entries.retain(|held| held.id != entry.id);
        self.entries.push(entry);
        self.generation += 1;
        self.prune(now);
        if let Some(extra) = self.entries.len().checked_sub(MAX_ENTRIES) {
            self.entries.drain(..extra);
        }
    }

    /// Removes the entry `id`, if there's one.
    pub(crate) fn remove(&mut self, id: u32) -> bool {
        !self.remove_all(&[id]).is_empty()
    }

    /// Removes the entries `ids` name, and returns those that were there.
    pub(crate) fn remove_all(&mut self, ids: &[u32]) -> Vec<u32> {
        let mut removed = Vec::new();
        self.entries.retain(|entry| {
            let named = ids.contains(&entry.id);
            if named {
                removed.push(entry.id);
            }
            !named
        });
        if !removed.is_empty() {
            self.generation += 1;
        }
        removed
    }

    /// Removes what's older than [`MAX_AGE`] at `now`, and returns the ids that went.
    pub(crate) fn prune(&mut self, now: u64) -> Vec<u32> {
        let ids: Vec<u32> = self
            .entries
            .iter()
            .filter(|entry| entry.time.saturating_add(MAX_AGE) <= now)
            .map(|entry| entry.id)
            .collect();
        self.remove_all(&ids)
    }

    /// When the oldest entry goes, in seconds since the epoch.
    pub(crate) fn expires_at(&self) -> Option<u64> {
        self.entries
            .iter()
            .map(|entry| entry.time.saturating_add(MAX_AGE))
            .min()
    }
}

impl HistoryEntry {
    /// The entry of `notification` as `id`, which arrived at `time`.
    pub(crate) fn new(id: u32, notification: &Notification, time: u64) -> HistoryEntry {
        HistoryEntry {
            id,
            app: notification.app.clone(),
            bubble: notification.bubble.clone(),
            has_default: notification.has_default,
            resident: notification.resident,
            reply: notification.reply,
            time,
        }
    }

    /// The entry on the wire, which has all its pixels.
    pub(crate) fn to_variant(&self) -> Variant {
        let image = match &self.bubble.image {
            None => "".to_variant(),
            Some(Image::Icon(icon)) => icon_text(icon).to_variant(),
            Some(Image::Pixels(pixels)) => (
                pixels.width,
                pixels.height,
                pixels.rowstride,
                pixels.has_alpha,
                pixels.bits_per_sample,
                pixels.channels,
                pixels.data.to_vec(),
            )
                .to_variant(),
        };
        let actions: Vec<(&str, &str)> = self
            .bubble
            .actions
            .iter()
            .map(|action| (action.key.as_str(), action.label.as_str()))
            .collect();
        Variant::tuple_from_iter([
            self.id.to_variant(),
            self.app.to_variant(),
            self.bubble.app_name.to_variant(),
            self.bubble
                .app_icon
                .as_ref()
                .map_or_else(String::new, icon_text)
                .to_variant(),
            self.bubble.summary.to_variant(),
            self.bubble.body.to_variant(),
            Variant::from_variant(&image),
            actions.to_variant(),
            self.has_default.to_variant(),
            self.resident.to_variant(),
            self.reply.to_variant(),
            self.time.to_variant(),
        ])
    }

    /// The entry a variant of [`ENTRY_TYPE`] holds.
    fn from_variant(variant: &Variant) -> Option<HistoryEntry> {
        let text = |index| variant.child_value(index).str().map(str::to_owned);
        let image = variant.child_value(6).as_variant()?;
        let image = match image.str() {
            Some(text) => icon(text).map(Image::Icon),
            None => pixels(&image).map(Image::Pixels),
        };
        let actions: Vec<(String, String)> = variant.child_value(7).get()?;
        Some(HistoryEntry {
            id: variant.child_value(0).get()?,
            app: text(1)?,
            bubble: Bubble {
                app_name: text(2)?,
                app_icon: icon(&text(3)?),
                summary: text(4)?,
                body: text(5)?,
                image,
                actions: actions
                    .into_iter()
                    .map(|(key, label)| Action { key, label })
                    .collect(),
                earlier: Vec::new(),
            },
            has_default: variant.child_value(8).get()?,
            resident: variant.child_value(9).get()?,
            reply: variant.child_value(10).get()?,
            time: variant.child_value(11).get()?,
        })
    }
}

/// An icon as text: a file's path, or an icon theme's name.
fn icon_text(icon: &Icon) -> String {
    match icon {
        Icon::Name(name) => name.clone(),
        Icon::File(path) => path.to_string_lossy().into_owned(),
    }
}

/// The file's bytes for `entries`, oldest first.
fn encode(entries: &[HistoryEntry]) -> Vec<u8> {
    let entries: Vec<Variant> = entries
        .iter()
        .map(|entry| {
            let mut entry = entry.clone();
            if let Some(Image::Pixels(pixels)) = &entry.bubble.image
                && pixels.data.len() > MAX_PERSISTED_PIXELS
            {
                entry.bubble.image = None;
            }
            entry.to_variant()
        })
        .collect();
    let entry_type = glib::VariantTy::new(ENTRY_TYPE).expect("the entry type is valid");
    let list = Variant::array_from_iter_with_type(entry_type, entries);
    Variant::tuple_from_iter([FILE_VERSION.to_variant(), list])
        .data()
        .to_vec()
}

/// The entries `bytes` hold, if they're a file of this version.
fn decode(bytes: &[u8]) -> Option<Vec<HistoryEntry>> {
    let file_type = glib::VariantTy::new(FILE_TYPE).expect("the file type is valid");
    let file = Variant::from_bytes_with_type(&glib::Bytes::from(bytes), file_type);
    if !file.is_normal_form() || file.child_value(0).get::<u32>()? != FILE_VERSION {
        return None;
    }
    let list = file.child_value(1);
    // An entry that doesn't read is lost, and the others stay.
    Some(
        (0..list.n_children())
            .filter_map(|index| HistoryEntry::from_variant(&list.child_value(index)))
            .collect(),
    )
}

/// Writes `bytes` to `path`, through a temporary file so that a crash leaves the old one.
fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};

    // The bodies of a user's notifications are for the user alone.
    if let Some(directory) = path.parent() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory)?;
    }
    let temporary = path.with_extension("tmp");
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&temporary)?
        .write_all(bytes)?;
    std::fs::rename(&temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::Pixels;

    fn entry(id: u32, app: &str, time: u64) -> HistoryEntry {
        HistoryEntry {
            id,
            app: app.to_owned(),
            bubble: Bubble {
                app_name: app.to_owned(),
                app_icon: Some(Icon::Name("mail-unread".to_owned())),
                summary: format!("Summary {id}"),
                body: "A <b>body</b>".to_owned(),
                image: None,
                actions: vec![Action {
                    key: "reply".to_owned(),
                    label: "Reply".to_owned(),
                }],
                earlier: Vec::new(),
            },
            has_default: true,
            resident: false,
            reply: true,
            time,
        }
    }

    fn ids(history: &History) -> Vec<u32> {
        history.entries().map(|entry| entry.id).collect()
    }

    #[test]
    fn the_newest_entry_comes_first_and_inserting_an_id_again_replaces_it() {
        let mut history = History::new(None);
        history.insert(entry(1, "a", 10), 10);
        history.insert(entry(2, "b", 11), 11);
        history.insert(entry(1, "a", 12), 12);

        assert_eq!(ids(&history), [1, 2]);
        assert_eq!(history.get(1).map(|entry| entry.time), Some(12));
    }

    #[test]
    fn an_entry_older_than_a_week_goes_and_a_younger_one_stays() {
        let mut history = History::new(None);
        history.insert(entry(1, "a", 100), 100);
        history.insert(entry(2, "a", 101), 101);

        assert!(history.prune(100 + MAX_AGE - 1).is_empty());
        assert_eq!(history.prune(100 + MAX_AGE), [1]);
        assert_eq!(ids(&history), [2]);
        assert_eq!(history.prune(101 + MAX_AGE), [2]);
        assert_eq!(history.expires_at(), None);
    }

    #[test]
    fn the_oldest_entries_go_beyond_two_hundred() {
        let mut history = History::new(None);
        for id in 1..=MAX_ENTRIES as u32 + 2 {
            history.insert(entry(id, "a", 10), 10);
        }

        assert_eq!(history.len(), MAX_ENTRIES);
        assert_eq!(history.get(1).map(|entry| entry.id), None);
        assert_eq!(history.get(2).map(|entry| entry.id), None);
        assert_eq!(history.get(3).map(|entry| entry.id), Some(3));
    }

    #[test]
    fn counts_are_per_app_and_removals_say_what_went() {
        let mut history = History::new(None);
        history.insert(entry(1, "a", 10), 10);
        history.insert(entry(2, "b", 10), 10);
        history.insert(entry(3, "a", 10), 10);

        assert_eq!(
            history.counts().into_iter().collect::<Vec<_>>(),
            [("a".to_owned(), 2), ("b".to_owned(), 1)]
        );
        assert_eq!(history.ids_of("a"), [3, 1]);
        assert_eq!(history.remove_all(&[1, 2, 9]), [1, 2]);
        assert_eq!(ids(&history), [3]);
    }

    #[test]
    fn the_file_keeps_the_entries_for_the_next_daemon() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .join("caminito")
            .join("notification-history");
        let mut history = History::new(Some(path.clone()));
        let mut with_pixels = entry(2, "b", 11);
        with_pixels.bubble.image = Some(Image::Icon(Icon::File("/tmp/a.png".into())));
        history.insert(entry(1, "a", 10), 10);
        history.insert(with_pixels.clone(), 11);
        history.save();

        let loaded = History::load(path, 12);

        assert_eq!(
            loaded.entries().cloned().collect::<Vec<_>>(),
            [with_pixels, entry(1, "a", 10)]
        );
        assert_eq!(loaded.max_id(), 2);
    }

    #[test]
    fn a_missing_or_broken_file_is_an_empty_history_and_old_entries_are_not_loaded() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history");
        assert_eq!(History::load(path.clone(), 0).len(), 0);

        std::fs::write(&path, b"not a variant").unwrap();
        assert_eq!(History::load(path.clone(), 0).len(), 0);

        let mut history = History::new(Some(path.clone()));
        history.insert(entry(1, "a", 10), 10);
        history.save();
        assert_eq!(History::load(path, 10 + MAX_AGE).len(), 0);
    }

    #[test]
    fn pixels_are_kept_in_the_file_only_when_they_are_small() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history");
        let pixels = |size: usize| {
            Image::Pixels(Pixels {
                width: 1,
                height: 1,
                rowstride: 4,
                has_alpha: true,
                bits_per_sample: 8,
                channels: 4,
                data: glib::Bytes::from_owned(vec![7u8; size.max(4)]),
            })
        };
        let mut small = entry(1, "a", 10);
        small.bubble.image = Some(pixels(4));
        let mut large = entry(2, "a", 10);
        large.bubble.image = Some(pixels(MAX_PERSISTED_PIXELS + 1));
        let mut history = History::new(Some(path.clone()));
        history.insert(small.clone(), 10);
        history.insert(large.clone(), 10);
        history.save();

        let loaded = History::load(path, 10);

        assert_eq!(loaded.get(1), Some(&small));
        assert_eq!(loaded.get(2).map(|entry| &entry.bubble.image), Some(&None));
        // The variant that goes over D-Bus has all of them.
        assert_eq!(history.get(2).unwrap().to_variant().n_children(), 12);
    }
}
