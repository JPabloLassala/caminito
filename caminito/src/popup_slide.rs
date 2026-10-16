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
//! role, so only the way back turns round for one.
//!
//! An override-redirect window, an X11 menu or tooltip, slides as well, but for one of type
//! Utility, as Plasma leaves out, or DND, a drag icon that follows the pointer. It has no
//! anchor, so it slides away from where the pointer was when it mapped. It opens at its map,
//! from its first frame with a buffer, and closes at its unmap or its destruction, from a picture taken while its surface still
//! has its buffer, which Xwayland may destroy before the unmap reaches Caminito. Its place
//! is global, on no window, and one mapped again while its picture slides back turns round.

use std::cell::Cell;
use std::time::Duration;

use caminito_policy::OutputId;
use smithay::backend::renderer::utils::with_renderer_surface_state;
use smithay::desktop::{PopupKind, PopupManager, find_popup_root_surface};
use smithay::output::Output;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, Rectangle, Size};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::xdg::PopupSurface;
use smithay::xwayland::X11Surface;
use smithay::xwayland::xwm::{WmWindowType, X11Window};

use caminito_theme::{Title, pango};

use crate::open_close_animation::Picture;
use crate::output::{output_geometry, output_id};
use crate::scene::override_redirect_bbox;
use crate::state::{Caminito, State};
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

/// The edge an override-redirect window at `window` comes out of, with the pointer at
/// `pointer` when it mapped, both global: away from the pointer, down when it's above the
/// window's middle row and up when it's below it, and sideways when it's beside the window
/// and neither above nor below it.
fn pointer_edge(window: Rectangle<i32, Logical>, pointer: Point<f64, Logical>) -> Edge {
    let window = window.to_f64();
    let end = window.loc + window.size.to_point();
    let between_rows = window.loc.y <= pointer.y && pointer.y < end.y;
    if between_rows && pointer.x < window.loc.x {
        Edge::Left
    } else if between_rows && pointer.x >= end.x {
        Edge::Right
    } else if pointer.y <= window.loc.y + window.size.h / 2.0 {
        Edge::Top
    } else {
        Edge::Bottom
    }
}

/// Whether an override-redirect window of `kind` slides: all but a Utility one, as Plasma
/// leaves out (`isPopupWindow`), and a DND one, a drag icon that follows the pointer.
fn slides(kind: Option<WmWindowType>) -> bool {
    !matches!(kind, Some(WmWindowType::Utility | WmWindowType::Dnd))
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
    popup: Popup,
    motion: Motion,
}

/// A popup that slides out of its edge.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Popup {
    /// An xdg_popup, by its surface.
    Xdg(WlSurface),
    /// An override-redirect window, by its X id: it may have no surface yet as it maps.
    OverrideRedirect(X11Window),
}

/// What a closed popup's picture is drawn on, which its place is from.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Root {
    /// The surface of an xdg_popup's chain's window or Layer surface.
    Surface(WlSurface),
    /// Nothing, for an override-redirect window, by its X id: its place is global. A
    /// window mapped again while its picture slides back takes the picture's place.
    Global(X11Window),
}

/// How a popup slides out of its edge, or back into it.
#[derive(Debug, Clone, Copy)]
struct Motion {
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

