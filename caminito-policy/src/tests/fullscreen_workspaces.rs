//! Fullscreen workspaces (ADR 0057, CMN-304): with `fullscreen-workspaces` on, a focused
//! window that goes fullscreen is on a workspace of its own, right after its Home workspace,
//! which its output switches to, and which goes when the window leaves fullscreen or closes.

use super::per_output_workspaces::active;
use super::*;
use crate::OutputSwitch;

/// Two outputs with `workspaces-per-output` on and `fullscreen-workspaces` as `on` says:
/// windows 1 and 3 on the left one, window 1 focused, and window 2 on the right one.
fn two_outputs_with_key(on: bool) -> Policy<TestWindow> {
    let mut policy = Policy::new(Config {
        workspaces_per_output: true,
        fullscreen_workspaces: on,
        ..Config::default()
    });
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy.add_output(output(2, 1920, 0, 1280, 1024));
    policy.map_window(
        TestWindow::new(2, 400, 300).with_requested_position(2100, 100),
        ms(1),
    );
    policy.map_window(
        TestWindow::new(3, 400, 300).with_requested_position(600, 100),
        ms(2),
    );
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(100, 100),
        ms(3),
    );
    assert_eq!(policy.focus(SEAT), Some(id(1)));
    policy.take_output_switches();
    policy
}

/// `two_outputs_with_key(true)` with window 1 gone fullscreen on its Fullscreen workspace.
fn on_a_fullscreen_workspace() -> Policy<TestWindow> {
    let mut policy = two_outputs_with_key(true);
    policy.set_fullscreen(id(1), true, None);
    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), Some(id(1)));
    policy.take_output_switches();
    policy
}

/// The left output's switch between its Home workspace, 0, and window 1's Fullscreen
/// workspace, `cells` columns to the right.
fn switch_between(from: Option<u64>, to: Option<u64>, cells: i32) -> OutputSwitch {
    OutputSwitch {
        output: OutputId(1),
        from: 0,
        to: 0,
        from_fullscreen: from.map(id),
        to_fullscreen: to.map(id),
        cells: (cells, 0),
    }
}

#[test]
fn a_focused_window_that_goes_fullscreen_is_on_a_fullscreen_workspace_its_output_slides_to() {
    let mut policy = two_outputs_with_key(true);

    policy.set_fullscreen(id(1), true, None);

    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), Some(id(1)));
    assert_eq!(policy.fullscreen_workspace_on(OutputId(2)), None);
    // The Home workspace stays the output's active one, by its number.
    assert_eq!(active(&policy), (0, 0));
    assert!(policy.is_showing(id(1)));
    assert!(!policy.is_showing(id(3)));
    assert!(policy.is_showing(id(2)));
    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert_eq!(policy.workspace_of(id(1)), Some(0));
    assert_eq!(
        policy.take_output_switches(),
        [switch_between(None, Some(1), 1)]
    );
}

#[test]
fn left_from_a_fullscreen_workspace_is_its_home_workspace_and_right_from_there_comes_back() {
    let mut policy = on_a_fullscreen_workspace();

    policy.action(SEAT, Action::SwitchToWorkspaceLeft, ms(10));

    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), None);
    assert_eq!(active(&policy), (0, 0));
    assert!(policy.is_showing(id(3)));
    assert!(!policy.is_showing(id(1)));
    assert_eq!(policy.focus(SEAT), Some(id(3)));
    assert_eq!(
        policy.take_output_switches(),
        [switch_between(Some(1), None, -1)]
    );

    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(11));

    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), Some(id(1)));
    assert!(policy.is_showing(id(1)));
    assert!(!policy.is_showing(id(3)));
    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert_eq!(
        policy.take_output_switches(),
        [switch_between(None, Some(1), 1)]
    );
}

#[test]
fn right_from_a_fullscreen_workspace_is_the_grids_next_after_its_home_workspace() {
    let mut policy = on_a_fullscreen_workspace();

    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(10));

    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), None);
    assert_eq!(active(&policy), (1, 0));
    assert!(!policy.is_showing(id(1)));
    assert_eq!(
        policy.take_output_switches(),
        [OutputSwitch {
            from: 0,
            to: 1,
            ..switch_between(Some(1), None, 1)
        }]
    );

    policy.action(SEAT, Action::SwitchToWorkspaceLeft, ms(11));

    // The grid's left of workspace 1, which is the Home workspace itself.
    assert_eq!(active(&policy), (0, 0));
    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), None);
    assert!(policy.is_showing(id(3)));
}

