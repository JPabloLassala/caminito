//! The Overview's transition (243, ADR 0033): for 250 ms of Caminito's clock as it opens,
//! each window moves and shrinks from its real place to its place in the layout, and back
//! as it closes, on the Workspace slide's ease-out. Pure time and geometry: what's drawn
//! from it is the scene's.
//!
//! Policy has the Overview open or closed from the start, as it has a workspace switched
//! from the start of a slide (233): only the drawing takes its time. The shortcut pressed
//! again part-way reverses it from where it is.

use std::time::Duration;

use smithay::utils::{Logical, Point, Rectangle, Size};

use crate::workspace_slide::{LENGTH, way_left};

/// The Overview's drawing on its way open or closed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Transition {
    /// Whether it's on its way open, or closed.
    pub(crate) opening: bool,
    /// How open the drawing was at `start`, from 0, closed, to 1, open: where the
    /// transition it reversed had it, or the whole way.
    from: f64,
    /// When it started, on Caminito's clock.
    start: Duration,
}

impl Transition {
    /// One on its way open, or closed, from `from` at `now`.
    pub(crate) fn new(opening: bool, from: f64, now: Duration) -> Self {
        Self {
            opening,
            from,
            start: now,
        }
    }

    /// When it's over.
    pub(crate) fn end(&self) -> Duration {
        self.start + LENGTH
    }

    pub(crate) fn is_over(&self, now: Duration) -> bool {
        now >= self.end()
    }

    /// How open the drawing is at `now`, from 0, closed, to 1, open.
    pub(crate) fn part(&self, now: Duration) -> f64 {
        let to = if self.opening { 1.0 } else { 0.0 };
        to + (self.from - to) * way_left(now.saturating_sub(self.start))
    }
}

/// The rectangle `part` of the way from `real` to `slot`: `real` closed, at 0, and `slot`
/// open, at 1.
pub(crate) fn between(
    real: Rectangle<i32, Logical>,
    slot: Rectangle<i32, Logical>,
    part: f64,
) -> Rectangle<i32, Logical> {
    let at = |from: i32, to: i32| (f64::from(from) + f64::from(to - from) * part).round() as i32;
    Rectangle::new(
        Point::from((at(real.loc.x, slot.loc.x), at(real.loc.y, slot.loc.y))),
        Size::from((at(real.size.w, slot.size.w), at(real.size.h, slot.size.h))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn after(millis: u64) -> Duration {
        Duration::from_secs(10) + Duration::from_millis(millis)
    }

    #[test]
    fn opening_starts_at_its_fastest_and_slows_down_into_the_layout() {
        let opening = Transition::new(true, 0.0, after(0));

        assert_eq!(opening.part(after(0)), 0.0);
        // A cubic ease-out: half the time leaves an eighth of the way.
        assert_eq!(opening.part(after(125)), 0.875);
        assert_eq!(opening.part(after(250)), 1.0);
        assert_eq!(opening.part(after(900)), 1.0);
        assert!(!opening.is_over(after(249)) && opening.is_over(after(250)));
    }

    #[test]
    fn closing_is_the_same_way_back() {
        let closing = Transition::new(false, 1.0, after(0));

        assert_eq!(closing.part(after(0)), 1.0);
        assert_eq!(closing.part(after(125)), 0.125);
        assert_eq!(closing.part(after(250)), 0.0);
    }

    #[test]
    fn a_reversal_goes_back_from_where_the_drawing_is() {
        let opening = Transition::new(true, 0.0, after(0));
        let now = after(125);

        let closing = Transition::new(false, opening.part(now), now);

        assert_eq!(closing.part(now), opening.part(now));
        assert_eq!(closing.part(now + LENGTH), 0.0);
        assert_eq!(closing.end(), now + LENGTH);
    }

    #[test]
    fn a_window_is_drawn_between_its_real_place_and_its_slot() {
        let real = Rectangle::new((100, 200).into(), (800, 600).into());
        let slot = Rectangle::new((500, 400).into(), (400, 300).into());

        assert_eq!(between(real, slot, 0.0), real);
        assert_eq!(between(real, slot, 1.0), slot);
        assert_eq!(
            between(real, slot, 0.5),
            Rectangle::new((300, 300).into(), (600, 450).into())
        );
    }
}
