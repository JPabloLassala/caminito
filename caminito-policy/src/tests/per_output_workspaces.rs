//! Per-output workspaces (ADR 0032, 230): each output's active workspace, with
//! `workspaces-per-output` on and off.

use super::*;

fn at(x: f64, y: f64) -> Point<f64, Logical> {
    Point::new(x, y)
}

/// Two outputs side by side, the first the primary, with `workspaces-per-output` as `on`
/// says, window 1 on the left one and window 2 on the right one, both on workspace 0.
fn two_outputs(on: bool) -> Policy<TestWindow> {
    let mut policy = Policy::new(Config {
        workspaces_per_output: on,
        ..Config::default()
    });
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy.add_output(output(2, 1920, 0, 1280, 1024));
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(100, 100),
        ms(1),
    );
    policy.map_window(
        TestWindow::new(2, 400, 300).with_requested_position(2100, 100),
        ms(2),
    );
    policy
}

fn set_per_output(policy: &mut Policy<TestWindow>, on: bool) {
    policy.settings_changed(Config {
        workspaces_per_output: on,
        ..Config::default()
    });
}

fn active(policy: &Policy<TestWindow>) -> (usize, usize) {
    (
        policy.active_workspace_on(OutputId(1)),
        policy.active_workspace_on(OutputId(2)),
    )
}

#[test]
fn a_switch_of_one_output_leaves_the_other_alone() {
    let mut policy = two_outputs(true);

    policy.switch_output_workspace(OutputId(2), 1);

    assert_eq!(active(&policy), (0, 1));
    assert!(policy.is_showing(id(1)));
    assert!(!policy.is_showing(id(2)));
    assert_eq!(window(&policy, 1).last_outputs(), Some(&[OutputId(1)][..]));
    assert_eq!(window(&policy, 2).last_outputs(), Some(&[][..]));
    assert_eq!(policy.active_stacking(), [id(1)]);
}

#[test]
fn a_window_opened_on_the_switched_output_is_on_its_workspace_and_shows() {
    let mut policy = two_outputs(true);
    policy.switch_output_workspace(OutputId(2), 1);
    policy.move_to_workspace(id(2), 1);

    assert!(policy.is_showing(id(1)));
    assert!(policy.is_showing(id(2)));
    assert_eq!(policy.active_stacking(), [id(1), id(2)]);

    policy.switch_output_workspace(OutputId(2), 0);
    assert!(policy.is_showing(id(1)));
    assert!(!policy.is_showing(id(2)));
}

