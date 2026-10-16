//! The Popup slide (ADR 0037): an xdg_popup that's shown, of a window or of a Layer surface
//! and at any depth, slides out of the edge it's attached to over 200 ms of Caminito's clock,
//! and is cut off at its own final rectangle, so nothing of it is drawn over what it's
//! attached to. It's Plasma's Sliding popups effect (KWin's
//! `src/plugins/slidingpopups/slidingpopups.cpp`), with the edge chosen by Caminito, since
//! no MATE client names one.
//!
//! Policy knows nothing of it: a popup has its grab, the keyboard and the pointer at its
//! real place from its first frame, and only the drawing takes its time. The slide starts on
//! the popup's first commit with a buffer, and the scene draws it moved off its place and
//! faded as far as it has got, at every frame of its outputs, until it's over. It reads the
//! popup's place each frame, so a reposition part-way keeps the edge and the way done.
//!
//! Closing, and a slide that's interrupted, are the next slice's (CMN-179).

use std::cell::Cell;
use std::time::Duration;

use smithay::desktop::{PopupKind, PopupManager, find_popup_root_surface};
use smithay::output::Output;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, Rectangle};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::xdg::PopupSurface;

use caminito_theme::{Title, pango};

use crate::output::output_geometry;
use crate::state::Caminito;
use crate::workspace_slide::eased_way_left;

/// How long the slide lasts: the Sliding popups effect's.
pub(crate) const LENGTH: Duration = Duration::from_millis(200);

/// How many lines of the interface font a popup slides at most: the Sliding popups effect's.
const LINES: i32 = 8;

/// The edge of its final rectangle a popup comes out of, which is the one it's attached by:
/// a popup below its anchor comes out of its top edge, so it slides down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

impl Edge {
    /// Its name in the debug dump.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Edge::Top => "top",
            Edge::Bottom => "bottom",
            Edge::Left => "left",
            Edge::Right => "right",
        }
    }

    fn is_vertical(self) -> bool {
        matches!(self, Edge::Top | Edge::Bottom)
    }
}

/// The edge a popup placed at `popup` comes out of, with the rectangle it's anchored to at
/// `anchor`, both in its parent's coordinates: away from the anchor, down when it lies
/// below it, up when above it, and sideways when beside it. On a diagonal, the axis its
/// middle is farther along from the anchor's wins. One that overlaps its anchor on both
/// axes, as a combo box's list does its button, slides down.
fn edge(popup: Rectangle<i32, Logical>, anchor: Rectangle<i32, Logical>) -> Edge {
    let (popup_end, anchor_end) = (popup.loc + popup.size, anchor.loc + anchor.size);
    let vertical = if popup.loc.y >= anchor_end.y {
        Some(Edge::Top)
    } else if popup_end.y <= anchor.loc.y {
        Some(Edge::Bottom)
    } else {
        None
    };
    let horizontal = if popup.loc.x >= anchor_end.x {
        Some(Edge::Left)
    } else if popup_end.x <= anchor.loc.x {
        Some(Edge::Right)
    } else {
        None
    };
    match (vertical, horizontal) {
        (Some(vertical), Some(horizontal)) => {
            // Twice the middles, which are then whole.
            let twice_middle = |r: Rectangle<i32, Logical>| r.loc + r.loc + r.size.to_point();
            let apart = twice_middle(popup) - twice_middle(anchor);
            if apart.x.abs() > apart.y.abs() {
                horizontal
            } else {
                vertical
            }
        }
        (Some(edge), None) | (None, Some(edge)) => edge,
        (None, None) => Edge::Top,
    }
}

/// How far a popup of `size` along its slide slides: its size, or `LINES` lines of
/// `line_height`, whichever is less, and whether it fades as well, which it does only when
/// it slides less than its size.
fn distance(size: i32, line_height: i32) -> (i32, bool) {
    let lines = LINES * line_height;
    if lines < size {
        (lines, true)
    } else {
        (size, false)
    }
}

