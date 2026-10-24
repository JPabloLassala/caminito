//! `caminito_intellihide_manager_v1` (CMN-224, ADR 0047): a panel's layer surface asks, with
//! a `caminito_intellihide_v1` object of its own, to be hidden while a window overlaps it.
//! The object's existence is the whole request, and it has no events, since the panel is
//! never told that it is hidden.
//!
//! This module owns the global and which layer surfaces have an object. It knows nothing of
//! windows or of the slide: the compositor reads [`IntellihideState::asks`] in its refresh
//! pass. It's offered to the clients that may make layer surfaces.

use smithay::reexports::wayland_protocols_wlr::layer_shell::v1::server::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;
use smithay::reexports::wayland_server::backend::{ClientId, GlobalId};
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource, Weak,
};
use smithay::wayland::{Dispatch2, GlobalDispatch2};

use crate::protocols::GlobalFilter;
use crate::protocols::raw::intellihide::caminito_intellihide_manager_v1::{
    self, CaminitoIntellihideManagerV1, Error,
};
use crate::protocols::raw::intellihide::caminito_intellihide_v1::{self, CaminitoIntellihideV1};

/// The protocol's one version.
const VERSION: u32 = 1;

/// What the compositor lends the protocol: where its state is.
pub trait IntellihideHandler: 'static {
    /// The protocol's state.
    fn intellihide_state(&mut self) -> &mut IntellihideState;
}

/// The manager's global and the layer surfaces that ask for intellihide.
#[derive(Debug)]
pub struct IntellihideState {
    _global: GlobalId,
    asking: Vec<Asking>,
}

#[derive(Debug)]
struct Asking {
    /// The layer surface that asked.
    layer_surface: Weak<ZwlrLayerSurfaceV1>,
    /// The object that asks for it, which ending the request destroys.
    object: Weak<CaminitoIntellihideV1>,
}

/// The manager global's data: the clients it's offered to.
pub struct ManagerGlobalData {
    filter: GlobalFilter,
}

/// A manager's data.
#[derive(Debug)]
pub struct ManagerData;

/// A `caminito_intellihide_v1`'s data.
#[derive(Debug)]
pub struct IntellihideData;

impl IntellihideState {
    /// Offers the manager to the clients `filter` allows.
    pub fn new<D, F>(display: &DisplayHandle, filter: F) -> Self
    where
        D: GlobalDispatch<CaminitoIntellihideManagerV1, ManagerGlobalData> + 'static,
        F: for<'c> Fn(&'c Client) -> bool + Send + Sync + 'static,
    {
        let data = ManagerGlobalData {
            filter: Box::new(filter),
        };
        let global = display.create_global::<D, CaminitoIntellihideManagerV1, _>(VERSION, data);
        Self {
            _global: global,
            asking: Vec::new(),
        }
    }

    /// Whether `layer_surface` asks for intellihide now.
    pub fn asks(&self, layer_surface: &ZwlrLayerSurfaceV1) -> bool {
        self.asking
            .iter()
            .any(|asking| asking.is_for(layer_surface) && asking.object.upgrade().is_ok())
    }
}

impl Asking {
    /// Whether it's `layer_surface` that asked.
    fn is_for(&self, layer_surface: &ZwlrLayerSurfaceV1) -> bool {
        self.layer_surface.upgrade().ok().as_ref() == Some(layer_surface)
    }
}

impl<D> GlobalDispatch2<CaminitoIntellihideManagerV1, D> for ManagerGlobalData
where
    D: IntellihideHandler + Dispatch<CaminitoIntellihideManagerV1, ManagerData>,
{
    fn bind(
        &self,
        _state: &mut D,
        _display: &DisplayHandle,
        _client: &Client,
        resource: New<CaminitoIntellihideManagerV1>,
        data_init: &mut DataInit<'_, D>,
    ) {
        data_init.init(resource, ManagerData);
    }

    fn can_view(&self, client: &Client) -> bool {
        (self.filter)(client)
    }
}

impl<D> Dispatch2<CaminitoIntellihideManagerV1, D> for ManagerData
where
    D: IntellihideHandler + Dispatch<CaminitoIntellihideV1, IntellihideData>,
{
    fn request(
        &self,
        state: &mut D,
        _client: &Client,
        resource: &CaminitoIntellihideManagerV1,
        request: caminito_intellihide_manager_v1::Request,
        _display: &DisplayHandle,
        data_init: &mut DataInit<'_, D>,
    ) {
        use caminito_intellihide_manager_v1::Request;

        match request {
            Request::GetIntellihide { id, layer_surface } => {
                let asking = &mut state.intellihide_state().asking;
                asking.retain(|asking| {
                    asking.layer_surface.upgrade().is_ok() && asking.object.upgrade().is_ok()
                });
                if asking.iter().any(|asking| asking.is_for(&layer_surface)) {
                    resource.post_error(
                        Error::AlreadyConstructed,
                        format!("{} already has an intellihide object", layer_surface.id()),
                    );
                    return;
                }
                let object = data_init.init(id, IntellihideData);
                asking.push(Asking {
                    layer_surface: layer_surface.downgrade(),
                    object: object.downgrade(),
                });
            }
            // `destroy`: the objects it made go on.
            Request::Destroy => {}
        }
    }
}

impl<D: IntellihideHandler> Dispatch2<CaminitoIntellihideV1, D> for IntellihideData {
    fn request(
        &self,
        _state: &mut D,
        _client: &Client,
        _resource: &CaminitoIntellihideV1,
        _request: caminito_intellihide_v1::Request,
        _display: &DisplayHandle,
        _data_init: &mut DataInit<'_, D>,
    ) {
        // `destroy` is its only request, and `destroyed` follows it.
    }

    fn destroyed(&self, state: &mut D, _client: ClientId, object: &CaminitoIntellihideV1) {
        state
            .intellihide_state()
            .asking
            .retain(|asking| asking.object.upgrade().ok().as_ref() != Some(object));
    }
}
