//! The Overview's side in the compositor (241, [ADR 0033]): a view of each output's active
//! workspace with its windows shrunk and set side by side, which a System shortcut opens.
//! Policy has whether it's open, the windows listed on each output and what a choice does,
//! since it's the seat's Interaction (187). Here are the keys and the clicks that drive it
//! while it's open, and what it's laid out and drawn from. `layout` has each window's
//! place, `bar` each workspace's cell, `draw` draws the labels, and `scene` puts the bar,
//! the windows, the labels and the dimming on the output, in place of what the output
//! shows otherwise.
//!
//! The arrow keys move policy's selection in the layout and Enter chooses it, and a middle
//! click on a window asks it to close (242). A window that opens or closes while it's open
//! joins the layout or leaves it at the next refresh pass, and the others take their new
//! places in that frame, or ease to them from where they are if one closes (250).
//!
//! The bar of workspaces (244) is one row along the top, of a live picture of each
//! workspace on the output, and the layout is beneath it. A click on a workspace, or a
//! workspace binding, switches the output there with the Overview open, and the Workspace
//! slide moves the laid-out windows: the old workspace's layout leaves and the new one's
//! arrives. A window that only the bar shows gets its frame callbacks at a reduced rate.
//!
//! It opens and closes with its transition (243): each window moves between its real
//! place and its place in the layout, and the rest fades. Policy has it open or closed
//! from the first frame, so the keys and the clicks act on what policy has at once: here
//! the Overview stays for the closing's frames after policy has closed its own. Without
//! `overview-animation`, it opens and closes in one frame.
//!
//! A window of the layout is dragged onto a workspace of the bar to move it there (245).
//! Policy has the press and the drag, and what a drop does: here the pointer's moves and
//! the release are told to it, with the cell under the pointer. The dragged window is
//! drawn under the pointer, over the layout and the bar, and the cell under it is marked.
//!
//! It's on every output at once (246), each laid out here from what policy lists there,
//! with its own bar, and they share one transition. A window is dragged from one output's
//! layout onto another output: policy hears which output the pointer is on, and the
//! dragged window is drawn on each output it's over.
//!
//! Over a bar, the dragged window shrinks to the scale of its cells, and grows back off it
//! (249). Dropped, it eases from under the pointer to where it's drawn next: its place in
//! a cell's picture, or its slot in a layout. Policy moves it at the release, and only the
//! drawing takes time: the Overview closing, a switch to the workspace it went to or a
//! press on it cuts the drop to its end.
//!
//! When a window leaves an output's layout, dropped on another workspace or onto another
//! output, or joins it from another output's, each window that stays there eases from
//! where it's drawn to its new slot (250). Another change of the layout on the way eases
//! each from where it's drawn then. The Overview closing or a switch of the output's
//! workspace cuts that to its end.
//!
//! The Application overview (CMN-185) is the Overview with policy's list of one
//! Application's windows, and is laid out and drawn as the Overview is. A change of scope
//! between them while it's open eases the windows that stay to their new slots, as above,
//! and fades in those that join and fades out those that leave, as the transition fades a
//! minimised window.
//!
//! [ADR 0033]: ../../../docs/adr/0033-the-workspace-slide-and-the-overview-are-taken-from-plasma.md

pub(crate) mod bar;
pub(crate) mod drag;
pub(crate) mod draw;
pub(crate) mod layout;
pub(crate) mod reflow;
pub(crate) mod timing;
pub(crate) mod transition;

use std::time::{Duration, Instant};

use caminito_policy::{
    Action, Direction, OutputId, OverviewOutput, OverviewScope, PolicyWindow, WindowId,
};
use caminito_theme::{PaletteColors, Title, Variant, pango};
use smithay::backend::input::{ButtonState, InputTime, Keycode};
use smithay::backend::renderer::element::Id;
use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
use smithay::input::keyboard::{FilterResult, Keysym, KeysymHandle, ModifiersState};
use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle, Size};

use self::bar::Bar;
use self::drag::{Aim, Dropping, Shrink};
use self::draw::Label;
use self::layout::Layout;
use self::reflow::Reflow;
use self::transition::{Heading, Transition};
use crate::decoration::IconSource;
use crate::input::bindings::Binding;
use crate::input::{BTN_LEFT, BTN_MIDDLE, Bound, press_keysyms};
use crate::output::{output_geometry, output_id};
use crate::state::{Caminito, SEAT, State};

/// How much of black is over the wallpaper while the Overview is open.
pub(crate) const DIM: f32 = 0.5;

/// How often a window that only the bar shows gets frame callbacks (244): 10 a second,
/// the developer's start value of 2026-10-13, which 247 checks in a Session.
pub(crate) const BAR_FRAME_INTERVAL: Duration = Duration::from_millis(100);

/// The compositor's side of the Overview on one output, as the refresh pass last laid it
/// out: while policy has it open, and while it's drawn closing after (243).
pub(crate) struct Overview {
    /// The output it's on.
    pub(crate) output: Output,
    /// Its bar of workspaces (244).
    pub(crate) bar: Bar,
    /// The active workspace's windows, which policy listed, laid out beneath the bar.
    pub(crate) laid: Laid,
    /// The windows of each workspace that a Workspace slide on the output leaves (244),
    /// laid out as they were, while it runs.
    pub(crate) leaving: Vec<Laid>,
    /// The dimming's render element, the same one at every frame.
    pub(crate) dim: Id,
    /// Each cell's backing render element, the same ones at every frame.
    pub(crate) cell_backs: Vec<Id>,
    /// The windows each cell shows, from the first workspace's, bottom to top: those of
    /// the workspace whose Main output's this one, minimised ones aside.
    pub(crate) cell_windows: Vec<Vec<WindowId>>,
    /// The active workspace, whose cell is marked.
    pub(crate) active: usize,
    /// The sides of the active workspace's mark, the same ones at every frame.
    pub(crate) mark: [Id; 4],
    /// The mark over the cell a drag is on (245), the same one at every frame.
    pub(crate) target_mark: Id,
    /// When the windows that only the bar shows next get frame callbacks, on Caminito's
    /// clock.
    next_bar_frames: Duration,
    /// The transition, while it's on its way open or closed (243): every output's is the
    /// same (246).
    pub(crate) transition: Option<Transition>,
    /// The active workspace's windows on their way to their new slots, after one left the
    /// layout or joined it from another output's (250), or after a change of scope
    /// (CMN-185).
    pub(crate) reflow: Option<Reflow>,
    /// Which windows policy lists: every one, or one Application's (CMN-185).
    pub(crate) scope: OverviewScope,
    /// Where this output's first frame of a held opening is.
    first_frame: FirstFrame,
}

/// Where an output's first frame of a held opening is: its clock starts once every
/// output's is on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FirstFrame {
    Undrawn,
    Drawn,
    Shown,
}

