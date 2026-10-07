//! `zwlr_foreign_toplevel_management_unstable_v1` version 3 (06, 65): clients such as
//! mate-panel's window list, through libxfce4windowing, see the windows as toplevels with
//! their titles, apps and states, and ask for them to be activated, closed, minimised,
//! maximised and made fullscreen.
//!
//! This module owns the protocol's objects: which handle each client has for each toplevel,
//! and what each was last sent. It knows nothing of windows. The compositor gives it the
//! toplevels there are in its refresh pass, through [`ForeignToplevelState::refresh`], and
//! answers requests through [`ForeignToplevelHandler`].
//!
//! Written from the protocol's XML. niri's and cosmic-comp's implementations are GPL-3, so
//! they weren't used (03).

use smithay::output::Output;
use smithay::reexports::wayland_protocols_wlr::foreign_toplevel::v1::server::zwlr_foreign_toplevel_handle_v1::{
    self, ZwlrForeignToplevelHandleV1,
};
use smithay::reexports::wayland_protocols_wlr::foreign_toplevel::v1::server::zwlr_foreign_toplevel_manager_v1::{
    self, ZwlrForeignToplevelManagerV1,
};
use smithay::reexports::wayland_server::backend::{ClientId, GlobalId};
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
};
use smithay::utils::{Logical, Rectangle};
use smithay::wayland::{Dispatch2, GlobalDispatch2};

use super::GlobalFilter;

/// The version of the protocol Caminito offers.
const VERSION: u32 = 3;
/// The fullscreen state, and its two requests, are since version 2.
const SINCE_FULLSCREEN: u32 = 2;
/// `zwlr_foreign_toplevel_handle_v1.parent` is since version 3.
const SINCE_PARENT: u32 = 3;

/// What the compositor names a toplevel by, from one refresh to the next. A toplevel that's
/// gone never gives its id to another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ToplevelId(pub u64);

/// A window as foreign toplevel management shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Toplevel {
    /// What requests name it by.
    pub id: ToplevelId,
    pub title: String,
    /// What a client finds its `.desktop` file and icon by, or empty.
    pub app_id: String,
    pub states: ToplevelStates,
    /// The outputs it's on.
    pub outputs: Vec<Output>,
    /// The toplevel it's transient for, if that one is shown too.
    pub parent: Option<ToplevelId>,
}

/// The states the protocol has.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ToplevelStates {
    pub maximized: bool,
    pub minimized: bool,
    /// Whether it has the focus.
    pub activated: bool,
    /// Sent to handles of version 2 and later.
    pub fullscreen: bool,
}

impl ToplevelStates {
    /// The states as the `state` event's array, for a handle of `version`.
    fn to_array(self, version: u32) -> Vec<u8> {
        use zwlr_foreign_toplevel_handle_v1::State;

        [
            (self.maximized, State::Maximized),
            (self.minimized, State::Minimized),
            (self.activated, State::Activated),
            (
                self.fullscreen && version >= SINCE_FULLSCREEN,
                State::Fullscreen,
            ),
        ]
        .into_iter()
        .filter(|(set, _)| *set)
        .flat_map(|(_, state)| (state as u32).to_ne_bytes())
        .collect()
    }
}

/// What the compositor does with clients' requests. A request for a toplevel the client was
/// told is closed never gets here, but one may name a toplevel that's gone since the last
/// refresh, which the compositor ignores.
pub trait ForeignToplevelHandler: 'static {
    /// The compositor's foreign toplevel state.
    fn foreign_toplevel_state(&mut self) -> &mut ForeignToplevelState;

    /// Brings the toplevel to the user.
    fn activate_toplevel(&mut self, toplevel: ToplevelId);

    /// Asks the toplevel's client to close it.
    fn close_toplevel(&mut self, toplevel: ToplevelId);

    /// Maximises the toplevel, or takes it back to the size it had.
    fn set_toplevel_maximized(&mut self, toplevel: ToplevelId, maximized: bool);

    /// Minimises the toplevel, or shows it again.
    fn set_toplevel_minimized(&mut self, toplevel: ToplevelId, minimized: bool);

    /// Makes the toplevel fullscreen, on `output` if the client named one.
    fn fullscreen_toplevel(&mut self, toplevel: ToplevelId, output: Option<Output>);

    /// Takes the toplevel out of fullscreen.
    fn unfullscreen_toplevel(&mut self, toplevel: ToplevelId);

    /// Where the client shows the toplevel, as a rectangle on one of its surfaces, or
    /// `None` if it no longer shows it anywhere.
    fn set_toplevel_rectangle(
        &mut self,
        toplevel: ToplevelId,
        rectangle: Option<(WlSurface, Rectangle<i32, Logical>)>,
    );
}

