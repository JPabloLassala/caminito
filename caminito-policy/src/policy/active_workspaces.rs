//! Each output's active workspace, and the one before it (ADR 0032). With
//! `workspaces-per-output` on an output can switch by itself; off, every switch sets them
//! all, so they're one: Spanning workspaces, which is a mode of this code path and not a
//! second one. A window is shown by its own Main output's (230).

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
    /// Every change to them all changes it too, and a switch of one output doesn't.
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

    /// `output`'s, to switch it by itself.
    pub(super) fn of_mut(&mut self, output: OutputId) -> Option<&mut Shown> {
        self.outputs.get_mut(&output)
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
    /// The primary output's, or the one an output added takes while policy has none. It
    /// stands in for the Active output's until ticket 231 gives the seat one.
    pub(super) fn primary_shown(&self) -> Shown {
        let primary = self.primary_output().map(|output| output.id);
        self.active_workspaces.of(primary)
    }

    /// Every output takes the primary output's active and previous workspace, as
    /// `workspaces-per-output` is turned off (230): it stands in for the Active output's
    /// (231). No window changes its workspace, and an output that shows another one stops
    /// showing it, as a switch does.
    pub(super) fn span_workspaces(&mut self) {
        let primary = self.primary_shown();
        let switches = self
            .active_workspaces
            .each()
            .any(|shown| shown.active != primary.active);
        for shown in self.active_workspaces.each_mut() {
            *shown = primary;
        }
        if switches {
            self.close_switcher(switcher::Close::Dropped);
            self.window_menu = None;
        }
    }

    /// Notes each window's Main output, with no window following it.
    pub(super) fn note_main_outputs(&mut self) {
        let ids: Vec<WindowId> = self.windows.keys().copied().collect();
        for id in ids {
            let output = self.output_of(self.windows[&id].frame()).map(|o| o.id);
            self.windows.get_mut(&id).unwrap().main_output = output;
        }
    }

    /// A window whose Main output changed since `apply` last looked, because most of it
    /// has crossed to another output, takes the active workspace of the output it's on now
    /// if it was showing on the one it left, so that it stays in view (230). Its family
    /// goes with it, so it's a root's crossing that counts: a transient's is ticket 223's,
    /// as is a sticky window's, which is on every workspace. Spanning workspaces have
    /// one active workspace, so there it's always the same.
    pub(super) fn follow_main_outputs(&mut self) {
        let ids: Vec<WindowId> = self.windows.keys().copied().collect();
        for id in ids {
            let managed = &self.windows[&id];
            let now = self.output_of(managed.frame()).map(|o| o.id);
            let was = managed.main_output;
            let follows =
                self.config.workspaces_per_output && managed.parent.is_none() && !managed.sticky;
            let workspace = managed.workspace;
            self.windows.get_mut(&id).unwrap().main_output = now;
            let (Some(was), Some(now)) = (was, now) else {
                continue;
            };
            let to = self.active_workspaces.of(Some(now)).active;
            let shown_on_was = self.active_workspaces.of(Some(was)).active == workspace;
            if was == now || !follows || !shown_on_was || to == workspace {
                continue;
            }
            for w in self.tree(id) {
                self.windows.get_mut(&w).unwrap().workspace = to;
            }
        }
    }

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
        self.primary_active_workspace()
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
