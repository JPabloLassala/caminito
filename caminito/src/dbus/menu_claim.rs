//! The menu claim (115, ADR 0018): a program with no window of its own says that the
//! dbusmenu it serves is the menubar of its process's focused toplevel. It's how the global
//! menu extension gives VS Code's window on native Wayland a global menu.
//!
//! This module owns the ties the claims make, and finds the toplevel a claim is for. A tie
//! is its caller's, and ends with it or with its toplevel. Which window's menu is
//! published is `global_menu`'s.

use std::collections::HashMap;

use caminito_policy::{PolicyWindow, WindowId};
use zbus::zvariant::OwnedObjectPath;

use crate::state::{Caminito, SEAT};

/// The menu a claim tied a toplevel to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Tie {
    /// The unique name of the connection that made the claim, which serves the menu.
    pub(crate) caller: String,
    /// The `com.canonical.dbusmenu` object.
    pub(crate) path: OwnedObjectPath,
}

/// The ties the claims made, by toplevel.
#[derive(Debug, Default)]
pub(crate) struct Claims {
    ties: HashMap<WindowId, Tie>,
}

impl Claims {
    /// The menu `window` is tied to, if any.
    pub(crate) fn menu(&self, window: WindowId) -> Option<&Tie> {
        self.ties.get(&window)
    }

    /// Every tie, in the order of the toplevels' ids.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (WindowId, &Tie)> {
        let mut ties: Vec<_> = self.ties.iter().map(|(id, tie)| (*id, tie)).collect();
        ties.sort_by_key(|(id, _)| id.0);
        ties.into_iter()
    }

    /// Ties `window` to `caller`'s menu at `path`, in place of any it was tied to.
    fn tie(&mut self, window: WindowId, caller: String, path: OwnedObjectPath) {
        self.ties.insert(window, Tie { caller, path });
    }

    /// `window` is no longer managed, and its tie ends.
    pub(crate) fn toplevel_gone(&mut self, window: WindowId) {
        self.ties.remove(&window);
    }

    /// Ends the ties of a caller that left the bus.
    pub(super) fn caller_left(&mut self, caller: &str) {
        self.ties.retain(|_, tie| tie.caller != caller);
    }

    /// Ends every tie: the global menu is no longer offered.
    pub(super) fn forget_all(&mut self) {
        self.ties.clear();
    }
}

/// The parent of the process `pid`, from `/proc/<pid>/stat`. A caller can't lie about it,
/// as it could about anything it sent (ADR 0018).
pub(crate) fn parent_process(pid: u32) -> Option<u32> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    parent_in_stat(&stat)
}

/// The parent's pid in a process's `stat` line: the fourth field. The second is the
/// command's name in brackets, which may have spaces and brackets of its own, so the
/// fields are counted from the last closing bracket.
fn parent_in_stat(stat: &str) -> Option<u32> {
    let (_, after_name) = stat.rsplit_once(')')?;
    after_name.split_whitespace().nth(1)?.parse().ok()
}

impl Caminito {
    /// `ClaimGlobalMenu` from `caller`, a connection of the process `process` if the bus
    /// knows it: ties the focused toplevel to the menu at `path`, if its client is of that
    /// process or of its parent, one step up and no further. Any other claim is dropped.
    pub(super) fn claim_global_menu(
        &mut self,
        caller: String,
        path: OwnedObjectPath,
        process: Option<u32>,
    ) {
        if !self.global_menu_offered {
            tracing::debug!("dropped the menu claim of {caller}: the global menu isn't offered");
            return;
        }
        let Some(process) = process else {
            tracing::debug!("dropped the menu claim of {caller}: the bus doesn't know its process");
            return;
        };
        let focused = self.policy.focus(SEAT);
        let client = focused
            .and_then(|id| self.policy.window(id))
            .and_then(PolicyWindow::pid);
        let Some((id, client)) = focused.zip(client) else {
            tracing::debug!("dropped the menu claim of {caller}: no toplevel is focused");
            return;
        };
        // `/proc` is left alone for a caller that's the client itself.
        let matched = if client == process {
            "the caller's"
        } else if parent_process(process) == Some(client) {
            "its parent's"
        } else {
            tracing::debug!(
                "dropped the menu claim of {caller}, of process {process}: the focused \
                 toplevel is of process {client}, neither the caller's nor its parent's"
            );
            return;
        };
        tracing::debug!(
            "tied window {} to the menu claim of {caller}, of process {process}: its \
             client's process {client} is {matched}",
            id.0
        );
        self.menu_claims.tie(id, caller, path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parent_is_the_fourth_field_of_stat() {
        let stat = "4242 (code) S 4100 4100 4100 0 -1 4194304 1 0 0 0";

        assert_eq!(parent_in_stat(stat), Some(4100));
    }

    #[test]
    fn a_commands_name_with_spaces_and_brackets_doesnt_move_the_fields() {
        let stat = "4242 (Web Content) (1) 7) R 4100 4242 4242 0 -1 4194560 1 0 0 0";

        assert_eq!(parent_in_stat(stat), Some(4100));
    }

    #[test]
    fn a_line_thats_no_stat_has_no_parent() {
        assert_eq!(parent_in_stat(""), None);
        assert_eq!(parent_in_stat("4242 (code"), None);
        assert_eq!(parent_in_stat("4242 (code) S"), None);
        assert_eq!(parent_in_stat("4242 (code) S init"), None);
    }

    #[test]
    fn the_tests_own_parent_is_read_from_proc() {
        let parent = std::os::unix::process::parent_id();

        assert_eq!(parent_process(std::process::id()), Some(parent));
    }
}
