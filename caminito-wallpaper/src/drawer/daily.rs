//! The drawer's pictures of the day (214): each shown source's latest picture, read from
//! the cache, and its fetches, run on GLib's loop through [`Http`]. A source is fetched when
//! it's first shown in a session, after local midnight, on a wake from suspend, when the
//! network comes back after a fetch that failed, and within the hour while the day's
//! picture isn't in ([`Attempt::due`]). Whatever fails, the last picture stays.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gio::glib;
use gio::prelude::*;

use super::Refresh;
use crate::DOMAIN;
use crate::daily::{Attempt, Cache, Day, Error, Kept, Source, update};
use crate::http::Http;

/// The pictures of the day, shared with their fetches and the signals that start them.
pub(super) struct Daily {
    cache: Cache,
    /// Each source's latest picture, read from the cache the first time it's shown.
    latest: HashMap<Source, Option<Kept>>,
    attempts: HashMap<Source, Attempt>,
    /// The sources being fetched.
    running: HashSet<Source>,
    /// Logind's `PrepareForSleep`, while the drawer listens to it.
    _sleep: Option<gio::SignalSubscription>,
}

impl Daily {
    /// The pictures of the day in the user's cache, fetched again on a wake and when the
    /// network comes back, each of which schedules `refresh`.
    pub(super) fn open(refresh: &Refresh) -> Rc<RefCell<Daily>> {
        let daily = Rc::new(RefCell::new(Daily {
            cache: Cache::user(),
            latest: HashMap::new(),
            attempts: HashMap::new(),
            running: HashSet::new(),
            _sleep: None,
        }));
        let sleep = gio::bus_get_sync(gio::BusType::System, None::<&gio::Cancellable>)
            .inspect_err(|err| {
                glib::g_debug!(DOMAIN, "can't hear a wake from suspend: {err}");
            })
            .ok()
            .map(|bus| {
                let (weak, refresh) = (Rc::downgrade(&daily), refresh.clone());
                bus.subscribe_to_signal(
                    Some("org.freedesktop.login1"),
                    Some("org.freedesktop.login1.Manager"),
                    Some("PrepareForSleep"),
                    Some("/org/freedesktop/login1"),
                    None,
                    gio::DBusSignalFlags::NONE,
                    move |signal| {
                        let woke = signal.parameters.get::<(bool,)>() == Some((false,));
                        if let Some(daily) = weak.upgrade().filter(|_| woke) {
                            daily.borrow_mut().attempts.clear();
                            refresh.schedule();
                        }
                    },
                )
            });
        daily.borrow_mut()._sleep = sleep;
        let (weak, refresh) = (Rc::downgrade(&daily), refresh.clone());
        gio::NetworkMonitor::default().connect_network_changed(move |_, available| {
            if let Some(daily) = weak.upgrade().filter(|_| available) {
                daily
                    .borrow_mut()
                    .attempts
                    .retain(|_, attempt| attempt.got_today);
                refresh.schedule();
            }
        });
        daily
    }

    /// The latest picture of `source`, if one is kept.
    pub(super) fn latest(&mut self, source: Source) -> Option<Kept> {
        let cache = &self.cache;
        self.latest
            .entry(source)
            .or_insert_with(|| cache.latest(source))
            .clone()
    }

    /// Forgets every source but those in `shown`, but for their attempts, so that showing
    /// one again doesn't fetch it again the same day.
    pub(super) fn keep_only(&mut self, shown: &[Source]) {
        self.latest.retain(|source, _| shown.contains(source));
    }
}

/// Fetches `source` if it's due at `local`, seconds of the local clock, and schedules
/// `refresh` once a new picture is kept.
pub(super) fn fetch_if_due(
    daily: &Rc<RefCell<Daily>>,
    source: Source,
    local: i64,
    refresh: &Refresh,
) {
    let cache = {
        let mut state = daily.borrow_mut();
        if state.running.contains(&source) || !Attempt::due(state.attempts.get(&source), local) {
            return;
        }
        state.running.insert(source);
        state.attempts.insert(
            source,
            Attempt {
                day: Day::of_local(local),
                at: local,
                got_today: false,
            },
        );
        state.cache.clone()
    };
    let today = Day::of_local(local);
    let (daily, refresh) = (daily.clone(), refresh.clone());
    glib::MainContext::default().spawn_local(async move {
        let result = update(&Http, &cache, source, today).await;
        let mut state = daily.borrow_mut();
        state.running.remove(&source);
        match result {
            Ok(kept) => {
                if let Some(attempt) = state.attempts.get_mut(&source) {
                    attempt.got_today = kept.day >= today;
                }
                if state.latest.get(&source).and_then(Option::as_ref) != Some(&kept) {
                    glib::g_debug!(
                        DOMAIN,
                        "{}'s picture of the day is {}",
                        source.name(),
                        kept.picture.display()
                    );
                    state.latest.insert(source, Some(kept));
                    refresh.schedule();
                }
            }
            Err(err @ Error::NotAPicture(_)) => {
                glib::g_message!(DOMAIN, "{}: {err}, so the last one stays", source.name());
            }
            Err(err) => {
                glib::g_warning!(
                    DOMAIN,
                    "can't fetch {}'s picture of the day, so the last one stays: {err}",
                    source.name()
                );
            }
        }
    });
}
