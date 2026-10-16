//! The Opening animation (251, ADR 0034): a window that's shown, for the first time or again
//! after its client hid it, grows from 0.8 of its size to its own about its centre and fades
//! in, for 200 ms of Caminito's clock, with its Frame, its shadow and its popups. It's
//! Plasma's Scale effect.
//!
//! The Closing animation (252) is its reverse, drawn from a picture: a window that its client
//! destroys or hides while it's on screen leaves its last frame, with its Frame, shadow and
//! popups, drawn at its place in the stack, which shrinks to 0.8 of its size about its centre
//! and fades out. The picture is drawn into a texture of the backend's renderer while the
//! window still has its buffers (`State::keep_closing_picture`): just before a close
//! handler has the window leave policy, when its client commits a null buffer, and when its
//! surface is destroyed, which can come before Xwayland's unmap or with a client that goes.
//! Policy lets the window go at once: the focus moves on, a click goes to what's beneath,
//! and no query of policy sees the picture. A window with no buffer left closes at once.
//!
//! Policy has the window mapped from the start, as it has a window minimised from the start
//! of the minimise animation (172): the focus, input and what its client is told are as
//! with no animation, at its real place, and only the drawing takes its time. A window
//! maps into policy as a new one each time its client shows it (`window/lifecycle.rs`),
//! which notes it here; the refresh pass then starts the animation of each one noted that
//! shows and is a window the animation is for (`animates`), and the scene draws it at the
//! size and opacity it has reached, at every frame of its outputs, until it's over.
//!
//! An animation that's interrupted hands over from what's on screen (253), where Plasma
//! starts again from the animation's own start (ADR 0034). An opening `done` of the way and
//! a closing `1 - done` of the way are drawn at the same size and opacity, so one that takes
//! over from the other starts that far into its own curve, from the rectangle the other was
//! drawn at (`Handover`): a window that closes on its way open, and one its client shows
//! again while its picture fades, which is a new window in policy with the same key. A
//! window whose frame changes on its way open grows from where it's drawn into its new
//! frame, read each frame; one minimised on its way open stops opening, and the minimise
//! animation takes it from its real place.
//!
//! The numbers and the windows are those of Plasma's Scale effect, in KWin's
//! `src/plugins/scale/package/contents/code/main.js` (ADR 0034).

use std::any::Any;
use std::time::Duration;

use caminito_policy::{OutputId, PolicyWindow, WindowId, WindowType};
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::Id;
use smithay::backend::renderer::{
    Bind, Color32F, ContextId, ImportAll, ImportMem, Offscreen, Renderer, Texture,
};
use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle, Size, Transform};

use crate::output::output_geometry;
use crate::scene::{self, SceneLayer};
use crate::state::{Caminito, State};
use crate::window::WindowKey;
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

/// How far a closing `elapsed` after its start is on its way, from 0 to 1: the Scale
/// effect's cubic ease-in, so the picture leaves its place slowly and goes fastest at the end.
fn way_closed(elapsed: Duration) -> f64 {
    (elapsed.as_secs_f64() / LENGTH.as_secs_f64())
        .min(1.0)
        .powi(3)
}

/// How long after its start an opening is `done` of the way: `way_done` turned round. An
/// opening `done` of the way and a closing `1 - done` of the way are drawn alike, so one
/// that takes over from the other starts this far into its own curve (253).
fn opened_for(done: f64) -> Duration {
    LENGTH.mul_f64(1.0 - (1.0 - done.clamp(0.0, 1.0)).cbrt())
}

