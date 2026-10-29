//! The daemon's private API (ADR 0051), `org.mate.Caminito.Notifications` at
//! `/org/mate/Caminito/Notifications`, on the name the program owns as a single-instance
//! application. Its clients are the Notifications applet and the Dock applet, which ship in
//! the same package, so it's unversioned. It opens the settings window (CMN-261); the
//! Notification history's calls (CMN-259) and Do Not Disturb's (CMN-263) join it.

use gio::glib::{self, Variant};

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
    /// Exports the private API on `connection`. `open_settings` opens the settings window,
    /// at the page of the Notifying app it's given, or at the General page for none.
    pub fn new(
        connection: &gio::DBusConnection,
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
                handle(&open_settings, method, &parameters, invocation);
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
        _ => invocation.return_dbus_error(
            "org.freedesktop.DBus.Error.UnknownMethod",
            &format!("there's no method {method}"),
        ),
    }
}
