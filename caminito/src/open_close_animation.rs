//! The Opening animation (251, ADR 0034): a window that's shown, for the first time or again
//! after its client hid it, grows from 0.8 of its size to its own about its centre and fades
//! in, for 200 ms of Caminito's clock, with its Frame, its shadow and its popups. It's
//! Plasma's Scale effect.
//!
//! Policy has the window mapped from the start, as it has a window minimised from the start
//! of the minimise animation (172): the focus, input and what its client is told are as
//! with no animation, at its real place, and only the drawing takes its time. A window
//! maps into policy as a new one each time its client shows it (`window/lifecycle.rs`),
//! which notes it here; the refresh pass then starts the animation of each one noted that
//! shows and is a window the animation is for (`animates`), and the scene draws it at the
//! size and opacity it has reached, at every frame of its outputs, until it's over.
//!
//! The numbers and the windows are those of Plasma's Scale effect, in KWin's
//! `src/plugins/scale/package/contents/code/main.js` (ADR 0034).

use std::time::Duration;

use caminito_policy::{PolicyWindow, WindowId, WindowType};
use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle, Size};

use crate::output::output_geometry;
use crate::state::Caminito;
use crate::workspace_slide::eased_way_left;
/// How long the animation lasts: the Scale effect's.
pub(crate) const LENGTH: Duration = Duration::from_millis(200);

/// How much of its own size a window opens from: the Scale effect's.
const FROM_SCALE: f64 = 0.8;

/// How far an animation `elapsed` after its start is on its way, from 0 to 1: the Scale
/// effect's cubic ease-out, so the window grows fastest at first and settles into its place.
fn way_done(elapsed: Duration) -> f64 {
    1.0 - eased_way_left(elapsed, LENGTH)
}

/// Where a window whose frame is at `frame` is drawn `done` of the way through its opening:
/// scaled about the frame's centre, never empty.
pub(crate) fn opening_rect(frame: Rectangle<i32, Logical>, done: f64) -> Rectangle<i32, Logical> {
    let scale = FROM_SCALE + (1.0 - FROM_SCALE) * done;
    let at = |n: i32| ((f64::from(n) * scale).round() as i32).max(1);
    let size = Size::from((at(frame.size.w), at(frame.size.h)));
    let loc = frame.loc + Point::from(((frame.size.w - size.w) / 2, (frame.size.h - size.h) / 2));
    Rectangle::new(loc, size)
}

/// How opaque a window is drawn `done` of the way through its opening: it fades in as it
/// grows.
pub(crate) fn opening_alpha(done: f64) -> f32 {
    done as f32
}

/// One window on its way open.
#[derive(Debug)]
struct Opening {
    window: WindowId,
    /// When it was shown, on Caminito's clock.
    start: Duration,
}

impl Opening {
    fn is_over(&self, now: Duration) -> bool {
        now.saturating_sub(self.start) >= LENGTH
    }
}

/// The windows on their way open, and those shown since the last refresh pass.
#[derive(Debug, Default)]
pub(crate) struct OpenCloseAnimations {
    /// The windows that mapped into policy since the last refresh pass.
    mapped: Vec<WindowId>,
    running: Vec<Opening>,
}

impl OpenCloseAnimations {
    /// Notes a window that mapped into policy, whose animation the refresh pass starts.
    pub(crate) fn window_mapped(&mut self, window: WindowId) {
        self.mapped.push(window);
    }

    /// When the first of the running animations is over: `deadlines` then draws its window
    /// at rest (174).
    pub(crate) fn deadline(&self) -> Option<Duration> {
        self.running
            .iter()
            .map(|opening| opening.start + LENGTH)
            .min()
    }

    /// How far `window`'s opening is on its way at `now`, from 0 to 1, while it runs.
    pub(crate) fn opening(&self, window: WindowId, now: Duration) -> Option<f64> {
        let mut opening = self.opening_windows(now);
        opening.find(|(id, _)| *id == window).map(|(_, done)| done)
    }

    /// The windows on their way open at `now`, in the order they were shown, each with how
    /// far it is on its way, from 0 to 1.
    pub(crate) fn opening_windows(&self, now: Duration) -> impl Iterator<Item = (WindowId, f64)> {
        self.running
            .iter()
            .filter(move |opening| !opening.is_over(now))
            .map(move |opening| (opening.window, way_done(now.saturating_sub(opening.start))))
    }
}