/// The manager's global, the toplevels last sent, and every client's handles for them.
#[derive(Debug)]
pub struct ForeignToplevelState {
    _global: GlobalId,
    toplevels: Vec<Toplevel>,
    managers: Vec<Manager>,
}

/// One client's manager, and the handles it was sent.
#[derive(Debug)]
struct Manager {
    resource: ZwlrForeignToplevelManagerV1,
    client: Client,
    /// Whether the client asked for no more toplevels, which destroys the manager. The
    /// handles it has stay up to date, so the manager is kept here until they're gone.
    stopped: bool,
    handles: Vec<Handle>,
}

#[derive(Debug)]
struct Handle {
    id: ToplevelId,
    resource: ZwlrForeignToplevelHandleV1,
    /// The client's `wl_output`s the handle has entered.
    outputs: Vec<WlOutput>,
    /// The parent the handle was last sent, which is one the client has a handle for.
    parent: Option<ToplevelId>,
}

/// The manager global's data: the clients it's offered to.
pub struct ManagerGlobalData {
    filter: GlobalFilter,
}

/// A manager's data.
#[derive(Debug)]
pub struct ManagerData;

/// A handle's data: the toplevel it's for.
#[derive(Debug)]
pub struct HandleData(ToplevelId);

impl ForeignToplevelState {
    /// Offers the manager's global to the clients `filter` allows.
    pub fn new<D, F>(display: &DisplayHandle, filter: F) -> Self
    where
        D: GlobalDispatch<ZwlrForeignToplevelManagerV1, ManagerGlobalData> + 'static,
        F: for<'c> Fn(&'c Client) -> bool + Send + Sync + 'static,
    {
        let data = ManagerGlobalData {
            filter: Box::new(filter),
        };
        let global = display.create_global::<D, ZwlrForeignToplevelManagerV1, _>(VERSION, data);
        Self {
            _global: global,
            toplevels: Vec::new(),
            managers: Vec::new(),
        }
    }

    /// Tells every client what changed since the toplevels were last sent: the toplevels
    /// that are new and those that are gone, and for the rest only what differs, each ended
    /// with `done`.
    pub fn refresh<D>(&mut self, display: &DisplayHandle, toplevels: Vec<Toplevel>)
    where
        D: Dispatch<ZwlrForeignToplevelHandleV1, HandleData> + 'static,
    {
        for manager in &mut self.managers {
            manager.refresh::<D>(display, &self.toplevels, &toplevels);
        }
        self.toplevels = toplevels;
    }

    /// The toplevel a client's handle is for, unless the handle was sent `closed`.
    pub fn toplevel(&self, handle: &ZwlrForeignToplevelHandleV1) -> Option<ToplevelId> {
        let mut handles = self.managers.iter().flat_map(|manager| &manager.handles);
        let handle = handles.find(|known| known.resource == *handle)?;
        Some(handle.id)
    }
}

impl Manager {
    /// Sends what differs between `old`, which the client has, and `new`.
    fn refresh<D>(&mut self, display: &DisplayHandle, old: &[Toplevel], new: &[Toplevel])
    where
        D: Dispatch<ZwlrForeignToplevelHandleV1, HandleData> + 'static,
    {
        self.handles.retain(|handle| {
            let kept = new.iter().any(|toplevel| toplevel.id == handle.id);
            if !kept {
                handle.resource.closed();
            }
            kept
        });
        for toplevel in new {
            let sent = old.iter().find(|sent| sent.id == toplevel.id);
            // A toplevel the client was sent before isn't new: if it has no handle, the
            // client destroyed it.
            if sent.is_none() && !self.stopped {
                self.add_handle::<D>(display, toplevel.id);
            }
            let parent = self.parent_handle(toplevel);
            if let Some(handle) = self.handles.iter_mut().find(|h| h.id == toplevel.id) {
                handle.send_changes(&self.client, sent, toplevel, parent.as_ref());
            }
        }
        // A new toplevel's details go right after it's announced, so its parent, if that
        // one was announced after it, is sent here.
        for toplevel in new {
            let parent = self.parent_handle(toplevel);
            if let Some(handle) = self.handles.iter_mut().find(|h| h.id == toplevel.id)
                && handle.send_parent(toplevel, parent.as_ref())
            {
                handle.resource.done();
            }
        }
    }