/// What the refresh pass needs of an output's layout as it was, to ease its windows to
/// their new slots (250).
struct WasLaid {
    workspace: usize,
    /// Which windows it listed (CMN-185).
    scope: OverviewScope,
    /// Whether the Overview was on its way closed.
    closing: bool,
    /// Each window listed, with its slot, bottom to top.
    slots: Vec<(WindowId, Rectangle<i32, Logical>)>,
    /// Each window listed, with where it was drawn on its way to its slot.
    places: Vec<(WindowId, Rectangle<i32, Logical>)>,
    /// The windows' way to their slots, while it ran.
    reflow: Option<Reflow>,
}

impl WasLaid {
    /// `open`'s layout of the active workspace at `now`.
    fn of(open: &Overview, now: Duration) -> Self {
        let slots = open.laid.windows.iter().zip(&open.laid.layout.slots);
        Self {
            workspace: open.laid.workspace,
            scope: open.scope,
            closing: open.is_closing(),
            slots: slots.map(|(id, slot)| (*id, slot.window)).collect(),
            places: open.places(now),
            reflow: open.reflowing(now).cloned(),
        }
    }
}

/// One window as the Overview draws it at one time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DrawnWindow {
    pub(crate) id: WindowId,
    /// Where it's drawn, from the output's top left corner.
    pub(crate) rect: Rectangle<i32, Logical>,
    /// How opaque it's drawn.
    pub(crate) alpha: f32,
}

/// One workspace's windows laid out beneath the bar.
pub(crate) struct Laid {
    pub(crate) workspace: usize,
    /// The windows listed, bottom to top.
    pub(crate) windows: Vec<WindowId>,
    /// Each window's place, from the output's top left corner, in `windows`' order.
    pub(crate) layout: Layout,
    /// Each window's label as last drawn, in `windows`' order. A label with no room has
    /// none, and neither has one cairo couldn't draw.
    pub(crate) labels: Vec<Option<MemoryRenderBuffer>>,
    /// What the labels were drawn from, to draw them again only when that changes.
    drawn: Drawn,
}

/// What one workspace's layout is made from, before its labels are drawn.
struct Planned {
    workspace: usize,
    windows: Vec<WindowId>,
    layout: Layout,
    drawn: Drawn,
}

impl Planned {
    /// Whether `laid` is this as it was laid out, labels and all.
    fn matches(&self, laid: &Laid) -> bool {
        self.workspace == laid.workspace
            && self.windows == laid.windows
            && self.layout == laid.layout
            && self.drawn == laid.drawn
    }
}

/// Everything the labels are drawn from.
#[derive(PartialEq)]
struct Drawn {
    /// Each listed window's label.
    labels: Vec<LabelInputs>,
    scale: i32,
    colors: PaletteColors,
    font: String,
    /// The frames' generation, which counts the changes to the font rendering and the
    /// icon theme.
    generation: u64,
}

/// What one window's label is drawn from.
#[derive(PartialEq)]
struct LabelInputs {
    /// Where it is.
    rect: Rectangle<i32, Logical>,
    title: String,
    /// Where its icon comes from.
    icon: IconSource,
    /// Whether the window is the selected one (242).
    selected: bool,
}

impl Overview {
    /// The active workspace's layout, then those a Workspace slide leaves.
    pub(crate) fn layouts(&self) -> impl Iterator<Item = &Laid> {
        std::iter::once(&self.laid).chain(&self.leaving)
    }

    /// Where window `id` of the active workspace's layout is drawn at `now` before the
    /// transition moves it, from the output's top left corner: on its way to `slot`, its
    /// place in the layout, after the layout changed (250), or there.
    pub(crate) fn place(
        &self,
        id: WindowId,
        slot: Rectangle<i32, Logical>,
        now: Duration,
    ) -> Rectangle<i32, Logical> {
        match self.reflowing(now) {
            Some(reflow) => reflow.rect(id, slot, now),
            None => slot,
        }
    }

    /// The active workspace's windows on their way to their new slots at `now` (250), if
    /// they're not there yet.
    pub(crate) fn reflowing(&self, now: Duration) -> Option<&Reflow> {
        self.reflow.as_ref().filter(|reflow| !reflow.is_over(now))
    }

    /// Where each window of the active workspace's layout is drawn at `now` before the
    /// transition moves it, as `place` has it.
    pub(crate) fn places(&self, now: Duration) -> Vec<(WindowId, Rectangle<i32, Logical>)> {
        let slots = self.laid.windows.iter().zip(&self.laid.layout.slots);
        slots
            .map(|(id, slot)| (*id, self.place(*id, slot.window, now)))
            .collect()
    }

    /// How open it's drawn at `now`, from 0, closed, to 1, open.
    pub(crate) fn part(&self, now: Duration) -> f64 {
        self.transition
            .map_or(1.0, |transition| transition.part(now))
    }

    /// Whether it's drawn on its way closed, after policy has closed it.
    fn is_closing(&self) -> bool {
        self.transition
            .is_some_and(|transition| transition.heading == Heading::Closing)
    }

    /// How much of black is over the wallpaper at `now`: the dimming fades in as it opens
    /// and out as it closes.
    pub(crate) fn dim(&self, now: Duration) -> f32 {
        DIM * self.part(now) as f32
    }

    /// How opaque the labels are drawn at `now`, which fade with the dimming.
    pub(crate) fn labels_alpha(&self, now: Duration) -> f32 {
        self.part(now) as f32
    }

    /// How opaque the panel and caja's desktop are drawn at `now`: they fade out as it
    /// opens and in as it closes.
    pub(crate) fn faded_alpha(&self, now: Duration) -> f32 {
        1.0 - self.part(now) as f32
    }
}

impl Caminito {
    /// The Overview on `output`, while it's open there and laid out, and while it's drawn
    /// closing there after (243).
    pub(crate) fn overview_on(&self, output: &Output) -> Option<&Overview> {
        self.overview.iter().find(|open| open.output == *output)
    }

    /// The Overview laid out on the output `location` is on, in global logical
    /// coordinates, and `location` from that output's top left corner.
    fn overview_at(
        &self,
        location: Point<f64, Logical>,
    ) -> Option<(&Overview, Point<f64, Logical>)> {
        self.overview.iter().find_map(|open| {
            let geometry = output_geometry(&open.output);
            let on = geometry.to_f64().contains(location);
            on.then(|| (open, location - geometry.loc.to_f64()))
        })
    }

    /// The window of the Overview's layout at `location`, in global logical coordinates:
    /// the one whose place or whose label it's on, on any output (246).
    /// While the layout's windows are on their way to their new slots (250), it's the
    /// topmost one drawn at `location`.
    fn overview_window_at(&self, location: Point<f64, Logical>) -> Option<WindowId> {
        let (open, at) = self.overview_at(location)?;
        if open.reflowing(self.now()).is_some() {
            let places = open.places(self.now()).into_iter().rev();
            return places
                .filter(|(_, place)| place.to_f64().contains(at))
                .map(|(id, _)| id)
                .next();
        }
        let slot = open.laid.layout.slot_at(at)?;
        open.laid.windows.get(slot).copied()
    }

