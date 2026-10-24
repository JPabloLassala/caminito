//! `caminito_toplevel_identifier_manager_v1` (CMN-243, ADR 0049): the Dock applet has a
//! window's `zwlr_foreign_toplevel_handle_v1` from libxfce4windowing, and asks with it for
//! the identifier that ext-foreign-toplevel-list gives the window, which is what image
//! capture and `org.mate.Caminito` name a window by.
//!
//! This module owns the global and answers each request at once. It knows nothing of
//! windows: the compositor says, through [`ToplevelIdentifierHandler`], which identifier a
//! handle's window has. It's offered to the clients that may see the other windows.

use smithay::reexports::wayland_protocols_wlr::foreign_toplevel::v1::server::zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1;
use smithay::reexports::wayland_server::backend::GlobalId;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New,
};
use smithay::wayland::{Dispatch2, GlobalDispatch2};

use crate::protocols::GlobalFilter;
use crate::protocols::raw::toplevel_identifier::caminito_toplevel_identifier_manager_v1::{
    self, CaminitoToplevelIdentifierManagerV1,
};
use crate::protocols::raw::toplevel_identifier::caminito_toplevel_identifier_v1::{
    self, CaminitoToplevelIdentifierV1,
};

/// The protocol's one version.
const VERSION: u32 = 1;

/// What the compositor lends the protocol: the name of a window.
pub trait ToplevelIdentifierHandler: 'static {
    /// The identifier of the foreign toplevel list for the window `handle` is for, or
    /// `None` if the window is gone or isn't listed.
    fn identifier_of(&mut self, handle: &ZwlrForeignToplevelHandleV1) -> Option<String>;
}

/// The manager's global.
#[derive(Debug)]
pub struct ToplevelIdentifierState {
    _global: GlobalId,
}

/// The manager global's data: the clients it's offered to.
pub struct ManagerGlobalData {
    filter: GlobalFilter,
}

/// A manager's data.
#[derive(Debug)]
pub struct ManagerData;

/// A `caminito_toplevel_identifier_v1`'s data.
#[derive(Debug)]
pub struct IdentifierData;

impl ToplevelIdentifierState {
    /// Offers the manager to the clients `filter` allows.
    pub fn new<D, F>(display: &DisplayHandle, filter: F) -> Self
    where
        D: GlobalDispatch<CaminitoToplevelIdentifierManagerV1, ManagerGlobalData> + 'static,
        F: for<'c> Fn(&'c Client) -> bool + Send + Sync + 'static,
    {
        let data = ManagerGlobalData {
            filter: Box::new(filter),
        };
        let global =
            display.create_global::<D, CaminitoToplevelIdentifierManagerV1, _>(VERSION, data);
        Self { _global: global }
    }
}

impl<D> GlobalDispatch2<CaminitoToplevelIdentifierManagerV1, D> for ManagerGlobalData
where
    D: ToplevelIdentifierHandler + Dispatch<CaminitoToplevelIdentifierManagerV1, ManagerData>,
{
    fn bind(
        &self,
        _state: &mut D,
        _display: &DisplayHandle,
        _client: &Client,
        resource: New<CaminitoToplevelIdentifierManagerV1>,
        data_init: &mut DataInit<'_, D>,
    ) {
        data_init.init(resource, ManagerData);
    }

    fn can_view(&self, client: &Client) -> bool {
        (self.filter)(client)
    }
}

impl<D> Dispatch2<CaminitoToplevelIdentifierManagerV1, D> for ManagerData
where
    D: ToplevelIdentifierHandler + Dispatch<CaminitoToplevelIdentifierV1, IdentifierData>,
{
    fn request(
        &self,
        state: &mut D,
        _client: &Client,
        _resource: &CaminitoToplevelIdentifierManagerV1,
        request: caminito_toplevel_identifier_manager_v1::Request,
        _display: &DisplayHandle,
        data_init: &mut DataInit<'_, D>,
    ) {
        use caminito_toplevel_identifier_manager_v1::Request;

        match request {
            Request::GetIdentifier { id, handle } => {
                let identifier = state.identifier_of(&handle);
                let object = data_init.init(id, IdentifierData);
                match identifier {
                    Some(identifier) => object.identifier(identifier),
                    None => object.gone(),
                }
            }
            // `destroy`: the objects it made go on.
            Request::Destroy => {}
        }
    }
}

impl<D: ToplevelIdentifierHandler> Dispatch2<CaminitoToplevelIdentifierV1, D> for IdentifierData {
    fn request(
        &self,
        _state: &mut D,
        _client: &Client,
        _resource: &CaminitoToplevelIdentifierV1,
        _request: caminito_toplevel_identifier_v1::Request,
        _display: &DisplayHandle,
        _data_init: &mut DataInit<'_, D>,
    ) {
        // `destroy` is its only request.
    }
}
