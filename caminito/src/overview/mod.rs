//! The Overview's side in the compositor (241, [ADR 0033]): a view of an output's active
//! workspace with its windows shrunk and set side by side, which a System shortcut opens.
//! Policy has whether it's open, its output, the windows listed and what a choice does,
//! since it's the seat's Interaction (187). Here are the keys and the clicks that drive it
//! while it's open, and what it's laid out and drawn from. `layout` has each window's
//! place, `draw` draws the labels, and `scene` puts the windows, the labels and the
//! dimming on the output, in place of what the output shows otherwise.
//!
//! The arrow keys move policy's selection in the layout and Enter chooses it, and a middle
//! click on a window asks it to close (242). A window that opens or closes while it's open
//! joins the layout or leaves it at the next refresh pass, and the others take their new
//! places in that frame.
//!
//! It opens and closes with its transition (243): each window moves between its real
//! place and its place in the layout, and the rest fades. Policy has it open or closed
//! from the first frame, so the keys and the clicks act on what policy has at once: here
//! the Overview stays for the closing's frames after policy has closed its own. Without
//! `overview-animation`, it opens and closes in one frame. Its bar of workspaces is 244's,
//! and every output at once 246's.
//!
//! [ADR 0033]: ../../../docs/adr/0033-the-workspace-slide-and-the-overview-are-taken-from-plasma.md

pub(crate) mod draw;
pub(crate) mod layout;
pub(crate) mod transition;

use std::time::Duration;

use caminito_policy::{Action, Direction, WindowId};
use caminito_theme::{PaletteColors, Title, Variant, pango};
use smithay::backend::input::{ButtonState, InputTime, Keycode};
use smithay::backend::renderer::element::Id;
use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
use smithay::input::keyboard::{FilterResult, Keysym, KeysymHandle, ModifiersState};
use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle};

use self::draw::Label;
use self::layout::Layout;
use self::transition::{Transition, between};
use crate::decoration::IconSource;
use crate::input::bindings::Binding;
use crate::input::{BTN_LEFT, BTN_MIDDLE, Bound, press_keysyms};
use crate::output::{output_geometry, output_id};
use crate::state::{Caminito, SEAT, State};

/// How much of black is over the wallpaper while the Overview is open.
pub(crate) const DIM: f32 = 0.5;

/// The compositor's side of the Overview, as the refresh pass last laid it out: while
/// policy has it open, and while it's drawn closing after (243).
pub(crate) struct Overview {
    /// The output it's on.
    pub(crate) output: Output,
    /// The windows policy listed, bottom to top.
    pub(crate) windows: Vec<WindowId>,
    /// Each listed window's place, from the output's top left corner, in `windows`' order.
    pub(crate) layout: Layout,
    /// Each listed window's label as last drawn, in `windows`' order. A label with no room
    /// has none, and neither has one cairo couldn't draw.
    pub(crate) labels: Vec<Option<MemoryRenderBuffer>>,
    /// What the labels were drawn from, to draw them again only when that changes.
    drawn: Drawn,
    /// The dimming's render element, the same one at every frame.
    pub(crate) dim: Id,
    /// The transition, while it's on its way open or closed (243).
    pub(crate) transition: Option<Transition>,
}

impl Overview {
    /// How open it's drawn at `now`, from 0, closed, to 1, open.
    pub(crate) fn part(&self, now: Duration) -> f64 {
        self.transition
            .map_or(1.0, |transition| transition.part(now))
    }

    /// Whether it's drawn on its way closed, after policy has closed it.
    fn is_closing(&self) -> bool {
        self.transition
            .is_some_and(|transition| !transition.opening)
    }
}

