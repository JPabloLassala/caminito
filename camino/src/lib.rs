//! Camino, a Wayland compositor for the MATE desktop.

pub mod backend;
pub mod cursor;
pub mod dbus;
pub mod debug;
pub mod decoration;
mod handlers;
pub mod helpers;
pub mod icons;
pub mod input;
pub mod logging;
pub mod output;
pub mod protocols;
pub mod scene;
pub mod settings;
pub mod state;
pub mod window;
pub mod xwayland;

#[cfg(test)]
mod tests;

use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::Display;

use crate::state::State;

/// Runs Camino on `kind`'s backend until the winit window closes, or SIGTERM or SIGINT
/// comes.
pub fn run(kind: backend::Kind) -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop = EventLoop::try_new()?;
    let mut state = State::new(&event_loop, Display::new()?, kind)?;
    state.camino.stop_on_signals()?;

    let socket = state.camino.add_socket()?;
    tracing::info!("listening on WAYLAND_DISPLAY={}", socket.to_string_lossy());
    state.camino.start_xwayland();
    state.camino.start_palette_helper(&socket);
    if let Err(err) = state.camino.start_dbus(dbus::Names::WellKnown) {
        tracing::warn!("can't serve Camino's D-Bus interfaces: {err}");
    }

    event_loop.run(None, &mut state, State::refresh_and_flush_clients)?;
    Ok(())
}