    /// The output and the workspace of the Overview's bar at `location`, in global logical
    /// coordinates: the one whose cell it's on (244), on any output (246).
    fn overview_cell_at(&self, location: Point<f64, Logical>) -> Option<(OutputId, usize)> {
        let (open, at) = self.overview_at(location)?;
        let workspace = open.bar.cell_at(at)?;
        Some((output_id(&open.output), workspace))
    }

    /// The pointer moved to `location`, in global logical coordinates: policy hears it
    /// with the output it's on and the cell it's on there (246), while a press is held on a
    /// window of the Overview (245).
    pub(crate) fn overview_motion(&mut self, location: Point<f64, Logical>) {
        if self.overview_press_held() {
            let onto = self.overview_at(location);
            let onto = onto.map(|(open, _)| output_id(&open.output));
            let cell = self
                .overview_cell_at(location)
                .map(|(_, workspace)| workspace);
            self.policy.overview_pointer_on(location, onto, cell);
            self.aim_overview_drag(location);
        }
    }

    /// Whether the left button is held on a window of the open Overview (245).
    fn overview_press_held(&self) -> bool {
        self.policy
            .overview()
            .is_some_and(|open| open.drag.is_some())
    }

    /// The window that a drag takes (245), drawn under the pointer, with the point it was
    /// grabbed by under the pointer: at its size in the layout of its own output, or on its
    /// way to a cell's scale or back from it (249). Its rectangle is from the top left
    /// corner of `open`'s output, which it may be over or not (246). None while there's no
    /// drag, and for one past the threshold not yet.
    pub(crate) fn overview_dragged(&self, open: &Overview) -> Option<DrawnWindow> {
        let (id, rect) = self.overview_dragged_global(self.now())?;
        Some(DrawnWindow {
            id,
            rect: local(rect, &open.output),
            alpha: 1.0,
        })
    }

    /// The window that a drag takes and where it's drawn at `now`, in global logical
    /// coordinates, as `overview_dragged` has it.
    fn overview_dragged_global(
        &self,
        now: Duration,
    ) -> Option<(WindowId, Rectangle<i32, Logical>)> {
        let drag = self.policy.overview()?.drag.filter(|drag| drag.dragging)?;
        let slot = self.overview_slot(drag.window)?;
        let (grab, size) = match self.overview_shrink_of(drag.window) {
            Some(shrink) => {
                let to = self.overview_aim_size(shrink.aim, drag.window, slot.size);
                (shrink.grab, shrink.size(to, now))
            }
            None => (drag::grab_point(slot, drag.from), slot.size.to_f64()),
        };
        Some((drag.window, drag::about_grab(drag.pointer, grab, size)))
    }

    /// The size of window `id` as a drag takes it (249).
    fn overview_shrink_of(&self, id: WindowId) -> Option<Shrink> {
        self.overview_shrink.filter(|shrink| shrink.window == id)
    }

    /// Window `id`'s place in the layout of the output that lists it, in global logical
    /// coordinates.
    fn overview_slot(&self, id: WindowId) -> Option<Rectangle<i32, Logical>> {
        self.overview.iter().find_map(|open| {
            let place = open.laid.windows.iter().position(|listed| *listed == id)?;
            let slot = open.laid.layout.slots.get(place)?.window;
            Some(Rectangle::new(
                slot.loc + output_geometry(&open.output).loc,
                slot.size,
            ))
        })
    }

    /// The size window `id` is drawn at for `aim` (249), with `layout` its size in the
    /// layout: that one, or its real size times a cell's scale.
    fn overview_aim_size(
        &self,
        aim: Aim,
        id: WindowId,
        layout: Size<i32, Logical>,
    ) -> Size<f64, Logical> {
        match aim {
            Aim::Layout => layout.to_f64(),
            Aim::Scale(scale) => {
                let real = self.policy.frame_rect(id).unwrap_or_default().size.to_f64();
                Size::from((real.w * scale, real.h * scale))
            }
        }
    }

    /// Aims the dragged window's size at the pointer's `location` (249): a cell's scale over
    /// the row of cells of an output's bar, and its layout size anywhere else. Its size
    /// eases there from where it's drawn, with `overview-animation` on. The point it's
    /// grabbed by is kept from the drag's start, whatever the layout does after.
    fn aim_overview_drag(&mut self, location: Point<f64, Logical>) {
        let now = self.now();
        let drag = self.policy.overview().and_then(|open| open.drag);
        let Some(drag) = drag.filter(|drag| drag.dragging) else {
            self.overview_shrink = None;
            return;
        };
        let Some(slot) = self.overview_slot(drag.window) else {
            return;
        };
        let aim = match self.overview_at(location) {
            Some((open, at)) if open.bar.over_cells(at) => {
                let output = output_geometry(&open.output).size;
                Aim::Scale(drag::cell_scale(open.bar.cells[0], output))
            }
            _ => Aim::Layout,
        };
        let shrink = self.overview_shrink_of(drag.window).unwrap_or_else(|| {
            let grab = drag::grab_point(slot, drag.from);
            Shrink::new(drag.window, grab, slot.size.to_f64(), now)
        });
        let sizes = |aim| self.overview_aim_size(aim, drag.window, slot.size);
        let aimed = shrink.aimed(aim, sizes, now, self.settings.overview_animation);
        if aimed != shrink {
            self.queue_redraw_all();
        }
        self.overview_shrink = Some(aimed);
    }

    /// The drag ended with the window drawn at `dropped`, in global logical coordinates, if
    /// it was a drag: policy has done what the drop does, and the layouts are as it left
    /// them. With `overview-animation` on the window eases from there to where it's drawn
    /// next (249): its slot in the layout that lists it, or, listed in none, its place in
    /// the picture of its workspace's cell on its output.
    fn overview_dropped(&mut self, dropped: Option<(WindowId, Rectangle<i32, Logical>)>) {
        self.overview_shrink = None;
        let Some((id, from)) = dropped.filter(|_| self.settings.overview_animation) else {
            return;
        };
        let listed = self
            .overview
            .iter()
            .any(|open| open.laid.windows.contains(&id));
        let cell = match listed {
            true => None,
            false => {
                let output = self.policy.output_showing(id);
                let workspace = self.policy.workspace_of(id);
                let Some(cell) = output.zip(workspace) else {
                    return;
                };
                Some(cell)
            }
        };
        self.overview_drop = Some(Dropping::new(id, from, cell, self.now()));
        self.queue_redraw_all();
    }

