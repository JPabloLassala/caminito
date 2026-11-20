//! A window's fifo and commit-timing barriers, and how often they're released (CMN-316).
//! A window that's drawn has them released at each frame of its main output
//! (`State::post_render`), and one that only the Overview's bar shows at the bar's rate
//! (244). One that isn't drawn at all has them released at the frame-callback throttle's
//! rate, so a game on a workspace the user has left doesn't draw at the output's rate for
//! nothing. Once it shows again, its next frame releases them.

use std::collections::HashMap;
use std::time::Duration;

use smithay::desktop::Window;
use smithay::reexports::wayland_server::backend::ClientId;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, Resource};
use smithay::utils::{Monotonic, Time};
use smithay::wayland::commit_timing::CommitTimerBarrierStateUserData;
use smithay::wayland::compositor::SurfaceData;
use smithay::wayland::fifo::FifoBarrierCachedState;

use caminito_policy::{PolicyWindow, WindowId};

use crate::state::{Caminito, FRAME_CALLBACK_THROTTLE, State};

/// How often a window's barriers are released.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BarrierRate {
    /// At each frame of its main output: it's drawn.
    Frame,
    /// At the Overview bar's rate: only the bar shows it.
    Bar,
    /// At the frame-callback throttle's rate: nothing draws it.
    Throttled,
}

/// The clients a release of barriers let commits of through, each one time.
#[derive(Default)]
pub(crate) struct LetThrough(HashMap<ClientId, Client>);

impl LetThrough {
    /// Releases `surface`'s fifo barrier and its commit-timing barriers due at `now`.
    pub(crate) fn release(
        &mut self,
        surface: &WlSurface,
        states: &SurfaceData,
        now: Time<Monotonic>,
    ) {
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
        if signalled && let Some(client) = surface.client() {
            self.0.insert(client.id(), client);
        }
    }

    /// Releases the barriers of `window`'s surfaces, its popups' too.
    pub(crate) fn release_window(&mut self, window: &Window, now: Time<Monotonic>) {
        window.with_surfaces(|surface, states| self.release(surface, states, now));
    }
}

/// When a surface's barriers are next worth releasing, at the throttle's beat `due`: then
/// for a fifo barrier, which a commit may wait on, and for a commit-timing barrier, at
/// the later of `due` and its time, on Caminito's clock, which runs `ahead` of the
/// presentation clock's.
fn next_release(states: &SurfaceData, due: Duration, ahead: Duration) -> Option<Duration> {
    let fifo = states
        .cached_state
        .get::<FifoBarrierCachedState>()
        .current()
        .barrier
        .is_some();
    let timed = states
        .data_map
        .get::<CommitTimerBarrierStateUserData>()
        .and_then(|timer| timer.lock().unwrap().next_deadline())
        .map(|time| due.max(Duration::from(Time::<Monotonic>::from(time)) + ahead));
    fifo.then_some(due).into_iter().chain(timed).min()
}

impl Caminito {
    /// How often each window's barriers are released.
    pub(crate) fn barrier_rates(&self) -> HashMap<WindowId, BarrierRate> {
        let bar_only = self.overview_bar_only_windows();
        let minimizing: Vec<WindowId> = self
            .minimize_animations
            .boxes(self.now())
            .map(|(id, _)| id)
            .collect();
        let drawn = |id| {
            self.policy.is_showing(id)
                || self.overview_lays_out(id)
                || minimizing.contains(&id)
                || self.slides_out(id)
        };
        let rate = |id| {
            if bar_only.contains(&id) {
                BarrierRate::Bar
            } else if drawn(id) {
                BarrierRate::Frame
            } else {
                BarrierRate::Throttled
            }
        };
        let ids = self.policy.windows().map(|mapped| mapped.id());
        ids.map(|id| (id, rate(id))).collect()
    }

    /// Whether a Workspace slide draws window `id` on its way out (233).
    fn slides_out(&self, id: WindowId) -> bool {
        let now = self.now();
        let output = self.policy.main_output_of(id);
        let slide = output.and_then(|output| self.workspace_slides.on(output, now));
        slide.is_some_and(|slide| !slide.shifts(id, false, now).is_empty())
    }

    /// Whether `surface`'s barriers are released at each frame: it isn't one of a window's
    /// surfaces, or its window is drawn. A commit-timing timestamp lets a commit through at
    /// its time only then.
    pub(crate) fn releases_at_frames(&self, surface: &WlSurface) -> bool {
        let window = self.policy.windows().find(|mapped| {
            let mut its = false;
            mapped
                .window()
                .with_surfaces(|candidate, _| its |= candidate == surface);
            its
        });
        window.is_none_or(|mapped| self.barrier_rates()[&mapped.id()] == BarrierRate::Frame)
    }

    /// The windows whose barriers are released at the throttle's rate.
    fn throttled_windows(&self) -> Vec<WindowId> {
        let rates = self.barrier_rates();
        let throttled = rates
            .into_iter()
            .filter(|(_, rate)| *rate == BarrierRate::Throttled);
        throttled.map(|(id, _)| id).collect()
    }

    /// When the barriers of the windows that aren't drawn are next released, while one of
    /// them has one worth releasing.
    pub(crate) fn throttled_barriers_deadline(&self) -> Option<Duration> {
        let due = self.throttled_barriers_due;
        let ahead = self.now().saturating_sub(self.clock.now().into());
        let mut next: Option<Duration> = None;
        for id in self.throttled_windows() {
            let mapped = self.policy.window(id).unwrap();
            mapped.window().with_surfaces(|_, states| {
                let release = next_release(states, due, ahead);
                next = next.into_iter().chain(release).min();
            });
        }
        next
    }
}

impl State {
    /// Releases the barriers of the windows that aren't drawn, when they're due at `now`:
    /// at the frame-callback throttle's rate, in place of each frame of their output.
    pub(crate) fn throttled_barriers_elapse(&mut self, now: Duration) {
        if now < self.caminito.throttled_barriers_due {
            return;
        }
        let throttle = FRAME_CALLBACK_THROTTLE.expect("the throttle has a period");
        self.caminito.throttled_barriers_due = now + throttle;
        // What a client is told, as after a frame.
        let time = self.caminito.clock.now();
        let mut let_through = LetThrough::default();
        for id in self.caminito.throttled_windows() {
            let mapped = self.caminito.policy.window(id).unwrap();
            let_through.release_window(mapped.window(), time);
        }
        self.let_through(let_through);
    }

    /// Lets through the commits that a release of barriers no longer holds back.
    pub(crate) fn let_through(&mut self, let_through: LetThrough) {
        for client in let_through.0.into_values() {
            self.blocker_cleared(&client);
        }
    }
}
