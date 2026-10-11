//! Caminito, a Wayland compositor for the MATE desktop.

pub mod backend;
mod bell;
mod capture;
mod commands;
pub mod cursor;
pub mod dbus;
pub mod debug;
pub mod decoration;
mod handlers;
pub mod helpers;
pub mod icons;
mod idle;
pub mod input;
mod layer;
mod lock;
pub mod logging;
mod night_light;
pub mod output;
mod policy_timer;
pub mod protocols;
pub mod scene;
pub mod session;
pub mod settings;
pub mod state;
mod switcher;
pub mod window;
mod work_area;
mod workspace_popup;
pub mod xwayland;

#[cfg(test)]
mod tests;

use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::Display;

use crate::state::State;

/// Runs Caminito on `kind`'s backend, and returns the status to exit with.
///
/// Bare, with no `session`, it runs until the winit window closes, or SIGTERM or SIGINT
/// comes. With one it's a Session (08): the start-up below is in 08's order, the `session`
/// module spawns the command once Xwayland is ready and the bus has the environment, and
/// Caminito ends with the command and its status.
pub fn run(
    kind: backend::Kind,
    session: Option<session::SessionCommand>,
) -> Result<u8, Box<dyn std::error::Error>> {
    let mut event_loop = EventLoop::try_new()?;
    let mut state = State::new(&event_loop, Display::new()?, kind)?;
    state.caminito.stop_on_signals()?;
    state.caminito.helper_stderr = logging::helper_stderr();

    let socket = state.caminito.add_socket()?;
    tracing::info!("listening on WAYLAND_DISPLAY={}", socket.to_string_lossy());
    state.caminito.locker.set_socket(socket.clone());
    state.caminito.start_xwayland();
    state.caminito.start_palette_helper(&socket);
    // It returns with the Screensaver shim's names owned or refused (62), which a Session's
    // command must find taken.
    if let Err(err) = state.caminito.start_dbus(dbus::Names::WellKnown) {
        tracing::warn!("can't serve Caminito's D-Bus interfaces: {err}");
    }
    // Nested too: which `sleep-display` key applies is the machine's.
    state.caminito.start_upower(None);
    match kind {
        backend::Kind::Tty => state.caminito.start_logind(None),
        // The session it would find is the desktop's it's nested in (130).
        backend::Kind::Winit => {
            tracing::info!(
                "nested, with no logind session of its own: caminito locks without logind"
            );
        }
    }
    if let Some(command) = session {
        state.caminito.start_session(command, socket);
    }

    event_loop.run(None, &mut state, State::refresh_and_flush_clients)?;
    Ok(state.caminito.session_exit().unwrap_or(0))
}