/// How long after its start a closing is `done` of the way: `way_closed` turned round.
fn closed_for(done: f64) -> Duration {
    LENGTH.mul_f64(done.clamp(0.0, 1.0).cbrt())
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

/// Where an interrupted animation was drawn as it was interrupted, and how far on its way it
/// then was, from which it goes on to its end (253).
#[derive(Debug, Clone, Copy)]
struct Handover {
    rect: Rectangle<i32, Logical>,
    done: f64,
}

impl Handover {
    /// How an animation starts that takes over from another drawn at `rect` and `done` of
    /// its own way: as far into its own curve as the other has left to go, which `curve_for`
    /// turns into time, and from `rect`.
    fn taking_over(
        rect: Rectangle<i32, Logical>,
        done: f64,
        curve_for: fn(f64) -> Duration,
    ) -> (Duration, Option<Self>) {
        let left = 1.0 - done;
        (curve_for(left), Some(Self { rect, done: left }))
    }

    /// Where the animation is drawn `done` of the way, which it ends at `to`.
    fn towards(&self, to: Rectangle<i32, Logical>, done: f64) -> Rectangle<i32, Logical> {
        if self.done >= 1.0 {
            return to;
        }
        between(self.rect, to, (done - self.done) / (1.0 - self.done))
    }
}

/// `frame` at `scale` of its size about its centre, never empty.
fn zoomed(frame: Rectangle<i32, Logical>, scale: f64) -> Rectangle<i32, Logical> {
    let at = |n: i32| ((f64::from(n) * scale).round() as i32).max(1);
    let size = Size::from((at(frame.size.w), at(frame.size.h)));
    let loc = frame.loc + Point::from(((frame.size.w - size.w) / 2, (frame.size.h - size.h) / 2));
    Rectangle::new(loc, size)
}

/// Where a window whose frame is at `frame` is drawn `done` of the way through its opening:
/// scaled about the frame's centre, never empty.
fn opening_rect(frame: Rectangle<i32, Logical>, done: f64) -> Rectangle<i32, Logical> {
    zoomed(frame, FROM_SCALE + (1.0 - FROM_SCALE) * done)
}

/// How opaque a window is drawn `done` of the way through its opening: it fades in as it
/// grows.
fn opening_alpha(done: f64) -> f32 {
    done as f32
}

/// Where the frame of a window that closed at `frame` is drawn `done` of the way through its
/// closing: from its own size to 0.8 of it about its centre.
fn closing_rect(frame: Rectangle<i32, Logical>, done: f64) -> Rectangle<i32, Logical> {
    zoomed(frame, 1.0 - (1.0 - FROM_SCALE) * done)
}

/// How opaque a closing window's picture is drawn `done` of the way through: it fades out as
/// it shrinks.
fn closing_alpha(done: f64) -> f32 {
    (1.0 - done) as f32
}

/// A window's last frame, with its Frame, its shadow and its popups, in a texture of the
/// renderer that drew it.
pub(crate) struct Picture {
    /// The texture and its renderer's context, as `(ContextId<T>, T)` for the renderer's
    /// texture type `T`, which the scene, generic over the renderer, takes back.
    texture: Box<dyn Any>,
    /// What the texture covers, from the window's frame's corner.
    pub area: Rectangle<i32, Logical>,
    /// The element the scene draws it as, which keeps its id from frame to frame.
    pub id: Id,
}

impl std::fmt::Debug for Picture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Picture")
            .field("area", &self.area)
            .finish_non_exhaustive()
    }
}

impl Picture {
    /// The texture, for a renderer whose textures are `T`, or `None` for another one's.
    pub(crate) fn texture<T: Texture + 'static>(&self) -> Option<&(ContextId<T>, T)> {
        self.texture.downcast_ref()
    }
}

