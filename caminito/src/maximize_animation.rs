//! The Maximise animation (CMN-174, ADR 0036): a window that's maximised or unmaximised both
//! ways goes from its frame as it was to its new one over 250 ms of Caminito's clock, on a
//! cubic ease-out, with a picture of it as it was fading out over it on the same curve. It's
//! Plasma's Maximize effect, in KWin's `src/plugins/maximize/package/contents/code/main.js`.
//!
//! Policy has the window at its new place from the start, as with the other animations
//! (ADR 0033), but its client draws at the new size a frame or more later: until then
//! policy's frame is at the new place with the old size. So the box goes to the frame
//! policy asks for (`Policy::asked_frame_rect`), read each frame, and the live window is
//! drawn scaled into it at whatever size its client has drawn, with nothing waiting for
//! the client. A client that hasn't drawn by the end is drawn as policy has it, as with no
//! animation.
//!
//! The refresh pass notes each window on screen with its frame at every pass, before its
//! Frame is drawn again (`State::refresh_maximize_animations`), and so sees a window whose
//! maximised state changed since the last pass while its Frame and buffers are still as
//! they were: the picture is drawn from those, and the box starts at the frame it noted.
//!
//! One that's interrupted hands over from what is on screen (CMN-176), where Plasma starts
//! again from the window's geometry (ADR 0036). Maximised or unmaximised again, the box goes
//! from where it's drawn to the newest frame for a fresh 250 ms, and the first picture goes
//! on fading from the opacity it had. Moved or resized, the box goes from where it's drawn
//! to the new frame for what's left. Closed, the Closing animation takes its picture from the
//! box (`Caminito::start_closing`). Minimised, and dragged by the pointer, it ends; maximised
//! on its way open, none starts, as the Opening animation grows it into its new place.

use std::collections::HashMap;
use std::time::Duration;

use caminito_policy::{PolicyWindow, WindowId};
use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle, Scale, Size};

use crate::open_close_animation::{Picture, animates};
use crate::output::output_geometry;
use crate::state::{Caminito, SEAT, State};
use crate::workspace_slide::eased_way_left;

/// How long the animation lasts: the Maximize effect's.
pub(crate) const LENGTH: Duration = Duration::from_millis(250);

/// How far an animation `elapsed` after its start is on its way, from 0 to 1: the Maximize
/// effect's cubic ease-out, for the box and the picture's fade alike.
fn way_done(elapsed: Duration) -> f64 {
    1.0 - eased_way_left(elapsed, LENGTH)
}

/// The rectangle `fraction` of the way from `from` to `to`, never empty.
fn between(
    from: Rectangle<i32, Logical>,
    to: Rectangle<i32, Logical>,
    fraction: f64,
) -> Rectangle<i32, Logical> {
    let at = |from: i32, to: i32| from + (fraction * f64::from(to - from)).round() as i32;
    Rectangle::new(
        (at(from.loc.x, to.loc.x), at(from.loc.y, to.loc.y)).into(),
        (
            at(from.size.w, to.size.w).max(1),
            at(from.size.h, to.size.h).max(1),
        )
            .into(),
    )
}

/// Where `area`, from the corner of something of `size`, is once that's scaled to fill
/// `target`, as the scene draws a window and its picture into their box.
pub(crate) fn scaled_into(
    area: Rectangle<i32, Logical>,
    size: Size<i32, Logical>,
    target: Rectangle<i32, Logical>,
) -> Rectangle<f64, Logical> {
    let factor = Scale::from((
        f64::from(target.size.w) / f64::from(size.w.max(1)),
        f64::from(target.size.h) / f64::from(size.h.max(1)),
    ));
    let area = area.to_f64();
    Rectangle::new(
        target.loc.to_f64() + area.loc.upscale(factor),
        area.size.upscale(factor),
    )
}

/// A window on screen as a refresh pass found it.
#[derive(Debug, Clone, Copy)]
struct Seen {
    maximized: bool,
    /// Its frame, globally.
    frame: Rectangle<i32, Logical>,
    /// Its geometry's corner, globally.
    origin: Point<i32, Logical>,
}

