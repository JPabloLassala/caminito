//! The Window switcher's side in the compositor (108, [ADR 0016]): the popup that
//! `switch-windows` and its siblings open. A `cycle-*` action (163) is the same with no
//! popup, as Marco's is. Policy has the list, the selection, committing
//! and cancelling. Here are the keys and the clicks that drive it while it's open, when
//! and where its popup shows, and the grid it's laid out in. `draw` draws the popup's
//! chrome, and `scene` puts it and the thumbnails on the output.
//!
//! [ADR 0016]: ../../../docs/adr/0016-one-window-switcher-in-place-of-marcos-alt-tab-popup.md

pub(crate) mod draw;
pub(crate) mod layout;

use std::time::Duration;

use caminito_policy::{Action, SwitchEntry};
use caminito_theme::{PaletteColors, Title, Variant, pango};
use smithay::backend::input::{ButtonState, InputTime, Keycode};
use smithay::desktop::{PopupKind, PopupManager};
use smithay::input::keyboard::{FilterResult, Keysym, KeysymHandle, ModifiersState};
use smithay::output::Output;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, SERIAL_COUNTER};
use smithay::wayland::seat::WaylandFocus;

use self::draw::{Chrome, Entry};
use self::layout::{CellStyle, Direction, Grid};
use crate::decoration::IconSource;
use crate::input::bindings::Binding;
use crate::input::bindings::accelerators::{Trigger, x_button};
use crate::input::press_keysyms;
use crate::output::output_geometry;
use crate::scene;
use crate::state::{Caminito, SEAT, State};

/// How long after the key the popup appears, so a quick Alt+Tab shows nothing (108).
pub(crate) const POPUP_DELAY: Duration = Duration::from_millis(90);

/// The modifiers that hold a popup open: those of the binding that opened it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Holding {
    ctrl: bool,
    alt: bool,
    logo: bool,
    shift: bool,
}

impl Holding {
    /// The Window switcher's: `mods` without Shift, which only turns it backward.
    fn of(mods: &ModifiersState) -> Self {
        Self {
            shift: false,
            ..Self::with_shift_of(mods)
        }
    }

    /// The Workspace popup's (152): `mods` with Shift, which its bindings tell a move from
    /// a switch by.
    pub(crate) fn with_shift_of(mods: &ModifiersState) -> Self {
        Self {
            ctrl: mods.ctrl,
            alt: mods.alt,
            logo: mods.logo,
            shift: mods.shift,
        }
    }

    /// Whether it has no modifier, so that no release would let go of it.
    pub(crate) fn is_empty(self) -> bool {
        !(self.ctrl || self.alt || self.logo || self.shift)
    }

    /// Whether every one of them is down in `mods`.
    pub(crate) fn is_held_in(self, mods: &ModifiersState) -> bool {
        (!self.ctrl || mods.ctrl)
            && (!self.alt || mods.alt)
            && (!self.logo || mods.logo)
            && (!self.shift || mods.shift)
    }
}

/// The compositor's side of the open Window switcher.
pub(crate) struct OpenSwitcher {
    holding: Holding,
    /// The key or the mouse button of the binding that opened it, which steps it as Tab
    /// does.
    key: Trigger,
    /// When the popup appears, until it has: `deadlines` shows it then (175).
    popup_due: Option<Duration>,
    /// The popup, once it has appeared.
    popup: Option<Popup>,
}

/// The Window switcher's popup, on the output it appeared on. It doesn't follow the
/// pointer to another.
pub(crate) struct Popup {
    pub(crate) output: Output,
    pub(crate) grid: Grid,
    /// The chrome as last drawn, unless cairo couldn't draw it.
    pub(crate) chrome: Option<Chrome>,
    /// What the chrome was drawn from, to draw it again only when that changes.
    drawn: Option<Drawn>,
}

