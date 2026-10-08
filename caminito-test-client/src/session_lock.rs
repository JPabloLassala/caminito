//! An `ext_session_lock_v1` client, as a locker is (12): it asks for the lock, puts a lock
//! surface on each output, and unlocks.

use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::ext::session_lock::v1::client::ext_session_lock_manager_v1::{
    self, ExtSessionLockManagerV1,
};
use wayland_protocols::ext::session_lock::v1::client::ext_session_lock_surface_v1::{
    self, ExtSessionLockSurfaceV1,
};
use wayland_protocols::ext::session_lock::v1::client::ext_session_lock_v1::{
    self, ExtSessionLockV1,
};

use crate::{Buffer, Client, State};

/// The index of one of a client's session locks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LockId(usize);

/// The index of one of a client's lock surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LockSurfaceId(pub(crate) usize);

/// A session lock the client asked for.
pub struct SessionLock {
    lock: ExtSessionLockV1,
    /// Whether the compositor said the session is locked, with this lock.
    pub locked: bool,
    /// Whether the compositor refused the lock, or ended it.
    pub finished: bool,
}

/// A lock surface.
pub struct LockSurface {
    /// The `wl_surface` it's the role of.
    pub surface: WlSurface,
    lock_surface: ExtSessionLockSurfaceV1,
    /// The size of every configure received, oldest first.
    pub configures: Vec<(u32, u32)>,
    /// The serial of the last configure, which mapping acks.
    last_serial: Option<u32>,
    buffers: Vec<Buffer>,
}

impl Client {
    /// Asks for the session lock. The answer, `locked` or `finished`, comes with a
    /// roundtrip.
    pub fn lock_session(&mut self) -> LockId {
        let id = LockId(self.state.session_locks.len());
        let manager: ExtSessionLockManagerV1 = self.global(|state| &mut state.session_lock, 1);
        let lock = manager.lock(&self.qh, id);
        self.state.session_locks.push(SessionLock {
            lock,
            locked: false,
            finished: false,
        });
        id
    }

    /// A session lock the client asked for, and its answer.
    pub fn session_lock(&self, id: LockId) -> &SessionLock {
        &self.state.session_locks[id.0]
    }

    /// Creates a lock surface of `lock` for the `output`th bound output. Its configure
    /// comes with a roundtrip.
    pub fn create_lock_surface(&mut self, lock: LockId, output: usize) -> LockSurfaceId {
        let id = LockSurfaceId(self.state.lock_surfaces.len());
        let surface = self.compositor().create_surface(&self.qh, ());
        let output = &self.state.outputs[output].proxy;
        let lock = &self.state.session_locks[lock.0].lock;
        let lock_surface = lock.get_lock_surface(&surface, output, &self.qh, id);
        self.state.lock_surfaces.push(LockSurface {
            surface,
            lock_surface,
            configures: Vec::new(),
            last_serial: None,
            buffers: Vec::new(),
        });
        id
    }

    /// A lock surface, and its configures.
    pub fn lock_surface(&self, id: LockSurfaceId) -> &LockSurface {
        &self.state.lock_surfaces[id.0]
    }

    /// Acks the lock surface's latest configure and commits a buffer of the configured size
    /// filled with `argb`, which shows it.
    pub fn map_lock_surface(&mut self, id: LockSurfaceId, argb: u32) {
        let lock_surface = self.lock_surface(id);
        let size = *lock_surface.configures.last().expect("configured");
        let serial = lock_surface.last_serial.expect("configured");
        lock_surface.lock_surface.ack_configure(serial);
        let size = (size.0 as i32, size.1 as i32);
        let (pool, buffer) = self.shm_buffer(size, argb);
        let lock_surface = &mut self.state.lock_surfaces[id.0];
        lock_surface.surface.attach(Some(&buffer), 0, 0);
        lock_surface.surface.damage_buffer(0, 0, size.0, size.1);
        lock_surface.surface.commit();
        lock_surface.buffers.push(Buffer { buffer, pool });
    }

    /// Unlocks the session, as a locker does once the password is right.
    pub fn unlock_and_destroy(&self, id: LockId) {
        self.session_lock(id).lock.unlock_and_destroy();
    }
}

impl Dispatch<ExtSessionLockV1, LockId> for State {
    fn event(
        state: &mut Self,
        _: &ExtSessionLockV1,
        event: ext_session_lock_v1::Event,
        id: &LockId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let lock = &mut state.session_locks[id.0];
        match event {
            ext_session_lock_v1::Event::Locked => lock.locked = true,
            ext_session_lock_v1::Event::Finished => lock.finished = true,
            _ => {}
        }
    }
}

impl Dispatch<ExtSessionLockSurfaceV1, LockSurfaceId> for State {
    fn event(
        state: &mut Self,
        _: &ExtSessionLockSurfaceV1,
        event: ext_session_lock_surface_v1::Event,
        id: &LockSurfaceId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_session_lock_surface_v1::Event::Configure {
            serial,
            width,
            height,
        } = event
        {
            let lock_surface = &mut state.lock_surfaces[id.0];
            lock_surface.configures.push((width, height));
            lock_surface.last_serial = Some(serial);
        }
    }
}

impl Dispatch<ExtSessionLockManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ExtSessionLockManagerV1,
        _: ext_session_lock_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
