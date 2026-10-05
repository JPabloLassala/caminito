//! `ext_workspace_v1`, read-only (06, 66): clients such as mate-panel's workspace switcher,
//! through libxfce4windowing, see the workspaces with their names and places and which is
//! active, and ask for one to be activated.
//!
//! This module owns the protocol's objects and its rules: the one group and the workspace
//! handles each client has, what each was last sent, and the requests a client has made
//! since its last `commit`. It knows nothing of policy. The compositor gives it the outputs
//! and the workspaces there are in its refresh pass, through [`ExtWorkspaceState::refresh`],
//! and answers a committed `activate` through [`ExtWorkspaceHandler`].
//!
//! Smithay has no ext-workspace at the pinned revision. Written from the protocol's XML;
//! niri's and cosmic-comp's implementations are GPL-3, so they weren't used (03).

use smithay::output::Output;
use smithay::reexports::wayland_protocols::ext::workspace::v1::server::ext_workspace_group_handle_v1::{
    self, ExtWorkspaceGroupHandleV1, GroupCapabilities,
};
use smithay::reexports::wayland_protocols::ext::workspace::v1::server::ext_workspace_handle_v1::{
    self, ExtWorkspaceHandleV1, WorkspaceCapabilities,
};
use smithay::reexports::wayland_protocols::ext::workspace::v1::server::ext_workspace_manager_v1::{
    self, ExtWorkspaceManagerV1,
};
use smithay::reexports::wayland_server::backend::{ClientId, GlobalId};
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
};
use smithay::wayland::{Dispatch2, GlobalDispatch2};

use super::GlobalFilter;

/// The version of the protocol Camino offers.
const VERSION: u32 = 1;

/// A workspace as ext-workspace shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    /// What names the workspace from one session to the next, and to the handler. No two
    /// workspaces share one, and it never changes while the workspace is there.
    pub id: String,
    /// What the user calls it.
    pub name: String,
    /// Where it is in the group's grid: its column, then its row.
    pub coordinates: [u32; 2],
    /// Whether it's the one shown.
    pub active: bool,
}

impl Workspace {
    fn state(&self) -> ext_workspace_handle_v1::State {
        if self.active {
            ext_workspace_handle_v1::State::Active
        } else {
            ext_workspace_handle_v1::State::empty()
        }
    }

    /// The coordinates as the event's array. Always both, since libxfce4windowing reads the
    /// second without a proper bounds check (06).
    fn coordinates_array(&self) -> Vec<u8> {
        let values = self.coordinates.iter();
        values.flat_map(|value| value.to_ne_bytes()).collect()
    }
}

/// What the compositor does with clients' requests.
pub trait ExtWorkspaceHandler: 'static {
    /// The compositor's ext-workspace state.
    fn ext_workspace_state(&mut self) -> &mut ExtWorkspaceState;

    /// Makes the workspace with this id the active one. It may be gone since the last
    /// refresh, which the compositor ignores.
    fn activate_workspace(&mut self, id: &str);
}

/// The manager's global, the outputs and workspaces last sent, and every client's objects
/// for them.
#[derive(Debug)]
pub struct ExtWorkspaceState {
    _global: GlobalId,
    outputs: Vec<Output>,
    workspaces: Vec<Workspace>,
    managers: Vec<Manager>,
}

/// One client's manager, its handle for the one group, and its workspace handles.
#[derive(Debug)]
struct Manager {
    resource: ExtWorkspaceManagerV1,
    client: Client,
    /// `None` once the client has destroyed it.
    group: Option<ExtWorkspaceGroupHandleV1>,
    /// The client's `wl_output`s the group has entered.
    outputs: Vec<WlOutput>,
    handles: Vec<Handle>,
    /// The ids of the workspaces the client asked to activate since its last `commit`,
    /// oldest first.
    pending: Vec<String>,
}

