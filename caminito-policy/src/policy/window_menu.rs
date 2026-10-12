//! The window menu (161, ADR 0026): the items Marco's `meta_window_show_menu` gives a
//! window and which of them are greyed (window.c:7083-7208), what each does
//! (`menu_callback`, window.c:6938-7079), and its state while it's open: the selection and
//! the submenu. *Move Titlebar On-screen* is here too, as
//! `meta_window_titlebar_is_onscreen` and `meta_window_shove_titlebar_onscreen` have it
//! (window.c:7211-7296). Marco is read at `8522ed1`.

use std::time::Duration;

use smithay::utils::{Logical, Point, Rectangle};

use crate::grid::Direction;
use crate::{PolicyWindow, SeatId, WindowId, WindowType, WrapStyle};

use super::Policy;

/// What an item of the window menu does, as Marco's `MetaMenuOp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuOp {
    Minimize,
    Maximize,
    Unmaximize,
    /// A keyboard move (156).
    Move,
    /// A keyboard resize (156).
    Resize,
    /// *Move Titlebar On-screen*, Marco's `META_MENU_OP_RECOVER`.
    Recover,
    /// *Always on Top*, which is one item, checked or not.
    ToggleAbove,
    /// *Always on Visible Workspace*.
    Stick,
    /// *Only on This Workspace*.
    Unstick,
    /// The moves to the workspace beside the active one. The window goes, and the active
    /// workspace stays.
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    /// An item of the submenu: the move to a workspace, numbered from 0.
    MoveToWorkspace(usize),
    Close,
}

/// The mark drawn before an item's label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuMark {
    None,
    /// A check box, ticked or not.
    Check(bool),
    /// A radio button, set or not.
    Radio(bool),
}

/// One item of the window menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuItem {
    pub op: MenuOp,
    /// Whether it can be chosen. One that can't is greyed.
    pub sensitive: bool,
    pub mark: MenuMark,
}

/// One row of the window menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuEntry {
    Separator,
    Item(MenuItem),
    /// *Move to Another Workspace*, with its submenu's items: one for each workspace.
    Workspaces(Vec<MenuItem>),
}

/// A place in the open window menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuSelection {
    /// An entry of the menu, from the first.
    Top(usize),
    /// An item of the submenu, from the first.
    Sub(usize),
}

/// A key pressed while the window menu is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKey {
    Up,
    Down,
    Home,
    End,
    /// Closes the submenu.
    Left,
    /// Opens the submenu.
    Right,
    /// Return or space: runs the selected item, or opens its submenu.
    Activate,
    /// Closes the menu, the submenu with it, as GTK's does.
    Escape,
}

/// The open window menu. Its entries are the window's when it opened, as Marco builds its
/// menu once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowMenu {
    /// The seat that opened it, which a move or resize from it is for.
    pub seat: SeatId,
    pub window: WindowId,
    pub entries: Vec<MenuEntry>,
    /// The selected entry's place in `entries`.
    pub selected: Option<usize>,
    /// Whether the submenu shows. The selected entry is its parent then.
    pub submenu_open: bool,
    /// The selected item's place in the submenu.
    pub submenu_selected: Option<usize>,
}

impl WindowMenu {
    /// The submenu's items, if the menu has it.
    pub fn submenu(&self) -> Option<&[MenuItem]> {
        self.entries.iter().find_map(|entry| match entry {
            MenuEntry::Workspaces(items) => Some(items.as_slice()),
            _ => None,
        })
    }

    /// Whether the entry at `index` can be selected: an item that isn't greyed, or the
    /// submenu's parent.
    fn selectable(&self, index: usize) -> bool {
        match self.entries.get(index) {
            Some(MenuEntry::Item(item)) => item.sensitive,
            Some(MenuEntry::Workspaces(_)) => true,
            Some(MenuEntry::Separator) | None => false,
        }
    }

    fn submenu_selectable(&self, index: usize) -> bool {
        self.submenu()
            .and_then(|items| items.get(index))
            .is_some_and(|item| item.sensitive)
    }
}

/// The next place from `from` among `len`, towards the end or `backward`, that
/// `selectable` allows, wrapping at the ends. With no `from` it's the first, or the last.
fn step(
    from: Option<usize>,
    len: usize,
    backward: bool,
    selectable: impl Fn(usize) -> bool,
) -> Option<usize> {
    let start = match (from, backward) {
        (Some(from), false) => from + 1,
        (Some(from), true) => from + len - 1,
        (None, false) => 0,
        (None, true) => len.checked_sub(1)?,
    };
    (0..len)
        .map(|offset| {
            if backward {
                (start + len - offset) % len
            } else {
                (start + offset) % len
            }
        })
        .find(|index| selectable(*index))
}