/// Everything the chrome is drawn from.
#[derive(PartialEq)]
struct Drawn {
    grid: Grid,
    selected: usize,
    /// Each listed entry, whether it's urgent, and where its icon comes from.
    entries: Vec<(SwitchEntry, bool, IconSource)>,
    title: String,
    scale: i32,
    colors: PaletteColors,
    font: String,
    /// The frames' generation, which counts the changes to the font rendering and the
    /// icon theme.
    generation: u64,
}

impl Caminito {
    /// The grid the open switcher is laid out in, on the output it's shown on or would
    /// be: the one the pointer is on, else the primary output.
    pub(crate) fn switcher_grid(&self) -> Option<Grid> {
        let shown = self.visible_switcher().map(|popup| popup.output.clone());
        self.switcher_grid_on(&shown.or_else(|| self.popup_output())?)
    }

    /// The output a popup appears on, the Workspace popup too (152): the one the pointer is
    /// on, else the primary output.
    pub(crate) fn popup_output(&self) -> Option<Output> {
        self.outputs
            .iter()
            .find(|output| scene::pointer_on(self, output).is_some())
            .or(self.primary_output())
            .cloned()
    }

    /// The grid the open switcher has on `output`.
    fn switcher_grid_on(&self, output: &Output) -> Option<Grid> {
        let count = self.policy.switcher()?.entries.len();
        let settings = self.settings.switcher;
        let style = if settings.thumbnails {
            CellStyle::Thumbnails
        } else {
            CellStyle::Icons {
                icon_size: settings.icon_size,
            }
        };
        let size = output_geometry(output).size;
        Some(Grid::new(count, settings.max_columns, size, style))
    }

    /// Whether the Window switcher's popup is at `location`, in global logical
    /// coordinates.
    pub(crate) fn switcher_popup_at(&self, location: Point<f64, Logical>) -> bool {
        self.visible_switcher().is_some_and(|popup| {
            let origin = output_geometry(&popup.output).loc.to_f64();
            popup.grid.popup.to_f64().contains(location - origin)
        })
    }

    /// The Window switcher's popup, once it has appeared.
    pub(crate) fn visible_switcher(&self) -> Option<&Popup> {
        self.switcher.as_ref()?.popup.as_ref()
    }

    /// Lets go of the compositor's side when policy has closed its switcher: by a commit
    /// or a cancel here, or by itself, as when the workspace changed under it or its last
    /// listed window closed.
    pub(crate) fn refresh_switcher(&mut self) {
        self.give_keyboard_to_chosen();
        if self.policy.switcher().is_none() && self.switcher.take().is_some() {
            self.queue_redraw_all();
        }
    }

    /// When the popup appears, while an open switcher waits to show it.
    pub(crate) fn switcher_popup_deadline(&self) -> Option<Duration> {
        self.switcher.as_ref()?.popup_due
    }

    /// Shows the popup if it's due at `now`, for `deadlines`.
    pub(crate) fn switcher_popup_elapse(&mut self, now: Duration) {
        if let Some(open) = &mut self.switcher
            && open.popup_due.is_some_and(|due| due <= now)
        {
            open.popup_due = None;
            self.show_switcher_popup();
        }
    }

    /// The popup's delay is over: it appears, if the switcher is still open.
    fn show_switcher_popup(&mut self) {
        self.refresh_switcher();
        let Some(output) = self.popup_output() else {
            return;
        };
        let Some(grid) = self.switcher_grid_on(&output) else {
            return;
        };
        if let Some(open) = &mut self.switcher {
            open.popup = Some(Popup {
                output,
                grid,
                chrome: None,
                drawn: None,
            });
        }
        self.refresh_switcher_popup();
    }

