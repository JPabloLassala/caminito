//! The Window switcher's side in the compositor (108, [ADR 0016]): the popup that
//! `switch-windows` and its siblings open. Policy has the list, the selection, committing
//! and cancelling. Here are the keys that drive it while it's open, and the grid it's laid
//! out in.
//!
//! [ADR 0016]: ../../../docs/adr/0016-one-window-switcher-in-place-of-marcos-alt-tab-popup.md

pub(crate) mod layout;

use camino_policy::Action;
use smithay::backend::input::{InputTime, Keycode};
use smithay::desktop::{PopupKind, PopupManager};
use smithay::input::keyboard::{FilterResult, Keysym, KeysymHandle, ModifiersState};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::SERIAL_COUNTER;
use smithay::wayland::seat::WaylandFocus;

use self::layout::{CellStyle, Direction, Grid};
use crate::output::output_geometry;
use crate::scene;
use crate::state::{Camino, SEAT, State};

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
#[derive(Debug)]
pub(crate) struct OpenSwitcher {
    holding: Holding,
    /// The key of the binding that opened it, which steps it as Tab does.
    key: Keysym,
}

impl Camino {
    /// The grid the open switcher is laid out in, on the output it's shown on or would
    /// be: the one the pointer is on, else the primary output.
    pub(crate) fn switcher_grid(&self) -> Option<Grid> {
        let count = self.policy.switcher()?.windows.len();
        let output = self
            .outputs
            .iter()
            .find(|output| scene::pointer_on(self, output).is_some())
            .or(self.primary_output())?;
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

    /// Lets go of the compositor's side when policy has closed its switcher: by a commit
    /// or a cancel here, or by itself, as when the workspace changed under it or its last
    /// listed window closed.
    pub(crate) fn refresh_switcher(&mut self) {
        if self.switcher.is_some() && self.policy.switcher().is_none() {
            self.switcher = None;
            self.queue_redraw_all();
        }
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
            self.camino.switcher = Some(OpenSwitcher {
                holding: Holding::of(mods),
                key,
            });
        }
        self.camino.queue_redraw_all();
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
