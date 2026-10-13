//! Each output's active workspace, and the one before it (ADR 0032). With
//! `workspaces-per-output` on an output can switch by itself; off, every switch sets them
//! all, so they're one: Spanning workspaces, which is a mode of this code path and not a
//! second one. A window is shown by its own Main output's (230), and a transient by its
//! family's root's (223).

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

/// One output's change of active workspace, as a switch made it (233), or as
/// `dynamic-workspaces` removing the workspace it showed did (236). What turns
/// `workspaces-per-output` on or off, an output that comes or goes, and a lower
/// `num-workspaces` aren't switches, and give none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputSwitch {
    /// The output that switched.
    pub output: OutputId,
    /// The workspace that was active.
    pub from: usize,
    /// The one that is.
    pub to: usize,
    /// How far the switch goes in the Workspace grid, in columns to the right and in rows
    /// down: from the old workspace's cell to the new one's, or one cell the way a binding
    /// asked to go, which a switch that wraps past the grid's edge keeps to.
    pub cells: (i32, i32),
}

/// Every output's [`Shown`], by its id, and the one an output added takes.
#[derive(Debug, Clone, Default)]
pub(super) struct ActiveWorkspaces {
    outputs: BTreeMap<OutputId, Shown>,
    /// What stands for the outputs while policy has none, and what the first to come
    /// takes. With Spanning workspaces it's the one every output has, which an output added
    /// takes too. Every change to them all changes it too, and a switch of one output
    /// doesn't: `set_outputs` takes the primary output's as the last output goes.
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

    /// Follows policy's outputs as they become `outputs`: one removed takes its entry with
    /// it, and one added starts on the first workspace, with none before it (225). With
    /// Spanning workspaces, `spanning`, it takes the active workspace the others have.
    ///
    /// Where no output stays, `primary`'s is kept for those that come, at once or later,
    /// so that a user's only monitor shows what it did after it's unplugged, or swapped
    /// for another.
    pub(super) fn set_outputs(
        &mut self,
        outputs: &[Output],
        primary: Option<OutputId>,
        spanning: bool,
    ) {
        let stays = |id: &OutputId| outputs.iter().any(|output| output.id == *id);
        let added = if self.outputs.keys().any(stays) && !spanning {
            Shown::default()
        } else {
            if !spanning {
                self.unplaced = self.of(primary);
            }
            self.unplaced
        };
        self.outputs.retain(|id, _| stays(id));
        for output in outputs {
            self.outputs.entry(output.id).or_insert(added);
        }
    }
}

impl<W: PolicyWindow> Policy<W> {
    /// The primary output's, or the one an output added takes while policy has none.
    pub(super) fn primary_shown(&self) -> Shown {
        let primary = self.primary_output().map(|output| output.id);
        self.active_workspaces.of(primary)
    }

    /// `seat`'s Active output's, or the one an output added takes while policy has none.
    pub(super) fn active_shown(&self, seat: SeatId) -> Shown {
        self.active_workspaces.of(self.active_output(seat))
    }

    /// Notes the switch to `workspace` of each output that shows another, of `only` or of
    /// them all, before it's made. `towards` is the way a binding asked to go.
    pub(super) fn note_output_switches(
        &mut self,
        only: Option<OutputId>,
        workspace: usize,
        towards: Option<Direction>,
    ) {
        let grid = self.workspace_grid();
        for output in &self.outputs {
            let from = self.active_workspaces.of(Some(output.id)).active;
            if from == workspace || only.is_some_and(|only| only != output.id) {
                continue;
            }
            let cells = match towards {
                Some(direction) => direction.step(),
                None => grid.cells_between(from, workspace),
            };
            self.output_switches.push(OutputSwitch {
                output: output.id,
                from,
                to: workspace,
                cells,
            });
        }
    }

    /// Every output takes the active and previous workspace of the Active output, that of
    /// the seat that last acted, as `workspaces-per-output` is turned off (230, 231). No
    /// window changes its workspace, and an output that shows another one stops showing
    /// it, as a switch does.
    pub(super) fn span_workspaces(&mut self) {
        let active = self.active_shown(self.last_seat);
        let switches = self
            .active_workspaces
            .each()
            .any(|shown| shown.active != active.active);
        for shown in self.active_workspaces.each_mut() {
            *shown = active;
        }
        // The one workspace of a number shows its desktop if the Active output's did.
        self.give_shown_desktops(self.active_output(self.last_seat), &[None]);
        if switches {
            // As a switch closes them: the workspace changing under an open switcher (ruling
            // 9 of 108), and the window menu, whose moves are from the active workspace (161).
            self.close_switcher(switcher::Close::Dropped);
            self.window_menu = None;
        }
    }

    /// Gives the desktops `from`'s workspaces show to the workspaces of the same numbers of
    /// each of `to`, and no other workspace shows one: as `workspaces-per-output` changes,
    /// and as policy's first output comes or its last one goes (224).
    pub(super) fn give_shown_desktops(&mut self, from: Option<OutputId>, to: &[Option<OutputId>]) {
        let shown = std::mem::take(&mut self.showing_desktop);
        for ((_, workspace), shown) in shown.into_iter().filter(|((of, _), _)| *of == from) {
            for output in to {
                self.showing_desktop
                    .insert((*output, workspace), shown.clone());
            }
        }
    }

