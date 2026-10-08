//! A `gtk_shell1` client, as GTK3 is one (67): it gives its toplevels a `gtk_surface1`, says
//! through it whether they're modal, and rings the bell. As a `GtkApplication` of GTK3 or
//! GTK4 does, it says where a window's menus are on the bus, and records the capabilities
//! it's sent (110). The protocol is generated from the XML Caminito's own code is generated
//! from.

use wayland_client::{Connection, Dispatch, QueueHandle};

use self::raw::gtk_shell1::{self, GtkShell1};
use self::raw::gtk_surface1::{self, GtkSurface1};
use crate::{Client, State, WindowId};

/// The highest version GTK3 binds (`GTK_SHELL1_VERSION`, gdkdisplay-wayland.c:94).
const GTK3_VERSION: u32 = 5;

/// `gtk_shell1` and `gtk_surface1`, from `wayland-scanner`.
#[allow(
    dead_code,
    non_camel_case_types,
    non_upper_case_globals,
    unused_imports,
    clippy::all,
    reason = "generated code"
)]
pub(crate) mod raw {
    use wayland_client;
    use wayland_client::protocol::*;

    pub mod __interfaces {
        use wayland_client::backend as wayland_backend;
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("../caminito/resources/gtk-shell.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("../caminito/resources/gtk-shell.xml");
}

/// What a `GtkApplication` says of a window with `gtk_surface1.set_dbus_properties`: where
/// its menus and its actions are on the bus.
#[derive(Debug, Clone, Default)]
pub struct DbusProperties {
    pub application_id: Option<&'static str>,
    pub app_menu_path: Option<&'static str>,
    pub menubar_path: Option<&'static str>,
    pub window_object_path: Option<&'static str>,
    pub application_object_path: Option<&'static str>,
    pub unique_bus_name: Option<&'static str>,
}

impl Client {
    /// Binds `gtk_shell1`, as GTK does when it opens the display.
    pub fn bind_gtk_shell(&mut self) {
        self.gtk_shell();
    }

    /// The capabilities `gtk_shell1` was sent, oldest first.
    pub fn gtk_capabilities(&self) -> &[u32] {
        &self.state.gtk_capabilities
    }

    /// Says where the window's menus are on the bus, as `gtk_application_window` does when
    /// the window is realized.
    pub fn set_dbus_properties(&mut self, id: WindowId, properties: &DbusProperties) {
        let owned = |it: Option<&str>| it.map(str::to_owned);
        self.gtk_surface(id).set_dbus_properties(
            owned(properties.application_id),
            owned(properties.app_menu_path),
            owned(properties.menubar_path),
            owned(properties.window_object_path),
            owned(properties.application_object_path),
            owned(properties.unique_bus_name),
        );
    }

    /// `gtk_shell1`, bound on first use at the version GTK3 would bind.
    fn gtk_shell(&mut self) -> GtkShell1 {
        self.global(|state| &mut state.gtk_shell, GTK3_VERSION)
    }

    /// The window's `gtk_surface1`, made on first use as GTK3 makes it.
    fn gtk_surface(&mut self, id: WindowId) -> GtkSurface1 {
        if self.window(id).gtk_surface.is_none() {
            let shell = self.gtk_shell();
            let surface = shell.get_gtk_surface(&self.window(id).surface, &self.qh, ());
            self.window_mut(id).gtk_surface = Some(surface);
        }
        self.window(id).gtk_surface.clone().expect("made above")
    }

    /// Says the window is modal, as `gtk_window_set_modal` does.
    pub fn set_modal(&mut self, id: WindowId) {
        self.gtk_surface(id).set_modal();
    }

    /// Says the window isn't modal any more.
    pub fn unset_modal(&mut self, id: WindowId) {
        self.gtk_surface(id).unset_modal();
    }

    /// Rings the bell for a window, or for none, as `gdk_window_beep` and
    /// `gdk_display_beep` do.
    pub fn system_bell(&mut self, id: Option<WindowId>) {
        let surface = id.map(|id| self.gtk_surface(id));
        self.gtk_shell().system_bell(surface.as_ref());
    }

    /// Sends every other request GTK3 makes at version 1, of `gtk_shell1` and of the
    /// window's `gtk_surface1`, then drops the `gtk_surface1`'s proxy, which is all GTK3
    /// does to destroy one below version 4 (gdkwindow-wayland.c:3584-3588). A Wayland
    /// library refuses the requests that are newer than the version bound, so none is sent.
    pub fn send_other_gtk_requests(&mut self, id: WindowId) {
        let surface = self.gtk_surface(id);
        let shell = self.gtk_shell();
        shell.set_startup_id(Some("caminito-test-0".into()));
        shell.set_startup_id(None);
        surface.set_dbus_properties(
            Some("org.mate.pluma".into()),
            None,
            Some("/org/mate/pluma/menus/menubar".into()),
            Some("/org/mate/pluma/window/1".into()),
            Some("/org/mate/pluma".into()),
            Some(":1.42".into()),
        );
        surface.present(0);
        self.window_mut(id).gtk_surface = None;
    }
}

impl Dispatch<GtkShell1, ()> for State {
    fn event(
        state: &mut Self,
        _: &GtkShell1,
        event: gtk_shell1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let gtk_shell1::Event::Capabilities { capabilities } = event;
        state.gtk_capabilities.push(capabilities);
    }
}

impl Dispatch<GtkSurface1, ()> for State {
    fn event(
        _: &mut Self,
        _: &GtkSurface1,
        _: gtk_surface1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
