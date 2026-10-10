//! Policy's loop timer (153). Policy is free of time but for the `now` its calls take, so
//! it keeps its own deadlines and says when it next needs to run, and this wakes it then
//! with one loop timer, as `idle` does for its state (128). Auto-raise's delay is the one
//! deadline so far.

use std::time::Duration;

use smithay::reexports::calloop::RegistrationToken;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};

use crate::state::{Caminito, State};

/// The loop timer for policy's deadline, while it has one.
#[derive(Default)]
pub(crate) struct PolicyTimer(Option<Wake>);

struct Wake {
    due: Duration,
    timer: RegistrationToken,
}

impl State {
    /// Runs what policy has due, as the loop timer does when it fires, and as a test does
    /// once it has moved Caminito's clock on.
    pub(crate) fn policy_elapse(&mut self) {
        let now = self.caminito.now();
        self.caminito.policy.elapse(now);
    }
}

impl Caminito {
    /// Sets the loop timer for policy's deadline, from the refresh pass. A timer due before
    /// the deadline is kept: it finds nothing to do, and the refresh pass after it sets the
    /// next.
    pub(crate) fn wake_policy(&mut self) {
        let deadline = self.policy.deadline();
        if let Some(wake) = &self.policy_timer.0
            && deadline.is_some_and(|deadline| wake.due <= deadline)
        {
            return;
        }
        if let Some(wake) = self.policy_timer.0.take() {
            self.loop_handle.remove(wake.timer);
        }
        let Some(due) = deadline else {
            return;
        };
        let timer = Timer::from_duration(due.saturating_sub(self.now()));
        let timer = self.loop_handle.insert_source(timer, |_, _, state| {
            state.caminito.policy_timer.0 = None;
            state.policy_elapse();
            TimeoutAction::Drop
        });
        match timer {
            Ok(timer) => self.policy_timer.0 = Some(Wake { due, timer }),
            Err(err) => tracing::warn!("can't set policy's timer: {}", err.error),
        }
    }
}
