//! The Overview (241, ADR 0033): the windows it lists, what opens and closes it, and what
//! a choice does. That none opens over another Interaction, and none over it, is
//! `interaction`'s.

use super::*;
use crate::test_window::Call;
use crate::{Direction, Interaction};

fn four_workspaces() -> Config {
    Config {
        workspace_count: 4,
        ..Config::default()
    }
}

/// Four workspaces on one output, and three windows on the first, 3 focused and on top.
fn three() -> Policy<TestWindow> {
    let mut policy = Policy::new(four_workspaces());
    policy.add_output(output(1, 0, 0, 1920, 1080));
    for w in 1..=3 {
        policy.map_window(TestWindow::new(w, 400, 300), ms(w));
    }
    policy
}

fn toggle(policy: &mut Policy<TestWindow>) {
    policy.action(SEAT, Action::Overview, ms(100));
}

/// The windows the open Overview lists.
fn listed(policy: &Policy<TestWindow>) -> Vec<WindowId> {
    policy
        .overview()
        .expect("the Overview is open")
        .windows
        .clone()
}

// Opening and closing.

#[test]
fn the_shortcut_opens_the_overview_on_the_seats_active_output() {
    let mut policy = three();
    policy.add_output(output(2, 1920, 0, 1920, 1080));
    assert!(policy.overview().is_none());

    toggle(&mut policy);

    let overview = policy.overview().unwrap();
    assert_eq!(Some(overview.output), policy.active_output(SEAT));
    assert_eq!(policy.interaction(SEAT), Some(Interaction::Overview));
}

#[test]
fn the_shortcut_again_closes_it_and_changes_nothing() {
    let mut policy = three();
    policy.minimize(id(1));
    let (focus, stacking) = (policy.focus(SEAT), policy.active_stacking());
    toggle(&mut policy);

    toggle(&mut policy);

    assert!(policy.overview().is_none());
    assert_eq!(policy.interaction(SEAT), None);
    assert_eq!(policy.focus(SEAT), focus);
    assert_eq!(policy.active_stacking(), stacking);
    assert!(policy.is_minimized(id(1)));
}

#[test]
fn esc_or_a_click_on_the_empty_area_closes_it_and_changes_nothing() {
    let mut policy = three();
    policy.minimize(id(1));
    let (focus, stacking) = (policy.focus(SEAT), policy.active_stacking());
    toggle(&mut policy);

    // The compositor asks for the same close for both.
    policy.close_overview();

    assert!(policy.overview().is_none());
    assert_eq!(policy.focus(SEAT), focus);
    assert_eq!(policy.active_stacking(), stacking);
    assert!(policy.is_minimized(id(1)));
}

#[test]
fn it_opens_on_a_workspace_with_no_windows() {
    let mut policy = Policy::new(four_workspaces());
    policy.add_output(output(1, 0, 0, 1920, 1080));

    toggle(&mut policy);

    assert_eq!(listed(&policy), []);
}

#[test]
fn it_opens_with_no_output_nowhere() {
    let mut policy: Policy<TestWindow> = Policy::new(four_workspaces());

    toggle(&mut policy);

    assert!(policy.overview().is_none());
}

#[test]
fn a_focused_fullscreen_window_is_listed_like_any_other() {
    let mut policy = three();
    policy.set_fullscreen(id(3), true, None);

    toggle(&mut policy);

    assert_eq!(listed(&policy), [id(1), id(2), id(3)]);
}

#[test]
fn its_output_going_closes_it() {
    let mut policy = three();
    toggle(&mut policy);

    policy.remove_output(OutputId(1));

    assert!(policy.overview().is_none());
}

// The windows listed.

#[test]
fn it_lists_the_active_workspaces_windows_bottom_to_top() {
    let mut policy = three();
    policy.raise(id(1));

    toggle(&mut policy);

    assert_eq!(listed(&policy), [id(2), id(3), id(1)]);
}

#[test]
fn a_minimised_window_is_listed() {
    let mut policy = three();
    policy.minimize(id(2));

    toggle(&mut policy);

    assert!(listed(&policy).contains(&id(2)));
}

#[test]
fn a_window_that_skips_the_taskbar_isnt_listed() {
    let mut policy = three();
    policy.map_window(TestWindow::new(4, 200, 100).skipping_taskbar(), ms(4));

    toggle(&mut policy);

    assert_eq!(listed(&policy), [id(1), id(2), id(3)]);
}

