//! The Notifying apps whose grouping is off (CMN-265), which the private API's `Ungrouped`
//! gives and `UngroupedChanged` follows, for the Notification center to leave their entries
//! out of the groups. They're among the known apps and the apps of the history, each by its own
//! `group` or else the default's, which are followed as they change, from the settings window
//! or from outside the daemon.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::rc::{Rc, Weak};

use gio::glib;
use gio::prelude::*;

use super::{State, emit_on};
use crate::private_api::{INTERFACE_NAME as PRIVATE_INTERFACE, PATH as PRIVATE_PATH};
use crate::settings::GROUP;

/// The apps with grouping off, and the settings followed to keep them.
#[derive(Default)]
pub(super) struct Ungrouped {
    /// Their keys, in order.
    pub(super) apps: Vec<String>,
    /// Each app followed, with its own settings, which only send `changed` while they're kept.
    followed: HashMap<String, gio::Settings>,
    /// The global settings' handler, which hears of the default's `group` and the known apps.
    global: Option<glib::SignalHandlerId>,
    /// Whether a look again is on its way, so that a burst of changes makes one.
    queued: Rc<Cell<bool>>,
}

/// Starts following the settings, and finds the apps with grouping off now.
pub(super) fn follow(state: &Rc<RefCell<State>>) {
    let mut guard = state.borrow_mut();
    let (this, queued) = (guard.this.clone(), guard.ungrouped.queued.clone());
    let handler = guard
        .settings
        .global()
        .connect_changed(None, move |_, _| queue(&this, &queued));
    guard.ungrouped.global = Some(handler);
    drop(guard);
    refresh(state);
}

/// Stops following the global settings, which outlive the server.
pub(super) fn unfollow(state: &mut State) {
    if let Some(handler) = state.ungrouped.global.take() {
        state.settings.global().disconnect(handler);
    }
}

impl State {
    /// Looks at the apps again soon, as one may have joined the history.
    pub(super) fn queue_ungrouped(&self) {
        queue(&self.this, &self.ungrouped.queued);
    }
}

/// Looks at the apps again once the change that called it is over: a handler runs while the
/// state may be borrowed.
fn queue(this: &Weak<RefCell<State>>, queued: &Rc<Cell<bool>>) {
    if queued.replace(true) {
        return;
    }
    let (this, queued) = (this.clone(), queued.clone());
    glib::MainContext::ref_thread_default().spawn_local(async move {
        queued.set(false);
        if let Some(state) = this.upgrade() {
            refresh(&state);
        }
    });
}

/// Follows the apps not yet followed, and tells the clients if the apps with grouping off
/// changed.
fn refresh(state: &Rc<RefCell<State>>) {
    let mut guard = state.borrow_mut();
    let mut apps: BTreeSet<String> = guard.settings.known_apps().into_iter().collect();
    apps.extend(guard.history.entries().map(|entry| entry.app.clone()));
    for app in &apps {
        if guard.ungrouped.followed.contains_key(app) {
            continue;
        }
        let own = guard.settings.app(app);
        let (this, queued) = (guard.this.clone(), guard.ungrouped.queued.clone());
        own.connect_changed(Some(GROUP), move |_, _| queue(&this, &queued));
        guard.ungrouped.followed.insert(app.clone(), own);
    }
    let ungrouped: Vec<String> = apps
        .into_iter()
        .filter(|app| !guard.settings.effective(app).group)
        .collect();
    if ungrouped == guard.ungrouped.apps {
        return;
    }
    guard.ungrouped.apps = ungrouped;
    emit_on(
        &guard.connection,
        PRIVATE_PATH,
        PRIVATE_INTERFACE,
        "UngroupedChanged",
        (guard.ungrouped.apps.clone(),),
    );
}
