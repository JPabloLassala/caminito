//! wlr-foreign-toplevel-management and ext-foreign-toplevel-list (65): the windows are sent
//! in the refresh pass, and a client's request goes to policy as the same user action does.

use camino_policy::{PolicyWindow, WindowId};
use smithay::output::Output;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Rectangle};
use smithay::wayland::foreign_toplevel_list::{
    ForeignToplevelListHandler, ForeignToplevelListState,
};

use crate::output::{output_geometry, output_id};
use crate::protocols::foreign_toplevel::{
    ForeignToplevelHandler, ForeignToplevelState, Toplevel, ToplevelId, ToplevelStates,
};
use crate::state::{Camino, SEAT, State};
use crate::window::Mapped;

/// Policy's id for a toplevel, which is named by it.
pub(super) fn window_id(toplevel: ToplevelId) -> WindowId {
    WindowId(toplevel.0)
}

/// What foreign toplevel management names a window by.
fn toplevel_id(window: WindowId) -> ToplevelId {
    ToplevelId(window.0)
}

impl ForeignToplevelHandler for State {
    fn foreign_toplevel_state(&mut self) -> &mut ForeignToplevelState {
        &mut self.camino.foreign_toplevel_state
    }

    /// As Marco's `_NET_ACTIVE_WINDOW` from a pager: policy switches to the window's
    /// workspace, unminimises it, raises it and focuses it.
    fn activate_toplevel(&mut self, toplevel: ToplevelId) {
        let now = self.camino.now();
        self.camino.policy.activate(SEAT, window_id(toplevel), now);
        self.camino.queue_redraw_all();
    }

    fn close_toplevel(&mut self, toplevel: ToplevelId) {
        self.camino.policy.close(window_id(toplevel));
    }

    fn set_toplevel_maximized(&mut self, toplevel: ToplevelId, maximized: bool) {
        let policy = &mut self.camino.policy;
        policy.set_maximized(window_id(toplevel), maximized);
        self.camino.queue_redraw_all();
    }

    /// A window list restores a window by activating it, so that's what unminimising is.
    fn set_toplevel_minimized(&mut self, toplevel: ToplevelId, minimized: bool) {
        if minimized {
            self.camino.policy.minimize(window_id(toplevel));
            self.camino.queue_redraw_all();
        } else if self.camino.policy.is_minimized(window_id(toplevel)) {
            self.activate_toplevel(toplevel);
        }
    }

    fn fullscreen_toplevel(&mut self, toplevel: ToplevelId, output: Option<Output>) {
        let output = output.map(|output| output_id(&output));
        let policy = &mut self.camino.policy;
        policy.set_fullscreen(window_id(toplevel), true, output);
        self.camino.queue_redraw_all();
    }

    fn unfullscreen_toplevel(&mut self, toplevel: ToplevelId) {
        let policy = &mut self.camino.policy;
        policy.set_fullscreen(window_id(toplevel), false, None);
        self.camino.queue_redraw_all();
    }

    fn set_toplevel_rectangle(
        &mut self,
        toplevel: ToplevelId,
        rectangle: Option<(WlSurface, Rectangle<i32, Logical>)>,
    ) {
        if let Some(mapped) = self.camino.policy.window_mut(window_id(toplevel)) {
            mapped.set_minimize_rectangle(rectangle);
        }
    }
}

impl ForeignToplevelListHandler for State {
    fn foreign_toplevel_list_state(&mut self) -> &mut ForeignToplevelListState {
        &mut self.camino.foreign_toplevel_list_state
    }
}