#[test]
fn a_switch_of_one_output_keeps_the_focus_on_the_other() {
    let mut policy = two_outputs(true);
    policy.activate(SEAT, id(1), ms(3));

    policy.switch_output_workspace(OutputId(2), 1);

    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn a_switch_of_the_focused_windows_output_focuses_that_outputs_windows() {
    let mut policy = two_outputs(true);
    policy.move_to_workspace(id(2), 1);
    policy.map_window(
        TestWindow::new(3, 400, 300).with_requested_position(2200, 200),
        ms(3),
    );
    assert_eq!(policy.focus(SEAT), Some(id(3)));

    policy.switch_output_workspace(OutputId(2), 1);
    // Window 1 is on the other output, so it isn't focused for it.
    assert_eq!(policy.focus(SEAT), Some(id(2)));

    policy.switch_output_workspace(OutputId(2), 2);
    assert_eq!(policy.focus(SEAT), None);
}

#[test]
fn switch_to_workspace_prev_goes_by_the_primary_outputs() {
    let mut policy = two_outputs(true);
    policy.switch_output_workspace(OutputId(1), 2);
    policy.switch_output_workspace(OutputId(2), 3);

    policy.action(SEAT, Action::SwitchToWorkspacePrev, ms(3));

    // Every output, to the primary output's previous, until the Active output's (231).
    assert_eq!(active(&policy), (0, 0));
}

#[test]
fn a_window_shows_by_its_main_outputs_workspace() {
    let mut policy = two_outputs(true);
    policy.switch_output_workspace(OutputId(2), 1);
    // On workspace 1, on the left output, which shows 0.
    policy.move_to_workspace(id(1), 1);
    assert!(!policy.is_showing(id(1)));

    // Most of it on the right output, which shows 1, where it's kept. It wasn't showing on
    // the left one, so it keeps its workspace, and shows.
    policy.request_geometry(id(1), Some(Point::from((1800, 100))), None);
    assert_eq!(policy.geometry(id(1)).unwrap().loc.x, 1920);

    assert_eq!(policy.workspace_of(id(1)), Some(1));
    assert!(policy.is_showing(id(1)));
}

#[test]
fn a_window_straddling_two_outputs_is_on_both_while_its_main_output_shows_it() {
    let mut policy = two_outputs(true);
    // 300 of its 400 on the left output.
    policy.begin_move(SEAT, id(1), at(150., 150.));
    policy.grab_motion(SEAT, at(1670., 150.), false);
    policy.end_grab(SEAT);
    assert_eq!(policy.geometry(id(1)).unwrap().loc.x, 1620);
    assert_eq!(
        window(&policy, 1).last_outputs(),
        Some(&[OutputId(1), OutputId(2)][..])
    );

    policy.switch_output_workspace(OutputId(2), 1);
    assert!(policy.is_showing(id(1)));
    assert_eq!(
        window(&policy, 1).last_outputs(),
        Some(&[OutputId(1), OutputId(2)][..])
    );

    policy.switch_output_workspace(OutputId(1), 1);
    policy.switch_output_workspace(OutputId(2), 0);
    assert!(!policy.is_showing(id(1)));
    assert_eq!(window(&policy, 1).last_outputs(), Some(&[][..]));
}

#[test]
fn a_window_dragged_across_joins_the_other_outputs_workspace_once_most_of_it_has_crossed() {
    let mut policy = two_outputs(true);
    policy.switch_output_workspace(OutputId(2), 2);
    assert!(policy.begin_move(SEAT, id(1), at(150., 150.)));

    // To x 1620: 300 of its 400 still on the left output.
    policy.grab_motion(SEAT, at(1670., 150.), false);
    assert_eq!(policy.geometry(id(1)).unwrap().loc.x, 1620);
    assert_eq!(policy.workspace_of(id(1)), Some(0));
    assert!(policy.is_showing(id(1)));

    // To x 1820: 300 of it on the right output.
    policy.grab_motion(SEAT, at(1870., 150.), false);
    assert_eq!(policy.geometry(id(1)).unwrap().loc.x, 1820);
    assert_eq!(policy.workspace_of(id(1)), Some(2));
    assert!(policy.is_showing(id(1)));
    assert_eq!(policy.grab(SEAT), Some(id(1)));
    policy.end_grab(SEAT);

    // It's on the right output's workspace now, and goes with its switches.
    policy.switch_output_workspace(OutputId(2), 0);
    assert!(!policy.is_showing(id(1)));
    assert_eq!(active(&policy), (0, 0));
}

#[test]
fn a_dragged_windows_family_goes_with_it() {
    let mut policy = two_outputs(true);
    policy.map_window(
        TestWindow::new(3, 200, 100)
            .with_type(WindowType::Dialog)
            .with_parent(1)
            .with_requested_position(200, 200),
        ms(3),
    );
    policy.switch_output_workspace(OutputId(2), 1);

    policy.request_geometry(id(1), Some(Point::from((2000, 100))), None);

    assert_eq!(policy.workspace_of(id(1)), Some(1));
    assert_eq!(policy.workspace_of(id(3)), Some(1));
}

#[test]
fn a_sticky_window_dragged_across_stays_sticky() {
    let mut policy = two_outputs(true);
    policy.set_sticky(id(1), true);
    policy.switch_output_workspace(OutputId(2), 1);

    policy.request_geometry(id(1), Some(Point::from((2000, 100))), None);

    assert!(policy.is_sticky(id(1)));
    assert!(policy.is_showing(id(1)));
}

#[test]
fn an_output_change_isnt_a_drag_across() {
    let mut policy = two_outputs(true);
    policy.switch_output_workspace(OutputId(1), 1);
    policy.move_to_workspace(id(1), 1);

    // The outputs swap places: window 1 is on the right output now, by its frame.
    policy.set_outputs(vec![
        output(2, 0, 0, 1280, 1024),
        output(1, 1280, 0, 1920, 1080),
    ]);

    assert_eq!(policy.workspace_of(id(1)), Some(1));
    assert!(policy.is_showing(id(1)));
}

#[test]
fn with_the_key_off_both_outputs_switch_together() {
    let mut policy = two_outputs(false);

    policy.switch_output_workspace(OutputId(2), 1);

    assert_eq!(active(&policy), (1, 1));
    assert!(!policy.is_showing(id(1)));
    assert!(!policy.is_showing(id(2)));

    policy.switch_workspace(3);
    assert_eq!(active(&policy), (3, 3));
}

#[test]
fn with_the_key_off_a_dragged_window_keeps_its_workspace() {
    let mut policy = two_outputs(false);
    policy.begin_move(SEAT, id(1), at(150., 150.));
    policy.grab_motion(SEAT, at(2150., 150.), false);
    policy.end_grab(SEAT);

    assert_eq!(policy.workspace_of(id(1)), Some(0));
    assert!(policy.is_showing(id(1)));
}

#[test]
fn turning_the_key_off_gives_every_output_the_primary_outputs_workspace() {
    let mut policy = two_outputs(true);
    policy.set_primary_output(Some(OutputId(2)));
    policy.switch_output_workspace(OutputId(2), 2);
    policy.move_to_workspace(id(2), 2);
    policy.activate(SEAT, id(1), ms(3));

    set_per_output(&mut policy, false);

    assert_eq!(active(&policy), (2, 2));
    // No window's workspace changes.
    assert_eq!(policy.workspace_of(id(1)), Some(0));
    assert_eq!(policy.workspace_of(id(2)), Some(2));
    assert!(!policy.is_showing(id(1)));
    assert!(policy.is_showing(id(2)));
    // The focus that's hidden goes, to what shows.
    assert_eq!(policy.focus(SEAT), Some(id(2)));

    // And the outputs switch together from there.
    policy.switch_output_workspace(OutputId(1), 0);
    assert_eq!(active(&policy), (0, 0));
}

#[test]
fn turning_the_key_on_keeps_every_outputs_workspace() {
    let mut policy = two_outputs(false);
    policy.switch_workspace(2);

    set_per_output(&mut policy, true);
    assert_eq!(active(&policy), (2, 2));

    policy.switch_output_workspace(OutputId(2), 1);
    assert_eq!(active(&policy), (2, 1));
}

#[test]
fn with_one_output_the_key_changes_nothing() {
    let run = |on: bool| {
        let mut policy = Policy::new(Config {
            workspaces_per_output: on,
            ..Config::default()
        });
        policy.add_output(output(1, 0, 0, 1920, 1080));
        policy.map_window(TestWindow::new(1, 400, 300), ms(1));
        policy.switch_output_workspace(OutputId(1), 1);
        policy.map_window(TestWindow::new(2, 400, 300), ms(2));
        policy.begin_move(SEAT, id(2), at(150., 150.));
        policy.grab_motion(SEAT, at(1500., 600.), false);
        policy.end_grab(SEAT);
        policy.action(SEAT, Action::SwitchToWorkspaceLeft, ms(3));
        (
            policy.active_workspace(),
            policy.workspace_of(id(1)),
            policy.workspace_of(id(2)),
            policy.focus(SEAT),
            policy.active_stacking(),
        )
    };
    assert_eq!(run(true), run(false));
}
