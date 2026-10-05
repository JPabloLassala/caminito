//! An `xfce_foreign_toplevel_management_private_v1` client, as libxfce4windowing is one
//! (75): for every toplevel foreign toplevel management announces it asks for the Xfce
//! handle, and records the workspace that one says the toplevel is on, among the
//! toplevel's other events. The protocol is generated from the XML Camino's own code is
//! generated from.

use wayland_client::{Connection, Dispatch, QueueHandle};

use self::raw::xfce_foreign_toplevel_handle_v1::{self, XfceForeignToplevelHandleV1};
use self::raw::xfce_foreign_toplevel_icon_pixels_v1::{self, XfceForeignToplevelIconPixelsV1};
use self::raw::xfce_foreign_toplevel_manager_private_v1::{
    self, XfceForeignToplevelManagerPrivateV1,
};
use crate::{Client, State};

/// Xfce's three interfaces, from `wayland-scanner`.
#[allow(
    dead_code,
    non_camel_case_types,
    non_upper_case_globals,
    unused_imports,
    clippy::all,
    reason = "generated code"
)]
pub(crate) mod raw {
    use wayland_client;
    use wayland_client::protocol::*;
    use wayland_protocols::ext::workspace::v1::client::*;
    use wayland_protocols_wlr::foreign_toplevel::v1::client::*;

    pub mod __interfaces {
        use wayland_client::backend as wayland_backend;
        use wayland_client::protocol::__interfaces::*;
        use wayland_protocols::ext::workspace::v1::client::__interfaces::*;
        use wayland_protocols_wlr::foreign_toplevel::v1::client::__interfaces::*;
        wayland_scanner::generate_interfaces!(
            "../camino/resources/xfce-foreign-toplevel-management-private-v1.xml"
        );
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!(
        "../camino/resources/xfce-foreign-toplevel-management-private-v1.xml"
    );
}

impl Client {
    /// Binds Xfce's manager, and asks for the Xfce handle of every open toplevel the client
    /// has, and of each one announced from now on. `bind_foreign_toplevels` first.
    pub fn bind_toplevel_workspaces(&mut self) {
        let manager: XfceForeignToplevelManagerPrivateV1 =
            crate::bind(&self.registry, &self.state.globals, &self.qh, 1);
        for toplevel in &mut self.state.foreign_toplevels.toplevels {
            if !toplevel.closed {
                let xfce = manager.get_xfce_foreign_toplevel_handle(&toplevel.proxy, &self.qh, ());
                toplevel.xfce = Some(xfce);
            }
        }
        self.state.foreign_toplevels.xfce_manager = Some(manager);
    }

    /// Asks for a second Xfce handle for a toplevel that has one, which is a protocol
    /// error.
    pub fn get_another_xfce_handle(&self, index: usize) {
        let foreign = &self.state.foreign_toplevels;
        let manager = foreign.xfce_manager.as_ref();
        manager
            .expect("bind_toplevel_workspaces first")
            .get_xfce_foreign_toplevel_handle(&foreign.toplevels[index].proxy, &self.qh, ());
    }

    /// Makes every request of a toplevel's Xfce handle that changes the toplevel, which a
    /// compositor that only says where its toplevels are ignores, and asks for an icon's
    /// pixels, which it hasn't.
    pub fn send_other_xfce_requests(&mut self, index: usize) {
        let seat = self.seat();
        let output = self
            .state
            .outputs
            .first()
            .map(|output| output.proxy.clone());
        let workspace = self.state.workspaces.workspaces.last();
        let workspace = workspace.map(|workspace| workspace.proxy().clone());
        let xfce = self.state.foreign_toplevels.toplevels[index].xfce.clone();
        let xfce = xfce.expect("bind_toplevel_workspaces first");
        xfce.set_shaded();
        xfce.unset_shaded();
        xfce.set_sticky();
        xfce.unset_sticky();
        xfce.set_above();
        xfce.unset_above();
        xfce.set_below();
        xfce.unset_below();
        xfce.highlight(&seat);
        xfce.unhighlight();
        xfce._move(&seat);
        xfce.resize(&seat);
        if let Some(output) = &output {
            xfce.move_to_output(output);
        }
        if let Some(workspace) = &workspace {
            xfce.move_to_workspace(workspace);
        }
        xfce.get_icon_pixels(16, 1, &self.qh, index);
    }
}

impl Dispatch<XfceForeignToplevelManagerPrivateV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &XfceForeignToplevelManagerPrivateV1,
        _: xfce_foreign_toplevel_manager_private_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<XfceForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &XfceForeignToplevelHandleV1,
        event: xfce_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use xfce_foreign_toplevel_handle_v1::Event;

        let toplevels = &state.foreign_toplevels.toplevels;
        let Some(index) = (toplevels.iter()).position(|t| t.xfce.as_ref() == Some(proxy)) else {
            return;
        };
        let workspace = |workspace| state.workspace_index(workspace);
        // A workspace the client was told is gone is one whose handle it has destroyed, so
        // an event that names it comes too late.
        let name = |index: Option<usize>| match index {
            Some(index) if state.workspaces.workspaces[index].removed => {
                format!("workspace {index}, which was removed")
            }
            Some(index) => format!("workspace {index}"),
            None => "an unknown workspace".to_owned(),
        };
        let (text, on) = match event {
            Event::WorkspaceEnter { workspace: entered } => {
                let entered = workspace(&entered);
                (format!("workspace_enter {}", name(entered)), entered)
            }
            Event::WorkspaceLeave { workspace: left } => {
                (format!("workspace_leave {}", name(workspace(&left))), None)
            }
            Event::State { state: states } => {
                (format!("xfce state {states:?}"), toplevels[index].workspace)
            }
            Event::IconName { name } => (format!("icon_name {name}"), toplevels[index].workspace),
            Event::IconSize { size, scale } => (
                format!("icon_size {size} at {scale}"),
                toplevels[index].workspace,
            ),
            Event::NoIcon => ("no_icon".to_owned(), toplevels[index].workspace),
        };
        let foreign = &mut state.foreign_toplevels;
        foreign.toplevels[index].workspace = on;
        foreign.push_event(index, &text);
    }
}

/// The index of the toplevel whose icon was asked for.
impl Dispatch<XfceForeignToplevelIconPixelsV1, usize> for State {
    fn event(
        state: &mut Self,
        proxy: &XfceForeignToplevelIconPixelsV1,
        event: xfce_foreign_toplevel_icon_pixels_v1::Event,
        index: &usize,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use xfce_foreign_toplevel_icon_pixels_v1::Event;

        let text = match event {
            Event::Pixels { width, height, .. } => format!("icon pixels {width}x{height}"),
            Event::Failed { reason } => format!("icon failed {reason:?}"),
        };
        proxy.destroy();
        state.foreign_toplevels.push_event(*index, &text);
    }
}
