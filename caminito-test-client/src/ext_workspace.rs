//! An `ext_workspace_v1` client, as a panel's workspace switcher is one (66): it records the
//! workspace group and the workspaces the compositor shows and every event it sends for
//! them, and asks for a workspace to be activated.

use wayland_client::{Connection, Dispatch, QueueHandle, WEnum, event_created_child};
use wayland_protocols::ext::workspace::v1::client::ext_workspace_group_handle_v1::{
    self, ExtWorkspaceGroupHandleV1,
};
use wayland_protocols::ext::workspace::v1::client::ext_workspace_handle_v1::{
    self, ExtWorkspaceHandleV1,
};
use wayland_protocols::ext::workspace::v1::client::ext_workspace_manager_v1::{
    self, ExtWorkspaceManagerV1,
};

use crate::{Client, State};

/// What ext-workspace has told the client.
#[derive(Default)]
pub struct Workspaces {
    manager: Option<ExtWorkspaceManagerV1>,
    /// Every group the compositor has announced, in the order it did. A removed one stays,
    /// so a group's index names it for as long as the client lives.
    pub groups: Vec<Group>,
    /// Every workspace the compositor has announced, in the order it did. A removed one
    /// stays, so a workspace's index names it for as long as the client lives.
    pub workspaces: Vec<Workspace>,
    /// Every event since the client last took them, oldest first.
    events: Vec<String>,
    /// Whether a removed workspace's handle is kept, to make requests of, rather than
    /// destroyed as the protocol asks.
    keep_removed_handles: bool,
}

/// A group, as its last events left it.
pub struct Group {
    proxy: ExtWorkspaceGroupHandleV1,
    /// The names of the outputs it has entered and not left.
    pub outputs: Vec<String>,
    /// Whether the compositor said it's gone.
    pub removed: bool,
}

/// A workspace, as its last events left it.
pub struct Workspace {
    proxy: ExtWorkspaceHandleV1,
    /// Its `id`, once the compositor has sent it.
    pub id: Option<String>,
    /// The index of the group it has entered and not left.
    pub group: Option<usize>,
    pub name: Option<String>,
    /// Its last `coordinates`: every value the event had.
    pub coordinates: Vec<u32>,
    /// Whether its last `state` had `active`.
    pub active: bool,
    /// Whether the compositor said it's gone.
    pub removed: bool,
}

impl Client {
    /// Binds ext-workspace. Roundtrip before reading the workspaces.
    pub fn bind_workspaces(&mut self) {
        let manager = crate::bind(&self.registry, &self.state.globals, &self.qh, 1);
        self.state.workspaces.manager = Some(manager);
    }

    /// What ext-workspace has told the client.
    pub fn workspaces(&self) -> &Workspaces {
        &self.state.workspaces
    }

    /// The events since they were last taken, one per line: `group <index>: <event>`,
    /// `workspace <index>: <event>`, or `done`.
    pub fn take_workspace_events(&mut self) -> String {
        std::mem::take(&mut self.state.workspaces.events).join("\n")
    }

    /// Asks for a workspace to be activated. Nothing happens until
    /// [`commit_workspaces`](Self::commit_workspaces).
    pub fn activate_workspace(&self, index: usize) {
        self.workspace_proxy(index).activate();
    }

    /// Asks for a workspace to be removed, which a read-only compositor ignores.
    pub fn remove_workspace(&self, index: usize) {
        self.workspace_proxy(index).remove();
    }

    /// Asks for a workspace to be deactivated, which a read-only compositor ignores.
    pub fn deactivate_workspace(&self, index: usize) {
        self.workspace_proxy(index).deactivate();
    }

    /// Asks for a new workspace in the first group, which a read-only compositor ignores.
    pub fn create_workspace(&self, name: &str) {
        self.state.workspaces.groups[0]
            .proxy
            .create_workspace(name.to_owned());
    }

    /// Asks for no more events. The compositor answers `finished`.
    pub fn stop_workspaces(&self) {
        let manager = self.state.workspaces.manager.as_ref();
        manager.expect("bind_workspaces first").stop();
    }

    /// Whether the compositor has sent `finished`, or ext-workspace was never bound.
    pub fn workspaces_finished(&self) -> bool {
        self.state.workspaces.manager.is_none()
    }

    /// Destroys the client's handle for a workspace that's still there, which the protocol
    /// allows: the client is told no more of it.
    pub fn destroy_workspace_handle(&self, index: usize) {
        self.workspace_proxy(index).destroy();
    }

    /// Destroys the client's handle for a group that's still there, which the protocol
    /// allows: the client is told no more of it.
    pub fn destroy_workspace_group_handle(&self, index: usize) {
        self.state.workspaces.groups[index].proxy.destroy();
    }

    /// Keeps the handles of workspaces removed from now on, so the client can make requests
    /// of them, which a compositor ignores.
    pub fn keep_removed_workspace_handles(&mut self) {
        self.state.workspaces.keep_removed_handles = true;
    }

    /// Ends a series of requests, which take effect now.
    pub fn commit_workspaces(&self) {
        let manager = self.state.workspaces.manager.as_ref();
        manager.expect("bind_workspaces first").commit();
    }

    fn workspace_proxy(&self, index: usize) -> &ExtWorkspaceHandleV1 {
        &self.state.workspaces.workspaces[index].proxy
    }
}

