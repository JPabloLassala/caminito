//! A seat's Interaction (187): what it's in the middle of, which Marco keeps in one slot,
//! `display->grab_op`. Policy's state for each stays where it is, and here is the one
//! answer the rest asks, and the one rule: a seat has one Interaction at most, and one
//! opens only when the seat has none, as `meta_display_begin_grab_op` begins no grab over
//! another (display.c:3583-3590).
//!
//! A move or resize is its own seat's. The Window switcher, the Window menu and the
//! Workspace popup are one each, as Marco's grab is the display's, and every seat's: none
//! of the three opens while any seat has an Interaction.
//!
//! Three things open in place of themselves: the pointer's move or resize of a seat begins
//! in place of the seat's own, the Window menu opens in place of the one that's open, and
//! a `switch-*` or `cycle-*` action steps the open Window switcher.

use crate::{PolicyWindow, SeatId};

use super::Policy;

/// What a seat is in the middle of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interaction {
    /// A move or resize, by the pointer or the keys, a manual placement among them (154).
    MoveOrResize,
    /// The Window switcher (108), or a `cycle-*` action under way (163).
    WindowSwitcher,
    /// The Window menu (161).
    WindowMenu,
    /// The Workspace popup (152).
    WorkspacePopup,
}

/// The open Workspace popup (152). The compositor has the rest of it: the modifiers that
/// hold it open, its output and its drawing. The selected workspace is the active one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkspacePopup {
    /// Whether a move opened it, so that each further direction moves the focused window
    /// too, as Marco's `META_GRAB_OP_KEYBOARD_WORKSPACE_MOVING` does.
    pub moves: bool,
}

impl<W: PolicyWindow> Policy<W> {
    /// The Interaction `seat` has, if it has one.
    pub fn interaction(&self, seat: SeatId) -> Option<Interaction> {
        if self.grabs.contains_key(&seat) {
            Some(Interaction::MoveOrResize)
        } else {
            self.shared_interaction()
        }
    }

    /// The Interaction every seat has, if one of the three that aren't a seat's own is
    /// open.
    pub(super) fn shared_interaction(&self) -> Option<Interaction> {
        if self.switcher.is_some() {
            Some(Interaction::WindowSwitcher)
        } else if self.window_menu.is_some() {
            Some(Interaction::WindowMenu)
        } else if self.workspace_popup.is_some() {
            Some(Interaction::WorkspacePopup)
        } else {
            None
        }
    }

    /// Whether any seat has an Interaction, so that none that's every seat's can open.
    pub(super) fn a_seat_has_an_interaction(&self) -> bool {
        !self.grabs.is_empty() || self.shared_interaction().is_some()
    }

    /// The Workspace popup, while it's open.
    pub fn workspace_popup(&self) -> Option<&WorkspacePopup> {
        self.workspace_popup.as_ref()
    }

    /// Opens the Workspace popup, which a move opened if `moves`. Returns false if it
    /// doesn't open: a seat has an Interaction, an open popup among them, which stays as
    /// it is. The compositor asks once the binding's action has run, since only it knows
    /// that a binding with no modifier shows none, and says when the popup closes.
    pub fn open_workspace_popup(&mut self, moves: bool) -> bool {
        if self.a_seat_has_an_interaction() {
            return false;
        }
        self.workspace_popup = Some(WorkspacePopup { moves });
        true
    }

    /// Closes the Workspace popup, if it's open.
    pub fn close_workspace_popup(&mut self) {
        self.workspace_popup = None;
    }
}