impl<W: PolicyWindow> Policy<W> {
    /// The window menu, while it's open.
    pub fn window_menu(&self) -> Option<&WindowMenu> {
        self.window_menu.as_ref()
    }

    /// Opens `id`'s window menu for `seat`, in place of the one that's open. Returns false
    /// if it doesn't open: the window isn't showing, or a seat has another Interaction, as
    /// a GTK menu doesn't open under another grab.
    pub fn open_window_menu(&mut self, seat: SeatId, id: WindowId) -> bool {
        self.window_menu = None;
        if !self.windows.contains_key(&id)
            || !self.is_showing(id)
            || !self.no_seat_has_an_interaction()
        {
            return false;
        }
        self.set_last_seat(seat);
        self.window_menu = Some(WindowMenu {
            seat,
            window: id,
            entries: self.window_menu_entries(id),
            selected: None,
            submenu_open: false,
            submenu_selected: None,
        });
        true
    }

    /// Closes the window menu, if it's open, and runs nothing.
    pub fn close_window_menu(&mut self) {
        self.window_menu = None;
    }

    /// The pointer is over `at`: it's selected, if it can be. Over the submenu's parent
    /// the submenu opens, and over any other entry that can be selected it closes. A
    /// separator and a greyed item change nothing.
    pub fn window_menu_select(&mut self, at: MenuSelection) {
        let Some(menu) = &mut self.window_menu else {
            return;
        };
        match at {
            MenuSelection::Top(index) if menu.selectable(index) => {
                let parent = matches!(menu.entries[index], MenuEntry::Workspaces(_));
                if menu.selected != Some(index) || !parent {
                    menu.submenu_selected = None;
                }
                menu.selected = Some(index);
                menu.submenu_open = parent;
            }
            MenuSelection::Sub(index) if menu.submenu_open && menu.submenu_selectable(index) => {
                menu.submenu_selected = Some(index);
            }
            _ => {}
        }
    }

    /// A click on `at`, or its mnemonic: an item that isn't greyed is run and the menu
    /// closes, and the submenu's parent opens the submenu. Anything else does nothing.
    pub fn window_menu_activate(&mut self, at: MenuSelection, now: Duration) {
        let Some(menu) = &mut self.window_menu else {
            return;
        };
        match at {
            MenuSelection::Top(index) if menu.selectable(index) => {
                menu.selected = Some(index);
                menu.submenu_open = false;
                menu.submenu_selected = None;
            }
            MenuSelection::Sub(index) if menu.submenu_open && menu.submenu_selectable(index) => {
                menu.submenu_selected = Some(index);
            }
            _ => return,
        }
        self.window_menu_key(MenuKey::Activate, now);
    }

    /// A key pressed while the window menu is open.
    pub fn window_menu_key(&mut self, key: MenuKey, now: Duration) {
        let Some(menu) = &mut self.window_menu else {
            return;
        };
        let in_submenu = menu.submenu_open;
        let parent = menu
            .selected
            .is_some_and(|index| matches!(menu.entries[index], MenuEntry::Workspaces(_)));
        // The arrows, Home and End move in the submenu while it's open.
        let moved = |menu: &WindowMenu, from: Option<usize>, backward: bool| {
            if in_submenu {
                let len = menu.submenu().map_or(0, <[MenuItem]>::len);
                step(from, len, backward, |index| menu.submenu_selectable(index))
            } else {
                step(from, menu.entries.len(), backward, |index| {
                    menu.selectable(index)
                })
            }
        };
        let current = if in_submenu {
            menu.submenu_selected
        } else {
            menu.selected
        };
        let to = match key {
            MenuKey::Down => Some(moved(menu, current, false)),
            MenuKey::Up => Some(moved(menu, current, true)),
            MenuKey::Home => Some(moved(menu, None, false)),
            MenuKey::End => Some(moved(menu, None, true)),
            _ => None,
        };
        if let Some(to) = to {
            // Nothing can be selected in a menu whose items are all greyed.
            if in_submenu {
                menu.submenu_selected = to.or(current);
            } else {
                menu.selected = to.or(current);
            }
            return;
        }
        match key {
            MenuKey::Escape => self.window_menu = None,
            MenuKey::Left => {
                menu.submenu_open = false;
                menu.submenu_selected = None;
            }
            MenuKey::Right | MenuKey::Activate if parent && !in_submenu => {
                menu.submenu_open = true;
                let len = menu.submenu().map_or(0, <[MenuItem]>::len);
                menu.submenu_selected =
                    step(None, len, false, |index| menu.submenu_selectable(index));
            }
            MenuKey::Activate => {
                let item = if in_submenu {
                    let items = menu.submenu().unwrap_or_default();
                    menu.submenu_selected.and_then(|index| items.get(index))
                } else {
                    menu.selected.and_then(|index| match &menu.entries[index] {
                        MenuEntry::Item(item) => Some(item),
                        _ => None,
                    })
                };
                let Some(item) = item.filter(|item| item.sensitive).copied() else {
                    return;
                };
                let (seat, window) = (menu.seat, menu.window);
                self.window_menu = None;
                self.window_menu_op(seat, window, item.op, now);
            }
            _ => {}
        }
    }

