//! Keybinding actions, named after Marco's `org.mate.Marco.*` keybinding keys.

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
    /// `switch-windows`, without the popup until M8: it activates the most recently used
    /// window other than the focused one.
    SwitchWindows,
}
