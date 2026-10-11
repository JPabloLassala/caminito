//! Caminito's clock, for what waits on it (174): who is due, and when the loop must wake.
//! Each holder keeps its own deadline in a state that's free of the loop, and this runs
//! what's due and keeps one loop timer for the earliest deadline of them all. A timer of a
//! holder's own is for what waits on a process or a protocol object, which isn't this.

use std::time::Duration;

use smithay::reexports::calloop::RegistrationToken;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};

use crate::state::{Caminito, State};

/// Everything that waits on Caminito's clock, in the order `run_due` runs what's due at one
/// time:
///
/// 1. Policy, whose auto-raise restacks the windows the rest looks at (153).
/// 2. Idle, which blanks and locks (128).
/// 3. AccessX, whose keys go on to the clients and whose beeps ring the bell (134).
/// 4. The Global menu's grace period (114), which nothing else here looks at.
/// 5. The Window switcher's popup (108), the bell's flash (169) and the minimise animations
///    (172), which only draw.
const HOLDERS: [Holder; 7] = [
    Holder::Policy,
    Holder::Idle,
    Holder::AccessX,
    Holder::GlobalMenu,
    Holder::SwitcherPopup,
    Holder::Bell,
    Holder::MinimizeAnimations,
];

#[derive(Debug, Clone, Copy)]
enum Holder {
    Policy,
    Idle,
    AccessX,
    GlobalMenu,
    SwitcherPopup,
    Bell,
    MinimizeAnimations,
}

impl Holder {
    /// When the holder is next due, if it waits.
    fn deadline(self, caminito: &Caminito) -> Option<Duration> {
        match self {
            Holder::Policy => caminito.policy.deadline(),
            Holder::Idle => caminito.idle.deadline(),
            Holder::AccessX => caminito.accessx.deadline(),
            Holder::GlobalMenu => caminito.global_menu_switch.deadline(),
            Holder::SwitcherPopup => caminito.switcher_popup_deadline(),
            Holder::Bell => caminito.bells.deadline(),
            Holder::MinimizeAnimations => caminito.minimize_animations.deadline(),
        }
    }

    /// Runs what the holder has due at `now`, which is nothing when it isn't due. Idle's and
    /// AccessX's effects reach the backend and the keyboard path, so this takes the whole
    /// state.
    fn elapse(self, state: &mut State, now: Duration) {
        match self {
            Holder::Policy => state.caminito.policy.elapse(now),
            Holder::Idle => state.idle_elapse(now),
            Holder::AccessX => state.accessx_elapse(now),
            Holder::GlobalMenu => state.caminito.global_menu_time_passed(now),
            Holder::SwitcherPopup => state.caminito.switcher_popup_elapse(now),
            Holder::Bell => state.caminito.bell_elapse(now),
            Holder::MinimizeAnimations => state.caminito.minimize_animation_elapse(now),
        }
    }
}

/// The loop timer for the earliest deadline, while a holder waits.
#[derive(Default)]
pub(crate) struct DeadlineTimer(Option<Set>);

struct Set {
    /// The deadline it's for.
    due: Duration,
    token: RegistrationToken,
}

/// Whether a loop timer due at `set` serves when the earliest deadline is `earliest`. One
/// due at or before the deadline does: it finds nothing to do, and the refresh pass after
/// it sets the next. Input, which only ever moves idle's deadline later, then doesn't set a
/// timer at every event.
fn serves(set: Option<Duration>, earliest: Option<Duration>) -> bool {
    match (set, earliest) {
        (Some(set), Some(earliest)) => set <= earliest,
        (None, None) => true,
        (Some(_), None) | (None, Some(_)) => false,
    }
}

impl State {
    /// Runs every holder that's due, at one reading of Caminito's clock, as the loop timer
    /// does when it fires, and as a test does once it has moved the clock on.
    pub(crate) fn run_due(&mut self) {
        let now = self.caminito.now();
        for holder in HOLDERS {
            holder.elapse(self, now);
        }
    }
}

impl Caminito {
    /// Sets the loop timer for the earliest deadline, from the refresh pass, unless the one
    /// that's set serves.
    pub(crate) fn refresh_deadlines(&mut self) {
        let earliest = HOLDERS
            .into_iter()
            .filter_map(|holder| holder.deadline(self))
            .min();
        if serves(self.deadline_timer.0.as_ref().map(|set| set.due), earliest) {
            return;
        }
        self.drop_deadline_timer();
        let Some(due) = earliest else {
            return;
        };
        let timer = Timer::from_duration(due.saturating_sub(self.now()));
        let timer = self.loop_handle.insert_source(timer, |_, _, state| {
            // This timer is done, and the refresh pass sets the next.
            state.caminito.deadline_timer.0 = None;
            state.run_due();
            TimeoutAction::Drop
        });
        match timer {
            Ok(token) => self.deadline_timer.0 = Some(Set { due, token }),
            Err(err) => tracing::warn!("can't set the deadlines' timer: {}", err.error),
        }
    }

    /// Takes the loop timer away, so that the next refresh pass sets one. A test's move of
    /// the clock does too: the timer counts real time, and would fire that much late.
    pub(crate) fn drop_deadline_timer(&mut self) {
        if let Some(set) = self.deadline_timer.0.take() {
            self.loop_handle.remove(set.token);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: Duration = Duration::from_secs(1);

    #[test]
    fn a_timer_due_at_or_before_the_earliest_deadline_is_kept() {
        assert!(serves(Some(SECOND), Some(SECOND)));
        assert!(serves(Some(SECOND), Some(2 * SECOND)));
    }

    #[test]
    fn a_timer_due_after_the_earliest_deadline_is_replaced() {
        assert!(!serves(Some(2 * SECOND), Some(SECOND)));
    }

    #[test]
    fn a_deadline_with_no_timer_gets_one() {
        assert!(!serves(None, Some(SECOND)));
    }

    #[test]
    fn a_timer_goes_when_nothing_waits() {
        assert!(!serves(Some(SECOND), None));
        assert!(serves(None, None));
    }
}