/// One popup on its way out of its edge.
#[derive(Debug)]
struct Slide {
    surface: WlSurface,
    motion: Motion,
}

/// How a popup slides out of its edge.
#[derive(Debug)]
struct Motion {
    edge: Edge,
    /// How far it slides, from its place.
    distance: i32,
    /// Whether it fades in as it slides.
    fades: bool,
    /// When it was shown, on Caminito's clock.
    start: Duration,
}

impl Motion {
    fn is_over(&self, now: Duration) -> bool {
        now.saturating_sub(self.start) >= LENGTH
    }

    /// How far it is on its way at `now`, from 0 to 1: the cubic ease-out, so it moves
    /// fastest at first and settles into its place.
    fn done(&self, now: Duration) -> f64 {
        1.0 - eased_way_left(now.saturating_sub(self.start), LENGTH)
    }

    /// Where it's drawn at `now`, from its place.
    fn offset(&self, now: Duration) -> Point<i32, Logical> {
        let left = (f64::from(self.distance) * (1.0 - self.done(now))).round() as i32;
        match self.edge {
            Edge::Top => (0, -left),
            Edge::Bottom => (0, left),
            Edge::Left => (-left, 0),
            Edge::Right => (left, 0),
        }
        .into()
    }

    /// How it's drawn at `now`.
    fn slid(&self, now: Duration) -> Slid {
        Slid {
            offset: self.offset(now),
            alpha: self.alpha(now),
        }
    }

    /// How opaque it's drawn at `now`.
    fn alpha(&self, now: Duration) -> f32 {
        if self.fades {
            self.done(now) as f32
        } else {
            1.0
        }
    }
}

/// How a popup on its way out of its edge is drawn at a time: moved off its place by `offset`, at `alpha`, and
/// cut off at its place.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Slid {
    pub offset: Point<i32, Logical>,
    pub alpha: f32,
}

/// A popup's slide as it's drawn, for the debug dump.
pub(crate) struct DrawnSlide {
    /// Its window geometry at its place, globally. It's cut off at its surfaces' place,
    /// which has its shadow too if its client draws one.
    pub rect: Rectangle<i32, Logical>,
    pub edge: Edge,
    pub slid: Slid,
}

/// The popups on their way out of their edges.
#[derive(Debug, Default)]
pub(crate) struct PopupSlides {
    running: Vec<Slide>,
}

impl PopupSlides {
    /// When the first of the slides is over: `deadlines` then draws its popup at rest (174).
    pub(crate) fn deadline(&self) -> Option<Duration> {
        self.running
            .iter()
            .map(|slide| slide.motion.start + LENGTH)
            .min()
    }

    /// How `surface`, a popup's, is drawn at `now`, while it slides.
    pub(crate) fn slid(&self, surface: &WlSurface, now: Duration) -> Option<Slid> {
        let slide = self.sliding(now).find(|slide| slide.surface == *surface)?;
        Some(slide.motion.slid(now))
    }

    fn sliding(&self, now: Duration) -> impl Iterator<Item = &Slide> {
        self.running
            .iter()
            .filter(move |slide| !slide.motion.is_over(now))
    }
}

/// Whether a popup has a buffer, as its last commit left it.
#[derive(Default)]
struct HasBuffer(Cell<bool>);

impl Caminito {
    /// Starts `popup`'s slide if this commit shows it, which is its first with a buffer.
    pub(crate) fn popup_committed(&mut self, popup: &PopupSurface, has_buffer: bool) {
        let surface = popup.wl_surface();
        let had_buffer = with_states(surface, |states| {
            states.data_map.insert_if_missing(HasBuffer::default);
            let data = states.data_map.get::<HasBuffer>().unwrap();
            data.0.replace(has_buffer)
        });
        if has_buffer && !had_buffer {
            self.start_popup_slide(popup);
        }
    }