#[test]
fn a_dock_and_the_desktop_arent_listed() {
    let mut policy = three();
    let dock = TestWindow::new(4, 1920, 24).with_type(WindowType::Dock);
    policy.map_window(dock, ms(4));
    let desktop = TestWindow::new(5, 1920, 1080).with_type(WindowType::Desktop);
    policy.map_window(desktop, ms(5));

    toggle(&mut policy);

    assert_eq!(listed(&policy), [id(1), id(2), id(3)]);
}

#[test]
fn another_workspaces_window_isnt_listed() {
    let mut policy = three();
    policy.move_to_workspace(id(2), 1);

    toggle(&mut policy);

    assert_eq!(listed(&policy), [id(1), id(3)]);
}

#[test]
fn a_sticky_window_is_listed_on_every_workspace() {
    let mut policy = three();
    policy.set_sticky(id(2), true);

    for workspace in 0..4 {
        policy.switch_workspace(workspace);
        toggle(&mut policy);

        let expected: &[WindowId] = if workspace == 0 {
            &[id(1), id(2), id(3)]
        } else {
            &[id(2)]
        };
        assert_eq!(listed(&policy), expected, "workspace {workspace}");
        toggle(&mut policy);
    }
}

#[test]
fn it_lists_the_windows_of_its_own_output() {
    let mut policy = Policy::new(Config {
        workspaces_per_output: true,
        ..four_workspaces()
    });
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy.add_output(output(2, 1920, 0, 1920, 1080));
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(
        TestWindow::new(2, 400, 300).with_requested_position(2000, 100),
        ms(2),
    );
    policy.activate(SEAT, id(1), ms(3));

    toggle(&mut policy);

    assert_eq!(policy.overview().unwrap().output, OutputId(1));
    assert_eq!(listed(&policy), [id(1)]);
}

#[test]
fn a_modal_dialog_stands_in_for_its_parent_as_in_the_window_switcher() {
    let mut policy = three();
    let dialog = TestWindow::new(4, 200, 100)
        .with_parent(3)
        .with_type(WindowType::ModalDialog);
    policy.map_window(dialog, ms(4));

    toggle(&mut policy);

    assert_eq!(listed(&policy), [id(1), id(2), id(4)]);
}

#[test]
fn a_listed_window_that_goes_leaves_the_list() {
    let mut policy = three();
    toggle(&mut policy);

    policy.unmap_window(id(2));

    assert_eq!(listed(&policy), [id(1), id(3)]);
}

// A choice.

#[test]
fn a_click_on_a_window_activates_it_and_closes_the_overview() {
    let mut policy = three();
    toggle(&mut policy);

    policy.overview_choose(id(1), ms(200));

    assert!(policy.overview().is_none());
    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert_eq!(policy.active_stacking(), [id(2), id(3), id(1)]);
}

#[test]
fn a_click_on_a_minimised_window_unminimises_it() {
    let mut policy = three();
    policy.minimize(id(2));
    toggle(&mut policy);

    policy.overview_choose(id(2), ms(200));

    assert!(policy.overview().is_none());
    assert!(!policy.is_minimized(id(2)));
    assert_eq!(policy.focus(SEAT), Some(id(2)));
}

#[test]
fn a_choice_of_a_window_that_isnt_listed_does_nothing() {
    let mut policy = three();
    policy.move_to_workspace(id(2), 1);
    toggle(&mut policy);

    policy.overview_choose(id(2), ms(200));

    assert!(policy.overview().is_some());
    assert_eq!(policy.active_workspace(), 0);
}

#[test]
fn a_choice_with_the_overview_closed_does_nothing() {
    let mut policy = three();

    policy.overview_choose(id(1), ms(200));

    assert_eq!(policy.focus(SEAT), Some(id(3)));
}

// The Scene.

#[test]
fn opening_and_closing_change_what_the_scene_reads() {
    let mut policy = three();
    policy.take_scene_changed();

    toggle(&mut policy);
    assert!(policy.take_scene_changed());
    assert!(!policy.take_scene_changed());

    toggle(&mut policy);
    assert!(policy.take_scene_changed());
}

// The selection (242).

/// The open Overview's selection.
fn selected(policy: &Policy<TestWindow>) -> Option<WindowId> {
    policy.overview().expect("the Overview is open").selected
}

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Logical> {
    Rectangle::new((x, y).into(), (w, h).into())
}

