//! Keybinding actions, named after Marco's `org.mate.Marco.*` keybinding keys.

use crate::SwitchList;

/// An action a keybinding triggers. Only the variants M1 binds exist so far; M8 adds the rest.
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
    /// A `switch-*` action (108): opens the Window switcher over `list`, or steps the one
    /// that is open, towards the end of the list or `backward`.
    Switch { list: SwitchList, backward: bool },
    /// `show-desktop`
    ShowDesktop,
}
