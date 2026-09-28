//! `org.mate.Camino.Debug` (14), a thin interface over the [`crate::debug`] module.

use futures_channel::oneshot;
use smithay::reexports::calloop::channel::Sender;

use crate::state::Camino;

pub const NAME: &str = "org.mate.Camino.Debug";
pub const PATH: &str = "/org/mate/Camino/Debug";

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

    /// Sends the event loop a request, and waits for its reply.
    async fn ask<T>(
        &self,
        request: impl FnOnce(oneshot::Sender<T>) -> Request,
    ) -> zbus::fdo::Result<T> {
        let (reply, answer) = oneshot::channel();
        let gone = || zbus::fdo::Error::Failed("Camino's event loop has stopped".into());
        self.requests.send(request(reply)).map_err(|_| gone())?;
        answer.await.map_err(|_| gone())
    }
}

#[zbus::interface(name = "org.mate.Camino.Debug")]
impl DebugInterface {
    /// Windows, workspaces, stacking, outputs and focus, as JSON.
    async fn dump_state(&self) -> zbus::fdo::Result<String> {
        self.ask(Request::DumpState).await
    }

    /// Turns frame debug on or off, and returns whether it's now on.
    async fn toggle_frame_debug(&self) -> zbus::fdo::Result<bool> {
        self.ask(Request::ToggleFrameDebug).await
    }
}

impl Camino {
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
