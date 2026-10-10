//! Keybinding actions, named after Marco's `org.mate.Marco.*` keybinding keys.

use crate::SwitchList;

/// An action a keybinding triggers: those with a behaviour in policy. The window actions act
/// on the seat's focused window, and do nothing without one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// `switch-to-workspace-left`
    SwitchToWorkspaceLeft,
    /// `switch-to-workspace-right`
    SwitchToWorkspaceRight,
    /// `switch-to-workspace-N`, numbered from 0 here.
    SwitchToWorkspace(usize),
    /// `move-to-workspace-left`
    MoveToWorkspaceLeft,
    /// `move-to-workspace-right`
    MoveToWorkspaceRight,
    /// `move-to-workspace-N`, numbered from 0 here.
    MoveToWorkspace(usize),
    /// `close`
    Close,
    /// `toggle-maximized`
    ToggleMaximized,
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
    /// A `switch-*` action (108): opens the Window switcher over `list`, or steps the one
    /// that is open, towards the end of the list or `backward`.
    Switch { list: SwitchList, backward: bool },
    /// `show-desktop`
    ShowDesktop,
}
