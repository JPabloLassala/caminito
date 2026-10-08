//! The session lock (12, ADR 0007): Caminito locks the session itself, and a locker only
//! draws on it. The state machine is `state`'s pure type; this applies it. While the
//! session is locked, `scene` draws nothing under the lock, the pointer and the keyboard
//! reach only the locker's lock surfaces, and the keyboard path runs no binding but VT
//! switching. ext-session-lock is how a locker takes the lock, draws on it and unlocks.

mod state;

pub(crate) use state::LockState;

use smithay::backend::input::InputTime;
use smithay::backend::renderer::element::Id;
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
    /// The process of the locker Caminito spawned, which alone takes over a lock that no
    /// locker holds. Nothing spawns one until 126.
    pub(crate) spawned_locker: Option<u32>,
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
            spawned_locker: None,
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

    /// The id of the background `scene` draws on every locked output.
    pub(crate) fn background(&self) -> &Id {
        &self.background
    }
}

impl SessionLockHandler for State {
    fn lock_state(&mut self) -> &mut SessionLockManagerState {
        &mut self.caminito.session_lock_state
    }

    /// A client's `lock` is a lock request like any other (12): the session is locked
    /// before the client hears `locked`, and no frame drawn from here on shows anything
    /// under the lock. A refused client hears `finished`, as Smithay tells it when the
    /// confirmation is dropped.
    fn lock(&mut self, confirmation: SessionLocker) {
        // The locker that held the lock may have gone in this same dispatch.
        self.caminito.notice_locker_gone();
        let held = confirmation.ext_session_lock().clone();
        let lock = &self.caminito.lock;
        let was_locked = lock.is_locked();
        match lock.state.client_locks(self.caminito.asker(&held)) {
            Ok(state) => {
                let lock = &mut self.caminito.lock;
                lock.state = state;
                lock.held = Some(held);
                lock.surfaces.clear();
                if !was_locked {
                    self.session_locked();
                }
                confirmation.lock();
            }
            Err(refused) => tracing::warn!("can't give a client the session lock: {refused}"),
        }
    }

    /// `unlock_and_destroy` from the lock that's held, which Smithay has checked. The
    /// refresh pass gives the keyboard and the pointer back to what policy focuses and
    /// what's under the pointer.
    fn unlock(&mut self) {
        let lock = &mut self.caminito.lock;
        lock.state = lock.state.unlocked();
        lock.held = None;
        lock.surfaces.clear();
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
    /// go. The refresh pass then gives the keyboard and the pointer to the lock.
    fn session_locked(&mut self) {
        let time = InputTime::from_micros(self.caminito.now().as_micros() as u64);
        // Menus, with their grabs of the keyboard and the pointer, and the keyboard grab an
        // override-redirect window holds through Xwayland (11).
        self.dismiss_menus(time);
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
        caminito.stop_accelerator_repeat();
        caminito.keyboard_layer = None;
        caminito.queue_redraw_all();
    }
}

impl Caminito {
    /// Who the client that asks for `lock` is: a locker Caminito spawned is known by its
    /// process (12).
    fn asker(&self, lock: &ExtSessionLockV1) -> Asker {
        let process = lock
            .client()
            .and_then(|client| client_process(&client, &self.display_handle));
        match self.lock.spawned_locker {
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
            lock.state = lock.state.locker_gone();
            lock.held = None;
            lock.surfaces.clear();
            self.queue_redraw_all();
        }
    }

    /// The refresh pass's part of the lock: notices a locker that's gone, and has each
    /// lock surface that's left follow its output's size and scale.
    pub(crate) fn refresh_lock(&mut self) {
        self.notice_locker_gone();
        let outputs = &self.outputs;
        let surfaces = &mut self.lock.surfaces;
        surfaces.retain(|(output, surface)| surface.alive() && outputs.contains(output));
        for (output, surface) in surfaces.iter() {
            configure_lock_surface(surface, output);
        }
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
