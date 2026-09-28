//! `Fixture`: Camino on the headless backend and any number of test clients in one calloop
//! (14, 15). This is niri's `tests/fixture.rs` shape.

use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use camino_test_client::Client;
use smithay::output::Output;
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::Display;
use smithay::utils::{Physical, Size};

use crate::state::{ClientState, State};

/// How many dispatches a roundtrip may take before the test fails rather than hangs.
const MAX_ROUNDTRIP_DISPATCHES: usize = 100;

pub struct Fixture {
    event_loop: EventLoop<'static, State>,
    pub state: State,
    clients: Vec<Client>,
}

/// The index of one of a fixture's clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientId(usize);

impl Fixture {
    /// Camino with no outputs and no clients.
    pub fn new() -> Self {
        let event_loop = EventLoop::try_new().unwrap();
        let state = State::new_headless(&event_loop, Display::new().unwrap());
        Self {
            event_loop,
            state,
            clients: Vec::new(),
        }
    }

    pub fn add_output(&mut self, size: impl Into<Size<i32, Physical>>, scale: f64) -> Output {
        self.state.add_headless_output(size.into(), scale)
    }

    /// Connects a new client over a socket pair. It has seen nothing until a roundtrip.
    pub fn add_client(&mut self) -> ClientId {
        let (server, client) = UnixStream::pair().unwrap();
        self.state
            .camino
            .display_handle
            .insert_client(server, Arc::new(ClientState::default()))
            .unwrap();
        self.clients.push(Client::new(client));
        ClientId(self.clients.len() - 1)
    }

    pub fn client(&mut self, id: ClientId) -> &mut Client {
        &mut self.clients[id.0]
    }

    /// One pass of the compositor's loop and its refresh, without waiting.
    fn dispatch_server(&mut self) {
        self.event_loop
            .dispatch(Duration::ZERO, &mut self.state)
            .unwrap();
        self.state.refresh_and_flush_clients();
    }

    /// Every client sends its requests, the compositor handles them, then every client handles
    /// the compositor's answers.
    pub fn dispatch(&mut self) {
        for client in &mut self.clients {
            client.flush();
        }
        self.dispatch_server();
        for client in &mut self.clients {
            client.dispatch();
        }
    }

    /// Dispatches until the compositor has handled every request `id` has sent, and `id` has
    /// handled every event sent in reply.
    pub fn roundtrip(&mut self, id: ClientId) {
        let done = self.client(id).send_sync();
        for _ in 0..MAX_ROUNDTRIP_DISPATCHES {
            self.dispatch();
            if done.load(Ordering::SeqCst) {
                return;
            }
        }
        panic!("roundtrip didn't finish in {MAX_ROUNDTRIP_DISPATCHES} dispatches");
    }
}