/// One window on its way to its new frame.
#[derive(Debug)]
pub(crate) struct Maximizing {
    pub window: WindowId,
    /// Where the box starts, globally: the window's frame as it was, or where its box was
    /// drawn when it was interrupted.
    from: Rectangle<i32, Logical>,
    /// How far on its way it was when the box left `from`: none but after a move or a
    /// resize, from which it goes on for what's left (CMN-176).
    from_done: f64,
    /// Where the box goes, globally: the frame policy asked for when the refresh pass last
    /// looked.
    to: Rectangle<i32, Logical>,
    /// When it started, on Caminito's clock.
    start: Duration,
    /// How opaque its picture was at `start`: less than whole for one maximised again on its
    /// way, whose first picture goes on fading (CMN-176).
    picture_alpha: f32,
    /// The window as it was, with its area from the corner of `picture_frame`, if it had a
    /// buffer to draw it from.
    pub picture: Option<Picture>,
    /// The frame the picture was taken of, globally, which it's drawn scaled from.
    pub picture_frame: Rectangle<i32, Logical>,
}

impl Maximizing {
    fn is_over(&self, now: Duration) -> bool {
        now.saturating_sub(self.start) >= LENGTH
    }

    fn done(&self, now: Duration) -> f64 {
        way_done(now.saturating_sub(self.start))
    }

    /// Where the box is at `now`, globally.
    pub(crate) fn rect(&self, now: Duration) -> Rectangle<i32, Logical> {
        if self.from_done >= 1.0 {
            return self.to;
        }
        let fraction = (self.done(now) - self.from_done) / (1.0 - self.from_done);
        between(self.from, self.to, fraction)
    }

    /// How opaque the picture is drawn at `now`.
    pub(crate) fn alpha(&self, now: Duration) -> f32 {
        self.picture_alpha * (1.0 - self.done(now)) as f32
    }

    /// Sends the box from where it's drawn at `now` to `to`, for what's left of the
    /// animation: its window was moved or resized (CMN-176).
    fn hand_over(&mut self, to: Rectangle<i32, Logical>, now: Duration) {
        self.from = self.rect(now);
        self.from_done = self.done(now);
        self.to = to;
    }
}

/// The windows on their way to their new frames, and the windows on screen at the last
/// refresh pass.
#[derive(Debug, Default)]
pub(crate) struct MaximizeAnimations {
    seen: HashMap<WindowId, Seen>,
    running: Vec<Maximizing>,
}

impl MaximizeAnimations {
    /// When the first of the running animations is over: `deadlines` then draws its window
    /// at rest (174).
    pub(crate) fn deadline(&self) -> Option<Duration> {
        self.running.iter().map(|m| m.start + LENGTH).min()
    }

    /// `window`'s animation, while it runs at `now`.
    pub(crate) fn maximizing(&self, window: WindowId, now: Duration) -> Option<&Maximizing> {
        self.running
            .iter()
            .find(|m| m.window == window && !m.is_over(now))
    }

    /// Ends `window`'s animation, while it runs at `now`, and gives it to the closing that
    /// takes over from it, with its picture (CMN-176).
    pub(crate) fn take(&mut self, window: WindowId, now: Duration) -> Option<Maximizing> {
        let index = self
            .running
            .iter()
            .position(|m| m.window == window && !m.is_over(now))?;
        Some(self.running.remove(index))
    }
}

