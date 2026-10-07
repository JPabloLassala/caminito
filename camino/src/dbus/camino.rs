//! `org.mate.Camino` (ADR 0002's third channel): what MATE's components and
//! camino-displays ask of Camino that no protocol carries. So far, the primary output (81)
//! and saving the layout (83), both ADR 0014's, and showing the desktop (94).

use futures_channel::oneshot;
use smithay::reexports::calloop::channel::Sender;

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
    /// Reads `ShowingDesktop`.
    ShowingDesktop(oneshot::Sender<bool>),
    /// Writes `ShowingDesktop`, with the value written.
    SetShowingDesktop(bool, oneshot::Sender<()>),
}

/// A property Camino signals itself, when it changes without a caller writing it.
#[derive(Clone, Copy)]
enum Property {
    PrimaryOutput,
    ShowingDesktop,
}

impl Property {
    fn name(self) -> &'static str {
        match self {
            Self::PrimaryOutput => "PrimaryOutput",
            Self::ShowingDesktop => "ShowingDesktop",
        }
    }
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

    /// Whether the active workspace shows its desktop, with Marco's meaning (94): its
    /// windows are hidden, not minimised, until this is written false or one of them is
    /// mapped or activated. It follows the active workspace, and Ctrl+Alt+D changes it too.
    #[zbus(property)]
    async fn showing_desktop(&self) -> zbus::fdo::Result<bool> {
        super::ask(&self.requests, Request::ShowingDesktop).await
    }

    /// Shows the desktop on the active workspace, or brings its windows back.
    #[zbus(property)]
    async fn set_showing_desktop(&self, showing: bool) -> zbus::fdo::Result<()> {
        let show = |reply| Request::SetShowingDesktop(showing, reply);
        super::ask(&self.requests, show).await
    }
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
            Request::ShowingDesktop(reply) => {
                let _ = reply.send(self.policy.is_showing_desktop());
            }
            Request::SetShowingDesktop(showing, reply) => {
                self.policy.set_showing_desktop(showing);
                self.queue_redraw_all();
                // zbus signals this one too.
                self.showing_desktop_signalled = Some(showing);
                let _ = reply.send(());
            }
        }
    }

    /// The interface is served: a caller reads the properties as they are now, so nothing
    /// is signalled until one changes.
    pub(super) fn properties_read_from_now_on(&mut self) {
        self.primary_output_signalled = Some(self.primary_output_name());
        self.showing_desktop_signalled = Some(self.policy.is_showing_desktop());
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
        let name = self.primary_output_name();
        if self.dbus.is_none() || self.primary_output_signalled.as_ref() == Some(&name) {
            return;
        }
        self.signal(Property::PrimaryOutput);
        self.primary_output_signalled = Some(name);
    }

    /// Signals `ShowingDesktop` if it's not what the callers last heard (94): the key
    /// changed it, a window mapped or activated ended it, or the active workspace is
    /// another, which shows its desktop or doesn't for itself.
    pub(crate) fn refresh_showing_desktop_property(&mut self) {
        let showing = self.policy.is_showing_desktop();
        if self.dbus.is_none() || self.showing_desktop_signalled == Some(showing) {
            return;
        }
        self.signal(Property::ShowingDesktop);
        self.showing_desktop_signalled = Some(showing);
    }

    /// Signals that `property` changed.
    fn signal(&self, property: Property) {
        let Some(dbus) = &self.dbus else {
            return;
        };
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
                let interface = interface.get().await;
                match property {
                    Property::PrimaryOutput => interface.primary_output_changed(emitter).await,
                    Property::ShowingDesktop => interface.showing_desktop_changed(emitter).await,
                }
            };
            if let Err(err) = signalled.await {
                tracing::warn!("can't signal {} on D-Bus: {err}", property.name());
            }
        };
        dbus.connection()
            .executor()
            .spawn(signal, "property changed")
            .detach();
    }
}
