//! The window menu (161): which items each kind of window has and which are greyed, as
//! Marco's `meta_window_show_menu` has them, what each item does, the keys that move in it,
//! and *Move Titlebar On-screen*.

use super::*;

use crate::test_window::Call;
use crate::{MenuEntry, MenuItem, MenuKey, MenuMark, MenuOp, MenuSelection, Orientation};

fn at(x: f64, y: f64) -> Point<f64, Logical> {
    Point::new(x, y)
}

/// A policy with one output and `count` workspaces in one row.
fn workspaces(count: usize) -> Policy<TestWindow> {
    let mut policy = Policy::new(Config {
        workspace_count: count,
        ..Config::default()
    });
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy
}

/// An SSD window with a titlebar 24 high: its frame is 408 by 328.
fn framed(id: u64) -> TestWindow {
    TestWindow::new(id, 400, 300)
        .with_frame(4, 4, 24, 4)
        .with_requested_position(100, 100)
}

fn item(item: &MenuItem) -> String {
    let grey = if item.sensitive { "" } else { "~" };
    let mark = match item.mark {
        MenuMark::None => "",
        MenuMark::Check(false) => "[ ] ",
        MenuMark::Check(true) => "[x] ",
        MenuMark::Radio(false) => "( ) ",
        MenuMark::Radio(true) => "(o) ",
    };
    format!("{grey}{mark}{:?}", item.op)
}

/// The open menu's entries, one word each: `~` before a greyed item, its mark, and its
/// operation.
fn entries(policy: &Policy<TestWindow>) -> Vec<String> {
    let menu = policy.window_menu().expect("the window menu is open");
    menu.entries
        .iter()
        .map(|entry| match entry {
            MenuEntry::Separator => "---".to_owned(),
            MenuEntry::Item(entry) => item(entry),
            MenuEntry::Workspaces(_) => "Workspaces>".to_owned(),
        })
        .collect()
}

/// The open menu's submenu of workspaces.
fn submenu(policy: &Policy<TestWindow>) -> Vec<String> {
    let menu = policy.window_menu().expect("the window menu is open");
    let workspaces = menu.entries.iter().find_map(|entry| match entry {
        MenuEntry::Workspaces(items) => Some(items),
        _ => None,
    });
    workspaces
        .expect("the menu has the submenu")
        .iter()
        .map(item)
        .collect()
}

/// The place of `op` among the open menu's entries.
fn place(policy: &Policy<TestWindow>, op: MenuOp) -> MenuSelection {
    let menu = policy.window_menu().expect("the window menu is open");
    let index = menu
        .entries
        .iter()
        .position(|entry| matches!(entry, MenuEntry::Item(item) if item.op == op))
        .unwrap_or_else(|| panic!("the menu has no {op:?}"));
    MenuSelection::Top(index)
}

/// Opens window 1's menu and chooses `op` in it.
fn choose(policy: &mut Policy<TestWindow>, op: MenuOp) {
    assert!(policy.open_window_menu(SEAT, id(1)));
    let at = place(policy, op);
    policy.window_menu_activate(at, ms(50));
}

/// Moves a window's frame to `(x, y)` with the pointer, which may leave it off every
/// output.
fn put_frame_at(policy: &mut Policy<TestWindow>, window: u64, x: i32, y: i32) {
    let frame = policy.frame_rect(id(window)).unwrap();
    assert!(policy.begin_move(SEAT, id(window), at(0., 0.)));
    let delta = Point::<i32, Logical>::new(x, y) - frame.loc;
    // Far off first, and back: an output's edge holds a frame that comes to it from inside
    // (157).
    policy.grab_motion(SEAT, at(-9000., -9000.), false);
    policy.grab_motion(SEAT, delta.to_f64(), false);
    policy.end_grab(SEAT);
    assert_eq!(policy.frame_rect(id(window)).unwrap().loc, (x, y).into());
}

#[test]
fn a_normal_window_has_marcos_items_in_marcos_order() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    assert!(policy.open_window_menu(SEAT, id(1)));

    assert_eq!(
        entries(&policy),
        [
            "Minimize",
            "Maximize",
            "Move",
            "Resize",
            "---",
            "[ ] ToggleAbove",
            "( ) Stick",
            "(o) Unstick",
            "MoveRight",
            "Workspaces>",
            "---",
            "Close",
        ]
    );
    // The window's own workspace is greyed.
    assert_eq!(
        submenu(&policy),
        [
            "~MoveToWorkspace(0)",
            "MoveToWorkspace(1)",
            "MoveToWorkspace(2)",
            "MoveToWorkspace(3)",
        ]
    );
    let menu = policy.window_menu().unwrap();
    assert_eq!((menu.window, menu.selected), (id(1), None));
    assert!(!menu.submenu_open);
}

