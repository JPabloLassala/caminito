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
//! A popup that closes while it's on screen, destroyed or unmapped by its client or
//! dismissed by Caminito (`popup_done`, a click outside), slides back into its edge over as
//! long, on the same curve played backwards, drawn from a picture of its last frame taken
//! while it still has its buffer, as the Closing animation keeps a window's (252). Policy
//! lets it go at once: its grab has ended and a click goes through its picture. A chain
//! dismissed together slides out together, each into its own edge. A popup with no buffer
//! left goes at once, and one whose window or Layer surface closes leaves with it: inside
//! the window's Closing picture, or at once.
//!
//! A popup that closes on its way out turns round from where it is, at the same speed, as
//! Plasma's does: the curve played backwards is at the same point after what's left of it.
//! An xdg_popup can't be shown again once it's closed, as Smithay forgets an unmapped one's
//! role, so only the way back turns round here.
//!
//! The Window menu and its submenu slide too, on the same motion (`window_menu::slide`).

use std::cell::Cell;
use std::time::Duration;

use caminito_policy::OutputId;
use smithay::desktop::{PopupKind, PopupManager, find_popup_root_surface};
use smithay::output::Output;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, Rectangle, Size};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::xdg::PopupSurface;

use caminito_theme::{Title, pango};

use crate::open_close_animation::Picture;
use crate::output::{output_geometry, output_id};
use crate::state::{Caminito, State};
use crate::window_menu::slide::MenuSlides;
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

    pub(crate) fn is_vertical(self) -> bool {
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

/// How a popup slides out of its edge, or back into it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Motion {
    edge: Edge,
    /// How far it slides, from its place.
    distance: i32,
    /// Whether it fades in as it slides.
    fades: bool,
    /// Whether it slides back into its edge, as it closes.
    closing: bool,
    /// When it was shown, or closed, on Caminito's clock.
    start: Duration,
    /// How much of the curve it had run by then: one that turned round starts where the
    /// other had got to.
    skipped: Duration,
}

impl Motion {
    fn elapsed(&self, now: Duration) -> Duration {
        now.saturating_sub(self.start) + self.skipped
    }

    pub(crate) fn is_over(&self, now: Duration) -> bool {
        self.elapsed(now) >= LENGTH
    }

    /// When it's over, on Caminito's clock.
    pub(crate) fn end(&self) -> Duration {
        self.start + LENGTH.saturating_sub(self.skipped)
    }

    pub(crate) fn edge(&self) -> Edge {
        self.edge
    }

    /// How far it is from its edge at `now`, from 0 to 1, where 1 is its place: the cubic
    /// ease-out on its way out, so it moves fastest at first and settles into its place,
    /// and the same curve played backwards on its way back.
    fn done(&self, now: Duration) -> f64 {
        let elapsed = self.elapsed(now).min(LENGTH);
        let along = if self.closing {
            LENGTH - elapsed
        } else {
            elapsed
        };
        1.0 - eased_way_left(along, LENGTH)
    }

