//! `org.mate.Camino` (ADR 0002's third channel): what MATE's components and
//! camino-displays ask of Camino that no protocol carries. So far, the primary output (81)
//! and saving the layout (83), both ADR 0014's, and the window a window screenshot is of
//! (104).

use futures_channel::oneshot;
use smithay::reexports::calloop::channel::Sender;
use zbus::message::Header;

use crate::output::{primary, saved};
use crate::state::Camino;

/// The well-known name, which carried patches look for an owner of (ADR 0011).
pub(crate) const NAME: &str = "org.mate.Camino";
/// The object the interface is served at.
pub(crate) const PATH: &str = "/org/mate/Camino";

/// A call, for the event loop to answer.
pub(crate) enum Request {
    /// Reads `PrimaryOutput`.
    PrimaryOutput(oneshot::Sender<String>),
    /// Writes `PrimaryOutput`, with the name written.
    ChoosePrimaryOutput(String, oneshot::Sender<Result<(), primary::Error>>),
    /// Calls `SaveLayout`.
    SaveLayout(oneshot::Sender<Result<(), saved::Error>>),
    /// Calls `GetActiveToplevel`, with the caller's process if the bus knows it.
    ActiveToplevel(Option<u32>, oneshot::Sender<Option<String>>),
}

/// `org.mate.Camino`, as zbus serves it.
pub(crate) struct CaminoInterface {
    requests: Sender<Request>,
}

impl CaminoInterface {
    pub(crate) fn new(requests: Sender<Request>) -> Self {
        Self { requests }
    }
}

#[zbus::interface(name = "org.mate.Camino")]
impl CaminoInterface {
    /// The primary output's name, as `wl_output`, output management and RandR have it: the
    /// user's choice, or its stand-in while that monitor is unplugged or disabled. It's
    /// the empty string when there are no outputs.
    #[zbus(property)]
    async fn primary_output(&self) -> zbus::fdo::Result<String> {
        super::ask(&self.requests, Request::PrimaryOutput).await
    }

    /// Chooses the primary output, by the name of an enabled output. The choice is of
    /// its monitor, and returns with it after a stand-in.
    #[zbus(property)]
    async fn set_primary_output(&self, name: String) -> zbus::fdo::Result<()> {
        let choose = |reply| Request::ChoosePrimaryOutput(name, reply);
        super::ask(&self.requests, choose)
            .await?
            .map_err(|err| zbus::fdo::Error::InvalidArgs(err.to_string()))
    }

    /// Saves the layout as it is now, for the monitors connected now: which are enabled,
    /// their modes, places and scales, and the primary output. Camino restores it when it
    /// next has those monitors, at start-up and as they're plugged in. Nothing else saves
    /// one: a configuration applied over output management lasts for the Session.
    async fn save_layout(&self) -> zbus::fdo::Result<()> {
        super::ask(&self.requests, Request::SaveLayout)
            .await?
            .map_err(|err| zbus::fdo::Error::Failed(err.to_string()))
    }

    /// The ext-foreign-toplevel-list identifier of the window a window screenshot is of:
    /// the window on screen that most recently had the focus and isn't of the caller's
    /// process, since the caller's own dialog may have it. It's the empty string when
    /// there's no such window.
    async fn get_active_toplevel(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> zbus::fdo::Result<String> {
        let caller = caller_process(connection, &header).await;
        let active = |reply| Request::ActiveToplevel(caller, reply);
        // The empty string is the method's "none".
        Ok(super::ask(&self.requests, active)
            .await?
            .unwrap_or_default())
    }
}

/// The process that sent the call with `header`, as the bus knows it. Without it, none of
/// the windows is taken for the caller's.
async fn caller_process(connection: &zbus::Connection, header: &Header<'_>) -> Option<u32> {
    let sender = header.sender()?.to_owned();
    let asked = async {
        let bus = zbus::fdo::DBusProxy::new(connection).await?;
        bus.get_connection_unix_process_id(sender.into()).await
    };
    asked
        .await
        .inspect_err(|err| tracing::warn!("can't ask the bus for a caller's process: {err}"))
        .ok()
}

impl Camino {
    pub(super) fn camino_request(&mut self, request: Request) {
        // A caller that has gone away doesn't want the answer.
        match request {
            Request::PrimaryOutput(reply) => {
                let _ = reply.send(self.primary_output_name());
            }
            Request::ChoosePrimaryOutput(name, reply) => {
                let chosen = self.choose_primary_output(&name);
                if chosen.is_ok() {
                    // zbus signals a property that its own setter changed.
                    self.primary_output_signalled = Some(self.primary_output_name());
                }
                let _ = reply.send(chosen);
            }
            Request::SaveLayout(reply) => {
                let _ = reply.send(self.save_layout());
            }
            Request::ActiveToplevel(caller, reply) => {
                let _ = reply.send(self.active_toplevel(caller));
            }
        }
    }

    /// The interface is served: a caller reads the primary output as it is now, so
    /// nothing is signalled until it changes.
    pub(super) fn primary_output_read_from_now_on(&mut self) {
        self.primary_output_signalled = Some(self.primary_output_name());
    }

    /// The primary output's name as the property has it.
    fn primary_output_name(&self) -> String {
        self.primary_output()
            .map(|output| output.name())
            .unwrap_or_default()
    }

    /// Signals `PrimaryOutput` if it's not what the callers last heard: the stand-in went
    /// in, or the choice returned (81).
    pub(crate) fn refresh_primary_output_property(&mut self) {
        let Some(dbus) = &self.dbus else {
            return;
        };
        let name = self.primary_output_name();
        if self.primary_output_signalled.as_ref() == Some(&name) {
            return;
        }
        let connection = dbus.connection().clone();
        // On zbus's thread, which asks the event loop for the value it sends: the main
        // thread never waits on the bus (15).
        let signal = async move {
            let signalled = async {
                let interface = connection
                    .object_server()
                    .interface::<_, CaminoInterface>(PATH)
                    .await?;
                let emitter = interface.signal_emitter();
                interface.get().await.primary_output_changed(emitter).await
            };
            if let Err(err) = signalled.await {
                tracing::warn!("can't signal the primary output on D-Bus: {err}");
            }
        };
        dbus.connection()
            .executor()
            .spawn(signal, "primary output changed")
            .detach();
        self.primary_output_signalled = Some(name);
    }
}
