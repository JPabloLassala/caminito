//! The Overview (241, ADR 0033): whether it's open, the output it's on and the windows it
//! lists, and what a choice does. The compositor has its layout, its drawing, its keys and
//! its clicks, and asks here for what they do.
//!
//! It's the seat's Interaction (187), and every seat's, as the Window switcher is: the
//! shortcut opens it only while no seat has one, and nothing else opens while it's open.

use std::time::Duration;

use crate::{OutputId, PolicyWindow, SeatId, WindowId};

use super::Policy;
use super::switcher::SwitchList;

/// The open Overview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Overview {
    /// The seat that opened it, which a chosen window is activated for.
    pub seat: SeatId,
    /// The output it's on, whose active workspace's windows it lists.
    pub output: OutputId,
    /// The windows listed, bottom to top. Policy keeps the list as the windows are now: one
    /// that goes leaves it.
    pub windows: Vec<WindowId>,
}

impl<W: PolicyWindow> Policy<W> {
    /// The Overview, while it's open.
    pub fn overview(&self) -> Option<&Overview> {
        self.overview.as_ref()
    }

    /// The Overview's shortcut: closes the open one, or opens one on `seat`'s Active
    /// output. Nothing opens while a seat has another Interaction, nor with no output.
    pub(super) fn toggle_overview(&mut self, seat: SeatId) {
        if self.overview.take().is_some() || self.a_seat_has_an_interaction() {
            return;
        }
        let Some(output) = self.active_output(seat) else {
            return;
        };
        self.overview = Some(Overview {
            seat,
            output,
            windows: self.overview_windows(output),
        });
    }

    /// Closes the Overview, if it's open, and changes nothing else: Esc, a click on the
    /// empty area, and the session lock.
    pub fn close_overview(&mut self) {
        self.overview = None;
        self.apply();
    }

    /// A listed window was chosen: the Overview closes and the window is activated, as
    /// `activate` does, which shows it again if it's minimised. Does nothing for a window
    /// the Overview doesn't list.
    pub fn overview_choose(&mut self, id: WindowId, now: Duration) {
        let Some(overview) = self.overview.take_if(|open| open.windows.contains(&id)) else {
            return;
        };
        // As the Window switcher's choice: the pointer, which is over some other window
        // once they're back in their places, doesn't take the focus until it moves.
        self.pointer_focus.keyboard_focused();
        self.activate(overview.seat, id, now);
    }

    /// Brings the open Overview's list up to date, and closes it if its output has gone.
    pub(super) fn refresh_overview(&mut self) {
        let Some(output) = self.overview.as_ref().map(|open| open.output) else {
            return;
        };
        if !self.outputs.iter().any(|o| o.id == output) {
            self.overview = None;
            return;
        }
        let windows = self.overview_windows(output);
        if let Some(open) = &mut self.overview {
            open.windows = windows;
        }
    }

    /// The windows the Overview lists on `output`, bottom to top: those the Window
    /// switcher lists for the output's active workspace, so the minimised ones are among
    /// them, one that skips the taskbar isn't, and a modal dialog stands in for its
    /// parent. A sticky window is on every workspace of its output.
    fn overview_windows(&self, output: OutputId) -> Vec<WindowId> {
        let mut listed = Vec::new();
        let on_output = |id: &WindowId| self.output_showing(*id) == Some(output);
        let candidates = self.stack.iter().copied().filter(on_output);
        for base in candidates.filter(|id| self.is_switch_candidate(*id, SwitchList::Windows, None))
        {
            let window = self.modal_transient(base);
            if !listed.contains(&window) {
                listed.push(window);
            }
        }
        listed
    }
}
