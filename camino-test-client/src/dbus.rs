//! zbus proxies for Camino's D-Bus names (15), used on the test binary's private bus.
//!
//! The calls block, and Camino answers them from its event loop, so a test makes them on
//! another thread while it dispatches the compositor.

/// `org.mate.Camino.Debug` (14).
#[zbus::proxy(
    interface = "org.mate.Camino.Debug",
    default_service = "org.mate.Camino.Debug",
    default_path = "/org/mate/Camino/Debug",
    gen_async = false,
    blocking_name = "DebugProxy"
)]
pub trait Debug {
    /// The state dump, as JSON.
    fn dump_state(&self) -> zbus::Result<String>;

    /// Turns frame debug on or off, and returns whether it's now on.
    fn toggle_frame_debug(&self) -> zbus::Result<bool>;
}

/// A proxy for `org.mate.Camino.Debug` at `destination`, a unique or well-known name, on a
/// new connection to the session bus.
pub fn debug_proxy(destination: &str) -> zbus::Result<DebugProxy<'static>> {
    let connection = zbus::blocking::Connection::session()?;
    DebugProxy::builder(&connection)
        .destination(destination.to_owned())?
        .build()
}

/// `org.mate.Camino` (ADR 0002's third channel).
#[zbus::proxy(
    interface = "org.mate.Camino",
    default_service = "org.mate.Camino",
    default_path = "/org/mate/Camino",
    gen_async = false,
    blocking_name = "CaminoProxy"
)]
pub trait Camino {
    /// The primary output's name (81), or the empty string when there are no outputs.
    #[zbus(property)]
    fn primary_output(&self) -> zbus::Result<String>;

    /// Chooses the primary output, by the name of an enabled output.
    #[zbus(property)]
    fn set_primary_output(&self, name: &str) -> zbus::Result<()>;

    /// Saves the layout the connected monitors have now (83).
    fn save_layout(&self) -> zbus::Result<()>;

    /// Grabs each accelerator, in GTK's syntax, and returns its action id, or 0 for one
    /// that wasn't grabbed (98).
    fn grab_accelerators(&self, accelerators: &[&str]) -> zbus::Result<Vec<u32>>;

    /// Releases the caller's grabs with these action ids.
    fn ungrab_accelerators(&self, actions: &[u32]) -> zbus::Result<()>;

    /// A grabbed accelerator was pressed, at `time` in the key event's milliseconds. Only
    /// its grabber is sent it.
    #[zbus(signal)]
    fn accelerator_activated(&self, action: u32, time: u32) -> zbus::Result<()>;
}

/// A proxy for `org.mate.Camino` at `destination`, a unique or well-known name, on a new
/// connection to the session bus.
pub fn camino_proxy(destination: &str) -> zbus::Result<CaminoProxy<'static>> {
    let connection = zbus::blocking::Connection::session()?;
    CaminoProxy::builder(&connection)
        .destination(destination.to_owned())?
        .build()
}

/// `org.mate.ScreenSaver`, the Screensaver shim (12), with mate-screensaver's methods.
#[zbus::proxy(
    interface = "org.mate.ScreenSaver",
    default_service = "org.mate.ScreenSaver",
    default_path = "/org/mate/ScreenSaver",
    gen_async = false,
    blocking_name = "ScreenSaverProxy"
)]
pub trait ScreenSaver {
    fn lock(&self) -> zbus::Result<()>;
    fn unlock(&self) -> zbus::Result<()>;
    fn cycle(&self) -> zbus::Result<()>;
    fn simulate_user_activity(&self) -> zbus::Result<()>;
    /// Returns the inhibitor's cookie.
    fn inhibit(&self, application_name: &str, reason: &str) -> zbus::Result<u32>;
    fn un_inhibit(&self, cookie: u32) -> zbus::Result<()>;
    /// A description of each inhibitor.
    fn get_inhibitors(&self) -> zbus::Result<Vec<String>>;
    /// Returns the throttle's cookie.
    fn throttle(&self, application_name: &str, reason: &str) -> zbus::Result<u32>;
    fn un_throttle(&self, cookie: u32) -> zbus::Result<()>;
    fn get_active(&self) -> zbus::Result<bool>;
    fn get_active_time(&self) -> zbus::Result<u32>;
    fn set_active(&self, value: bool) -> zbus::Result<()>;
    fn show_message(&self, summary: &str, body: &str, icon: &str) -> zbus::Result<()>;
}

/// `org.freedesktop.ScreenSaver`, the Screensaver shim's other name (12).
#[zbus::proxy(
    interface = "org.freedesktop.ScreenSaver",
    default_service = "org.freedesktop.ScreenSaver",
    default_path = "/org/freedesktop/ScreenSaver",
    gen_async = false,
    blocking_name = "FreedesktopScreenSaverProxy"
)]
pub trait FreedesktopScreenSaver {
    /// Returns the inhibitor's cookie.
    fn inhibit(&self, application_name: &str, reason_for_inhibit: &str) -> zbus::Result<u32>;
    fn un_inhibit(&self, cookie: u32) -> zbus::Result<()>;
}

/// A proxy for `org.mate.ScreenSaver` at `destination` on `connection`. The caller it is to
/// Camino lasts as long as `connection` does.
pub fn screensaver_proxy(
    connection: &zbus::blocking::Connection,
    destination: &str,
) -> zbus::Result<ScreenSaverProxy<'static>> {
    ScreenSaverProxy::builder(connection)
        .destination(destination.to_owned())?
        .build()
}

/// A proxy for `org.freedesktop.ScreenSaver` at `destination` and `path` on `connection`.
pub fn freedesktop_screensaver_proxy(
    connection: &zbus::blocking::Connection,
    destination: &str,
    path: &str,
) -> zbus::Result<FreedesktopScreenSaverProxy<'static>> {
    FreedesktopScreenSaverProxy::builder(connection)
        .destination(destination.to_owned())?
        .path(path.to_owned())?
        .build()
}

/// A new connection to the session bus.
pub fn session() -> zbus::Result<zbus::blocking::Connection> {
    zbus::blocking::Connection::session()
}