impl Camino {
    /// Tells foreign toplevel management's clients of the windows that changed: every
    /// managed window on every workspace, but those that skip the taskbar by Marco's rules
    /// (35).
    pub(crate) fn refresh_foreign_toplevels(&mut self) {
        let policy = &self.policy;
        let mut listed: Vec<&Mapped> = policy
            .windows()
            .filter(|mapped| !mapped.skips().taskbar)
            .collect();
        // In the order the windows mapped.
        listed.sort_by_key(|mapped| mapped.id());
        let is_listed = |id: &WindowId| listed.iter().any(|mapped| mapped.id() == *id);
        let focused = policy.focus(SEAT);
        let toplevels = listed
            .iter()
            .map(|mapped| {
                let id = mapped.id();
                // Where the window is, shown or not: a window list for one monitor keeps
                // the windows minimised there, and those on its other workspaces.
                let geometry = policy.geometry(id).unwrap_or_default();
                let overlaps = |output: &&Output| output_geometry(output).overlaps(geometry);
                Toplevel {
                    id: toplevel_id(id),
                    title: mapped.title(),
                    app_id: mapped.app_id().unwrap_or_default(),
                    states: ToplevelStates {
                        maximized: policy.is_maximized(id),
                        minimized: policy.is_minimized(id),
                        activated: focused == Some(id),
                        fullscreen: policy.is_fullscreen(id),
                    },
                    outputs: self.outputs.iter().filter(overlaps).cloned().collect(),
                    parent: policy.parent(id).filter(is_listed).map(toplevel_id),
                }
            })
            .collect();
        self.foreign_toplevel_state
            .refresh::<State>(&self.display_handle, toplevels);
    }

    /// Tells the foreign toplevel list's clients of the windows that changed. It lists
    /// every managed window, the ones that skip the taskbar too: its identifiers are what
    /// M6's `GetActiveToplevel` and image capture name a window by, and a dialog is one.
    pub(crate) fn refresh_foreign_toplevel_list(&mut self) {
        let list = &mut self.foreign_toplevel_list_state;
        let policy = &self.policy;
        self.foreign_toplevel_handles.retain(|id, handle| {
            let managed = policy.window(*id).is_some();
            if !managed {
                list.remove_toplevel(handle);
            }
            managed
        });
        let mut managed: Vec<&Mapped> = policy.windows().collect();
        // In the order the windows mapped.
        managed.sort_by_key(|mapped| mapped.id());
        for mapped in managed {
            let (title, app_id) = (mapped.title(), mapped.app_id().unwrap_or_default());
            match self.foreign_toplevel_handles.get(&mapped.id()) {
                Some(handle) => {
                    if handle.title() != title || handle.app_id() != app_id {
                        handle.send_title(&title);
                        handle.send_app_id(&app_id);
                        handle.send_done();
                    }
                }
                None => {
                    let handle = list.new_toplevel::<State>(title, app_id);
                    self.foreign_toplevel_handles.insert(mapped.id(), handle);
                }
            }
        }
    }

    /// The identifier ext-foreign-toplevel-list's clients know a window by, which stays the
    /// same while the window is managed.
    pub(crate) fn toplevel_identifier(&self, id: WindowId) -> Option<String> {
        let handle = self.foreign_toplevel_handles.get(&id)?;
        Some(handle.identifier())
    }

    /// The identifier of the window a window screenshot is of (104), or the empty string
    /// when there's none: the window on screen that most recently had the focus, which is
    /// Marco's active window, passing over those of the process `caller`. mate-screenshot's
    /// dialog is a window of the caller's, and has the focus when it asks.
    pub(crate) fn active_toplevel(&self, caller: Option<u32>) -> String {
        let of_another = |id: &WindowId| {
            let process = self.policy.window(*id).and_then(|mapped| mapped.pid());
            caller.is_none() || process != caller
        };
        self.policy
            .recently_focused()
            .find(of_another)
            .and_then(|id| self.toplevel_identifier(id))
            .unwrap_or_default()
    }

    /// The window ext-foreign-toplevel-list's clients know by `identifier`.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "M6's image capture finds a window by it")
    )]
    pub(crate) fn window_for_toplevel_identifier(&self, identifier: &str) -> Option<WindowId> {
        self.foreign_toplevel_handles
            .iter()
            .find(|(_, handle)| handle.identifier() == identifier)
            .map(|(id, _)| *id)
    }
}