    /// The motion that turns this one round at `now`, from where it is and at the same
    /// speed: the curve played the other way is at the same point after what's left of
    /// this one.
    pub(crate) fn turned(&self, now: Duration) -> Motion {
        Motion {
            closing: !self.closing,
            start: now,
            skipped: LENGTH.saturating_sub(self.elapsed(now)),
            ..*self
        }
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
    pub(crate) fn slid(&self, now: Duration) -> Slid {
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

/// A popup on screen, as it was last seen: what's needed to slide it back into its edge once
/// it's gone from the popups' trees.
#[derive(Debug)]
struct Shown {
    surface: WlSurface,
    /// The window's or the Layer surface's surface its chain is attached to.
    root: WlSurface,
    /// Its window geometry, from its root's origin: a window's geometry's corner or a Layer
    /// surface's.
    place: Rectangle<i32, Logical>,
    /// Where its window geometry is in its surface.
    geometry: Point<i32, Logical>,
    edge: Edge,
}

/// A closed popup's picture on its way back into its edge.
#[derive(Debug)]
pub(crate) struct ClosingPopup {
    /// The surface of its chain's window or Layer surface, which it's drawn on.
    pub root: WlSurface,
    /// Its window geometry, from its root's origin.
    place: Rectangle<i32, Logical>,
    /// Where its window geometry was in its surface.
    geometry: Point<i32, Logical>,
    /// Its last frame, from its surface's origin.
    pub picture: Picture,
    motion: Motion,
    /// The output it was taken on, which takes it away when it goes.
    output: Option<OutputId>,
}

impl ClosingPopup {
    /// What its picture covers at its place, from its root's origin: it's cut off there.
    pub(crate) fn covered(&self) -> Rectangle<i32, Logical> {
        let area = self.picture.area;
        Rectangle::new(self.place.loc - self.geometry + area.loc, area.size)
    }

    /// How it's drawn at `now`.
    pub(crate) fn slid(&self, now: Duration) -> Slid {
        self.motion.slid(now)
    }
}

/// The popups on their way out of their edges, the pictures of those on their way back, and
/// those on screen, and the Window menu's.
#[derive(Debug, Default)]
pub(crate) struct PopupSlides {
    running: Vec<Slide>,
    closing: Vec<ClosingPopup>,
    shown: Vec<Shown>,
    pub(crate) window_menu: MenuSlides,
}

impl PopupSlides {
    /// When the first of the slides is over: `deadlines` then draws its popup at rest, or
    /// takes its picture away (174).
    pub(crate) fn deadline(&self) -> Option<Duration> {
        let opening = self.running.iter().map(|slide| slide.motion.end());
        let closing = self.closing.iter().map(|closing| closing.motion.end());
        let menu = self.window_menu.deadline();
        opening.chain(closing).chain(menu).min()
    }

    /// The pictures of the popups of `root`'s chains on their way back at `now`, those
    /// shown last first, which are drawn over the others.
    pub(crate) fn closing_on<'a>(
        &'a self,
        root: &'a WlSurface,
        now: Duration,
    ) -> impl Iterator<Item = &'a ClosingPopup> {
        self.closing
            .iter()
            .rev()
            .filter(move |closing| closing.root == *root && !closing.motion.is_over(now))
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
    /// Notes where `popup` is after a commit, and starts its slide if this commit shows it,
    /// which is its first with a buffer.
    pub(crate) fn popup_committed(&mut self, popup: &PopupSurface, has_buffer: bool) {
        let surface = popup.wl_surface();
        let had_buffer = with_states(surface, |states| {
            states.data_map.insert_if_missing(HasBuffer::default);
            let data = states.data_map.get::<HasBuffer>().unwrap();
            data.0.replace(has_buffer)
        });
        if !has_buffer {
            return;
        }
        self.note_shown(popup);
        if !had_buffer {
            self.start_popup_slide(popup);
        }
    }

    /// Notes `popup` as on screen, with its place and its edge as its last commit left
    /// them.
    fn note_shown(&mut self, popup: &PopupSurface) {
        let surface = popup.wl_surface();
        let Ok(root) = find_popup_root_surface(&PopupKind::Xdg(popup.clone())) else {
            return;
        };
        let placed = popup.with_committed_state(|state| {
            state.map(|state| (state.geometry, state.positioner.anchor_rect))
        });
        let Some((place, anchor)) = placed else {
            return;
        };
        let geometry = PopupKind::Xdg(popup.clone()).geometry().loc;
        let at = popup_offset(&root, surface).unwrap_or(place.loc);
        let shown = Shown {
            surface: surface.clone(),
            root,
            place: Rectangle::new(at, place.size),
            geometry,
            edge: edge(place, anchor),
        };
        let slides = &mut self.popup_slides;
        match slides.shown.iter_mut().find(|s| s.surface == *surface) {
            Some(was) => *was = shown,
            None => slides.shown.push(shown),
        }
    }

    /// Starts `popup`'s slide, if it's seen to open: the slide is on, the session isn't
    /// locked, the Overview isn't open or on its way, and what the popup is attached to
    /// shows. A window on its way open has its popups inside its own zoom (251), which
    /// doesn't slide them as well.
    fn start_popup_slide(&mut self, popup: &PopupSurface) {
        let surface = popup.wl_surface();
        let Some(shown) = self
            .popup_slides
            .shown
            .iter()
            .find(|s| s.surface == *surface)
        else {
            return;
        };
        if !self.popup_slides_seen(&shown.root) {
            return;
        }
        let motion = self.motion(shown.edge, shown.place.size, false);
        self.popup_slides.running.push(Slide {
            surface: surface.clone(),
            motion,
        });
        self.queue_redraw_all();
    }

    /// A popup's motion from now with its edge and its `size`, out of its edge or, when
    /// `closing`, back into it.
    pub(crate) fn motion(&self, edge: Edge, size: Size<i32, Logical>, closing: bool) -> Motion {
        let size = if edge.is_vertical() { size.h } else { size.w };
        let (distance, fades) = distance(size, self.interface_line_height());
        Motion {
            edge,
            distance,
            fades,
            closing,
            start: self.now(),
            skipped: Duration::ZERO,
        }
    }

    /// Whether a popup of the chain attached to `root` is seen to open or close now: the
    /// slide is on, no slide stops, and `root` shows, which a window on its way open
    /// doesn't for this (251).
    fn popup_slides_seen(&self, root: &WlSurface) -> bool {
        let now = self.now();
        let shows = self.layer_for_surface(root).is_some()
            || self.mapped_for_surface(root).is_some_and(|id| {
                self.policy.is_showing(id) && self.open_close_animations.opening(id, now).is_none()
            });
        self.popup_slides_run() && shows
    }

    /// Whether a popup that opens or closes now slides: the slide is on and no slide stops.
    pub(crate) fn popup_slides_run(&self) -> bool {
        self.settings.popup_slide && !self.popup_slides_stop()
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

    /// The origin of `root`, globally, which its popups' places are from: a window's
    /// geometry's corner or a Layer surface's.
    fn popup_root_origin(&self, root: &WlSurface) -> Option<Point<i32, Logical>> {
        if let Some((output, layer)) = self.layer_for_surface(root) {
            let map = smithay::desktop::layer_map_for_output(&output);
            return Some(map.layer_geometry(&layer)?.loc + output_geometry(&output).loc);
        }
        let id = self.mapped_for_surface(root)?;
        self.policy.geometry(id).map(|geometry| geometry.loc)
    }

    /// Ends the slides of popups that are gone or have no buffer, and every slide, out or
    /// back, while the session is locked or the Overview is open or on its way. A picture
    /// whose window or Layer surface is gone goes with it, as does one whose output went.
    fn refresh_running_popup_slides(&mut self) {
        let stops = self.popup_slides_stop();
        let roots: Vec<bool> = self
            .popup_slides
            .closing
            .iter()
            .map(|closing| self.popup_root_origin(&closing.root).is_some())
            .collect();
        let outputs: Vec<OutputId> = self.outputs.iter().map(output_id).collect();
        let slides = &mut self.popup_slides;
        let menu_changed = slides.window_menu.refresh(stops, &self.outputs);
        let before = slides.running.len() + slides.closing.len();
        slides.running.retain(|slide| {
            let shown = with_states(&slide.surface, |states| {
                states
                    .data_map
                    .get::<HasBuffer>()
                    .is_some_and(|data| data.0.get())
            });
            !stops && slide.surface.is_alive() && shown
        });
        let mut roots = roots.into_iter();
        slides.closing.retain(|closing| {
            let root = roots.next().unwrap_or(false);
            let output = closing.output.is_none_or(|id| outputs.contains(&id));
            !stops && root && output
        });
        if slides.running.len() + slides.closing.len() != before || menu_changed {
            self.queue_redraw_all();
        }
    }

    /// Ends the slides that are over at `now`, and draws their popups at rest or takes their
    /// pictures away, for `deadlines`. While one runs, the outputs it's on are drawn again
    /// at each of their frames (`State::redraw_queued_outputs`), so there's no step to wait
    /// for.
    pub(crate) fn popup_slide_elapse(&mut self, now: Duration) {
        let slides = &mut self.popup_slides;
        let before = slides.running.len() + slides.closing.len();
        slides.running.retain(|slide| !slide.motion.is_over(now));
        slides
            .closing
            .retain(|closing| !closing.motion.is_over(now));
        let menu_changed = slides.window_menu.elapse(now);
        if slides.running.len() + slides.closing.len() != before || menu_changed {
            self.queue_redraw_all();
        }
    }

    /// Whether a popup that slides, out or back, at `now` is on `output`, which is then
    /// drawn again at its next frame.
    pub(crate) fn popup_slide_on(&self, output: &Output, now: Duration) -> bool {
        let area = output_geometry(output);
        let opening = self.drawn_popup_slides(now).into_iter();
        let closing = self.drawn_closing_popups(now).into_iter();
        opening
            .chain(closing)
            .any(|drawn| drawn.rect.overlaps(area))
    }

    /// Each popup that slides at `now`, in the order they were shown, as it's drawn, for the
    /// debug dump: the Window menu's after the others.
    pub(crate) fn drawn_popup_slides(&self, now: Duration) -> Vec<DrawnSlide> {
        let popups = self.popup_slides.sliding(now).filter_map(|slide| {
            Some(DrawnSlide {
                rect: self.popup_rect(&slide.surface)?,
                edge: slide.motion.edge,
                slid: slide.motion.slid(now),
            })
        });
        let menu = self.drawn_window_menu_slides(now).into_iter();
        let menu = menu.filter(|(closing, _)| !closing).map(|(_, drawn)| drawn);
        popups.chain(menu).collect()
    }

    /// Each closed popup's picture that slides back at `now`, in the order they closed, as
    /// it's drawn, for the debug dump: the Window menu's after the others.
    pub(crate) fn drawn_closing_popups(&self, now: Duration) -> Vec<DrawnSlide> {
        let menu = self.drawn_window_menu_slides(now).into_iter();
        let menu = menu.filter(|(closing, _)| *closing).map(|(_, drawn)| drawn);
        self.popup_slides
            .closing
            .iter()
            .filter(|closing| !closing.motion.is_over(now))
            .filter_map(|closing| {
                let origin = self.popup_root_origin(&closing.root)?;
                let place = closing.place;
                Some(DrawnSlide {
                    rect: Rectangle::new(origin + place.loc, place.size),
                    edge: closing.motion.edge,
                    slid: closing.motion.slid(now),
                })
            })
            .chain(menu)
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
        let origin = self.popup_root_origin(&root)?;
        Some(Rectangle::new(origin + popup_offset(&root, surface)?, size))
    }
}

/// Where the window geometry of the popup whose surface is `surface` is from its chain's
/// root's origin, while it's in the chain.
fn popup_offset(root: &WlSurface, surface: &WlSurface) -> Option<Point<i32, Logical>> {
    let (_, offset) =
        PopupManager::popups_for_surface(root).find(|(kind, _)| kind.wl_surface() == surface)?;
    Some(offset)
}

impl State {
    /// Starts the slide back of the popup whose surface is `surface`, if it was on screen,
    /// from a picture of it taken now, while it still has its buffer: its client destroys
    /// or unmaps it, or Caminito dismissed it. One closed on its way out turns round. With
    /// no buffer left, it goes at once.
    pub(crate) fn popup_closes(&mut self, surface: &WlSurface) {
        let caminito = &mut self.caminito;
        let now = caminito.now();
        let slides = &mut caminito.popup_slides;
        let Some(index) = slides.shown.iter().position(|s| s.surface == *surface) else {
            return;
        };
        let shown = slides.shown.remove(index);
        let opening = slides
            .running
            .iter()
            .position(|slide| slide.surface == *surface)
            .map(|index| slides.running.remove(index))
            .filter(|slide| !slide.motion.is_over(now));
        if !caminito.popup_slides_seen(&shown.root) {
            return;
        }
        let Some(origin) = caminito.popup_root_origin(&shown.root) else {
            return;
        };
        let rect = Rectangle::new(origin + shown.place.loc, shown.place.size);
        let output = caminito
            .outputs
            .iter()
            .find(|output| output_geometry(output).overlaps(rect))
            .cloned();
        let scale = output
            .as_ref()
            .map_or(1.0, |output| output.current_scale().fractional_scale());
        let Some(picture) = self.backend.take_popup_picture(surface, scale) else {
            tracing::debug!("no picture of a closing popup: it goes at once");
            return;
        };
        let caminito = &mut self.caminito;
        let motion = match opening {
            Some(opening) => opening.motion.turned(now),
            None => caminito.motion(shown.edge, shown.place.size, true),
        };
        caminito.popup_slides.closing.push(ClosingPopup {
            root: shown.root,
            place: shown.place,
            geometry: shown.geometry,
            picture,
            motion,
            output: output.as_ref().map(output_id),
        });
        caminito.queue_redraw_all();
    }

    /// Notes where each popup on screen now is, and starts the slide back of each one that's
    /// left its chain while it still has its buffer, which Caminito dismissed. One whose
    /// window or Layer surface is gone leaves with it. Then ends the slides that stop.
    pub(crate) fn refresh_popup_slides(&mut self) {
        let mut dismissed = Vec::new();
        self.caminito.popup_slides.shown.retain_mut(|shown| {
            if !shown.surface.is_alive() || !shown.root.is_alive() {
                return false;
            }
            match popup_offset(&shown.root, &shown.surface) {
                Some(offset) => {
                    shown.place.loc = offset;
                    true
                }
                None => {
                    dismissed.push(shown.surface.clone());
                    true
                }
            }
        });
        for surface in dismissed {
            self.popup_closes(&surface);
        }
        self.caminito.refresh_running_popup_slides();
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
    fn a_slide_turned_round_goes_back_from_where_it_is_at_the_same_speed() {
        let start = Duration::from_secs(10);
        let out = Motion {
            edge: Edge::Top,
            distance: 80,
            fades: true,
            closing: false,
            start,
            skipped: Duration::ZERO,
        };
        let part_way = start + Duration::from_millis(50);

        let back = out.turned(part_way);
        assert_eq!(back.offset(part_way), out.offset(part_way), "no jump");
        assert_eq!(back.alpha(part_way), out.alpha(part_way));
        // As long back to its edge as it took to get there.
        assert!(!back.is_over(part_way + Duration::from_millis(49)));
        assert!(back.is_over(part_way + Duration::from_millis(50)));
        assert_eq!(
            back.offset(part_way + Duration::from_millis(50)),
            (0, -80).into()
        );

        // And round again.
        let later = part_way + Duration::from_millis(20);
        let again = back.turned(later);
        assert_eq!(again.offset(later), back.offset(later));
        assert!(!again.closing);
    }

    #[test]
    fn a_slide_back_is_the_slide_out_played_backwards() {
        let start = Duration::from_secs(10);
        let motion = |closing| Motion {
            edge: Edge::Left,
            distance: 80,
            fades: false,
            closing,
            start,
            skipped: Duration::ZERO,
        };
        let (out, back) = (motion(false), motion(true));

        assert_eq!(back.offset(start), (0, 0).into(), "from its place");
        for ms in [10, 50, 100, 150] {
            let at = Duration::from_millis(ms);
            assert_eq!(back.offset(start + at), out.offset(start + LENGTH - at));
        }
        assert_eq!(
            back.offset(start + LENGTH),
            (-80, 0).into(),
            "into its edge"
        );
    }

    #[test]
    fn a_slide_eases_out_of_its_edge_into_its_place() {
        let start = Duration::from_secs(10);
        let slide = |edge, fades| Motion {
            edge,
            distance: 80,
            fades,
            closing: false,
            start,
            skipped: Duration::ZERO,
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
