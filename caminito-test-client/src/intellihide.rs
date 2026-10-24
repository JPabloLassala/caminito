//! A `caminito_intellihide_manager_v1` client, as the patched mate-panel is (CMN-224, ADR
//! 0047): it asks for intellihide on a layer surface. The protocol is generated from the XML
//! Caminito's own code is generated from.

use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};

use self::raw::caminito_intellihide_manager_v1::{self, CaminitoIntellihideManagerV1};
use self::raw::caminito_intellihide_v1::{self, CaminitoIntellihideV1};
use crate::layer_shell::LayerId;
use crate::{Client, State};

/// Caminito's two interfaces, from `wayland-scanner`.
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
    use wayland_protocols_wlr::layer_shell::v1::client::*;

    pub mod __interfaces {
        use wayland_client::backend as wayland_backend;
        use wayland_client::protocol::__interfaces::*;
        use wayland_protocols_wlr::layer_shell::v1::client::__interfaces::*;
        wayland_scanner::generate_interfaces!("../caminito/resources/caminito-intellihide-v1.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("../caminito/resources/caminito-intellihide-v1.xml");
}

impl Client {
    /// Whether the compositor offers `caminito_intellihide_manager_v1`, as of the last
    /// roundtrip.
    pub fn has_intellihide_manager(&self) -> bool {
        let interface = CaminitoIntellihideManagerV1::interface().name;
        self.state.globals.iter().any(|g| g.interface == interface)
    }

    /// Asks for intellihide on the layer surface, as the patched panel does when the
    /// panel's Intellihide is on. It is the object that asks, so a second call makes a
    /// second object, which the compositor refuses.
    pub fn ask_for_intellihide(&mut self, layer: LayerId) {
        let manager = self.global(|state| &mut state.intellihide_manager, 1);
        let surface = &self.layer_surface(layer).layer_surface;
        let object = manager.get_intellihide(surface, &self.qh, ());
        self.state.intellihides.push((layer, object));
    }

    /// Stops asking for intellihide on the layer surface, as the panel does when its
    /// Intellihide goes off.
    pub fn stop_asking_for_intellihide(&mut self, layer: LayerId) {
        let (asking, rest) = std::mem::take(&mut self.state.intellihides)
            .into_iter()
            .partition(|(asked, _)| *asked == layer);
        self.state.intellihides = rest;
        for (_, object) in asking {
            object.destroy();
        }
    }
}

impl Dispatch<CaminitoIntellihideManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &CaminitoIntellihideManagerV1,
        event: caminito_intellihide_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // The interface has no events.
        match event {}
    }
}

impl Dispatch<CaminitoIntellihideV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &CaminitoIntellihideV1,
        event: caminito_intellihide_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // The panel is never told that it is hidden.
        match event {}
    }
}