    /// Brings the visible popup up to date, in the refresh pass: moves it off an output
    /// that's gone, lays its grid out again, and draws its chrome again if anything it's
    /// drawn from changed.
    pub(crate) fn refresh_switcher_popup(&mut self) {
        self.refresh_switcher();
        let Some(shown) = self.visible_switcher().map(|popup| popup.output.clone()) else {
            return;
        };
        // The popup's output was unplugged or turned off under it.
        let output = if self.outputs.contains(&shown) {
            shown
        } else if let Some(output) = self.popup_output() {
            output
        } else {
            let now = self.now();
            self.policy.switcher_cancel(now);
            self.refresh_switcher();
            return;
        };
        let Some(drawn) = self.switcher_chrome_inputs(&output) else {
            return;
        };
        let stale = self
            .visible_switcher()
            .is_none_or(|popup| popup.output != output || popup.drawn.as_ref() != Some(&drawn));
        if !stale {
            return;
        }
        let chrome = self.draw_switcher_chrome(&drawn);
        if let Some(popup) = self.switcher.as_mut().and_then(|open| open.popup.as_mut()) {
            popup.output = output;
            popup.grid = drawn.grid.clone();
            popup.chrome = chrome;
            popup.drawn = Some(drawn);
        }
        self.queue_redraw_all();
    }

    /// What the popup's chrome is drawn from on `output`, now.
    fn switcher_chrome_inputs(&self, output: &Output) -> Option<Drawn> {
        let switcher = self.policy.switcher()?;
        // A Layer surface has no title, app id or icon of its own: its namespace names it,
        // and an icon too where the icon theme has one by that name (163).
        let namespace = |entry: &SwitchEntry| self.layer_namespace(*entry);
        let window = |entry: &SwitchEntry| self.policy.window(entry.window()?);
        Some(Drawn {
            grid: self.switcher_grid_on(output)?,
            selected: switcher.selected,
            entries: switcher
                .entries
                .iter()
                .filter_map(|entry| {
                    if let Some(namespace) = namespace(entry) {
                        return Some((*entry, false, IconSource::AppId(Some(namespace))));
                    }
                    let (id, mapped) = (entry.window()?, window(entry)?);
                    Some((*entry, self.policy.is_urgent(id), mapped.icon_source()))
                })
                .collect(),
            title: {
                let selected = &switcher.entries[switcher.selected];
                let title = window(selected).map(|mapped| mapped.title());
                title.or_else(|| namespace(selected)).unwrap_or_default()
            },
            scale: output.current_scale().fractional_scale().ceil() as i32,
            colors: self.palette.palette().variant(Variant::Default).clone(),
            font: self.settings.interface.font_name.clone(),
            generation: self.frame_context.generation(),
        })
    }

    /// Draws the chrome from `drawn`, with each window's icon loaded at the size its
    /// cell shows it.
    fn draw_switcher_chrome(&mut self, drawn: &Drawn) -> Option<Chrome> {
        let grid = &drawn.grid;
        let entries: Vec<Entry> = drawn
            .entries
            .iter()
            .enumerate()
            .map(|(index, (_, urgent, source))| Entry {
                urgent: *urgent,
                icon: self.frame_context.window_icon(
                    source,
                    draw::icon_size(grid, index),
                    drawn.scale,
                ),
            })
            .collect();
        let font = pango::FontDescription::from_string(&drawn.font);
        let title = Title::new(self.frame_context.text_context(), &font, 1.0, &drawn.title);
        draw::draw(
            grid,
            drawn.selected,
            &entries,
            &title,
            &drawn.colors,
            drawn.scale,
        )
        .inspect_err(|err| tracing::warn!("can't draw the window switcher: {err}"))
        .ok()
    }