#[test]
fn a_maximised_window_has_unmaximize_and_cant_be_moved_or_resized() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.set_maximized(id(1), true);

    policy.open_window_menu(SEAT, id(1));

    assert_eq!(
        entries(&policy)[..4],
        ["Minimize", "Unmaximize", "~Move", "~Resize"]
    );
}

#[test]
fn a_fixed_size_window_has_maximize_and_resize_greyed() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_hints((400, 300), (400, 300)),
        ms(1),
    );

    policy.open_window_menu(SEAT, id(1));

    assert_eq!(
        entries(&policy)[..4],
        ["Minimize", "~Maximize", "Move", "~Resize"]
    );
}

#[test]
fn a_fullscreen_window_cant_be_maximised_moved_or_resized() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.set_fullscreen(id(1), true, None);

    policy.open_window_menu(SEAT, id(1));

    assert_eq!(
        entries(&policy)[..4],
        ["Minimize", "~Maximize", "~Move", "~Resize"]
    );
}

#[test]
fn a_shaded_window_cant_be_resized() {
    let mut policy = policy();
    policy.map_window(framed(1), ms(1));
    policy.set_shaded(id(1), true);

    policy.open_window_menu(SEAT, id(1));

    assert_eq!(
        entries(&policy)[..4],
        ["Minimize", "Maximize", "Move", "~Resize"]
    );
}

#[test]
fn a_dock_has_only_close_and_the_submenu_to_choose() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 1920, 30).with_type(WindowType::Dock),
        ms(1),
    );

    policy.open_window_menu(SEAT, id(1));

    // It's on every workspace, so it has no move to the workspace beside.
    assert_eq!(
        entries(&policy),
        [
            "~Minimize",
            "~Maximize",
            "~Move",
            "~Resize",
            "---",
            "~[ ] ToggleAbove",
            "~(o) Stick",
            "~( ) Unstick",
            "Workspaces>",
            "---",
            "Close",
        ]
    );
}

#[test]
fn a_dialog_has_a_normal_windows_items_and_a_splash_screen_only_the_workspace_ones() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_type(WindowType::Dialog),
        ms(1),
    );
    policy.map_window(
        TestWindow::new(2, 400, 300).with_type(WindowType::Splashscreen),
        ms(2),
    );

    policy.open_window_menu(SEAT, id(1));
    assert_eq!(
        entries(&policy)[..6],
        [
            "Minimize",
            "Maximize",
            "Move",
            "Resize",
            "---",
            "[ ] ToggleAbove"
        ]
    );

    policy.open_window_menu(SEAT, id(2));
    assert_eq!(
        entries(&policy)[..6],
        [
            "~Minimize",
            "~Maximize",
            "~Move",
            "~Resize",
            "---",
            "~[ ] ToggleAbove"
        ]
    );
}

#[test]
fn a_window_kept_on_top_has_its_item_checked() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.set_above(id(1), true);

    policy.open_window_menu(SEAT, id(1));

    assert_eq!(entries(&policy)[5], "[x] ToggleAbove");
}

#[test]
fn a_window_on_every_workspace_has_no_move_beside_and_every_workspace_to_go_to() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.set_sticky(id(1), true);

    policy.open_window_menu(SEAT, id(1));

    assert_eq!(
        entries(&policy)[5..],
        [
            "[ ] ToggleAbove",
            "(o) Stick",
            "( ) Unstick",
            "Workspaces>",
            "---",
            "Close"
        ]
    );
    assert_eq!(
        submenu(&policy),
        [
            "MoveToWorkspace(0)",
            "MoveToWorkspace(1)",
            "MoveToWorkspace(2)",
            "MoveToWorkspace(3)",
        ]
    );
}

#[test]
fn one_workspace_leaves_no_workspace_item() {
    let mut policy = workspaces(1);
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    policy.open_window_menu(SEAT, id(1));

    assert_eq!(
        entries(&policy),
        [
            "Minimize",
            "Maximize",
            "Move",
            "Resize",
            "[ ] ToggleAbove",
            "---",
            "Close",
        ]
    );
}