/// Draws window `id`'s picture with `renderer`, into a texture of its own at the scale of the
/// window's main output, or returns `None` if it has no buffer left to draw from or the
/// renderer fails. `T` is what the renderer draws into offscreen, which is its texture or
/// becomes one.
pub(crate) fn take_picture<R, T>(
    renderer: &mut R,
    caminito: &Caminito,
    id: WindowId,
) -> Option<Picture>
where
    R: Renderer + ImportAll + ImportMem + Offscreen<T> + Bind<T>,
    R::TextureId: Send + Clone + From<T> + 'static,
{
    let main = caminito.policy.window(id)?.main_output();
    let scale = main.map_or(1.0, |main| main.current_scale().fractional_scale());
    let (elements, area) = scene::picture_elements(renderer, caminito, id, scale)?;
    let size = area.size.to_physical_precise_round(scale);
    let mut target = renderer
        .create_buffer(Fourcc::Argb8888, (size.w, size.h).into())
        .map_err(|err| tracing::debug!("can't keep a closing window's picture: {err:?}"))
        .ok()?;
    {
        let mut framebuffer = renderer.bind(&mut target).ok()?;
        let mut tracker = OutputDamageTracker::new(size, scale, Transform::Normal);
        let drawn = tracker
            .render_output(
                renderer,
                &mut framebuffer,
                0,
                &elements,
                Color32F::TRANSPARENT,
            )
            .map_err(|err| tracing::debug!("can't draw a closing window's picture: {err:?}"))
            .ok()?;
        // The texture is drawn from at once, so the renderer finishes it first.
        if let Err(err) = drawn.sync.wait() {
            tracing::debug!("can't wait for a closing window's picture: {err:?}");
        }
    }
    let texture = R::TextureId::from(target);
    let context = renderer.context_id();
    Some(Picture {
        texture: Box::new((context, texture)),
        area,
        id: Id::new(),
    })
}

/// One window on its way open.
#[derive(Debug)]
pub(crate) struct Opening {
    window: WindowId,
    /// When it was shown, on Caminito's clock.
    start: Duration,
    /// How much of the curve it had run by then: one that takes over from its window's
    /// closing picture starts where the picture had got to (253).
    skipped: Duration,
    /// The window's frame, as the refresh pass last found it.
    frame: Option<Rectangle<i32, Logical>>,
    /// Where it was drawn when it took over from a picture or its window's frame last
    /// changed: it grows from there into its frame, read each frame (253).
    from: Option<Handover>,
}

impl Opening {
    fn elapsed(&self, now: Duration) -> Duration {
        now.saturating_sub(self.start) + self.skipped
    }

    fn is_over(&self, now: Duration) -> bool {
        self.elapsed(now) >= LENGTH
    }

    /// How far it is on its way at `now`, from 0 to 1.
    pub(crate) fn done(&self, now: Duration) -> f64 {
        way_done(self.elapsed(now))
    }

    /// Where its window's frame is drawn at `now`, globally, with the frame at `frame`.
    pub(crate) fn rect(
        &self,
        frame: Rectangle<i32, Logical>,
        now: Duration,
    ) -> Rectangle<i32, Logical> {
        let done = self.done(now);
        match self.from {
            Some(from) => from.towards(frame, done),
            None => opening_rect(frame, done),
        }
    }

    /// How opaque its window is drawn at `now`.
    pub(crate) fn alpha(&self, now: Duration) -> f32 {
        opening_alpha(self.done(now))
    }
}

/// A window's picture on its way out, with where it was.
#[derive(Debug)]
pub(crate) struct Closing {
    /// The window it was, by the id it had in policy.
    pub window: WindowId,
    /// And by its key, which it keeps if its client shows it again (253).
    key: WindowKey,
    pub picture: Picture,
    /// When the window closed, on Caminito's clock.
    start: Duration,
    /// How much of the curve it had run by then: a window closed on its way open closes from
    /// where it had got to (253).
    skipped: Duration,
    /// Where its frame was, globally.
    pub frame: Rectangle<i32, Logical>,
    /// Where it was drawn as it took over from its window's opening, if it did (253).
    from: Option<Handover>,
    /// The windows that were beneath it in the stack, which it's drawn over.
    pub beneath: Vec<WindowId>,
    /// The scene layer it was drawn in: the windows', or the fullscreen windows'.
    pub layer: SceneLayer,
    /// Its workspace, or `None` for a sticky window, and the output it showed by: it's drawn
    /// while that output shows the workspace.
    workspace: Option<usize>,
    output: Option<OutputId>,
    /// Whether it's drawn, as the refresh pass last found from the two above, so the scene
    /// reads no more of policy for it.
    pub shows: bool,
}

