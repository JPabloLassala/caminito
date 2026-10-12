//! Each output's active workspace, and the one before it (ADR 0032). Every switch sets them
//! all, so for now they're one: Spanning workspaces, which is a mode of this code path and
//! not a second one. Ticket 230 has a window shown by its own output's.

use super::*;

/// One output's active workspace, and the one active before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct Shown {
    pub(super) active: usize,
    /// The workspace active before `active`, which `switch-to-workspace-prev` goes back to
    /// (150). Every switch sets it, as Marco's `meta_workspace_activate_with_focus` does
    /// (workspace.c:399-400), and it's forgotten when its workspace is removed.
    pub(super) previous: Option<usize>,
}

/// Every output's [`Shown`], by its id, and the one an output added takes.
#[derive(Debug, Clone, Default)]
pub(super) struct ActiveWorkspaces {
    outputs: BTreeMap<OutputId, Shown>,
    /// What an output added takes, and what stands for the outputs while policy has none.
    /// Every change to them all changes it too.
    unplaced: Shown,
}

impl ActiveWorkspaces {
    /// `output`'s, or the one an output added takes while policy has no such output.
    pub(super) fn of(&self, output: Option<OutputId>) -> Shown {
        output
            .and_then(|output| self.outputs.get(&output))
            .copied()
            .unwrap_or(self.unplaced)
    }

    /// The one every output has, while every switch sets them all. Each caller is a place
    /// where tickets 230 and 231 choose an output.
    pub(super) fn everywhere(&self) -> Shown {
        self.unplaced
    }

    /// Each output's, or the one that stands for them while policy has none.
    pub(super) fn each(&self) -> impl Iterator<Item = Shown> + '_ {
        let unplaced = self.outputs.is_empty().then_some(self.unplaced);
        self.outputs.values().copied().chain(unplaced)
    }

    /// Each output's, and the one an output added takes, to change them all alike.
    pub(super) fn each_mut(&mut self) -> impl Iterator<Item = &mut Shown> {
        self.outputs
            .values_mut()
            .chain(std::iter::once(&mut self.unplaced))
    }

    /// Follows policy's outputs: an output added takes the active workspace the others
    /// have, and one removed takes its entry with it.
    pub(super) fn set_outputs(&mut self, outputs: &[Output]) {
        self.outputs
            .retain(|id, _| outputs.iter().any(|output| output.id == *id));
        for output in outputs {
            self.outputs.entry(output.id).or_insert(self.unplaced);
        }
    }
}

impl<W: PolicyWindow> Policy<W> {
    /// The active workspace of `id`'s Main output, the output its frame is most on, or of
    /// the primary output for a window on none.
    pub(super) fn active_workspace_of(&self, id: WindowId) -> usize {
        let output = self
            .windows
            .get(&id)
            .and_then(|managed| self.output_of(managed.frame()))
            .or(self.primary_output())
            .map(|output| output.id);
        self.active_workspaces.of(output).active
    }

    /// Whether a managed window is on the workspace its Main output shows: it's sticky, or
    /// that's its workspace.
    pub(super) fn is_on_shown_workspace(&self, id: WindowId) -> bool {
        self.is_on_workspace(id, self.active_workspace_of(id))
    }

    /// The workspace a new window opens on, unless it asks for one: the primary output's
    /// active workspace, which stands in for the Active output's until ticket 222.
    pub(super) fn new_window_workspace(&self) -> usize {
        let primary = self.primary_output().map(|output| output.id);
        self.active_workspaces.of(primary).active
    }

    /// Every output's active workspace, once each, or the one that stands for them while
    /// policy has no output.
    pub(super) fn shown_workspaces(&self) -> BTreeSet<usize> {
        self.active_workspaces
            .each()
            .map(|shown| shown.active)
            .collect()
    }
}