#[test]
fn two_workspaces_leave_no_submenu_and_no_separator_before_the_marks() {
    let mut policy = workspaces(2);
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    policy.open_window_menu(SEAT, id(1));

    assert_eq!(
        entries(&policy),
        [
            "Minimize",
            "Maximize",
            "Move",
            "Resize",
            "[ ] ToggleAbove",
            "( ) Stick",
            "(o) Unstick",
            "MoveRight",
            "---",
            "Close",
        ]
    );
}

#[test]
fn the_moves_beside_are_those_the_workspace_grid_has_from_the_active_workspace() {
    // Six workspaces in two rows of three, and the fifth, in the middle of the second row,
    // is the active one.
    let mut policy = Policy::new(Config {
        workspace_count: 6,
        workspace_layout: WorkspaceLayout {
            orientation: Orientation::Horizontal,
            rows: 2,
            columns: 0,
        },
        ..Config::default()
    });
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy.switch_workspace(4);
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    policy.open_window_menu(SEAT, id(1));
    assert_eq!(entries(&policy)[8..11], ["MoveLeft", "MoveRight", "MoveUp"]);
    assert_eq!(entries(&policy)[11], "Workspaces>");

    // The first workspace has one to its right and one below.
    policy.switch_workspace(0);
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.open_window_menu(SEAT, id(2));
    assert_eq!(entries(&policy)[8..10], ["MoveRight", "MoveDown"]);
    assert_eq!(entries(&policy)[10], "Workspaces>");
}

#[test]
fn each_item_does_what_its_name_says() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    choose(&mut policy, MenuOp::Maximize);
    assert!(policy.is_maximized(id(1)));
    assert!(policy.window_menu().is_none());

    choose(&mut policy, MenuOp::Unmaximize);
    assert!(!policy.is_maximized(id(1)));

    choose(&mut policy, MenuOp::ToggleAbove);
    assert!(policy.is_above(id(1)));
    choose(&mut policy, MenuOp::ToggleAbove);
    assert!(!policy.is_above(id(1)));

    choose(&mut policy, MenuOp::Stick);
    assert!(policy.is_sticky(id(1)));
    choose(&mut policy, MenuOp::Unstick);
    assert!(!policy.is_sticky(id(1)));

    choose(&mut policy, MenuOp::Close);
    assert!(window(&policy, 1).calls.contains(&Call::Close));

    choose(&mut policy, MenuOp::Minimize);
    assert!(policy.is_minimized(id(1)));
}

#[test]
fn move_and_resize_start_the_keyboards_move_and_resize() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    choose(&mut policy, MenuOp::Move);
    assert_eq!(policy.keyboard_grab(SEAT), Some(id(1)));
    assert_eq!(policy.grab_edges(SEAT), None);
    policy.grab_key(SEAT, crate::GrabKey::Other);

    choose(&mut policy, MenuOp::Resize);
    assert_eq!(policy.keyboard_grab(SEAT), Some(id(1)));
    assert!(policy.grab_edges(SEAT).is_some());
}

#[test]
fn a_move_beside_takes_the_window_and_leaves_the_active_workspace() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.activate(SEAT, id(1), ms(3));

    choose(&mut policy, MenuOp::MoveRight);

    // Marco's menu only changes the window's workspace (window.c:7063-7071), where the
    // binding switches there too.
    assert_eq!(policy.workspace_of(id(1)), Some(1));
    assert_eq!(policy.active_workspace(), 0);
    assert_eq!(policy.focus(SEAT), Some(id(2)));
}

#[test]
fn the_submenu_takes_the_window_to_the_workspace_chosen() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.open_window_menu(SEAT, id(1));

    // Nothing of the submenu can be chosen while it's closed.
    policy.window_menu_activate(MenuSelection::Sub(2), ms(2));
    assert_eq!(policy.workspace_of(id(1)), Some(0));
    policy.window_menu_select(MenuSelection::Top(9));

    // Its own workspace is greyed, and choosing it does nothing.
    policy.window_menu_activate(MenuSelection::Sub(0), ms(2));
    assert!(policy.window_menu().is_some());

    policy.window_menu_activate(MenuSelection::Sub(2), ms(3));

    assert_eq!(policy.workspace_of(id(1)), Some(2));
    assert_eq!(policy.active_workspace(), 0);
    assert!(policy.window_menu().is_none());
}

#[test]
fn a_greyed_item_does_nothing_and_the_menu_stays() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.set_maximized(id(1), true);
    policy.open_window_menu(SEAT, id(1));

    let greyed = place(&policy, MenuOp::Move);
    policy.window_menu_activate(greyed, ms(2));

    assert!(policy.window_menu().is_some());
    assert_eq!(policy.keyboard_grab(SEAT), None);
}