/// Whether a window opens with the animation: one with a Frame, whatever its type, and one
/// of type Normal, Dialog or ModalDialog, as the Scale effect's `isScaleWindow` has it.
fn animates(window: &impl PolicyWindow) -> bool {
    let typed = matches!(
        window.window_type(),
        WindowType::Normal | WindowType::Dialog | WindowType::ModalDialog
    );
    typed || window.is_ssd()
}

impl Caminito {
    /// Starts the animation of each window that mapped since the last refresh pass, shows
    /// and animates, and ends that of each window that no longer shows. With
    /// `open-close-animation` off, or one of the minimise animation's three keys against
    /// it, none starts; a change of those keys lets one that runs finish.
    pub(crate) fn refresh_open_close_animations(&mut self) {
        let now = self.now();
        let policy = &self.policy;
        let animations = &mut self.open_close_animations;
        let mapped = std::mem::take(&mut animations.mapped);
        let before = animations.running.len();
        animations
            .running
            .retain(|opening| policy.is_showing(opening.window));
        let mut changed = animations.running.len() != before;
        if self.settings.open_close_animation {
            for id in mapped {
                // One on a workspace no output shows, or that maps minimised, isn't seen
                // to open.
                let shows = policy.window(id).is_some_and(animates) && policy.is_showing(id);
                if shows {
                    animations.running.push(Opening {
                        window: id,
                        start: now,
                    });
                    changed = true;
                }
            }
        }
        if changed {
            self.queue_redraw_all();
        }
    }

    /// Ends the animations that are over at `now`, and draws their windows at rest, for
    /// `deadlines`. While one runs, the outputs it's on are drawn again at each of their
    /// frames (`State::redraw_queued_outputs`), so there's no step to wait for.
    pub(crate) fn open_close_animation_elapse(&mut self, now: Duration) {
        let running = &mut self.open_close_animations.running;
        let before = running.len();
        running.retain(|opening| !opening.is_over(now));
        if running.len() != before {
            self.queue_redraw_all();
        }
    }

    /// Whether a window on its way open at `now` is drawn on `output`, which is then drawn
    /// again at its next frame.
    pub(crate) fn opening_on(&self, output: &Output, now: Duration) -> bool {
        let area = output_geometry(output);
        self.open_close_animations
            .opening_windows(now)
            .filter_map(|(id, done)| Some(opening_rect(self.policy.frame_rect(id)?, done)))
            .any(|drawn| drawn.overlaps(area))
    }

    /// Each window on its way open at `now`, in the order they were shown, with where its
    /// frame is drawn, globally, and its opacity, for the debug dump.
    pub(crate) fn drawn_opening_windows(
        &self,
        now: Duration,
    ) -> Vec<(WindowId, Rectangle<i32, Logical>, f32)> {
        self.open_close_animations
            .opening_windows(now)
            .filter_map(|(id, done)| {
                let frame = self.policy.frame_rect(id)?;
                Some((id, opening_rect(frame, done), opening_alpha(done)))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Logical> {
        Rectangle::new((x, y).into(), (w, h).into())
    }

    #[test]
    fn a_window_opens_from_four_fifths_of_its_size_about_its_centre_and_no_opacity() {
        let frame = rect(100, 50, 400, 300);

        assert_eq!(opening_rect(frame, 0.0), rect(140, 80, 320, 240));
        assert_eq!(opening_rect(frame, 0.5), rect(120, 65, 360, 270));
        assert_eq!(opening_rect(frame, 1.0), frame);
        assert_eq!(opening_alpha(0.0), 0.0);
        assert_eq!(opening_alpha(1.0), 1.0);
    }

    #[test]
    fn the_opening_eases_out() {
        assert_eq!(way_done(Duration::ZERO), 0.0);
        // A cubic ease-out is seven eighths of the way at half time.
        assert_eq!(way_done(LENGTH / 2), 0.875);
        assert_eq!(way_done(LENGTH), 1.0);
        assert_eq!(way_done(2 * LENGTH), 1.0);
    }

    #[test]
    fn an_opening_is_over_after_plasmas_200_ms() {
        let mut animations = OpenCloseAnimations::default();
        let start = Duration::from_secs(10);
        animations.running.push(Opening {
            window: WindowId(1),
            start,
        });

        assert_eq!(animations.opening(WindowId(1), start), Some(0.0));
        assert!(
            animations
                .opening(WindowId(1), start + LENGTH / 2)
                .is_some()
        );
        assert_eq!(animations.opening(WindowId(1), start + LENGTH), None);
        assert_eq!(animations.deadline(), Some(start + LENGTH));
    }
}
