//! Do Not Disturb in the server (CMN-263, CMN-264): the user's, kept in its file, with the
//! schedule and the triggers of [`crate::do_not_disturb`]. The settings are followed as they
//! change, and so are Caminito's `PointerOutputFullscreen` and `Screencasting` on
//! `org.mate.Caminito` (ADR 0051), which are false while Caminito isn't on the bus. A timer
//! wakes the server at the next time the clock alone changes it. Whatever changes it, the
//! clients hear `DoNotDisturbChanged` once it's another.

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::time::Duration;

use gio::glib;
use gio::prelude::*;

use super::{CAMINITO, CAMINITO_PATH, State, emit_on};
use crate::DOMAIN;
use crate::do_not_disturb::{DoNotDisturb, Schedule, Sources, Triggers};
use crate::private_api::{INTERFACE_NAME as PRIVATE_INTERFACE, PATH as PRIVATE_PATH};
use crate::settings::{
    DO_NOT_DISTURB_FROM, DO_NOT_DISTURB_SCHEDULE, DO_NOT_DISTURB_TO,
    DO_NOT_DISTURB_WHILE_FULLSCREEN, DO_NOT_DISTURB_WHILE_SCREENCASTING,
};

/// Caminito's properties the triggers follow.
const FULLSCREEN: &str = "PointerOutputFullscreen";
const SCREENCASTING: &str = "Screencasting";

/// The settings Do Not Disturb follows, besides the apps' and the critical ones'.
const KEYS: [&str; 5] = [
    DO_NOT_DISTURB_SCHEDULE,
    DO_NOT_DISTURB_FROM,
    DO_NOT_DISTURB_TO,
    DO_NOT_DISTURB_WHILE_FULLSCREEN,
    DO_NOT_DISTURB_WHILE_SCREENCASTING,
];

/// What follows the settings and Caminito, and the timer.
#[derive(Default)]
pub(super) struct Following {
    /// The global settings' handler.
    settings: Option<glib::SignalHandlerId>,
    /// Caminito's proxy, once it's made, and what stops its making before then.
    caminito: Option<gio::DBusProxy>,
    making: Option<gio::Cancellable>,
    /// Waits for the next time the clock alone changes Do Not Disturb.
    wake: Option<glib::JoinHandle<()>>,
}

/// Starts following the settings and Caminito, and tells the clients if it's on already.
pub(super) fn follow(state: &Rc<RefCell<State>>) {
    let mut guard = state.borrow_mut();
    let this = guard.this.clone();
    let handler = guard
        .settings
        .global()
        .connect_changed(None, move |_, key| {
            if KEYS.contains(&key) {
                queue_refresh(&this);
            }
        });
    guard.following.settings = Some(handler);

    let making = gio::Cancellable::new();
    guard.following.making = Some(making.clone());
    let this = guard.this.clone();
    gio::DBusProxy::new(
        &guard.connection,
        gio::DBusProxyFlags::DO_NOT_AUTO_START,
        None,
        Some(CAMINITO),
        CAMINITO_PATH,
        CAMINITO,
        Some(&making),
        move |made| match made {
            Ok(proxy) => watch(&this, proxy),
            Err(err) if err.matches(gio::IOErrorEnum::Cancelled) => {}
            Err(err) => glib::g_warning!(DOMAIN, "can't follow Caminito's properties: {err}"),
        },
    );
    guard.refresh_do_not_disturb();
}

/// Stops following, as the server stops.
pub(super) fn unfollow(state: &mut State) {
    if let Some(handler) = state.following.settings.take() {
        state.settings.global().disconnect(handler);
    }
    if let Some(making) = state.following.making.take() {
        making.cancel();
    }
    // The proxy's handlers hold the state weakly, so they do nothing once it's gone.
    state.following.caminito = None;
    if let Some(wake) = state.following.wake.take() {
        wake.abort();
    }
}

/// Follows `proxy`'s properties, which gio loads as Caminito comes on the bus and forgets
/// as it leaves.
fn watch(this: &Weak<RefCell<State>>, proxy: gio::DBusProxy) {
    let Some(state) = this.upgrade() else {
        return;
    };
    let changed = this.clone();
    proxy.connect_local("g-properties-changed", false, move |_| {
        queue_refresh(&changed);
        None
    });
    let changed = this.clone();
    proxy.connect_notify_local(Some("g-name-owner"), move |_, _| queue_refresh(&changed));
    let mut guard = state.borrow_mut();
    guard.following.making = None;
    guard.following.caminito = Some(proxy);
    guard.refresh_do_not_disturb();
}

