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

/// `config` with `fullscreen-workspaces` as `on` says, as `two_outputs_with_key` has it.
fn key(on: bool) -> Config {
    Config {
        workspaces_per_output: true,
        fullscreen_workspaces: on,
        ..Config::default()
    }
}

#[test]
fn turning_the_key_on_gives_each_fullscreen_window_a_fullscreen_workspace_and_no_output_switches() {
    let mut policy = two_outputs_with_key(false);
    policy.set_fullscreen(id(1), true, None);
    policy.set_fullscreen(id(2), true, None);
    policy.take_output_switches();

    policy.settings_changed(key(true));

    assert_eq!(policy.take_output_switches(), []);
    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), None);
    assert_eq!(active(&policy), (0, 0));
    assert!(!policy.is_showing(id(1)));
    assert!(!policy.is_showing(id(2)));
    assert!(policy.is_showing(id(3)));
    assert!(policy.is_fullscreen(id(1)));
    assert_eq!(policy.focus(SEAT), Some(id(3)));

    // Each is to the right of its Home workspace.
    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(10));
    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), Some(id(1)));
    assert!(policy.is_showing(id(1)));
}

#[test]
fn turning_the_key_off_returns_each_fullscreen_window_to_its_home_workspace_still_fullscreen() {
    let mut policy = on_a_fullscreen_workspace();
    policy.set_fullscreen(id(2), true, None);

    policy.settings_changed(key(false));

    // The output that showed one slides back to its Home workspace.
    assert_eq!(
        policy.take_output_switches(),
        [switch_between(Some(1), None, -1)]
    );
    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), None);
    assert_eq!(active(&policy), (0, 0));
    assert!(policy.is_fullscreen(id(1)));
    assert!(policy.is_fullscreen(id(2)));
    assert!(policy.is_showing(id(1)));
    assert!(policy.is_showing(id(2)));
    assert!(policy.is_showing(id(3)));
    assert_eq!(policy.focus(SEAT), Some(id(1)));

    // Right is the grid's next.
    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(10));
    assert_eq!(active(&policy), (1, 0));
    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), None);
}

#[test]
fn turning_the_key_on_gives_a_fullscreen_transient_no_fullscreen_workspace_of_its_own() {
    let mut policy = two_outputs_with_key(false);
    policy.map_window(
        TestWindow::new(4, 300, 200)
            .with_parent(1)
            .with_requested_position(200, 200),
        ms(10),
    );
    policy.set_fullscreen(id(4), true, None);
    assert!(policy.is_fullscreen(id(4)));

    policy.settings_changed(key(true));

    assert!(policy.is_showing(id(1)));
    assert!(policy.is_showing(id(4)));
    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(11));
    assert_eq!(policy.fullscreen_workspace_on(OutputId(1)), None);
    assert_eq!(active(&policy), (1, 0));
}
