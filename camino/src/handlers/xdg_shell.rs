//! `xdg_wm_base` and the protocols that extend its toplevels: xdg-activation, xdg-foreign,
//! xdg-dialog, xdg-toplevel-icon and xdg-system-bell.
//!
//! The toplevel handlers only translate xdg-shell's events into `window::lifecycle`'s: a new
//! toplevel is created, and pending until it commits a buffer; committing a null buffer
//! withdraws it, and it starts over as a new pending window.

use camino_policy::{ResizeEdges, WindowId};
use smithay::backend::renderer::utils::with_renderer_surface_state;
use smithay::desktop::{
    PopupKeyboardGrab, PopupKind, PopupPointerGrab, PopupUngrabStrategy, Window,
    find_popup_root_surface, get_popup_toplevel_coords,
};
use smithay::input::Seat;
use smithay::input::pointer::{Focus, GrabStartData};
use smithay::output::Output;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_seat::WlSeat;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::Serial;
use smithay::wayland::shell::xdg::dialog::XdgDialogHandler;
use smithay::wayland::shell::xdg::{
    PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
};
use smithay::wayland::xdg_activation::{
    XdgActivationHandler, XdgActivationState, XdgActivationToken, XdgActivationTokenData,
};
use smithay::wayland::xdg_foreign::{XdgForeignHandler, XdgForeignState};
use smithay::wayland::xdg_system_bell::XdgSystemBellHandler;
use smithay::wayland::xdg_toplevel_icon::XdgToplevelIconHandler;

use crate::input::GrabKind;
use crate::output::output_id;
use crate::state::{Camino, State};
use crate::window::{StateRequest, WindowKey, owe_configure};

impl XdgShellHandler for State {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.camino.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        // The initial configure goes out on the initial commit.
        let window = Window::new_wayland_window(surface);
        self.camino.window_created(window);
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        self.camino.window_destroyed(&key(&surface));
    }

    fn maximize_request(&mut self, surface: ToplevelSurface) {
        self.toplevel_request(&surface, StateRequest::Maximized(true));
    }

    fn unmaximize_request(&mut self, surface: ToplevelSurface) {
        self.toplevel_request(&surface, StateRequest::Maximized(false));
    }

    fn fullscreen_request(&mut self, surface: ToplevelSurface, output: Option<WlOutput>) {
        let output = output
            .and_then(|output| Output::from_resource(&output))
            .map(|output| output_id(&output));
        self.toplevel_request(&surface, StateRequest::Fullscreen(output));
    }

    fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
        self.toplevel_request(&surface, StateRequest::Unfullscreen);
    }

    fn minimize_request(&mut self, surface: ToplevelSurface) {
        self.toplevel_request(&surface, StateRequest::Minimized);
    }

    fn move_request(&mut self, surface: ToplevelSurface, seat: WlSeat, serial: Serial) {
        if let Some((id, start_data)) = self.interactive_request(&surface, &seat, serial) {
            self.begin_window_grab(id, GrabKind::Move, start_data, serial);
        }
    }

    fn resize_request(
        &mut self,
        surface: ToplevelSurface,
        seat: WlSeat,
        serial: Serial,
        edges: xdg_toplevel::ResizeEdge,
    ) {
        use xdg_toplevel::ResizeEdge as E;
        let edges = ResizeEdges {
            top: matches!(edges, E::Top | E::TopLeft | E::TopRight),
            bottom: matches!(edges, E::Bottom | E::BottomLeft | E::BottomRight),
            left: matches!(edges, E::Left | E::TopLeft | E::BottomLeft),
            right: matches!(edges, E::Right | E::TopRight | E::BottomRight),
        };
        if let Some((id, start_data)) = self.interactive_request(&surface, &seat, serial) {
            self.begin_window_grab(id, GrabKind::Resize(edges), start_data, serial);
        }
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        // The initial configure goes out on the initial commit.
        self.camino.unconstrain_popup(&surface);
        if let Err(err) = self.camino.popups.track_popup(PopupKind::Xdg(surface)) {
            tracing::warn!("error tracking a popup: {err}");
        }
    }

    fn reposition_request(
        &mut self,
        surface: PopupSurface,
        positioner: PositionerState,
        token: u32,
    ) {
        surface.with_pending_state(|state| {
            state.geometry = positioner.get_geometry();
            state.positioner = positioner;
        });
        self.camino.unconstrain_popup(&surface);
        surface.send_repositioned(token);
    }

    fn grab(&mut self, surface: PopupSurface, seat: WlSeat, serial: Serial) {
        let Some(seat) = Seat::<State>::from_resource(&seat) else {
            return;
        };
        let popup = PopupKind::Xdg(surface);
        let Ok(root) = find_popup_root_surface(&popup) else {
            return;
        };
        if self.camino.mapped_for_surface(&root).is_none() {
            return;
        }
        let Ok(mut grab) = self.camino.popups.grab_popup(root, popup, &seat, serial) else {
            return;
        };

        // The grab must come from the input event that holds the seat's current grab, if any.
        if let Some(keyboard) = seat.get_keyboard() {
            if keyboard.is_grabbed()
                && !(keyboard.has_grab(serial)
                    || keyboard.has_grab(grab.previous_serial().unwrap_or(serial)))
            {
                grab.ungrab(PopupUngrabStrategy::All);
                return;
            }
            keyboard.set_focus(self, grab.current_grab(), serial);
            keyboard.set_grab(self, PopupKeyboardGrab::new(&grab), serial);
        }
        if let Some(pointer) = seat.get_pointer() {
            if pointer.is_grabbed()
                && !(pointer.has_grab(serial)
                    || pointer.has_grab(grab.previous_serial().unwrap_or_else(|| grab.serial())))
            {
                grab.ungrab(PopupUngrabStrategy::All);
                return;
            }
            pointer.set_grab(self, PopupPointerGrab::new(&grab), serial, Focus::Keep);
        }
    }
}

