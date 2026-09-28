//! `xdg_wm_base`, answering each toplevel's initial commit with a configure. Mapping
//! toplevels into policy, and popups, come with the rest of xdg-shell in 22.

use smithay::reexports::wayland_server::protocol::wl_seat::WlSeat;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::Serial;
use smithay::wayland::shell::xdg::{
    PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
};

use crate::state::State;

impl XdgShellHandler for State {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.camino.xdg_shell_state
    }

    fn new_toplevel(&mut self, _surface: ToplevelSurface) {}

    fn new_popup(&mut self, _surface: PopupSurface, _positioner: PositionerState) {}

    fn grab(&mut self, _surface: PopupSurface, _seat: WlSeat, _serial: Serial) {}

    fn reposition_request(
        &mut self,
        _surface: PopupSurface,
        _positioner: PositionerState,
        _token: u32,
    ) {
    }
}

impl State {
    /// Sends a toplevel its first configure when it makes its initial commit.
    pub(super) fn xdg_shell_commit(&mut self, surface: &WlSurface) {
        let toplevel = self
            .camino
            .xdg_shell_state
            .toplevel_surfaces()
            .iter()
            .find(|toplevel| toplevel.wl_surface() == surface);
        if let Some(toplevel) = toplevel
            && !toplevel.is_initial_configure_sent()
        {
            toplevel.send_configure();
        }
    }
}
