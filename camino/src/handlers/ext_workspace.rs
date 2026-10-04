//! ext-workspace (66): policy's workspaces are sent in the refresh pass, and a client's
//! `activate` switches workspace through policy, as the keybinding does.

use camino_policy::Action;

use crate::protocols::ext_workspace::{ExtWorkspaceHandler, ExtWorkspaceState, Workspace};
use crate::state::{Camino, SEAT, State};

/// A workspace's id for ext-workspace: its number, from 1, which is what stays the same
/// from one session to the next.
fn workspace_id(index: usize) -> String {
    (index + 1).to_string()
}

/// The workspace an id names, if it's one of `count`.
fn workspace_index(id: &str, count: usize) -> Option<usize> {
    let index = id.parse::<usize>().ok()?.checked_sub(1)?;
    (index < count).then_some(index)
}

impl ExtWorkspaceHandler for State {
    fn ext_workspace_state(&mut self) -> &mut ExtWorkspaceState {
        &mut self.camino.ext_workspace_state
    }

    fn activate_workspace(&mut self, id: &str) {
        let camino = &mut self.camino;
        let Some(index) = workspace_index(id, camino.policy.workspace_count()) else {
            return;
        };
        let now = camino.now();
        camino
            .policy
            .action(SEAT, Action::SwitchToWorkspace(index), now);
        camino.queue_redraw_all();
    }
}

impl Camino {
    /// Tells ext-workspace's clients of the outputs and workspaces that changed. The
    /// workspaces are one row until M6's layout keys (09, 17).
    pub(crate) fn refresh_ext_workspace(&mut self) {
        let active = self.policy.active_workspace();
        let names = self.policy.workspace_names().iter();
        let workspaces = names
            .enumerate()
            .map(|(index, name)| Workspace {
                id: workspace_id(index),
                name: name.clone(),
                coordinates: [index as u32, 0],
                active: index == active,
            })
            .collect();
        self.ext_workspace_state.refresh::<State>(
            &self.display_handle,
            self.outputs.clone(),
            workspaces,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_names_the_workspace_of_its_number() {
        assert_eq!(workspace_id(0), "1");
        assert_eq!(workspace_index("1", 4), Some(0));
        assert_eq!(workspace_index("4", 4), Some(3));
    }

    #[test]
    fn an_id_that_isn_t_a_workspace_s_names_none() {
        assert_eq!(workspace_index("5", 4), None);
        assert_eq!(workspace_index("0", 4), None);
        assert_eq!(workspace_index("Mail", 4), None);
    }
}
