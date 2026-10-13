//! ext-workspace (66): policy's workspaces are sent in the refresh pass, each where the grid
//! has it (96), and a client's `activate` switches workspace through policy, as the
//! keybinding does.

use caminito_policy::{Action, Policy, WindowId};
use smithay::output::Output;

use crate::output::output_id;
use crate::protocols::ext_workspace::{ExtWorkspaceHandler, ExtWorkspaceState, Group, Workspace};
use crate::state::{Caminito, SEAT, State};
use crate::window::Mapped;

/// The id of the one group there is with `workspaces-per-output` off. No output's
/// connector has it.
const SPANNING_GROUP: &str = "";

/// A workspace's id for ext-workspace. The one group's workspace has its number, from 1,
/// which is what stays the same from one session to the next. A workspace of an output's
/// own group has `connector`, the output's, before its number, as `DP-1:3`: no two
/// outputs share a connector, and a monitor keeps its own while it stays plugged where it
/// is (226).
fn workspace_id(connector: Option<&str>, index: usize) -> String {
    match connector {
        Some(connector) => format!("{connector}:{}", index + 1),
        None => (index + 1).to_string(),
    }
}

/// The output's connector, if the id has one, and the workspace an id names, if it's one
/// of `count`.
fn parse_workspace_id(id: &str, count: usize) -> Option<(Option<&str>, usize)> {
    let (connector, number) = match id.rsplit_once(':') {
        Some((connector, number)) => (Some(connector), number),
        None => (None, id),
    };
    let index = number.parse::<usize>().ok()?.checked_sub(1)?;
    (index < count).then_some((connector, index))
}

impl ExtWorkspaceHandler for State {
    fn ext_workspace_state(&mut self) -> &mut ExtWorkspaceState {
        &mut self.caminito.ext_workspace_state
    }

    /// A workspace of the one group is switched to as the keybinding does. A workspace of
    /// an output's group switches that output, and makes no output the Active output
    /// (ADR 0032). An id of the other kind than the groups there are now is from before
    /// `workspaces-per-output` changed, and is ignored.
    fn activate_workspace(&mut self, id: &str) {
        let caminito = &mut self.caminito;
        let count = caminito.policy.workspace_count();
        let Some((connector, index)) = parse_workspace_id(id, count) else {
            return;
        };
        match (connector, caminito.policy.workspaces_per_output()) {
            (None, false) => {
                let now = caminito.now();
                caminito
                    .policy
                    .action(SEAT, Action::SwitchToWorkspace(index), now);
            }
            (Some(connector), true) => {
                let mut outputs = caminito.outputs.iter();
                if let Some(output) = outputs.find(|output| output.name() == connector) {
                    caminito
                        .policy
                        .switch_output_workspace(output_id(output), index);
                }
            }
            _ => {}
        }
    }
}

impl Caminito {
    /// Tells ext-workspace's clients of the groups, outputs and workspaces that changed.
    /// Each workspace's coordinates are its cell in policy's grid (96). With
    /// `workspaces-per-output` on, each output has a group with its own workspaces and its
    /// own active one (ADR 0032). Off, the one group, of every output, has the primary
    /// output's active workspace, which is every output's.
    pub(crate) fn refresh_ext_workspace(&mut self) {
        let policy = &self.policy;
        let grid = policy.workspace_grid();
        let workspaces = |connector: Option<&str>, active: usize| -> Vec<Workspace> {
            let names = policy.workspace_names().iter();
            names
                .enumerate()
                .map(|(index, name)| {
                    let (column, row) = grid.cell(index);
                    Workspace {
                        id: workspace_id(connector, index),
                        name: name.clone(),
                        coordinates: [column as u32, row as u32],
                        active: index == active,
                    }
                })
                .collect()
        };
        let groups = if policy.workspaces_per_output() {
            let outputs = self.outputs.iter();
            outputs
                .map(|output| {
                    let connector = output.name();
                    let active = policy.active_workspace_on(output_id(output));
                    Group {
                        workspaces: workspaces(Some(&connector), active),
                        id: connector,
                        outputs: vec![output.clone()],
                    }
                })
                .collect()
        } else {
            vec![Group {
                id: SPANNING_GROUP.to_owned(),
                outputs: self.outputs.clone(),
                workspaces: workspaces(None, policy.primary_active_workspace()),
            }]
        };
        self.ext_workspace_state
            .refresh::<State>(&self.display_handle, groups);
    }
}

/// The id ext-workspace knows `id`'s workspace by. With `workspaces-per-output` on, it's
/// the workspace of the group of the output `id` shows by, one of `outputs`: its own Main
/// output, or its root's for a transient (ADR 0032).
pub(super) fn workspace_id_of(
    policy: &Policy<Mapped>,
    outputs: &[Output],
    id: WindowId,
) -> Option<String> {
    let index = policy.workspace_of(id)?;
    if !policy.workspaces_per_output() {
        return Some(workspace_id(None, index));
    }
    let showing = policy.output_showing(id)?;
    let output = outputs.iter().find(|output| output_id(output) == showing)?;
    Some(workspace_id(Some(&output.name()), index))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_names_the_workspace_of_its_number() {
        assert_eq!(workspace_id(None, 0), "1");
        assert_eq!(parse_workspace_id("1", 4), Some((None, 0)));
        assert_eq!(parse_workspace_id("4", 4), Some((None, 3)));
    }

    #[test]
    fn an_id_of_an_output_s_group_names_the_output_s_connector_too() {
        assert_eq!(workspace_id(Some("HDMI-A-1"), 2), "HDMI-A-1:3");
        assert_eq!(
            parse_workspace_id("HDMI-A-1:3", 4),
            Some((Some("HDMI-A-1"), 2))
        );
    }

    #[test]
    fn an_id_that_isn_t_a_workspace_s_names_none() {
        assert_eq!(parse_workspace_id("5", 4), None);
        assert_eq!(parse_workspace_id("0", 4), None);
        assert_eq!(parse_workspace_id("Mail", 4), None);
        assert_eq!(parse_workspace_id("DP-1:5", 4), None);
        assert_eq!(parse_workspace_id("DP-1:", 4), None);
    }
}
