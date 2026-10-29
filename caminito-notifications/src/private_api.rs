//! The daemon's private API (ADR 0051), `org.mate.Caminito.Notifications` at
//! `/org/mate/Caminito/Notifications`, on the name the program owns as a single-instance
//! application. Its clients are the Notifications applet and the Dock applet, which ship in
//! the same package, so it's unversioned. It opens the settings window (CMN-261) and serves
//! the Notification history (CMN-259), whose calls the server answers; Do Not Disturb's
//! (CMN-263) join it.

use gio::glib::{self, Variant};

use crate::server::HistoryCalls;

/// The private API's interface.
pub const INTERFACE_NAME: &str = "org.mate.Caminito.Notifications";
/// Where it's exported.
pub const PATH: &str = "/org/mate/Caminito/Notifications";

const INTERFACE: &str = r#"
<node>
  <interface name="org.mate.Caminito.Notifications">
    <!-- Opens the settings window, at the page of the Notifying app whose key is app, or
         at the General page for "". -->
    <method name="OpenSettings">
      <arg type="s" name="app" direction="in"/>
    </method>

    <!-- The history, newest first. An entry is: id, the Notifying app's key, its name, its
         icon (a path or a theme's name), summary, body as Pango markup, image (a string like
         the icon, or the pixels of image-data as (iiibiiay), in a variant), actions without
         the default as (key, label), has a default action, resident, the reply field's
         placeholder or empty for none, and the time in seconds since the epoch. -->
    <method name="List">
      <arg type="a(usssssva(ss)bbst)" name="entries" direction="out"/>
    </method>
    <method name="Dismiss">
      <arg type="u" name="id" direction="in"/>
    </method>
    <method name="ClearApp">
      <arg type="s" name="app" direction="in"/>
    </method>
    <method name="ClearAll"/>
    <!-- The key "default" is a click: the default action, or else Caminito brings the app
         forward. The token is an xdg-activation token, or empty. -->
    <method name="InvokeAction">
      <arg type="u" name="id" direction="in"/>
      <arg type="s" name="key" direction="in"/>
      <arg type="s" name="token" direction="in"/>
    </method>
    <method name="Reply">
      <arg type="u" name="id" direction="in"/>
      <arg type="s" name="text" direction="in"/>
    </method>
    <!-- Each Notifying app's entries in the history, by its key. -->
    <method name="Counts">
      <arg type="a{su}" name="counts" direction="out"/>
    </method>
    <!-- The history changed: list it again. -->
    <signal name="HistoryChanged"/>
    <signal name="CountsChanged">
      <arg type="a{su}" name="counts"/>
    </signal>
  </interface>
</node>
"#;

/// A failure to export the private API.
#[derive(Debug)]
pub enum Error {
    /// The object can't be exported on the bus.
    Export(glib::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Export(err) => write!(f, "can't export the private API: {err}"),
        }
    }
}

impl std::error::Error for Error {}

/// The private API, on the bus until it's dropped.
pub struct PrivateApi {
    connection: gio::DBusConnection,
    registration: Option<gio::RegistrationId>,
}

impl PrivateApi {
    /// Exports the private API on `connection`. `history` answers the Notification history's
    /// calls, and `open_settings` opens the settings window, at the page of the Notifying app
    /// it's given, or at the General page for none.
    pub fn new(
        connection: &gio::DBusConnection,
        history: HistoryCalls,
        open_settings: impl Fn(Option<&str>) + 'static,
    ) -> Result<PrivateApi, Error> {
        let node =
            gio::DBusNodeInfo::for_xml(INTERFACE).expect("the private API's interface is valid");
        let interface = node
            .lookup_interface(INTERFACE_NAME)
            .expect("the node has the private API's interface");
        let registration = connection
            .register_object(PATH, &interface)
            .method_call(move |_, _, _, _, method, parameters, invocation| {
                handle(&history, &open_settings, method, &parameters, invocation);
            })
            .build()
            .map_err(Error::Export)?;
        Ok(PrivateApi {
            connection: connection.clone(),
            registration: Some(registration),
        })
    }
}

impl Drop for PrivateApi {
    fn drop(&mut self) {
        if let Some(registration) = self.registration.take() {
            let _ = self.connection.unregister_object(registration);
        }
    }
}

/// Answers a client's call. gio has checked its arguments against [`INTERFACE`].
fn handle(
    history: &HistoryCalls,
    open_settings: &impl Fn(Option<&str>),
    method: &str,
    parameters: &Variant,
    invocation: gio::DBusMethodInvocation,
) {
    match method {
        "OpenSettings" => {
            let app = parameters.child_value(0);
            open_settings(app.str().filter(|app| !app.is_empty()));
            invocation.return_value(None);
        }
        _ => history.handle(method, parameters, invocation),
    }
}