/// Five windows on one output, 5 focused, laid out in two rows that aren't in the order
/// they're listed in: 3, 1 and 5 on the first row, and 2 and 4 on the second, set off from
/// the first row's columns. 3 is the first row's shortest, and stands on its line as
/// the others do.
fn two_rows() -> (Policy<TestWindow>, Vec<Rectangle<i32, Logical>>) {
    let mut policy = Policy::new(four_workspaces());
    policy.add_output(output(1, 0, 0, 1920, 1080));
    for w in 1..=5 {
        policy.map_window(TestWindow::new(w, 400, 300), ms(w));
    }
    toggle(&mut policy);
    assert_eq!(listed(&policy), [id(1), id(2), id(3), id(4), id(5)]);
    let places = vec![
        rect(150, 0, 100, 100),
        rect(50, 150, 100, 100),
        rect(0, 20, 100, 80),
        rect(210, 150, 100, 100),
        rect(300, 0, 100, 100),
    ];
    (policy, places)
}

/// The selection after `direction` from `from`.
fn after(from: u64, direction: Direction) -> Option<WindowId> {
    let (mut policy, places) = two_rows();
    policy.set_overview_selection(Some(id(from)));
    policy.overview_select(direction, &places);
    selected(&policy)
}

#[test]
fn the_focused_window_is_selected_at_opening() {
    let mut policy = three();

    toggle(&mut policy);

    assert_eq!(selected(&policy), Some(id(3)));
}

#[test]
fn with_no_window_focused_none_is_selected_at_opening() {
    let mut policy = three();
    for w in 1..=3 {
        policy.minimize(id(w));
    }
    assert_eq!(policy.focus(SEAT), None);

    toggle(&mut policy);

    assert_eq!(listed(&policy), [id(1), id(2), id(3)]);
    assert_eq!(selected(&policy), None);
}

#[test]
fn left_and_right_move_along_a_row() {
    assert_eq!(after(1, Direction::Left), Some(id(3)));
    assert_eq!(after(1, Direction::Right), Some(id(5)));
    assert_eq!(after(2, Direction::Right), Some(id(4)));
    assert_eq!(after(4, Direction::Left), Some(id(2)));
}

#[test]
fn up_and_down_move_to_the_nearest_window_of_the_next_row() {
    assert_eq!(after(1, Direction::Down), Some(id(4)));
    assert_eq!(after(3, Direction::Down), Some(id(2)));
    assert_eq!(after(2, Direction::Up), Some(id(3)));
    assert_eq!(after(4, Direction::Up), Some(id(1)));
}

#[test]
fn an_arrow_key_at_the_layouts_edge_keeps_the_selection() {
    assert_eq!(after(3, Direction::Left), Some(id(3)));
    assert_eq!(after(5, Direction::Right), Some(id(5)));
    assert_eq!(after(5, Direction::Up), Some(id(5)));
    assert_eq!(after(4, Direction::Down), Some(id(4)));
    // The second row's first window has the first row's beside it, not to its left.
    assert_eq!(after(2, Direction::Left), Some(id(2)));
}

#[test]
fn left_and_right_dont_leave_the_row() {
    // 1 and 2 on the first row, and 3 alone on the second, centred, so 1 is wholly to its
    // left.
    let mut policy = three();
    toggle(&mut policy);
    let places = [
        rect(0, 0, 100, 100),
        rect(150, 0, 100, 100),
        rect(200, 150, 100, 100),
    ];
    policy.set_overview_selection(Some(id(3)));

    policy.overview_select(Direction::Left, &places);
    assert_eq!(selected(&policy), Some(id(3)));
    policy.overview_select(Direction::Right, &places);
    assert_eq!(selected(&policy), Some(id(3)));
}

#[test]
fn with_none_selected_an_arrow_key_selects_the_first_window_of_the_layout() {
    for direction in [
        Direction::Left,
        Direction::Right,
        Direction::Up,
        Direction::Down,
    ] {
        let (mut policy, places) = two_rows();
        policy.set_overview_selection(None);

        policy.overview_select(direction, &places);

        assert_eq!(selected(&policy), Some(id(3)), "{direction:?}");
    }
}

