//! Idle (06, 09, 128): Caminito owns it. After `idle-delay` without activity the outputs are
//! blanked, if `idle-activation-enabled` is on, and with `lock-enabled` the lock is asked
//! for `lock-delay` after the blank starts. Activity on any input device ends the blank and
//! starts the count again; it never unlocks. ext-idle-notify's clients are told whatever
//! `idle-activation-enabled` says.
//!
//! The timers are `state`'s pure type, which this applies on Caminito's clock, woken at its
//! deadline by `deadlines` (174). Inhibitors hold idle off: an idle-inhibit surface while
//! it's visible, mate-session's `IsInhibited` (`dbus::session_manager`), and the Screensaver
//! shim's (127). None applies while the session is locked, when no client surface is
//! visible (12).
//!
//! The blank is `scene`'s top layer: black, with nothing under it drawn.
//!
//! After mate-power-manager's `sleep-display-ac`, or `sleep-display-battery` while UPower
//! says `OnBattery` (`dbus::upower`), the outputs are powered off, held off by the same
//! inhibitors, and activity powers them on (129). On the tty that's DPMS, through the
//! backend; nested it's the blank and nothing more. No output goes or comes.

mod state;

use std::time::Duration;

use smithay::backend::renderer::element::Id;
use smithay::reexports::wayland_protocols::ext::idle_notify::v1::server::ext_idle_notification_v1::ExtIdleNotificationV1;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::compositor::get_parent;
use smithay::wayland::idle_inhibit::IdleInhibitHandler;

use caminito_policy::WindowId;

use self::state::{Effect, IdleState};
use crate::protocols::idle_notify::IdleNotifyHandler;
use crate::settings::translate::IdleSettings;
use crate::state::{Caminito, State};

/// Idle's timers, what holds them off, and the blank.
pub(crate) struct Idle {
    state: IdleState<ExtIdleNotificationV1>,
    /// The surfaces an idle-inhibit client asked to hold idle off while they're visible.
    inhibitors: Vec<WlSurface>,
    /// Whether mate-session last said an inhibitor of its own holds idle off.
    session_inhibited: bool,
    /// The blank's id in every output's scene, so it's damaged once and not at every
    /// frame.
    blank: Id,
}

impl Idle {
    pub(crate) fn new(settings: IdleSettings, now: Duration) -> Self {
        Idle {
            state: IdleState::new(settings, now),
            inhibitors: Vec::new(),
            session_inhibited: false,
            blank: Id::new(),
        }
    }

    /// Whether the outputs are blanked.
    pub(crate) fn is_blanked(&self) -> bool {
        self.state.is_blanked()
    }

    /// Whether the outputs are powered off (129).
    pub(crate) fn is_powered_off(&self) -> bool {
        self.state.is_powered_off()
    }

    /// Whether the scene is the blank alone: blanked, or powered off, which nested is the
    /// blank (129).
    pub(crate) fn shows_blank(&self) -> bool {
        self.is_blanked() || self.is_powered_off()
    }

    /// Whether UPower last said the machine runs on its battery.
    pub(crate) fn on_battery(&self) -> bool {
        self.state.is_on_battery()
    }

    /// Whether inhibitors hold idle off now.
    pub(crate) fn is_inhibited(&self) -> bool {
        self.state.is_inhibited()
    }

    /// The idle-inhibit surfaces that are still there.
    pub(crate) fn inhibitor_surfaces(&self) -> impl Iterator<Item = &WlSurface> {
        self.inhibitors.iter().filter(|surface| surface.is_alive())
    }

    /// Whether mate-session holds idle off.
    pub(crate) fn session_inhibited(&self) -> bool {
        self.session_inhibited
    }

    /// How many ext-idle-notify notifications there are.
    pub(crate) fn notification_count(&self) -> usize {
        self.state.notification_count()
    }

    /// When the timers are next due, if any waits.
    pub(crate) fn deadline(&self) -> Option<Duration> {
        self.state.deadline()
    }

    /// The id of the blank `scene` draws on every output while blanked.
    pub(crate) fn blank_id(&self) -> &Id {
        &self.blank
    }
}

impl State {
    /// Input on any device: the blank ends, and the count starts again (12: it never
    /// unlocks).
    pub(crate) fn idle_activity(&mut self) {
        let now = self.caminito.now();
        let effects = self.caminito.idle.state.activity(now);
        self.idle_effects(effects);
    }

    /// The Screensaver shim's `SetActive(true)` (127): the outputs blank now, and the lock
    /// delay counts from now.
    pub(crate) fn idle_blank_now(&mut self) {
        let now = self.caminito.now();
        let effects = self.caminito.idle.state.blank_now(now);
        self.idle_effects(effects);
    }

    /// New idle settings, which apply at once.
    pub(crate) fn idle_settings_changed(&mut self, settings: IdleSettings) {
        let now = self.caminito.now();
        let effects = self.caminito.idle.state.settings_changed(settings, now);
        self.idle_effects(effects);
    }