#[test]
fn the_arrows_skip_separators_and_greyed_items_and_wrap() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.set_maximized(id(1), true);
    policy.open_window_menu(SEAT, id(1));
    let selected = |policy: &Policy<TestWindow>| policy.window_menu().unwrap().selected;

    // Minimize, Unmaximize, then over Move, Resize and the separator to Always on Top.
    policy.window_menu_key(MenuKey::Down, ms(2));
    assert_eq!(selected(&policy), Some(0));
    policy.window_menu_key(MenuKey::Down, ms(2));
    policy.window_menu_key(MenuKey::Down, ms(2));
    assert_eq!(selected(&policy), Some(5));
    policy.window_menu_key(MenuKey::Up, ms(2));
    assert_eq!(selected(&policy), Some(1));

    // Up from the first goes to the last, Close, and Down from there to the first.
    policy.window_menu_key(MenuKey::Up, ms(2));
    policy.window_menu_key(MenuKey::Up, ms(2));
    assert_eq!(selected(&policy), Some(11));
    policy.window_menu_key(MenuKey::Down, ms(2));
    assert_eq!(selected(&policy), Some(0));

    policy.window_menu_key(MenuKey::End, ms(2));
    assert_eq!(selected(&policy), Some(11));
    policy.window_menu_key(MenuKey::Home, ms(2));
    assert_eq!(selected(&policy), Some(0));
}

#[test]
fn up_with_nothing_selected_selects_the_last_item() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.open_window_menu(SEAT, id(1));

    policy.window_menu_key(MenuKey::Up, ms(2));

    assert_eq!(policy.window_menu().unwrap().selected, Some(11));
}

#[test]
fn return_runs_the_selected_item_and_escape_closes_the_menu() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.open_window_menu(SEAT, id(1));

    // With nothing selected it does nothing.
    policy.window_menu_key(MenuKey::Activate, ms(2));
    assert!(policy.window_menu().is_some());

    policy.window_menu_key(MenuKey::Down, ms(2));
    policy.window_menu_key(MenuKey::Down, ms(2));
    policy.window_menu_key(MenuKey::Activate, ms(3));
    assert!(policy.is_maximized(id(1)));
    assert!(policy.window_menu().is_none());

    policy.open_window_menu(SEAT, id(1));
    policy.window_menu_key(MenuKey::Escape, ms(4));
    assert!(policy.window_menu().is_none());
    assert!(policy.is_maximized(id(1)));
}

#[test]
fn right_and_return_open_the_submenu_and_left_closes_it() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.open_window_menu(SEAT, id(1));
    let state = |policy: &Policy<TestWindow>| {
        let menu = policy.window_menu().unwrap();
        (menu.selected, menu.submenu_open, menu.submenu_selected)
    };

    // Right does nothing on an item with no submenu.
    policy.window_menu_key(MenuKey::Down, ms(2));
    policy.window_menu_key(MenuKey::Right, ms(2));
    assert_eq!(state(&policy), (Some(0), false, None));

    policy.window_menu_select(MenuSelection::Top(9));
    policy.window_menu_key(MenuKey::Left, ms(2));
    assert_eq!(state(&policy), (Some(9), false, None));

    // The submenu opens on its first item that isn't greyed.
    policy.window_menu_key(MenuKey::Right, ms(2));
    assert_eq!(state(&policy), (Some(9), true, Some(1)));

    // The arrows move in it, and wrap past the greyed first item.
    policy.window_menu_key(MenuKey::Up, ms(2));
    assert_eq!(state(&policy), (Some(9), true, Some(3)));
    policy.window_menu_key(MenuKey::Down, ms(2));
    assert_eq!(state(&policy), (Some(9), true, Some(1)));

    policy.window_menu_key(MenuKey::Left, ms(2));
    assert_eq!(state(&policy), (Some(9), false, None));

    policy.window_menu_key(MenuKey::Activate, ms(2));
    assert_eq!(state(&policy), (Some(9), true, Some(1)));
    policy.window_menu_key(MenuKey::Down, ms(2));
    policy.window_menu_key(MenuKey::Activate, ms(3));
    assert_eq!(policy.workspace_of(id(1)), Some(2));
    assert!(policy.window_menu().is_none());
}