    /// Closes the window menu when its window stops showing. The caller applies.
    pub(super) fn close_stale_window_menu(&mut self) {
        if self.window_menu.as_ref().is_some_and(|menu| {
            !self.windows.contains_key(&menu.window) || !self.is_showing(menu.window)
        }) {
            self.window_menu = None;
        }
    }

    /// The entries of `id`'s menu, in Marco's order (`menuitems`, menu.c:66-105). Marco
    /// leaves *Roll Up* and *Unroll* out (window.c:7140-7145).
    fn window_menu_entries(&self, id: WindowId) -> Vec<MenuEntry> {
        let managed = &self.windows[&id];
        let functions = managed.functions();
        let count = self.workspace_count;
        let desktop_or_dock = self.is_desktop_or_dock(id);
        // Marco's `on_all_workspaces`, which its `always_sticky` windows are too.
        let on_all = managed.sticky || desktop_or_dock;
        let item = |op, sensitive| MenuItem {
            op,
            sensitive,
            mark: MenuMark::None,
        };
        let marked = |op, sensitive, mark| MenuItem {
            op,
            sensitive,
            mark,
        };

        let mut entries = vec![MenuEntry::Item(item(MenuOp::Minimize, functions.minimize))];
        let maximize = if managed.maximized() {
            MenuOp::Unmaximize
        } else {
            MenuOp::Maximize
        };
        entries.push(MenuEntry::Item(item(maximize, functions.maximize)));
        // Marco greys them by `META_WINDOW_ALLOWS_MOVE` and `_RESIZE`
        // (window-private.h:440-444). Here they're greyed where the keyboard move or
        // resize wouldn't start, which a window maximised both ways adds to Marco's. Marco
        // gives a splash screen no move function (window.c:6808-6814).
        let splash = managed.window_type == WindowType::Splashscreen;
        entries.push(MenuEntry::Item(item(
            MenuOp::Move,
            !splash && self.can_grab(id, false),
        )));
        entries.push(MenuEntry::Item(item(
            MenuOp::Resize,
            functions.resize() && self.can_grab(id, true),
        )));
        if !desktop_or_dock && !self.titlebar_is_onscreen(id) {
            entries.push(MenuEntry::Item(item(MenuOp::Recover, true)));
        }
        // With two workspaces Marco drops the submenu, and its separator with it
        // (menu.c:309-313).
        let submenu = count > 2;
        if submenu {
            entries.push(MenuEntry::Separator);
        }
        let fixed_layer = matches!(
            managed.window_type,
            WindowType::Desktop | WindowType::Dock | WindowType::Splashscreen
        );
        entries.push(MenuEntry::Item(marked(
            MenuOp::ToggleAbove,
            !fixed_layer,
            MenuMark::Check(managed.above),
        )));
        if count > 1 {
            entries.push(MenuEntry::Item(marked(
                MenuOp::Stick,
                !desktop_or_dock,
                MenuMark::Radio(on_all),
            )));
            entries.push(MenuEntry::Item(marked(
                MenuOp::Unstick,
                !desktop_or_dock,
                MenuMark::Radio(!on_all),
            )));
        }
        if !on_all {
            // Those the grid has from the active workspace, with no wrapping
            // (window.c:7120-7134).
            for (op, direction) in [
                (MenuOp::MoveLeft, Direction::Left),
                (MenuOp::MoveRight, Direction::Right),
                (MenuOp::MoveUp, Direction::Up),
                (MenuOp::MoveDown, Direction::Down),
            ] {
                if self.workspace_beside(direction).is_some() {
                    entries.push(MenuEntry::Item(item(op, true)));
                }
            }
        }
        if submenu {
            // The window's own workspace is greyed, unless it's on all of them
            // (menu.c:398-399).
            let items = (0..count)
                .map(|workspace| {
                    item(
                        MenuOp::MoveToWorkspace(workspace),
                        on_all || managed.workspace != workspace,
                    )
                })
                .collect();
            entries.push(MenuEntry::Workspaces(items));
        }
        entries.push(MenuEntry::Separator);
        entries.push(MenuEntry::Item(item(MenuOp::Close, true)));
        entries
    }