    /// Announces a new toplevel. Its details are still to send.
    fn add_handle<D>(&mut self, display: &DisplayHandle, id: ToplevelId)
    where
        D: Dispatch<ZwlrForeignToplevelHandleV1, HandleData> + 'static,
    {
        let version = self.resource.version();
        let Ok(resource) = self
            .client
            .create_resource::<ZwlrForeignToplevelHandleV1, _, D>(display, version, HandleData(id))
        else {
            return;
        };
        self.resource.toplevel(&resource);
        self.handles.push(Handle {
            id,
            resource,
            outputs: Vec::new(),
            parent: None,
        });
    }

    /// The client's handle for a toplevel's parent, if it has one.
    fn parent_handle(&self, toplevel: &Toplevel) -> Option<ZwlrForeignToplevelHandleV1> {
        let parent = toplevel.parent?;
        let parent = self.handles.iter().find(|handle| handle.id == parent)?;
        Some(parent.resource.clone())
    }

    /// Whether the manager is still of use: it takes new toplevels, or has handles to keep
    /// up to date.
    fn is_live(&self) -> bool {
        !self.stopped || !self.handles.is_empty()
    }
}

impl Handle {
    /// Sends what differs between `sent`, which the client has unless the handle is new,
    /// and `toplevel`, then `done` if anything did.
    fn send_changes(
        &mut self,
        client: &Client,
        sent: Option<&Toplevel>,
        toplevel: &Toplevel,
        parent: Option<&ZwlrForeignToplevelHandleV1>,
    ) {
        let handle = &self.resource;
        let mut changed = false;
        if sent.map(|sent| &sent.title) != Some(&toplevel.title) {
            handle.title(toplevel.title.clone());
            changed = true;
        }
        if sent.map(|sent| &sent.app_id) != Some(&toplevel.app_id) {
            handle.app_id(toplevel.app_id.clone());
            changed = true;
        }
        // A client may bind a `wl_output` after it has the handle, so what it was sent is
        // compared by `wl_output`, not by output.
        let outputs: Vec<WlOutput> = toplevel
            .outputs
            .iter()
            .flat_map(|output| output.client_outputs(client))
            .collect();
        for left in self.outputs.iter().filter(|o| !outputs.contains(o)) {
            // A `wl_output` the client released needs no leave.
            if left.is_alive() {
                handle.output_leave(left);
                changed = true;
            }
        }
        for entered in outputs.iter().filter(|o| !self.outputs.contains(o)) {
            handle.output_enter(entered);
            changed = true;
        }
        self.outputs = outputs;
        // Compared as the handle's version shows them, so a version 1 handle isn't told
        // twice of the same states when only fullscreen changed.
        let states = toplevel.states.to_array(handle.version());
        if sent
            .map(|sent| sent.states.to_array(handle.version()))
            .as_ref()
            != Some(&states)
        {
            handle.state(states);
            changed = true;
        }
        changed |= self.send_parent(toplevel, parent);
        if changed {
            self.resource.done();
        }
    }

    /// Sends the toplevel's parent, `parent` being the client's handle for it, if it isn't
    /// the one the handle was last sent. Returns whether it did.
    fn send_parent(
        &mut self,
        toplevel: &Toplevel,
        parent: Option<&ZwlrForeignToplevelHandleV1>,
    ) -> bool {
        let id = toplevel.parent.filter(|_| parent.is_some());
        if id == self.parent || self.resource.version() < SINCE_PARENT {
            return false;
        }
        self.resource.parent(parent);
        self.parent = id;
        true
    }
}

impl<D> GlobalDispatch2<ZwlrForeignToplevelManagerV1, D> for ManagerGlobalData
where
    D: ForeignToplevelHandler
        + Dispatch<ZwlrForeignToplevelManagerV1, ManagerData>
        + Dispatch<ZwlrForeignToplevelHandleV1, HandleData>,
{
    fn bind(
        &self,
        state: &mut D,
        display: &DisplayHandle,
        client: &Client,
        resource: New<ZwlrForeignToplevelManagerV1>,
        data_init: &mut DataInit<'_, D>,
    ) {
        let mut manager = Manager {
            resource: data_init.init(resource, ManagerData),
            client: client.clone(),
            stopped: false,
            handles: Vec::new(),
        };
        let state = state.foreign_toplevel_state();
        manager.refresh::<D>(display, &[], &state.toplevels);
        state.managers.push(manager);
    }

    fn can_view(&self, client: &Client) -> bool {
        (self.filter)(client)
    }
}