    /// Where a dropped window goes (249), in global logical coordinates: its slot in the
    /// layout, or its real rectangle scaled into its workspace's cell, as the cell's
    /// picture draws it. None once it's in neither.
    fn overview_drop_to(&self, drop: &Dropping) -> Option<Rectangle<i32, Logical>> {
        let Some((output, workspace)) = drop.cell else {
            return self.overview_slot(drop.window);
        };
        let open = self
            .overview
            .iter()
            .find(|open| output_id(&open.output) == output)?;
        let cell = *open.bar.cells.get(workspace)?;
        let geometry = output_geometry(&open.output);
        let frame = self.policy.frame_rect(drop.window)?;
        let rect = drag::scaled_into(local(frame, &open.output), cell, geometry.size);
        Some(Rectangle::new(rect.loc + geometry.loc, rect.size))
    }

    /// The drop on its way at `now` (249).
    fn overview_drop_at(&self, now: Duration) -> Option<Dropping> {
        self.overview_drop.filter(|drop| !drop.is_over(now))
    }

    /// The window a drop is on its way with at `now` (249), which the layouts and the
    /// cells' pictures leave out until it's there.
    pub(crate) fn overview_dropping_at(&self, now: Duration) -> Option<WindowId> {
        self.overview_drop_at(now).map(|drop| drop.window)
    }

    /// The windows the picture of `open`'s cell of `workspace` draws now, bottom to top:
    /// not a dropped one on its way there (249).
    pub(crate) fn overview_pictured(&self, open: &Overview, workspace: usize) -> Vec<WindowId> {
        let dropping = self.overview_dropping_at(self.now());
        let windows = open.cell_windows.get(workspace).into_iter().flatten();
        windows
            .copied()
            .filter(|id| dropping != Some(*id))
            .collect()
    }

    /// The dropped window on its way (249), from the top left corner of `open`'s output.
    pub(crate) fn overview_dropping(&self, open: &Overview) -> Option<DrawnWindow> {
        self.overview_dropping_with_target(open)
            .map(|(dropping, _)| dropping)
    }

    /// The dropped window on its way (249), and where it goes, from the top left corner of
    /// `open`'s output.
    pub(crate) fn overview_dropping_with_target(
        &self,
        open: &Overview,
    ) -> Option<(DrawnWindow, Rectangle<i32, Logical>)> {
        let now = self.now();
        let drop = self.overview_drop_at(now)?;
        let to = self.overview_drop_to(&drop)?;
        let dropping = DrawnWindow {
            id: drop.window,
            rect: local(drop.rect(to, now), &open.output),
            alpha: 1.0,
        };
        Some((dropping, local(to, &open.output)))
    }

    /// Whether the dragged window's size or a drop is on its way at `now` (249).
    fn overview_drag_runs(&self, now: Duration) -> bool {
        let shrinks = self.overview_shrink.is_some_and(|shrink| shrink.runs(now));
        shrinks || self.overview_dropping_at(now).is_some()
    }

    /// Cuts a drop to its end (249).
    fn end_overview_drop(&mut self) {
        if self.overview_drop.take().is_some() {
            self.queue_redraw_all();
        }
    }

    /// The workspace whose cell of `output`'s bar a drag of the Overview's is on (245),
    /// which is marked.
    pub(crate) fn overview_drop_target(&self, output: &Output) -> Option<usize> {
        let drag = self.policy.overview()?.drag?;
        drag.target.filter(|_| drag.onto == Some(output_id(output)))
    }

    /// Brings the compositor's side up to date with policy's Overview, in the refresh
    /// pass: lets go of it when policy has closed its own, and on each output policy lists
    /// (246) lays out the bar and the listed windows again, and those of the workspaces a
    /// Workspace slide there leaves (244), and draws the labels again if anything they're
    /// drawn from changed. One on an output that's gone goes with it. The session lock
    /// closes it, as it ends every Interaction.
    ///
    /// With the transition on (243), one that opens starts on its way open, from where
    /// the closing had it if it was on its way closed, and one that policy closes stays,
    /// on its way closed from where it is. Every output has the same, and one that comes
    /// while it runs takes it from where it is. The lock ends it at once.
    pub(crate) fn refresh_overview(&mut self) {
        let now = self.now();
        let locked = self.lock.is_locked();
        let Some(open) = self.policy.overview().filter(|_| !locked).cloned() else {
            if self.policy.overview().is_some() {
                self.policy.close_overview();
            }
            self.overview_closed(now);
            return;
        };
        // What policy lists on each output there is, in the order of the outputs.
        let listed: Vec<(Output, OverviewOutput)> = self
            .outputs
            .iter()
            .filter_map(|output| Some((output.clone(), open.on(output_id(output))?)))
            .collect();
        // The one drawn already, if it's open or closing: every output's is the same. An
        // opening starts here.
        let (transition, starts) = match self.overview.first() {
            Some(before) if !before.is_closing() => (before.transition, false),
            _ if !self.settings.overview_animation => (None, false),
            Some(closing) => (
                Some(Transition::new(Heading::Opening, closing.part(now), now)),
                true,
            ),
            None => (Some(Transition::held(Heading::Opening, 0.0, now)), true),
        };
        let began = Instant::now();
        let mut labels = (0, 0);
        let mut before = std::mem::take(&mut self.overview);
        let mut changed = before.len() != listed.len();
        // Every output's layout as it was, so that a window that joins one from another's is
        // told from one that opens.
        let was_laid: Vec<WindowId> = before
            .iter()
            .flat_map(|old| old.laid.windows.iter().copied())
            .collect();
        for (output, on) in listed {
            let old = before
                .iter()
                .position(|old| old.output == output)
                .map(|index| before.swap_remove(index));
            let was = old.as_ref().map(|old| WasLaid::of(old, now));
            let selected = open.selected.filter(|id| on.windows.contains(id));
            let (mut laid_out, fresh) =
                self.lay_out_overview(output, on, selected, transition, old, &mut labels);
            laid_out.scope = open.scope();
            laid_out.reflow =
                was.and_then(|was| self.overview_reflow(was, &laid_out, &was_laid, now));
            changed |= !fresh;
            self.overview.push(laid_out);
        }
        // A switch to the workspace a window was dropped on cuts its drop to its end (249).
        // So does the window's going, or a switch away from its slot's workspace: it's in no
        // layout and no picture then, and has nowhere to go.
        let cut = self.overview_drop.is_some_and(|drop| {
            let switched = drop.cell.is_some_and(|(output, workspace)| {
                self.policy.active_workspace_on(output) == workspace
            });
            switched || self.overview_drop_to(&drop).is_none()
        });
        if cut {
            self.end_overview_drop();
        }
        // A drag that ended, with its window's going or a switch, takes its size with it.
        if !open.drag.is_some_and(|drag| drag.dragging) {
            self.overview_shrink = None;
        }
        if starts {
            let (drawn, kept) = labels;
            let outputs = self.overview.len();
            self.overview_timing
                .laid_out(outputs, began.elapsed(), drawn, kept);
            // Its clock starts once the layouts are ready: what their labels took to draw,
            // a window's icon loaded or the font's first use, isn't taken from its first
            // frames. An opening that's held starts its clock later still, and its limit
            // counts from here.
            let ready = self.now();
            for open in &mut self.overview {
                open.transition = open.transition.map(|t| t.starting_at(ready));
                open.first_frame = FirstFrame::Undrawn;
            }
        }
        if changed {
            self.queue_redraw_all();
        }
    }

