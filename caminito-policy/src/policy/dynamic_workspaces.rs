//! `dynamic-workspaces` (151): with Marco master's key on, policy keeps one empty workspace
//! after those with windows, as Marco's `dynamic_workspaces_idle_callback` does
//! (screen.c:1228-1358 at Marco `8522ed1`): it adds one when none is empty, closes up an
//! emptied workspace by moving the windows of those after it down one, and removes the
//! empty ones past the last.

use super::*;

impl<W: PolicyWindow> Policy<W> {
    /// Whether a window keeps its workspace from counting as empty: one that isn't on every
    /// workspace, as Marco's `workspace_has_non_sticky_windows` has it (screen.c:1214-1226).
    /// The desktop and docks are always on every workspace in Marco (window.c:6793-6795).
    /// A window on a Fullscreen workspace, with its family, doesn't keep its Home
    /// workspace (ADR 0057).
    fn keeps_workspace(&self, id: WindowId) -> bool {
        let managed = &self.windows[&id];
        !managed.sticky
            && !matches!(managed.window_type, WindowType::Desktop | WindowType::Dock)
            && self.fullscreen_workspace_of(id).is_none()
    }

    /// Adds and removes workspaces for the windows they have, while `dynamic-workspaces` is
    /// on.
    pub(super) fn update_dynamic_workspaces(&mut self) {
        if !self.config.dynamic_workspaces {
            return;
        }
        // The workspaces with windows, in order. Marco fills each empty workspace with the
        // windows of the next one that has any, from the first, so they close up in order.
        let occupied: Vec<usize> = (0..self.workspace_count)
            .filter(|workspace| {
                self.windows.iter().any(|(id, managed)| {
                    managed.workspace == *workspace && self.keeps_workspace(*id)
                })
            })
            .collect();
        let count = (occupied.len() + 1).min(MAX_WORKSPACES);
        let closed_up = occupied.iter().enumerate().all(|(to, from)| to == *from);
        if closed_up && count == self.workspace_count {
            return;
        }

        // The empty workspace after the others keeps its desktop shown, if it stays. The
        // numbers are one set for every output, so each output's workspaces move alike.
        let spare = occupied.len();
        for ((output, from), shown) in std::mem::take(&mut self.showing_desktop) {
            let to = occupied.iter().position(|o| *o == from);
            if let Some(to) = to.or((from == spare && spare < count).then_some(spare)) {
                self.showing_desktop.insert((output, to), shown);
            }
        }
        // A workspace with windows takes its place in order. What's left on a removed
        // workspace is on every workspace, and goes to the first, Marco's `safe_target` when
        // there are windows, as it is when there are none. The stacking order is one across
        // workspaces, and stays as it is. A Fullscreen workspace's family whose Home
        // workspace has gone takes the last, the empty one (ADR 0057).
        let on_fullscreen: BTreeSet<WindowId> = (self.windows.keys())
            .filter(|id| self.fullscreen_workspace_of(**id).is_some())
            .copied()
            .collect();
        for (id, managed) in self.windows.iter_mut() {
            let from = managed.workspace;
            managed.workspace = match occupied.iter().position(|o| *o == from) {
                Some(to) => to,
                None if on_fullscreen.contains(id) => count - 1,
                None if from < count => from,
                None => 0,
            };
        }
        // The grid as it was, for the switch away from a removed active workspace (170).
        let grid = self.workspace_grid();
        self.workspace_count = count;

        // Each output's previous workspace follows its windows too (150), or is kept as the
        // active one is, and forgotten if it's gone. Each output's active workspace follows
        // its windows, and the focus with them. Marco keeps the index, which shows the next
        // workspace's windows instead. An empty one stays if it can, as in Marco, or the
        // first is activated, Marco's `safe_target`.
        let from = self.active_shown(self.last_seat).active;
        // Whether a workspace is removed with no windows to follow down.
        let goes = |workspace: usize| !occupied.contains(&workspace) && workspace >= count;
        // Whether an output showed one. What stands for the outputs counts only while
        // policy has none: no output shows it. One that shows a Fullscreen workspace still
        // does after, with its window's Home workspace, which `apply` gives it next.
        let leaves =
            |shown: active_workspaces::Shown| shown.fullscreen.is_none() && goes(shown.active);
        let removed = (self.active_workspaces.each()).any(leaves);
        // Worked out on a copy, so that the switcher closes on the workspaces it was open
        // over, as before the count changed.
        let mut active_workspaces = self.active_workspaces.clone();
        for shown in active_workspaces.each_mut() {
            if let Some(to) = shown
                .previous
                .and_then(|previous| occupied.iter().position(|from| *from == previous))
            {
                shown.previous = Some(to);
            }
            match occupied.iter().position(|from| *from == shown.active) {
                Some(to) => shown.active = to,
                None if shown.active < count => {}
                None => shown.active = 0,
            }
        }
        if removed {
            // As a switch does, with the default focus (`safely_remove_workspaces`,
            // screen.c:1427-1428), and the open switcher closed (ruling 9 of 108). The sound
            // is one for every output's switch, from the Active output's active workspace.
            self.close_switcher(switcher::Close::Dropped);
            self.note_switch(grid, from, 0);
            // And each output that showed a removed workspace slides to the first (236),
            // the way between the two cells of the grid as it was. One whose active
            // workspace followed its windows down shows what it did, and has no switch.
            for output in &self.outputs {
                let shown = self.active_workspaces.of(Some(output.id));
                if !leaves(shown) {
                    continue;
                }
                let from = shown.active;
                self.output_switches.push(OutputSwitch {
                    output: output.id,
                    from,
                    to: 0,
                    from_fullscreen: None,
                    to_fullscreen: None,
                    cells: grid.cells_between(from, 0),
                });
            }
        }
        // The seats that take the default focus: every one with Spanning workspaces, and
        // else those whose Active output showed a removed workspace. The other outputs
        // show what they did, so their seats keep their focus (225).
        let spanning = !self.config.workspaces_per_output;
        let refocused: Vec<SeatId> = (self.seats.iter().copied())
            // A seat on a Fullscreen workspace keeps its focus there.
            .filter(|seat| {
                let shown = self.active_shown(*seat);
                removed && ((spanning && shown.fullscreen.is_none()) || leaves(shown))
            })
            .collect();
        self.active_workspaces = active_workspaces;
        for seat in refocused {
            match self.default_focus_for(seat) {
                Some(id) => self.focus.insert(seat, id),
                None => self.focus.remove(&seat),
            };
        }
        self.forget_removed_previous();
        self.name_workspaces();
    }
}