    /// A key went down while the Window switcher may be open. Returns what the keyboard
    /// path does with it, or `None` if the switcher is closed and the key is the other
    /// stages'. Every key but a modifier is the switcher's and reaches no client. A
    /// modifier goes on to the client, which is told of the modifiers with the keys it
    /// gets.
    pub(crate) fn switcher_key_pressed<T>(
        &mut self,
        keycode: Keycode,
        mods: &ModifiersState,
        keysym: &KeysymHandle<'_>,
    ) -> Option<FilterResult<Option<T>>> {
        self.refresh_switcher();
        let key = self.switcher.as_ref()?.key;
        let selected = self.policy.switcher()?.selected;
        if keysym.modified_sym().is_modifier_key() {
            return Some(FilterResult::Forward);
        }
        self.suppressed_keys.insert(keycode);
        let now = self.now();
        // Keys are named as the first layout does, like the bindings.
        let sym = keysym.raw_latin_sym_or_raw_current_sym();
        let direction = match sym {
            Some(Keysym::Left) => Some(Direction::Left),
            Some(Keysym::Right) => Some(Direction::Right),
            Some(Keysym::Up) => Some(Direction::Up),
            Some(Keysym::Down) => Some(Direction::Down),
            _ => None,
        };
        let bindings = &self.settings.bindings;
        let binding = bindings.binding_for(mods, &press_keysyms(keysym));
        if !self.switcher_binding_pressed(binding) {
            match sym {
                Some(sym) if Trigger::Key(sym) == key || sym == Keysym::Tab => {
                    self.policy.switcher_step(mods.shift);
                }
                Some(Keysym::Return | Keysym::KP_Enter) => self.policy.switcher_commit(now),
                Some(Keysym::Escape) => self.policy.switcher_cancel(now),
                _ => {
                    if let (Some(direction), Some(grid)) = (direction, self.switcher_grid()) {
                        self.policy
                            .switcher_select(grid.neighbour(selected, direction));
                    }
                }
            }
        }
        self.refresh_switcher();
        Some(FilterResult::Intercept(None))
    }

    /// What the press of a key or a mouse button with this `binding` does to the open
    /// switcher, and whether it did anything. A `switch-*` binding steps the Window switcher
    /// and a `cycle-*` one a cycle, and each ends the other's, as in Marco
    /// (`process_tab_grab`, core/keybindings.c:2141-2199), which also sets the stacking
    /// back. A cycle has no Escape of its own, so a `cycle-*` binding of another list ends
    /// it too, as there between the group's and the others'; the Window switcher is stepped
    /// by any `switch-*` binding (108).
    fn switcher_binding_pressed(&mut self, binding: Option<Binding>) -> bool {
        let Some(switcher) = self.policy.switcher() else {
            return false;
        };
        let (popup, list) = (switcher.popup, switcher.list);
        let (steps, backward) = match binding {
            Some(Binding::Policy(Action::Switch { backward, .. })) => (popup, backward),
            Some(Binding::Policy(Action::Cycle {
                list: its,
                backward,
            })) => (!popup && its == list, backward),
            _ => return false,
        };
        if steps {
            self.policy.switcher_step(backward);
        } else {
            let now = self.now();
            self.policy.switcher_cancel(now);
        }
        true
    }

    /// A key went up, leaving `mods` down: the switcher commits once the modifiers that
    /// hold it open aren't all down.
    pub(crate) fn switcher_key_released(&mut self, mods: &ModifiersState) {
        self.refresh_switcher();
        let Some(open) = &self.switcher else {
            return;
        };
        if !open.holding.is_held_in(mods) {
            let now = self.now();
            self.policy.switcher_commit(now);
            self.refresh_switcher();
        }
    }
}

impl State {
    /// A `switch-*` or `cycle-*` binding fired with the Window switcher closed: `key`, a
    /// key or a mouse button, with `mods` down. Nothing opens during a move or resize. A
    /// client's open menu is dismissed first. A `cycle-*` action times no popup (163).
    pub(crate) fn switch_action(
        &mut self,
        action: Action,
        key: Trigger,
        mods: &ModifiersState,
        time: InputTime,
    ) {
        if self.caminito.window_grab.is_some() {
            return;
        }
        self.dismiss_menus(time);
        let now = self.caminito.now();
        self.caminito.policy.action(SEAT, action, now);
        // One entry alone is chosen at once.
        self.caminito.give_keyboard_to_chosen();
        if let Some(switcher) = self.caminito.policy.switcher() {
            let popup_due = switcher.popup.then(|| now + POPUP_DELAY);
            self.caminito.switcher = Some(OpenSwitcher {
                holding: Holding::of(mods),
                key,
                popup_due,
                popup: None,
            });
        }
    }

