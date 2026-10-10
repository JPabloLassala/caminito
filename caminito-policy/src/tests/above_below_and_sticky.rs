//! Keeping a window above or below the others, and sticky windows, which are on every
//! workspace (155).

use super::*;
use crate::SwitchList;

fn dock(id: u64) -> TestWindow {
    TestWindow::new(id, 1920, 30).with_type(WindowType::Dock)
}

/// Two normal windows, 2 on top and focused.
fn two_windows() -> Policy<TestWindow> {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy
}

// Layering.

#[test]
fn a_window_kept_above_stays_over_a_normal_window_that_is_raised() {
    let mut policy = two_windows();
    policy.set_above(id(1), true);

    policy.click(SEAT, id(2), ms(3));

    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert_eq!(policy.stacking(0), [id(2), id(1)]);
    assert_eq!(policy.layer(id(1)), Some(Layer::Top));
}

#[test]
fn keeping_a_window_above_or_letting_it_go_raises_it() {
    let mut policy = two_windows();
    policy.set_above(id(1), true);
    policy.map_window(dock(3), ms(3));
    // The dock is mapped over it, in the same layer.
    assert_eq!(policy.stacking(0), [id(2), id(1), id(3)]);

    policy.set_above(id(1), false);
    assert_eq!(policy.stacking(0), [id(2), id(1), id(3)]);

    policy.set_above(id(1), true);
    assert_eq!(policy.stacking(0), [id(2), id(3), id(1)]);
}

#[test]
fn a_window_kept_below_stays_under_the_normal_windows_when_it_is_raised() {
    let mut policy = two_windows();
    policy.set_below(id(2), true);
    assert_eq!(policy.stacking(0), [id(2), id(1)]);

    policy.click(SEAT, id(2), ms(3));

    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert_eq!(policy.stacking(0), [id(2), id(1)]);
    assert_eq!(policy.layer(id(2)), Some(Layer::Bottom));

    policy.set_below(id(2), false);
    assert_eq!(policy.layer(id(2)), Some(Layer::Normal));
}

#[test]
fn a_dock_kept_below_goes_under_the_normal_windows_and_stays_over_the_desktop() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 1920, 1080).with_type(WindowType::Desktop),
        ms(1),
    );
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.map_window(dock(3), ms(3));

    policy.set_below(id(3), true);

    assert_eq!(policy.stacking(0), [id(1), id(3), id(2)]);
}

#[test]
fn a_transient_of_a_window_kept_above_goes_over_it() {
    let mut policy = two_windows();
    policy.set_above(id(1), true);

    policy.map_window(TestWindow::new(3, 200, 100).with_parent(1), ms(3));
    policy.click(SEAT, id(2), ms(4));

    assert_eq!(policy.stacking(0), [id(2), id(1), id(3)]);
    assert_eq!(policy.layer(id(3)), Some(Layer::Top));
}

#[test]
fn a_transient_kept_below_stays_over_its_parent() {
    let mut policy = two_windows();
    policy.map_window(TestWindow::new(3, 200, 100).with_parent(1), ms(3));

    policy.set_below(id(3), true);
    policy.click(SEAT, id(2), ms(4));

    assert_eq!(policy.stacking(0), [id(1), id(3), id(2)]);
    assert_eq!(policy.layer(id(3)), Some(Layer::Normal));
}

#[test]
fn a_focused_fullscreen_window_goes_over_one_kept_above_and_an_unfocused_one_under_it() {
    let mut policy = two_windows();
    policy.set_above(id(1), true);
    policy.set_fullscreen(id(2), true, None);
    policy.click(SEAT, id(2), ms(3));
    assert_eq!(policy.stacking(0), [id(1), id(2)]);

    policy.click(SEAT, id(1), ms(4));

    assert_eq!(policy.stacking(0), [id(2), id(1)]);
}

#[test]
fn a_fullscreen_window_kept_below_stays_below_while_it_has_the_focus() {
    let mut policy = two_windows();
    policy.set_fullscreen(id(2), true, None);
    policy.set_below(id(2), true);

    policy.click(SEAT, id(2), ms(3));

    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert_eq!(policy.stacking(0), [id(2), id(1)]);
}

