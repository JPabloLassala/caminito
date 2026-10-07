//! `org.mate.Caminito.Debug` (14), a thin interface over the [`crate::debug`] module.

use futures_channel::oneshot;
use smithay::reexports::calloop::channel::Sender;

use crate::state::Caminito;

pub const NAME: &str = "org.mate.Caminito.Debug";
pub const PATH: &str = "/org/mate/Caminito/Debug";

/// A call, for the event loop to answer.
pub enum Request {
    DumpState(oneshot::Sender<String>),
    ToggleFrameDebug(oneshot::Sender<bool>),
}

pub struct DebugInterface {
    requests: Sender<Request>,
}

impl DebugInterface {
    pub fn new(requests: Sender<Request>) -> Self {
        Self { requests }
    }
}

#[zbus::interface(name = "org.mate.Caminito.Debug")]
impl DebugInterface {
    /// Windows, workspaces, stacking, outputs and focus, as JSON.
    async fn dump_state(&self) -> zbus::fdo::Result<String> {
        super::ask(&self.requests, Request::DumpState).await
    }

    /// Turns frame debug on or off, and returns whether it's now on.
    async fn toggle_frame_debug(&self) -> zbus::fdo::Result<bool> {
        super::ask(&self.requests, Request::ToggleFrameDebug).await
    }
}

impl Caminito {
    pub(super) fn debug_request(&mut self, request: Request) {
        // A caller that has gone away doesn't want the answer.
        match request {
            Request::DumpState(reply) => {
                let _ = reply.send(self.state_dump());
            }
            Request::ToggleFrameDebug(reply) => {
                let _ = reply.send(self.toggle_frame_debug());
            }
        }
    }
}
