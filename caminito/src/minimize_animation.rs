//! The minimise animation (172): a window that's minimised shrinks into the rectangle a
//! window list's client set for it (65), as Marco's box animation does
//! (`effects.c:draw_box_animation`), for Marco's 250 ms, at a constant speed. Marco moves a
//! black outline; Caminito draws the window's last content, scaled into the box.
//!
//! Policy has the window minimised from the start, as Marco hides it at once
//! (`effects.c:run_handler` frees the effect, which runs `finish_minimize`, before the box
//! has moved), so the focus moves on at once and only the drawing takes its time. The
//! refresh pass sees a window that was on screen become minimised, whichever way it was,
//! and the scene draws the box until it's over.

use std::collections::HashSet;
use std::time::Duration;

use caminito_policy::{PolicyWindow, WindowId};
use smithay::desktop::layer_map_for_output;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, Rectangle};

use crate::output::output_geometry;
use crate::state::Caminito;

/// How long the animation lasts: Marco's `META_MINIMIZE_ANIMATION_LENGTH` (`effects.c:69`).
pub(crate) const LENGTH: Duration = Duration::from_millis(250);

/// How long after a step the next is drawn: Marco's timeout (`effects.c:486`).
const STEP: Duration = Duration::from_millis(15);

/// One window on its way to its button.
#[derive(Debug)]
struct Animation {
    window: WindowId,
    /// The window's frame rect as it was minimised, globally.
    from: Rectangle<i32, Logical>,
    /// Its button's rectangle then, globally.
    to: Rectangle<i32, Logical>,
    /// When it was minimised, on Caminito's clock.
    start: Duration,
}

impl Animation {
    /// Where the window is drawn at `now`, or `None` once the animation is over.
    fn box_at(&self, now: Duration) -> Option<Rectangle<i32, Logical>> {
        let elapsed = now.saturating_sub(self.start);
        (elapsed < LENGTH).then(|| {
            let fraction = elapsed.as_secs_f64() / LENGTH.as_secs_f64();
            box_between(self.from, self.to, fraction)
        })
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

/// The windows on their way to their buttons, and what the refresh pass needs to see the
/// next one go.
#[derive(Debug, Default)]
pub(crate) struct MinimizeAnimations {
    /// The windows that were on screen at the last refresh pass.
    shown: HashSet<WindowId>,
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
}

impl Caminito {
    /// Starts the animation of each window that was on screen at the last refresh pass and
    /// is minimised now, and ends that of each window that's no longer minimised, or gone.
    pub(crate) fn refresh_minimize_animations(&mut self) {
        let now = self.now();
        let policy = &self.policy;
        let shown = policy
            .windows()
            .map(|mapped| mapped.id())
            .filter(|&id| policy.is_showing(id))
            .collect();
        let was_shown = std::mem::replace(&mut self.minimize_animations.shown, shown);

        let running = &mut self.minimize_animations.running;
        let before = running.len();
        running.retain(|animation| policy.is_minimized(animation.window));
        let mut changed = running.len() != before;

        if self.settings.minimize_animation {
            for id in was_shown {
                // A window on its way is minimised, so it wasn't on screen: none starts twice.
                if !policy.is_minimized(id) {
                    continue;
                }
                // With no rectangle there's no animation, where Marco shrinks the box into
                // the screen's bottom right corner (`window.c:1509-1522`).
                let (Some(from), Some(to)) = (policy.frame_rect(id), self.minimize_target(id))
                else {
                    continue;
                };
                self.minimize_animations.running.push(Animation {
                    window: id,
                    from,
                    to,
                    start: now,
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
            from: rect(0, 0, 400, 300),
            to: rect(300, 570, 100, 30),
            start: Duration::from_secs(10),
        };

        assert_eq!(
            animation.box_at(Duration::from_secs(10)),
            Some(animation.from)
        );
        assert!(animation.box_at(Duration::from_millis(10_249)).is_some());
        assert_eq!(animation.box_at(Duration::from_millis(10_250)), None);
    }
}
