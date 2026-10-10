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
//! It says too whether a toplevel is sticky, kept above or kept below, and takes a
//! client's requests to change those, which it hands the compositor (155). The rest of the
//! protocol isn't Caminito's to offer yet. No icon is sent, so libxfce4windowing finds one by
//! the toplevel's app id, as it did. The other requests that change a toplevel are taken and
//! ignored, and an icon's pixels always fail.
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

/// The version of the protocol Caminito offers, which is the only one there is.
const VERSION: u32 = 1;

/// Where the compositor keeps the protocol's state, and what it does with the requests it
/// honours.
pub trait XfceForeignToplevelHandler: 'static {
    /// The compositor's state for Xfce's foreign toplevel protocol.
    fn xfce_foreign_toplevel_state(&mut self) -> &mut XfceForeignToplevelState;

    /// A client asks to set or unset a state of the toplevel of `toplevel`, a foreign
    /// toplevel handle, which may be one whose toplevel is gone.
    fn set_toplevel_state(
        &mut self,
        toplevel: &ZwlrForeignToplevelHandleV1,
        state: XfceState,
        set: bool,
    );
}

/// The states of a toplevel's this protocol says, and a client can ask to change (155).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XfceState {
    /// On every workspace, which libxfce4windowing calls pinned.
    Sticky,
    Above,
    Below,
}

/// What a client is told of a toplevel through its Xfce handle.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct XfceToplevel {
    /// The id ext-workspace knows the toplevel's workspace by.
    pub workspace: Option<String>,
    pub sticky: bool,
    pub above: bool,
    pub below: bool,
}

impl XfceToplevel {
    /// The states as the `state` event's array.
    fn state_array(&self) -> Vec<u8> {
        use xfce_foreign_toplevel_handle_v1::State;

        [
            (self.sticky, State::Sticky),
            (self.above, State::Above),
            (self.below, State::Below),
        ]
        .into_iter()
        .filter(|(set, _)| *set)
        .flat_map(|(_, state)| (state as u32).to_ne_bytes())
        .collect()
    }
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
    /// The states it was last sent, as the `state` event's array: none until it's sent one.
    states: Vec<u8>,
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

    /// Tells every client of the toplevels that changed workspace or state, each ended with
    /// the foreign toplevel handle's `done`, and ends a new handle's first details with one.
    /// `describe` says what there is to tell of a toplevel. A state is sent only once
    /// there's one to say, so a toplevel with none is sent no `state`.
    ///
    /// A client is told only of a workspace it has a handle for in `workspaces`. So this is
    /// for the compositor to call both before ext-workspace's own refresh, while a client
    /// still has the handle of a workspace that's going, to leave it, and after it, when a
    /// client has the handle of one that's new, to enter it.
    pub fn refresh(
        &mut self,
        toplevels: &ForeignToplevelState,
        workspaces: &ExtWorkspaceState,
        describe: impl Fn(ToplevelId) -> XfceToplevel,
    ) {
        for handle in &mut self.handles {
            // A handle whose toplevel's was sent `closed` is inert, as that one is.
            let Some(toplevel) = toplevels.toplevel(&handle.toplevel) else {
                continue;
            };
            let details = describe(toplevel);
            let workspace = handle.resource.client().and_then(|client| {
                workspaces.workspace_handle(&client, details.workspace.as_ref()?)
            });
            let mut changed = !handle.announced;
            let states = details.state_array();
            if states != handle.states {
                handle.resource.state(states.clone());
                handle.states = states;
                changed = true;
            }
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
                    states: Vec::new(),
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
        state: &mut D,
        _client: &Client,
        resource: &XfceForeignToplevelHandleV1,
        request: xfce_foreign_toplevel_handle_v1::Request,
        _display: &DisplayHandle,
        data_init: &mut DataInit<'_, D>,
    ) {
        use xfce_foreign_toplevel_handle_v1::Request;
        use xfce_foreign_toplevel_icon_pixels_v1::FailureReason;

        match request {
            Request::SetSticky => ask_state(state, resource, XfceState::Sticky, true),
            Request::UnsetSticky => ask_state(state, resource, XfceState::Sticky, false),
            Request::SetAbove => ask_state(state, resource, XfceState::Above, true),
            Request::UnsetAbove => ask_state(state, resource, XfceState::Above, false),
            Request::SetBelow => ask_state(state, resource, XfceState::Below, true),
            Request::UnsetBelow => ask_state(state, resource, XfceState::Below, false),
            // Caminito says where a toplevel is and the states above, and no more (ADR 0013,
            // 155). What else a window list asks of a window, it asks through foreign
            // toplevel management.
            Request::SetShaded
            | Request::UnsetShaded
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

/// Hands the compositor a client's request to set or unset a state of the toplevel of its
/// Xfce handle `resource`.
fn ask_state<D: XfceForeignToplevelHandler>(
    state: &mut D,
    resource: &XfceForeignToplevelHandleV1,
    xfce_state: XfceState,
    set: bool,
) {
    let handles = &state.xfce_foreign_toplevel_state().handles;
    let toplevel = handles
        .iter()
        .find(|handle| handle.resource == *resource)
        .map(|handle| handle.toplevel.clone());
    if let Some(toplevel) = toplevel {
        state.set_toplevel_state(&toplevel, xfce_state, set);
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