#[test]
fn the_pointer_selects_what_it_is_over_and_opens_the_submenu() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.open_window_menu(SEAT, id(1));
    let state = |policy: &Policy<TestWindow>| {
        let menu = policy.window_menu().unwrap();
        (menu.selected, menu.submenu_open, menu.submenu_selected)
    };

    policy.window_menu_select(MenuSelection::Top(1));
    assert_eq!(state(&policy), (Some(1), false, None));

    // Over the submenu's item the submenu opens, with nothing selected in it.
    policy.window_menu_select(MenuSelection::Top(9));
    assert_eq!(state(&policy), (Some(9), true, None));
    policy.window_menu_select(MenuSelection::Sub(2));
    assert_eq!(state(&policy), (Some(9), true, Some(2)));

    // Over another item it closes.
    policy.window_menu_select(MenuSelection::Top(0));
    assert_eq!(state(&policy), (Some(0), false, None));

    // A separator and a greyed item aren't selected, and neither is what isn't there.
    policy.window_menu_select(MenuSelection::Top(4));
    policy.window_menu_select(MenuSelection::Top(40));
    policy.window_menu_select(MenuSelection::Sub(1));
    assert_eq!(state(&policy), (Some(0), false, None));
}

#[test]
fn a_click_on_the_submenus_item_opens_it() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.open_window_menu(SEAT, id(1));

    policy.window_menu_activate(MenuSelection::Top(9), ms(2));

    let menu = policy.window_menu().unwrap();
    assert_eq!(
        (menu.selected, menu.submenu_open, menu.submenu_selected),
        (Some(9), true, Some(1))
    );
}

#[test]
fn the_binding_opens_the_focused_windows_menu() {
    let mut policy = policy();
    policy.action(SEAT, Action::ActivateWindowMenu, ms(1));
    assert!(policy.window_menu().is_none());

    policy.map_window(TestWindow::new(1, 400, 300), ms(2));
    policy.map_window(TestWindow::new(2, 400, 300), ms(3));

    policy.action(SEAT, Action::ActivateWindowMenu, ms(4));

    assert_eq!(policy.window_menu().unwrap().window, id(2));
}

#[test]
fn opening_another_windows_menu_replaces_the_open_one() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.open_window_menu(SEAT, id(1));
    policy.window_menu_key(MenuKey::Down, ms(3));

    assert!(policy.open_window_menu(SEAT, id(2)));

    let menu = policy.window_menu().unwrap();
    assert_eq!((menu.window, menu.selected), (id(2), None));
}

#[test]
fn no_menu_opens_during_a_move_or_with_the_window_switcher_open_or_for_a_hidden_window() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));

    policy.begin_move(SEAT, id(1), at(0., 0.));
    assert!(!policy.open_window_menu(SEAT, id(1)));
    policy.end_grab(SEAT);

    policy.action(
        SEAT,
        Action::Switch {
            list: crate::SwitchList::Windows,
            backward: false,
        },
        ms(3),
    );
    assert!(policy.switcher().is_some());
    assert!(!policy.open_window_menu(SEAT, id(1)));
    policy.switcher_cancel(ms(4));

    policy.minimize(id(1));
    assert!(!policy.open_window_menu(SEAT, id(1)));
    assert!(!policy.open_window_menu(SEAT, id(9)));
    assert!(policy.window_menu().is_none());
}

#[test]
fn the_menu_closes_when_its_window_goes_or_hides_or_the_workspace_changes() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));

    policy.open_window_menu(SEAT, id(1));
    policy.unmap_window(id(1));
    assert!(policy.window_menu().is_none());

    policy.open_window_menu(SEAT, id(2));
    policy.switch_workspace(1);
    assert!(policy.window_menu().is_none());

    policy.switch_workspace(0);
    policy.open_window_menu(SEAT, id(2));
    policy.minimize(id(2));
    assert!(policy.window_menu().is_none());
}

#[test]
fn a_window_with_its_titlebar_on_an_output_has_no_move_titlebar_on_screen() {
    let mut policy = policy();
    policy.map_window(framed(1), ms(1));
    assert!(policy.titlebar_is_onscreen(id(1)));

    // With 60 pixels of its titlebar's width still on the output, more than Marco's 50.
    put_frame_at(&mut policy, 1, 1860, 500);
    assert!(policy.titlebar_is_onscreen(id(1)));
    policy.open_window_menu(SEAT, id(1));
    assert!(!entries(&policy).contains(&"Recover".to_owned()));
    // No move begins over the open menu (187).
    policy.close_window_menu();

    // And with 50, which isn't more.
    put_frame_at(&mut policy, 1, 1870, 500);
    assert!(!policy.titlebar_is_onscreen(id(1)));
    policy.open_window_menu(SEAT, id(1));
    assert_eq!(
        entries(&policy)[..6],
        ["Minimize", "Maximize", "Move", "Resize", "Recover", "---"]
    );
}

