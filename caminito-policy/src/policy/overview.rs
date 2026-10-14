//! The Overview (241, ADR 0033): whether it's open, the output it's on, the windows it
//! lists and the one selected (242), the drag of a window (245), and what a choice, an
//! arrow key, a middle click, a click on a workspace of its bar (244) and a drop do. The
//! compositor has its layout, its bar, its drawing, its keys and its clicks, and asks here
//! for what they do.
//!
//! It's the seat's Interaction (187), and every seat's, as the Window switcher is: the
//! shortcut opens it only while no seat has one, and nothing else opens while it's open.

use std::time::Duration;

use smithay::utils::{Logical, Point, Rectangle};

use crate::{Direction, OutputId, PolicyWindow, SeatId, WindowId};

use super::Policy;
use super::switcher::SwitchList;

/// The open Overview.
#[derive(Debug, Clone, PartialEq)]
pub struct Overview {
    /// The seat that opened it, which a chosen window is activated for.
    pub seat: SeatId,
    /// The output it's on, whose active workspace's windows it lists.
    pub output: OutputId,
    /// That output's active workspace, as the list has it. A switch while it's open (244)
    /// lists the new one's windows.
    pub workspace: usize,
    /// The windows listed, bottom to top. Policy keeps the list as the windows are now: one
    /// that goes leaves it.
    pub windows: Vec<WindowId>,
    /// The selected window, which Enter chooses (242): the focused one at opening, and after
    /// a switch of workspace (244), or none. It's always one of `windows`: none is selected
    /// once it goes.
    pub selected: Option<WindowId>,
    /// The left button held down on a listed window, and the drag it became once the
    /// pointer went past `drag-threshold` (245). It ends with the window's going.
    pub drag: Option<OverviewDrag>,
}

