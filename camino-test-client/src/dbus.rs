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
