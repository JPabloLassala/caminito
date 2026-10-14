//! The Overview's side in the compositor (241, [ADR 0033]): a view of an output's active
//! workspace with its windows shrunk and set side by side, which a System shortcut opens.
//! Policy has whether it's open, its output, the windows listed and what a choice does,
//! since it's the seat's Interaction (187). Here are the keys and the clicks that drive it
//! while it's open, and what it's laid out and drawn from. `layout` has each window's
//! place, `bar` each workspace's cell, `draw` draws the labels, and `scene` puts the bar,
//! the windows, the labels and the dimming on the output, in place of what the output
//! shows otherwise.
//!
//! The arrow keys move policy's selection in the layout and Enter chooses it, and a middle
//! click on a window asks it to close (242). A window that opens or closes while it's open
//! joins the layout or leaves it at the next refresh pass, and the others take their new
//! places in that frame.
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
//! Every output at once is 246's.
//!
//! [ADR 0033]: ../../../docs/adr/0033-the-workspace-slide-and-the-overview-are-taken-from-plasma.md

pub(crate) mod bar;
pub(crate) mod draw;
pub(crate) mod layout;
pub(crate) mod transition;

use std::time::Duration;

use caminito_policy::{Action, Direction, PolicyWindow, WindowId};
use caminito_theme::{PaletteColors, Title, Variant, pango};
use smithay::backend::input::{ButtonState, InputTime, Keycode};
use smithay::backend::renderer::element::Id;
use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
use smithay::input::keyboard::{FilterResult, Keysym, KeysymHandle, ModifiersState};
use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle};

use self::bar::Bar;
use self::draw::Label;
use self::layout::Layout;
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

