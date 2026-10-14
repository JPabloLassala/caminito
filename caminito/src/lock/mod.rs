//! The session lock (12, ADR 0007): Caminito locks the session itself, and a locker only
//! draws on it. The state machine is `state`'s pure type; this applies it. While the
//! session is locked, `scene` draws nothing under the lock, the pointer and the keyboard
//! reach only the locker's lock surfaces, and the keyboard path runs no binding but VT
//! switching. ext-session-lock is how a locker takes the lock, draws on it and unlocks.
//! `locker` spawns the locker for Caminito's own lock requests.

mod locker;
mod state;

pub(crate) use locker::Locker;
pub(crate) use state::LockState;

use smithay::backend::input::InputTime;
use smithay::backend::renderer::element::Id;
use smithay::backend::renderer::utils::with_renderer_surface_state;
use smithay::desktop::WindowSurfaceType;
use smithay::desktop::utils::under_from_surface_tree;
use smithay::output::Output;
use smithay::reexports::wayland_protocols::ext::session_lock::v1::server::ext_session_lock_v1::ExtSessionLockV1;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, SERIAL_COUNTER};
use smithay::wayland::compositor::with_states;
use smithay::wayland::session_lock::{
    LockSurface, SessionLockHandler, SessionLockManagerState, SessionLocker,
};

use self::state::Asker;
use crate::input::{PointerFocus, Under};
use crate::output::output_geometry;
use crate::scene;
use crate::state::{Caminito, State, client_process};
use crate::window::send_preferred_scale;

/// The lock, and what its locker shows on it.
pub(crate) struct Lock {
    state: LockState,
    /// The lock the locker holds, while one does.
    held: Option<ExtSessionLockV1>,
    /// The lock surfaces of the lock that's held, each with its output.
    surfaces: Vec<(Output, LockSurface)>,
    /// What tells the locker that holds the lock `locked`, until every output has drawn
    /// the lock, and the outputs that haven't yet.
    confirmation: Option<SessionLocker>,
    undrawn: Vec<Output>,
    /// The outputs that have drawn a frame with the locker's lock surface on them, which
    /// logind's sleep delay waits for (130).
    shown: Vec<Output>,
    /// The background's id in every output's scene, so it's damaged once and not at
    /// every frame.
    background: Id,
}

impl Default for Lock {
    fn default() -> Self {
        Lock {
            state: LockState::default(),
            held: None,
            surfaces: Vec::new(),
            confirmation: None,
            undrawn: Vec::new(),
            shown: Vec::new(),
            background: Id::new(),
        }
    }
}

impl Lock {
    pub(crate) fn state(&self) -> LockState {
        self.state
    }

    pub(crate) fn is_locked(&self) -> bool {
        self.state.is_locked()
    }

    /// The lock surface the locker put on `output`, if it has one.
    pub(crate) fn surface_on(&self, output: &Output) -> Option<&LockSurface> {
        let mut surfaces = self.surfaces.iter();
        surfaces.find_map(|(on, surface)| (on == output).then_some(surface))
    }

    /// The lock surfaces, each with its output.
    pub(crate) fn surfaces(&self) -> impl Iterator<Item = (&Output, &LockSurface)> {
        self.surfaces
            .iter()
            .map(|(output, surface)| (output, surface))
    }

    /// A locker takes the lock, in `state`. It hears `locked` once each of `outputs` has
    /// drawn a frame of the lock, as ext-session-lock has it: by then nothing of the
    /// session is on screen.
    fn take(&mut self, state: LockState, confirmation: SessionLocker, outputs: &[Output]) {
        self.state = state;
        self.held = Some(confirmation.ext_session_lock().clone());
        self.surfaces.clear();
        self.confirmation = Some(confirmation);
        self.undrawn = outputs.to_vec();
        self.shown.clear();
    }

    /// Caminito asks for the lock itself (126).
    fn request(&mut self) {
        self.state = self.state.requested();
    }

    /// The locker that holds the lock unlocked.
    fn release(&mut self) {
        self.state = self.state.unlocked();
        self.forget_locker();
    }

    /// logind's `Unlock` ends the lock (130). A locker that holds it hears `finished`, which
    /// ext-session-lock allows for a lock the compositor unlocked some other way.
    fn end(&mut self) {
        self.state = self.state.ended();
        // A locker not told `locked` yet hears `finished` as its confirmation is dropped.
        if self.confirmation.is_none()
            && let Some(held) = &self.held
        {
            held.finished();
        }
        self.forget_locker();
    }

    /// The locker that holds the lock went away without unlocking.
    fn lose_locker(&mut self) {
        self.state = self.state.locker_gone();
        self.forget_locker();
    }

    /// Drops what the locker had on the lock. One that wasn't told `locked` yet hears
    /// `finished`, if it's there to hear it.
    fn forget_locker(&mut self) {
        self.held = None;
        self.surfaces.clear();
        self.confirmation = None;
        self.undrawn.clear();
        self.shown.clear();
    }

