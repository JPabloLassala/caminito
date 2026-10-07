//! The Window switcher's side in the compositor (108, [ADR 0016]): the popup that
//! `switch-windows` and its siblings open. Policy has the list, the selection, committing
//! and cancelling. Here are the keys and the clicks that drive it while it's open, when
//! and where its popup shows, and the grid it's laid out in. `draw` draws the popup's
//! chrome, and `scene` puts it and the thumbnails on the output.
//!
//! [ADR 0016]: ../../../docs/adr/0016-one-window-switcher-in-place-of-marcos-alt-tab-popup.md

pub(crate) mod draw;
pub(crate) mod layout;

use std::time::Duration;

use camino_policy::{Action, WindowId};
use camino_theme::{PaletteColors, Title, Variant, pango};
use smithay::backend::input::{ButtonState, InputTime, Keycode};
use smithay::desktop::{PopupKind, PopupManager};
use smithay::input::keyboard::{FilterResult, Keysym, KeysymHandle, ModifiersState};
use smithay::output::Output;
use smithay::reexports::calloop::RegistrationToken;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, SERIAL_COUNTER};
use smithay::wayland::seat::WaylandFocus;

use self::draw::{Chrome, Entry};
use self::layout::{CellStyle, Direction, Grid};
use crate::decoration::IconSource;
use crate::output::output_geometry;
use crate::scene;
use crate::state::{Camino, SEAT, State};

/// How long after the key the popup appears, so a quick Alt+Tab shows nothing (108).
pub(crate) const POPUP_DELAY: Duration = Duration::from_millis(90);

/// The modifiers that hold the Window switcher open: those of the binding that opened it,
/// without Shift, which only turns it backward.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Holding {
    ctrl: bool,
    alt: bool,
    logo: bool,
}

impl Holding {
    fn of(mods: &ModifiersState) -> Self {
        Self {
            ctrl: mods.ctrl,
            alt: mods.alt,
            logo: mods.logo,
        }
    }

    /// Whether every one of them is down in `mods`.
    fn is_held_in(self, mods: &ModifiersState) -> bool {
        (!self.ctrl || mods.ctrl) && (!self.alt || mods.alt) && (!self.logo || mods.logo)
    }
}

/// The compositor's side of the open Window switcher.
pub(crate) struct OpenSwitcher {
    holding: Holding,
    /// The key of the binding that opened it, which steps it as Tab does.
    key: Keysym,
    /// The timer that runs until the popup appears.
    timer: Option<RegistrationToken>,
    /// The popup, once it has appeared.
    popup: Option<Popup>,
}

/// The Window switcher's popup, on the output it appeared on. It doesn't follow the
/// pointer to another.
pub(crate) struct Popup {
    pub output: Output,
    pub grid: Grid,
    /// The chrome as last drawn, unless cairo couldn't draw it.
    pub chrome: Option<Chrome>,
    /// What the chrome was drawn from, to draw it again only when that changes.
    drawn: Option<Drawn>,
}

/// Everything the chrome is drawn from.
#[derive(PartialEq)]
struct Drawn {
    grid: Grid,
    selected: usize,
    /// Each listed window, whether it's urgent, and where its icon comes from.
    windows: Vec<(WindowId, bool, IconSource)>,
    title: String,
    scale: i32,
    colors: PaletteColors,
    font: String,
    /// The frames' generation, which counts the changes to the font rendering and the
    /// icon theme.
    generation: u64,
}

impl Camino {
    /// The grid the open switcher is laid out in, on the output it's shown on or would
    /// be: the one the pointer is on, else the primary output.
    pub(crate) fn switcher_grid(&self) -> Option<Grid> {
        let shown = self.visible_switcher().map(|popup| popup.output.clone());
        self.switcher_grid_on(&shown.or_else(|| self.switcher_output())?)
    }

    /// The output the popup appears on: the one the pointer is on, else the primary
    /// output.
    fn switcher_output(&self) -> Option<Output> {
        self.outputs
            .iter()
            .find(|output| scene::pointer_on(self, output).is_some())
            .or(self.primary_output())
            .cloned()
    }

