//! The Overview (241, ADR 0033): whether it's open, the output it's on, the windows it
//! lists and the one selected (242), and what a choice, an arrow key and a middle click
//! do. The compositor has its layout, its drawing, its keys and
//! its clicks, and asks here for what they do.
//!
//! It's the seat's Interaction (187), and every seat's, as the Window switcher is: the
//! shortcut opens it only while no seat has one, and nothing else opens while it's open.

use std::time::Duration;

use smithay::utils::{Logical, Rectangle};

use crate::{Direction, OutputId, PolicyWindow, SeatId, WindowId};

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
    /// The selected window, which Enter chooses (242): the focused one at opening, or none.
    /// It's always one of `windows`: none is selected once it goes.
    pub selected: Option<WindowId>,
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
        let windows = self.overview_windows(output);
        let selected = self.focus(seat).filter(|focused| windows.contains(focused));
        self.overview = Some(Overview {
            seat,
            output,
            windows,
            selected,
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

    /// Enter: chooses the selected window, as a click on it does. Does nothing with none
    /// selected.
    pub fn overview_enter(&mut self, now: Duration) {
        if let Some(id) = self.overview.as_ref().and_then(|open| open.selected) {
            self.overview_choose(id, now);
        }
    }

    /// An arrow key: selects the nearest window `direction`'s way from the selected one in
    /// the layout, or the layout's first with none selected: the left one of the top row.
    /// A row's windows stand on one line, so it's the one whose bottom edge is highest, and
    /// then the left one. `places` are the listed windows' places in the layout, in the
    /// list's order.
    ///
    /// The window that way is one wholly past the selected one's edge on that side. For
    /// Left and Right it's on the same row, and for Up and Down one in the same column comes
    /// before any other. Of those, it's the one whose centre is nearest. At the layout's
    /// edge, and at a row's end, the selection stays.
    pub fn overview_select(&mut self, direction: Direction, places: &[Rectangle<i32, Logical>]) {
        let Some(open) = &mut self.overview else {
            return;
        };
        let laid_out: Vec<(WindowId, Rectangle<i32, Logical>)> = open
            .windows
            .iter()
            .copied()
            .zip(places.iter().copied())
            .collect();
        let from = open
            .selected
            .and_then(|selected| laid_out.iter().find(|(id, _)| *id == selected));
        let next = match from {
            Some(&(_, from)) => nearest(from, direction, &laid_out),
            None => laid_out
                .iter()
                .min_by_key(|(_, place)| (place.loc.y + place.size.h, place.loc.x))
                .map(|(id, _)| *id),
        };
        if next.is_some() {
            open.selected = next;
        }
    }

    /// A middle click on a listed window: asks it to close, as `close` does. The Overview
    /// stays open, and the window leaves it once it goes. Does nothing for a window the
    /// Overview doesn't list.
    pub fn overview_close_window(&mut self, id: WindowId) {
        if self
            .overview
            .as_ref()
            .is_some_and(|open| open.windows.contains(&id))
        {
            self.close(id);
        }
    }

    /// Selects `selected` in the open Overview, as the arrow keys would.
    #[cfg(test)]
    pub(crate) fn set_overview_selection(&mut self, selected: Option<WindowId>) {
        if let Some(open) = &mut self.overview {
            open.selected = selected;
        }
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
            open.selected = open.selected.filter(|selected| windows.contains(selected));
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

/// The window of `laid_out` nearest `from` on `direction`'s side, as `overview_select` has it.
fn nearest(
    from: Rectangle<i32, Logical>,
    direction: Direction,
    laid_out: &[(WindowId, Rectangle<i32, Logical>)],
) -> Option<WindowId> {
    let overlaps = |a: (i32, i32), b: (i32, i32)| a.0 < b.1 && b.0 < a.1;
    let span_x = |r: &Rectangle<i32, Logical>| (r.loc.x, r.loc.x + r.size.w);
    let span_y = |r: &Rectangle<i32, Logical>| (r.loc.y, r.loc.y + r.size.h);
    let centre = |r: &Rectangle<i32, Logical>| {
        (
            i64::from(r.loc.x) * 2 + i64::from(r.size.w),
            i64::from(r.loc.y) * 2 + i64::from(r.size.h),
        )
    };
    let (fx, fy) = centre(&from);
    laid_out
        .iter()
        .filter(|(_, place)| match direction {
            Direction::Left => place.loc.x + place.size.w <= from.loc.x,
            Direction::Right => place.loc.x >= from.loc.x + from.size.w,
            Direction::Up => place.loc.y + place.size.h <= from.loc.y,
            Direction::Down => place.loc.y >= from.loc.y + from.size.h,
        })
        // Left and Right stay on the row: a row's windows stand on one line, so another row's
        // are below or above it.
        .filter(|(_, place)| match direction {
            Direction::Left | Direction::Right => overlaps(span_y(place), span_y(&from)),
            Direction::Up | Direction::Down => true,
        })
        .min_by_key(|(_, place)| {
            let beside = match direction {
                Direction::Left | Direction::Right => true,
                Direction::Up | Direction::Down => overlaps(span_x(place), span_x(&from)),
            };
            let (x, y) = centre(place);
            (!beside, (x - fx).pow(2) + (y - fy).pow(2))
        })
        .map(|(id, _)| *id)
}