#[derive(Debug)]
struct Handle {
    id: String,
    resource: ExtWorkspaceHandleV1,
}

/// The manager global's data: the clients it's offered to.
pub struct ManagerGlobalData {
    filter: GlobalFilter,
}

/// A manager's data.
#[derive(Debug)]
pub struct ManagerData;

/// The group handle's data.
#[derive(Debug)]
pub struct GroupData;

/// A workspace handle's data: the workspace's id.
#[derive(Debug)]
pub struct HandleData(String);

impl ExtWorkspaceState {
    /// Offers the manager's global to the clients `filter` allows.
    pub fn new<D, F>(display: &DisplayHandle, filter: F) -> Self
    where
        D: GlobalDispatch<ExtWorkspaceManagerV1, ManagerGlobalData> + 'static,
        F: for<'c> Fn(&'c Client) -> bool + Send + Sync + 'static,
    {
        let data = ManagerGlobalData {
            filter: Box::new(filter),
        };
        let global = display.create_global::<D, ExtWorkspaceManagerV1, _>(VERSION, data);
        Self {
            _global: global,
            outputs: Vec::new(),
            workspaces: Vec::new(),
            managers: Vec::new(),
        }
    }

    /// Tells every client what changed since the outputs and workspaces were last sent: the
    /// outputs that entered and left the group, the workspaces that are new and those that
    /// are gone, and for the rest only what differs, then one `done` if anything was sent.
    pub fn refresh<D>(
        &mut self,
        display: &DisplayHandle,
        outputs: Vec<Output>,
        workspaces: Vec<Workspace>,
    ) where
        D: Dispatch<ExtWorkspaceHandleV1, HandleData> + 'static,
    {
        for manager in &mut self.managers {
            if manager.refresh::<D>(display, &outputs, &self.workspaces, &workspaces) {
                manager.resource.done();
            }
        }
        self.outputs = outputs;
        self.workspaces = workspaces;
    }
}

impl Manager {
    /// Sends what differs between `old`, which the client has, and `new`, and the outputs
    /// that entered and left the group. Returns whether anything was sent.
    fn refresh<D>(
        &mut self,
        display: &DisplayHandle,
        outputs: &[Output],
        old: &[Workspace],
        new: &[Workspace],
    ) -> bool
    where
        D: Dispatch<ExtWorkspaceHandleV1, HandleData> + 'static,
    {
        let mut changed = self.send_outputs(outputs);
        let group = self.group.as_ref();
        self.handles.retain(|handle| {
            let kept = new.iter().any(|workspace| workspace.id == handle.id);
            if !kept {
                // A workspace leaves its group before it's removed.
                if let Some(group) = group {
                    group.workspace_leave(&handle.resource);
                }
                handle.resource.removed();
                changed = true;
            }
            kept
        });
        // A request for a workspace that's gone is for that one, not the next with its id.
        self.pending
            .retain(|id| new.iter().any(|workspace| workspace.id == *id));
        for workspace in new {
            let Some(sent) = old.iter().find(|sent| sent.id == workspace.id) else {
                self.add_workspace::<D>(display, workspace);
                changed = true;
                continue;
            };
            // A workspace the client was sent and has no handle for is one it destroyed.
            let Some(handle) = self.handles.iter().find(|h| h.id == workspace.id) else {
                continue;
            };
            let handle = &handle.resource;
            if sent.name != workspace.name {
                handle.name(workspace.name.clone());
                changed = true;
            }
            if sent.coordinates != workspace.coordinates {
                handle.coordinates(workspace.coordinates_array());
                changed = true;
            }
            if sent.active != workspace.active {
                handle.state(workspace.state());
                changed = true;
            }
        }
        changed
    }