/// The key `window::lifecycle` knows a toplevel by.
fn key(toplevel: &ToplevelSurface) -> WindowKey {
    WindowKey::Wayland(toplevel.wl_surface().clone())
}

impl State {
    /// Passes a toplevel's state request on. A managed toplevel is owed a configure in
    /// reply, whatever policy decides.
    fn toplevel_request(&mut self, surface: &ToplevelSurface, request: StateRequest) {
        if self.camino.window_state_request(&key(surface), request) {
            owe_configure(surface);
        }
    }

    /// Checks a move or resize request: the window is mapped, and `serial` is the button press
    /// that holds the pointer on one of its surfaces. Returns the window and how the pointer
    /// was when the press grabbed it.
    fn interactive_request(
        &self,
        surface: &ToplevelSurface,
        seat: &WlSeat,
        serial: Serial,
    ) -> Option<(WindowId, GrabStartData<State>)> {
        let id = self.camino.mapped_for_surface(surface.wl_surface())?;
        let pointer = Seat::<State>::from_resource(seat)?.get_pointer()?;
        if !pointer.has_grab(serial) {
            return None;
        }
        let start_data = pointer.grab_start_data()?;
        let (focus, _) = start_data.focus.as_ref()?;
        focus
            .id()
            .same_client_as(&surface.wl_surface().id())
            .then_some((id, start_data))
    }

    /// Handles a commit to a toplevel's surface: the initial configure, mapping on the first
    /// buffer, unmapping on a null one, and otherwise telling policy what changed.
    pub(super) fn xdg_shell_commit(&mut self, surface: &WlSurface) {
        let camino = &mut self.camino;
        camino.popups.commit(surface);
        if let Some(PopupKind::Xdg(popup)) = camino.popups.find_popup(surface) {
            if !popup.is_initial_configure_sent() {
                // The initial configure is sent once, so it can't fail.
                popup.send_configure().expect("the initial popup configure");
            }
            camino.queue_redraw_all();
            return;
        }

        let has_buffer =
            with_renderer_surface_state(surface, |state| state.buffer().is_some()).unwrap_or(false);
        let key = WindowKey::Wayland(surface.clone());
        if let Some(id) = camino.mapped_for_key(&key) {
            if has_buffer {
                camino.window_committed(&key);
            } else {
                // The toplevel is unmapped, and starts over from its initial commit.
                let window = camino.policy.window(id).unwrap().window().clone();
                camino.window_withdrawn(&key);
                camino.window_created(window);
            }
        } else if camino.is_pending(&key) {
            let toplevel = camino
                .xdg_shell_state
                .toplevel_surfaces()
                .iter()
                .find(|toplevel| toplevel.wl_surface() == surface)
                .cloned();
            if let Some(toplevel) = toplevel.filter(|t| !t.is_initial_configure_sent()) {
                camino.configure_pending(&key);
                toplevel.send_configure();
            } else if has_buffer {
                camino.window_committed(&key);
            }
        }
    }
}

impl Camino {
    /// Constrains a popup to the output its toplevel is most on, through the positioner's
    /// constraint adjustments.
    fn unconstrain_popup(&self, popup: &PopupSurface) {
        let kind = PopupKind::Xdg(popup.clone());
        let Some(geometry) = find_popup_root_surface(&kind)
            .ok()
            .and_then(|root| self.mapped_for_surface(&root))
            .and_then(|id| self.policy.geometry(id))
        else {
            return;
        };
        let Some(output) = self
            .policy
            .outputs()
            .iter()
            .filter_map(|o| {
                let overlap = o.geometry.intersection(geometry)?;
                Some((
                    o.geometry,
                    i64::from(overlap.size.w) * i64::from(overlap.size.h),
                ))
            })
            .max_by_key(|(_, area)| *area)
            .map(|(output, _)| output)
        else {
            return;
        };

        // The positioner works relative to the parent's window geometry.
        let mut target = output;
        target.loc -= geometry.loc;
        target.loc -= get_popup_toplevel_coords(&kind);
        popup.with_pending_state(|state| {
            state.geometry = state.positioner.get_unconstrained_geometry(target);
        });
    }
}

impl XdgDialogHandler for State {}

impl XdgToplevelIconHandler for State {}

impl XdgForeignHandler for State {
    fn xdg_foreign_state(&mut self) -> &mut XdgForeignState {
        &mut self.camino.xdg_foreign_state
    }
}

impl XdgSystemBellHandler for State {
    fn ring(&mut self, _surface: Option<WlSurface>) {}
}

impl XdgActivationHandler for State {
    fn activation_state(&mut self) -> &mut XdgActivationState {
        &mut self.camino.xdg_activation_state
    }

    fn request_activation(
        &mut self,
        _token: XdgActivationToken,
        _token_data: XdgActivationTokenData,
        _surface: WlSurface,
    ) {
    }
}
