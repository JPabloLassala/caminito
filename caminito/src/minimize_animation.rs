//! The minimise animation (172): a window that's minimised shrinks into the rectangle a
//! window list's client set for it (65), as Marco's box animation does
//! (`effects.c:draw_box_animation`), for Marco's 250 ms, at a constant speed. Marco moves a
//! black outline; Caminito draws the window's last content, scaled into the box.
//!
//! Its reverse, the unminimise animation, grows a window that's unminimised out of that
//! rectangle into its frame, the same way. Marco's unminimise effect draws nothing (05), so
//! this one is Caminito's own.
//!
//! Policy has the window minimised from the start, as Marco hides it at once
//! (`effects.c:run_handler` frees the effect, which runs `finish_minimize`, before the box
//! has moved), so the focus moves on at once and only the drawing takes its time; and it
//! has a window unminimised from the start too, at its real place, which the scene leaves
//! empty until the box has reached it. The refresh pass sees a window that was on screen
//! become minimised, or one that was minimised come on screen, whichever way it was, and
//! the scene draws the box until it's over. One that turns round on its way goes back from
//! where its box is, at the same speed, so it takes as long as it had been on its way.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use caminito_policy::{Policy, PolicyWindow, WindowId};
use smithay::desktop::layer_map_for_output;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, Rectangle};

use crate::output::output_geometry;
use crate::state::Caminito;
use crate::window::Mapped;

/// How long the animation lasts: Marco's `META_MINIMIZE_ANIMATION_LENGTH` (`effects.c:69`).
pub(crate) const LENGTH: Duration = Duration::from_millis(250);

/// How long after a step the next is drawn: Marco's timeout (`effects.c:486`).
const STEP: Duration = Duration::from_millis(15);

/// Which way a window is on its way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Way {
    /// From its frame into its button, as it's minimised.
    Minimize,
    /// From its button into its frame, as it's unminimised.
    Unminimize,
}

/// One window on its way to its button, or back from it.
#[derive(Debug)]
struct Animation {
    window: WindowId,
    way: Way,
    /// Where the box starts, globally: the window's frame rect as it was minimised, its
    /// button's rectangle as it was unminimised, or where the box was as it turned round.
    from: Rectangle<i32, Logical>,
    /// Where it ends, globally: the button's rectangle, or the window's frame rect.
    to: Rectangle<i32, Logical>,
    /// When it started, on Caminito's clock.
    start: Duration,
    /// How long it lasts: `LENGTH`, or as long as the one it turned round from had been on
    /// its way.
    length: Duration,
}

impl Animation {
    /// How far on its way it is at `now`, from 0 to 1.
    fn done(&self, now: Duration) -> f64 {
        let elapsed = now.saturating_sub(self.start);
        (elapsed.as_secs_f64() / self.length.as_secs_f64()).min(1.0)
    }

    /// Where the window is drawn at `now`, or `None` once the animation is over.
    fn box_at(&self, now: Duration) -> Option<Rectangle<i32, Logical>> {
        let elapsed = now.saturating_sub(self.start);
        (elapsed < self.length).then(|| box_between(self.from, self.to, self.done(now)))
    }

    /// Whether it goes on while policy has its window as it has now: minimised, or on
    /// screen.
    fn goes_on(&self, policy: &Policy<Mapped>) -> bool {
        match self.way {
            Way::Minimize => policy.is_minimized(self.window),
            Way::Unminimize => policy.is_showing(self.window),
        }
    }
}

/// The box `fraction` of the way from `from` to `to`, never empty, as Marco works it out
/// (`effects.c:369-382`).
fn box_between(
    from: Rectangle<i32, Logical>,
    to: Rectangle<i32, Logical>,
    fraction: f64,
) -> Rectangle<i32, Logical> {
    let at = |from: i32, to: i32| from + (fraction * f64::from(to - from)) as i32;
    Rectangle::new(
        (at(from.loc.x, to.loc.x), at(from.loc.y, to.loc.y)).into(),
        (
            at(from.size.w, to.size.w).max(1),
            at(from.size.h, to.size.h).max(1),
        )
            .into(),
    )
}

/// The windows on their way to their buttons or back, and what the refresh pass needs to
/// see the next one go.
#[derive(Debug, Default)]
pub(crate) struct MinimizeAnimations {
    /// The windows that were on screen at the last refresh pass.
    shown: HashSet<WindowId>,
    /// The windows that were minimised at the last refresh pass.
    minimized: HashSet<WindowId>,
    running: Vec<Animation>,
    /// When the next step is drawn, while any runs: `deadlines` draws it then (174).
    next_step: Option<Duration>,
}

impl MinimizeAnimations {
    /// When the next step is due, while any runs.
    pub(crate) fn deadline(&self) -> Option<Duration> {
        self.next_step
    }

    /// Each window on its way at `now`, with the box it's drawn in, globally.
    pub(crate) fn boxes(
        &self,
        now: Duration,
    ) -> impl Iterator<Item = (WindowId, Rectangle<i32, Logical>)> {
        self.running
            .iter()
            .filter_map(move |animation| Some((animation.window, animation.box_at(now)?)))
    }

    /// Whether `window` is on its way out of its button at `now`, so the scene draws it in
    /// its box and not at its place.
    pub(crate) fn unminimizing(&self, window: WindowId, now: Duration) -> bool {
        self.running.iter().any(|animation| {
            animation.window == window
                && animation.way == Way::Unminimize
                && animation.box_at(now).is_some()
        })
    }
}