/// A press of the left button on a window of the Overview's layout, which is a drag of the
/// window once the pointer goes past `drag-threshold` (245). Until then its release is a
/// click.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverviewDrag {
    /// The window pressed, which a drop moves.
    pub window: WindowId,
    /// Where the button went down, in global logical coordinates.
    pub from: Point<f64, Logical>,
    /// Where the pointer is now.
    pub pointer: Point<f64, Logical>,
    /// Whether the pointer has gone past `drag-threshold` from `from`, as the titlebar's
    /// drag has it: a press is a drag from then on, wherever the pointer goes after.
    pub dragging: bool,
    /// The workspace of the bar the pointer is on, which a drop sends the window to: none
    /// on anything else, and none before the drag.
    pub target: Option<usize>,
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
        let workspace = self.active_workspace_on(output);
        let windows = self.overview_windows(output, workspace);
        let selected = self.focus(seat).filter(|focused| windows.contains(focused));
        self.overview = Some(Overview {
            seat,
            output,
            workspace,
            windows,
            selected,
            drag: None,
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

    /// A click on a workspace of the bar (244): switches the Overview's output to it, as
    /// `switch_output_workspace` does, so every output with `workspaces-per-output` off.
    /// The Overview stays open and lists the new workspace's windows, with its focused one
    /// selected. Does nothing with the Overview closed, or for a workspace past the count.
    pub fn overview_switch(&mut self, workspace: usize) {
        let Some(output) = self.overview.as_ref().map(|open| open.output) else {
            return;
        };
        if workspace < self.workspace_count {
            self.switch_output_workspace(output, workspace);
        }
    }

    /// The left button went down at `at` on a listed window (245): a click on it, or a
    /// drag of it once the pointer goes past `drag-threshold`. Does nothing for a window
    /// the Overview doesn't list, or while a press is held already.
    pub fn overview_press(&mut self, id: WindowId, at: Point<f64, Logical>) {
        let Some(open) = &mut self.overview else {
            return;
        };
        if open.drag.is_none() && open.windows.contains(&id) {
            open.drag = Some(OverviewDrag {
                window: id,
                from: at,
                pointer: at,
                dragging: false,
                target: None,
            });
        }
    }

    /// The pointer moved to `at` with a press held on a window of the Overview (245),
    /// onto the bar's cell of `target` or onto no cell. Past `drag-threshold` from where
    /// the button went down, the press is a drag, whose drop is on `target`.
    pub fn overview_pointer(&mut self, at: Point<f64, Logical>, target: Option<usize>) {
        let threshold = f64::from(self.config.drag_threshold);
        let Some(drag) = self.overview.as_mut().and_then(|open| open.drag.as_mut()) else {
            return;
        };
        let moved = at - drag.from;
        drag.pointer = at;
        drag.dragging |= moved.x.abs() > threshold || moved.y.abs() > threshold;
        drag.target = target.filter(|_| drag.dragging);
    }

    /// The left button came up with a press held on a window of the Overview (245). Within
    /// `drag-threshold` it's a click, which chooses the window. A drag dropped on another
    /// workspace's cell moves the window there, as the Window menu's move does, so a sticky
    /// one stops being sticky, and it leaves the list. Dropped on the active workspace's
    /// cell or on no cell, it stays where it was. The Overview stays open after a drop.
    pub fn overview_release(&mut self, now: Duration) {
        let Some(open) = &mut self.overview else {
            return;
        };
        let workspace = open.workspace;
        let Some(drag) = open.drag.take() else {
            return;
        };
        if !drag.dragging {
            self.overview_choose(drag.window, now);
            return;
        }
        let elsewhere = |target: &usize| *target != workspace && *target < self.workspace_count;
        if let Some(target) = drag.target.filter(elsewhere) {
            self.move_to_workspace(drag.window, target);
        }
    }

    /// Esc during a drag of a window of the Overview (245): ends the drag, and the window
    /// stays where it was. Returns whether a drag was under way. A press that isn't a drag
    /// yet stays, as Esc closes the Overview then.
    pub fn overview_end_drag(&mut self) -> bool {
        let Some(open) = &mut self.overview else {
            return false;
        };
        open.drag.take_if(|drag| drag.dragging).is_some()
    }

    /// The windows the open Overview would list on `workspace` of its output, bottom to
    /// top, as it lists those of the active one: the compositor lays out those of a
    /// workspace a Workspace slide leaves (244). None with the Overview closed.
    pub fn overview_windows_on(&self, workspace: usize) -> Vec<WindowId> {
        self.overview
            .as_ref()
            .map(|open| self.overview_windows(open.output, workspace))
            .unwrap_or_default()
    }

    /// Selects `selected` in the open Overview, as the arrow keys would.
    #[cfg(test)]
    pub(crate) fn set_overview_selection(&mut self, selected: Option<WindowId>) {
        if let Some(open) = &mut self.overview {
            open.selected = selected;
        }
    }

    /// Brings the open Overview's list up to date, and closes it if its output has gone.
    /// After a switch of its output's workspace (244) it lists the new one's windows, and
    /// selects the seat's focused window among them, or none.
    pub(super) fn refresh_overview(&mut self) {
        let Some((seat, output)) = self.overview.as_ref().map(|open| (open.seat, open.output))
        else {
            return;
        };
        if !self.outputs.iter().any(|o| o.id == output) {
            self.overview = None;
            return;
        }
        let workspace = self.active_workspace_on(output);
        let windows = self.overview_windows(output, workspace);
        let focus = self.focus(seat);
        if let Some(open) = &mut self.overview {
            if open.workspace != workspace {
                open.workspace = workspace;
                open.selected = focus;
            }
            open.selected = open.selected.filter(|selected| windows.contains(selected));
            open.drag = open.drag.filter(|drag| windows.contains(&drag.window));
            open.windows = windows;
        }
    }

    /// The windows the Overview lists for `workspace` on `output`, bottom to top: those the
    /// Window switcher lists for it, so the minimised ones are among them, one that skips
    /// the taskbar isn't, and a modal dialog stands in for its parent. A sticky window is
    /// on every workspace of its output.
    fn overview_windows(&self, output: OutputId, workspace: usize) -> Vec<WindowId> {
        let mut listed = Vec::new();
        let on_output = |id: &WindowId| self.output_showing(*id) == Some(output);
        let candidates = self.stack.iter().copied().filter(on_output);
        // The Window switcher's list for the active workspace, of any workspace.
        let listable = |id: &WindowId| {
            self.is_switch_candidate(*id, SwitchList::WindowsAll, None)
                && self.is_on_workspace(*id, workspace)
        };
        for base in candidates.filter(listable) {
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