    /// `output` drew a frame, which shows the lock if the session is locked.
    pub(crate) fn frame_drawn(&mut self, output: &Output) {
        self.undrawn.retain(|undrawn| undrawn != output);
        self.confirm_if_drawn();
        let drawn = self.surface_on(output).is_some_and(|surface| {
            with_renderer_surface_state(surface.wl_surface(), |state| state.buffer().is_some())
                .unwrap_or(false)
        });
        if drawn && !self.shown.contains(output) {
            self.shown.push(output.clone());
        }
    }

    /// Whether a locker holds the lock and each of `outputs` has drawn its lock surface:
    /// what's on screen is the locker's, not the background alone (130).
    pub(crate) fn is_shown_on(&self, outputs: &[Output]) -> bool {
        self.state == LockState::Locked && outputs.iter().all(|output| self.shown.contains(output))
    }

    /// Tells the locker `locked` if no output is left to draw the lock.
    fn confirm_if_drawn(&mut self) {
        if self.undrawn.is_empty()
            && let Some(confirmation) = self.confirmation.take()
        {
            confirmation.lock();
        }
    }

    /// The id of the background `scene` draws on every locked output.
    pub(crate) fn background(&self) -> &Id {
        &self.background
    }
}

impl SessionLockHandler for State {
    fn lock_state(&mut self) -> &mut SessionLockManagerState {
        &mut self.caminito.session_lock_state
    }

    /// A client's `lock` is a lock request like any other (12): the session is locked at
    /// once, no frame drawn from here on shows anything under the lock, and the client
    /// hears `locked` once every output has drawn one. A refused client hears `finished`,
    /// as Smithay tells it when the confirmation is dropped.
    fn lock(&mut self, confirmation: SessionLocker) {
        // The locker that held the lock may have gone in this same dispatch.
        self.caminito.notice_locker_gone();
        let caminito = &mut self.caminito;
        let asker = caminito.asker(confirmation.ext_session_lock());
        let was_locked = caminito.lock.is_locked();
        match caminito.lock.state.client_locks(asker) {
            Ok(state) => {
                caminito.lock.take(state, confirmation, &caminito.outputs);
                caminito.queue_redraw_all();
                if !was_locked {
                    self.session_locked();
                }
            }
            Err(refused) => tracing::warn!("can't give a client the session lock: {refused}"),
        }
    }

    /// `unlock_and_destroy` from the lock that's held, which Smithay has checked. The
    /// refresh pass gives the keyboard and the pointer back to what policy focuses and
    /// what's under the pointer.
    fn unlock(&mut self) {
        // Smithay takes the locker of a lock that logind ended (130) for the one that holds
        // the lock until another is told `locked`. Its unlock mustn't end a later lock.
        let lock = &self.caminito.lock;
        if lock.state() != LockState::Locked || lock.confirmation.is_some() {
            return;
        }
        self.caminito.lock.release();
        self.caminito.locker.finish();
        self.caminito.queue_redraw_all();
    }

    fn new_surface(&mut self, surface: LockSurface, output: WlOutput) {
        let caminito = &mut self.caminito;
        // Smithay doesn't call this for a lock that was refused, but it may for one that
        // unlocked.
        if caminito.lock.held.as_ref() != Some(surface.ext_session_lock()) {
            return;
        }
        // Left unconfigured, a surface for an output that's gone never shows.
        let Some(output) =
            Output::from_resource(&output).filter(|output| caminito.outputs.contains(output))
        else {
            return;
        };
        configure_lock_surface(&surface, &output);
        caminito.lock.surfaces.push((output, surface));
    }
}

/// Has a lock surface cover its output, at the output's scale. Smithay sends the configure
/// if the size is new to the surface.
fn configure_lock_surface(surface: &LockSurface, output: &Output) {
    let size = output_geometry(output).size;
    surface.with_pending_state(|state| {
        state.size = Some((size.w as u32, size.h as u32).into());
    });
    with_states(surface.wl_surface(), |data| {
        send_preferred_scale(surface.wl_surface(), data, output);
    });
    surface.send_configure();
}

