//! Hot corners (CMN-183): which pixels are corners, when one the pointer rests in fires,
//! and what it does then. It's Plasma's screen edges, without the edges.
//!
//! A corner fires whatever is under the pointer, the panel included, and shows nothing. It
//! is off while a button is held, while the seat has an Interaction other than the
//! corner's own Overview, while the session is locked, while a fullscreen window is focused
//! on its output and while a client has locked or confined the pointer.

use std::time::Duration;

use caminito_policy::{Action, Interaction};
use smithay::backend::input::InputTime;
use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle};

use crate::output::{output_geometry, output_id};
use crate::settings::translate::HotCornerAction;
use crate::state::{Caminito, SEAT, State};

/// How long the pointer rests in a corner before it fires: Plasma's activation delay,
/// KWin's `ElectricBorderDelay`.
const REST: Duration = Duration::from_millis(150);

/// How long after a corner fires before it fires again: Plasma's reactivation delay,
/// KWin's `ElectricBorderCooldown`.
const REARM: Duration = Duration::from_millis(350);

/// A corner of an output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// A Hot corner: the corner pixel of an output, in global logical coordinates, and which
/// corner of it that is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Spot {
    at: Point<i32, Logical>,
    corner: Corner,
}

/// The Hot corner at `at` of the Output layout of `outputs`, if `at` is one: the corner
/// pixel of an output where the pointer can go no further either way. That's an outer
/// corner of the layout, or an inner one that outputs of different sizes or offset ones
/// leave, but not a corner another output continues past, not even one that only touches
/// it at the corner, where the pointer goes on diagonally.
fn corner_at(outputs: &[Rectangle<i32, Logical>], at: Point<i32, Logical>) -> Option<Spot> {
    let on_an_output = |x, y| outputs.iter().any(|output| output.contains((x, y)));
    outputs.iter().find_map(|output| {
        let (left, top) = (output.loc.x, output.loc.y);
        let (right, bottom) = (left + output.size.w - 1, top + output.size.h - 1);
        let corner = match (at.x, at.y) {
            (x, y) if x == left && y == top => Corner::TopLeft,
            (x, y) if x == right && y == top => Corner::TopRight,
            (x, y) if x == left && y == bottom => Corner::BottomLeft,
            (x, y) if x == right && y == bottom => Corner::BottomRight,
            _ => return None,
        };
        let dx = if at.x == left { -1 } else { 1 };
        let dy = if at.y == top { -1 } else { 1 };
        let stops = [(dx, 0), (0, dy), (dx, dy)]
            .into_iter()
            .all(|(dx, dy)| !on_an_output(at.x + dx, at.y + dy));
        stops.then_some(Spot { at, corner })
    })
}

/// When the Hot corner the pointer rests in fires.
#[derive(Debug, Default)]
pub(crate) struct HotCornerState {
    /// The corner the pointer rests in, and since when.
    resting: Option<(Spot, Duration)>,
    /// The corner that fired last, when, and whether the pointer has left it since. Another
    /// corner doesn't wait on it.
    fired: Option<Fired>,
}

#[derive(Debug, Clone, Copy)]
struct Fired {
    spot: Spot,
    at: Duration,
    left: bool,
}

impl HotCornerState {
    /// The pointer is in `spot`, or in no Hot corner, at `now`.
    fn pointer_at(&mut self, spot: Option<Spot>, now: Duration) {
        if let Some(fired) = &mut self.fired
            && spot != Some(fired.spot)
        {
            fired.left = true;
        }
        if self.resting.map(|(resting, _)| resting) != spot {
            self.resting = spot.map(|spot| (spot, now));
        }
    }

    /// The pointer's rest in its corner doesn't count, as while the corner is off. The next
    /// motion in it starts another.
    fn cancel(&mut self) {
        self.resting = None;
    }

    /// When the corner the pointer rests in fires, if it will: once it has rested there
    /// `REST`, and, if it's the corner that fired last, once the pointer has left it and
    /// `REARM` has passed since it fired.
    pub(crate) fn deadline(&self) -> Option<Duration> {
        let (spot, since) = self.resting?;
        let rested = since + REST;
        match self.fired {
            Some(fired) if fired.spot == spot && !fired.left => None,
            Some(fired) if fired.spot == spot => Some(rested.max(fired.at + REARM)),
            _ => Some(rested),
        }
    }

    /// Whether the corner the pointer rests in fires at `now`.
    fn elapse(&mut self, now: Duration) -> bool {
        let Some((spot, _)) = self
            .resting
            .filter(|_| self.deadline().is_some_and(|due| due <= now))
        else {
            return false;
        };
        self.fired = Some(Fired {
            spot,
            at: now,
            left: false,
        });
        true
    }
}

/// The Hot corner the pointer is in, with what it does and its output.
struct AtCorner {
    spot: Spot,
    action: HotCornerAction,
    output: Output,
}