/// One window as the Overview draws it at one time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DrawnWindow {
    pub(crate) id: WindowId,
    /// Where it's drawn, from the output's top left corner.
    pub(crate) rect: Rectangle<i32, Logical>,
    /// Its place in the layout.
    pub(crate) slot: usize,
    /// How opaque it's drawn.
    pub(crate) alpha: f32,
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
        let slot = open.layout.slot_at(location - origin)?;
        open.windows.get(slot).copied()
    }

    /// Brings the compositor's side up to date with policy's Overview, in the refresh
    /// pass: lays the listed windows out again, and draws the labels again if anything
    /// they're drawn from changed. The session lock closes it, as it ends every
    /// Interaction.
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
            Some((output.clone(), open.windows.clone(), open.selected))
        });
        let Some((output, windows, selected)) = on else {
            if self.policy.overview().is_some() {
                self.policy.close_overview();
            }
            self.overview_closed(now);
            return;
        };
        let geometry = output_geometry(&output);
        let frames: Vec<Rectangle<i32, Logical>> = windows
            .iter()
            .map(|id| {
                let frame = policy.frame_rect(*id).unwrap_or_default();
                Rectangle::new(frame.loc - geometry.loc, frame.size)
            })
            .collect();
        let layout = Layout::new(&frames, Rectangle::from_size(geometry.size));
        let drawn = self.overview_label_inputs(&output, &windows, selected, &layout);
        // The one that was drawn there, if it's open or closing.
        let before = self
            .overview
            .take()
            .filter(|before| before.output == output);
        let transition = match &before {
            Some(before) if !before.is_closing() => before.transition,
            _ if !self.settings.overview_animation => None,
            Some(closing) => Some(Transition::new(true, closing.part(now), now)),
            None => Some(Transition::new(true, 0.0, now)),
        };
        let fresh = before.as_ref().is_some_and(|before| {
            before.windows == windows
                && before.layout == layout
                && before.drawn == drawn
                && before.transition == transition
        });
        if fresh {
            self.overview = before;
            return;
        }
        let labels = self.draw_overview_labels(&drawn);
        let dim = before.map_or_else(Id::new, |before| before.dim);
        self.overview = Some(Overview {
            output,
            windows,
            layout,
            labels,
            drawn,
            dim,
            transition,
        });
        self.queue_redraw_all();
    }

    /// Policy has no Overview open: the one drawn goes on its way closed from where it is,
    /// with the transition on and its output still there, or goes at once. One that's
    /// closing already goes on, and the lock ends it.
    fn overview_closed(&mut self, now: Duration) {
        let Some(drawn) = &mut self.overview else {
            return;
        };
        let output_is_there = self.outputs.contains(&drawn.output);
        let animated = self.settings.overview_animation && !self.lock.is_locked();
        if drawn.is_closing() && animated && output_is_there {
            return;
        }
        if animated && output_is_there {
            let from = drawn.part(now);
            drawn.transition = Some(Transition::new(false, from, now));
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
        if transition.opening {
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

    /// Each window `open` draws at `now`, bottom to top: between its real place and its
    /// place in the layout, as far as the transition has it (243). A minimised window
    /// stays at its place and fades instead, in as it opens and out as it closes; one
    /// that was chosen is shown again, and moves back with the others. They're in
    /// policy's stacking order, so a window chosen is at the top as they go.
    pub(crate) fn overview_drawn_windows(
        &self,
        open: &Overview,
        now: Duration,
    ) -> Vec<DrawnWindow> {
        let policy = &self.policy;
        let origin = output_geometry(&open.output).loc;
        let part = open.part(now);
        let mut drawn: Vec<DrawnWindow> = open
            .windows
            .iter()
            .zip(&open.layout.slots)
            .enumerate()
            .filter_map(|(index, (id, slot))| {
                let frame = policy.frame_rect(*id)?;
                let real = Rectangle::new(frame.loc - origin, frame.size);
                let (rect, alpha) = if policy.is_minimized(*id) {
                    (slot.window, part as f32)
                } else {
                    (between(real, slot.window, part), 1.0)
                };
                Some(DrawnWindow {
                    id: *id,
                    rect,
                    slot: index,
                    alpha,
                })
            })
            .collect();
        let stacking = policy.stacking_all();
        drawn.sort_by_key(|window| stacking.iter().position(|id| *id == window.id));
        drawn
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
    /// selection in the layout, and Enter chooses the selected window (242). A media key,
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
        let closes =
            binding == Some(Binding::Policy(Action::Overview)) || sym == Some(Keysym::Escape);
        if closes {
            self.policy.close_overview();
            return Some(FilterResult::Intercept(None));
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
        let places: Vec<_> = open.layout.slots.iter().map(|slot| slot.window).collect();
        self.policy.overview_select(direction, &places);
        self.refresh_overview();
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
    /// it to close and leaves the Overview open (242), and a press of any button on the
    /// empty area closes the Overview, as does that of a button with the Overview's
    /// shortcut. The release of such a press is swallowed with the Window switcher's.
    pub(crate) fn overview_button(
        &mut self,
        button: u32,
        state: ButtonState,
        location: Point<f64, Logical>,
    ) -> bool {
        let caminito = &mut self.caminito;
        if state == ButtonState::Released || caminito.policy.overview().is_none() {
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
        match caminito.overview_window_at(location) {
            _ if is_shortcut => caminito.policy.close_overview(),
            Some(id) if button == BTN_LEFT => {
                let now = caminito.now();
                caminito.policy.overview_choose(id, now);
            }
            Some(id) if button == BTN_MIDDLE => caminito.policy.overview_close_window(id),
            Some(_) => {}
            None => caminito.policy.close_overview(),
        }
        caminito.refresh_overview();
        true
    }
}
