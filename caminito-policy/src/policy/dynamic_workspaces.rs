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
    fn keeps_workspace(&self, id: WindowId) -> bool {
        let managed = &self.windows[&id];
        !managed.sticky && !matches!(managed.window_type, WindowType::Desktop | WindowType::Dock)
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

        let mut showing_desktop = std::mem::take(&mut self.showing_desktop);
        for (to, &from) in occupied.iter().enumerate() {
            if let Some(focus) = showing_desktop.remove(&from) {
                self.showing_desktop.insert(to, focus);
            }
        }
        // The empty workspace after the others keeps its desktop shown, if it stays.
        let spare = occupied.len();
        if spare < count
            && let Some(focus) = showing_desktop.remove(&spare)
        {
            self.showing_desktop.insert(spare, focus);
        }
        // A workspace with windows takes its place in order. What's left on a removed
        // workspace is on every workspace, and goes to the first, Marco's `safe_target` when
        // there are windows, as it is when there are none. The stacking order is one across
        // workspaces, and stays as it is.
        for managed in self.windows.values_mut() {
            let from = managed.workspace;
            managed.workspace = match occupied.iter().position(|o| *o == from) {
                Some(to) => to,
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
        let from = self.primary_shown().active;
        // Worked out on a copy, so that the switcher closes on the workspaces it was open
        // over, as before the count changed.
        let mut active_workspaces = self.active_workspaces.clone();
        let mut removed = false;
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
                None => {
                    shown.active = 0;
                    removed = true;
                }
            }
        }
        if removed {
            // As a switch does, with the default focus (`safely_remove_workspaces`,
            // screen.c:1427-1428), and the open switcher closed (ruling 9 of 108). The sound
            // is one for every output's switch, from the primary output's active workspace,
            // which stands in for the Active output's (231).
            self.close_switcher(switcher::Close::Dropped);
            self.note_switch(grid, from, 0);
        }
        self.active_workspaces = active_workspaces;
        if removed {
            self.focus.clear();
            if let Some(id) = self.default_focus() {
                for seat in &self.seats {
                    self.focus.insert(*seat, id);
                }
            }
        }
        self.forget_removed_previous();
        self.name_workspaces();
    }
}