#[test]
fn enter_chooses_the_selected_window() {
    let mut policy = three();
    policy.minimize(id(1));
    toggle(&mut policy);
    policy.set_overview_selection(Some(id(1)));

    policy.overview_enter(ms(200));

    assert!(policy.overview().is_none());
    assert!(!policy.is_minimized(id(1)));
    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn enter_with_none_selected_does_nothing() {
    let mut policy = three();
    toggle(&mut policy);
    policy.set_overview_selection(None);
    let stacking = policy.active_stacking();

    policy.overview_enter(ms(200));

    assert!(policy.overview().is_some());
    assert_eq!(policy.active_stacking(), stacking);
}

#[test]
fn the_selected_window_going_leaves_none_selected() {
    let mut policy = three();
    toggle(&mut policy);
    policy.set_overview_selection(Some(id(2)));

    policy.unmap_window(id(2));

    assert_eq!(selected(&policy), None);
}

#[test]
fn another_window_going_keeps_the_selection() {
    let mut policy = three();
    toggle(&mut policy);
    policy.set_overview_selection(Some(id(2)));

    policy.unmap_window(id(1));

    assert_eq!(selected(&policy), Some(id(2)));
}

#[test]
fn a_window_that_maps_while_it_is_open_joins_the_list() {
    let mut policy = three();
    toggle(&mut policy);

    policy.map_window(TestWindow::new(4, 400, 300), ms(200));

    assert!(listed(&policy).contains(&id(4)));
    assert!(policy.overview().is_some());
}

// The middle click (242).

#[test]
fn a_middle_click_asks_the_window_to_close_and_the_overview_stays_open() {
    let mut policy = three();
    toggle(&mut policy);

    policy.overview_close_window(id(2));

    assert_eq!(window(&policy, 2).calls.last(), Some(&Call::Close));
    assert!(policy.overview().is_some());
    assert_eq!(listed(&policy), [id(1), id(2), id(3)]);
}

#[test]
fn a_middle_click_on_a_window_that_isnt_listed_does_nothing() {
    let mut policy = three();
    policy.move_to_workspace(id(2), 1);
    toggle(&mut policy);

    policy.overview_close_window(id(2));

    assert!(!window(&policy, 2).calls.contains(&Call::Close));
}

// The bar of workspaces (244).

/// `three` with 2 and 3 on the second workspace, 1 alone on the first.
fn two_on_the_second() -> Policy<TestWindow> {
    let mut policy = three();
    policy.move_to_workspace(id(2), 1);
    policy.move_to_workspace(id(3), 1);
    policy
}

#[test]
fn a_click_on_a_workspace_switches_the_output_and_the_overview_stays_open() {
    let mut policy = two_on_the_second();
    toggle(&mut policy);
    assert_eq!(listed(&policy), [id(1)]);

    policy.overview_switch(1);

    assert_eq!(policy.active_workspace_on(OutputId(1)), 1);
    assert_eq!(policy.interaction(SEAT), Some(Interaction::Overview));
    assert_eq!(listed(&policy), [id(2), id(3)]);
    assert_eq!(policy.overview().unwrap().workspace, 1);
}

#[test]
fn after_a_switch_the_new_workspaces_focused_window_is_selected() {
    let mut policy = two_on_the_second();
    toggle(&mut policy);
    assert_eq!(selected(&policy), Some(id(1)));

    policy.overview_switch(1);

    assert_eq!(policy.focus(SEAT), Some(id(3)));
    assert_eq!(selected(&policy), Some(id(3)));
}

#[test]
fn after_a_switch_to_an_empty_workspace_none_is_selected() {
    let mut policy = three();
    toggle(&mut policy);

    policy.overview_switch(2);

    assert_eq!(listed(&policy), []);
    assert_eq!(selected(&policy), None);
}

#[test]
fn a_click_on_the_active_workspace_keeps_the_selection() {
    let mut policy = three();
    toggle(&mut policy);
    policy.set_overview_selection(Some(id(1)));

    policy.overview_switch(0);

    assert_eq!(selected(&policy), Some(id(1)));
}

#[test]
fn a_click_on_a_workspace_past_the_count_does_nothing() {
    let mut policy = three();
    toggle(&mut policy);

    policy.overview_switch(4);

    assert_eq!(policy.active_workspace(), 0);
    assert!(policy.overview().is_some());
}

#[test]
fn a_click_on_a_workspace_switches_its_output_alone_with_workspaces_per_output() {
    let mut policy = Policy::new(Config {
        workspaces_per_output: true,
        ..four_workspaces()
    });
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy.add_output(output(2, 1920, 0, 1920, 1080));
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    toggle(&mut policy);

    policy.overview_switch(3);

    assert_eq!(policy.active_workspace_on(OutputId(1)), 3);
    assert_eq!(policy.active_workspace_on(OutputId(2)), 0);
}

#[test]
fn a_workspace_binding_switches_the_active_output_with_the_overview_open() {
    let mut policy = two_on_the_second();
    toggle(&mut policy);

    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(200));

    assert_eq!(policy.active_workspace_on(OutputId(1)), 1);
    assert_eq!(listed(&policy), [id(2), id(3)]);
    assert_eq!(selected(&policy), Some(id(3)));
    // The seat has an Interaction, so the Workspace popup doesn't open.
    assert!(!policy.open_workspace_popup(false));
}