    /// Starts `popup`'s slide, if it's seen to open: the slide is on, the session isn't
    /// locked, the Overview isn't open or on its way, and what the popup is attached to
    /// shows. A window on its way open has its popups inside its own zoom (251), which
    /// doesn't slide them as well.
    fn start_popup_slide(&mut self, popup: &PopupSurface) {
        let now = self.now();
        if !self.settings.popup_slide || self.popup_slides_stop() {
            return;
        }
        let Ok(root) = find_popup_root_surface(&PopupKind::Xdg(popup.clone())) else {
            return;
        };
        let shows = self.layer_for_surface(&root).is_some()
            || self.mapped_for_surface(&root).is_some_and(|id| {
                self.policy.is_showing(id) && self.open_close_animations.opening(id, now).is_none()
            });
        let placed = popup.with_committed_state(|state| {
            state.map(|state| (state.geometry, state.positioner.anchor_rect))
        });
        let Some((place, anchor)) = placed.filter(|_| shows) else {
            return;
        };
        let edge = edge(place, anchor);
        let size = if edge.is_vertical() {
            place.size.h
        } else {
            place.size.w
        };
        let (distance, fades) = distance(size, self.interface_line_height());
        self.popup_slides.running.push(Slide {
            surface: popup.wl_surface().clone(),
            motion: Motion {
                edge,
                distance,
                fades,
                start: now,
            },
        });
        self.queue_redraw_all();
    }

    /// Whether no popup slides now: the session is locked, or the Overview is open or on its
    /// way.
    fn popup_slides_stop(&self) -> bool {
        let overview =
            self.policy.overview().is_some() || self.overview_transition_deadline().is_some();
        self.lock.is_locked() || overview
    }

    /// The height of a line of the interface font, `org.mate.interface font-name`.
    fn interface_line_height(&self) -> i32 {
        let font = pango::FontDescription::from_string(&self.settings.interface.font_name);
        let context = self.frame_context.text_context();
        Title::new(context, &font, 1.0, "").text_height
    }

    /// Ends the slides of popups that are gone or have no buffer, and every slide while the
    /// session is locked or the Overview is open or on its way.
    pub(crate) fn refresh_popup_slides(&mut self) {
        let stops = self.popup_slides_stop();
        let slides = &mut self.popup_slides;
        let before = slides.running.len();
        slides.running.retain(|slide| {
            let shown = with_states(&slide.surface, |states| {
                states
                    .data_map
                    .get::<HasBuffer>()
                    .is_some_and(|data| data.0.get())
            });
            !stops && slide.surface.is_alive() && shown
        });
        if slides.running.len() != before {
            self.queue_redraw_all();
        }
    }

    /// Ends the slides that are over at `now`, and draws their popups at rest, for
    /// `deadlines`. While one runs, the outputs it's on are drawn again at each of their
    /// frames (`State::redraw_queued_outputs`), so there's no step to wait for.
    pub(crate) fn popup_slide_elapse(&mut self, now: Duration) {
        let slides = &mut self.popup_slides;
        let before = slides.running.len();
        slides.running.retain(|slide| !slide.motion.is_over(now));
        if slides.running.len() != before {
            self.queue_redraw_all();
        }
    }

    /// Whether a popup that slides at `now` is on `output`, which is then drawn again at its
    /// next frame.
    pub(crate) fn popup_slide_on(&self, output: &Output, now: Duration) -> bool {
        let area = output_geometry(output);
        self.popup_slides
            .sliding(now)
            .filter_map(|slide| self.popup_rect(&slide.surface))
            .any(|rect| rect.overlaps(area))
    }

    /// Each popup that slides at `now`, in the order they were shown, as it's drawn, for the
    /// debug dump.
    pub(crate) fn drawn_popup_slides(&self, now: Duration) -> Vec<DrawnSlide> {
        self.popup_slides
            .sliding(now)
            .filter_map(|slide| {
                Some(DrawnSlide {
                    rect: self.popup_rect(&slide.surface)?,
                    edge: slide.motion.edge,
                    slid: slide.motion.slid(now),
                })
            })
            .collect()
    }