#[test]
fn toggle_above_keeps_the_focused_window_above_and_lets_it_go() {
    let mut policy = two_windows();

    policy.action(SEAT, Action::ToggleAbove, ms(3));
    assert!(policy.is_above(id(2)));
    policy.click(SEAT, id(1), ms(4));
    assert_eq!(policy.stacking(0), [id(1), id(2)]);

    policy.click(SEAT, id(2), ms(5));
    policy.action(SEAT, Action::ToggleAbove, ms(6));
    assert!(!policy.is_above(id(2)));
}

// Sticky windows.

#[test]
fn a_sticky_window_shows_on_every_workspace_in_its_place() {
    let mut policy = two_windows();
    policy.set_sticky(id(1), true);
    let geometry = window(&policy, 1).last_geometry();

    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(3));

    assert!(policy.is_showing(id(1)) && !policy.is_showing(id(2)));
    assert_eq!(policy.stacking(1), [id(1)]);
    assert_eq!(window(&policy, 1).last_outputs(), Some(&[OutputId(1)][..]));
    assert_eq!(window(&policy, 1).last_geometry(), geometry);
    // The switch gave it the focus, as the only window there.
    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert!(policy.is_sticky(id(1)));
}

#[test]
fn a_sticky_window_raised_on_one_workspace_is_raised_on_the_others() {
    let mut policy = two_windows();
    policy.set_sticky(id(1), true);
    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(3));
    policy.map_window(TestWindow::new(3, 400, 300), ms(4));
    policy.click(SEAT, id(1), ms(5));

    policy.action(SEAT, Action::SwitchToWorkspaceLeft, ms(6));

    assert_eq!(policy.stacking(1), [id(3), id(1)]);
    assert_eq!(policy.stacking(0), [id(2), id(1)]);
}

#[test]
fn a_new_window_is_placed_beside_a_sticky_window_on_any_workspace() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.set_sticky(id(1), true);
    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(2));

    policy.map_window(TestWindow::new(2, 400, 300), ms(3));

    let (first, _) = window(&policy, 1).last_geometry().unwrap();
    let (second, _) = window(&policy, 2).last_geometry().unwrap();
    assert!(!first.overlaps(second), "{second:?} is clear of {first:?}");
}

#[test]
fn activating_a_sticky_window_stays_on_the_active_workspace() {
    let mut policy = two_windows();
    policy.set_sticky(id(1), true);
    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(3));
    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(4));

    policy.activate(SEAT, id(1), ms(5));

    assert_eq!(policy.active_workspace(), 2);
    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn unsticking_leaves_a_window_on_the_active_workspace() {
    let mut policy = two_windows();
    policy.set_sticky(id(1), true);
    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(3));

    policy.set_sticky(id(1), false);

    assert_eq!(policy.workspace_of(id(1)), Some(1));
    assert!(policy.is_showing(id(1)));
    policy.action(SEAT, Action::SwitchToWorkspaceLeft, ms(4));
    assert!(!policy.is_showing(id(1)));
}

#[test]
fn toggle_on_all_workspaces_sticks_the_focused_window_and_unsticks_it() {
    let mut policy = two_windows();

    policy.action(SEAT, Action::ToggleOnAllWorkspaces, ms(3));
    assert!(policy.is_sticky(id(2)));
    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(4));
    assert!(policy.is_showing(id(2)));

    policy.action(SEAT, Action::ToggleOnAllWorkspaces, ms(5));
    assert!(!policy.is_sticky(id(2)));
    assert_eq!(policy.workspace_of(id(2)), Some(1));
}

#[test]
fn moving_a_sticky_window_to_a_workspace_unsticks_it() {
    let mut policy = two_windows();
    policy.set_sticky(id(2), true);

    policy.move_to_workspace(id(2), 3);

    assert!(!policy.is_sticky(id(2)) && !policy.is_showing(id(2)));
    assert_eq!(policy.workspace_of(id(2)), Some(3));
}

#[test]
fn move_to_workspace_down_unsticks_the_window_it_moves() {
    let mut policy = two_windows();
    policy.settings_changed(Config {
        workspace_layout: WorkspaceLayout {
            rows: 2,
            ..WorkspaceLayout::default()
        },
        ..Config::default()
    });
    policy.set_sticky(id(2), true);

    policy.action(SEAT, Action::MoveToWorkspaceDown, ms(3));

    assert!(!policy.is_sticky(id(2)));
    assert_eq!(policy.workspace_of(id(2)), Some(2));
    assert_eq!(policy.active_workspace(), 2);
}

