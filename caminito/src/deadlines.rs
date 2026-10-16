//! Caminito's clock, for what waits on it (174): who is due, and when the loop must wake.
//! Each holder keeps its own deadline in a state that's free of the loop, and this runs
//! what's due and keeps one loop timer for the earliest deadline of them all. A timer of a
//! holder's own is for what waits on a process or a protocol object, which isn't this.

use std::time::Duration;

use smithay::reexports::calloop::RegistrationToken;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};

use crate::input::repeat::Repeat;
use crate::state::{Caminito, State};

/// Everything that waits on Caminito's clock, in the order `run_due` runs what's due at one
/// time:
///
/// 1. Policy, whose auto-raise restacks the windows the rest looks at (153).
/// 2. The pings that go with a request to close (168): one with no answer brings the "not
///    responding" dialog forward if it shows, which restacks the windows too.
/// 3. Idle, which blanks and locks (128).
/// 4. AccessX, whose keys go on to the clients and whose beeps ring the bell (134).
/// 5. The repeat of a held accelerator (99), which signals a key AccessX let through.
/// 6. The Global menu's grace period (114), which nothing else here looks at.
/// 7. The Window switcher's popup (108), the bell's flash (169), the minimise animations
///    (172), the Workspace slides (233), the Overview's transition (243), the Opening
///    animations (251) and the Popup slides (ADR 0037), which only draw.
/// 8. The Overview's bar (244), which only sends frame callbacks to the windows it alone
///    shows.
/// 9. Night Light (138), which only moves the outputs' temperatures.
///
/// A periodic holder that's late runs one time, and is next due a period after the `now`
/// it ran at (176).
const HOLDERS: [Holder; 15] = [
    Holder::Policy,
    Holder::ClosePings,
    Holder::Idle,
    Holder::AccessX,
    Holder::AcceleratorRepeat,
    Holder::GlobalMenu,
    Holder::SwitcherPopup,
    Holder::Bell,
    Holder::MinimizeAnimations,
    Holder::WorkspaceSlides,
    Holder::OverviewTransition,
    Holder::OpenCloseAnimations,
    Holder::PopupSlides,
    Holder::OverviewBar,
    Holder::NightLight,
];

#[derive(Debug, Clone, Copy)]
enum Holder {
    Policy,
    ClosePings,
    Idle,
    AccessX,
    AcceleratorRepeat,
    GlobalMenu,
    SwitcherPopup,
    Bell,
    MinimizeAnimations,
    WorkspaceSlides,
    OverviewTransition,
    OpenCloseAnimations,
    PopupSlides,
    OverviewBar,
    NightLight,
}

impl Holder {
    /// When the holder is next due, if it waits.
    fn deadline(self, caminito: &Caminito) -> Option<Duration> {
        match self {
            Holder::Policy => caminito.policy.deadline(),
            Holder::ClosePings => caminito.not_responding.deadline(),
            Holder::Idle => caminito.idle.deadline(),
            Holder::AccessX => caminito.accessx.deadline(),
            Holder::AcceleratorRepeat => caminito.accelerator_repeat.as_ref().map(Repeat::deadline),
            Holder::GlobalMenu => caminito.global_menu_switch.deadline(),
            Holder::SwitcherPopup => caminito.switcher_popup_deadline(),
            Holder::Bell => caminito.bells.deadline(),
            Holder::MinimizeAnimations => caminito.minimize_animations.deadline(),
            Holder::WorkspaceSlides => caminito.workspace_slides.deadline(),
            Holder::OverviewTransition => caminito.overview_transition_deadline(),
            Holder::OpenCloseAnimations => caminito.open_close_animations.deadline(),
            Holder::PopupSlides => caminito.popup_slides.deadline(),
            Holder::OverviewBar => caminito.overview_deadline(),
            Holder::NightLight => caminito.night_light.deadline(caminito.now()),
        }
    }

    /// Runs what the holder has due at `now`, which is nothing when it isn't due. Idle's and
    /// AccessX's effects reach the backend and the keyboard path, so this takes the whole
    /// state.
    fn elapse(self, state: &mut State, now: Duration) {
        match self {
            Holder::Policy => state.caminito.policy.elapse(now),
            Holder::ClosePings => state.caminito.close_pings_elapse(now),
            Holder::Idle => state.idle_elapse(now),
            Holder::AccessX => state.accessx_elapse(now),
            Holder::AcceleratorRepeat => state.caminito.accelerator_repeat_elapse(now),
            Holder::GlobalMenu => state.caminito.global_menu_time_passed(now),
            Holder::SwitcherPopup => state.caminito.switcher_popup_elapse(now),
            Holder::Bell => state.caminito.bell_elapse(now),
            Holder::MinimizeAnimations => state.caminito.minimize_animation_elapse(now),
            Holder::WorkspaceSlides => state.caminito.workspace_slide_elapse(now),
            Holder::OverviewTransition => state.caminito.overview_transition_elapse(now),
            Holder::OpenCloseAnimations => state.caminito.open_close_animation_elapse(now),
            Holder::PopupSlides => state.caminito.popup_slide_elapse(now),
            Holder::OverviewBar => state.caminito.overview_elapse(now),
            Holder::NightLight => state.caminito.night_light_elapse(now),
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
