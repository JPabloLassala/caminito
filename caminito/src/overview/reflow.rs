//! The layout's windows easing to their new slots (250): pure time and geometry, as
//! `transition` is. What's drawn from it is the scene's.
//!
//! When a window leaves an output's layout, or joins it, the layout is laid out again and
//! each window that stays there eases from where it was drawn to its new slot, in 250 ms
//! on the Workspace slide's ease-out. Another change part-way eases each window from where
//! it's drawn then to its newest slot, as a reversal of the transition does.

use std::time::Duration;

use smithay::utils::{Logical, Rectangle};

use caminito_policy::WindowId;

use super::transition::between;
use crate::workspace_slide::{LENGTH, way_left};

/// The windows of one layout on their way from where they were drawn to their slots.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Reflow {
    /// Where each window that stayed was drawn when the layout changed, from the output's
    /// top left corner. A window that joined isn't here, and is at its slot at once.
    from: Vec<(WindowId, Rectangle<i32, Logical>)>,
    /// When the layout changed, on Caminito's clock.
    start: Duration,
}

impl Reflow {
    /// The windows of `from`, drawn there at `now`, on their way to their slots.
    pub(crate) fn new(from: Vec<(WindowId, Rectangle<i32, Logical>)>, now: Duration) -> Self {
        Self { from, start: now }
    }

    /// Where window `id` is drawn at `now`, on its way to `slot`.
    pub(crate) fn rect(
        &self,
        id: WindowId,
        slot: Rectangle<i32, Logical>,
        now: Duration,
    ) -> Rectangle<i32, Logical> {
        let from = self.from.iter().find(|(window, _)| *window == id);
        match from {
            Some((_, from)) => between(*from, slot, 1.0 - way_left(now.saturating_sub(self.start))),
            None => slot,
        }
    }

    /// When every window is at its slot.
    pub(crate) fn end(&self) -> Duration {
        self.start + LENGTH
    }

    /// Whether every window is at its slot at `now`.
    pub(crate) fn is_over(&self, now: Duration) -> bool {
        now >= self.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn after(millis: u64) -> Duration {
        Duration::from_secs(10) + Duration::from_millis(millis)
    }

    #[test]
    fn a_window_that_stays_eases_from_where_it_was_to_its_new_slot() {
        let old = Rectangle::new((100, 200).into(), (400, 300).into());
        let slot = Rectangle::new((500, 200).into(), (600, 450).into());
        let reflow = Reflow::new(vec![(WindowId(1), old)], after(0));

        assert_eq!(reflow.rect(WindowId(1), slot, after(0)), old);
        // Half the time leaves an eighth of the way.
        assert_eq!(
            reflow.rect(WindowId(1), slot, after(125)),
            Rectangle::new((450, 200).into(), (575, 431).into())
        );
        assert_eq!(reflow.rect(WindowId(1), slot, after(250)), slot);
        assert!(!reflow.is_over(after(249)) && reflow.is_over(after(250)));
    }

    #[test]
    fn a_window_that_joined_is_at_its_slot() {
        let reflow = Reflow::new(Vec::new(), after(0));
        let slot = Rectangle::new((500, 200).into(), (600, 450).into());

        assert_eq!(reflow.rect(WindowId(2), slot, after(0)), slot);
    }
}
