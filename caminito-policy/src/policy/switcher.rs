//! The Window switcher: the list the `switch-*` actions open and step through, and its state
//! while it is open (108, ADR 0016). The `cycle-*` actions walk the same lists with no popup,
//! and the panels' list has the Layer surfaces that accept the keyboard (163).

use std::time::Duration;

use crate::{PolicyWindow, SeatId, UrgentPlacement, WindowId, WindowType};

use super::Policy;

/// The windows a `switch-*` action lists, as Marco's `MetaTabList` has them (108).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchList {
    /// `switch-windows`: the windows on every output's active workspace (224).
    Windows,
    /// `switch-windows-all`: every workspace's, of every output.
    WindowsAll,
    /// `switch-group`: the focused application's, on every output's active workspace.
    Group,
    /// `switch-panels`: docks and the desktop, on every output's active workspace.
    Panels,
}

/// A Layer surface that accepts the keyboard, by the name the compositor gives it (163).
/// Policy manages none: it only lists them among the panels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LayerSurfaceId(pub u64);

/// What a list has: a window, or among the panels a Layer surface (163).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SwitchEntry {
    /// A window policy manages.
    Window(WindowId),
    /// A Layer surface that accepts the keyboard, which only the panels' list has.
    Layer(LayerSurfaceId),
}

impl SwitchEntry {
    /// The window, if it is one.
    pub fn window(self) -> Option<WindowId> {
        match self {
            Self::Window(id) => Some(id),
            Self::Layer(_) => None,
        }
    }
}

impl PartialEq<WindowId> for SwitchEntry {
    fn eq(&self, other: &WindowId) -> bool {
        *self == Self::Window(*other)
    }
}

/// `alt-tab-minimized-placement`, Marco master's: where the lists have the minimised
/// windows (163).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MinimizedPlacement {
    /// Among the others, most recently used first.
    Mru,
    /// After the others.
    #[default]
    End,
    /// Nowhere.
    Hidden,
}

/// The open Window switcher (108), or a `cycle-*` action under way (163): what it lists,
/// and which entry is selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Switcher {
    /// The seat that opened it, which the selected entry is activated for.
    pub seat: SeatId,
    /// The action's list, which `entries` was made from.
    pub list: SwitchList,
    /// Whether it has a popup. A `cycle-*` action's has none, and raises the selected
    /// window at once in its place.
    pub popup: bool,
    /// What's listed, in order. Never empty.
    pub entries: Vec<SwitchEntry>,
    /// The selected entry's place in `entries`.
    pub selected: usize,
}

