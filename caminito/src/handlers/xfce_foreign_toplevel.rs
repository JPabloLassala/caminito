//! Xfce's foreign toplevel protocol (75): each listed window's workspace is sent in the
//! refresh pass, by the id ext-workspace knows the workspace by.

use crate::protocols::xfce_foreign_toplevel::{
    XfceForeignToplevelHandler, XfceForeignToplevelState,
};
use crate::state::{Caminito, State};

use super::ext_workspace::workspace_id;
use super::foreign_toplevel::window_id;

impl XfceForeignToplevelHandler for State {
    fn xfce_foreign_toplevel_state(&mut self) -> &mut XfceForeignToplevelState {
        &mut self.caminito.xfce_foreign_toplevel_state
    }
}

impl Caminito {
    /// Tells the clients of Xfce's foreign toplevel protocol of the windows that changed
    /// workspace. A sticky window is on the one workspace policy has it on, until being
    /// sticky takes effect in M8 (17).
    pub(crate) fn refresh_toplevel_workspaces(&mut self) {
        let policy = &self.policy;
        self.xfce_foreign_toplevel_state.refresh(
            &self.foreign_toplevel_state,
            &self.ext_workspace_state,
            |toplevel| policy.workspace_of(window_id(toplevel)).map(workspace_id),
        );
    }
}
