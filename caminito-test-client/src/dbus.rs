//! zbus proxies for Caminito's D-Bus names (15), used on the test binary's private bus.
//!
//! The calls block, and Caminito answers them from its event loop, so a test makes them on
//! another thread while it dispatches the compositor.

use std::collections::HashMap;

/// `org.mate.Caminito.Debug` (14).
#[zbus::proxy(
    interface = "org.mate.Caminito.Debug",
    default_service = "org.mate.Caminito.Debug",
    default_path = "/org/mate/Caminito/Debug",
    gen_async = false,
    blocking_name = "DebugProxy"
)]
pub trait Debug {
    /// The state dump, as JSON.
    fn dump_state(&self) -> zbus::Result<String>;

    /// Turns frame debug on or off, and returns whether it's now on.
    fn toggle_frame_debug(&self) -> zbus::Result<bool>;

    /// Switches the output named `output` to `workspace`, from 0, and returns whether there
    /// was such an output and such a workspace.
    fn switch_output_workspace(&self, output: &str, workspace: u32) -> zbus::Result<bool>;
}

/// A proxy for `org.mate.Caminito.Debug` at `destination`, a unique or well-known name, on a
/// new connection to the session bus.
pub fn debug_proxy(destination: &str) -> zbus::Result<DebugProxy<'static>> {
    let connection = zbus::blocking::Connection::session()?;
    DebugProxy::builder(&connection)
        .destination(destination.to_owned())?
        .build()
}

/// `GlobalMenu` as it is on the bus, `(ssa{ss}s)`.
pub type GlobalMenu = (String, String, HashMap<String, String>, String);

/// `org.mate.Caminito` (ADR 0002's third channel).
#[zbus::proxy(
    interface = "org.mate.Caminito",
    default_service = "org.mate.Caminito",
    default_path = "/org/mate/Caminito",
    gen_async = false,
    blocking_name = "CaminitoProxy"
)]
pub trait Caminito {
    /// The primary output's name (81), or the empty string when there are no outputs.
    #[zbus(property)]
    fn primary_output(&self) -> zbus::Result<String>;

    /// Chooses the primary output, by the name of an enabled output.
    #[zbus(property)]
    fn set_primary_output(&self, name: &str) -> zbus::Result<()>;

    /// Whether the active workspace shows its desktop (94).
    #[zbus(property)]
    fn showing_desktop(&self) -> zbus::Result<bool>;

    /// Shows the desktop on the active workspace, or brings its windows back.
    #[zbus(property)]
    fn set_showing_desktop(&self, showing: bool) -> zbus::Result<()>;

    /// The focused window's menu (110): its kind, the bus name that serves it, its object
    /// paths, and the application's id.
    #[zbus(property)]
    fn global_menu(&self) -> zbus::Result<GlobalMenu>;

    /// Whether the global menu is offered, and so whether a menu claim is taken (115).
    #[zbus(property)]
    fn global_menu_offered(&self) -> zbus::Result<bool>;

    /// Night Light (137): `temperature`, `location-source` and `next-sunrise`.
    #[zbus(property)]
    fn night_light(&self) -> zbus::Result<HashMap<String, zbus::zvariant::OwnedValue>>;

    /// The sun's position (215): `elevation`, `azimuth` and `location-source`.
    #[zbus(property)]
    fn sun(&self) -> zbus::Result<HashMap<String, zbus::zvariant::OwnedValue>>;

    /// Flips `night-light-enabled` (137).
    fn toggle_night_light(&self) -> zbus::Result<()>;

    /// Pauses Night Light until the next sunrise (140).
    fn pause_night_light(&self) -> zbus::Result<()>;

    /// Ends Night Light's pause (140).
    fn resume_night_light(&self) -> zbus::Result<()>;