#[test]
fn a_window_that_leaves_fullscreen_is_on_its_home_workspace_with_the_focus() {
    let mut policy = on_a_fullscreen_workspace();

    policy.set_fullscreen(id(1), false, None);

    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), None);
    assert_eq!(active(&policy), (0, 0));
    assert!(policy.is_showing(id(1)));
    assert!(policy.is_showing(id(3)));
    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert_eq!(
        policy.take_output_switches(),
        [switch_between(Some(1), None, -1)]
    );

    // Its Fullscreen workspace has gone: right is the grid's next.
    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(10));
    assert_eq!(active(&policy), (1, 0));
    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), None);
}

#[test]
fn a_window_that_closes_on_its_fullscreen_workspace_leaves_the_output_on_its_home_workspace() {
    let mut policy = on_a_fullscreen_workspace();

    policy.unmap_window(id(1));

    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), None);
    assert_eq!(active(&policy), (0, 0));
    assert!(policy.is_showing(id(3)));
    assert_eq!(policy.focus(SEAT), Some(id(3)));
    assert_eq!(
        policy.take_output_switches(),
        [switch_between(Some(1), None, -1)]
    );
}

#[test]
fn a_window_that_opens_fullscreen_is_on_a_fullscreen_workspace_its_output_slides_to() {
    let mut policy = two_outputs_with_key(true);

    let states = MapStates {
        fullscreen: Some(None),
        ..MapStates::default()
    };
    policy.map_window_with(
        TestWindow::new(4, 400, 300).with_requested_position(200, 200),
        states,
        ms(10),
    );

    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), Some(id(4)));
    assert!(policy.is_fullscreen(id(4)));
    assert!(policy.is_showing(id(4)));
    assert!(!policy.is_showing(id(1)));
    assert_eq!(policy.focus(SEAT), Some(id(4)));
    assert_eq!(
        policy.take_output_switches(),
        [switch_between(None, Some(4), 1)]
    );
}

#[test]
fn with_the_key_off_a_fullscreen_window_stays_on_its_workspace() {
    let mut policy = two_outputs_with_key(false);

    policy.set_fullscreen(id(1), true, None);

    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), None);
    assert!(policy.is_showing(id(1)));
    assert!(policy.is_showing(id(3)));
    assert_eq!(policy.take_output_switches(), []);
    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(10));
    assert_eq!(active(&policy), (1, 0));
}

#[test]
fn activating_a_window_on_its_fullscreen_workspace_switches_its_output_there() {
    let mut policy = on_a_fullscreen_workspace();
    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(10));
    policy.take_output_switches();

    policy.activate(SEAT, id(1), ms(11));

    assert_eq!(active(&policy), (0, 0));
    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), Some(id(1)));
    assert!(policy.is_showing(id(1)));
    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn under_spanning_workspaces_only_the_windows_output_shows_its_fullscreen_workspace() {
    let mut policy = Policy::new(Config {
        fullscreen_workspaces: true,
        ..Config::default()
    });
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy.add_output(output(2, 1920, 0, 1280, 1024));
    policy.map_window(
        TestWindow::new(2, 400, 300).with_requested_position(2100, 100),
        ms(1),
    );
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(100, 100),
        ms(2),
    );

    policy.set_fullscreen(id(1), true, None);

    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), Some(id(1)));
    assert_eq!(policy.fullscreen_workspace_on(OutputId(2)), None);
    assert!(policy.is_showing(id(2)));

    // A switch to a numbered workspace moves every output, away from it too.
    policy.switch_workspace(1);
    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), None);
    assert_eq!(active(&policy), (1, 1));
}

#[test]
fn a_window_that_goes_fullscreen_without_focus_gets_a_fullscreen_workspace_and_no_switch() {
    let mut policy = two_outputs_with_key(true);

    policy.set_fullscreen(id(3), true, None);

    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), None);
    assert_eq!(policy.take_output_switches(), []);
    assert!(!policy.is_showing(id(3)));
    assert!(policy.is_showing(id(1)));
    assert_eq!(policy.focus(SEAT), Some(id(1)));

    // It's right of the Home workspace.
    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(10));
    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), Some(id(3)));
    assert!(policy.is_showing(id(3)));
}

#[test]
fn moving_a_fullscreen_window_to_a_workspace_makes_that_its_home_workspace() {
    let mut policy = on_a_fullscreen_workspace();

    policy.move_to_workspace(id(1), 2);

    // It stays on its Fullscreen workspace, which the output still shows, now after 2.
    assert_eq!(policy.workspace_of(id(1)), Some(2));
    assert!(policy.is_fullscreen(id(1)));
    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), Some(id(1)));
    assert_eq!(active(&policy), (2, 0));
    assert!(policy.is_showing(id(1)));
    assert_eq!(policy.take_output_switches(), []);

    policy.action(SEAT, Action::SwitchToWorkspaceLeft, ms(10));
    assert_eq!(active(&policy), (2, 0));
    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), None);
    assert!(!policy.is_showing(id(3)));
}