    /// The workspace beside the active one in the grid, with no wrapping, as the menu's
    /// moves have it.
    fn workspace_beside(&self, direction: Direction) -> Option<usize> {
        self.workspace_grid()
            .neighbour(self.active, direction, WrapStyle::NoWrap)
    }

    /// Runs an item of `id`'s menu for `seat`, as Marco's `menu_callback` does.
    fn window_menu_op(&mut self, seat: SeatId, id: WindowId, op: MenuOp, now: Duration) {
        if !self.windows.contains_key(&id) {
            return;
        }
        self.set_last_seat(seat);
        let workspace = match op {
            MenuOp::Minimize => {
                self.minimize(id);
                None
            }
            MenuOp::Maximize => {
                self.set_maximized(id, true);
                None
            }
            MenuOp::Unmaximize => {
                self.set_maximized(id, false);
                None
            }
            MenuOp::Move | MenuOp::Resize => {
                // The keyboard move is the focused window's in the compositor's keyboard
                // path, and Marco's menu is the focused window's too.
                if self.focus_window(seat, id, now) {
                    self.raise_family(id);
                }
                self.begin_keyboard_grab(seat, id, op == MenuOp::Resize);
                None
            }
            MenuOp::Recover => {
                self.shove_titlebar_onscreen(id);
                None
            }
            MenuOp::ToggleAbove => {
                self.set_above(id, !self.windows[&id].above);
                None
            }
            MenuOp::Stick => {
                self.set_sticky(id, true);
                None
            }
            MenuOp::Unstick => {
                self.set_sticky(id, false);
                None
            }
            MenuOp::MoveLeft => self.workspace_beside(Direction::Left),
            MenuOp::MoveRight => self.workspace_beside(Direction::Right),
            MenuOp::MoveUp => self.workspace_beside(Direction::Up),
            MenuOp::MoveDown => self.workspace_beside(Direction::Down),
            MenuOp::MoveToWorkspace(workspace) => {
                Some(workspace).filter(|workspace| *workspace < self.workspace_count)
            }
            MenuOp::Close => {
                self.close(id);
                None
            }
        };
        // Marco only changes the window's workspace (window.c:7063-7071): the active one
        // stays, and so the seat focuses the default window. The desktop and docks are on
        // every workspace always.
        if let Some(workspace) = workspace
            && !self.is_desktop_or_dock(id)
        {
            self.move_to_workspace(id, workspace);
        }
        self.apply();
    }

    /// Whether enough of `id`'s titlebar is on an output's work area to count as on
    /// screen: more than 8 pixels of its height, and more than half its width or 50
    /// pixels, whichever is less (`meta_window_titlebar_is_onscreen`). A window with no
    /// titlebar showing has none off screen. Marco tests against the rectangles that span
    /// its screen less the struts; these are each output's work area, so a titlebar across
    /// two outputs counts by the larger part alone.
    pub fn titlebar_is_onscreen(&self, id: WindowId) -> bool {
        let Some(managed) = self.windows.get(&id) else {
            return true;
        };
        let top = managed.visible_extents().top;
        if top <= 0 {
            return true;
        }
        let mut titlebar = managed.frame();
        titlebar.size.h = top;
        self.outputs.iter().map(Self::work_area).any(|area| {
            area.intersection(titlebar).is_some_and(|overlap| {
                overlap.size.h > titlebar.size.h.min(8)
                    && overlap.size.w > (titlebar.size.w / 2).min(50)
            })
        })
    }

