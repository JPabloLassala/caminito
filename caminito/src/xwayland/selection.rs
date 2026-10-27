//! The clipboard and primary selection between X and Wayland clients, with the same focus
//! gate as cosmic-comp (11): an X client reaches a selection only while one of Xwayland's
//! surfaces has the keyboard, and a Wayland selection is pushed into X lazily, as one of them
//! next takes the keyboard, so X clients in the background can't read it. An X client's
//! selection is set on the seat as the compositor's own, which Wayland clients read through
//! `X11Wm`.

use std::os::fd::OwnedFd;

use smithay::reexports::wayland_server::Resource;
use smithay::wayland::selection::SelectionTarget;
use smithay::wayland::selection::data_device::{
    clear_data_device_selection, current_data_device_selection_userdata,
    request_data_device_client_selection, set_data_device_selection,
};
use smithay::wayland::selection::primary_selection::{
    clear_primary_selection, current_primary_selection_userdata, request_primary_client_selection,
    set_primary_selection,
};

use crate::input::KeyboardFocus;
use crate::state::{Caminito, State};

/// A Wayland selection X hasn't been told of.
#[derive(Clone)]
pub(super) struct Owed {
    target: SelectionTarget,
    /// Its mime types, or `None` once it's cleared.
    mime_types: Option<Vec<String>>,
}

/// The seat's Wayland selections, by their mime types. They outlive Xwayland, so that one
/// started again is owed them (42).
#[derive(Default)]
pub(crate) struct WaylandSelections(Vec<Owed>);

impl WaylandSelections {
    /// A Wayland client set or cleared `target`, or an X client took it (`None`).
    fn set(&mut self, target: SelectionTarget, mime_types: Option<Vec<String>>) {
        self.0.retain(|owed| owed.target != target);
        if mime_types.is_some() {
            self.0.push(Owed { target, mime_types });
        }
    }

    /// What a new Xwayland is owed.
    pub(super) fn owed(&self) -> Vec<Owed> {
        self.0.clone()
    }
}

impl Caminito {
    /// Whether `focus` is one of Xwayland's surfaces: a managed X window, or an
    /// override-redirect menu holding a keyboard grab, whose Paste item reads the selection.
    fn is_xwayland(&self, focus: &KeyboardFocus) -> bool {
        let Some(xwayland) = &self.xwayland else {
            return false;
        };
        match focus {
            KeyboardFocus::X11(_) => true,
            KeyboardFocus::Layer(_) => false,
            KeyboardFocus::Surface(surface) => surface.client().as_ref() == Some(&xwayland.client),
        }
    }

    fn xwayland_has_keyboard(&self) -> bool {
        let keyboard = self.seat.get_keyboard().unwrap();
        keyboard
            .current_focus()
            .is_some_and(|focus| self.is_xwayland(&focus))
    }

    /// Drops what X was owed of `target`, which something newer replaces.
    fn forget_owed(&mut self, target: SelectionTarget) {
        if let Some(xwayland) = self.xwayland.as_mut() {
            xwayland
                .owed_selections
                .retain(|owed| owed.target != target);
        }
    }

    /// Tells X of a Wayland selection, as Smithay's selection window owning it, or clears
    /// it if Smithay's window owns it.
    fn push_to_x(&mut self, Owed { target, mime_types }: Owed) {
        let Some(wm) = self.xwm() else {
            return;
        };
        if let Err(err) = wm.new_selection(target, mime_types) {
            tracing::warn!("can't give X the {target:?} selection: {err}");
        }
    }
}

impl State {
    /// A Wayland client set or cleared a selection. X is told at once if one of its windows
    /// has the keyboard (the client needn't have it, if it set it through data-control), and
    /// otherwise when one next takes it.
    pub(crate) fn wayland_selection_changed(
        &mut self,
        target: SelectionTarget,
        mime_types: Option<Vec<String>>,
    ) {
        let caminito = &mut self.caminito;
        caminito.wayland_selections.set(target, mime_types.clone());
        let owed = Owed { target, mime_types };
        if caminito.xwayland_has_keyboard() {
            caminito.push_to_x(owed);
        } else {
            caminito.forget_owed(target);
            if let Some(xwayland) = caminito.xwayland.as_mut() {
                xwayland.owed_selections.push(owed);
            }
        }
    }