    /// The Overview on `output`, laid out from what policy lists there, `on`, with
    /// `selected` selected and with `transition`, and whether it's as `old`, the one laid
    /// out there before, had it. What `old` drew and the ids of its elements are kept.
    /// `labels` counts the labels drawn anew and those kept, for the timing.
    fn lay_out_overview(
        &mut self,
        output: Output,
        on: OverviewOutput,
        selected: Option<WindowId>,
        transition: Option<Transition>,
        old: Option<Overview>,
        labels: &mut (usize, usize),
    ) -> (Overview, bool) {
        let now = self.now();
        let policy = &self.policy;
        let workspace = on.workspace;
        let size = output_geometry(&output).size;
        let bar = Bar::new(size, policy.workspace_count());
        let area = bar.area_beneath(size);
        let slide = self.workspace_slides.on(output_id(&output), now);
        let on_output = |id: &WindowId| policy.output_showing(*id) == Some(output_id(&output));
        let cell_windows: Vec<Vec<WindowId>> = (0..bar.cells.len())
            .map(|cell| {
                let stacking = policy.stacking(cell).into_iter();
                let shown = stacking.filter(|id| on_output(id) && !policy.is_minimized(*id));
                shown.collect()
            })
            .collect();
        let mut planned = vec![self.plan_overview(&output, workspace, on.windows, selected, area)];
        for left in slide
            .map(|slide| slide.left_workspaces())
            .unwrap_or_default()
        {
            if left != workspace {
                let windows = self.policy.overview_windows_of(output_id(&output), left);
                planned.push(self.plan_overview(&output, left, windows, None, area));
            }
        }
        let fresh = old.as_ref().is_some_and(|open| {
            open.transition == transition
                && open.bar == bar
                && open.cell_windows == cell_windows
                && open.active == workspace
                && planned.len() == open.layouts().count()
                && planned
                    .iter()
                    .zip(open.layouts())
                    .all(|(plan, laid)| plan.matches(laid))
        });
        let first_frame = old
            .as_ref()
            .map_or(FirstFrame::Undrawn, |old| old.first_frame);
        let (dim, mut cell_backs, mark, target_mark, next_bar_frames, mut previous) = match old {
            Some(old) => {
                let mut previous = vec![old.laid];
                previous.extend(old.leaving);
                (
                    old.dim,
                    old.cell_backs,
                    old.mark,
                    old.target_mark,
                    old.next_bar_frames,
                    previous,
                )
            }
            None => (
                Id::new(),
                Vec::new(),
                [Id::new(), Id::new(), Id::new(), Id::new()],
                Id::new(),
                now + BAR_FRAME_INTERVAL,
                Vec::new(),
            ),
        };
        cell_backs.resize_with(bar.cells.len(), Id::new);
        let mut layouts = planned.into_iter().map(|plan| {
            // Labels drawn from the same are drawn already.
            let same = previous
                .iter_mut()
                .find(|laid| laid.workspace == plan.workspace && laid.drawn == plan.drawn);
            let labels = match same {
                Some(laid) => {
                    labels.1 += laid.labels.len();
                    std::mem::take(&mut laid.labels)
                }
                None => {
                    labels.0 += plan.drawn.labels.len();
                    self.draw_overview_labels(&plan.drawn)
                }
            };
            Laid {
                workspace: plan.workspace,
                windows: plan.windows,
                layout: plan.layout,
                labels,
                drawn: plan.drawn,
            }
        });
        let laid = layouts.next().expect("the active workspace is laid out");
        let leaving = layouts.collect();
        let laid_out = Overview {
            output,
            bar,
            laid,
            leaving,
            dim,
            cell_backs,
            cell_windows,
            active: workspace,
            mark,
            target_mark,
            next_bar_frames,
            transition,
            reflow: None,
            scope: OverviewScope::Workspace,
            first_frame,
        };
        (laid_out, fresh)
    }

    /// The windows of `new`'s layout of the active workspace on their way to their new
    /// slots there (250), as `was` had the layout before: from where they were drawn, when
    /// a window left the layout, or joined it from another output's, which `was_laid` lists
    /// with this one's, with `overview-animation` on. A window that opens joins its slot at
    /// once and the others take theirs, as before (242). While they're on their way,
    /// another change of the layout eases each from where it's drawn to its newest slot, and
    /// with no change they go on. A switch of the output's workspace cuts it to its end, as
    /// does a closing that the layout was on its way with.
    ///
    /// A change of scope (CMN-185) eases the windows that stay too, and those that join
    /// fade in at their slots and those that leave fade out where they were drawn.
    fn overview_reflow(
        &self,
        was: WasLaid,
        new: &Overview,
        was_laid: &[WindowId],
        now: Duration,
    ) -> Option<Reflow> {
        if was.workspace != new.laid.workspace || was.closing {
            return None;
        }
        let windows = &new.laid.windows;
        let left = was.slots.iter().any(|(id, _)| !windows.contains(id));
        let joined = windows
            .iter()
            .filter(|id| !was.slots.iter().any(|(was, _)| was == *id));
        let moved_in = joined.clone().any(|id| was_laid.contains(id));
        let rescoped = was.scope != new.scope;
        let slots = new.laid.windows.iter().zip(&new.laid.layout.slots);
        let slots: Vec<_> = slots.map(|(id, slot)| (*id, slot.window)).collect();
        let reflow = match was.reflow {
            Some(reflow) if slots == was.slots && !rescoped => return Some(reflow),
            Some(_) => true,
            None => left || moved_in || rescoped,
        };
        if !reflow || !self.settings.overview_animation {
            return None;
        }
        let (stay, gone) = was
            .places
            .into_iter()
            .partition(|(id, _)| windows.contains(id));
        if !rescoped {
            return Some(Reflow::new(stay, now));
        }
        let joined = joined.copied().collect();
        Some(Reflow::rescoped(stay, joined, gone, now))
    }

    /// Policy has no Overview open: the one drawn goes on its way closed from where it is
    /// on each output that's still there, with the transition on, or goes at once. One
    /// that's closing already goes on, and the lock ends it.
    fn overview_closed(&mut self, now: Duration) {
        // Its closing cuts a drop to its end (249).
        self.overview_shrink = None;
        self.overview_drop = None;
        let outputs = &self.outputs;
        let before = self.overview.len();
        self.overview
            .retain(|shown| outputs.contains(&shown.output));
        let Some(first) = self.overview.first() else {
            if before > 0 {
                self.queue_redraw_all();
            }
            return;
        };
        let animated = self.settings.overview_animation && !self.lock.is_locked();
        if animated && first.is_closing() {
            if before > self.overview.len() {
                self.queue_redraw_all();
            }
            return;
        }
        if animated {
            let closing = Transition::new(Heading::Closing, first.part(now), now);
            for shown in &mut self.overview {
                shown.transition = Some(closing);
                // The windows go back from their new slots (250).
                shown.reflow = None;
            }
        } else {
            self.overview.clear();
        }
        self.queue_redraw_all();
    }

