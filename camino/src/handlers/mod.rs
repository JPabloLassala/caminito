//! Smithay's protocol handlers, implemented for `State`, one file per area (15).

mod compositor;
mod xdg_shell;

use smithay::input::{SeatHandler, SeatState};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::output::OutputHandler;
use smithay::wayland::pointer_constraints::PointerConstraintsHandler;

use crate::state::State;

impl SeatHandler for State {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<State> {
        &mut self.camino.seat_state
    }
}

// Pointer focus on a `WlSurface` needs this; the pointer-constraints global comes in 22.
impl PointerConstraintsHandler for State {}

impl OutputHandler for State {}

smithay::delegate_dispatch2!(State);
