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
        if !self.dynamic_workspaces {
            return;
        }
        // The workspaces with windows, in order. Marco fills each empty workspace with the
        // windows of the next one that has any, from the first, so they close up in order.
        let occupied: Vec<usize> = (0..self.stacks.len())
            .filter(|workspace| {
                self.stacks[*workspace]
                    .iter()
                    .any(|id| self.keeps_workspace(*id))
            })
            .collect();
        let count = (occupied.len() + 1).min(MAX_WORKSPACES);
        let closed_up = occupied.iter().enumerate().all(|(to, from)| to == *from);
        if closed_up && count == self.stacks.len() {
            return;
        }

        let mut stacks = std::mem::take(&mut self.stacks);
        let mut showing_desktop = std::mem::take(&mut self.showing_desktop);
        for (to, &from) in occupied.iter().enumerate() {
            if from != to {
                let moved = std::mem::take(&mut stacks[from]);
                stacks[to].extend(moved);
            }
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
        // What's left on a removed workspace is on every workspace, and goes to the first,
        // Marco's `safe_target` when there are windows, as it is when there are none.
        if stacks.len() > count {
            let left: Vec<WindowId> = stacks.drain(count..).flatten().collect();
            stacks[0].extend(left);
        }
        stacks.resize(count, Vec::new());
        for (workspace, stack) in stacks.iter().enumerate() {
            for id in stack {
                self.windows.get_mut(id).unwrap().workspace = workspace;
            }
        }
        self.stacks = stacks;

        // The active workspace follows its windows, and the focus with them. Marco keeps the
        // index, which shows the next workspace's windows instead. An empty one stays if it
        // can, as in Marco, or the first is activated, Marco's `safe_target`.
        self.active = match occupied.iter().position(|from| *from == self.active) {
            Some(to) => to,
            None if self.active < count => self.active,
            None => {
                // Marco activates it as a switch does, with the default focus
                // (`safely_remove_workspaces`, screen.c:1420-1421).
                self.focus.clear();
                self.active = 0;
                if let Some(id) = self.default_focus() {
                    for seat in &self.seats {
                        self.focus.insert(*seat, id);
                    }
                }
                0
            }
        };
        self.name_workspaces();
    }
}
