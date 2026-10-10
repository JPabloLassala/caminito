//! The Window switcher: the list the `switch-*` actions open and step through, and its state
//! while it is open (108, ADR 0016).

use std::time::Duration;

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

/// The open Window switcher (108): what it lists, and which entry is selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Switcher {
    /// The seat that opened it, which the selected window is activated for.
    pub seat: SeatId,
    /// The action's list, which `windows` was made from.
    pub list: SwitchList,
    /// The windows listed, in order. Never empty.
    pub windows: Vec<WindowId>,
    /// The selected entry's place in `windows`.
    pub selected: usize,
}

/// The open Window switcher, with what policy keeps beside it.
pub(super) struct Open {
    switcher: Switcher,
    /// Each workspace's stacking when the switcher opened, to put back what raising the
    /// selected window changed.
    stacks: Vec<Vec<WindowId>>,
    /// Whether the switcher has raised a window, so the stacking is put back on closing.
    raised: bool,
    /// The windows mapped while it's open, and when, in the order they mapped. They get the
    /// focus they would have had when it closes.
    mapped: Vec<(WindowId, Duration)>,
}

/// How the Window switcher closes.
#[derive(Clone, Copy)]
pub(super) enum Close {
    /// The selected window is activated.
    Commit(Duration),
    /// Nothing is activated.
    Cancel(Duration),
    /// Nothing is activated, as policy's own decision (the workspace changed, or no listed
    /// window is left) and with no time of the user's.
    Dropped,
}

impl<W: PolicyWindow> Policy<W> {
    /// The Window switcher, while it's open.
    pub fn switcher(&self) -> Option<&Switcher> {
        self.switcher.as_ref().map(|open| &open.switcher)
    }

    /// Selects the next entry, or the one before with `backward`, wrapping at the ends.
    pub fn switcher_step(&mut self, backward: bool) {
        let Some(open) = &mut self.switcher else {
            return;
        };
        let switcher = &mut open.switcher;
        let len = switcher.windows.len();
        switcher.selected = if backward {
            (switcher.selected + len - 1) % len
        } else {
            (switcher.selected + 1) % len
        };
        self.raise_selected();
        self.apply();
    }

    /// Selects the entry at `index`. Does nothing if there's none.
    pub fn switcher_select(&mut self, index: usize) {
        let Some(open) = &mut self.switcher else {
            return;
        };
        if index < open.switcher.windows.len() {
            open.switcher.selected = index;
            self.raise_selected();
        }
        self.apply();
    }

    /// Closes the switcher and activates the selected window, as `activate` does.
    pub fn switcher_commit(&mut self, now: Duration) {
        self.close_switcher(Close::Commit(now));
        self.apply();
    }

    /// Closes the switcher and leaves focus and stacking as they were when it opened.
    pub fn switcher_cancel(&mut self, now: Duration) {
        self.close_switcher(Close::Cancel(now));
        self.apply();
    }

    /// A `switch-*` action: steps the open switcher, or opens one over `list`. Nothing opens
    /// during a move or resize, nor with no window to list, and one window alone is
    /// activated with no switcher (108).
    pub(super) fn switch(&mut self, seat: SeatId, list: SwitchList, backward: bool, now: Duration) {
        if self.switcher.is_some() {
            self.switcher_step(backward);
            return;
        }
        if self.grab(seat).is_some() {
            return;
        }
        let windows = self.switch_list(seat, list);
        match windows.as_slice() {
            [] => {}
            [only] => {
                self.pointer_focus.keyboard_focused();
                self.activate(seat, *only, now);
            }
            _ => {
                // Ruling 7: the second entry when the first is the focused window, which
                // the switcher is for leaving; backward, the last.
                let selected = if backward {
                    windows.len() - 1
                } else if self.focus(seat) == Some(windows[0]) {
                    1
                } else {
                    0
                };
                self.switcher = Some(Open {
                    switcher: Switcher {
                        seat,
                        list,
                        windows,
                        selected,
                    },
                    stacks: self.stacks.clone(),
                    raised: false,
                    mapped: Vec::new(),
                });
                self.raise_selected();
            }
        }
    }

    /// Closes the switcher, if it's open. The caller applies.
    pub(super) fn close_switcher(&mut self, how: Close) {
        let Some(open) = self.switcher.take() else {
            return;
        };
        if open.raised {
            self.restore_stacking(&open.stacks);
        }
        let time = match how {
            Close::Commit(now) => {
                self.pointer_focus.keyboard_focused();
                let switcher = &open.switcher;
                self.activate(switcher.seat, switcher.windows[switcher.selected], now);
                Some(now)
            }
            Close::Cancel(now) => Some(now),
            Close::Dropped => None,
        };
        // After the activation, in the order they mapped. Their own time is stale once the
        // activation has set the seat's focus time.
        for (id, mapped_at) in open.mapped {
            self.focus_mapped(id, time.unwrap_or(mapped_at));
        }
    }

    /// Keeps the focus where it is for a window mapped while the switcher is open.
    pub(super) fn switcher_defer_focus(&mut self, id: WindowId, now: Duration) {
        if let Some(open) = &mut self.switcher {
            open.mapped.push((id, now));
        }
    }

    /// Takes a window that went away out of the switcher's list.
    pub(super) fn switcher_window_unmapped(&mut self, id: WindowId) {
        let Some(open) = &mut self.switcher else {
            return;
        };
        open.mapped.retain(|(mapped, _)| *mapped != id);
        let switcher = &mut open.switcher;
        let Some(place) = switcher.windows.iter().position(|w| *w == id) else {
            return;
        };
        switcher.windows.remove(place);
        if switcher.windows.is_empty() {
            self.close_switcher(Close::Dropped);
            return;
        }
        if place < switcher.selected {
            // The selection stays on its window.
            switcher.selected -= 1;
        } else if switcher.selected == switcher.windows.len() {
            // The selected window was the last: the first is next.
            switcher.selected = 0;
        }
        self.raise_selected();
    }

    /// With `alt-tab-raise-windows`, raises the selected window over the stacking the
    /// switcher opened with, so that only that window is out of place.
    fn raise_selected(&mut self) {
        if !self.alt_tab_raise_windows {
            return;
        }
        let Some(mut open) = self.switcher.take() else {
            return;
        };
        self.restore_stacking(&open.stacks);
        self.raise_family(open.switcher.windows[open.switcher.selected]);
        open.raised = true;
        self.switcher = Some(open);
    }

    /// Puts each workspace's stacking back as `saved` has it. A window that's gone is left
    /// out and one that wasn't there goes on top, in the order it has now.
    fn restore_stacking(&mut self, saved: &[Vec<WindowId>]) {
        for (stack, before) in self.stacks.iter_mut().zip(saved) {
            let mut restored: Vec<WindowId> = before
                .iter()
                .copied()
                .filter(|id| stack.contains(id))
                .collect();
            restored.extend(stack.iter().copied().filter(|id| !before.contains(id)));
            *stack = restored;
        }
    }

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