    /// Sends the group the outputs it entered and left. A client may bind a `wl_output`
    /// after it has the group, so what it was sent is compared by `wl_output`, not by
    /// output. Returns whether anything was sent.
    fn send_outputs(&mut self, outputs: &[Output]) -> bool {
        let Some(group) = &self.group else {
            return false;
        };
        let outputs: Vec<WlOutput> = outputs
            .iter()
            .flat_map(|output| output.client_outputs(&self.client))
            .collect();
        let mut changed = false;
        for left in self.outputs.iter().filter(|o| !outputs.contains(o)) {
            // A `wl_output` the client released needs no leave.
            if left.is_alive() {
                group.output_leave(left);
                changed = true;
            }
        }
        for entered in outputs.iter().filter(|o| !self.outputs.contains(o)) {
            group.output_enter(entered);
            changed = true;
        }
        self.outputs = outputs;
        changed
    }

    /// Announces a new workspace, with its details, and puts it in the group.
    fn add_workspace<D>(&mut self, display: &DisplayHandle, workspace: &Workspace)
    where
        D: Dispatch<ExtWorkspaceHandleV1, HandleData> + 'static,
    {
        let version = self.resource.version();
        let data = HandleData(workspace.id.clone());
        let Ok(handle) = self
            .client
            .create_resource::<ExtWorkspaceHandleV1, _, D>(display, version, data)
        else {
            return;
        };
        self.resource.workspace(&handle);
        handle.id(workspace.id.clone());
        handle.name(workspace.name.clone());
        handle.coordinates(workspace.coordinates_array());
        handle.state(workspace.state());
        // Read-only (06): a client can't deactivate, remove or assign one.
        handle.capabilities(WorkspaceCapabilities::Activate);
        if let Some(group) = &self.group {
            group.workspace_enter(&handle);
        }
        self.handles.push(Handle {
            id: workspace.id.clone(),
            resource: handle,
        });
    }
}

impl<D> GlobalDispatch2<ExtWorkspaceManagerV1, D> for ManagerGlobalData
where
    D: ExtWorkspaceHandler
        + Dispatch<ExtWorkspaceManagerV1, ManagerData>
        + Dispatch<ExtWorkspaceGroupHandleV1, GroupData>
        + Dispatch<ExtWorkspaceHandleV1, HandleData>,
{
    fn bind(
        &self,
        state: &mut D,
        display: &DisplayHandle,
        client: &Client,
        resource: New<ExtWorkspaceManagerV1>,
        data_init: &mut DataInit<'_, D>,
    ) {
        let resource = data_init.init(resource, ManagerData);
        let version = resource.version();
        // The one group, which holds every output: Marco's workspaces span all monitors.
        let group = client
            .create_resource::<ExtWorkspaceGroupHandleV1, _, D>(display, version, GroupData)
            .ok();
        if let Some(group) = &group {
            resource.workspace_group(group);
            // Read-only (06): a client can't create a workspace.
            group.capabilities(GroupCapabilities::empty());
        }
        let mut manager = Manager {
            resource,
            client: client.clone(),
            group,
            outputs: Vec::new(),
            handles: Vec::new(),
            pending: Vec::new(),
        };
        let state = state.ext_workspace_state();
        manager.refresh::<D>(display, &state.outputs, &[], &state.workspaces);
        manager.resource.done();
        state.managers.push(manager);
    }

    fn can_view(&self, client: &Client) -> bool {
        (self.filter)(client)
    }
}

impl<D: ExtWorkspaceHandler> Dispatch2<ExtWorkspaceManagerV1, D> for ManagerData {
    fn request(
        &self,
        state: &mut D,
        _client: &Client,
        resource: &ExtWorkspaceManagerV1,
        request: ext_workspace_manager_v1::Request,
        _display: &DisplayHandle,
        _data_init: &mut DataInit<'_, D>,
    ) {
        let managers = &mut state.ext_workspace_state().managers;
        match request {
            ext_workspace_manager_v1::Request::Commit => {
                let manager = managers.iter_mut().find(|m| m.resource == *resource);
                let pending = manager.map(|manager| std::mem::take(&mut manager.pending));
                // One workspace is active, so of a series of activations the last counts.
                if let Some(id) = pending.as_ref().and_then(|pending| pending.last()) {
                    state.activate_workspace(id);
                }
            }
            ext_workspace_manager_v1::Request::Stop => {
                // The client gets nothing more: every later event would need the
                // manager's `done`, and `finished` destroys the manager.
                managers.retain(|manager| manager.resource != *resource);
                resource.finished();
            }
            _ => unreachable!("ext_workspace_manager_v1 has no other request"),
        }
    }