    /// When the running transition is over: `deadlines` then draws the Overview at rest
    /// (174).
    ///
    /// So is a dragged window's size or a drop's (249), and the layout's windows' way to
    /// their new slots (250).
    pub(crate) fn overview_transition_deadline(&self) -> Option<Duration> {
        let now = self.now();
        let open = self.overview.first()?;
        let transition = open.transition.map(|transition| transition.end());
        let shrink = self.overview_shrink.filter(|shrink| shrink.runs(now));
        let drop = self.overview_drop.map(|drop| drop.end());
        let reflows = self.overview.iter();
        let reflows = reflows.filter_map(|open| open.reflow.as_ref().map(Reflow::end));
        [transition, shrink.map(|shrink| shrink.end()), drop]
            .into_iter()
            .flatten()
            .chain(reflows)
            .min()
    }

    /// Ends the transition if it's over at `now`, and draws the outputs at rest: the
    /// Overview open, or gone once it's closed. While it runs, its outputs are drawn again
    /// at each of their frames (`State::redraw_queued_outputs`). An opening held for its
    /// limit starts its clock instead.
    ///
    /// A dragged window that's at its size, or a drop that's over (249), is drawn at rest,
    /// and so are a layout's windows at their new slots (250).
    pub(crate) fn overview_transition_elapse(&mut self, now: Duration) {
        let mut settled = false;
        for open in &mut self.overview {
            if open
                .reflow
                .as_ref()
                .is_some_and(|reflow| reflow.is_over(now))
            {
                open.reflow = None;
                settled = true;
            }
        }
        if settled {
            self.queue_redraw_all();
        }
        if let Some((shrink, arrived)) = self.overview_shrink.map(|shrink| shrink.settled(now)) {
            self.overview_shrink = Some(shrink);
            if arrived {
                self.queue_redraw_all();
            }
        }
        if self.overview_drop.is_some_and(|drop| drop.is_over(now)) {
            self.end_overview_drop();
        }
        let Some(open) = self.overview.first() else {
            return;
        };
        if open
            .transition
            .is_some_and(|t| t.is_held() && now >= t.end())
        {
            self.start_overview_transition(now);
            return;
        }
        let Some(transition) = open.transition.filter(|t| t.is_over(now)) else {
            return;
        };
        if transition.heading == Heading::Opening {
            for open in &mut self.overview {
                open.transition = None;
            }
        } else {
            self.overview.clear();
        }
        self.queue_redraw_all();
    }

    /// `output` drew a frame: its first of a held opening, if the opening's held.
    pub(crate) fn overview_frame_drawn(&mut self, output: &Output) {
        let open = self.overview.iter_mut().find(|open| open.output == *output);
        if let Some(open) = open
            && open.transition.is_some_and(|t| t.is_held())
            && open.first_frame == FirstFrame::Undrawn
        {
            open.first_frame = FirstFrame::Drawn;
        }
    }

    /// `output` takes a new frame at `now`, so the last one it drew is on screen. A held
    /// opening's clock starts once every output's first frame is: the frames after it are
    /// drawn from there, however long the GPU took over it.
    pub(crate) fn overview_output_ready(&mut self, output: &Output, now: Duration) {
        let open = self.overview.iter_mut().find(|open| open.output == *output);
        let Some(open) = open.filter(|open| open.transition.is_some_and(|t| t.is_held())) else {
            return;
        };
        if open.first_frame == FirstFrame::Drawn {
            open.first_frame = FirstFrame::Shown;
        }
        let shown = |open: &Overview| open.first_frame == FirstFrame::Shown;
        if self.overview.iter().all(shown) {
            self.start_overview_transition(now);
        }
    }

    /// Starts a held opening's clock at `now`, on every output.
    fn start_overview_transition(&mut self, now: Duration) {
        for open in &mut self.overview {
            open.transition = open.transition.map(|t| t.started(now));
        }
        self.overview_timing.started();
    }

    /// Whether the Overview's transition runs on `output` at `now`, or a dragged window's
    /// size or a drop does there (249), or its layout's windows are on their way to their
    /// new slots (250).
    pub(crate) fn overview_transition_on(&self, output: &Output, now: Duration) -> bool {
        let open = self.overview_on(output);
        let transition = open.and_then(|open| open.transition);
        let runs = transition.is_some_and(|transition| !transition.is_over(now));
        let reflows = open.is_some_and(|open| open.reflowing(now).is_some());
        runs || reflows || (open.is_some() && self.overview_drag_runs(now))
    }

    /// `windows` of `workspace` on `output` laid out in `area`, with `selected` selected,
    /// and what their labels are drawn from.
    fn plan_overview(
        &self,
        output: &Output,
        workspace: usize,
        windows: Vec<WindowId>,
        selected: Option<WindowId>,
        area: Rectangle<i32, Logical>,
    ) -> Planned {
        let origin = output_geometry(output).loc;
        let frames: Vec<Rectangle<i32, Logical>> = windows
            .iter()
            .map(|id| {
                let frame = self.policy.frame_rect(*id).unwrap_or_default();
                Rectangle::new(frame.loc - origin, frame.size)
            })
            .collect();
        let layout = Layout::new(&frames, area);
        let drawn = self.overview_label_inputs(output, &windows, selected, &layout);
        Planned {
            workspace,
            windows,
            layout,
            drawn,
        }
    }

    /// What the labels of `windows` are drawn from on `output`, in `layout`, with
    /// `selected` selected.
    fn overview_label_inputs(
        &self,
        output: &Output,
        windows: &[WindowId],
        selected: Option<WindowId>,
        layout: &Layout,
    ) -> Drawn {
        let labels = windows
            .iter()
            .zip(&layout.slots)
            .map(|(id, slot)| {
                let mapped = self.policy.window(*id);
                LabelInputs {
                    rect: slot.label,
                    title: mapped.map(|mapped| mapped.title()).unwrap_or_default(),
                    icon: mapped.map_or(IconSource::AppId(None), |mapped| mapped.icon_source()),
                    selected: selected == Some(*id),
                }
            })
            .collect();
        Drawn {
            labels,
            scale: output.current_scale().fractional_scale().ceil() as i32,
            colors: self.palette.palette().variant(Variant::Default).clone(),
            font: self.settings.interface.font_name.clone(),
            generation: self.frame_context.generation(),
        }
    }

