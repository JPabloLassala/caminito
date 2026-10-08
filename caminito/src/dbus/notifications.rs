//! `org.freedesktop.Notifications`, the notification daemon's name: where Caminito posts
//! the few things it has to tell the user itself (13).

use std::collections::HashMap;

use zbus::zvariant::Value;

use crate::state::Caminito;

const NAME: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";
const INTERFACE: &str = "org.freedesktop.Notifications";
/// The icon the control center has for its accessibility pages.
const ICON: &str = "preferences-desktop-accessibility";

impl Caminito {
    /// Posts a notification. The call is made on zbus's thread, and the main thread never
    /// waits on it (15). Without the bus or a notification daemon there's none.
    pub(crate) fn notify(&self, summary: &str, body: &str) {
        let Some(dbus) = &self.dbus else {
            tracing::debug!("no notification without the session bus: {summary}");
            return;
        };
        let connection = dbus.connection().clone();
        let (summary, body) = (summary.to_owned(), body.to_owned());
        let call = async move {
            // None. A slice, which is an array on the bus where a Rust array is a struct.
            let actions: &[&str] = &[];
            let hints: HashMap<&str, Value<'_>> = HashMap::new();
            // Replacing none, and shown for as long as the daemon's default.
            let notification = (
                "Caminito", 0u32, ICON, &summary, &body, actions, hints, -1i32,
            );
            let called = connection
                .call_method(Some(NAME), PATH, Some(INTERFACE), "Notify", &notification)
                .await;
            if let Err(err) = called {
                tracing::warn!("can't post the notification {summary:?}: {err}");
            }
        };
        dbus.connection()
            .executor()
            .spawn(call, "notification")
            .detach();
    }
}