#[test]
fn the_windows_of_another_workspace_are_listed_for_its_layout() {
    let mut policy = two_on_the_second();
    toggle(&mut policy);

    assert_eq!(policy.overview_windows_on(1), [id(2), id(3)]);
    assert_eq!(policy.overview_windows_on(0), [id(1)]);
    assert_eq!(policy.overview_windows_on(2), []);
}

#[test]
fn fewer_workspaces_while_it_is_open_list_the_new_active_ones_windows() {
    let mut policy = two_on_the_second();
    policy.switch_workspace(1);
    toggle(&mut policy);
    assert_eq!(listed(&policy), [id(2), id(3)]);

    policy.settings_changed(Config {
        workspace_count: 1,
        ..Config::default()
    });

    let overview = policy.overview().unwrap();
    assert_eq!(overview.workspace, 0);
    assert_eq!(listed(&policy), [id(1), id(2), id(3)]);
}

// A drag onto the bar (245). The default `drag-threshold` is 8.

fn at(x: f64, y: f64) -> Point<f64, Logical> {
    Point::from((x, y))
}

/// The open Overview's drag.
fn drag(policy: &Policy<TestWindow>) -> Option<crate::OverviewDrag> {
    policy.overview().expect("the Overview is open").drag
}

/// `three`, open, with window `w` pressed at (500, 500) and dragged to (600, 50), over the
/// bar's cell of `target` or no cell.
fn dragged(w: u64, target: Option<usize>) -> Policy<TestWindow> {
    let mut policy = three();
    toggle(&mut policy);
    policy.overview_press(id(w), at(500.0, 500.0));
    policy.overview_pointer(at(600.0, 50.0), target);
    policy
}

#[test]
fn a_press_and_a_release_within_the_drag_threshold_is_a_click() {
    let mut policy = three();
    toggle(&mut policy);

    policy.overview_press(id(1), at(500.0, 500.0));
    policy.overview_pointer(at(508.0, 492.0), Some(1));
    assert!(!drag(&policy).unwrap().dragging);
    assert_eq!(drag(&policy).unwrap().target, None);
    policy.overview_release(ms(200));

    assert!(policy.overview().is_none());
    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert_eq!(policy.workspace_of(id(1)), Some(0));
}

#[test]
fn past_the_drag_threshold_the_press_is_a_drag_wherever_the_pointer_goes_after() {
    let mut policy = three();
    toggle(&mut policy);
    policy.overview_press(id(1), at(500.0, 500.0));

    policy.overview_pointer(at(509.0, 500.0), Some(2));
    let moved = drag(&policy).unwrap();
    assert!(moved.dragging);
    assert_eq!(moved.window, id(1));
    assert_eq!(moved.pointer, at(509.0, 500.0));
    assert_eq!(moved.target, Some(2));

    policy.overview_pointer(at(500.0, 500.0), None);
    assert!(drag(&policy).unwrap().dragging);
    assert_eq!(drag(&policy).unwrap().target, None);
}

#[test]
fn a_drop_on_a_cell_moves_the_window_there_and_the_overview_stays_open() {
    let mut policy = dragged(1, Some(2));
    let geometry = policy.geometry(id(1));

    policy.overview_release(ms(200));

    assert_eq!(policy.workspace_of(id(1)), Some(2));
    assert_eq!(policy.geometry(id(1)), geometry);
    assert_eq!(policy.active_workspace(), 0);
    assert_eq!(policy.interaction(SEAT), Some(Interaction::Overview));
    assert_eq!(listed(&policy), [id(2), id(3)]);
    assert_eq!(drag(&policy), None);
}

