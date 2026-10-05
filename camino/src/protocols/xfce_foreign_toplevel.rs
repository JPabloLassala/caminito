//! Xfce's `xfce_foreign_toplevel_management_private_v1` (75, ADR 0013), for the one thing
//! neither foreign toplevel management nor ext-workspace says: which workspace a toplevel
//! is on. libxfce4windowing reads it from here, and with it mate-panel's window list shows
//! the active workspace's windows only.
//!
//! A client asks for an Xfce handle beside a foreign toplevel handle it has, and the Xfce
//! handle names the client's own ext-workspace handle for the toplevel's workspace. This
//! module owns those handles and what each was last sent. It knows nothing of windows or of
//! policy: the compositor gives it each toplevel's workspace in its refresh pass, through
//! [`XfceForeignToplevelState::refresh`], and it finds the client's objects in the other
//! two protocols' states.
//!
//! The rest of the protocol isn't Camino's to offer yet. No extra state is sent, so a
//! toplevel has no icon of its own and libxfce4windowing finds one by its app id, as it
//! did. The requests that change a toplevel are taken and ignored, and an icon's pixels
//! always fail.
//!
//! Written from the protocol's XML.

use smithay::reexports::wayland_protocols::ext::workspace::v1::server::ext_workspace_handle_v1::ExtWorkspaceHandleV1;
use smithay::reexports::wayland_protocols_wlr::foreign_toplevel::v1::server::zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1;
use smithay::reexports::wayland_server::backend::{ClientId, GlobalId};
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
};
use smithay::wayland::{Dispatch2, GlobalDispatch2};

use super::GlobalFilter;
use super::ext_workspace::ExtWorkspaceState;
use super::foreign_toplevel::{ForeignToplevelState, ToplevelId};
use super::raw::xfce::xfce_foreign_toplevel_handle_v1::{self, XfceForeignToplevelHandleV1};
use super::raw::xfce::xfce_foreign_toplevel_icon_pixels_v1::{
    self, XfceForeignToplevelIconPixelsV1,
};
use super::raw::xfce::xfce_foreign_toplevel_manager_private_v1::{
    self, XfceForeignToplevelManagerPrivateV1,
};

/// The version of the protocol Camino offers, which is the only one there is.
const VERSION: u32 = 1;

/// Where the compositor keeps the protocol's state. There are no requests for it to answer.
pub trait XfceForeignToplevelHandler: 'static {
    /// The compositor's state for Xfce's foreign toplevel protocol.
    fn xfce_foreign_toplevel_state(&mut self) -> &mut XfceForeignToplevelState;
}

/// The manager's global and every client's Xfce handles.
#[derive(Debug)]
pub struct XfceForeignToplevelState {
    _global: GlobalId,
    handles: Vec<Handle>,
}

#[derive(Debug)]
struct Handle {
    resource: XfceForeignToplevelHandleV1,
    /// The foreign toplevel handle it's beside, which is the same client's.
    toplevel: ZwlrForeignToplevelHandleV1,
    /// The client's workspace handle it has entered.
    workspace: Option<ExtWorkspaceHandleV1>,
    /// Whether its first details were ended with `done`.
    announced: bool,
}

/// The manager global's data: the clients it's offered to.
pub struct ManagerGlobalData {
    filter: GlobalFilter,
}

/// A manager's data.
#[derive(Debug)]
pub struct ManagerData;

/// An Xfce handle's data.
#[derive(Debug)]
pub struct HandleData;

/// The data of an icon's pixels.
#[derive(Debug)]
pub struct IconPixelsData;

impl XfceForeignToplevelState {
    /// Offers the manager's global to the clients `filter` allows.
    pub fn new<D, F>(display: &DisplayHandle, filter: F) -> Self
    where
        D: GlobalDispatch<XfceForeignToplevelManagerPrivateV1, ManagerGlobalData> + 'static,
        F: for<'c> Fn(&'c Client) -> bool + Send + Sync + 'static,
    {
        let data = ManagerGlobalData {
            filter: Box::new(filter),
        };
        let global =
            display.create_global::<D, XfceForeignToplevelManagerPrivateV1, _>(VERSION, data);
        Self {
            _global: global,
            handles: Vec::new(),
        }
    }

    /// Tells every client of the toplevels that changed workspace, each ended with the
    /// foreign toplevel handle's `done`, and ends a new handle's first details with one.
    /// `workspace_of` gives the id ext-workspace knows a toplevel's workspace by.
    ///
    /// A client is told only of a workspace it has a handle for in `workspaces`. So this is
    /// for the compositor to call both before ext-workspace's own refresh, while a client
    /// still has the handle of a workspace that's going, to leave it, and after it, when a
    /// client has the handle of one that's new, to enter it.
    pub fn refresh(
        &mut self,
        toplevels: &ForeignToplevelState,
        workspaces: &ExtWorkspaceState,
        workspace_of: impl Fn(ToplevelId) -> Option<String>,
    ) {
        for handle in &mut self.handles {
            // A handle whose toplevel's was sent `closed` is inert, as that one is.
            let Some(toplevel) = toplevels.toplevel(&handle.toplevel) else {
                continue;
            };
            let workspace = handle
                .resource
                .client()
                .and_then(|client| workspaces.workspace_handle(&client, &workspace_of(toplevel)?));
            let mut changed = !handle.announced;
            if workspace != handle.workspace {
                // A workspace handle the client destroyed needs no leave.
                if let Some(left) = handle.workspace.take()
                    && left.is_alive()
                {
                    handle.resource.workspace_leave(&left);
                }
                if let Some(entered) = &workspace {
                    handle.resource.workspace_enter(entered);
                }
                handle.workspace = workspace;
                changed = true;
            }
            if changed {
                handle.toplevel.done();
                handle.announced = true;
            }
        }
    }
}

