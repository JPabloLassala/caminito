//! The Window switcher's list and, to come, its state: what the `switch-*` actions open and
//! step through (108, ADR 0016).

use crate::{PolicyWindow, SeatId, WindowId, WindowType};

use super::Policy;

/// The windows a `switch-*` action lists, as Marco's `MetaTabList` has them (108).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchList {
    /// `switch-windows`: the active workspace's windows.
    Windows,
    /// `switch-windows-all`: every workspace's.
    WindowsAll,
    /// `switch-group`: the focused application's, on the active workspace.
    Group,
    /// `switch-panels`: docks and the desktop.
    Panels,
}

impl<W: PolicyWindow> Policy<W> {
    /// The windows `list` shows `seat`, in the Window switcher's order.
    pub fn switch_list(&self, seat: SeatId, list: SwitchList) -> Vec<WindowId> {
        let group = match list {
            SwitchList::Group => match self.focus(seat) {
                Some(focused) => Some(self.root(focused)),
                None => return Vec::new(),
            },
            _ => None,
        };

        // Every window in the fallback order: stacking, from the top, the active
        // workspace's stack first and then the others from the first.
        let stacks = std::iter::once(self.active)
            .chain((0..self.stacks.len()).filter(|w| *w != self.active));
        let in_stacking_order = stacks.flat_map(|w| self.stacks[w].iter().rev().copied());

        let mut entries: Vec<_> = in_stacking_order
            .filter(|id| self.is_switch_candidate(*id, list, group))
            .map(|base| (base, self.modal_transient(base)))
            .collect();

        // Most recently used first, by the better place of the window and its dialog. A window
        // that never had the focus is in neither, and keeps its stacking order after those that
        // did. Then the minimised go last. The sort is stable, so ties keep the stacking order.
        let place = |id: WindowId| self.mru.iter().position(|w| *w == id);
        entries.sort_by_key(|&(base, listed)| {
            let used = place(base).into_iter().chain(place(listed)).min();
            (self.windows[&base].minimized, used.unwrap_or(usize::MAX))
        });

        let mut listed = Vec::new();
        for (_, window) in entries {
            if !listed.contains(&window) {
                listed.push(window);
            }
        }
        listed
    }

    /// Whether `id` is one of the windows `list` starts from, before the modal dialogs stand in
    /// for their parents. `group` is the root of the focused window, for `Group`.
    fn is_switch_candidate(&self, id: WindowId, list: SwitchList, group: Option<WindowId>) -> bool {
        let managed = &self.windows[&id];
        let on_workspace = managed.workspace == self.active;
        let panel = matches!(managed.window_type, WindowType::Dock | WindowType::Desktop);
        match list {
            SwitchList::Panels => on_workspace && panel,
            SwitchList::Windows | SwitchList::WindowsAll | SwitchList::Group => {
                let listable = !panel && !managed.window.skips_taskbar();
                let in_scope = match list {
                    SwitchList::WindowsAll => true,
                    SwitchList::Group => {
                        on_workspace && group.is_some_and(|g| self.same_app(g, id))
                    }
                    _ => on_workspace,
                };
                listable && in_scope
            }
        }
    }

    /// Whether `a` and `b` belong to one application (ruling 10 of 108): the same pid when both
    /// have one, else the same app id when both have one, else only when they are one window.
    fn same_app(&self, a: WindowId, b: WindowId) -> bool {
        let (a_window, b_window) = (&self.windows[&a].window, &self.windows[&b].window);
        match (
            (a_window.pid(), b_window.pid()),
            (a_window.app_id(), b_window.app_id()),
        ) {
            ((Some(a), Some(b)), _) => a == b,
            (_, (Some(a), Some(b))) => a == b,
            _ => a == b,
        }
    }
}