    /// A pointer button with the pointer at `location`. Returns whether it was the Window
    /// switcher's, and so no client's: while the switcher is open, a press in a cell of
    /// its popup commits that window and any other press cancels, as a click ends Marco's
    /// grab; and the release of such a press is swallowed with it. A button with a
    /// `switch-*` or `cycle-*` binding drives the switcher as such a key does instead, and
    /// so does the button that opened it (184).
    pub(crate) fn switcher_button(
        &mut self,
        button: u32,
        state: ButtonState,
        location: Point<f64, Logical>,
    ) -> bool {
        let caminito = &mut self.caminito;
        if state == ButtonState::Released {
            return caminito.suppressed_buttons.remove(&button);
        }
        // The grid as policy's list is now: a window may have closed since the refresh
        // pass, and its cell would be the next window's.
        caminito.refresh_switcher_popup();
        if caminito.switcher.is_none() {
            return false;
        }
        caminito.suppressed_buttons.insert(button);
        let mods = caminito.seat.get_keyboard().unwrap().modifier_state();
        let pressed = x_button(button);
        let bindings = &caminito.settings.bindings;
        let binding = pressed.and_then(|button| bindings.binding_for_button(&mods, button));
        let opened_it = |open: &OpenSwitcher| pressed.map(Trigger::Button) == Some(open.key);
        if caminito.switcher_binding_pressed(binding) {
            caminito.refresh_switcher();
            return true;
        }
        if caminito.switcher.as_ref().is_some_and(opened_it) {
            caminito.policy.switcher_step(mods.shift);
            caminito.refresh_switcher();
            return true;
        }
        let cell = caminito.visible_switcher().and_then(|popup| {
            let origin = output_geometry(&popup.output).loc.to_f64();
            popup.grid.cell_at(location - origin)
        });
        let now = caminito.now();
        match cell {
            Some(index) => {
                caminito.policy.switcher_select(index);
                caminito.policy.switcher_commit(now);
            }
            None => caminito.policy.switcher_cancel(now),
        }
        caminito.refresh_switcher();
        true
    }

    /// Dismisses the menus clients have open: xdg popups with their grab of the keyboard
    /// and the pointer, and the keyboard grab an override-redirect window took through
    /// Xwayland.
    pub(crate) fn dismiss_menus(&mut self, time: InputTime) {
        let caminito = &self.caminito;
        let windows = caminito
            .policy
            .windows()
            .filter_map(|mapped| mapped.window().wl_surface().map(|s| s.into_owned()));
        let layers = caminito.outputs.iter().flat_map(|output| {
            let surfaces = caminito.layer_surfaces(output).into_iter();
            surfaces.map(|(surface, _)| surface.wl_surface().clone())
        });
        let roots: Vec<WlSurface> = windows.chain(layers).collect();
        let mut dismissed = false;
        for root in roots {
            let popups: Vec<PopupKind> = PopupManager::popups_for_surface(&root)
                .map(|(popup, _)| popup)
                .collect();
            for popup in popups {
                // It fails only for a popup that's gone already.
                dismissed |= PopupManager::dismiss_popup(&root, &popup).is_ok();
            }
        }
        let keyboard = self.caminito.seat.get_keyboard().unwrap();
        let xwayland = self.caminito.xwayland_keyboard_grab.take().is_some();
        if (dismissed || xwayland) && keyboard.is_grabbed() {
            keyboard.unset_grab(self);
        }
        // Only a popup's grab: a drag or a press on a frame keeps the pointer.
        let pointer = self.caminito.seat.get_pointer().unwrap();
        if dismissed && pointer.is_grabbed() {
            pointer.unset_grab(self, SERIAL_COUNTER.next_serial(), time);
        }
    }
}