    /// Draws each label of `drawn`, with its window's icon loaded at the size it shows at.
    fn draw_overview_labels(&mut self, drawn: &Drawn) -> Vec<Option<MemoryRenderBuffer>> {
        let font = pango::FontDescription::from_string(&drawn.font);
        drawn
            .labels
            .iter()
            .enumerate()
            .map(|(index, inputs)| {
                if inputs.rect.is_empty() {
                    return None;
                }
                let start = Instant::now();
                let icon =
                    self.frame_context
                        .window_icon(&inputs.icon, draw::ICON_SIZE, drawn.scale);
                let loaded = Instant::now();
                let context = self.frame_context.text_context();
                let title = Title::new(context, &font, 1.0, &inputs.title);
                let laid_out = Instant::now();
                let found = icon.is_some();
                let label = Label {
                    icon,
                    title,
                    selected: inputs.selected,
                };
                let buffer = draw::draw(inputs.rect, &label, &drawn.colors, drawn.scale)
                    .inspect_err(|err| tracing::warn!("can't draw the overview's label: {err}"))
                    .ok();
                timing::label(
                    index,
                    loaded - start,
                    laid_out - loaded,
                    laid_out.elapsed(),
                    found,
                );
                buffer
            })
            .collect()
    }

    /// A key went down while the Overview may be open. Returns what the keyboard path does
    /// with it, or `None` if the Overview is closed and the key is the other stages'. Esc
    /// and the Overview's own shortcut close it. An arrow key that's no binding moves the
    /// selection in the layout, and Enter chooses the selected window (242). A workspace
    /// binding switches the Active output's workspace as ever (244), with no Workspace
    /// popup since the seat has an Interaction already. A media key,
    /// which is an accelerator grab of a key with an `XF86` keysym, is its grabber's as
    /// ever. Every other key but a modifier is swallowed, the other System shortcuts among
    /// them, and reaches no client. A modifier goes on to the client, as with the Window
    /// switcher open.
    pub(crate) fn overview_key_pressed(
        &mut self,
        keycode: Keycode,
        mods: &ModifiersState,
        keysym: &KeysymHandle<'_>,
    ) -> Option<FilterResult<Option<Bound>>> {
        self.policy.overview()?;
        if keysym.modified_sym().is_modifier_key() {
            return Some(FilterResult::Forward);
        }
        self.suppressed_keys.insert(keycode);
        // Keys are named as the first layout does, like the bindings.
        let keysyms = press_keysyms(keysym);
        let binding = self.settings.bindings.binding_for(mods, &keysyms);
        let sym = keysym.raw_latin_sym_or_raw_current_sym();
        // Esc during a drag ends the drag, and the Overview stays open (245). The window
        // eases back to its slot (249).
        let dragged = self.overview_dragged_global(self.now());
        if sym == Some(Keysym::Escape) && self.policy.overview_end_drag() {
            self.refresh_overview();
            self.overview_dropped(dragged);
            return Some(FilterResult::Intercept(None));
        }
        if sym == Some(Keysym::Escape) {
            self.policy.close_overview();
            return Some(FilterResult::Intercept(None));
        }
        // Its own scope's shortcut closes it, and the other's changes it in place
        // (CMN-185).
        if let Some(Binding::Policy(action)) = binding
            && OverviewScope::of(action).is_some()
        {
            let now = self.now();
            self.policy.action(SEAT, action, now);
            return Some(FilterResult::Intercept(None));
        }
        if let Some(Binding::Policy(action)) = binding
            && switches_workspace(action)
        {
            return Some(FilterResult::Intercept(Some(Bound::Action(action))));
        }
        if binding.is_none() {
            let direction = match sym {
                Some(Keysym::Left) => Some(Direction::Left),
                Some(Keysym::Right) => Some(Direction::Right),
                Some(Keysym::Up) => Some(Direction::Up),
                Some(Keysym::Down) => Some(Direction::Down),
                _ => None,
            };
            if let Some(direction) = direction {
                self.overview_select(direction);
                return Some(FilterResult::Intercept(None));
            }
            if matches!(sym, Some(Keysym::Return | Keysym::KP_Enter)) {
                let now = self.now();
                self.policy.overview_enter(now);
                self.refresh_overview();
                return Some(FilterResult::Intercept(None));
            }
        }
        let media_key = self.accelerator_grabs.pressed_media_key(mods, &keysyms);
        let bound = media_key.map(|(action, owner)| Bound::Accelerator {
            owner: owner.to_owned(),
            action,
        });
        Some(FilterResult::Intercept(bound))
    }

    /// An arrow key: moves policy's selection `direction`'s way in the layout of the
    /// output it's on (246), as it is now.
    fn overview_select(&mut self, direction: Direction) {
        // The layout as policy's list is now: a window may have opened or closed since the
        // refresh pass.
        self.refresh_overview();
        let Some(output) = self.policy.overview().map(|open| open.output) else {
            return;
        };
        let open = self
            .overview
            .iter()
            .find(|open| output_id(&open.output) == output);
        let Some(open) = open else {
            return;
        };
        let places: Vec<_> = open
            .laid
            .layout
            .slots
            .iter()
            .map(|slot| slot.window)
            .collect();
        self.policy.overview_select(direction, &places);
        self.refresh_overview();
    }
}

/// `rect`, in global logical coordinates, from `output`'s top left corner.
fn local(rect: Rectangle<i32, Logical>, output: &Output) -> Rectangle<i32, Logical> {
    Rectangle::new(rect.loc - output_geometry(output).loc, rect.size)
}

/// Whether `action` is one of the workspace bindings that switch the Active output's
/// workspace, which work with the Overview open (244). Those that move a window there too
/// don't.
fn switches_workspace(action: Action) -> bool {
    matches!(
        action,
        Action::SwitchToWorkspace(_)
            | Action::SwitchToWorkspaceLeft
            | Action::SwitchToWorkspaceRight
            | Action::SwitchToWorkspaceUp
            | Action::SwitchToWorkspaceDown
            | Action::SwitchToWorkspacePrev
    )
}

impl Caminito {
    /// How far from its places `laid`, one of `open`'s layouts, is drawn now, once for each
    /// place: where the Workspace slide on its output has its workspace (244), or at its
    /// places with no slide. A layout that a slide which is over left isn't drawn, until
    /// the refresh pass lets go of it.
    pub(crate) fn overview_shifts(&self, open: &Overview, laid: &Laid) -> Vec<Point<i32, Logical>> {
        let now = self.now();
        match self.workspace_slides.on(output_id(&open.output), now) {
            Some(slide) => slide.workspace_shifts(laid.workspace, now),
            None if laid.workspace == open.laid.workspace => vec![Point::default()],
            None => Vec::new(),
        }
    }

    /// The windows the open Overview shows in its bars alone (244): those whose main
    /// output is one it's on and that are in none of its layouts there. A minimised window
    /// of another workspace, which nothing shows, is among them. None while it's closed.
    pub(crate) fn overview_bar_only_windows(&self) -> Vec<WindowId> {
        self.overview
            .iter()
            .flat_map(|open| self.bar_only_windows_of(open))
            .collect()
    }

