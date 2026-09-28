//! Camino, a Wayland compositor for the MATE desktop.

pub mod backend;
pub mod decoration;
mod handlers;
pub mod helpers;
pub mod icons;
pub mod input;
pub mod logging;
pub mod output;
pub mod scene;
pub mod settings;
pub mod state;
pub mod window;

#[cfg(test)]
mod tests;

use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::Display;

use crate::state::State;

/// Runs Camino nested, in a winit window, until the window closes.
pub fn run_nested() -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop = EventLoop::try_new()?;
    let mut state = State::new_winit(&event_loop, Display::new()?)?;

    let socket = state.camino.add_socket()?;
    tracing::info!("listening on WAYLAND_DISPLAY={}", socket.to_string_lossy());
    state.camino.start_palette_helper(&socket);

    event_loop.run(None, &mut state, State::refresh_and_flush_clients)?;
    Ok(())
}
