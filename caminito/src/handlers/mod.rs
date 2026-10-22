//! Smithay's protocol handlers, implemented for `State`, one file per area (15).

mod compositor;
mod decoration;
mod ext_workspace;
mod foreign_toplevel;
mod gamma_control;
mod gtk_shell;
mod image_capture;
mod input;
mod layer_shell;
mod output_management;
mod rendering;
mod security_context;
mod selection;
mod xdg_shell;
mod xfce_foreign_toplevel;

pub(crate) use decoration::kde_server_side;
pub(crate) use image_capture::CaptureSession;
pub(crate) use security_context::sees_privileged_globals;
pub(crate) use selection::DragIcon;

use smithay::input::keyboard::LedState;
use smithay::input::pointer::CursorImageStatus;
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::output::OutputHandler;
use smithay::wayland::seat::WaylandFocus;
use smithay::wayland::selection::data_device::set_data_device_focus;
use smithay::wayland::selection::primary_selection::set_primary_focus;

use crate::input::{KeyboardFocus, PointerFocus};
use crate::state::State;

impl SeatHandler for State {
    type KeyboardFocus = KeyboardFocus;
    type PointerFocus = PointerFocus;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<State> {
        &mut self.caminito.seat_state
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, image: CursorImageStatus) {
        self.caminito.client_cursor = image;
        self.caminito.queue_redraw_all();
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&KeyboardFocus>) {
        // The new focus is told of the modifiers held as it enters, those held back from the
        // last (CMN-188) among them, so they're held back no longer.
        self.caminito.held_back = None;
        // The clipboards follow keyboard focus.
        let dh = &self.caminito.display_handle;
        let client = focused
            .and_then(|focus| focus.wl_surface())
            .and_then(|surface| dh.get_client(surface.id()).ok());
        set_data_device_focus(dh, seat, client.clone());
        set_primary_focus(dh, seat, client);
    }

    fn led_state_changed(&mut self, _seat: &Seat<Self>, leds: LedState) {
        self.backend.update_leds(leds);
    }
}

impl OutputHandler for State {}

smithay::delegate_dispatch2!(State);
