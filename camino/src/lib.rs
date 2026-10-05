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
mod layer;
pub mod logging;
pub mod output;
pub mod protocols;
pub mod scene;
pub mod session;
pub mod settings;
pub mod state;
pub mod window;
mod work_area;
pub mod xwayland;

#[cfg(test)]
mod tests;

use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::Display;

use crate::state::State;

/// Runs Camino on `kind`'s backend, and returns the status to exit with.
///
/// Bare, with no `session`, it runs until the winit window closes, or SIGTERM or SIGINT
/// comes. With one it's a Session (08): the start-up below is in 08's order, the `session`
/// module spawns the command once Xwayland is ready and the bus has the environment, and
/// Camino ends with the command and its status.
pub fn run(
    kind: backend::Kind,
    session: Option<session::SessionCommand>,
) -> Result<u8, Box<dyn std::error::Error>> {
    let mut event_loop = EventLoop::try_new()?;
    let mut state = State::new(&event_loop, Display::new()?, kind)?;
    state.camino.stop_on_signals()?;
    state.camino.helper_stderr = logging::helper_stderr();

    let socket = state.camino.add_socket()?;
    tracing::info!("listening on WAYLAND_DISPLAY={}", socket.to_string_lossy());
    state.camino.start_xwayland();
    state.camino.start_palette_helper(&socket);
    // It returns with the Screensaver shim's names owned or refused (62), which a Session's
    // command must find taken.
    if let Err(err) = state.camino.start_dbus(dbus::Names::WellKnown) {
        tracing::warn!("can't serve Camino's D-Bus interfaces: {err}");
    }
    if let Some(command) = session {
        state.camino.start_session(command, socket);
    }

    event_loop.run(None, &mut state, State::refresh_and_flush_clients)?;
    Ok(state.camino.session_exit().unwrap_or(0))
}