impl State {
    /// Notes each window on screen with its frame, and starts the animation of each whose
    /// maximised state changed since the last pass, from the frame noted then, with a
    /// picture of it as it still is. Called before the Frames are drawn again in their new
    /// state. Ends the animation of each window no longer on screen or that the pointer
    /// drags, and hands each over to its window's new frame from where it's drawn.
    pub(crate) fn refresh_maximize_animations(&mut self) {
        let now = self.caminito.now();
        let policy = &self.caminito.policy;
        let animations = &mut self.caminito.maximize_animations;
        let mut seen = HashMap::new();
        let mut changed = Vec::new();
        for mapped in policy.windows() {
            let id = mapped.id();
            let (Some(frame), Some(geometry)) = (policy.frame_rect(id), policy.geometry(id)) else {
                continue;
            };
            if !policy.is_showing(id) {
                continue;
            }
            let now_seen = Seen {
                maximized: policy.is_maximized(id),
                frame,
                origin: geometry.loc,
            };
            if let Some(was) = animations.seen.get(&id)
                && was.maximized != now_seen.maximized
            {
                changed.push((id, *was));
            }
            seen.insert(id, now_seen);
        }
        animations.seen = seen;
        let before = animations.running.len();
        // A minimised window goes to its button from its real place, and a dragged one
        // follows the pointer at once (CMN-176).
        let dragged = policy.dragging(SEAT);
        let showing = |m: &Maximizing| {
            policy.is_showing(m.window) && dragged != Some(m.window) && !m.is_over(now)
        };
        animations.running.retain(showing);
        let mut redraw = animations.running.len() != before;
        for m in &mut animations.running {
            if let Some(to) = policy.asked_frame_rect(m.window)
                && to != m.to
            {
                m.hand_over(to, now);
            }
        }

        for (id, was) in changed {
            if !self.caminito.maximize_seen(id, now) {
                continue;
            }
            let policy = &self.caminito.policy;
            let (Some(frame), Some(geometry), Some(to)) = (
                policy.frame_rect(id),
                policy.geometry(id),
                policy.asked_frame_rect(id),
            ) else {
                continue;
            };
            let animations = &mut self.caminito.maximize_animations;
            let running = animations.running.iter().position(|m| m.window == id);
            let maximizing = match running.map(|index| animations.running.remove(index)) {
                // Changed again on its way: it goes on from where its box is drawn for a fresh
                // 250 ms, with the first picture fading on from where it had got to.
                Some(m) => Maximizing {
                    from: m.rect(now),
                    from_done: 0.0,
                    to,
                    start: now,
                    picture_alpha: m.alpha(now),
                    ..m
                },
                None => {
                    // The picture's area is from the corner of the frame policy has now,
                    // around the Frame and buffers as they were, which were at `was`.
                    let picture = self.backend.take_picture(&self.caminito, id);
                    let picture = picture.map(|mut picture| {
                        let shift = (frame.loc - geometry.loc) + (was.origin - was.frame.loc);
                        picture.area.loc += shift;
                        picture
                    });
                    Maximizing {
                        window: id,
                        from: was.frame,
                        from_done: 0.0,
                        to,
                        start: now,
                        picture_alpha: 1.0,
                        picture,
                        picture_frame: was.frame,
                    }
                }
            };
            self.caminito.maximize_animations.running.push(maximizing);
            redraw = true;
        }
        if redraw {
            self.caminito.queue_redraw_all();
        }
    }
}

impl Caminito {
    /// Whether window `id`, whose maximised state changed, is seen to change at `now`: the
    /// animation is on, the window is one it's for, it's not fullscreen, it isn't on its
    /// way open, which grows it into its new place itself (CMN-112), and no grab pulled it
    /// off its maximised place, which it then follows the pointer from (CMN-176).
    fn maximize_seen(&self, id: WindowId, now: Duration) -> bool {
        let policy = &self.policy;
        self.settings.maximize_animation
            && policy.window(id).is_some_and(animates)
            && !policy.is_fullscreen(id)
            && self.open_close_animations.opening(id, now).is_none()
            && policy.grab(SEAT) != Some(id)
    }

    /// Ends the animations that are over at `now`, and draws their windows at rest, for
    /// `deadlines`. While one runs, the outputs it's on are drawn again at each of their
    /// frames (`State::redraw_queued_outputs`).
    pub(crate) fn maximize_animation_elapse(&mut self, now: Duration) {
        let animations = &mut self.maximize_animations;
        let before = animations.running.len();
        animations.running.retain(|m| !m.is_over(now));
        if animations.running.len() != before {
            self.queue_redraw_all();
        }
    }

