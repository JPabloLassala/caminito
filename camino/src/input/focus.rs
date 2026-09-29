//! What the keyboard can focus: a Wayland surface, or an XWayland window, through which
//! Smithay's `KeyboardTarget` follows the window's ICCCM input model and sends
//! `WM_TAKE_FOCUS` (11). The pointer still focuses surfaces only, until 37.

use std::borrow::Cow;

use smithay::backend::input::{InputTime, KeyState};
use smithay::desktop::PopupKind;
use smithay::input::Seat;
use smithay::input::keyboard::{KeyboardTarget, KeysymHandle, ModifiersState};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{IsAlive, Serial};
use smithay::wayland::seat::WaylandFocus;
use smithay::xwayland::X11Surface;

use crate::state::State;

/// The keyboard's focus.
#[derive(Debug, Clone, PartialEq)]
pub enum KeyboardFocus {
    Surface(WlSurface),
    X11(Box<X11Surface>),
}

impl From<WlSurface> for KeyboardFocus {
    fn from(surface: WlSurface) -> Self {
        KeyboardFocus::Surface(surface)
    }
}

impl From<PopupKind> for KeyboardFocus {
    fn from(popup: PopupKind) -> Self {
        KeyboardFocus::Surface(popup.wl_surface().clone())
    }
}

/// Popup grabs hand the keyboard's focus to the pointer.
impl From<KeyboardFocus> for WlSurface {
    fn from(focus: KeyboardFocus) -> Self {
        focus
            .wl_surface()
            .expect("Camino focuses only XWayland windows that have content")
            .into_owned()
    }
}

impl IsAlive for KeyboardFocus {
    fn alive(&self) -> bool {
        match self {
            KeyboardFocus::Surface(surface) => surface.alive(),
            KeyboardFocus::X11(surface) => surface.alive(),
        }
    }
}

impl WaylandFocus for KeyboardFocus {
    fn wl_surface(&self) -> Option<Cow<'_, WlSurface>> {
        match self {
            KeyboardFocus::Surface(surface) => Some(Cow::Borrowed(surface)),
            KeyboardFocus::X11(surface) => surface.wl_surface().map(Cow::Owned),
        }
    }
}

impl KeyboardTarget<State> for KeyboardFocus {
    fn enter(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        keys: Vec<KeysymHandle<'_>>,
        serial: Serial,
    ) {
        match self {
            KeyboardFocus::Surface(surface) => {
                KeyboardTarget::enter(surface, seat, data, keys, serial)
            }
            KeyboardFocus::X11(surface) => {
                KeyboardTarget::enter(&**surface, seat, data, keys, serial)
            }
        }
    }

    fn leave(&self, seat: &Seat<State>, data: &mut State, serial: Serial) {
        match self {
            KeyboardFocus::Surface(surface) => KeyboardTarget::leave(surface, seat, data, serial),
            KeyboardFocus::X11(surface) => KeyboardTarget::leave(&**surface, seat, data, serial),
        }
    }

    fn key(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        key: KeysymHandle<'_>,
        state: KeyState,
        serial: Serial,
        time: InputTime,
    ) {
        match self {
            KeyboardFocus::Surface(surface) => {
                KeyboardTarget::key(surface, seat, data, key, state, serial, time)
            }
            KeyboardFocus::X11(surface) => {
                KeyboardTarget::key(&**surface, seat, data, key, state, serial, time)
            }
        }
    }

    fn modifiers(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        modifiers: ModifiersState,
        serial: Serial,
    ) {
        match self {
            KeyboardFocus::Surface(surface) => {
                KeyboardTarget::modifiers(surface, seat, data, modifiers, serial)
            }
            KeyboardFocus::X11(surface) => {
                KeyboardTarget::modifiers(&**surface, seat, data, modifiers, serial)
            }
        }
    }
}
