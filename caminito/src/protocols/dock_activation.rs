//! `caminito_dock_activation_manager_v1` (CMN-245, ADR 0050): Caminito binds the keys that
//! activate a Dock item, since an applet can't grab keys on Wayland, and tells a Dock applet
//! which item the user asked for. Each applet makes a `caminito_dock_activation_v1` for its
//! panel's surface.
//!
//! This module owns the global and the applets' objects. It knows nothing of outputs: the
//! compositor reads each applet's surface from [`DockActivationState::applets`], and chooses
//! the one it tells. It's offered to the clients that may make layer surfaces.

use smithay::reexports::wayland_server::backend::{ClientId, GlobalId};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource, Weak,
};
use smithay::wayland::{Dispatch2, GlobalDispatch2};

use crate::protocols::GlobalFilter;
use crate::protocols::raw::dock_activation::caminito_dock_activation_manager_v1::{
    self, CaminitoDockActivationManagerV1,
};
use crate::protocols::raw::dock_activation::caminito_dock_activation_v1::{
    self, CaminitoDockActivationV1,
};

/// The protocol's one version.
const VERSION: u32 = 1;

/// What the compositor lends the protocol: where its state is.
pub trait DockActivationHandler: 'static {
    /// The protocol's state.
    fn dock_activation_state(&mut self) -> &mut DockActivationState;
}

/// The manager's global and the applets that made an object, oldest first.
#[derive(Debug)]
pub struct DockActivationState {
    _global: GlobalId,
    applets: Vec<DockApplet>,
}

/// A Dock applet that is told through its object.
#[derive(Debug)]
pub struct DockApplet {
    object: CaminitoDockActivationV1,
    /// The panel's surface the applet named.
    surface: Weak<WlSurface>,
}

/// The manager global's data: the clients it's offered to.
pub struct ManagerGlobalData {
    filter: GlobalFilter,
}

/// A manager's data.
#[derive(Debug)]
pub struct ManagerData;

/// A `caminito_dock_activation_v1`'s data.
#[derive(Debug)]
pub struct DockActivationData;

impl DockActivationState {
    /// Offers the manager to the clients `filter` allows.
    pub fn new<D, F>(display: &DisplayHandle, filter: F) -> Self
    where
        D: GlobalDispatch<CaminitoDockActivationManagerV1, ManagerGlobalData> + 'static,
        F: for<'c> Fn(&'c Client) -> bool + Send + Sync + 'static,
    {
        let data = ManagerGlobalData {
            filter: Box::new(filter),
        };
        let global = display.create_global::<D, CaminitoDockActivationManagerV1, _>(VERSION, data);
        Self {
            _global: global,
            applets: Vec::new(),
        }
    }

    /// The applets that can be told now, oldest first.
    pub fn applets(&self) -> impl Iterator<Item = &DockApplet> {
        self.applets
            .iter()
            .filter(|applet| applet.object.is_alive())
    }
}

impl DockApplet {
    /// The panel's surface, unless it's gone.
    pub fn surface(&self) -> Option<WlSurface> {
        self.surface.upgrade().ok()
    }

    /// Tells the applet to activate its Dock item at `index`, counted from 0.
    pub fn activate(&self, index: u32) {
        self.object.activate(index);
    }
}

impl<D> GlobalDispatch2<CaminitoDockActivationManagerV1, D> for ManagerGlobalData
where
    D: DockActivationHandler + Dispatch<CaminitoDockActivationManagerV1, ManagerData>,
{
    fn bind(
        &self,
        _state: &mut D,
        _display: &DisplayHandle,
        _client: &Client,
        resource: New<CaminitoDockActivationManagerV1>,
        data_init: &mut DataInit<'_, D>,
    ) {
        data_init.init(resource, ManagerData);
    }

    fn can_view(&self, client: &Client) -> bool {
        (self.filter)(client)
    }
}

impl<D> Dispatch2<CaminitoDockActivationManagerV1, D> for ManagerData
where
    D: DockActivationHandler + Dispatch<CaminitoDockActivationV1, DockActivationData>,
{
    fn request(
        &self,
        state: &mut D,
        _client: &Client,
        _resource: &CaminitoDockActivationManagerV1,
        request: caminito_dock_activation_manager_v1::Request,
        _display: &DisplayHandle,
        data_init: &mut DataInit<'_, D>,
    ) {
        use caminito_dock_activation_manager_v1::Request;

        match request {
            Request::GetDockActivation { id, surface } => {
                let object = data_init.init(id, DockActivationData);
                let applets = &mut state.dock_activation_state().applets;
                applets.retain(|applet| applet.object.is_alive());
                applets.push(DockApplet {
                    object,
                    surface: surface.downgrade(),
                });
            }
            // `destroy`: the objects it made go on.
            Request::Destroy => {}
        }
    }
}

impl<D: DockActivationHandler> Dispatch2<CaminitoDockActivationV1, D> for DockActivationData {
    fn request(
        &self,
        _state: &mut D,
        _client: &Client,
        _resource: &CaminitoDockActivationV1,
        _request: caminito_dock_activation_v1::Request,
        _display: &DisplayHandle,
        _data_init: &mut DataInit<'_, D>,
    ) {
        // `destroy` is its only request, and `destroyed` follows it.
    }

    fn destroyed(&self, state: &mut D, _client: ClientId, object: &CaminitoDockActivationV1) {
        state
            .dock_activation_state()
            .applets
            .retain(|applet| applet.object != *object);
    }
}