    /// Moves `id` so that its titlebar is on an output, as Marco's *Move Titlebar
    /// On-screen* does: `meta_window_shove_titlebar_onscreen` moves its frame up or down
    /// into the work area it's over or under, and the move then goes through Marco's
    /// constraints, of which `constrain_titlebar_visible` (constraints.c:1483-1562) brings
    /// in a window beside every output and lifts the titlebar onto the bottom edge. A
    /// window with no titlebar, or held by its state, stays.
    fn shove_titlebar_onscreen(&mut self, id: WindowId) {
        let managed = &self.windows[&id];
        let e = managed.visible_extents();
        if e.top <= 0 || managed.constrained() {
            return;
        }
        let areas: Vec<Rectangle<i32, Logical>> =
            self.outputs.iter().map(Self::work_area).collect();
        let size = managed.size;
        let mut frame = managed.frame();

        // `meta_window_shove_titlebar_onscreen`: each area grown by the frame's width at
        // the sides and by its height at the bottom, and the frame shoved in with its x
        // fixed.
        let grown: Vec<_> = areas
            .iter()
            .map(|area| grow(*area, frame.size.w, 0, frame.size.h))
            .collect();
        if let Some(area) = best_to_shove_into(&grown, frame, true) {
            frame.loc.y = shoved(frame.loc.y, frame.size.h, area.loc.y, area.size.h);
        }

        // `constrain_titlebar_visible`: a quarter of the window's width stays on, within
        // 10 to 75 pixels, the titlebar never goes over the top, and at the bottom the
        // window may hang off by all but its titlebar.
        let onscreen = (size.w / 4).clamp(10, 75);
        let offscreen = (size.w - onscreen).max(0);
        let grown: Vec<_> = areas
            .iter()
            .map(|area| {
                let sideways = if area.size.w >= onscreen {
                    offscreen
                } else {
                    0
                };
                let down = if area.size.h >= e.top {
                    size.h + e.bottom
                } else {
                    0
                };
                grow(*area, sideways, 0, down)
            })
            .collect();
        let contained = grown.iter().any(|area| area.contains_rect(frame));
        if !contained && let Some(area) = best_to_shove_into(&grown, frame, false) {
            frame.loc.x = shoved(frame.loc.x, frame.size.w, area.loc.x, area.size.w);
            frame.loc.y = shoved(frame.loc.y, frame.size.h, area.loc.y, area.size.h);
        }

        let managed = self.windows.get_mut(&id).unwrap();
        managed.geometry.loc = Point::new(frame.loc.x + e.left, frame.loc.y + e.top);
    }

    /// Whether a move of `id`, or a resize, could start, but for what the seat is doing:
    /// `begin_grab`'s own conditions.
    pub(super) fn can_grab(&self, id: WindowId, resize: bool) -> bool {
        self.is_showing(id)
            && self.windows.get(&id).is_some_and(|m| {
                !m.maximized()
                    && !m.fullscreen
                    && !(m.shaded && resize)
                    && !matches!(m.window_type, WindowType::Desktop | WindowType::Dock)
            })
    }
}

/// `area` grown by `sideways` at its left and its right, `up` at its top and `down` at its
/// bottom, as Marco's `meta_rectangle_expand_region` grows each rectangle of a region.
fn grow(
    area: Rectangle<i32, Logical>,
    sideways: i32,
    up: i32,
    down: i32,
) -> Rectangle<i32, Logical> {
    Rectangle::new(
        (area.loc.x - sideways, area.loc.y - up).into(),
        (area.size.w + 2 * sideways, area.size.h + up + down).into(),
    )
}

/// The rectangle of `areas` that Marco's `meta_rectangle_shove_into_region` shoves `rect`
/// into (boxes.c:987-1040): the one it could overlap most, and of those the nearest. With
/// `fixed_x` only one that already holds `rect`'s width where it is.
fn best_to_shove_into(
    areas: &[Rectangle<i32, Logical>],
    rect: Rectangle<i32, Logical>,
    fixed_x: bool,
) -> Option<Rectangle<i32, Logical>> {
    let right = |r: Rectangle<i32, Logical>| r.loc.x + r.size.w;
    let bottom = |r: Rectangle<i32, Logical>| r.loc.y + r.size.h;
    let mut best: Option<(Rectangle<i32, Logical>, i64, i32)> = None;
    for area in areas.iter().copied() {
        if fixed_x && (area.loc.x > rect.loc.x || right(area) < right(rect)) {
            continue;
        }
        let overlap =
            i64::from(rect.size.w.min(area.size.w)) * i64::from(rect.size.h.min(area.size.h));
        let distance = (area.loc.x - rect.loc.x).max(0)
            + (right(rect) - right(area)).max(0)
            + (area.loc.y - rect.loc.y).max(0)
            + (bottom(rect) - bottom(area)).max(0);
        let better = best.is_none_or(|(_, best_overlap, best_distance)| {
            overlap > best_overlap || (overlap == best_overlap && distance < best_distance)
        });
        if better {
            best = Some((area, overlap, distance));
        }
    }
    best.map(|(area, ..)| area)
}

/// Where a span of `length` at `at` goes to be within the span of `within_length` at
/// `within`: forward to its start, then back to its end.
fn shoved(at: i32, length: i32, within: i32, within_length: i32) -> i32 {
    let at = at.max(within);
    if at + length > within + within_length {
        within + within_length - length
    } else {
        at
    }
}