impl Closing {
    fn elapsed(&self, now: Duration) -> Duration {
        now.saturating_sub(self.start) + self.skipped
    }

    fn is_finished(&self, now: Duration) -> bool {
        self.elapsed(now) >= LENGTH
    }

    /// How far it is on its way at `now`, from 0 to 1.
    fn done(&self, now: Duration) -> f64 {
        way_closed(self.elapsed(now))
    }

    /// Where its window's frame is drawn at `now`, globally.
    pub(crate) fn rect(&self, now: Duration) -> Rectangle<i32, Logical> {
        let done = self.done(now);
        match self.from {
            Some(from) => from.towards(closing_rect(self.frame, 1.0), done),
            None => closing_rect(self.frame, done),
        }
    }

    /// How opaque its picture is drawn at `now`.
    pub(crate) fn alpha(&self, now: Duration) -> f32 {
        closing_alpha(self.done(now))
    }

    /// Whether it's drawn just over `window`, going down the stack from the top: the first
    /// window met that was beneath it.
    pub(crate) fn is_over(&self, window: WindowId) -> bool {
        self.beneath.contains(&window)
    }

    /// What its picture covers at its start, globally, which it never leaves as it shrinks.
    fn covered(&self) -> Rectangle<i32, Logical> {
        let area = self.picture.area;
        Rectangle::new(self.frame.loc + area.loc, area.size)
    }
}

/// A closing window's picture as it's drawn, for the debug dump.
pub(crate) struct DrawnClosing {
    /// The id its window had in policy.
    pub window: WindowId,
    /// Where its frame is drawn, globally.
    pub rect: Rectangle<i32, Logical>,
    pub alpha: f32,
    /// The window it's drawn just over, if any is left of those that were beneath it.
    pub over: Option<WindowId>,
}

/// The windows on their way open, the pictures of those on their way out, and the windows
/// shown since the last refresh pass.
#[derive(Debug, Default)]
pub(crate) struct OpenCloseAnimations {
    /// The windows that mapped into policy since the last refresh pass.
    mapped: Vec<WindowId>,
    running: Vec<Opening>,
    /// The pictures kept of managed windows about to close, each the latest of its window.
    kept: Vec<(WindowId, Picture)>,
    closing: Vec<Closing>,
}

impl OpenCloseAnimations {
    /// Notes a window that mapped into policy, whose animation the refresh pass starts.
    pub(crate) fn window_mapped(&mut self, window: WindowId) {
        self.mapped.push(window);
    }

    /// Keeps `picture` of `window`, for its closing, in place of one kept before.
    fn keep(&mut self, window: WindowId, picture: Picture) {
        self.kept.retain(|(id, _)| *id != window);
        self.kept.push((window, picture));
    }

    /// Forgets the picture kept of `window`, which has drawn a new frame since.
    pub(crate) fn forget_kept(&mut self, window: WindowId) {
        self.kept.retain(|(id, _)| *id != window);
    }

    /// Takes the picture kept of `window`.
    fn take_kept(&mut self, window: WindowId) -> Option<Picture> {
        let index = self.kept.iter().position(|(id, _)| *id == window)?;
        Some(self.kept.remove(index).1)
    }

    /// When the first of the running animations is over: `deadlines` then draws its window
    /// at rest, or takes its picture away (174).
    pub(crate) fn deadline(&self) -> Option<Duration> {
        let end = |start: Duration, skipped: Duration| start + LENGTH.saturating_sub(skipped);
        let opening = self.running.iter().map(|o| end(o.start, o.skipped));
        let closing = self.closing.iter().map(|c| end(c.start, c.skipped));
        opening.chain(closing).min()
    }