#[test]
fn a_drop_of_the_focused_window_on_a_cell_keeps_the_selection_among_those_listed() {
    let mut policy = dragged(3, Some(1));

    policy.overview_release(ms(200));

    assert_eq!(policy.workspace_of(id(3)), Some(1));
    assert_eq!(listed(&policy), [id(1), id(2)]);
    assert!(selected(&policy).is_none_or(|selected| listed(&policy).contains(&selected)));
}

#[test]
fn a_drop_on_the_active_workspaces_cell_leaves_the_window_where_it_was() {
    let mut policy = dragged(1, Some(0));

    policy.overview_release(ms(200));

    assert_eq!(policy.workspace_of(id(1)), Some(0));
    assert_eq!(listed(&policy), [id(1), id(2), id(3)]);
    assert!(policy.overview().is_some());
    assert_eq!(policy.focus(SEAT), Some(id(3)));
}

#[test]
fn a_drop_on_nothing_leaves_the_window_where_it_was() {
    let mut policy = dragged(1, None);

    policy.overview_release(ms(200));

    assert_eq!(policy.workspace_of(id(1)), Some(0));
    assert_eq!(listed(&policy), [id(1), id(2), id(3)]);
    assert!(policy.overview().is_some());
    assert_eq!(drag(&policy), None);
}

#[test]
fn a_drop_on_a_workspace_past_the_count_leaves_the_window_where_it_was() {
    let mut policy = dragged(1, Some(4));

    policy.overview_release(ms(200));

    assert_eq!(policy.workspace_of(id(1)), Some(0));
    assert!(policy.overview().is_some());
}

#[test]
fn a_sticky_window_dropped_on_a_cell_is_no_longer_sticky() {
    let mut policy = three();
    policy.set_sticky(id(2), true);
    toggle(&mut policy);
    policy.overview_press(id(2), at(500.0, 500.0));
    policy.overview_pointer(at(600.0, 50.0), Some(3));

    policy.overview_release(ms(200));

    assert!(!policy.is_sticky(id(2)));
    assert_eq!(policy.workspace_of(id(2)), Some(3));
    assert_eq!(listed(&policy), [id(1), id(3)]);
}

#[test]
fn a_sticky_window_dropped_on_the_active_workspaces_cell_stays_sticky() {
    let mut policy = three();
    policy.set_sticky(id(2), true);
    toggle(&mut policy);
    policy.overview_press(id(2), at(500.0, 500.0));
    policy.overview_pointer(at(600.0, 50.0), Some(0));

    policy.overview_release(ms(200));

    assert!(policy.is_sticky(id(2)));
}

#[test]
fn esc_during_a_drag_ends_it_and_the_overview_stays_open() {
    let mut policy = dragged(1, Some(2));

    assert!(policy.overview_end_drag());

    assert_eq!(drag(&policy), None);
    assert!(policy.overview().is_some());
    // The release after is no drop and no click.
    policy.overview_release(ms(200));
    assert_eq!(policy.workspace_of(id(1)), Some(0));
    assert!(policy.overview().is_some());
}

#[test]
fn esc_with_a_press_that_isnt_a_drag_yet_is_no_drag_to_end() {
    let mut policy = three();
    toggle(&mut policy);
    policy.overview_press(id(1), at(500.0, 500.0));

    assert!(!policy.overview_end_drag());

    assert!(drag(&policy).is_some());
}

#[test]
fn closing_the_overview_ends_the_drag() {
    let mut policy = dragged(1, Some(2));

    policy.close_overview();
    toggle(&mut policy);

    assert_eq!(drag(&policy), None);
    assert_eq!(policy.workspace_of(id(1)), Some(0));
}

#[test]
fn the_dragged_window_going_ends_the_drag() {
    let mut policy = dragged(1, Some(2));

    policy.unmap_window(id(1));

    assert_eq!(drag(&policy), None);
    assert!(policy.overview().is_some());
}

#[test]
fn a_press_on_a_window_that_isnt_listed_is_no_press() {
    let mut policy = three();
    policy.move_to_workspace(id(2), 1);
    toggle(&mut policy);

    policy.overview_press(id(2), at(500.0, 500.0));

    assert_eq!(drag(&policy), None);
}
