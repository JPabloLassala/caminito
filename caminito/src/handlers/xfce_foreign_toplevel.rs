//! Xfce's foreign toplevel protocol (75): each listed window's workspace, by the id
//! ext-workspace knows the workspace by, and whether it's sticky, kept above or kept below,
//! are sent in the refresh pass, and a request to change one of those states goes to policy
//! (155). Whether it's urgent is sent too, for a window of either kind (166).

use smithay::reexports::wayland_protocols_wlr::foreign_toplevel::v1::server::zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1;

use crate::protocols::xfce_foreign_toplevel::{
    XfceForeignToplevelHandler, XfceForeignToplevelState, XfceState, XfceToplevel,
};
use crate::state::{Caminito, State};

use super::ext_workspace::workspace_id;
use super::foreign_toplevel::window_id;

impl XfceForeignToplevelHandler for State {
    fn xfce_foreign_toplevel_state(&mut self) -> &mut XfceForeignToplevelState {
        &mut self.caminito.xfce_foreign_toplevel_state
    }

    /// As the same request through `_NET_WM_STATE` does.
    fn set_toplevel_state(
        &mut self,
        toplevel: &ZwlrForeignToplevelHandleV1,
        state: XfceState,
        set: bool,
    ) {
        let caminito = &mut self.caminito;
        let Some(toplevel) = caminito.foreign_toplevel_state.toplevel(toplevel) else {
            return;
        };
        let id = window_id(toplevel);
        let policy = &mut caminito.policy;
        match state {
            XfceState::Sticky => policy.set_sticky(id, set),
            XfceState::Above => policy.set_above(id, set),
            XfceState::Below => policy.set_below(id, set),
        }
        caminito.queue_redraw_all();
    }
}

impl Caminito {
    /// Tells the clients of Xfce's foreign toplevel protocol of the windows that changed
    /// workspace or state. A sticky window is on every workspace, which the panel learns
    /// from its state, and is said to be on the one policy keeps for it.
    pub(crate) fn refresh_toplevel_workspaces(&mut self) {
        let policy = &self.policy;
        self.xfce_foreign_toplevel_state.refresh(
            &self.foreign_toplevel_state,
            &self.ext_workspace_state,
            |toplevel| {
                let id = window_id(toplevel);
                XfceToplevel {
                    workspace: policy.workspace_of(id).map(workspace_id),
                    sticky: policy.is_sticky(id),
                    above: policy.is_above(id),
                    below: policy.is_below(id),
                    demands_attention: policy.is_urgent(id),
                }
            },
        );
    }
}