    /// The window geometry of the popup whose surface is `surface`, at its place, globally:
    /// from its root's, a window's window geometry or a Layer surface's corner.
    fn popup_rect(&self, surface: &WlSurface) -> Option<Rectangle<i32, Logical>> {
        let PopupKind::Xdg(popup) = self.popups.find_popup(surface)? else {
            return None;
        };
        let size = popup.with_committed_state(|state| state.map(|state| state.geometry.size))?;
        let root = find_popup_root_surface(&PopupKind::Xdg(popup)).ok()?;
        let origin = if let Some((output, layer)) = self.layer_for_surface(&root) {
            let map = smithay::desktop::layer_map_for_output(&output);
            map.layer_geometry(&layer)?.loc + output_geometry(&output).loc
        } else {
            let id = self.mapped_for_surface(&root)?;
            self.policy.geometry(id)?.loc
        };
        let (_, offset) = PopupManager::popups_for_surface(&root)
            .find(|(kind, _)| kind.wl_surface() == surface)?;
        Some(Rectangle::new(origin + offset, size))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Logical> {
        Rectangle::new((x, y).into(), (w, h).into())
    }

    #[test]
    fn a_popup_comes_out_of_the_edge_that_faces_its_anchor() {
        let anchor = rect(100, 100, 50, 20);

        assert_eq!(edge(rect(100, 120, 200, 300), anchor), Edge::Top, "below");
        assert_eq!(
            edge(rect(100, -200, 200, 300), anchor),
            Edge::Bottom,
            "above"
        );
        assert_eq!(edge(rect(150, 50, 200, 300), anchor), Edge::Left, "right");
        assert_eq!(edge(rect(-100, 50, 200, 300), anchor), Edge::Right, "left");
    }

    #[test]
    fn a_popup_that_overlaps_its_anchor_slides_down() {
        let button = rect(100, 100, 120, 30);

        assert_eq!(edge(rect(100, 60, 120, 200), button), Edge::Top);
    }

    #[test]
    fn on_a_diagonal_the_axis_its_middle_is_farther_along_wins() {
        let anchor = rect(100, 100, 20, 20);

        // A tall popup below and to the right, whose middle is farther down than across.
        assert_eq!(edge(rect(120, 120, 100, 400), anchor), Edge::Top);
        // A wide one, whose middle is farther across.
        assert_eq!(edge(rect(120, 120, 400, 100), anchor), Edge::Left);
        // Above and to the left.
        assert_eq!(edge(rect(-300, 0, 400, 100), anchor), Edge::Right);
    }

    #[test]
    fn a_popup_slides_its_size_or_eight_lines_and_fades_only_when_that_is_less() {
        assert_eq!(distance(100, 17), (100, false));
        assert_eq!(distance(136, 17), (136, false));
        assert_eq!(distance(300, 17), (136, true));
    }

    #[test]
    fn a_slide_eases_out_of_its_edge_into_its_place() {
        let start = Duration::from_secs(10);
        let slide = |edge, fades| Motion {
            edge,
            distance: 80,
            fades,
            start,
        };

        let down = slide(Edge::Top, false);
        assert_eq!(down.offset(start), (0, -80).into());
        // A cubic ease-out is seven eighths of the way at half time.
        assert_eq!(down.offset(start + LENGTH / 2), (0, -10).into());
        assert_eq!(down.offset(start + LENGTH), (0, 0).into());
        assert_eq!(down.alpha(start), 1.0);
        assert!(down.is_over(start + LENGTH));

        assert_eq!(slide(Edge::Bottom, false).offset(start), (0, 80).into());
        assert_eq!(slide(Edge::Left, false).offset(start), (-80, 0).into());
        assert_eq!(slide(Edge::Right, false).offset(start), (80, 0).into());

        let fading = slide(Edge::Top, true);
        assert_eq!(fading.alpha(start), 0.0);
        assert_eq!(fading.alpha(start + LENGTH / 2), 0.875);
        assert_eq!(fading.alpha(start + LENGTH), 1.0);
    }
}
