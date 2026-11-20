//! A window's fifo and commit-timing barriers, and how often they're released (CMN-316).
//! A window that shows has them released at each frame of its main output
//! (`State::post_render`), and one that only the Overview's bar shows at the bar's rate
//! (244). One that isn't showing has them released at the frame-callback throttle's rate,
//! so a game on a workspace the user has left doesn't draw at the output's rate for
//! nothing. Once it shows again, its next frame releases them.

use std::time::Duration;

use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, Resource};
use smithay::utils::{Monotonic, Time};
use smithay::wayland::commit_timing::CommitTimerBarrierStateUserData;
use smithay::wayland::compositor::{SurfaceData, get_parent};
use smithay::wayland::fifo::FifoBarrierCachedState;

use caminito_policy::{PolicyWindow, WindowId};

use crate::state::{Caminito, FRAME_CALLBACK_THROTTLE, State};

/// How often a window's barriers are released.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BarrierRate {
    /// At each frame of its main output: it shows.
    Frame,
    /// At the Overview bar's rate: only the bar shows it.
    Bar,
    /// At the frame-callback throttle's rate: nothing shows it.
    Throttled,
}

/// Releases a surface's fifo barrier and its commit-timing barriers due at `now`. Returns
/// whether that lets a commit through.
pub(crate) fn release_barriers(states: &SurfaceData, now: Time<Monotonic>) -> bool {
    let mut signalled = false;
    if let Some(timer) = states.data_map.get::<CommitTimerBarrierStateUserData>() {
        signalled |= timer.lock().unwrap().signal_until(now);
    }
    let fifo = states
        .cached_state
        .get::<FifoBarrierCachedState>()
        .current()
        .barrier
        .take();
    if let Some(fifo) = fifo {
        fifo.signal();
        signalled = true;
    }
    signalled
}

/// Whether a surface has a barrier that a release would signal: a fifo barrier, which a
/// commit may wait on, or a commit-timing barrier.
fn has_barriers(states: &SurfaceData) -> bool {
    let timed = states
        .data_map
        .get::<CommitTimerBarrierStateUserData>()
        .is_some_and(|timer| timer.lock().unwrap().next_deadline().is_some());
    timed
        || states
            .cached_state
            .get::<FifoBarrierCachedState>()
            .current()
            .barrier
            .is_some()
}

impl Caminito {
    /// How often window `id`'s barriers are released.
    pub(crate) fn barrier_rate(&self, id: WindowId) -> BarrierRate {
        if self.overview_bar_only_windows().contains(&id) {
            BarrierRate::Bar
        } else if self.policy.is_showing(id) || self.overview_lays_out(id) {
            BarrierRate::Frame
        } else {
            BarrierRate::Throttled
        }
    }

    /// Whether `surface`'s barriers are released at each frame: it isn't a window's, or the
    /// window shows. A commit-timing timestamp lets a commit through at its time only then.
    pub(crate) fn releases_at_frames(&self, surface: &WlSurface) -> bool {
        let mut root = surface.clone();
        while let Some(parent) = get_parent(&root) {
            root = parent;
        }
        self.mapped_for_surface(&root)
            .is_none_or(|id| self.barrier_rate(id) == BarrierRate::Frame)
    }

    /// The windows whose barriers are released at the throttle's rate.
    fn throttled_windows(&self) -> Vec<WindowId> {
        self.policy
            .windows()
            .map(|mapped| mapped.id())
            .filter(|&id| self.barrier_rate(id) == BarrierRate::Throttled)
            .collect()
    }

    /// When the barriers of the windows that aren't showing are next released, while one
    /// of them has one.
    pub(crate) fn throttled_barriers_deadline(&self) -> Option<Duration> {
        let waiting = self.throttled_windows().into_iter().any(|id| {
            let mut found = false;
            let mapped = self.policy.window(id).unwrap();
            mapped
                .window()
                .with_surfaces(|_, states| found |= has_barriers(states));
            found
        });
        waiting.then_some(self.throttled_barriers_due)
    }
}

impl State {
    /// Releases the barriers of the windows that aren't showing, when they're due at
    /// `now`: at the frame-callback throttle's rate, in place of each frame of their
    /// output.
    pub(crate) fn throttled_barriers_elapse(&mut self, now: Duration) {
        if now < self.caminito.throttled_barriers_due {
            return;
        }
        let throttle = FRAME_CALLBACK_THROTTLE.expect("the throttle has a period");
        self.caminito.throttled_barriers_due = now + throttle;
        // What a client is told, as after a frame.
        let time = self.caminito.clock.now();
        let mut cleared: Vec<Client> = Vec::new();
        for id in self.caminito.throttled_windows() {
            let mapped = self.caminito.policy.window(id).unwrap();
            mapped.window().with_surfaces(|surface, states| {
                note_cleared(&mut cleared, surface, release_barriers(states, time));
            });
        }
        self.blockers_cleared(cleared);
    }

    /// Lets through the commits of `clients` that nothing holds back any more.
    pub(crate) fn blockers_cleared(&mut self, clients: Vec<Client>) {
        for client in clients {
            self.blocker_cleared(&client);
        }
    }
}

/// Adds `surface`'s client to `cleared`, once, if a release let one of its commits through.
pub(crate) fn note_cleared(cleared: &mut Vec<Client>, surface: &WlSurface, signalled: bool) {
    if !signalled {
        return;
    }
    if let Some(client) = surface.client()
        && !cleared.iter().any(|known| known.id() == client.id())
    {
        cleared.push(client);
    }
}