/// Refreshes Do Not Disturb once the change that called it is over: a handler may run while
/// the state is borrowed.
fn queue_refresh(this: &Weak<RefCell<State>>) {
    let this = this.clone();
    glib::MainContext::ref_thread_default().spawn_local(async move {
        if let Some(state) = this.upgrade() {
            state.borrow_mut().refresh_do_not_disturb();
        }
    });
}

impl State {
    /// What the settings turn Do Not Disturb on for.
    fn triggers(&self) -> Triggers {
        let settings = self.settings.global();
        let schedule = settings.boolean(DO_NOT_DISTURB_SCHEDULE).then(|| Schedule {
            from: settings.uint(DO_NOT_DISTURB_FROM),
            to: settings.uint(DO_NOT_DISTURB_TO),
        });
        Triggers {
            schedule,
            while_fullscreen: settings.boolean(DO_NOT_DISTURB_WHILE_FULLSCREEN),
            while_screencasting: settings.boolean(DO_NOT_DISTURB_WHILE_SCREENCASTING),
        }
    }

    /// Caminito's property `name`, false while it isn't on the bus.
    fn caminito_says(&self, name: &str) -> bool {
        let proxy = self.following.caminito.as_ref();
        proxy
            .and_then(|proxy| proxy.cached_property(name))
            .and_then(|value| value.get::<bool>())
            .unwrap_or(false)
    }

    /// Brings Do Not Disturb up to date with the clock, the settings and Caminito.
    pub(super) fn refresh_do_not_disturb(&mut self) {
        self.change_do_not_disturb(|_, _, _, _| {});
    }

    /// The user sets their Do Not Disturb to `to`. Turning it off holds off what else has it
    /// on.
    pub(super) fn set_do_not_disturb(&mut self, to: DoNotDisturb) {
        self.change_do_not_disturb(|sources, triggers, now, zone| match to {
            DoNotDisturb::Off => sources.turn_off(triggers, now, zone),
            DoNotDisturb::On { .. } => sources.manual = to,
        });
    }

    /// Applies `change` to the sources, up to date with Caminito and with what the clock
    /// ended. If the user's changed, its file is written, and if what the clients see
    /// changed, they hear of it. The timer is set again either way.
    fn change_do_not_disturb(
        &mut self,
        change: impl FnOnce(&mut Sources, &Triggers, u64, &glib::TimeZone),
    ) {
        let (now, zone, triggers) = (self.clock.now(), self.clock.zone(), self.triggers());
        let (fullscreen, screencasting) = (
            self.caminito_says(FULLSCREEN),
            self.caminito_says(SCREENCASTING),
        );
        let sources = &mut self.do_not_disturb;
        let manual = sources.manual;
        sources.set_fullscreen(fullscreen);
        sources.set_screencasting(screencasting);
        sources.settle(now);
        change(sources, &triggers, now, &zone);
        if sources.manual != manual {
            sources.manual.save(&self.do_not_disturb_file);
        }
        let effective = sources.effective(&triggers, now, &zone);
        let next = sources.next_change(&triggers, now, &zone);
        if effective != self.do_not_disturb_now {
            self.do_not_disturb_now = effective;
            emit_on(
                &self.connection,
                PRIVATE_PATH,
                PRIVATE_INTERFACE,
                "DoNotDisturbChanged",
                effective.to_wire(),
            );
        }
        self.wake_at(next, now);
    }

    /// Wakes at `next`, at least a second and at most a day after `now`, and refreshes then,
    /// so that a clock that jumped is followed.
    fn wake_at(&mut self, next: Option<u64>, now: u64) {
        if let Some(wake) = self.following.wake.take() {
            wake.abort();
        }
        let Some(next) = next else {
            return;
        };
        let wait = next.saturating_sub(now).clamp(1, 24 * 60 * 60);
        let this = self.this.clone();
        self.following.wake = Some(glib::MainContext::ref_thread_default().spawn_local(
            async move {
                glib::timeout_future(Duration::from_secs(wait)).await;
                if let Some(state) = this.upgrade() {
                    let mut guard = state.borrow_mut();
                    // Its own handle, which the refresh would abort as it runs.
                    guard.following.wake = None;
                    guard.refresh_do_not_disturb();
                }
            },
        ));
    }
}