impl Workspace {
    pub(crate) fn proxy(&self) -> &ExtWorkspaceHandleV1 {
        &self.proxy
    }
}

impl State {
    pub(crate) fn workspace_index(&self, proxy: &ExtWorkspaceHandleV1) -> Option<usize> {
        let workspaces = &self.workspaces.workspaces;
        workspaces.iter().position(|w| w.proxy == *proxy)
    }
}

impl Dispatch<ExtWorkspaceManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ExtWorkspaceManagerV1,
        event: ext_workspace_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use ext_workspace_manager_v1::Event;

        let workspaces = &mut state.workspaces;
        match event {
            Event::WorkspaceGroup { workspace_group } => {
                let index = workspaces.groups.len();
                workspaces.events.push(format!("group {index}: new"));
                workspaces.groups.push(Group {
                    proxy: workspace_group,
                    outputs: Vec::new(),
                    removed: false,
                });
            }
            Event::Workspace { workspace } => {
                let index = workspaces.workspaces.len();
                workspaces.events.push(format!("workspace {index}: new"));
                workspaces.workspaces.push(Workspace {
                    proxy: workspace,
                    id: None,
                    group: None,
                    name: None,
                    coordinates: Vec::new(),
                    active: false,
                    removed: false,
                });
            }
            Event::Done => workspaces.events.push("done".to_owned()),
            Event::Finished => workspaces.manager = None,
            _ => {}
        }
    }

    event_created_child!(State, ExtWorkspaceManagerV1, [
        ext_workspace_manager_v1::EVT_WORKSPACE_GROUP_OPCODE => (ExtWorkspaceGroupHandleV1, ()),
        ext_workspace_manager_v1::EVT_WORKSPACE_OPCODE => (ExtWorkspaceHandleV1, ()),
    ]);
}

impl Dispatch<ExtWorkspaceGroupHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ExtWorkspaceGroupHandleV1,
        event: ext_workspace_group_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use ext_workspace_group_handle_v1::Event;

        let groups = &state.workspaces.groups;
        let Some(index) = groups.iter().position(|group| group.proxy == *proxy) else {
            return;
        };
        let workspace = |state: &Self, workspace: &ExtWorkspaceHandleV1| match state
            .workspace_index(workspace)
        {
            Some(index) => format!("workspace {index}"),
            None => "an unknown workspace".to_owned(),
        };
        let text = match event {
            Event::Capabilities {
                capabilities: WEnum::Value(capabilities),
            } => format!("capabilities {capabilities:?}"),
            Event::OutputEnter { output } => {
                let name = state.output_name(&output);
                state.workspaces.groups[index].outputs.push(name.clone());
                format!("output_enter {name}")
            }
            Event::OutputLeave { output } => {
                let name = state.output_name(&output);
                state.workspaces.groups[index]
                    .outputs
                    .retain(|entered| *entered != name);
                // libxfce4windowing can only take a monitor out of the group while it still
                // has it, which is until its global is removed (91).
                let late = if state.output_is_advertised(&output) {
                    ""
                } else {
                    ", after its global was removed"
                };
                format!("output_leave {}{late}", state.output_name(&output))
            }
            Event::WorkspaceEnter { workspace: entered } => {
                if let Some(entered) = state.workspace_index(&entered) {
                    state.workspaces.workspaces[entered].group = Some(index);
                }
                format!("workspace_enter {}", workspace(state, &entered))
            }
            Event::WorkspaceLeave { workspace: left } => {
                if let Some(left) = state.workspace_index(&left) {
                    state.workspaces.workspaces[left].group = None;
                }
                format!("workspace_leave {}", workspace(state, &left))
            }
            Event::Removed => {
                let group = &mut state.workspaces.groups[index];
                group.removed = true;
                group.proxy.destroy();
                "removed".to_owned()
            }
            _ => return,
        };
        let events = &mut state.workspaces.events;
        events.push(format!("group {index}: {text}"));
    }
}

impl Dispatch<ExtWorkspaceHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ExtWorkspaceHandleV1,
        event: ext_workspace_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use ext_workspace_handle_v1::Event;

        let Some(index) = state.workspace_index(proxy) else {
            return;
        };
        let keep_removed = state.workspaces.keep_removed_handles;
        let workspace = &mut state.workspaces.workspaces[index];
        let text = match event {
            Event::Id { id } => {
                let text = format!("id {id}");
                workspace.id = Some(id);
                text
            }
            Event::Name { name } => {
                let text = format!("name {name}");
                workspace.name = Some(name);
                text
            }
            Event::Coordinates { coordinates } => {
                let (values, _) = coordinates.as_chunks::<4>();
                let values: Vec<u32> = values.iter().map(|b| u32::from_ne_bytes(*b)).collect();
                let text = format!("coordinates {values:?}");
                workspace.coordinates = values;
                text
            }
            Event::State {
                state: WEnum::Value(states),
            } => {
                workspace.active = states.contains(ext_workspace_handle_v1::State::Active);
                format!("state {states:?}")
            }
            Event::Capabilities {
                capabilities: WEnum::Value(capabilities),
            } => format!("capabilities {capabilities:?}"),
            Event::Removed => {
                workspace.removed = true;
                if !keep_removed {
                    workspace.proxy.destroy();
                }
                "removed".to_owned()
            }
            _ => return,
        };
        let events = &mut state.workspaces.events;
        events.push(format!("workspace {index}: {text}"));
    }
}