    /// The desktops shown follow policy's outputs as they become `outputs`: those of an
    /// output that goes go with it. With no output the primary one's are kept for the
    /// first that comes.
    pub(super) fn desktops_follow_outputs(&mut self, outputs: &[Output]) {
        if !self.config.workspaces_per_output {
            return;
        }
        let to: Vec<Option<OutputId>> = outputs.iter().map(|output| Some(output.id)).collect();
        if outputs.is_empty() {
            let primary = self.primary_output().map(|output| output.id);
            self.give_shown_desktops(primary, &[None]);
        } else if self.outputs.is_empty() {
            self.give_shown_desktops(None, &to);
        } else {
            self.showing_desktop
                .retain(|(output, _), _| to.contains(output));
        }
    }

    /// What a workspace that shows its desktop is `output`'s by: `output`, or none with
    /// `workspaces-per-output` off, where a workspace is every output's.
    pub(super) fn desktop_output(&self, output: Option<OutputId>) -> Option<OutputId> {
        output.filter(|_| self.config.workspaces_per_output)
    }

    /// Each output, as `desktop_output` has it, or none while policy has no output.
    pub(super) fn desktop_outputs(&self) -> Vec<Option<OutputId>> {
        let none = self.outputs.is_empty().then_some(None);
        let outputs = self.outputs.iter().map(|output| Some(output.id));
        let mut outputs: Vec<_> = (outputs.chain(none))
            .map(|output| self.desktop_output(output))
            .collect();
        outputs.dedup();
        outputs
    }

    /// The Main output of `id`'s frame now, if it's on one.
    fn main_output_now(&self, id: WindowId) -> Option<OutputId> {
        self.output_of(self.windows[&id].frame()).map(|o| o.id)
    }

    /// Notes each window's Main output, with no window following it.
    pub(super) fn note_main_outputs(&mut self) {
        let ids: Vec<WindowId> = self.windows.keys().copied().collect();
        for id in ids {
            let output = self.main_output_now(id);
            self.windows.get_mut(&id).unwrap().main_output = output;
        }
    }

    /// A window whose Main output changed since `apply` last looked, because most of it
    /// has crossed to another output, takes the active workspace of the output it's on now
    /// if it was showing on the one it left, so that it stays in view (230). Its family
    /// goes with it, so it's a root's crossing that counts: a transient that crosses alone
    /// stays on its family's workspace, and shows with its root (223). A sticky window is
    /// on every workspace of the output it's on now. Spanning workspaces have one active
    /// workspace, so there it's always the same.
    pub(super) fn follow_main_outputs(&mut self) {
        let ids: Vec<WindowId> = self.windows.keys().copied().collect();
        for id in ids {
            let now = self.main_output_now(id);
            let managed = &self.windows[&id];
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

    /// The active workspace of the output `id` shows by.
    pub(super) fn active_workspace_of(&self, id: WindowId) -> usize {
        self.active_workspaces.of(self.output_showing(id)).active
    }

    /// The output whose active workspace says whether `id` shows: the Main output of its
    /// transient family's root, wherever `id` itself has been dragged, so that a dialog
    /// shows and hides with its root (ADR 0032). That's the primary output for a root on
    /// none.
    pub fn output_showing(&self, id: WindowId) -> Option<OutputId> {
        // A window policy doesn't manage has no root, and takes the primary output.
        let root = self.windows.contains_key(&id).then(|| self.root(id));
        self.main_output_of(root.unwrap_or(id))
    }

    /// `id`'s own Main output, the output its frame is most on, or the primary output for a
    /// window on none. It's where the user sees `id`, so a seat's Active output, an
    /// output's default focus and the Workspace slide go by it.
    pub fn main_output_of(&self, id: WindowId) -> Option<OutputId> {
        self.windows
            .get(&id)
            .and_then(|managed| self.output_of(managed.frame()))
            .or(self.primary_output())
            .map(|output| output.id)
    }

    /// The output a switch made for `id` switches, to bring it into view or to go with it:
    /// the one it shows by. None is every output, as with `workspaces-per-output` off.
    pub(super) fn output_switched_for(&self, id: WindowId) -> Option<OutputId> {
        self.output_showing(id)
            .filter(|_| self.config.workspaces_per_output)
    }

    /// Whether a managed window is on the workspace the output it shows by has active:
    /// it's sticky, or that's its workspace.
    pub(super) fn is_on_shown_workspace(&self, id: WindowId) -> bool {
        self.is_on_workspace(id, self.active_workspace_of(id))
    }

    /// Every output's active workspace, as the one that shows its desktop for itself: an
    /// output's own with `workspaces-per-output` on, and else the one of that number. With
    /// no output it's the one that stands for them.
    pub(super) fn shown_desktops(&self) -> BTreeSet<DesktopOf> {
        let outputs = self.outputs.iter().map(|output| Some(output.id));
        let none = self.outputs.is_empty().then_some(None);
        (outputs.chain(none))
            .map(|output| {
                let workspace = self.active_workspaces.of(output).active;
                (self.desktop_output(output), workspace)
            })
            .collect()
    }
}