impl Caminito {
    /// Starts the animation of each window that was on screen at the last refresh pass and
    /// is minimised now, and of each that was minimised then and is on screen now, and ends
    /// that of each window that's no longer as its animation has it, or gone. One that
    /// turns round goes back from where its box is.
    pub(crate) fn refresh_minimize_animations(&mut self) {
        let now = self.now();
        let policy = &self.policy;
        let ids = || policy.windows().map(|mapped| mapped.id());
        let shown = ids().filter(|&id| policy.is_showing(id)).collect();
        let minimized = ids().filter(|&id| policy.is_minimized(id)).collect();
        let animations = &mut self.minimize_animations;
        let was_shown = std::mem::replace(&mut animations.shown, shown);
        let was_minimized = std::mem::replace(&mut animations.minimized, minimized);

        // Where each window that turned round was on its way, and for how long it had been.
        let mut turned: HashMap<WindowId, (Rectangle<i32, Logical>, Duration)> = HashMap::new();
        let before = animations.running.len();
        animations.running.retain(|animation| {
            let goes_on = animation.goes_on(policy);
            if !goes_on && let Some(drawn) = animation.box_at(now) {
                let gone = animation.length.mul_f64(animation.done(now));
                turned.insert(animation.window, (drawn, gone));
            }
            goes_on
        });
        let mut changed = animations.running.len() != before;

        if self.settings.minimize_animation {
            // A window on its way out is on screen, and one on its way in is minimised:
            // none starts twice.
            let minimizing = was_shown
                .into_iter()
                .filter(|&id| policy.is_minimized(id))
                .map(|id| (id, Way::Minimize));
            // In the Overview, a window that's unminimised is at its place in the layout
            // (241), and comes out of the Overview with it.
            let overview = policy.overview().is_some();
            let unminimizing = was_minimized
                .into_iter()
                .filter(|&id| !overview && policy.is_showing(id))
                .map(|id| (id, Way::Unminimize));
            let starting: Vec<_> = minimizing.chain(unminimizing).collect();
            for (id, way) in starting {
                // With no rectangle there's no animation, where Marco shrinks the box into
                // the screen's bottom right corner (`window.c:1509-1522`).
                let (Some(frame), Some(button)) =
                    (self.policy.frame_rect(id), self.minimize_target(id))
                else {
                    continue;
                };
                let (from, to) = match way {
                    Way::Minimize => (frame, button),
                    Way::Unminimize => (button, frame),
                };
                let (from, length) = match turned.get(&id) {
                    Some(&(drawn, gone)) if !gone.is_zero() => (drawn, gone),
                    _ => (from, LENGTH),
                };
                self.minimize_animations.running.push(Animation {
                    window: id,
                    way,
                    from,
                    to,
                    start: now,
                    length,
                });
                changed = true;
            }
        }
        if changed {
            self.queue_redraw_all();
            self.minimize_animations.next_step.get_or_insert(now + STEP);
        }
    }

    /// Where a window list shows `window`, globally: the rectangle its client set, on a
    /// layer surface or on a window that's on screen.
    fn minimize_target(&self, window: WindowId) -> Option<Rectangle<i32, Logical>> {
        let (surface, rectangle) = self.policy.window(window)?.minimize_rectangle()?;
        let origin = self.surface_origin(surface)?;
        Some(Rectangle::new(rectangle.loc + origin, rectangle.size))
    }

    /// Where the origin of a layer surface, or of the surface of a window on screen, is,
    /// globally.
    fn surface_origin(&self, surface: &WlSurface) -> Option<Point<i32, Logical>> {
        if let Some((output, layer)) = self.layer_for_surface(surface) {
            let geometry = layer_map_for_output(&output).layer_geometry(&layer)?;
            return Some(geometry.loc + output_geometry(&output).loc);
        }
        let id = self.mapped_for_surface(surface)?;
        let policy = &self.policy;
        let (mapped, geometry) = (policy.window(id)?, policy.geometry(id)?);
        policy
            .is_showing(id)
            .then(|| geometry.loc - mapped.window().geometry().loc)
    }

    /// Ends the animations that are over and draws the next step of the others, if a step
    /// is due at `now`, for `deadlines`. A step that's late isn't made up for: the next is
    /// one step after this one.
    pub(crate) fn minimize_animation_elapse(&mut self, now: Duration) {
        let animations = &mut self.minimize_animations;
        if animations.next_step.is_none_or(|step| now < step) {
            return;
        }
        animations
            .running
            .retain(|animation| animation.box_at(now).is_some());
        animations.next_step = (!animations.running.is_empty()).then_some(now + STEP);
        self.queue_redraw_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Logical> {
        Rectangle::new((x, y).into(), (w, h).into())
    }

    #[test]
    fn the_box_goes_from_the_window_to_its_button_at_a_constant_speed() {
        let (from, to) = (rect(100, 50, 400, 300), rect(300, 570, 100, 30));

        assert_eq!(box_between(from, to, 0.0), from);
        assert_eq!(box_between(from, to, 0.5), rect(200, 310, 250, 165));
        assert_eq!(box_between(from, to, 1.0), to);
    }

    #[test]
    fn the_box_is_never_empty() {
        let (from, to) = (rect(0, 0, 400, 300), rect(10, 10, 0, 0));

        assert_eq!(box_between(from, to, 1.0), rect(10, 10, 1, 1));
    }

    #[test]
    fn an_animation_is_over_after_marcos_quarter_second() {
        let animation = Animation {
            window: WindowId(1),
            way: Way::Minimize,
            from: rect(0, 0, 400, 300),
            to: rect(300, 570, 100, 30),
            start: Duration::from_secs(10),
            length: LENGTH,
        };

        assert_eq!(
            animation.box_at(Duration::from_secs(10)),
            Some(animation.from)
        );
        assert!(animation.box_at(Duration::from_millis(10_249)).is_some());
        assert_eq!(animation.box_at(Duration::from_millis(10_250)), None);
    }
}
