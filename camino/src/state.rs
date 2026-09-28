//! `State` and `Camino`: everything the event loop runs on (15).

use std::collections::HashSet;
use std::ffi::OsString;
use std::sync::Arc;

use camino_policy::{Config, OutputId, Policy};
use smithay::input::{Seat, SeatState};
use smithay::output::Output;
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{EventLoop, Interest, LoopHandle, LoopSignal, Mode, PostAction};
use smithay::reexports::wayland_server::backend::{ClientData, ClientId, DisconnectReason};
use smithay::reexports::wayland_server::{Display, DisplayHandle};
use smithay::wayland::compositor::{CompositorClientState, CompositorState};
use smithay::wayland::shell::xdg::XdgShellState;
use smithay::wayland::shm::ShmState;
use smithay::wayland::socket::ListeningSocketSource;

use crate::backend::Backend;
use crate::window::Mapped;

/// The whole compositor: the backend, and everything that doesn't depend on it.
pub struct State {
    pub backend: Backend,
    pub camino: Camino,
}

/// Camino's backend-independent state.
pub struct Camino {
    pub display_handle: DisplayHandle,
    pub loop_handle: LoopHandle<'static, State>,
    pub loop_signal: LoopSignal,

    pub compositor_state: CompositorState,
    pub shm_state: ShmState,
    pub xdg_shell_state: XdgShellState,
    pub seat_state: SeatState<State>,
    pub seat: Seat<State>,

    /// Every output, in the order they were added.
    pub outputs: Vec<Output>,
    pub(crate) next_output_id: u64,
    /// Outputs to render in the next refresh pass.
    pub(crate) redraws: HashSet<OutputId>,

    pub policy: Policy<Mapped>,
}

/// Per-client data.
#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

impl State {
    /// Opens a winit window as the one output.
    pub fn new_winit(
        event_loop: &EventLoop<'static, State>,
        display: Display<State>,
    ) -> Result<Self, smithay::backend::winit::Error> {
        let mut camino = Camino::new(event_loop, display);
        let backend = Backend::Winit(Box::new(crate::backend::winit::Winit::new(&mut camino)?));
        Ok(Self { backend, camino })
    }

    /// Starts with no outputs; tests add them with `add_headless_output`.
    #[cfg(test)]
    pub fn new_headless(event_loop: &EventLoop<'static, State>, display: Display<State>) -> Self {
        let camino = Camino::new(event_loop, display);
        let backend = Backend::Headless(crate::backend::headless::Headless::default());
        Self { backend, camino }
    }

    #[cfg(test)]
    pub fn add_headless_output(
        &mut self,
        size: smithay::utils::Size<i32, smithay::utils::Physical>,
        scale: f64,
    ) -> Output {
        self.backend
            .headless_mut()
            .add_output(&mut self.camino, size, scale)
    }

    /// The number of render elements the scene has for `output`.
    #[cfg(test)]
    pub fn scene_element_count(&mut self, output: &Output) -> usize {
        let renderer = self.backend.headless_mut().renderer();
        crate::scene::output_elements(renderer, &self.camino, output).len()
    }

    /// The refresh pass, after each loop dispatch: brings the outward views up to date with
    /// policy, renders the outputs that asked for it, and flushes the clients.
    pub fn refresh_and_flush_clients(&mut self) {
        self.camino.refresh_views();
        self.redraw_queued_outputs();
        if let Err(err) = self.camino.display_handle.flush_clients() {
            tracing::warn!("error flushing clients: {err}");
        }
    }

    fn redraw_queued_outputs(&mut self) {
        for output in self.camino.take_queued_redraws() {
            self.backend.render(&mut self.camino, &output);
        }
    }
}

impl Camino {
    fn new(event_loop: &EventLoop<'static, State>, display: Display<State>) -> Self {
        let display_handle = display.handle();
        let loop_handle = event_loop.handle();

        let source = Generic::new(display, Interest::READ, Mode::Level);
        loop_handle
            .insert_source(source, |_, display, state| {
                // Safety: the display is never dropped while the loop holds it.
                if let Err(err) = unsafe { display.get_mut().dispatch_clients(state) } {
                    tracing::warn!("error dispatching clients: {err}");
                }
                Ok(PostAction::Continue)
            })
            .expect("the display joins the event loop");

        let compositor_state = CompositorState::new::<State>(&display_handle);
        let shm_state = ShmState::new::<State>(&display_handle, Vec::new());
        let xdg_shell_state = XdgShellState::new::<State>(&display_handle);
        let mut seat_state = SeatState::new();
        let seat = seat_state.new_wl_seat(&display_handle, "seat0");

        Self {
            display_handle,
            loop_handle,
            loop_signal: event_loop.get_signal(),
            compositor_state,
            shm_state,
            xdg_shell_state,
            seat_state,
            seat,
            outputs: Vec::new(),
            next_output_id: 0,
            redraws: HashSet::new(),
            policy: Policy::new(Config::default()),
        }
    }

    /// Opens a Wayland socket for clients to connect on, and returns its name.
    pub fn add_socket(&mut self) -> Result<OsString, Box<dyn std::error::Error>> {
        let source = ListeningSocketSource::new_auto()?;
        let name = source.socket_name().to_os_string();
        self.loop_handle
            .insert_source(source, |stream, _, state| {
                let client = state
                    .camino
                    .display_handle
                    .insert_client(stream, Arc::new(ClientState::default()));
                if let Err(err) = client {
                    tracing::warn!("error adding a client: {err}");
                }
            })
            .map_err(|err| err.error)?;
        Ok(name)
    }

    /// Sends the outward views of policy state whatever changed since they last sent. There
    /// are no views yet.
    fn refresh_views(&mut self) {}
}