    /// The grid the open switcher has on `output`.
    fn switcher_grid_on(&self, output: &Output) -> Option<Grid> {
        let count = self.policy.switcher()?.windows.len();
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

    /// The Window switcher's popup, once it has appeared.
    pub(crate) fn visible_switcher(&self) -> Option<&Popup> {
        self.switcher.as_ref()?.popup.as_ref()
    }

    /// Lets go of the compositor's side when policy has closed its switcher: by a commit
    /// or a cancel here, or by itself, as when the workspace changed under it or its last
    /// listed window closed.
    pub(crate) fn refresh_switcher(&mut self) {
        if self.policy.switcher().is_none()
            && let Some(open) = self.switcher.take()
        {
            if let Some(timer) = open.timer {
                self.loop_handle.remove(timer);
            }
            self.queue_redraw_all();
        }
    }

    /// The popup's delay is over: it appears, if the switcher is still open.
    fn show_switcher_popup(&mut self) {
        self.refresh_switcher();
        let Some(output) = self.switcher_output() else {
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
        } else if let Some(output) = self.switcher_output() {
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
        let window = |id: &WindowId| self.policy.window(*id);
        Some(Drawn {
            grid: self.switcher_grid_on(output)?,
            selected: switcher.selected,
            windows: switcher
                .windows
                .iter()
                .filter_map(|id| {
                    let mapped = window(id)?;
                    Some((*id, mapped.is_urgent(), mapped.icon_source()))
                })
                .collect(),
            title: window(&switcher.windows[switcher.selected])
                .map(|mapped| mapped.title())
                .unwrap_or_default(),
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
            .windows
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
        match sym {
            Some(sym) if sym == key || sym == Keysym::Tab => {
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
        self.refresh_switcher();
        self.queue_redraw_all();
        Some(FilterResult::Intercept(None))
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
            self.queue_redraw_all();
        }
    }
}

impl State {
    /// A `switch-*` binding fired with the Window switcher closed: `key` with `mods` down.
    /// Nothing opens during a move or resize. A client's open menu is dismissed first.
    pub(crate) fn switch_action(
        &mut self,
        action: Action,
        key: Keysym,
        mods: &ModifiersState,
        time: InputTime,
    ) {
        if self.camino.window_grab.is_some() {
            return;
        }
        self.dismiss_menus(time);
        let now = self.camino.now();
        self.camino.policy.action(SEAT, action, now);
        if self.camino.policy.switcher().is_some() {
            let timer = Timer::from_duration(POPUP_DELAY);
            let timer = self
                .camino
                .loop_handle
                .insert_source(timer, |_, _, state| {
                    if let Some(open) = &mut state.camino.switcher {
                        open.timer = None;
                    }
                    state.camino.show_switcher_popup();
                    TimeoutAction::Drop
                })
                .inspect_err(|err| {
                    tracing::warn!("can't time the window switcher's popup: {}", err.error)
                })
                .ok();
            self.camino.switcher = Some(OpenSwitcher {
                holding: Holding::of(mods),
                key,
                timer,
                popup: None,
            });
        }
        self.camino.queue_redraw_all();
    }

    /// A pointer button with the pointer at `location`. Returns whether it was the Window
    /// switcher's, and so no client's: while the switcher is open, a press in a cell of
    /// its popup commits that window and any other press cancels, as a click ends Marco's
    /// grab; and the release of such a press is swallowed with it.
    pub(crate) fn switcher_button(
        &mut self,
        button: u32,
        state: ButtonState,
        location: Point<f64, Logical>,
    ) -> bool {
        let camino = &mut self.camino;
        if state == ButtonState::Released {
            return camino.suppressed_buttons.remove(&button);
        }
        camino.refresh_switcher();
        if camino.switcher.is_none() {
            return false;
        }
        camino.suppressed_buttons.insert(button);
        let cell = camino.visible_switcher().and_then(|popup| {
            let origin = output_geometry(&popup.output).loc.to_f64();
            popup.grid.cell_at(location - origin)
        });
        let now = camino.now();
        match cell {
            Some(index) => {
                camino.policy.switcher_select(index);
                camino.policy.switcher_commit(now);
            }
            None => camino.policy.switcher_cancel(now),
        }
        camino.refresh_switcher();
        camino.queue_redraw_all();
        true
    }

    /// Dismisses the menus clients have open: xdg popups with their grab of the keyboard
    /// and the pointer, and the keyboard grab an override-redirect window took through
    /// Xwayland.
    fn dismiss_menus(&mut self, time: InputTime) {
        let camino = &self.camino;
        let windows = camino
            .policy
            .windows()
            .filter_map(|mapped| mapped.window().wl_surface().map(|s| s.into_owned()));
        let layers = camino.outputs.iter().flat_map(|output| {
            let surfaces = camino.layer_surfaces(output).into_iter();
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
        let keyboard = self.camino.seat.get_keyboard().unwrap();
        let xwayland = self.camino.xwayland_keyboard_grab.take().is_some();
        if (dismissed || xwayland) && keyboard.is_grabbed() {
            keyboard.unset_grab(self);
        }
        // Only a popup's grab: a drag or a press on a frame keeps the pointer.
        let pointer = self.camino.seat.get_pointer().unwrap();
        if dismissed && pointer.is_grabbed() {
            pointer.unset_grab(self, SERIAL_COUNTER.next_serial(), time);
        }
    }
}