#[test]
fn a_sticky_window_takes_its_family_along() {
    let mut policy = two_windows();
    policy.map_window(TestWindow::new(3, 200, 100).with_parent(1), ms(3));

    // Stuck from the dialog, the family goes from its root.
    policy.set_sticky(id(3), true);
    assert!(policy.is_sticky(id(1)) && policy.is_sticky(id(3)));

    // A transient mapped for it later is sticky too.
    policy.map_window(TestWindow::new(4, 200, 100).with_parent(1), ms(4));
    assert!(policy.is_sticky(id(4)));
    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(5));
    assert_eq!(policy.stacking(1), [id(1), id(3), id(4)]);

    policy.set_sticky(id(1), false);
    assert!(!policy.is_sticky(id(3)) && !policy.is_sticky(id(4)));
    assert_eq!(policy.workspace_of(id(3)), Some(1));
}

#[test]
fn the_window_switcher_lists_a_sticky_window_on_every_workspace_once() {
    let mut policy = two_windows();
    policy.set_sticky(id(1), true);
    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(3));
    policy.map_window(TestWindow::new(3, 400, 300), ms(4));

    assert_eq!(
        policy.switch_list(SEAT, SwitchList::Windows),
        [id(3), id(1)]
    );
    assert_eq!(
        policy.switch_list(SEAT, SwitchList::WindowsAll),
        [id(3), id(1), id(2)]
    );
}

#[test]
fn showing_the_desktop_hides_a_sticky_window_only_where_it_is_shown() {
    let mut policy = two_windows();
    policy.set_sticky(id(1), true);
    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(3));

    policy.action(SEAT, Action::ShowDesktop, ms(4));
    assert!(!policy.is_showing(id(1)));
    assert_eq!(policy.focus(SEAT), None);

    // The first workspace doesn't show its desktop, so it shows there.
    policy.action(SEAT, Action::SwitchToWorkspaceLeft, ms(5));
    assert!(policy.is_showing(id(1)));
    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(6));
    assert!(!policy.is_showing(id(1)));

    // Stopping brings it back with the focus it had.
    policy.action(SEAT, Action::ShowDesktop, ms(7));
    assert!(policy.is_showing(id(1)));
}

#[test]
fn a_window_mapped_where_the_desktop_shows_minimises_a_sticky_window_there() {
    let mut policy = two_windows();
    policy.set_sticky(id(1), true);
    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(3));
    policy.action(SEAT, Action::ShowDesktop, ms(4));

    policy.map_window(TestWindow::new(3, 400, 300), ms(5));

    assert!(policy.is_minimized(id(1)));
    assert!(!policy.is_minimized(id(2)));
    assert!(policy.is_showing(id(3)));
}

#[test]
fn a_sticky_dock_s_reserved_edges_bound_every_workspace_s_work_area() {
    let mut policy = policy();
    policy.map_window(dock(1), ms(1));
    policy.set_sticky(id(1), true);
    // What the compositor reserves for the dock's strut (64).
    policy.set_outputs(vec![reserving(output(1, 0, 0, 1920, 1080), 0, 0, 30, 0)]);
    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(2));
    policy.map_window(TestWindow::new(2, 400, 300), ms(3));

    policy.set_maximized(id(2), true);

    assert!(policy.is_showing(id(1)));
    assert_eq!(
        window(&policy, 2).last_geometry().unwrap().0,
        Rectangle::new((0, 30).into(), (1920, 1050).into())
    );
    assert_eq!(
        policy.work_areas().collect::<Vec<_>>(),
        [Rectangle::new((0, 30).into(), (1920, 1050).into())]
    );
}

#[test]
fn a_sticky_window_leaves_a_dynamic_workspace_empty_and_stays_sticky() {
    let mut policy = policy();
    policy.settings_changed(Config {
        dynamic_workspaces: true,
        ..Config::default()
    });
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.set_sticky(id(1), true);

    assert_eq!(policy.workspace_count(), 1);
    assert!(policy.is_sticky(id(1)) && policy.is_showing(id(1)));
}
