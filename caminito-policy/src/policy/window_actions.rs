//! The keybinding actions on the focused window (149): its state, its stacking and its
//! workspace, each as Marco's handler in `core/keybindings.c` (1.29.0) does it, and only
//! where Marco gives the window the function (`recalc_window_features`).

use crate::{Action, PolicyWindow, SizeHints, WindowId, WindowType};

use super::Policy;

/// The functions Marco gives a window that the actions need, from its type and size hints
/// (`recalc_window_features`, window.c:6670-6830). An SSD window's frame shows the same
/// through its buttons (`decoration::style`).
struct Functions {
    minimize: bool,
    maximize: bool,
    /// Resizing, which maximising one way needs.
    resize: bool,
    fullscreen: bool,
}

impl<W: PolicyWindow> Policy<W> {
    /// Runs a window action on `id`, a seat's focused window. The moves to the workspace
    /// beside are `move_to_workspace_towards`'s (150).
    pub(super) fn window_action(&mut self, id: WindowId, action: Action) {
        let functions = self.functions(id);
        let managed = &self.windows[&id];
        let (horizontally, vertically) =
            (managed.maximized_horizontally, managed.maximized_vertically);
        match action {
            Action::Close => self.close(id),
            Action::ToggleMaximized if horizontally && vertically => {
                self.set_maximized(id, false);
            }
            Action::ToggleMaximized | Action::Maximize if functions.maximize => {
                self.set_maximized(id, true);
            }
            // `handle_unmaximize` undoes either way alone too.
            Action::Unmaximize if horizontally || vertically => self.set_maximized(id, false),
            Action::ToggleMaximizedHorizontally if functions.resize => {
                self.set_maximized_horizontally(id, !horizontally);
            }
            Action::ToggleMaximizedVertically if functions.resize => {
                self.set_maximized_vertically(id, !vertically);
            }
            Action::ToggleFullscreen if managed.fullscreen => self.set_fullscreen(id, false, None),
            Action::ToggleFullscreen if functions.fullscreen => self.set_fullscreen(id, true, None),
            // `set_shaded` shades only what has a shade function.
            Action::ToggleShaded => self.set_shaded(id, !managed.shaded),
            Action::Minimize if functions.minimize => self.minimize(id),
            Action::Raise => self.raise(id),
            Action::Lower => self.lower_in_layer(id),
            Action::RaiseOrLower => self.raise_or_lower(id),
            Action::MoveToWorkspace(workspace) => self.move_to_numbered_workspace(id, workspace),
            _ => {}
        }
    }

    fn functions(&self, id: WindowId) -> Functions {
        let managed = &self.windows[&id];
        let SizeHints { min, max } = managed.hints;
        // A zero maximum is unbounded.
        let grows = |min: i32, max: i32| max == 0 || min < max;
        let fixed = matches!(
            managed.window_type,
            WindowType::Desktop | WindowType::Dock | WindowType::Splashscreen
        );
        let resizable = !fixed && (grows(min.w, max.w) || grows(min.h, max.h));
        let normal_or_dialog =
            matches!(managed.window_type, WindowType::Normal | WindowType::Dialog);
        // Marco also allows fullscreen to a fixed-size window whose minimum size is the
        // screen's, which no window Caminito manages asks for.
        Functions {
            minimize: normal_or_dialog,
            maximize: normal_or_dialog && resizable && !managed.fullscreen,
            resize: resizable && !managed.fullscreen,
            fullscreen: normal_or_dialog && resizable,
        }
    }

    /// Lowers a window to the bottom of its layer, as Marco's `meta_window_lower` does: the
    /// focus stays where it is, unlike the titlebar's `lower`.
    pub(super) fn lower_in_layer(&mut self, id: WindowId) {
        let stack = &mut self.stacks[self.windows[&id].workspace];
        stack.retain(|w| *w != id);
        stack.insert(0, id);
    }

    /// Raises a window if a showing window in its own layer above it overlaps its frame,
    /// and lowers it otherwise, as `handle_raise_or_lower` does (keybindings.c:3433-3476).
    /// Marco lowers the window at the top of the stack first, which nothing overlaps from
    /// above either.
    fn raise_or_lower(&mut self, id: WindowId) {
        let frame = self.windows[&id].frame();
        let layer = self.layer(id);
        let stack = &self.stacks[self.windows[&id].workspace];
        let overlapped = stack.iter().skip_while(|w| **w != id).skip(1).any(|w| {
            self.layer(*w) == layer
                && self.is_showing(*w)
                && self.windows[w].frame().overlaps(frame)
        });
        if overlapped {
            self.raise_family(id);
        } else {
            self.lower_in_layer(id);
        }
    }

    /// `move-to-workspace-N`: the window goes, and the active workspace and the focus stay
    /// (`handle_move_to_workspace`, keybindings.c:3393-3430), so the seat focuses the
    /// default window. A number past the count does nothing. The desktop and docks are
    /// always on every workspace in Marco (`always_sticky`), so they don't move.
    fn move_to_numbered_workspace(&mut self, id: WindowId, workspace: usize) {
        if workspace < self.stacks.len() && !self.is_desktop_or_dock(id) {
            self.move_to_workspace(id, workspace);
        }
    }
}
