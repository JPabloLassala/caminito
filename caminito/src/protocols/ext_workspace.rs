//! `ext_workspace_v1`, read-only (06, 66): clients such as mate-panel's workspace switcher,
//! through libxfce4windowing, see the workspaces with their names and places and which is
//! active, and ask for one to be activated.
//!
//! This module owns the protocol's objects and its rules: the group and workspace handles
//! each client has, what each was last sent, and the requests a client has made since its
//! last `commit`. It knows nothing of policy. The compositor gives it the groups there are,
//! each with its outputs and workspaces, in its refresh pass, through
//! [`ExtWorkspaceState::refresh`], and answers a committed `activate` through
//! [`ExtWorkspaceHandler`]. The groups are one that holds every output, or one for each
//! output (ADR 0032, 226).
//!
//! Smithay has no ext-workspace at the pinned revision. Written from the protocol's XML;
//! nothing was copied from niri's or cosmic-comp's implementations (03). cosmic-comp's code
//! can't be used, as it is GPL-3.0-only (ADR 0051).

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

/// The version of the protocol Caminito offers.
const VERSION: u32 = 1;

/// A group of workspaces as ext-workspace shows it, with the outputs that show them.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    /// What names the group from one refresh to the next. No two groups share one.
    pub id: String,
    /// The outputs that show the group's workspaces.
    pub outputs: Vec<Output>,
    /// The group's workspaces, in their order.
    pub workspaces: Vec<Workspace>,
}

/// A workspace as ext-workspace shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    /// What names the workspace from one session to the next, and to the handler. No two
    /// workspaces share one, in any group, and it never changes while the workspace is
    /// there.
    pub id: String,
    /// What the user calls it.
    pub name: String,
    /// Where it is in the group's grid: its column, then its row.
    pub coordinates: [u32; 2],
    /// Whether it's the one its group's outputs show.
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

    /// Makes the workspace with this id the active one of its group. It may be gone since
    /// the last refresh, which the compositor ignores.
    fn activate_workspace(&mut self, id: &str);
}

/// The manager's global, the groups last sent, and every client's objects for them.
#[derive(Debug)]
pub struct ExtWorkspaceState {
    _global: GlobalId,
    groups: Vec<Group>,
    managers: Vec<Manager>,
}

/// One client's manager, its group handles, and its workspace handles.
#[derive(Debug)]
struct Manager {
    resource: ExtWorkspaceManagerV1,
    client: Client,
    /// The groups the client was sent that weren't sent `removed`.
    groups: Vec<GroupHandle>,
    handles: Vec<Handle>,
    /// The workspaces the client asked to activate since its last `commit`, oldest first.
    pending: Vec<Activation>,
}

/// A client's handle for a group.
#[derive(Debug)]
struct GroupHandle {
    /// The group's id.
    id: String,
    /// `None` once the client has destroyed it.
    resource: Option<ExtWorkspaceGroupHandleV1>,
    /// The client's `wl_output`s the group has entered.
    outputs: Vec<WlOutput>,
}

/// A client's handle for a workspace.
#[derive(Debug)]
struct Handle {
    id: String,
    /// The id of the workspace's group.
    group: String,
    resource: ExtWorkspaceHandleV1,
}

/// A client's request to activate a workspace, not yet committed.
#[derive(Debug)]
struct Activation {
    /// The workspace's id.
    id: String,
    /// The id of the workspace's group.
    group: String,
}

/// The manager global's data: the clients it's offered to.
pub struct ManagerGlobalData {
    filter: GlobalFilter,
}

/// A manager's data.
#[derive(Debug)]
pub struct ManagerData;

/// A group handle's data.
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
            groups: Vec::new(),
            managers: Vec::new(),
        }
    }

    /// Tells every client what changed since the groups were last sent: the groups that
    /// are gone and those that are new, the outputs that entered and left each, the
    /// workspaces that are new and those that are gone, and for the rest only what differs,
    /// then one `done` if anything was sent.
    pub fn refresh<D>(&mut self, display: &DisplayHandle, groups: Vec<Group>)
    where
        D: Dispatch<ExtWorkspaceGroupHandleV1, GroupData>
            + Dispatch<ExtWorkspaceHandleV1, HandleData>
            + 'static,
    {
        for manager in &mut self.managers {
            if manager.refresh::<D>(display, &self.groups, &groups) {
                manager.resource.done();
            }
        }
        self.groups = groups;
    }

    /// A client's handle for the workspace with this id, if it has one that wasn't sent
    /// `removed`.
    pub fn workspace_handle(&self, client: &Client, id: &str) -> Option<ExtWorkspaceHandleV1> {
        let managers = self.managers.iter().filter(|m| m.client == *client);
        let mut handles = managers.flat_map(|manager| &manager.handles);
        let handle = handles.find(|handle| handle.id == id)?;
        Some(handle.resource.clone())
    }
}