impl<D: ForeignToplevelHandler> Dispatch2<ZwlrForeignToplevelManagerV1, D> for ManagerData {
    fn request(
        &self,
        state: &mut D,
        _client: &Client,
        resource: &ZwlrForeignToplevelManagerV1,
        request: zwlr_foreign_toplevel_manager_v1::Request,
        _display: &DisplayHandle,
        _data_init: &mut DataInit<'_, D>,
    ) {
        match request {
            zwlr_foreign_toplevel_manager_v1::Request::Stop => {
                let managers = &mut state.foreign_toplevel_state().managers;
                if let Some(manager) = managers.iter_mut().find(|m| m.resource == *resource) {
                    manager.stopped = true;
                }
                // Which destroys the manager. Its handles live on.
                resource.finished();
            }
            _ => unreachable!("zwlr_foreign_toplevel_manager_v1 has no other request"),
        }
    }

    fn destroyed(&self, state: &mut D, _client: ClientId, resource: &ZwlrForeignToplevelManagerV1) {
        let managers = &mut state.foreign_toplevel_state().managers;
        managers.retain(|manager| manager.resource != *resource || manager.stopped);
        managers.retain(Manager::is_live);
    }
}

impl<D: ForeignToplevelHandler> Dispatch2<ZwlrForeignToplevelHandleV1, D> for HandleData {
    fn request(
        &self,
        state: &mut D,
        _client: &Client,
        resource: &ZwlrForeignToplevelHandleV1,
        request: zwlr_foreign_toplevel_handle_v1::Request,
        _display: &DisplayHandle,
        _data_init: &mut DataInit<'_, D>,
    ) {
        use zwlr_foreign_toplevel_handle_v1::{Error, Request};

        let toplevel = self.0;
        // A handle that was sent `closed` is inert, whatever became of the toplevel.
        let toplevels = &state.foreign_toplevel_state().toplevels;
        let closed = !toplevels.iter().any(|sent| sent.id == toplevel);
        if closed && !matches!(request, Request::Destroy) {
            return;
        }
        match request {
            Request::SetMaximized => state.set_toplevel_maximized(toplevel, true),
            Request::UnsetMaximized => state.set_toplevel_maximized(toplevel, false),
            Request::SetMinimized => state.set_toplevel_minimized(toplevel, true),
            Request::UnsetMinimized => state.set_toplevel_minimized(toplevel, false),
            // The compositor has one seat.
            Request::Activate { seat: _ } => state.activate_toplevel(toplevel),
            Request::Close => state.close_toplevel(toplevel),
            Request::SetRectangle {
                surface,
                x,
                y,
                width,
                height,
            } => {
                if width < 0 || height < 0 {
                    resource.post_error(
                        Error::InvalidRectangle,
                        format!("{width}x{height} isn't a rectangle's size"),
                    );
                    return;
                }
                // An empty rectangle takes the last one back.
                let rectangle = (width > 0 || height > 0).then(|| {
                    (
                        surface,
                        Rectangle::new((x, y).into(), (width, height).into()),
                    )
                });
                state.set_toplevel_rectangle(toplevel, rectangle);
            }
            Request::SetFullscreen { output } => {
                let output = output.and_then(|output| Output::from_resource(&output));
                state.fullscreen_toplevel(toplevel, output);
            }
            Request::UnsetFullscreen => state.unfullscreen_toplevel(toplevel),
            // The handle is forgotten in `destroyed`.
            Request::Destroy => {}
            _ => unreachable!("zwlr_foreign_toplevel_handle_v1 has no other request"),
        }
    }

    fn destroyed(&self, state: &mut D, _client: ClientId, resource: &ZwlrForeignToplevelHandleV1) {
        let managers = &mut state.foreign_toplevel_state().managers;
        for manager in managers.iter_mut() {
            manager
                .handles
                .retain(|handle| handle.resource != *resource);
        }
        managers.retain(Manager::is_live);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn states(array: &[u8]) -> Vec<u32> {
        let (chunks, rest) = array.as_chunks::<4>();
        assert!(rest.is_empty());
        chunks.iter().map(|b| u32::from_ne_bytes(*b)).collect()
    }

    #[test]
    fn the_state_array_has_the_states_that_are_set() {
        let all = ToplevelStates {
            maximized: true,
            minimized: true,
            activated: true,
            fullscreen: true,
        };
        // The protocol's numbers: maximized 0, minimized 1, activated 2, fullscreen 3.
        assert_eq!(states(&all.to_array(3)), [0, 1, 2, 3]);
        assert_eq!(
            states(&ToplevelStates::default().to_array(3)),
            Vec::<u32>::new()
        );
    }

    #[test]
    fn a_version_1_handle_isn_t_told_of_fullscreen() {
        let fullscreen = ToplevelStates {
            activated: true,
            fullscreen: true,
            ..ToplevelStates::default()
        };
        assert_eq!(states(&fullscreen.to_array(1)), [2]);
    }
}