    fn destroyed(&self, state: &mut D, _client: ClientId, resource: &ExtWorkspaceManagerV1) {
        let managers = &mut state.ext_workspace_state().managers;
        managers.retain(|manager| manager.resource != *resource);
    }
}

impl<D: ExtWorkspaceHandler> Dispatch2<ExtWorkspaceGroupHandleV1, D> for GroupData {
    fn request(
        &self,
        _state: &mut D,
        _client: &Client,
        _resource: &ExtWorkspaceGroupHandleV1,
        request: ext_workspace_group_handle_v1::Request,
        _display: &DisplayHandle,
        _data_init: &mut DataInit<'_, D>,
    ) {
        match request {
            // The group hasn't the capability, so the request is ignored, as the protocol
            // says.
            ext_workspace_group_handle_v1::Request::CreateWorkspace { .. } => {}
            // The handle is forgotten in `destroyed`.
            ext_workspace_group_handle_v1::Request::Destroy => {}
            _ => unreachable!("ext_workspace_group_handle_v1 has no other request"),
        }
    }

    fn destroyed(&self, state: &mut D, _client: ClientId, resource: &ExtWorkspaceGroupHandleV1) {
        for manager in &mut state.ext_workspace_state().managers {
            if manager.group.as_ref() == Some(resource) {
                manager.group = None;
                manager.outputs.clear();
            }
        }
    }
}

impl<D: ExtWorkspaceHandler> Dispatch2<ExtWorkspaceHandleV1, D> for HandleData {
    fn request(
        &self,
        state: &mut D,
        _client: &Client,
        resource: &ExtWorkspaceHandleV1,
        request: ext_workspace_handle_v1::Request,
        _display: &DisplayHandle,
        _data_init: &mut DataInit<'_, D>,
    ) {
        use ext_workspace_handle_v1::Request;

        match request {
            Request::Activate => {
                // A handle that was sent `removed` is no longer among its manager's, so
                // it's inert, though a newer workspace may have its id.
                let managers = &mut state.ext_workspace_state().managers;
                let manager = managers
                    .iter_mut()
                    .find(|m| m.handles.iter().any(|handle| handle.resource == *resource));
                if let Some(manager) = manager {
                    manager.pending.push(self.0.clone());
                }
            }
            // The workspace hasn't these capabilities, so the requests are ignored, as the
            // protocol says.
            Request::Deactivate | Request::Assign { .. } | Request::Remove => {}
            // The handle is forgotten in `destroyed`.
            Request::Destroy => {}
            _ => unreachable!("ext_workspace_handle_v1 has no other request"),
        }
    }

    fn destroyed(&self, state: &mut D, _client: ClientId, resource: &ExtWorkspaceHandleV1) {
        for manager in &mut state.ext_workspace_state().managers {
            manager
                .handles
                .retain(|handle| handle.resource != *resource);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinates_are_always_a_column_and_a_row() {
        let workspace = Workspace {
            id: "3".into(),
            name: "Workspace 3".into(),
            coordinates: [2, 0],
            active: false,
        };
        let array = workspace.coordinates_array();
        let (values, rest) = array.as_chunks::<4>();
        assert!(rest.is_empty());
        let values: Vec<u32> = values.iter().map(|b| u32::from_ne_bytes(*b)).collect();
        assert_eq!(values, [2, 0]);
    }
}