impl State {
    /// The session went from unlocked to locked: whatever a client held of the seat is let
    /// go, and the keyboard and the pointer leave the session here, not in the refresh
    /// pass, before which the loop may handle a key that's waiting.
    fn session_locked(&mut self) {
        let time = InputTime::from_micros(self.caminito.now().as_micros() as u64);
        // Menus, with their grabs of the keyboard and the pointer, and the keyboard grab an
        // override-redirect window holds through Xwayland (11).
        self.dismiss_menus(time);
        // Any other grab of the keyboard, as an input method's.
        let keyboard = self.caminito.seat.get_keyboard().unwrap();
        if keyboard.is_grabbed() {
            keyboard.unset_grab(self);
        }
        // A move, a resize, a press on a frame or a drag.
        let pointer = self.caminito.seat.get_pointer().unwrap();
        if pointer.is_grabbed() {
            pointer.unset_grab(self, SERIAL_COUNTER.next_serial(), time);
        }
        let caminito = &mut self.caminito;
        if caminito.policy.switcher().is_some() {
            let now = caminito.now();
            caminito.policy.switcher_cancel(now);
            caminito.refresh_switcher();
        }
        caminito.policy.close_window_menu();
        caminito.refresh_window_menu();
        caminito.close_workspace_popup();
        caminito.policy.close_overview();
        caminito.refresh_overview();
        caminito.stop_accelerator_repeat();
        caminito.keyboard_layer = None;
        self.update_keyboard_focus();
        self.update_pointer_focus();
    }
}

impl Caminito {
    /// Who the client that asks for `lock` is: a locker Caminito spawned is known by its
    /// process (12).
    fn asker(&self, lock: &ExtSessionLockV1) -> Asker {
        let process = lock
            .client()
            .and_then(|client| client_process(&client, &self.display_handle));
        match self.locker.process() {
            Some(spawned) if process == Some(spawned) => Asker::SpawnedLocker,
            _ => Asker::Other,
        }
    }

    /// A locker that went away without `unlock_and_destroy` leaves the session locked,
    /// with nothing of its own on the lock.
    fn notice_locker_gone(&mut self) {
        let lock = &mut self.lock;
        if lock.held.as_ref().is_some_and(|held| !held.is_alive()) {
            tracing::warn!("the locker went away without unlocking; the session stays locked");
            lock.lose_locker();
            self.queue_redraw_all();
        }
    }

    /// The refresh pass's part of the lock: notices a locker that's gone, and has each
    /// lock surface that's left follow its output's size and scale. An output that went
    /// away isn't waited for to draw the lock.
    pub(crate) fn refresh_lock(&mut self) {
        self.notice_locker_gone();
        self.refresh_locker_message();
        let outputs = &self.outputs;
        self.lock.undrawn.retain(|output| outputs.contains(output));
        self.lock.confirm_if_drawn();
        let surfaces = &mut self.lock.surfaces;
        surfaces.retain(|(output, surface)| surface.alive() && outputs.contains(output));
        let shown = &mut self.lock.shown;
        shown.retain(|output| surfaces.iter().any(|(on, _)| on == output));
        for (output, surface) in surfaces.iter() {
            configure_lock_surface(surface, output);
        }
    }

    /// logind's `Unlock` (130): the lock ends whatever holds it, and the locker Caminito
    /// keeps for it is stopped, from the give-up state too (126). The refresh pass gives the
    /// keyboard and the pointer back, as at the locker's unlock.
    pub(crate) fn end_lock(&mut self) {
        if !self.lock.is_locked() {
            return;
        }
        tracing::debug!("logind unlocked the session");
        self.lock.end();
        self.locker.end();
        self.queue_redraw_all();
    }

    /// Queues a redraw of the output `surface` is the lock surface's root of, and returns
    /// whether it's one.
    pub(crate) fn lock_surface_commit(&mut self, surface: &WlSurface) -> bool {
        let output = self.lock.surfaces().find_map(|(output, lock_surface)| {
            (lock_surface.wl_surface() == surface).then(|| output.clone())
        });
        match output {
            Some(output) => {
                self.queue_redraw(&output);
                true
            }
            None => false,
        }
    }

    /// What of the lock is at `location`: the lock surface of the output there, inside its
    /// input region. Nothing else takes the pointer while the session is locked.
    pub(crate) fn lock_under(&self, location: Point<f64, Logical>) -> Option<Under> {
        let output = self
            .outputs
            .iter()
            .find(|output| output_geometry(output).to_f64().contains(location))?;
        let surface = self.lock.surface_on(output)?;
        let origin = output_geometry(output).loc;
        let (under, loc) = under_from_surface_tree(
            surface.wl_surface(),
            location,
            origin,
            WindowSurfaceType::ALL,
        )?;
        Some(Under::Lock(PointerFocus::Surface(under), loc.to_f64()))
    }

    /// The lock surface that has the keyboard while the session is locked: the one on the
    /// pointer's output, where a locker with a surface on every output shows its dialog
    /// (12), or else any of them.
    pub(crate) fn lock_surface_with_keyboard(&self) -> Option<WlSurface> {
        let lock = &self.lock;
        let pointed = self
            .outputs
            .iter()
            .find(|output| scene::pointer_on(self, output).is_some())
            .and_then(|output| lock.surface_on(output));
        pointed
            .or(lock.surfaces().map(|(_, surface)| surface).next())
            .map(|surface| surface.wl_surface().clone())
    }
}