/// The compositor's side of the Overview, as the refresh pass last laid it out: while
/// policy has it open, and while it's drawn closing after (243).
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
    /// The transition, while it's on its way open or closed (243).
    pub(crate) transition: Option<Transition>,
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
        self.overview.as_ref().filter(|open| open.output == *output)
    }

    /// The window of the Overview's layout at `location`, in global logical coordinates:
    /// the one whose place or whose label it's on.
    fn overview_window_at(&self, location: Point<f64, Logical>) -> Option<WindowId> {
        let open = self.overview.as_ref()?;
        let origin = output_geometry(&open.output).loc.to_f64();
        let slot = open.laid.layout.slot_at(location - origin)?;
        open.laid.windows.get(slot).copied()
    }

    /// The workspace of the Overview's bar at `location`, in global logical coordinates:
    /// the one whose cell it's on (244).
    fn overview_cell_at(&self, location: Point<f64, Logical>) -> Option<usize> {
        let open = self.overview.as_ref()?;
        let origin = output_geometry(&open.output).loc.to_f64();
        open.bar.cell_at(location - origin)
    }

    /// The pointer moved to `location`, in global logical coordinates: policy hears it
    /// with the cell it's on, while a press is held on a window of the Overview (245).
    pub(crate) fn overview_motion(&mut self, location: Point<f64, Logical>) {
        let held = self
            .policy
            .overview()
            .is_some_and(|open| open.drag.is_some());
        if held {
            let cell = self.overview_cell_at(location);
            self.policy.overview_pointer(location, cell);
        }
    }

    /// The window that a drag of `open`'s takes (245), drawn under the pointer: at its
    /// place in the layout, as far from it as the pointer is from where the button went
    /// down. None while there's no drag, and for one past the threshold not yet.
    pub(crate) fn overview_dragged(&self, open: &Overview) -> Option<DrawnWindow> {
        let drag = self.policy.overview()?.drag.filter(|drag| drag.dragging)?;
        let place = open.laid.windows.iter().position(|id| *id == drag.window)?;
        let slot = open.laid.layout.slots.get(place)?;
        let moved = (drag.pointer - drag.from).to_i32_round();
        Some(DrawnWindow {
            id: drag.window,
            rect: Rectangle::new(slot.window.loc + moved, slot.window.size),
            alpha: 1.0,
        })
    }

    /// The workspace whose cell a drag of the Overview's is on (245), which is marked.
    pub(crate) fn overview_drop_target(&self) -> Option<usize> {
        self.policy.overview()?.drag?.target
    }

    /// Brings the compositor's side up to date with policy's Overview, in the refresh
    /// pass: lets go of it when policy has closed its own, lays out the bar and the listed
    /// windows again, and those of the workspaces a Workspace slide leaves (244), and draws
    /// the labels again if anything they're drawn from changed. The session lock closes
    /// it, as it ends every Interaction.
    ///
    /// With the transition on (243), one that opens starts on its way open, from where
    /// the closing had it if it was on its way closed, and one that policy closes stays,
    /// on its way closed from where it is. The lock ends it at once, and so does the end
    /// of its output.
    pub(crate) fn refresh_overview(&mut self) {
        let now = self.now();
        let locked = self.lock.is_locked();
        let policy = &self.policy;
        let open = policy.overview().filter(|_| !locked);
        let on = open.and_then(|open| {
            let mut outputs = self.outputs.iter();
            let output = outputs.find(|output| output_id(output) == open.output)?;
            Some((
                output.clone(),
                open.workspace,
                open.windows.clone(),
                open.selected,
            ))
        });
        let Some((output, workspace, windows, selected)) = on else {
            if self.policy.overview().is_some() {
                self.policy.close_overview();
            }
            self.overview_closed(now);
            return;
        };
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
        let mut planned = vec![self.plan_overview(&output, workspace, windows, selected, area)];
        for left in slide
            .map(|slide| slide.left_workspaces())
            .unwrap_or_default()
        {
            if left != workspace {
                let windows = self.policy.overview_windows_on(left);
                planned.push(self.plan_overview(&output, left, windows, None, area));
            }
        }
        // The one that was drawn there, if it's open or closing.
        let before = self
            .overview
            .as_ref()
            .filter(|before| before.output == output);
        let transition = match before {
            Some(before) if !before.is_closing() => before.transition,
            _ if !self.settings.overview_animation => None,
            Some(closing) => Some(Transition::new(Heading::Opening, closing.part(now), now)),
            None => Some(Transition::new(Heading::Opening, 0.0, now)),
        };
        let fresh = before.is_some_and(|open| {
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
        if fresh {
            return;
        }
        let old = self.overview.take().filter(|old| old.output == output);
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
                Some(laid) => std::mem::take(&mut laid.labels),
                None => self.draw_overview_labels(&plan.drawn),
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
        self.overview = Some(Overview {
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
        });
        self.queue_redraw_all();
    }

    /// Policy has no Overview open: the one drawn goes on its way closed from where it is,
    /// with the transition on and its output still there, or goes at once. One that's
    /// closing already goes on, and the lock ends it.
    fn overview_closed(&mut self, now: Duration) {
        let Some(shown) = &mut self.overview else {
            return;
        };
        let animated = self.settings.overview_animation
            && !self.lock.is_locked()
            && self.outputs.contains(&shown.output);
        if animated && shown.is_closing() {
            return;
        }
        if animated {
            let from = shown.part(now);
            shown.transition = Some(Transition::new(Heading::Closing, from, now));
        } else {
            self.overview = None;
        }
        self.queue_redraw_all();
    }

    /// When the running transition is over: `deadlines` then draws the Overview at rest
    /// (174).
    pub(crate) fn overview_transition_deadline(&self) -> Option<Duration> {
        let open = self.overview.as_ref()?;
        open.transition.map(|transition| transition.end())
    }

    /// Ends the transition if it's over at `now`, and draws the output at rest: the
    /// Overview open, or gone once it's closed. While it runs, its output is drawn again
    /// at each of its frames (`State::redraw_queued_outputs`).
    pub(crate) fn overview_transition_elapse(&mut self, now: Duration) {
        let Some(open) = &mut self.overview else {
            return;
        };
        let Some(transition) = open.transition.filter(|t| t.is_over(now)) else {
            return;
        };
        if transition.heading == Heading::Opening {
            open.transition = None;
        } else {
            self.overview = None;
        }
        self.queue_redraw_all();
    }

    /// Whether the Overview's transition runs on `output` at `now`.
    pub(crate) fn overview_transition_on(&self, output: &Output, now: Duration) -> bool {
        let open = self.overview_on(output);
        open.and_then(|open| open.transition)
            .is_some_and(|transition| !transition.is_over(now))
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
            .map(|inputs| {
                if inputs.rect.is_empty() {
                    return None;
                }
                let label = Label {
                    icon: self.frame_context.window_icon(
                        &inputs.icon,
                        draw::ICON_SIZE,
                        drawn.scale,
                    ),
                    title: Title::new(self.frame_context.text_context(), &font, 1.0, &inputs.title),
                    selected: inputs.selected,
                };
                draw::draw(inputs.rect, &label, &drawn.colors, drawn.scale)
                    .inspect_err(|err| tracing::warn!("can't draw the overview's label: {err}"))
                    .ok()
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
        // Esc during a drag ends the drag, and the Overview stays open (245).
        if sym == Some(Keysym::Escape) && self.policy.overview_end_drag() {
            self.refresh_overview();
            return Some(FilterResult::Intercept(None));
        }
        let closes =
            binding == Some(Binding::Policy(Action::Overview)) || sym == Some(Keysym::Escape);
        if closes {
            self.policy.close_overview();
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

    /// An arrow key: moves policy's selection `direction`'s way in the layout as it is now.
    fn overview_select(&mut self, direction: Direction) {
        // The layout as policy's list is now: a window may have opened or closed since the
        // refresh pass.
        self.refresh_overview();
        let Some(open) = &self.overview else {
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

    /// The windows the open Overview shows in its bar alone (244): those whose main output
    /// is its output and that are in none of its layouts. A minimised window of another
    /// workspace, which nothing shows, is among them. None while it's closed.
    pub(crate) fn overview_bar_only_windows(&self) -> Vec<WindowId> {
        let Some(open) = &self.overview else {
            return Vec::new();
        };
        self.policy
            .windows()
            .filter(|mapped| mapped.main_output() == Some(&open.output))
            .map(|mapped| mapped.id())
            .filter(|id| !open.layouts().any(|laid| laid.windows.contains(id)))
            .collect()
    }

    /// When the windows that only the Overview's bar shows next get frame callbacks, while
    /// it's open and has one.
    pub(crate) fn overview_deadline(&self) -> Option<Duration> {
        let open = self.overview.as_ref()?;
        let waiting = !self.overview_bar_only_windows().is_empty();
        waiting.then_some(open.next_bar_frames)
    }

    /// Sends the windows that only the Overview's bar shows their frame callbacks, if
    /// they're due at `now`: at [`BAR_FRAME_INTERVAL`], in place of each frame of the
    /// output, which the windows of the layout get theirs at (`State::post_render`).
    pub(crate) fn overview_elapse(&mut self, now: Duration) {
        let Some(open) = &self.overview else {
            return;
        };
        if now < open.next_bar_frames {
            return;
        }
        let output = open.output.clone();
        // What a client is told, as after a frame.
        let time = self.clock.now();
        for id in self.overview_bar_only_windows() {
            if let Some(mapped) = self.policy.window(id) {
                let main = |_: &_, _: &_| Some(output.clone());
                mapped.window().send_frame(&output, time, None, main);
            }
        }
        if let Some(open) = &mut self.overview {
            open.next_bar_frames = now + BAR_FRAME_INTERVAL;
        }
    }
}

impl State {
    /// The Overview's shortcut fired: it closes the open Overview, or opens one on the
    /// Active output. Nothing opens during a move or resize, and a client's open menu is
    /// dismissed first, as for the Window switcher.
    pub(crate) fn toggle_overview(&mut self, time: InputTime) {
        // The pointer is held for a move or resize, which outlasts one that Policy ended
        // itself. Policy refuses for the seat's Interaction (187).
        if self.caminito.window_grab.is_some() {
            return;
        }
        self.dismiss_menus(time);
        let now = self.caminito.now();
        self.caminito.policy.action(SEAT, Action::Overview, now);
    }

    /// A pointer button with the pointer at `location`. Returns whether its press was the
    /// Overview's, and so no client's: while the Overview is open, a press of the left
    /// button on a window of the layout chooses that window, one of the middle button asks
    /// it to close and leaves the Overview open (242), one of the left button on a
    /// workspace of the bar switches the output there (244), and a press of any button on
    /// the empty area closes the Overview, as does that of a button with the Overview's
    /// shortcut. Another button on a workspace of the bar does nothing. The release of such
    /// a press is swallowed with the Window switcher's.
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
    ) -> bool {
        let caminito = &mut self.caminito;
        if state == ButtonState::Released {
            let held = caminito
                .policy
                .overview()
                .is_some_and(|open| open.drag.is_some());
            if button != BTN_LEFT || !held {
                return false;
            }
            caminito.overview_motion(location);
            let now = caminito.now();
            caminito.policy.overview_release(now);
            caminito.refresh_overview();
            caminito.suppressed_buttons.remove(&button);
            return true;
        }
        if caminito.policy.overview().is_none() {
            return false;
        }
        // The layout as policy's list is now: a window may have closed since the refresh
        // pass, and its place would be another's.
        caminito.refresh_overview();
        if caminito.overview.is_none() {
            return false;
        }
        caminito.suppressed_buttons.insert(button);
        let mods = caminito.seat.get_keyboard().unwrap().modifier_state();
        let binding = caminito.bound_to_button(button, &mods);
        let is_shortcut = matches!(binding, Some((_, Binding::Policy(Action::Overview))));
        let cell = caminito.overview_cell_at(location);
        match caminito.overview_window_at(location) {
            _ if is_shortcut => caminito.policy.close_overview(),
            _ if cell.is_some() => {
                if let Some(workspace) = cell.filter(|_| button == BTN_LEFT) {
                    caminito.policy.overview_switch(workspace);
                }
            }
            Some(id) if button == BTN_LEFT => caminito.policy.overview_press(id, location),
            Some(id) if button == BTN_MIDDLE => caminito.policy.overview_close_window(id),
            Some(_) => {}
            None => caminito.policy.close_overview(),
        }
        caminito.refresh_overview();
        true
    }
}
