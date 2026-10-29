//! Caminito's calls to caminito-notifications's private API (ADR 0051, CMN-263),
//! `org.mate.Caminito.Notifications`: Super+N opens the Notification center, and Do Not
//! Disturb's shortcut toggles it. The Notifications applet owns no name to be called at, so
//! the daemon passes the first on to it with a signal. Caminito decides nothing about Do Not
//! Disturb.
//!
//! Without the daemon, as under a Session that hasn't started it yet, the key does nothing
//! but log.

use crate::state::Caminito;

const NAME: &str = "org.mate.Caminito.Notifications";
const PATH: &str = "/org/mate/Caminito/Notifications";
const INTERFACE: &str = "org.mate.Caminito.Notifications";

/// What Caminito asks caminito-notifications for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationsAction {
    /// The Notification center open.
    OpenCenter,
    /// Do Not Disturb off while it's on, and on until turned off while it's off.
    ToggleDoNotDisturb,
}

impl NotificationsAction {
    /// The private API's method that does it, which takes nothing.
    fn method(self) -> &'static str {
        match self {
            NotificationsAction::OpenCenter => "OpenCenter",
            NotificationsAction::ToggleDoNotDisturb => "ToggleDoNotDisturb",
        }
    }
}

impl Caminito {
    /// Asks caminito-notifications for `action`. The call is made on zbus's thread, and the
    /// main thread never waits on it (15).
    pub(crate) fn ask_notifications(&self, action: NotificationsAction) {
        let Some(dbus) = &self.dbus else {
            tracing::debug!("can't ask for {action:?}: Caminito isn't on the session bus");
            return;
        };
        let connection = dbus.connection().clone();
        let call = async move {
            let called = connection
                .call_method(Some(NAME), PATH, Some(INTERFACE), action.method(), &())
                .await;
            if let Err(err) = called {
                tracing::debug!("can't ask caminito-notifications for {action:?}: {err}");
            }
        };
        dbus.connection()
            .executor()
            .spawn(call, "notifications call")
            .detach();
    }
}
