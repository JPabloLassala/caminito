//! Keybinding actions, named after Marco's `org.mate.Marco.*` keybinding keys.

use crate::SwitchList;

/// An action a keybinding triggers. Only the variants M1 binds exist so far; M8 adds the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// `switch-to-workspace-left`
    SwitchToWorkspaceLeft,
    /// `switch-to-workspace-right`
    SwitchToWorkspaceRight,
    /// `switch-to-workspace-N`, numbered from 0 here.
    SwitchToWorkspace(usize),
    /// `close`
    Close,
    /// `toggle-maximized`
    ToggleMaximized,
    /// A `switch-*` action (108): opens the Window switcher over `list`, or steps the one
    /// that is open, towards the end of the list or `backward`.
    Switch { list: SwitchList, backward: bool },
    /// `show-desktop`
    ShowDesktop,
}
