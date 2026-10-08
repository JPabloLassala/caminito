//! KDE's `org_kde_kwin_appmenu_manager` (112, ADR 0017): Qt 6 and Firefox say through it
//! where a window's menubar is on the bus, as a `com.canonical.dbusmenu`.
//!
//! This module owns the protocol's objects and each surface's address. It knows nothing of
//! windows: the compositor reads an address with [`address`] when it next publishes the
//! global menu, so there's nothing to tell it and no handler trait. The global is offered
//! only while something shows the menus, since an app that finds it draws no menubar of its
//! own.

use std::sync::Mutex;

use smithay::reexports::wayland_server::backend::{ClientId, GlobalId, ObjectId};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource, Weak,
};
use smithay::wayland::compositor::with_states;
use smithay::wayland::{Dispatch2, GlobalDispatch2};

use crate::protocols::raw::appmenu::org_kde_kwin_appmenu::{self, OrgKdeKwinAppmenu};
use crate::protocols::raw::appmenu::org_kde_kwin_appmenu_manager::{
    self, OrgKdeKwinAppmenuManager,
};

/// The protocol's one version.
const VERSION: u32 = 1;

/// The `org_kde_kwin_appmenu_manager` global, while it's offered.
#[derive(Debug, Default)]
pub struct AppmenuState {
    global: Option<GlobalId>,
}

/// The global's data.
#[derive(Debug)]
pub struct AppmenuGlobalData;

/// An `org_kde_kwin_appmenu_manager`'s data.
#[derive(Debug)]
pub struct AppmenuManagerData;

/// An `org_kde_kwin_appmenu`'s data.
#[derive(Debug)]
pub struct AppmenuData {
    surface: Weak<WlSurface>,
}

/// Where a surface's `com.canonical.dbusmenu` is on the bus, as `set_address` said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    /// The connection that serves the menu.
    pub service_name: String,
    pub object_path: String,
}

/// A surface's address, in its data map, with the `org_kde_kwin_appmenu` that set it: the
/// address goes with that object, and not with another the client made for the surface.
#[derive(Default)]
struct Menu(Mutex<Option<(ObjectId, Address)>>);

impl AppmenuState {
    /// Offers the global to every client, or stops offering it. It returns the global that
    /// is no longer offered, for the caller to tell the clients of and to remove.
    pub fn set_offered<D>(&mut self, display: &DisplayHandle, offered: bool) -> Option<GlobalId>
    where
        D: GlobalDispatch<OrgKdeKwinAppmenuManager, AppmenuGlobalData> + 'static,
    {
        if !offered {
            return self.global.take();
        }
        self.global.get_or_insert_with(|| {
            display.create_global::<D, OrgKdeKwinAppmenuManager, _>(VERSION, AppmenuGlobalData)
        });
        None
    }
}

/// What a client last said, with `org_kde_kwin_appmenu.set_address`, of where `surface`'s
/// menu is on the bus, unless it has released the object since.
pub fn address(surface: &WlSurface) -> Option<Address> {
    with_states(surface, |states| {
        let menu = states.data_map.get::<Menu>()?;
        let menu = menu.0.lock().unwrap();
        menu.as_ref().map(|(_, address)| address.clone())
    })
}

/// Changes what's recorded of where a surface's menu is.
fn with_menu(surface: &WlSurface, change: impl FnOnce(&mut Option<(ObjectId, Address)>)) {
    with_states(surface, |states| {
        let menu = states.data_map.get_or_insert_threadsafe(Menu::default);
        change(&mut menu.0.lock().unwrap());
    });
}

impl<D> GlobalDispatch2<OrgKdeKwinAppmenuManager, D> for AppmenuGlobalData
where
    D: Dispatch<OrgKdeKwinAppmenuManager, AppmenuManagerData>,
{
    fn bind(
        &self,
        _state: &mut D,
        _display: &DisplayHandle,
        _client: &Client,
        resource: New<OrgKdeKwinAppmenuManager>,
        data_init: &mut DataInit<'_, D>,
    ) {
        data_init.init(resource, AppmenuManagerData);
    }
}

impl<D> Dispatch2<OrgKdeKwinAppmenuManager, D> for AppmenuManagerData
where
    D: Dispatch<OrgKdeKwinAppmenu, AppmenuData>,
{
    fn request(
        &self,
        _state: &mut D,
        _client: &Client,
        _resource: &OrgKdeKwinAppmenuManager,
        request: org_kde_kwin_appmenu_manager::Request,
        _display: &DisplayHandle,
        data_init: &mut DataInit<'_, D>,
    ) {
        let org_kde_kwin_appmenu_manager::Request::Create { id, surface } = request;
        let data = AppmenuData {
            surface: surface.downgrade(),
        };
        data_init.init(id, data);
    }
}

impl<D> Dispatch2<OrgKdeKwinAppmenu, D> for AppmenuData {
    fn request(
        &self,
        _state: &mut D,
        _client: &Client,
        resource: &OrgKdeKwinAppmenu,
        request: org_kde_kwin_appmenu::Request,
        _display: &DisplayHandle,
        _data_init: &mut DataInit<'_, D>,
    ) {
        match request {
            org_kde_kwin_appmenu::Request::SetAddress {
                service_name,
                object_path,
            } => {
                let Ok(surface) = self.surface.upgrade() else {
                    return;
                };
                // An address that names no connection or no object is none at all.
                let address = (!service_name.is_empty() && !object_path.is_empty()).then(|| {
                    let address = Address {
                        service_name,
                        object_path,
                    };
                    (resource.id(), address)
                });
                with_menu(&surface, |menu| *menu = address);
            }
            // `destroyed` takes the address away, for a client that goes without it too.
            org_kde_kwin_appmenu::Request::Release => {}
        }
    }

    fn destroyed(&self, _state: &mut D, _client: ClientId, resource: &OrgKdeKwinAppmenu) {
        let Ok(surface) = self.surface.upgrade() else {
            return;
        };
        with_menu(&surface, |menu| {
            if menu.as_ref().is_some_and(|(by, _)| *by == resource.id()) {
                *menu = None;
            }
        });
    }
}
