//! The Lock screen's list of notifications (CMN-300), which the Locker reads through the
//! private API's `LockScreenList` and the daemon alone decides: a notification is on it if it
//! arrived since the session was locked, as Caminito's `Locked` says, and would have had a
//! bubble, so Delivery and Do Not Disturb with its break-through apply. It leaves it when it
//! leaves the history (its app withdraws it, or it's dismissed), and a transient one is never
//! there. The key `lock-screen` sets how much of it the Locker gets, and an app whose Previews
//! are off gives its name and "Notification". The Locker gets text only, in single lines and
//! limited in number and length, so that no notification can crash it into its give-up
//! state. `LockScreenChanged` tells it to ask again. Nothing about the list outlives the lock
//! or the daemon: a restarted Locker asks the same daemon and gets the same list.

use std::cell::RefCell;
use std::rc::Rc;

use gio::glib;
use gio::prelude::*;

use super::{State, emit_on};
use crate::markup;
use crate::notification::Notification;
use crate::private_api::{INTERFACE_NAME as PRIVATE_INTERFACE, PATH as PRIVATE_PATH};
use crate::settings::{AppSettings, LOCK_SCREEN, LockScreen, NO_PREVIEW, PREVIEWS};

/// Caminito's property that says whether the session is locked.
pub(super) const LOCKED: &str = "Locked";

/// How many notifications the list holds. The newest stay.
const MAX_ENTRIES: usize = 8;
/// The most characters of an app's name, of a summary and of a body.
const MAX_APP: usize = 60;
const MAX_SUMMARY: usize = 120;
const MAX_BODY: usize = 300;

/// An entry as the Locker gets it: the app, the summary and the body.
pub(super) type Text = (String, String, String);

/// What the Lock screen has of the lock, and what the Locker was last told.
#[derive(Default)]
pub(super) struct Lock {
    /// Whether Caminito says the session is locked.
    locked: bool,
    /// The notifications that arrived since the lock began and would have had a bubble, oldest
    /// first, each with the key its settings go by.
    arrived: Vec<(u32, String)>,
    /// The list as the Locker last heard it was changed.
    told: Vec<Text>,
    /// The global settings' handler.
    settings: Option<glib::SignalHandlerId>,
}

/// Starts following the keys that change the list.
pub(super) fn follow(state: &Rc<RefCell<State>>) {
    let mut guard = state.borrow_mut();
    let this = guard.this.clone();
    let handler = guard
        .settings
        .global()
        .connect_changed(None, move |_, key| {
            if key == LOCK_SCREEN || key == PREVIEWS {
                // A handler may run while the state is borrowed.
                let this = this.clone();
                glib::MainContext::ref_thread_default().spawn_local(async move {
                    if let Some(state) = this.upgrade() {
                        state.borrow_mut().refresh_lock_screen();
                    }
                });
            }
        });
    guard.lock_screen.settings = Some(handler);
}

/// Stops following, as the server stops.
pub(super) fn unfollow(state: &mut State) {
    if let Some(handler) = state.lock_screen.settings.take() {
        state.settings.global().disconnect(handler);
    }
}

impl State {
    /// Takes in what Caminito says of the lock: the list starts empty when the lock begins and
    /// empties when it ends.
    pub(super) fn refresh_locked(&mut self) {
        let locked: bool = self.caminito_says(LOCKED);
        if locked != self.lock_screen.locked {
            self.lock_screen.locked = locked;
            self.lock_screen.arrived.clear();
        }
        self.refresh_lock_screen();
    }

    /// Notes that the notification `id` arrived, while the session is locked, if it would have
    /// had a bubble, and that it didn't if it wouldn't. `app` is its app's settings.
    pub(super) fn track_for_lock_screen(
        &mut self,
        id: u32,
        notification: &Notification,
        app: &AppSettings,
    ) {
        if !self.lock_screen.locked {
            return;
        }
        let bubbles = app.delivery == crate::settings::Delivery::Bubble
            && (!self.do_not_disturb_now.is_on() || self.breaks_through(notification));
        let arrived = &mut self.lock_screen.arrived;
        if !bubbles {
            arrived.retain(|(arrived, _)| *arrived != id);
        } else if !arrived.iter().any(|(arrived, _)| *arrived == id) {
            arrived.push((id, notification.settings_key.clone()));
        }
    }

    /// The list the Locker gets: newest first.
    pub(super) fn lock_screen_list(&self) -> Vec<Text> {
        let mode = LockScreen::of(self.settings.global());
        if mode == LockScreen::None || !self.lock_screen.locked {
            return Vec::new();
        }
        self.history
            .entries()
            .filter_map(|entry| {
                let (_, key) = self
                    .lock_screen
                    .arrived
                    .iter()
                    .find(|(id, _)| *id == entry.id)?;
                let app = markup::line(&entry.bubble.app_name, MAX_APP);
                if !self.settings.effective(key).previews {
                    return Some((app, NO_PREVIEW.to_owned(), String::new()));
                }
                let summary = markup::line(&entry.bubble.summary, MAX_SUMMARY);
                let body = match mode {
                    LockScreen::Full => markup::line(&markup::plain(&entry.bubble.body), MAX_BODY),
                    _ => String::new(),
                };
                Some((app, summary, body))
            })
            .take(MAX_ENTRIES)
            .collect()
    }

    /// Tells the Locker to ask again if the list isn't what it last heard.
    pub(super) fn refresh_lock_screen(&mut self) {
        let list = self.lock_screen_list();
        if list == self.lock_screen.told {
            return;
        }
        self.lock_screen.told = list;
        emit_on(
            &self.connection,
            PRIVATE_PATH,
            PRIVATE_INTERFACE,
            "LockScreenChanged",
            (),
        );
    }
}