    /// The pictures on their way out at `now`, oldest first.
    pub(crate) fn closing(&self, now: Duration) -> impl Iterator<Item = &Closing> {
        self.closing
            .iter()
            .filter(move |closing| !closing.is_finished(now))
    }

    /// `window`'s opening, while it runs at `now`.
    pub(crate) fn opening(&self, window: WindowId, now: Duration) -> Option<&Opening> {
        self.opening_windows(now)
            .find(|opening| opening.window == window)
    }

    /// The windows on their way open at `now`, in the order they were shown.
    pub(crate) fn opening_windows(&self, now: Duration) -> impl Iterator<Item = &Opening> {
        self.running
            .iter()
            .filter(move |opening| !opening.is_over(now))
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

impl State {
    /// Keeps a picture of the managed window `key` names for its closing, if it would be
    /// seen to close now and has a buffer to draw it from. Called just before the window
    /// leaves policy, and wherever its buffers are about to go before that: a null buffer's
    /// commit and the surface's destruction.
    pub(crate) fn keep_closing_picture(&mut self, key: &WindowKey) {
        let caminito = &self.caminito;
        let Some(id) = caminito.mapped_for_key(key) else {
            return;
        };
        if !caminito.closes_seen(id) {
            return;
        }
        if let Some(picture) = self.backend.take_picture(caminito, id) {
            self.caminito.open_close_animations.keep(id, picture);
        }
    }
}

impl Caminito {
    /// Whether window `id` would be seen to close now: the animation is on, the window is
    /// one it's for, and it's on screen.
    fn closes_seen(&self, id: WindowId) -> bool {
        let policy = &self.policy;
        self.settings.open_close_animation
            && policy.window(id).is_some_and(animates)
            && policy.is_showing(id)
    }

    /// Starts window `id`'s closing from the picture kept of it, just before it leaves
    /// policy, at its place in the stack. With no picture kept, it closes at once.
    pub(crate) fn start_closing(&mut self, id: WindowId) {
        let Some(picture) = self.open_close_animations.take_kept(id) else {
            if self.closes_seen(id) {
                tracing::debug!(?id, "no picture of a closing window: it closes at once");
            }
            return;
        };
        if !self.closes_seen(id) {
            return;
        }
        let policy = &self.policy;
        let (Some(frame), Some(window)) = (policy.frame_rect(id), policy.window(id)) else {
            return;
        };
        let key = WindowKey::of(window.window());
        let now = self.now();
        // One closed on its way open shrinks and fades from where it had got to, for what's
        // left of the closing's curve from there (253).
        let (skipped, from) = match self.open_close_animations.opening(id, now) {
            Some(opening) => {
                Handover::taking_over(opening.rect(frame, now), opening.done(now), closed_for)
            }
            None => (Duration::ZERO, None),
        };
        let stack = policy.stacking_all();
        let beneath = match stack.iter().position(|other| *other == id) {
            Some(index) => stack[..index].to_vec(),
            None => Vec::new(),
        };
        let layer = if scene::window_is_in(self, id, SceneLayer::FullscreenWindows) {
            SceneLayer::FullscreenWindows
        } else {
            SceneLayer::Windows
        };
        let workspace = (!policy.is_sticky(id))
            .then(|| policy.workspace_of(id))
            .flatten();
        let closing = Closing {
            window: id,
            key,
            picture,
            start: now,
            skipped,
            frame,
            from,
            beneath,
            layer,
            workspace,
            output: policy.output_showing(id),
            shows: true,
        };
        self.open_close_animations.closing.push(closing);
        self.queue_redraw_all();
    }

    /// Whether `closing`'s picture is drawn: while the output its window showed by shows
    /// its workspace.
    fn closing_shows(&self, closing: &Closing) -> bool {
        let Some(workspace) = closing.workspace else {
            return true;
        };
        let active = match closing.output {
            Some(output) => self.policy.active_workspace_on(output),
            None => self.policy.primary_active_workspace(),
        };
        active == workspace
    }

    /// Starts the animation of each window that mapped since the last refresh pass, shows
    /// and animates, and ends that of each window that no longer shows. With
    /// `open-close-animation` off, or one of the minimise animation's three keys against
    /// it, none starts; a change of those keys lets one that runs finish. A closing
    /// window's picture is drawn while its workspace shows, and a picture kept of a window
    /// that's no longer managed, which left policy some other way, goes.
    pub(crate) fn refresh_open_close_animations(&mut self) {
        let now = self.now();
        let shows: Vec<bool> = self
            .open_close_animations
            .closing
            .iter()
            .map(|closing| self.closing_shows(closing))
            .collect();
        let mut changed = false;
        for (closing, shows) in self.open_close_animations.closing.iter_mut().zip(shows) {
            changed |= closing.shows != shows;
            closing.shows = shows;
        }
        let policy = &self.policy;
        let animations = &mut self.open_close_animations;
        animations
            .kept
            .retain(|(id, _)| policy.window(*id).is_some());
        let mapped = std::mem::take(&mut animations.mapped);
        let before = animations.running.len();
        animations
            .running
            .retain(|opening| policy.is_showing(opening.window));
        changed |= animations.running.len() != before;
        for opening in &mut animations.running {
            let Some(frame) = policy.frame_rect(opening.window) else {
                continue;
            };
            if let Some(was) = opening.frame.filter(|was| *was != frame) {
                // Maximised, resized or moved on its way open: it grows into its new place
                // from where it's drawn (253).
                let rect = opening.rect(was, now);
                let done = opening.done(now);
                opening.from = Some(Handover { rect, done });
                changed = true;
            }
            opening.frame = Some(frame);
        }
        for id in mapped {
            let Some(window) = policy.window(id) else {
                continue;
            };
            // Shown again while its picture fades: the picture goes, and the window opens
            // from where it was drawn (253).
            let key = WindowKey::of(window.window());
            let picture = animations
                .closing
                .iter()
                .position(|closing| closing.key == key && !closing.is_finished(now))
                .map(|index| animations.closing.remove(index));
            changed |= picture.is_some();
            // One on a workspace no output shows, or that maps minimised, isn't seen to open.
            let shows = animates(window) && policy.is_showing(id);
            if !self.settings.open_close_animation || !shows {
                continue;
            }
            // One whose workspace was hidden wasn't drawn, so the window opens as a new one.
            let (skipped, from) = match picture.filter(|closing| closing.shows) {
                Some(closing) => {
                    Handover::taking_over(closing.rect(now), closing.done(now), opened_for)
                }
                None => (Duration::ZERO, None),
            };
            animations.running.push(Opening {
                window: id,
                start: now,
                skipped,
                frame: policy.frame_rect(id),
                from,
            });
            changed = true;
        }
        if changed {
            self.queue_redraw_all();
        }
    }

    /// Ends the animations that are over at `now`, and draws their windows at rest, for
    /// `deadlines`. While one runs, the outputs it's on are drawn again at each of their
    /// frames (`State::redraw_queued_outputs`), so there's no step to wait for.
    pub(crate) fn open_close_animation_elapse(&mut self, now: Duration) {
        let animations = &mut self.open_close_animations;
        let before = animations.running.len() + animations.closing.len();
        animations.running.retain(|opening| !opening.is_over(now));
        animations
            .closing
            .retain(|closing| !closing.is_finished(now));
        if animations.running.len() + animations.closing.len() != before {
            self.queue_redraw_all();
        }
    }

    /// Whether a window on its way open or a closing window's picture at `now` is drawn on
    /// `output`, which is then drawn again at its next frame.
    pub(crate) fn open_close_animation_on(&self, output: &Output, now: Duration) -> bool {
        let area = output_geometry(output);
        let animations = &self.open_close_animations;
        let opening = animations.opening_windows(now).filter_map(|opening| {
            let frame = self.policy.frame_rect(opening.window)?;
            Some(opening.rect(frame, now))
        });
        let closing = animations
            .closing(now)
            .filter(|closing| closing.shows)
            .map(Closing::covered);
        opening.chain(closing).any(|drawn| drawn.overlaps(area))
    }

    /// Each closing window's picture at `now`, oldest first, as it's drawn, for the debug
    /// dump.
    pub(crate) fn drawn_closing_windows(&self, now: Duration) -> Vec<DrawnClosing> {
        let stack = self.policy.stacking_all();
        self.open_close_animations
            .closing(now)
            .filter(|closing| closing.shows)
            .map(|closing| {
                let over = stack.iter().rev().find(|id| closing.is_over(**id)).copied();
                DrawnClosing {
                    window: closing.window,
                    rect: closing.rect(now),
                    alpha: closing.alpha(now),
                    over,
                }
            })
            .collect()
    }

    /// Each window on its way open at `now`, in the order they were shown, with where its
    /// frame is drawn, globally, and its opacity, for the debug dump.
    pub(crate) fn drawn_opening_windows(
        &self,
        now: Duration,
    ) -> Vec<(WindowId, Rectangle<i32, Logical>, f32)> {
        self.open_close_animations
            .opening_windows(now)
            .filter_map(|opening| {
                let frame = self.policy.frame_rect(opening.window)?;
                Some((opening.window, opening.rect(frame, now), opening.alpha(now)))
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
    fn a_window_closes_to_four_fifths_of_its_size_about_its_centre_and_no_opacity() {
        let frame = rect(100, 50, 400, 300);

        assert_eq!(closing_rect(frame, 0.0), frame);
        assert_eq!(closing_rect(frame, 0.5), rect(120, 65, 360, 270));
        assert_eq!(closing_rect(frame, 1.0), rect(140, 80, 320, 240));
        assert_eq!(closing_alpha(0.0), 1.0);
        assert_eq!(closing_alpha(1.0), 0.0);
    }

    #[test]
    fn the_closing_eases_in() {
        assert_eq!(way_closed(Duration::ZERO), 0.0);
        // A cubic ease-in is an eighth of the way at half time.
        assert_eq!(way_closed(LENGTH / 2), 0.125);
        assert_eq!(way_closed(LENGTH), 1.0);
        assert_eq!(way_closed(2 * LENGTH), 1.0);
    }

    #[test]
    fn an_animation_that_takes_over_starts_as_far_into_its_curve_as_the_other_had_got() {
        for done in [0.0, 0.25, 0.5, 0.9, 1.0] {
            assert!((way_done(opened_for(done)) - done).abs() < 1e-6, "{done}");
            assert!((way_closed(closed_for(done)) - done).abs() < 1e-6, "{done}");
        }
        // The curves mirror each other: half way open is a closing an eighth of the way to go.
        assert_eq!(closed_for(1.0 - way_done(LENGTH / 2)), LENGTH / 2);
    }

    #[test]
    fn a_handover_goes_from_where_it_was_drawn_to_its_end() {
        let from = Handover {
            rect: rect(100, 100, 200, 100),
            done: 0.5,
        };
        let to = rect(300, 100, 400, 300);

        assert_eq!(from.towards(to, 0.5), from.rect);
        assert_eq!(from.towards(to, 0.75), rect(200, 100, 300, 200));
        assert_eq!(from.towards(to, 1.0), to);
    }

    #[test]
    fn an_opening_is_over_after_plasmas_200_ms() {
        let mut animations = OpenCloseAnimations::default();
        let start = Duration::from_secs(10);
        animations.running.push(Opening {
            window: WindowId(1),
            start,
            skipped: Duration::ZERO,
            frame: None,
            from: None,
        });

        let done = |at: Duration| animations.opening(WindowId(1), at).map(|o| o.done(at));
        assert_eq!(done(start), Some(0.0));
        assert!(done(start + LENGTH / 2).is_some());
        assert_eq!(done(start + LENGTH), None);
        assert_eq!(animations.deadline(), Some(start + LENGTH));
    }
}