impl<D> GlobalDispatch2<XfceForeignToplevelManagerPrivateV1, D> for ManagerGlobalData
where
    D: XfceForeignToplevelHandler + Dispatch<XfceForeignToplevelManagerPrivateV1, ManagerData>,
{
    fn bind(
        &self,
        _state: &mut D,
        _display: &DisplayHandle,
        _client: &Client,
        resource: New<XfceForeignToplevelManagerPrivateV1>,
        data_init: &mut DataInit<'_, D>,
    ) {
        data_init.init(resource, ManagerData);
    }

    fn can_view(&self, client: &Client) -> bool {
        (self.filter)(client)
    }
}

impl<D> Dispatch2<XfceForeignToplevelManagerPrivateV1, D> for ManagerData
where
    D: XfceForeignToplevelHandler + Dispatch<XfceForeignToplevelHandleV1, HandleData>,
{
    fn request(
        &self,
        state: &mut D,
        _client: &Client,
        resource: &XfceForeignToplevelManagerPrivateV1,
        request: xfce_foreign_toplevel_manager_private_v1::Request,
        _display: &DisplayHandle,
        data_init: &mut DataInit<'_, D>,
    ) {
        use xfce_foreign_toplevel_manager_private_v1::{Error, Request};

        match request {
            Request::GetXfceForeignToplevelHandle {
                id,
                foreign_toplevel_handle: toplevel,
            } => {
                let handles = &mut state.xfce_foreign_toplevel_state().handles;
                if handles.iter().any(|handle| handle.toplevel == toplevel) {
                    resource.post_error(
                        Error::AlreadyBound,
                        format!("{} already has an Xfce handle", toplevel.id()),
                    );
                    return;
                }
                // What it's sent, its first `done` too, goes in the refresh pass.
                handles.push(Handle {
                    resource: data_init.init(id, HandleData),
                    toplevel,
                    workspace: None,
                    announced: false,
                });
            }
            // The manager's handles live on, as the protocol has no word for their end.
            Request::Destroy => {}
        }
    }
}

impl<D> Dispatch2<XfceForeignToplevelHandleV1, D> for HandleData
where
    D: XfceForeignToplevelHandler + Dispatch<XfceForeignToplevelIconPixelsV1, IconPixelsData>,
{
    fn request(
        &self,
        _state: &mut D,
        _client: &Client,
        _resource: &XfceForeignToplevelHandleV1,
        request: xfce_foreign_toplevel_handle_v1::Request,
        _display: &DisplayHandle,
        data_init: &mut DataInit<'_, D>,
    ) {
        use xfce_foreign_toplevel_handle_v1::Request;
        use xfce_foreign_toplevel_icon_pixels_v1::FailureReason;

        match request {
            // Camino says where a toplevel is, and no more (ADR 0013). What a window list
            // asks of a window, it asks through foreign toplevel management.
            Request::SetShaded
            | Request::UnsetShaded
            | Request::SetSticky
            | Request::UnsetSticky
            | Request::SetAbove
            | Request::UnsetAbove
            | Request::SetBelow
            | Request::UnsetBelow
            | Request::Highlight { .. }
            | Request::Unhighlight
            | Request::Move { .. }
            | Request::Resize { .. }
            | Request::MoveToOutput { .. }
            | Request::MoveToWorkspace { .. } => {}
            // No size was ever advertised, so there are no pixels to ask for.
            Request::GetIconPixels { id, .. } => {
                data_init
                    .init(id, IconPixelsData)
                    .failed(FailureReason::NoPixelData);
            }
            // The handle is forgotten in `destroyed`.
            Request::Destroy => {}
        }
    }

    fn destroyed(&self, state: &mut D, _client: ClientId, resource: &XfceForeignToplevelHandleV1) {
        let handles = &mut state.xfce_foreign_toplevel_state().handles;
        handles.retain(|handle| handle.resource != *resource);
    }
}

impl<D: XfceForeignToplevelHandler> Dispatch2<XfceForeignToplevelIconPixelsV1, D>
    for IconPixelsData
{
    fn request(
        &self,
        _state: &mut D,
        _client: &Client,
        _resource: &XfceForeignToplevelIconPixelsV1,
        request: xfce_foreign_toplevel_icon_pixels_v1::Request,
        _display: &DisplayHandle,
        _data_init: &mut DataInit<'_, D>,
    ) {
        match request {
            xfce_foreign_toplevel_icon_pixels_v1::Request::Destroy => {}
        }
    }
}