impl Manager {
    /// Sends what differs between `old`, which the client has, and `new`. Returns whether
    /// anything was sent.
    fn refresh<D>(&mut self, display: &DisplayHandle, old: &[Group], new: &[Group]) -> bool
    where
        D: Dispatch<ExtWorkspaceGroupHandleV1, GroupData>
            + Dispatch<ExtWorkspaceHandleV1, HandleData>
            + 'static,
    {
        let mut changed = false;
        let is_there = |group: &str, id: &str| {
            let mut groups = new.iter().filter(|g| g.id == group);
            groups.any(|g| g.workspaces.iter().any(|workspace| workspace.id == id))
        };
        let groups = &self.groups;
        self.handles.retain(|handle| {
            let kept = is_there(&handle.group, &handle.id);
            if !kept {
                // A workspace leaves its group before it's removed.
                let group = groups.iter().find(|group| group.id == handle.group);
                if let Some(group) = group.and_then(|group| group.resource.as_ref()) {
                    group.workspace_leave(&handle.resource);
                }
                handle.resource.removed();
                changed = true;
            }
            kept
        });
        // A request for a workspace that's gone is for that one, not the next with its id.
        self.pending
            .retain(|activation| is_there(&activation.group, &activation.id));
        // A group goes once its workspaces have left it, as the protocol asks.
        self.groups.retain(|group| {
            let kept = new.iter().any(|g| g.id == group.id);
            if !kept && let Some(resource) = &group.resource {
                // A `wl_output` the client released needs no leave.
                for left in group.outputs.iter().filter(|output| output.is_alive()) {
                    resource.output_leave(left);
                }
                resource.removed();
                changed = true;
            }
            kept
        });
        for group in new {
            if !self.groups.iter().any(|g| g.id == group.id) {
                self.add_group::<D>(display, &group.id);
                changed = true;
            }
            changed |= self.send_outputs(&group.id, &group.outputs);
            let old = old.iter().find(|sent| sent.id == group.id);
            for workspace in &group.workspaces {
                let sent = old.and_then(|old| old.workspaces.iter().find(|s| s.id == workspace.id));
                let Some(sent) = sent else {
                    self.add_workspace::<D>(display, &group.id, workspace);
                    changed = true;
                    continue;
                };
                // A workspace the client was sent and has no handle for is one it
                // destroyed.
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
        }
        changed
    }

    /// Announces a new group, which has no output and no workspace yet.
    fn add_group<D>(&mut self, display: &DisplayHandle, id: &str)
    where
        D: Dispatch<ExtWorkspaceGroupHandleV1, GroupData> + 'static,
    {
        let version = self.resource.version();
        let resource = self
            .client
            .create_resource::<ExtWorkspaceGroupHandleV1, _, D>(display, version, GroupData)
            .ok();
        if let Some(resource) = &resource {
            self.resource.workspace_group(resource);
            // Read-only (06): a client can't create a workspace.
            resource.capabilities(GroupCapabilities::empty());
        }
        self.groups.push(GroupHandle {
            id: id.to_owned(),
            resource,
            outputs: Vec::new(),
        });
    }

    /// Sends the group with this id the outputs it entered and left. A client may bind a
    /// `wl_output` after it has the group, so what it was sent is compared by `wl_output`,
    /// not by output. Returns whether anything was sent.
    fn send_outputs(&mut self, group: &str, outputs: &[Output]) -> bool {
        let Some(group) = self.groups.iter_mut().find(|g| g.id == group) else {
            return false;
        };
        let Some(resource) = &group.resource else {
            return false;
        };
        let outputs: Vec<WlOutput> = outputs
            .iter()
            .flat_map(|output| output.client_outputs(&self.client))
            .collect();
        let mut changed = false;
        for left in group.outputs.iter().filter(|o| !outputs.contains(o)) {
            // A `wl_output` the client released needs no leave.
            if left.is_alive() {
                resource.output_leave(left);
                changed = true;
            }
        }
        for entered in outputs.iter().filter(|o| !group.outputs.contains(o)) {
            resource.output_enter(entered);
            changed = true;
        }
        group.outputs = outputs;
        changed
    }

    /// Announces a new workspace, with its details, and puts it in the group with the id
    /// `group`.
    fn add_workspace<D>(&mut self, display: &DisplayHandle, group: &str, workspace: &Workspace)
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
        let entered = self.groups.iter().find(|g| g.id == group);
        if let Some(entered) = entered.and_then(|g| g.resource.as_ref()) {
            entered.workspace_enter(&handle);
        }
        self.handles.push(Handle {
            id: workspace.id.clone(),
            group: group.to_owned(),
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
        let mut manager = Manager {
            resource: data_init.init(resource, ManagerData),
            client: client.clone(),
            groups: Vec::new(),
            handles: Vec::new(),
            pending: Vec::new(),
        };
        let state = state.ext_workspace_state();
        manager.refresh::<D>(display, &[], &state.groups);
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
                // One workspace is active in a group, so of a series of activations in one
                // group the last counts.
                let mut last: Vec<Activation> = Vec::new();
                for activation in pending.unwrap_or_default() {
                    last.retain(|earlier| earlier.group != activation.group);
                    last.push(activation);
                }
                for activation in last {
                    state.activate_workspace(&activation.id);
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
        let managers = &mut state.ext_workspace_state().managers;
        for group in managers.iter_mut().flat_map(|m| &mut m.groups) {
            if group.resource.as_ref() == Some(resource) {
                group.resource = None;
                group.outputs.clear();
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
                for manager in managers {
                    let mut handles = manager.handles.iter();
                    if let Some(handle) = handles.find(|handle| handle.resource == *resource) {
                        manager.pending.push(Activation {
                            id: self.0.clone(),
                            group: handle.group.clone(),
                        });
                    }
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
