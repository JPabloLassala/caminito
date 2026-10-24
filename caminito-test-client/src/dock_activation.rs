//! A `caminito_dock_activation_manager_v1` client, as the Dock applet is (CMN-245, ADR 0050):
//! it makes an object for its panel's surface, and is told through it which Dock item to
//! activate. The protocol is generated from the XML Caminito's own code is generated from.

use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};

use self::raw::caminito_dock_activation_manager_v1::{self, CaminitoDockActivationManagerV1};
use self::raw::caminito_dock_activation_v1::{self, CaminitoDockActivationV1};
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

    pub mod __interfaces {
        use wayland_client::backend as wayland_backend;
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!(
            "../caminito/resources/caminito-dock-activation-v1.xml"
        );
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("../caminito/resources/caminito-dock-activation-v1.xml");
}

/// One Dock applet's object, as `bind_dock_activation` returns it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DockActivationId(usize);

/// The manager, once bound, and the objects made, in the order they were made.
#[derive(Default)]
pub struct DockActivations {
    manager: Option<CaminitoDockActivationManagerV1>,
    applets: Vec<Applet>,
}

struct Applet {
    object: CaminitoDockActivationV1,
    /// The indices `activate` was sent, oldest first.
    activated: Vec<u32>,
}

impl Client {
    /// Whether the compositor offers `caminito_dock_activation_manager_v1`, as of the last
    /// roundtrip.
    pub fn has_dock_activation_manager(&self) -> bool {
        let interface = CaminitoDockActivationManagerV1::interface().name;
        self.state.globals.iter().any(|g| g.interface == interface)
    }

    /// Makes an object for the panel on the layer surface, as a Dock applet in that panel
    /// does.
    pub fn bind_dock_activation(&mut self, panel: LayerId) -> DockActivationId {
        let manager = self.global(|state| &mut state.dock_activations.manager, 1);
        let surface = &self.state.layer_surfaces[panel.0].surface;
        let applets = &mut self.state.dock_activations.applets;
        let object = manager.get_dock_activation(surface, &self.qh, applets.len());
        applets.push(Applet {
            object,
            activated: Vec::new(),
        });
        DockActivationId(applets.len() - 1)
    }

    /// Destroys the object, as an applet that is removed from its panel does.
    pub fn destroy_dock_activation(&mut self, applet: DockActivationId) {
        self.state.dock_activations.applets[applet.0]
            .object
            .destroy();
    }

    /// The indices the object was told to activate, oldest first, as of the last
    /// roundtrip.
    pub fn dock_activations(&self, applet: DockActivationId) -> &[u32] {
        &self.state.dock_activations.applets[applet.0].activated
    }
}

impl Dispatch<CaminitoDockActivationManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &CaminitoDockActivationManagerV1,
        event: caminito_dock_activation_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // The interface has no events.
        match event {}
    }
}

impl Dispatch<CaminitoDockActivationV1, usize> for State {
    fn event(
        state: &mut Self,
        _: &CaminitoDockActivationV1,
        event: caminito_dock_activation_v1::Event,
        applet: &usize,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use caminito_dock_activation_v1::Event;

        match event {
            Event::Activate { index } => {
                state.dock_activations.applets[*applet]
                    .activated
                    .push(index);
            }
        }
    }
}