    /// Makes a menu claim (115): the dbusmenu the caller serves at `path` is the menubar
    /// of its process's focused window.
    fn claim_global_menu(&self, path: &zbus::zvariant::ObjectPath<'_>) -> zbus::Result<()>;

    /// Saves the layout the connected monitors have now (83).
    fn save_layout(&self) -> zbus::Result<()>;

    /// The ext-foreign-toplevel-list identifier of the window a window screenshot is of,
    /// or the empty string when there's none (104).
    fn get_active_toplevel(&self) -> zbus::Result<String>;

    /// Grabs each accelerator, in GTK's syntax, and returns its action id, or 0 for one
    /// that wasn't grabbed (98).
    fn grab_accelerators(&self, accelerators: &[&str]) -> zbus::Result<Vec<u32>>;

    /// Releases the caller's grabs with these action ids.
    fn ungrab_accelerators(&self, actions: &[u32]) -> zbus::Result<()>;

    /// A grabbed accelerator was pressed, at `time` in the key event's milliseconds. Only
    /// its grabber is sent it.
    #[zbus(signal)]
    fn accelerator_activated(&self, action: u32, time: u32) -> zbus::Result<()>;

    /// A grabbed accelerator's key is still held, and repeated at `time` (99). Only its
    /// grabber is sent it.
    #[zbus(signal)]
    fn accelerator_repeated(&self, action: u32, time: u32) -> zbus::Result<()>;
}

/// A proxy for `org.mate.Caminito` at `destination`, a unique or well-known name, on a new
/// connection to the session bus.
pub fn caminito_proxy(destination: &str) -> zbus::Result<CaminitoProxy<'static>> {
    let connection = zbus::blocking::Connection::session()?;
    CaminitoProxy::builder(&connection)
        .destination(destination.to_owned())?
        .build()
}

/// `com.canonical.AppMenu.Registrar` (113), where an X11 client says which dbusmenu is a
/// window's.
#[zbus::proxy(
    interface = "com.canonical.AppMenu.Registrar",
    default_service = "com.canonical.AppMenu.Registrar",
    default_path = "/com/canonical/AppMenu/Registrar",
    gen_async = false,
    blocking_name = "RegistrarProxy"
)]
pub trait Registrar {
    /// Says that the caller serves the X window's menu at `menu_object_path`.
    fn register_window(
        &self,
        window_id: u32,
        menu_object_path: &zbus::zvariant::ObjectPath<'_>,
    ) -> zbus::Result<()>;

    /// Takes the X window's menu back.
    fn unregister_window(&self, window_id: u32) -> zbus::Result<()>;

    /// The bus name and the object path of the X window's menu.
    fn get_menu_for_window(
        &self,
        window_id: u32,
    ) -> zbus::Result<(String, zbus::zvariant::OwnedObjectPath)>;
}

/// A proxy for `com.canonical.AppMenu.Registrar` at `destination` on `connection`. The
/// caller it is to Caminito lasts as long as `connection` does.
pub fn registrar_proxy(
    connection: &zbus::blocking::Connection,
    destination: &str,
) -> zbus::Result<RegistrarProxy<'static>> {
    RegistrarProxy::builder(connection)
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

    /// The session was blanked or locked, or is neither any more (127).
    #[zbus(signal)]
    fn active_changed(&self, new_value: bool) -> zbus::Result<()>;
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
/// Caminito lasts as long as `connection` does.
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

/// `org.freedesktop.a11y.KeyboardMonitor`, the Keyboard monitor for Orca (135), as Mutter
/// has it (`data/dbus-interfaces/org.freedesktop.a11y.xml`).
#[zbus::proxy(
    interface = "org.freedesktop.a11y.KeyboardMonitor",
    default_service = "org.freedesktop.a11y.Manager",
    default_path = "/org/freedesktop/a11y/Manager",
    gen_async = false,
    blocking_name = "KeyboardMonitorProxy"
)]
pub trait KeyboardMonitor {
    fn grab_keyboard(&self) -> zbus::Result<()>;
    fn ungrab_keyboard(&self) -> zbus::Result<()>;
    fn watch_keyboard(&self) -> zbus::Result<()>;
    fn unwatch_keyboard(&self) -> zbus::Result<()>;
    /// `modifiers` are keysyms, and each keystroke is a keysym and a modifier mask.
    fn set_key_grabs(&self, modifiers: &[u32], keystrokes: &[(u32, u32)]) -> zbus::Result<()>;

    #[zbus(signal)]
    fn key_event(
        &self,
        released: bool,
        state: u32,
        keysym: u32,
        unichar: u32,
        keycode: u16,
    ) -> zbus::Result<()>;
}

/// A proxy for the Keyboard monitor at `destination` on `connection`. The caller it is to
/// Caminito lasts as long as `connection` does.
pub fn keyboard_monitor_proxy(
    connection: &zbus::blocking::Connection,
    destination: &str,
) -> zbus::Result<KeyboardMonitorProxy<'static>> {
    KeyboardMonitorProxy::builder(connection)
        .destination(destination.to_owned())?
        .build()
}

/// A new connection to the session bus.
pub fn session() -> zbus::Result<zbus::blocking::Connection> {
    zbus::blocking::Connection::session()
}
