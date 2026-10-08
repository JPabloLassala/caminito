//! An `org_kde_kwin_appmenu_manager` client, as Qt 6 and Firefox are (112): it says where a
//! window's `com.canonical.dbusmenu` is on the bus. The protocol is generated from the XML
//! Caminito's own code is generated from.

use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};

use self::raw::org_kde_kwin_appmenu::{self, OrgKdeKwinAppmenu};
use self::raw::org_kde_kwin_appmenu_manager::{self, OrgKdeKwinAppmenuManager};
use crate::{Client, State, WindowId};

/// KDE's two interfaces, from `wayland-scanner`.
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
        wayland_scanner::generate_interfaces!("../caminito/resources/appmenu.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("../caminito/resources/appmenu.xml");
}

impl Client {
    /// Whether the compositor offers `org_kde_kwin_appmenu_manager`, as of the last
    /// roundtrip.
    pub fn has_appmenu_manager(&self) -> bool {
        let interface = OrgKdeKwinAppmenuManager::interface().name;
        self.state.globals.iter().any(|g| g.interface == interface)
    }

    /// Says where the window's dbusmenu is on the bus, as Qt does when a window with a
    /// menubar is shown. The window's `org_kde_kwin_appmenu` is made on first use.
    pub fn set_appmenu_address(&mut self, id: WindowId, service_name: &str, object_path: &str) {
        if self.window(id).appmenu.is_none() {
            let manager = self.global(|state| &mut state.appmenu_manager, 1);
            let appmenu = manager.create(&self.window(id).surface, &self.qh, ());
            self.window_mut(id).appmenu = Some(appmenu);
        }
        let appmenu = self.window(id).appmenu.as_ref().expect("made above");
        appmenu.set_address(service_name.to_owned(), object_path.to_owned());
    }

    /// Releases the window's `org_kde_kwin_appmenu`, as Qt does when the window's menubar
    /// goes.
    pub fn release_appmenu(&mut self, id: WindowId) {
        if let Some(appmenu) = self.window_mut(id).appmenu.take() {
            appmenu.release();
        }
    }
}

impl Dispatch<OrgKdeKwinAppmenuManager, ()> for State {
    fn event(
        _: &mut Self,
        _: &OrgKdeKwinAppmenuManager,
        event: org_kde_kwin_appmenu_manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // The interface has no events.
        match event {}
    }
}

impl Dispatch<OrgKdeKwinAppmenu, ()> for State {
    fn event(
        _: &mut Self,
        _: &OrgKdeKwinAppmenu,
        event: org_kde_kwin_appmenu::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // The interface has no events.
        match event {}
    }
}