#[test]
fn the_move_binding_takes_a_fullscreen_window_to_another_home_workspace_and_no_switch() {
    let mut policy = on_a_fullscreen_workspace();

    policy.action(SEAT, Action::MoveToWorkspaceRight, ms(10));

    assert_eq!(policy.workspace_of(id(1)), Some(1));
    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), Some(id(1)));
    assert_eq!(active(&policy), (1, 0));
    assert!(policy.is_showing(id(1)));
    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert_eq!(policy.take_output_switches(), []);
}

#[test]
fn a_fullscreen_window_doesnt_keep_its_home_workspace_and_takes_the_last_when_it_goes() {
    let mut policy = Policy::new(Config {
        workspaces_per_output: true,
        fullscreen_workspaces: true,
        dynamic_workspaces: true,
        ..Config::default()
    });
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy.map_window(TestWindow::new(2, 400, 300), ms(1));
    policy.switch_workspace(1);
    policy.map_window(TestWindow::new(1, 400, 300), ms(2));
    policy.switch_workspace(0);
    assert_eq!(policy.workspace_count(), 3);
    assert_eq!(policy.focus(SEAT), Some(id(2)));
    policy.take_output_switches();

    policy.set_fullscreen(id(2), true, None);

    // Workspace 0 had only window 2, and is gone: window 1 closes up onto it, and window 2
    // takes the last, the empty one after it, which the output has as its Home workspace.
    assert_eq!(policy.workspace_count(), 2);
    assert_eq!(policy.workspace_of(id(1)), Some(0));
    assert_eq!(policy.workspace_of(id(2)), Some(1));
    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), Some(id(2)));
    assert_eq!(policy.active_workspace(), 1);
    assert!(policy.is_showing(id(2)));
    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert_eq!(
        policy.take_output_switches(),
        [switch_between(None, Some(2), 1)]
    );

    // Leaving fullscreen, it's on its Home workspace, which it keeps again.
    policy.set_fullscreen(id(2), false, None);
    assert_eq!(policy.workspace_count(), 3);
    assert_eq!(policy.active_workspace(), 1);
    assert!(policy.is_showing(id(2)));
}

#[test]
fn an_unplugged_outputs_fullscreen_workspace_goes_to_the_primary_output_and_stays_there() {
    let mut policy = two_outputs_with_key(true);
    policy.move_to_workspace(id(2), 1);
    policy.activate(SEAT, id(2), ms(10));
    policy.set_fullscreen(id(2), true, None);
    assert_eq!(policy.fullscreen_workspace_on(OutputId(2)), Some(id(2)));
    policy.take_output_switches();

    policy.remove_output(OutputId(2));

    // With its window, after the same-numbered Home workspace, and the output stays where
    // it is.
    assert_eq!(policy.main_output_of(id(2)), Some(OutputId(1)));
    assert_eq!(policy.workspace_of(id(2)), Some(1));
    assert!(policy.is_fullscreen(id(2)));
    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), None);
    assert_eq!(policy.take_output_switches(), []);
    assert!(!policy.is_showing(id(2)));
    assert!(policy.is_showing(id(1)));
    assert_eq!(policy.focus(SEAT), Some(id(1)));
    policy.switch_output_workspace(OutputId(1), 1);
    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(11));
    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), Some(id(2)));
    assert!(policy.is_showing(id(2)));

    // It doesn't return with the output.
    policy.add_output(output(2, 1920, 0, 1280, 1024));
    assert_eq!(policy.main_output_of(id(2)), Some(OutputId(1)));
    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), Some(id(2)));
    assert_eq!(policy.fullscreen_workspace_on(OutputId(2)), None);
    assert!(policy.is_showing(id(2)));
}

#[test]
fn under_spanning_workspaces_the_other_outputs_follow_a_fullscreen_windows_new_home_workspace() {
    let mut policy = Policy::new(Config {
        fullscreen_workspaces: true,
        ..Config::default()
    });
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy.add_output(output(2, 1920, 0, 1280, 1024));
    policy.map_window(
        TestWindow::new(2, 400, 300).with_requested_position(2100, 100),
        ms(1),
    );
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(100, 100),
        ms(2),
    );
    policy.set_fullscreen(id(1), true, None);
    policy.take_output_switches();

    policy.move_to_workspace(id(1), 2);

    assert_eq!(active(&policy), (2, 2));
    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), Some(id(1)));
    assert!(!policy.is_showing(id(2)));
    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert_eq!(
        policy.take_output_switches(),
        [OutputSwitch {
            output: OutputId(2),
            from: 0,
            to: 2,
            from_fullscreen: None,
            to_fullscreen: None,
            cells: (2, 0),
        }]
    );
}