#[test]
fn a_titlebar_counts_as_on_screen_with_more_than_8_pixels_of_its_height_on_an_output() {
    let mut policy = policy();
    policy.map_window(framed(1), ms(1));

    put_frame_at(&mut policy, 1, 500, -15);
    assert!(policy.titlebar_is_onscreen(id(1)));
    put_frame_at(&mut policy, 1, 500, -16);
    assert!(!policy.titlebar_is_onscreen(id(1)));
}

#[test]
fn a_titlebar_under_a_panel_is_not_on_screen() {
    let mut policy = Policy::new(Config::default());
    policy.add_output(reserving(output(1, 0, 0, 1920, 1080), 0, 0, 30, 0));
    policy.map_window(framed(1), ms(1));

    put_frame_at(&mut policy, 1, 500, 0);
    assert!(!policy.titlebar_is_onscreen(id(1)));

    choose(&mut policy, MenuOp::Recover);
    assert_eq!(policy.frame_rect(id(1)).unwrap().loc, (500, 30).into());
}

#[test]
fn a_csd_window_has_no_titlebar_to_move_on_screen() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    put_frame_at(&mut policy, 1, 500, -250);

    assert!(policy.titlebar_is_onscreen(id(1)));
    policy.open_window_menu(SEAT, id(1));
    assert!(!entries(&policy).contains(&"Recover".to_owned()));
}

/// Puts window 1's frame at `from`, chooses *Move Titlebar On-screen*, and returns where
/// the frame is then.
fn recovered(policy: &mut Policy<TestWindow>, from: (i32, i32)) -> (i32, i32) {
    put_frame_at(policy, 1, from.0, from.1);
    assert!(!policy.titlebar_is_onscreen(id(1)));
    choose(policy, MenuOp::Recover);
    assert!(policy.titlebar_is_onscreen(id(1)));
    let loc = policy.frame_rect(id(1)).unwrap().loc;
    // The window is told.
    let (geometry, _) = window(policy, 1).last_geometry().unwrap();
    assert_eq!(geometry.loc, loc + Point::new(4, 24));
    (loc.x, loc.y)
}

#[test]
fn move_titlebar_on_screen_brings_down_a_window_above_every_output() {
    let mut policy = policy();
    policy.map_window(framed(1), ms(1));

    // The frame's top goes to the work area's, and it isn't moved sideways.
    assert_eq!(recovered(&mut policy, (500, -400)), (500, 0));
}

#[test]
fn move_titlebar_on_screen_brings_up_a_window_below_every_output() {
    let mut policy = policy();
    policy.map_window(framed(1), ms(1));

    // The titlebar, 24 high, sits on the work area's bottom, as Marco's titlebar constraint
    // leaves it (constraints.c:1528-1536).
    assert_eq!(recovered(&mut policy, (500, 1500)), (500, 1056));
}

#[test]
fn move_titlebar_on_screen_brings_in_a_window_beside_every_output() {
    let mut policy = policy();
    policy.map_window(framed(1), ms(1));

    // 75 pixels of the window's width stay on the output, a quarter of it at most
    // (constraints.c:1513-1527), and it isn't moved up or down.
    assert_eq!(recovered(&mut policy, (2500, 200)), (1837, 200));
    assert_eq!(recovered(&mut policy, (-1000, 200)), (-325, 200));
}

#[test]
fn move_titlebar_on_screen_brings_in_a_window_off_a_corner() {
    let mut policy = policy();
    policy.map_window(framed(1), ms(1));

    assert_eq!(recovered(&mut policy, (2500, 1500)), (1837, 1056));
    assert_eq!(recovered(&mut policy, (-1000, -900)), (-325, 0));
}

#[test]
fn move_titlebar_on_screen_goes_to_the_output_the_window_is_under() {
    let mut policy = policy();
    policy.add_output(output(2, 1920, 0, 1280, 1024));
    policy.map_window(framed(1), ms(1));

    assert_eq!(recovered(&mut policy, (2200, 1500)), (2200, 1000));
    assert_eq!(recovered(&mut policy, (300, 1500)), (300, 1056));
    // Right of both, it comes in onto the second.
    assert_eq!(recovered(&mut policy, (4000, 200)), (3117, 200));
}