    /// The windows `open` shows in its bar alone, on its output.
    fn bar_only_windows_of(&self, open: &Overview) -> Vec<WindowId> {
        self.policy
            .windows()
            .filter(|mapped| mapped.main_output() == Some(&open.output))
            .map(|mapped| mapped.id())
            .filter(|id| !open.layouts().any(|laid| laid.windows.contains(id)))
            .collect()
    }

    /// When the windows that only the Overview's bar shows next get frame callbacks, on
    /// the first output that's due of those that have one, while it's open.
    pub(crate) fn overview_deadline(&self) -> Option<Duration> {
        let waiting = self.overview.iter();
        let waiting = waiting.filter(|open| !self.bar_only_windows_of(open).is_empty());
        waiting.map(|open| open.next_bar_frames).min()
    }

    /// Sends the windows that only the Overview's bar shows their frame callbacks on each
    /// output where they're due at `now`: at [`BAR_FRAME_INTERVAL`], in place of each frame
    /// of the output, which the windows of the layout get theirs at (`State::post_render`).
    pub(crate) fn overview_elapse(&mut self, now: Duration) {
        // What a client is told, as after a frame.
        let time = self.clock.now();
        for open in &self.overview {
            if now < open.next_bar_frames {
                continue;
            }
            let output = &open.output;
            for id in self.bar_only_windows_of(open) {
                if let Some(mapped) = self.policy.window(id) {
                    let main = |_: &_, _: &_| Some(output.clone());
                    mapped.window().send_frame(output, time, None, main);
                }
            }
        }
        for open in &mut self.overview {
            if now >= open.next_bar_frames {
                open.next_bar_frames = now + BAR_FRAME_INTERVAL;
            }
        }
    }
}

impl State {
    /// The shortcut of the Overview or of the Application overview, as `scope` says,
    /// fired: it closes the open one of that scope, changes the other to it in place
    /// (CMN-185), or opens one on the Active output. Nothing opens during a move or
    /// resize, and a client's open menu is dismissed first, as for the Window switcher.
    pub(crate) fn toggle_overview(&mut self, scope: OverviewScope, time: InputTime) {
        self.toggle_overview_on(None, scope, time);
    }

    /// `toggle_overview`, with the selection on `selection_on` if it opens one, as a Hot
    /// corner of that output asks (CMN-183).
    pub(crate) fn toggle_overview_on(
        &mut self,
        selection_on: Option<OutputId>,
        scope: OverviewScope,
        time: InputTime,
    ) {
        // The pointer is held for a move or resize, which outlasts one that Policy ended
        // itself. Policy refuses for the seat's Interaction (187).
        if self.caminito.window_grab.is_some() {
            return;
        }
        self.dismiss_menus(time);
        let opens = self.caminito.policy.overview().is_none();
        self.caminito.overview_timing.pressed(opens);
        self.caminito.policy.overview_on(SEAT, selection_on, scope);
    }

    /// A pointer button with the pointer at `location`. Returns whether its press was the
    /// Overview's, and so no client's: while the Overview is open, a press of the left
    /// button on a window of a layout chooses that window, one of the middle button asks
    /// it to close and leaves the Overview open (242), one of the left button on a
    /// workspace of a bar switches that bar's output there (244, 246), and a press of any button on
    /// the empty area closes the Overview, as does that of a button with the Overview's
    /// shortcut. A button bound to a workspace switch does what its key does wherever the
    /// pointer is (248): it switches the Active output's workspace, and the Overview stays
    /// open. Another button on a workspace of the bar does nothing. The release of such a
    /// press is swallowed with the Window switcher's.
    ///
    /// A press of the left button on a window is held instead (245): its release chooses
    /// the window if the pointer stayed within `drag-threshold`, and drops it on the cell
    /// under the pointer, or on nothing, if it went past. That release is the Overview's,
    /// so it's asked here before the Window switcher swallows it.
    pub(crate) fn overview_button(
        &mut self,
        button: u32,
        state: ButtonState,
        location: Point<f64, Logical>,
        time: InputTime,
    ) -> bool {
        let caminito = &mut self.caminito;
        if state == ButtonState::Released {
            if button != BTN_LEFT || !caminito.overview_press_held() {
                return false;
            }
            caminito.overview_motion(location);
            let now = caminito.now();
            let dragged = caminito.overview_dragged_global(now);
            caminito.policy.overview_release(now);
            caminito.refresh_overview();
            caminito.overview_dropped(dragged);
            caminito.suppressed_buttons.remove(&button);
            return true;
        }
        if caminito.policy.overview().is_none() {
            return false;
        }
        // The layout as policy's list is now: a window may have closed since the refresh
        // pass, and its place would be another's.
        caminito.refresh_overview();
        if caminito.overview.is_empty() {
            return false;
        }
        caminito.suppressed_buttons.insert(button);
        let mods = caminito.seat.get_keyboard().unwrap().modifier_state();
        let binding = caminito.bound_to_button(button, &mods);
        let shortcut = binding.and_then(|(_, binding)| match binding {
            Binding::Policy(action) => OverviewScope::of(action),
            _ => None,
        });
        let switch = binding.and_then(|(_, binding)| match binding {
            Binding::Policy(action) if switches_workspace(action) => Some(action),
            _ => None,
        });
        let cell = caminito.overview_cell_at(location);
        let window = caminito.overview_window_at(location);
        // A press on a dropped window, at its slot or where it's drawn on its way, cuts the
        // drop to its end (249).
        let now = caminito.now();
        let dropping = caminito.overview_dropping_at(now).filter(|id| {
            let drawn = caminito.overview_at(location).and_then(|(open, at)| {
                let drawn = caminito.overview_dropping(open)?;
                Some(drawn.rect.to_f64().contains(at))
            });
            window == Some(*id) || drawn == Some(true)
        });
        if dropping.is_some() {
            caminito.end_overview_drop();
        }
        match window {
            // Its own scope's closes it, and the other's changes it in place (CMN-185).
            _ if let Some(scope) = shortcut => caminito.policy.overview_on(SEAT, None, scope),
            // As the binding's key does (244): the Workspace popup doesn't show under the
            // Overview, and a drag whose window the switch takes away ends (245).
            _ if switch.is_some() => {
                if let Some(action) = switch {
                    self.run_action(action, time);
                }
            }
            _ if cell.is_some() => {
                if let Some((output, workspace)) = cell.filter(|_| button == BTN_LEFT) {
                    caminito.policy.overview_switch_on(output, workspace);
                }
            }
            Some(id) if button == BTN_LEFT => caminito.policy.overview_press(id, location),
            Some(id) if button == BTN_MIDDLE => caminito.policy.overview_close_window(id),
            Some(_) => {}
            None => caminito.policy.close_overview(),
        }
        self.caminito.refresh_overview();
        true
    }
}