impl Caminito {
    /// The Hot corner at `location`, if its key gives it an action.
    fn hot_corner_at(&self, location: Point<f64, Logical>) -> Option<AtCorner> {
        let outputs: Vec<_> = self.outputs.iter().map(output_geometry).collect();
        let spot = corner_at(&outputs, location.to_i32_floor())?;
        let corners = &self.settings.hot_corners;
        let action = match spot.corner {
            Corner::TopLeft => corners.top_left,
            Corner::TopRight => corners.top_right,
            Corner::BottomLeft => corners.bottom_left,
            Corner::BottomRight => corners.bottom_right,
        };
        if action == HotCornerAction::None {
            return None;
        }
        let output = self
            .outputs
            .iter()
            .find(|output| output_geometry(output).contains(spot.at))?
            .clone();
        Some(AtCorner {
            spot,
            action,
            output,
        })
    }
}

impl State {
    /// The pointer moved to `location`: a rest in a Hot corner starts, or ends.
    pub(crate) fn hot_corner_motion(&mut self, location: Point<f64, Logical>) {
        let now = self.caminito.now();
        self.hot_corner_pointer_at(location, now);
    }

    /// The pointer is at `location` at `now`: the Hot corner it's in, with an action and
    /// not off, if it's in one. One that's off doesn't count the rest.
    fn hot_corner_pointer_at(
        &mut self,
        location: Point<f64, Logical>,
        now: Duration,
    ) -> Option<AtCorner> {
        let at = self.caminito.hot_corner_at(location);
        let corners = &mut self.caminito.hot_corners;
        corners.pointer_at(at.as_ref().map(|at| at.spot), now);
        let at = at?;
        if self.hot_corner_is_off(&at) {
            self.caminito.hot_corners.cancel();
            return None;
        }
        Some(at)
    }

    /// Fires the Hot corner the pointer has rested in long enough, if it's still a corner
    /// with an action, as the outputs and the keys may have changed since it came, and if
    /// it isn't off.
    pub(crate) fn hot_corner_elapse(&mut self, now: Duration) {
        if self
            .caminito
            .hot_corners
            .deadline()
            .is_none_or(|due| due > now)
        {
            return;
        }
        let location = self.caminito.seat.get_pointer().unwrap().current_location();
        let Some(at) = self.hot_corner_pointer_at(location, now) else {
            return;
        };
        if !self.caminito.hot_corners.elapse(now) {
            return;
        }
        match at.action {
            // `hot_corner_at` gives none.
            HotCornerAction::None => {}
            // As the Overview's binding, on the corner's output.
            HotCornerAction::Overview => {
                let time = InputTime::from_micros(now.as_micros() as u64);
                self.toggle_overview_on(Some(output_id(&at.output)), time);
            }
            HotCornerAction::ShowDesktop => {
                self.caminito.policy.action(SEAT, Action::ShowDesktop, now);
            }
            HotCornerAction::Lock => {
                self.request_lock();
            }
        }
    }

