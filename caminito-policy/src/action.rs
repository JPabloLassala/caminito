//! Keybinding actions, named after Marco's `org.mate.Marco.*` keybinding keys.

use crate::{Direction, SwitchList, Tile};

/// An action a keybinding triggers: those with a behaviour in policy. The window actions act
/// on the seat's focused window, and do nothing without one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// `switch-to-workspace-left`
    SwitchToWorkspaceLeft,
    /// `switch-to-workspace-right`
    SwitchToWorkspaceRight,
    /// `switch-to-workspace-up`
    SwitchToWorkspaceUp,
    /// `switch-to-workspace-down`
    SwitchToWorkspaceDown,
    /// `switch-to-workspace-prev`: back to the workspace active before this one (150).
    SwitchToWorkspacePrev,
    /// `switch-to-workspace-N`, numbered from 0 here.
    SwitchToWorkspace(usize),
    /// `move-to-workspace-left`
    MoveToWorkspaceLeft,
    /// `move-to-workspace-right`
    MoveToWorkspaceRight,
    /// `move-to-workspace-N`, numbered from 0 here.
    MoveToWorkspace(usize),
    /// `move-to-workspace-up`: the focused window to the workspace above, which becomes
    /// the active one, the window keeping the focus.
    MoveToWorkspaceUp,
    /// `move-to-workspace-down`, as `move-to-workspace-up`.
    MoveToWorkspaceDown,
    /// `close`
    Close,
    /// `toggle-maximized`
    ToggleMaximized,
    /// `toggle-above`: keeps the focused window above the others, or stops (155).
    ToggleAbove,
    /// `toggle-on-all-workspaces`: shows the focused window on every workspace, or only on
    /// the active one (155).
    ToggleOnAllWorkspaces,
    /// `maximize`
    Maximize,
    /// `unmaximize`
    Unmaximize,
    /// `maximize-horizontally`, which toggles.
    ToggleMaximizedHorizontally,
    /// `maximize-vertically`, which toggles.
    ToggleMaximizedVertically,
    /// `toggle-fullscreen`
    ToggleFullscreen,
    /// `toggle-shaded`
    ToggleShaded,
    /// `minimize`
    Minimize,
    /// `raise`
    Raise,
    /// `lower`
    Lower,
    /// `raise-or-lower`
    RaiseOrLower,
    /// `tile-to-side-e` and `-w`, and `tile-to-corner-ne`, `-nw`, `-se` and `-sw` (159):
    /// tiles the focused window there, cycles its size, or untiles it.
    Tile(Tile),
    /// `move-to-corner-*`, `move-to-side-*` and `move-to-center` (160): moves the focused
    /// window there in the work area of the output it's most on, without resizing it.
    MoveTo(Anchor),
    /// `move-to-monitor-n`, `-s`, `-e` and `-w` (160): moves the focused window to the
    /// output beside its own that way.
    MoveToMonitor(Direction),
    /// A `switch-*` action (108): opens the Window switcher over `list`, or steps the one
    /// that is open, towards the end of the list or `backward`.
    Switch { list: SwitchList, backward: bool },
    /// A `cycle-*` action (163): as its `switch-*` action, with no popup, and the selected
    /// window raised at once.
    Cycle { list: SwitchList, backward: bool },
    /// `show-desktop`
    ShowDesktop,
    /// `begin-move`: a keyboard move of the focused window (156).
    BeginMove,
    /// `begin-resize`: a keyboard resize of the focused window (156).
    BeginResize,
}

/// Where in a work area `Action::MoveTo` puts a window's Frame (160): against a corner,
/// against a side, which moves it along one axis alone, or in the middle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    NorthWest,
    North,
    NorthEast,
    West,
    Centre,
    East,
    SouthWest,
    South,
    SouthEast,
}