    fn is_over(&self, now: Duration) -> bool {
        self.elapsed(now) >= LENGTH
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
    fn turned(&self, now: Duration) -> Motion {
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
    /// What it's drawn on.
    pub root: Root,
    /// Its window geometry, from its root's origin: an override-redirect window's surface,
    /// globally.
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

/// An override-redirect window on screen that slides, with the edge it comes out of.
#[derive(Debug)]
struct ShownOverrideRedirect {
    window: X11Window,
    edge: Edge,
    /// Whether its slide waits for its first frame with a buffer, which shows it.
    waiting: bool,
}

/// What a popup that closes slides back from: its root, its place from its root's origin,
/// where its window geometry is in its surface, its edge, and its slide out if it was
/// still on its way.
struct SlideBack {
    root: Root,
    place: Rectangle<i32, Logical>,
    geometry: Point<i32, Logical>,
    edge: Edge,
    opening: Option<Slide>,
}

/// The popups on their way out of their edges, the pictures of those on their way back, and
/// those on screen.
#[derive(Debug, Default)]
pub(crate) struct PopupSlides {
    running: Vec<Slide>,
    closing: Vec<ClosingPopup>,
    shown: Vec<Shown>,
    override_redirect: Vec<ShownOverrideRedirect>,
}

impl PopupSlides {
    /// When the first of the slides is over: `deadlines` then draws its popup at rest, or
    /// takes its picture away (174).
    pub(crate) fn deadline(&self) -> Option<Duration> {
        let end = |motion: &Motion| motion.start + LENGTH.saturating_sub(motion.skipped);
        let opening = self.running.iter().map(|slide| end(&slide.motion));
        let closing = self.closing.iter().map(|closing| end(&closing.motion));
        opening.chain(closing).min()
    }

    /// The pictures of the popups of `root`'s chains on their way back at `now`, those
    /// shown last first, which are drawn over the others.
    pub(crate) fn closing_on<'a>(
        &'a self,
        root: &'a WlSurface,
        now: Duration,
    ) -> impl Iterator<Item = &'a ClosingPopup> {
        self.closing.iter().rev().filter(move |closing| {
            matches!(&closing.root, Root::Surface(surface) if surface == root)
                && !closing.motion.is_over(now)
        })
    }

    /// The pictures of the override-redirect windows on their way back at `now`, those
    /// closed last first, which are drawn over the others.
    pub(crate) fn closing_override_redirect(
        &self,
        now: Duration,
    ) -> impl Iterator<Item = &ClosingPopup> {
        self.closing.iter().rev().filter(move |closing| {
            matches!(closing.root, Root::Global(_)) && !closing.motion.is_over(now)
        })
    }

    /// How `popup` is drawn at `now`, while it slides.
    pub(crate) fn slid(&self, popup: &Popup, now: Duration) -> Option<Slid> {
        let slide = self.sliding(now).find(|slide| slide.popup == *popup)?;
        Some(slide.motion.slid(now))
    }

    /// Takes `popup`'s slide out, which ends, and returns it if it's still on its way at
    /// `now`.
    fn take_opening(&mut self, popup: &Popup, now: Duration) -> Option<Slide> {
        let index = self
            .running
            .iter()
            .position(|slide| slide.popup == *popup)?;
        Some(self.running.remove(index)).filter(|slide| !slide.motion.is_over(now))
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
            popup: Popup::Xdg(surface.clone()),
            motion,
        });
        self.queue_redraw_all();
    }

    /// A popup's motion from now with its edge and its `size`, out of its edge or, when
    /// `closing`, back into it.
    fn motion(&self, edge: Edge, size: Size<i32, Logical>, closing: bool) -> Motion {
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

    /// Whether popups slide now: the slide is on and no slide stops. That's all an
    /// override-redirect window needs to be seen to open or close, as it's on no workspace.
    fn popup_slides_run(&self) -> bool {
        self.settings.popup_slide && !self.popup_slides_stop()
    }

    /// Notes override-redirect `window`, which X has just mapped, with the edge away from
    /// where the pointer is now, unless it's of a type that doesn't slide. Its slide out
    /// starts with its first frame with a buffer, now if it has one. One whose picture
    /// slides back, unmapped part-way, turns round from where it is at once: its picture
    /// was on screen.
    pub(crate) fn override_redirect_shown(&mut self, window: &X11Surface) {
        if !slides(window.window_type()) {
            return;
        }
        let id = window.window_id();
        let pointer = self.seat.get_pointer().unwrap().current_location();
        let now = self.now();
        let slides = &mut self.popup_slides;
        let picture = slides
            .closing
            .iter()
            .position(|closing| closing.root == Root::Global(id))
            .map(|index| slides.closing.remove(index))
            .filter(|closing| !closing.motion.is_over(now));
        slides
            .running
            .retain(|slide| slide.popup != Popup::OverrideRedirect(id));
        slides.override_redirect.retain(|shown| shown.window != id);
        let Some(picture) = picture else {
            slides.override_redirect.push(ShownOverrideRedirect {
                window: id,
                edge: pointer_edge(window.last_configure(), pointer),
                waiting: true,
            });
            if has_buffer(window) {
                self.override_redirect_committed(window);
            }
            return;
        };
        let motion = picture.motion.turned(now);
        slides.override_redirect.push(ShownOverrideRedirect {
            window: id,
            edge: motion.edge,
            waiting: false,
        });
        if self.popup_slides_run() {
            self.popup_slides.running.push(Slide {
                popup: Popup::OverrideRedirect(id),
                motion,
            });
        }
        self.queue_redraw_all();
    }

    /// Starts override-redirect `window`'s slide out of its edge if this commit, which has
    /// a buffer, is its first since it mapped: it's shown from now.
    pub(crate) fn override_redirect_committed(&mut self, window: &X11Surface) {
        let id = window.window_id();
        let shown = self
            .popup_slides
            .override_redirect
            .iter_mut()
            .find(|shown| shown.window == id && shown.waiting);
        let Some(shown) = shown else {
            return;
        };
        shown.waiting = false;
        let edge = shown.edge;
        if !self.popup_slides_run() {
            return;
        }
        let motion = self.motion(edge, override_redirect_bbox(window).size, false);
        self.popup_slides.running.push(Slide {
            popup: Popup::OverrideRedirect(id),
            motion,
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
            .map(|closing| self.closing_origin(&closing.root).is_some())
            .collect();
        let outputs: Vec<OutputId> = self.outputs.iter().map(output_id).collect();
        let override_redirect: Vec<X11Window> = self
            .override_redirect_windows()
            .map(X11Surface::window_id)
            .collect();
        let slides = &mut self.popup_slides;
        let before = slides.running.len() + slides.closing.len();
        slides.running.retain(|slide| {
            let shown = match &slide.popup {
                Popup::Xdg(surface) => {
                    let has_buffer = with_states(surface, |states| {
                        states
                            .data_map
                            .get::<HasBuffer>()
                            .is_some_and(|data| data.0.get())
                    });
                    surface.is_alive() && has_buffer
                }
                Popup::OverrideRedirect(window) => override_redirect.contains(window),
            };
            !stops && shown
        });
        let mut roots = roots.into_iter();
        slides.closing.retain(|closing| {
            let root = roots.next().unwrap_or(false);
            let output = closing.output.is_none_or(|id| outputs.contains(&id));
            !stops && root && output
        });
        if slides.running.len() + slides.closing.len() != before {
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
        if slides.running.len() + slides.closing.len() != before {
            self.queue_redraw_all();
        }
    }

    /// Where a closed popup's picture on `root` has its place from, globally: its window's
    /// or Layer surface's origin, or the global origin for an override-redirect window.
    fn closing_origin(&self, root: &Root) -> Option<Point<i32, Logical>> {
        match root {
            Root::Surface(root) => self.popup_root_origin(root),
            Root::Global(_) => Some(Point::default()),
        }
    }

    /// Whether a popup that slides, out or back, at `now` is on `output`, which is then
    /// drawn again at its next frame.
    pub(crate) fn popup_slide_on(&self, output: &Output, now: Duration) -> bool {
        let area = output_geometry(output);
        let opening = self
            .popup_slides
            .sliding(now)
            .filter_map(|slide| self.sliding_rect(&slide.popup));
        let closing = self
            .drawn_closing_popups(now)
            .into_iter()
            .map(|drawn| drawn.rect);
        opening.chain(closing).any(|rect| rect.overlaps(area))
    }

    /// Each popup that slides at `now`, in the order they were shown, as it's drawn, for the
    /// debug dump.
    pub(crate) fn drawn_popup_slides(&self, now: Duration) -> Vec<DrawnSlide> {
        self.popup_slides
            .sliding(now)
            .filter_map(|slide| {
                Some(DrawnSlide {
                    rect: self.sliding_rect(&slide.popup)?,
                    edge: slide.motion.edge,
                    slid: slide.motion.slid(now),
                })
            })
            .collect()
    }

    /// Each closed popup's picture that slides back at `now`, in the order they closed, as
    /// it's drawn, for the debug dump.
    pub(crate) fn drawn_closing_popups(&self, now: Duration) -> Vec<DrawnSlide> {
        self.popup_slides
            .closing
            .iter()
            .filter(|closing| !closing.motion.is_over(now))
            .filter_map(|closing| {
                let origin = self.closing_origin(&closing.root)?;
                let place = closing.place;
                Some(DrawnSlide {
                    rect: Rectangle::new(origin + place.loc, place.size),
                    edge: closing.motion.edge,
                    slid: closing.motion.slid(now),
                })
            })
            .collect()
    }

    /// Where `popup` is at its place, globally: an xdg_popup's window geometry or an
    /// override-redirect window's surface.
    fn sliding_rect(&self, popup: &Popup) -> Option<Rectangle<i32, Logical>> {
        match popup {
            Popup::Xdg(surface) => self.popup_rect(surface),
            Popup::OverrideRedirect(id) => self
                .override_redirect_windows()
                .find(|window| window.window_id() == *id)
                .map(override_redirect_bbox),
        }
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

/// Whether override-redirect `window`'s surface has a buffer.
fn has_buffer(window: &X11Surface) -> bool {
    window.wl_surface().is_some_and(|surface| {
        with_renderer_surface_state(&surface, |state| state.buffer().is_some()).unwrap_or(false)
    })
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
        if let Some(window) = self.caminito.override_redirect_for_surface(surface) {
            let window = window.clone();
            self.override_redirect_closes(&window);
            return;
        }
        let caminito = &mut self.caminito;
        let now = caminito.now();
        let slides = &mut caminito.popup_slides;
        let Some(index) = slides.shown.iter().position(|s| s.surface == *surface) else {
            return;
        };
        let shown = slides.shown.remove(index);
        let opening = slides.take_opening(&Popup::Xdg(surface.clone()), now);
        if !caminito.popup_slides_seen(&shown.root) {
            return;
        }
        let back = SlideBack {
            root: Root::Surface(shown.root),
            place: shown.place,
            geometry: shown.geometry,
            edge: shown.edge,
            opening,
        };
        self.slide_back(surface, back);
    }

    /// Starts the slide back of override-redirect `window`, if it slides and is on screen,
    /// from a picture of it taken now, while its surface still has its buffer: X unmapped
    /// or destroyed it, or Xwayland destroyed its surface first. One closed on its way out
    /// turns round. With no surface or buffer left, it goes at once.
    pub(crate) fn override_redirect_closes(&mut self, window: &X11Surface) {
        let caminito = &mut self.caminito;
        let id = window.window_id();
        let now = caminito.now();
        let slides = &mut caminito.popup_slides;
        let Some(index) = slides.override_redirect.iter().position(|s| s.window == id) else {
            return;
        };
        let shown = slides.override_redirect.remove(index);
        let opening = slides.take_opening(&Popup::OverrideRedirect(id), now);
        if !caminito.popup_slides_run() {
            return;
        }
        let Some(surface) = window.wl_surface() else {
            tracing::debug!("no surface of a closing override-redirect window: it goes at once");
            return;
        };
        let back = SlideBack {
            root: Root::Global(id),
            place: override_redirect_bbox(window),
            geometry: Point::default(),
            edge: shown.edge,
            opening,
        };
        self.slide_back(&surface, back);
    }

    /// Starts a popup's slide back into its edge as `back` has it, from a picture of
    /// `surface` taken now at the scale of the output it's on: from where it is if it was
    /// on its way out, from its place otherwise. With no buffer left, it goes at once.
    fn slide_back(&mut self, surface: &WlSurface, back: SlideBack) {
        let caminito = &self.caminito;
        let Some(origin) = caminito.closing_origin(&back.root) else {
            return;
        };
        let rect = Rectangle::new(origin + back.place.loc, back.place.size);
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
        let motion = match back.opening {
            Some(opening) => opening.motion.turned(caminito.now()),
            None => caminito.motion(back.edge, back.place.size, true),
        };
        caminito.popup_slides.closing.push(ClosingPopup {
            root: back.root,
            place: back.place,
            geometry: back.geometry,
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
    fn an_override_redirect_window_comes_out_of_the_edge_away_from_the_pointer() {
        let menu = rect(300, 200, 150, 200);
        let at = |x, y| pointer_edge(menu, Point::from((x, y)));

        assert_eq!(
            at(300.0, 200.0),
            Edge::Top,
            "under the pointer, at its corner"
        );
        assert_eq!(at(320.0, 120.0), Edge::Top, "above it");
        assert_eq!(at(320.0, 300.0), Edge::Top, "on its middle row");
        assert_eq!(at(320.0, 301.0), Edge::Bottom, "below its middle row");
        assert_eq!(
            at(320.0, 790.0),
            Edge::Bottom,
            "below it, as a bottom panel's"
        );
        assert_eq!(at(100.0, 300.0), Edge::Left, "beside it, to its left");
        assert_eq!(at(450.0, 300.0), Edge::Right, "beside it, to its right");
        assert_eq!(at(100.0, 100.0), Edge::Top, "above it and to its left");
        assert_eq!(at(600.0, 450.0), Edge::Bottom, "below it and to its right");
    }

    #[test]
    fn every_override_redirect_window_slides_but_a_utility_or_a_dnd_one() {
        assert!(slides(None));
        assert!(slides(Some(WmWindowType::PopupMenu)));
        assert!(slides(Some(WmWindowType::Tooltip)));
        assert!(!slides(Some(WmWindowType::Utility)));
        assert!(!slides(Some(WmWindowType::Dnd)));
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