/// The open Window switcher, with what policy keeps beside it.
pub(super) struct Open {
    switcher: Switcher,
    /// The stacking when the switcher opened, to put back what raising the selected window
    /// changed.
    stack: Vec<WindowId>,
    /// Whether the switcher has raised a window, so the stacking is put back on closing.
    raised: bool,
    /// The minimised window a cycle shows while it's selected, as Marco's `tab_unminimized`
    /// (core/keybindings.c:2313-2326). It's minimised again when the selection leaves it.
    peeked: Option<WindowId>,
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
        let len = switcher.entries.len();
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
        if index < open.switcher.entries.len() {
            open.switcher.selected = index;
            self.raise_selected();
        }
        self.apply();
    }

    /// Closes the switcher and activates the selected window, as `activate` does. A Layer
    /// surface is the compositor's to give the keyboard: `take_chosen` names it.
    pub fn switcher_commit(&mut self, now: Duration) {
        self.close_switcher(Close::Commit(now));
        self.apply();
    }

    /// Closes the switcher and leaves focus and stacking as they were when it opened.
    pub fn switcher_cancel(&mut self, now: Duration) {
        self.close_switcher(Close::Cancel(now));
        self.apply();
    }

    /// What a commit last chose, once: the compositor takes the keyboard from a Layer
    /// surface for a window, and gives it to a Layer surface (163).
    pub fn take_chosen(&mut self) -> Option<SwitchEntry> {
        self.chosen.take()
    }

    /// The Layer surfaces that accept the keyboard, and the one that has it in place of the
    /// focused window, if one does. Policy keeps them most recently used first: by when
    /// each had the keyboard, and then in the order they came.
    pub fn set_layers(&mut self, layers: &[LayerSurfaceId], keyboard: Option<LayerSurfaceId>) {
        let mut ordered: Vec<LayerSurfaceId> = keyboard.into_iter().collect();
        let known = self.layers.iter().filter(|id| layers.contains(id));
        for id in known.chain(layers) {
            if !ordered.contains(id) {
                ordered.push(*id);
            }
        }
        if ordered == self.layers && keyboard == self.keyboard_layer {
            return;
        }
        self.layers = ordered;
        self.keyboard_layer = keyboard;
        let listed = self.switcher.iter().flat_map(|open| &open.switcher.entries);
        let gone: Vec<SwitchEntry> = listed
            .copied()
            .filter(|entry| matches!(entry, SwitchEntry::Layer(id) if !layers.contains(id)))
            .collect();
        for entry in gone {
            self.switcher_entry_gone(entry);
        }
        self.apply();
    }

    /// A `switch-*` action, or with no `popup` a `cycle-*` one: steps the open switcher, or
    /// opens one over `list`. Nothing opens while a seat has another Interaction, nor with
    /// nothing to list, and one entry alone is activated with no switcher (108).
    pub(super) fn switch(
        &mut self,
        seat: SeatId,
        list: SwitchList,
        backward: bool,
        popup: bool,
        now: Duration,
    ) {
        if self.switcher.is_some() {
            self.switcher_step(backward);
            return;
        }
        if self.a_seat_has_an_interaction() {
            return;
        }
        let entries = self.switch_list(seat, list);
        match entries.as_slice() {
            [] => {}
            [only] => {
                self.pointer_focus.keyboard_focused();
                self.choose(seat, *only, now);
            }
            _ => {
                // Ruling 7: the second entry when the first has the keyboard, which the
                // switcher is for leaving; backward, the last.
                let selected = if backward {
                    entries.len() - 1
                } else if self.has_keyboard(seat, entries[0]) {
                    1
                } else {
                    0
                };
                self.switcher = Some(Open {
                    switcher: Switcher {
                        seat,
                        list,
                        popup,
                        entries,
                        selected,
                    },
                    stack: self.stack.clone(),
                    raised: false,
                    peeked: None,
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
            self.restore_stacking(&open.stack);
        }
        // Activating it shows it again, if it's the one chosen.
        if let Some(managed) = open.peeked.and_then(|id| self.windows.get_mut(&id)) {
            managed.minimized = true;
        }
        let time = match how {
            Close::Commit(now) => {
                self.pointer_focus.keyboard_focused();
                let switcher = &open.switcher;
                self.choose(switcher.seat, switcher.entries[switcher.selected], now);
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

    /// The stacking an open switcher puts back, for a window that maps under another (165).
    pub(super) fn switcher_saved_stack(&mut self) -> Option<&mut Vec<WindowId>> {
        self.switcher.as_mut().map(|open| &mut open.stack)
    }

    /// Takes a window that went away out of the switcher's list.
    pub(super) fn switcher_window_unmapped(&mut self, id: WindowId) {
        let Some(open) = &mut self.switcher else {
            return;
        };
        open.mapped.retain(|(mapped, _)| *mapped != id);
        if open.peeked == Some(id) {
            open.peeked = None;
        }
        self.switcher_entry_gone(SwitchEntry::Window(id));
    }

    /// Takes an entry that went away out of the switcher's list.
    fn switcher_entry_gone(&mut self, entry: SwitchEntry) {
        let Some(open) = &mut self.switcher else {
            return;
        };
        let switcher = &mut open.switcher;
        let Some(place) = switcher.entries.iter().position(|e| *e == entry) else {
            return;
        };
        switcher.entries.remove(place);
        if switcher.entries.is_empty() {
            self.close_switcher(Close::Dropped);
            return;
        }
        if place < switcher.selected {
            // The selection stays on its entry.
            switcher.selected -= 1;
        } else if switcher.selected == switcher.entries.len() {
            // The selected entry was the last: the first is next.
            switcher.selected = 0;
        }
        self.raise_selected();
    }

    /// Activates what the switcher chose, and leaves its name for the compositor.
    fn choose(&mut self, seat: SeatId, entry: SwitchEntry, now: Duration) {
        if let SwitchEntry::Window(id) = entry {
            self.activate(seat, id, now);
        }
        self.chosen = Some(entry);
    }

    /// Whether `entry` is what the keys go to: the focused window, unless a Layer surface
    /// has the keyboard in its place, or that Layer surface.
    fn has_keyboard(&self, seat: SeatId, entry: SwitchEntry) -> bool {
        match entry {
            SwitchEntry::Window(id) => {
                self.keyboard_layer.is_none() && self.focus(seat) == Some(id)
            }
            SwitchEntry::Layer(id) => self.keyboard_layer == Some(id),
        }
    }

    /// Raises the selected window over the stacking the switcher opened with, so that only
    /// that window is out of place: a `switch-*` action with `alt-tab-raise-windows`, and
    /// a `cycle-*` one always, which also shows the window while it's minimised, as
    /// Marco's does (core/keybindings.c:2301-2327).
    fn raise_selected(&mut self) {
        let Some(mut open) = self.switcher.take() else {
            return;
        };
        let cycle = !open.switcher.popup;
        if cycle || self.config.alt_tab_raise_windows {
            self.restore_stacking(&open.stack);
            let selected = open.switcher.entries[open.switcher.selected].window();
            if cycle && open.peeked != selected {
                if let Some(managed) = open.peeked.take().and_then(|id| self.windows.get_mut(&id)) {
                    managed.minimized = true;
                }
                if let Some(id) = selected
                    && let Some(managed) = self.windows.get_mut(&id)
                    && managed.minimized
                {
                    managed.minimized = false;
                    open.peeked = Some(id);
                }
            }
            if let Some(id) = selected {
                self.raise_family(id);
            }
            open.raised = true;
        }
        self.switcher = Some(open);
    }

    /// Puts the stacking back as `saved` has it. A window that's gone is left out and one
    /// that wasn't there goes on top, in the order it has now.
    fn restore_stacking(&mut self, saved: &[WindowId]) {
        let stack = &self.stack;
        let mut restored: Vec<WindowId> = saved
            .iter()
            .copied()
            .filter(|id| stack.contains(id))
            .collect();
        restored.extend(stack.iter().copied().filter(|id| !saved.contains(id)));
        self.stack = restored;
    }

    /// What `list` shows `seat`, in the Window switcher's order.
    pub fn switch_list(&self, seat: SeatId, list: SwitchList) -> Vec<SwitchEntry> {
        let group = match list {
            SwitchList::Group => match self.focus(seat) {
                Some(focused) => Some(self.root(focused)),
                None => return Vec::new(),
            },
            _ => None,
        };

        // Every window in the fallback order: stacking, from the top, the windows on their
        // output's active workspace first, sticky ones among them, and then the others' by
        // their workspace from the first.
        let top_down = || self.stack.iter().rev().copied();
        let on_active = top_down().filter(|id| self.is_on_shown_workspace(*id));
        let others = (0..self.workspace_count).flat_map(|w| {
            top_down().filter(move |id| {
                !self.is_on_shown_workspace(*id) && self.windows[id].workspace == w
            })
        });
        let in_stacking_order = on_active.chain(others);

        let mut entries: Vec<_> = in_stacking_order
            .filter(|id| self.is_switch_candidate(*id, list, group))
            .map(|base| (base, self.modal_transient(base)))
            .collect();

        // Most recently used first, by the better place of the window and its dialog. A window
        // that never had the focus is in neither, and keeps its stacking order after those that
        // did. Then `alt-tab-minimized-placement` places the minimised (163), and with
        // `alt-tab-urgent-placement`'s `start` the urgent ones go first among the rest (165),
        // as Marco master's `meta_display_get_tab_list` has them (core/display.c:4801-4818).
        // The sort is stable, so ties keep the stacking order.
        let placement = self.config.alt_tab_minimized_placement;
        let minimized = |id: &WindowId| self.windows[id].minimized;
        entries.retain(|(base, _)| placement != MinimizedPlacement::Hidden || !minimized(base));
        let place = |id: WindowId| self.mru.iter().position(|w| *w == id);
        let urgent_first = self.config.urgent_placement == UrgentPlacement::Start;
        entries.sort_by_key(|&(base, listed)| {
            let used = place(base).into_iter().chain(place(listed)).min();
            let last = placement == MinimizedPlacement::End && minimized(&base);
            let urgent = urgent_first && (self.is_urgent(base) || self.is_urgent(listed));
            (last, !urgent, used.unwrap_or(usize::MAX))
        });

        let mut listed = Vec::new();
        for (_, window) in entries {
            if !listed.contains(&SwitchEntry::Window(window)) {
                listed.push(SwitchEntry::Window(window));
            }
        }
        if list == SwitchList::Panels {
            // Policy has no order between a window and a Layer surface: the Layer surfaces
            // are first when one of them has the keyboard, so that the next is one press
            // away, and else after the windows.
            let layers = self.layers.iter().copied().map(SwitchEntry::Layer);
            if self.keyboard_layer.is_some() {
                listed.splice(0..0, layers);
            } else {
                listed.extend(layers);
            }
        }
        listed
    }

    /// Whether `id` is one of the windows `list` starts from, before the modal dialogs stand in
    /// for their parents. `group` is the root of the focused window, for `Group`.
    fn is_switch_candidate(&self, id: WindowId, list: SwitchList, group: Option<WindowId>) -> bool {
        let managed = &self.windows[&id];
        let on_workspace = self.is_on_shown_workspace(id);
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