    /// UPower said whether the machine runs on its battery, which picks the `sleep-display`
    /// key (129).
    pub(crate) fn idle_on_battery(&mut self, on_battery: bool) {
        if on_battery == self.caminito.idle.on_battery() {
            return;
        }
        tracing::debug!("on battery: {on_battery}");
        let now = self.caminito.now();
        let effects = self.caminito.idle.state.set_on_battery(on_battery, now);
        self.idle_effects(effects);
    }

    /// The refresh pass's part of idle: whether inhibitors hold it off now, as the windows
    /// they're on show and hide, and as the session locks and unlocks.
    pub(crate) fn refresh_idle(&mut self) {
        let caminito = &mut self.caminito;
        caminito.idle.inhibitors.retain(Resource::is_alive);
        let inhibited = !caminito.lock.is_locked()
            && (caminito.idle.session_inhibited
                || caminito.screensaver.is_inhibiting()
                || caminito
                    .idle
                    .inhibitors
                    .iter()
                    .any(|surface| caminito.shows_surface(surface)));
        if inhibited == caminito.idle.is_inhibited() {
            return;
        }
        let now = caminito.now();
        let effects = caminito.idle.state.set_inhibited(inhibited, now);
        self.idle_effects(effects);
    }

    /// Runs the timers that are due at `now`, for `deadlines`.
    pub(crate) fn idle_elapse(&mut self, now: Duration) {
        let effects = self.caminito.idle.state.elapse(now);
        self.idle_effects(effects);
    }

    /// Does what the timers asked.
    fn idle_effects(&mut self, effects: Vec<Effect<ExtIdleNotificationV1>>) {
        for effect in effects {
            match effect {
                Effect::Blank => {
                    tracing::debug!("the session is idle; blanking the outputs");
                    self.caminito.queue_redraw_all();
                }
                Effect::Unblank => {
                    tracing::debug!("activity; unblanking the outputs");
                    self.caminito.queue_redraw_all();
                }
                // A lock that's refused, as with `disable-lock-screen`, is logged there.
                Effect::Lock => {
                    tracing::debug!(target: crate::lock::TARGET, "the blank lasted lock-delay; locking");
                    self.request_lock();
                }
                Effect::PowerOff => {
                    tracing::debug!("the session is idle; powering the outputs off");
                    self.backend.set_outputs_powered(&mut self.caminito, false);
                    self.caminito.queue_redraw_all();
                }
                Effect::PowerOn => {
                    tracing::debug!("activity; powering the outputs on");
                    self.backend.set_outputs_powered(&mut self.caminito, true);
                    self.caminito.queue_redraw_all();
                }
                Effect::Idled(notification) => notification.idled(),
                Effect::Resumed(notification) => notification.resumed(),
            }
        }
    }
}

impl Caminito {
    /// mate-session said whether an inhibitor of its own holds idle off. The refresh pass
    /// applies it.
    pub(crate) fn session_inhibited_changed(&mut self, inhibited: bool) {
        self.idle.session_inhibited = inhibited;
    }

    /// Whether `surface` is on screen as part of a window that shows, or of a layer
    /// surface: an idle inhibitor counts only then (128). A window on another workspace or
    /// minimised doesn't show.
    pub(crate) fn shows_surface(&self, surface: &WlSurface) -> bool {
        let root = root(surface);
        match self.mapped_for_surface(&root) {
            Some(id) => self.policy.is_showing(id),
            None => self.layer_for_surface(&root).is_some(),
        }
    }

    /// The window `surface` is part of, if it's a window's.
    pub(crate) fn window_of_surface(&self, surface: &WlSurface) -> Option<WindowId> {
        self.mapped_for_surface(&root(surface))
    }
}

/// The root of `surface`'s tree of subsurfaces.
fn root(surface: &WlSurface) -> WlSurface {
    let mut root = surface.clone();
    while let Some(parent) = get_parent(&root) {
        root = parent;
    }
    root
}

impl IdleInhibitHandler for State {
    fn inhibit(&mut self, surface: WlSurface) {
        self.caminito.idle.inhibitors.push(surface);
    }

    fn uninhibit(&mut self, surface: WlSurface) {
        let inhibitors = &mut self.caminito.idle.inhibitors;
        // A surface may have more than one inhibitor, of which one goes.
        if let Some(index) = inhibitors.iter().position(|held| *held == surface) {
            inhibitors.remove(index);
        }
    }
}

impl IdleNotifyHandler for State {
    fn new_idle_notification(
        &mut self,
        notification: ExtIdleNotificationV1,
        timeout: Duration,
        input_only: bool,
    ) {
        let now = self.caminito.now();
        let idle = &mut self.caminito.idle.state;
        let effects = idle.add_notification(notification, timeout, input_only, now);
        self.idle_effects(effects);
    }

    fn idle_notification_destroyed(&mut self, notification: &ExtIdleNotificationV1) {
        self.caminito.idle.state.remove_notification(notification);
    }
}