    /// Each window on its way at `now`, with its box, globally, and its picture's opacity.
    pub(crate) fn drawn_maximizing_windows(
        &self,
        now: Duration,
    ) -> Vec<(WindowId, Rectangle<i32, Logical>, f32)> {
        self.maximize_animations
            .running
            .iter()
            .filter(|m| !m.is_over(now))
            .map(|m| (m.window, m.rect(now), m.alpha(now)))
            .collect()
    }

    /// Whether a window on its way at `now` is drawn on `output`, which is then drawn again
    /// at its next frame. What's drawn reaches past its box, scaled with it: its surfaces past
    /// its window geometry, as a CSD window's shadow, its Frame's shadow, and its picture's.
    pub(crate) fn maximize_animation_on(&self, output: &Output, now: Duration) -> bool {
        let area = output_geometry(output).to_f64();
        self.maximize_animations
            .running
            .iter()
            .filter(|m| !m.is_over(now))
            .filter_map(|m| {
                let frame = self.policy.frame_rect(m.window)?;
                let drawn = m.rect(now);
                let mut covered = drawn.to_f64();
                if let Some(reach) = self.reach(m.window) {
                    covered = covered.merge(scaled_into(reach, frame.size, drawn));
                }
                if let Some(picture) = &m.picture {
                    let size = m.picture_frame.size;
                    covered = covered.merge(scaled_into(picture.area, size, drawn));
                }
                Some(covered)
            })
            .any(|covered| covered.overlaps(area))
    }

    /// What window `id` draws, with its popups and its Frame's shadow, from its frame's
    /// corner.
    fn reach(&self, id: WindowId) -> Option<Rectangle<i32, Logical>> {
        let policy = &self.policy;
        let (mapped, geometry, frame) = (
            policy.window(id)?,
            policy.geometry(id)?,
            policy.frame_rect(id)?,
        );
        let window = mapped.window();
        let mut reach = Rectangle::from_size(frame.size);
        let mut surfaces = window.bbox_with_popups();
        surfaces.loc += geometry.loc - window.geometry().loc - frame.loc;
        reach = reach.merge(surfaces);
        for (band, _) in mapped.frame().shadow() {
            let loc = geometry.loc - frame.loc + band.loc;
            reach = reach.merge(Rectangle::new(loc, band.size));
        }
        Some(reach)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Logical> {
        Rectangle::new((x, y).into(), (w, h).into())
    }

    #[test]
    fn the_box_eases_out_from_the_old_frame_to_the_new() {
        let m = maximizing();

        assert_eq!(m.rect(Duration::from_secs(10)), m.from);
        assert_eq!(m.alpha(Duration::from_secs(10)), 1.0);
        // Half the time, seven eighths of the way.
        assert_eq!(
            m.rect(Duration::from_millis(10_125)),
            rect(12, 12, 750, 563)
        );
        assert_eq!(m.rect(Duration::from_millis(10_250)), m.to);
        assert!(m.is_over(Duration::from_millis(10_250)));
        assert!(!m.is_over(Duration::from_millis(10_249)));
    }

    fn maximizing() -> Maximizing {
        Maximizing {
            window: WindowId(1),
            from: rect(100, 100, 400, 300),
            from_done: 0.0,
            to: rect(0, 0, 800, 600),
            start: Duration::from_secs(10),
            picture_alpha: 1.0,
            picture: None,
            picture_frame: rect(100, 100, 400, 300),
        }
    }

    #[test]
    fn a_handover_goes_from_where_the_box_is_drawn_to_the_new_frame_for_whats_left() {
        let mut m = maximizing();
        let at = Duration::from_millis(10_125);
        let drawn = m.rect(at);
        let alpha = m.alpha(at);

        m.hand_over(rect(50, 50, 300, 200), at);

        assert_eq!(m.rect(at), drawn);
        assert_eq!(m.alpha(at), alpha, "the picture fades on as it did");
        assert_eq!(
            m.rect(Duration::from_millis(10_250)),
            rect(50, 50, 300, 200)
        );
        assert!(m.is_over(Duration::from_millis(10_250)));
    }
}