    /// Whether the Hot corner `at` is off.
    fn hot_corner_is_off(&self, at: &AtCorner) -> bool {
        let caminito = &self.caminito;
        let policy = &caminito.policy;
        let own_overview = at.action == HotCornerAction::Overview;
        let interaction = match policy.interaction(SEAT) {
            Some(Interaction::Overview) => !own_overview,
            interaction => interaction.is_some(),
        };
        let fullscreen = policy.focus(SEAT).is_some_and(|id| {
            policy.is_fullscreen(id) && policy.output_showing(id) == Some(output_id(&at.output))
        });
        // A press that ran a binding, or that the Overview took, reached no client.
        let held = self.pointer_is_held() || !caminito.suppressed_buttons.is_empty();
        caminito.lock.is_locked()
            || held
            || interaction
            || fullscreen
            || self.pointer_is_constrained()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Logical> {
        Rectangle::new((x, y).into(), (w, h).into())
    }

    fn spot(x: i32, y: i32, corner: Corner) -> Option<Spot> {
        Some(Spot {
            at: (x, y).into(),
            corner,
        })
    }

    fn ms(ms: u64) -> Duration {
        Duration::from_millis(ms)
    }

    /// One output of 1920 × 1080.
    fn one() -> [Rectangle<i32, Logical>; 1] {
        [rect(0, 0, 1920, 1080)]
    }

    #[test]
    fn an_outputs_four_corner_pixels_are_corners() {
        assert_eq!(
            corner_at(&one(), (0, 0).into()),
            spot(0, 0, Corner::TopLeft)
        );
        assert_eq!(
            corner_at(&one(), (1919, 0).into()),
            spot(1919, 0, Corner::TopRight)
        );
        assert_eq!(
            corner_at(&one(), (0, 1079).into()),
            spot(0, 1079, Corner::BottomLeft)
        );
        assert_eq!(
            corner_at(&one(), (1919, 1079).into()),
            spot(1919, 1079, Corner::BottomRight)
        );
    }

    #[test]
    fn a_pixel_beside_a_corner_is_none() {
        assert_eq!(corner_at(&one(), (1, 0).into()), None);
        assert_eq!(corner_at(&one(), (0, 1).into()), None);
        assert_eq!(corner_at(&one(), (1918, 1079).into()), None);
        assert_eq!(corner_at(&one(), (500, 500).into()), None);
    }

    #[test]
    fn a_corner_another_output_continues_past_is_none() {
        let outputs = [rect(0, 0, 1920, 1080), rect(1920, 0, 1920, 1080)];
        assert_eq!(corner_at(&outputs, (1919, 0).into()), None);
        assert_eq!(corner_at(&outputs, (1920, 1079).into()), None);
        assert_eq!(
            corner_at(&outputs, (3839, 0).into()),
            spot(3839, 0, Corner::TopRight)
        );
    }

    #[test]
    fn the_inner_corner_of_a_taller_output_beside_a_shorter_one_is_a_corner() {
        let outputs = [rect(0, 0, 1920, 1080), rect(1920, 0, 1920, 1200)];
        assert_eq!(
            corner_at(&outputs, (1920, 1199).into()),
            spot(1920, 1199, Corner::BottomLeft)
        );
        // The shorter one's bottom-right corner, which the taller one continues past.
        assert_eq!(corner_at(&outputs, (1919, 1079).into()), None);
    }

    #[test]
    fn a_corner_another_output_touches_diagonally_is_none() {
        let outputs = [rect(0, 0, 1920, 1080), rect(1920, 1080, 1920, 1080)];
        assert_eq!(corner_at(&outputs, (1919, 1079).into()), None);
        assert_eq!(corner_at(&outputs, (1920, 1080).into()), None);
        assert_eq!(
            corner_at(&outputs, (1919, 0).into()),
            spot(1919, 0, Corner::TopRight)
        );
    }

    #[test]
    fn another_corner_doesnt_wait_the_350_ms() {
        let mut state = HotCornerState::default();
        state.pointer_at(spot(0, 0, Corner::TopLeft), ms(1000));
        state.elapse(ms(1150));
        state.pointer_at(spot(1919, 0, Corner::TopRight), ms(1160));

        assert_eq!(state.deadline(), Some(ms(1310)));
    }

    #[test]
    fn a_corner_fires_after_the_pointer_rests_in_it_150_ms() {
        let mut state = HotCornerState::default();
        let corner = spot(0, 0, Corner::TopLeft);
        state.pointer_at(corner, ms(1000));

        assert_eq!(state.deadline(), Some(ms(1150)));
        assert!(!state.elapse(ms(1149)));
        assert!(state.elapse(ms(1150)));
        assert_eq!(state.deadline(), None);
    }

    #[test]
    fn a_corner_the_pointer_leaves_sooner_doesnt_fire() {
        let mut state = HotCornerState::default();
        state.pointer_at(spot(0, 0, Corner::TopLeft), ms(1000));
        state.pointer_at(None, ms(1100));

        assert_eq!(state.deadline(), None);
        assert!(!state.elapse(ms(1150)));
    }

    #[test]
    fn motion_within_the_corner_keeps_the_rest() {
        let mut state = HotCornerState::default();
        let corner = spot(0, 0, Corner::TopLeft);
        state.pointer_at(corner, ms(1000));
        state.pointer_at(corner, ms(1100));

        assert!(state.elapse(ms(1150)));
    }

    #[test]
    fn a_corner_fires_once_while_the_pointer_stays() {
        let mut state = HotCornerState::default();
        let corner = spot(0, 0, Corner::TopLeft);
        state.pointer_at(corner, ms(1000));
        state.elapse(ms(1150));
        state.pointer_at(corner, ms(2000));

        assert_eq!(state.deadline(), None);
        assert!(!state.elapse(ms(5000)));
    }

    #[test]
    fn it_fires_again_only_350_ms_after_it_fired() {
        let mut state = HotCornerState::default();
        let corner = spot(0, 0, Corner::TopLeft);
        state.pointer_at(corner, ms(1000));
        state.elapse(ms(1150));
        state.pointer_at(None, ms(1160));
        state.pointer_at(corner, ms(1170));

        assert_eq!(state.deadline(), Some(ms(1500)));
        assert!(!state.elapse(ms(1499)));
        assert!(state.elapse(ms(1500)));
    }

    #[test]
    fn a_rest_that_outlasts_the_350_ms_fires_150_ms_after_it_began() {
        let mut state = HotCornerState::default();
        let corner = spot(0, 0, Corner::TopLeft);
        state.pointer_at(corner, ms(1000));
        state.elapse(ms(1150));
        state.pointer_at(None, ms(1200));
        state.pointer_at(corner, ms(2000));

        assert_eq!(state.deadline(), Some(ms(2150)));
    }

    #[test]
    fn a_cancelled_rest_starts_again_with_the_next_motion() {
        let mut state = HotCornerState::default();
        let corner = spot(0, 0, Corner::TopLeft);
        state.pointer_at(corner, ms(1000));
        state.cancel();
        assert_eq!(state.deadline(), None);

        state.pointer_at(corner, ms(1100));
        assert_eq!(state.deadline(), Some(ms(1250)));
    }
}