    /// `focus` is about to take the keyboard; if it's one of Xwayland's, X is told of what
    /// it's owed. For an X window that's before Smithay sets X's input focus, whose flush
    /// sends X the selections too, so they're there by the time the client hears it has the
    /// focus. A menu's grab sends X nothing, and `X11Wm::new_selection` doesn't flush, so
    /// for a menu whose window never had the keyboard they wait for `X11Wm`'s next flush.
    pub(crate) fn keyboard_entering(&mut self, focus: &KeyboardFocus) {
        let caminito = &mut self.caminito;
        if !caminito.is_xwayland(focus) {
            return;
        }
        let Some(xwayland) = caminito.xwayland.as_mut() else {
            return;
        };
        for owed in std::mem::take(&mut xwayland.owed_selections) {
            caminito.push_to_x(owed);
        }
    }

    /// The focus gate, which `X11Wm` asks before it answers an X client's `ConvertSelection`.
    pub(super) fn allow_x_selection_access(&self) -> bool {
        self.caminito.xwayland_has_keyboard()
    }

    /// An X client set a selection. It replaces whatever Wayland selection X was owed.
    pub(super) fn x_selection_set(&mut self, target: SelectionTarget, mime_types: Vec<String>) {
        let caminito = &mut self.caminito;
        caminito.forget_owed(target);
        caminito.wayland_selections.set(target, None);
        let (dh, seat) = (&caminito.display_handle, &caminito.seat);
        match target {
            SelectionTarget::Clipboard => set_data_device_selection(dh, seat, mime_types, ()),
            SelectionTarget::Primary => set_primary_selection(dh, seat, mime_types, ()),
        }
    }

    /// The X client whose selection it was let go of it. A Wayland client's selection since
    /// then is left alone.
    pub(super) fn x_selection_cleared(&mut self, target: SelectionTarget) {
        let (dh, seat) = (&self.caminito.display_handle, &self.caminito.seat);
        match target {
            SelectionTarget::Clipboard => {
                if current_data_device_selection_userdata(seat).is_some() {
                    clear_data_device_selection(dh, seat);
                }
            }
            SelectionTarget::Primary => {
                if current_primary_selection_userdata(seat).is_some() {
                    clear_primary_selection(dh, seat);
                }
            }
        }
    }

    /// An X client reads a Wayland selection: the Wayland client that owns it writes it to
    /// `fd`, which `X11Wm` passes on.
    pub(super) fn send_wayland_selection_to_x(
        &mut self,
        target: SelectionTarget,
        mime_type: String,
        fd: OwnedFd,
    ) {
        let seat = &self.caminito.seat;
        let requested = match target {
            SelectionTarget::Clipboard => {
                request_data_device_client_selection(seat, mime_type, fd).map_err(|e| e.to_string())
            }
            SelectionTarget::Primary => {
                request_primary_client_selection(seat, mime_type, fd).map_err(|e| e.to_string())
            }
        };
        if let Err(err) = requested {
            tracing::debug!("can't pass the {target:?} selection to X: {err}");
        }
    }

    /// A Wayland client reads an X client's selection, which the seat holds as the
    /// compositor's.
    pub(crate) fn send_x_selection_to_wayland(
        &mut self,
        target: SelectionTarget,
        mime_type: String,
        fd: OwnedFd,
    ) {
        let Some(wm) = self.caminito.xwm() else {
            return;
        };
        if let Err(err) = wm.send_selection(target, mime_type, fd) {
            tracing::debug!("can't pass the {target:?} selection from X: {err}");
        }
    }
}
