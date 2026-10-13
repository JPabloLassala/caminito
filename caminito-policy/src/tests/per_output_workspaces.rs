//! Per-output workspaces (ADR 0032, 230): each output's active workspace, with
//! `workspaces-per-output` on and off.

use super::*;
use crate::{Direction, OutputSwitch, WrapStyle};

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
fn turning_the_key_off_gives_every_output_the_active_outputs_workspace() {
    let mut policy = two_outputs(true);
    policy.switch_output_workspace(OutputId(1), 3);
    policy.switch_output_workspace(OutputId(2), 2);
    policy.move_to_workspace(id(2), 2);
    policy.click(SEAT, id(2), ms(3));
    assert_eq!(policy.active_output(SEAT), Some(OutputId(2)));

    set_per_output(&mut policy, false);

    // The Active output's, and not the primary output's.
    assert_eq!(active(&policy), (2, 2));
    // No window's workspace changes.
    assert_eq!(policy.workspace_of(id(1)), Some(0));
    assert_eq!(policy.workspace_of(id(2)), Some(2));
    assert!(!policy.is_showing(id(1)));
    assert!(policy.is_showing(id(2)));
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

// The Active output (231): the focused window's Main output, which the bindings switch.

#[test]
fn a_binding_switches_the_focused_windows_output_and_not_the_other() {
    let mut policy = two_outputs(true);
    // Window 2, on the right output, is focused as the last one mapped.
    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert_eq!(policy.active_output(SEAT), Some(OutputId(2)));

    policy.action(SEAT, Action::SwitchToWorkspace(1), ms(3));
    assert_eq!(active(&policy), (0, 1));
    assert!(policy.is_showing(id(1)));
    assert!(!policy.is_showing(id(2)));

    policy.click(SEAT, id(1), ms(4));
    policy.action(SEAT, Action::SwitchToWorkspace(2), ms(5));
    assert_eq!(active(&policy), (2, 1));
}

#[test]
fn a_binding_switches_to_the_workspace_beside_the_active_outputs() {
    let mut policy = two_outputs(true);
    policy.switch_output_workspace(OutputId(1), 2);

    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(3));

    // Beside the right output's workspace 0, and not the primary output's 2.
    assert_eq!(active(&policy), (2, 1));
}

#[test]
fn a_switch_to_an_empty_workspace_focuses_nothing_and_keeps_the_active_output() {
    let mut policy = two_outputs(true);
    // The pointer is on the left output, which decides nothing.
    policy.set_pointer(at(300., 300.));

    policy.action(SEAT, Action::SwitchToWorkspace(1), ms(3));

    // Window 1 shows on the left output, and isn't focused for it.
    assert_eq!(policy.focus(SEAT), None);
    assert_eq!(policy.active_output(SEAT), Some(OutputId(2)));

    // A second switch acts on the same output.
    policy.action(SEAT, Action::SwitchToWorkspace(2), ms(4));
    assert_eq!(active(&policy), (0, 2));
    assert_eq!(policy.focus(SEAT), None);

    // And back to the top window showing on it.
    policy.action(SEAT, Action::SwitchToWorkspace(0), ms(5));
    assert_eq!(active(&policy), (0, 0));
    assert_eq!(policy.focus(SEAT), Some(id(2)));
}

#[test]
fn switch_to_workspace_prev_goes_back_on_the_active_output_only() {
    let mut policy = two_outputs(true);
    policy.switch_output_workspace(OutputId(1), 2);

    policy.action(SEAT, Action::SwitchToWorkspace(3), ms(3));
    assert_eq!(active(&policy), (2, 3));

    // The right output's previous, 0, and not the left one's.
    policy.action(SEAT, Action::SwitchToWorkspacePrev, ms(4));
    assert_eq!(active(&policy), (2, 0));
    policy.action(SEAT, Action::SwitchToWorkspacePrev, ms(5));
    assert_eq!(active(&policy), (2, 3));
}

#[test]
fn the_focus_stays_off_the_other_output_until_the_user_focuses_a_window_there() {
    let mut policy = two_outputs(true);

    // The focused window goes, and the other output's window isn't focused for it.
    policy.unmap_window(id(2));
    assert_eq!(policy.focus(SEAT), None);
    assert_eq!(policy.active_output(SEAT), Some(OutputId(2)));

    policy.click(SEAT, id(1), ms(3));
    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert_eq!(policy.active_output(SEAT), Some(OutputId(1)));

    policy.action(SEAT, Action::SwitchToWorkspace(1), ms(4));
    assert_eq!(active(&policy), (1, 0));
}

#[test]
fn a_minimised_focused_window_gives_the_focus_to_its_own_outputs_windows() {
    let mut policy = two_outputs(true);
    policy.map_window(
        TestWindow::new(3, 400, 300).with_requested_position(2200, 200),
        ms(3),
    );
    policy.click(SEAT, id(1), ms(4));
    policy.click(SEAT, id(3), ms(5));

    // Window 1 was focused before window 3, and is on the other output.
    policy.minimize(id(3));
    assert_eq!(policy.focus(SEAT), Some(id(2)));

    policy.minimize(id(2));
    assert_eq!(policy.focus(SEAT), None);
    assert_eq!(policy.active_output(SEAT), Some(OutputId(2)));
}

#[test]
fn the_active_output_follows_the_focused_window_across() {
    let mut policy = two_outputs(true);

    policy.request_geometry(id(2), Some(Point::from((200, 200))), None);

    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert_eq!(policy.active_output(SEAT), Some(OutputId(1)));
}

#[test]
fn the_primary_output_stands_in_while_a_seat_has_no_active_output() {
    let mut policy = Policy::<TestWindow>::new(Config {
        workspaces_per_output: true,
        ..Config::default()
    });
    assert_eq!(policy.active_output(SEAT), None);
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy.add_output(output(2, 1920, 0, 1280, 1024));
    policy.set_primary_output(Some(OutputId(2)));
    assert_eq!(policy.active_output(SEAT), Some(OutputId(2)));

    policy.action(SEAT, Action::SwitchToWorkspace(1), ms(1));

    assert_eq!(active(&policy), (0, 1));
}

#[test]
fn the_primary_output_stands_in_once_the_active_output_is_gone() {
    let mut policy = two_outputs(true);
    policy.action(SEAT, Action::SwitchToWorkspace(1), ms(3));
    assert_eq!(policy.active_output(SEAT), Some(OutputId(2)));

    policy.remove_output(OutputId(2));
    assert_eq!(policy.active_output(SEAT), Some(OutputId(1)));

    // It isn't the Active output again for coming back.
    policy.add_output(output(2, 1920, 0, 1280, 1024));
    assert_eq!(policy.active_output(SEAT), Some(OutputId(1)));
}

#[test]
fn the_switch_sound_goes_the_way_of_the_output_that_switched() {
    let mut policy = two_outputs(true);
    policy.switch_output_workspace(OutputId(2), 2);
    policy.move_to_workspace(id(2), 2);
    policy.take_workspace_switches();

    policy.action(SEAT, Action::SwitchToWorkspace(1), ms(3));

    // From the right output's workspace 2, and not from the primary output's 0.
    assert_eq!(policy.take_workspace_switches(), [Direction::Left]);
}

#[test]
fn with_the_key_off_the_bindings_switch_every_output() {
    let mut policy = two_outputs(false);

    policy.action(SEAT, Action::SwitchToWorkspace(1), ms(3));
    assert_eq!(active(&policy), (1, 1));
    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(4));
    assert_eq!(active(&policy), (2, 2));
    policy.action(SEAT, Action::SwitchToWorkspacePrev, ms(5));
    assert_eq!(active(&policy), (1, 1));
}

#[test]
fn with_the_key_off_the_focus_goes_to_a_window_on_any_output() {
    let mut policy = two_outputs(false);

    policy.unmap_window(id(2));

    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

// What the compositor's Workspace slide is told of a switch (233).

/// Four workspaces in two rows, on one output:
///
/// ```text
/// 0 1
/// 2 3
/// ```
fn two_rows(wrap_style: WrapStyle) -> Policy<TestWindow> {
    let mut policy = Policy::new(Config {
        workspace_layout: WorkspaceLayout {
            rows: 2,
            ..WorkspaceLayout::default()
        },
        wrap_style,
        ..Config::default()
    });
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy
}

fn switch(output: u64, from: usize, to: usize, cells: (i32, i32)) -> OutputSwitch {
    OutputSwitch {
        output: OutputId(output),
        from,
        to,
        cells,
    }
}

#[test]
fn a_switch_is_noted_with_the_way_between_the_two_cells() {
    let mut policy = two_rows(WrapStyle::NoWrap);

    policy.action(SEAT, Action::SwitchToWorkspace(3), ms(1));
    assert_eq!(policy.take_output_switches(), [switch(1, 0, 3, (1, 1))]);

    policy.action(SEAT, Action::SwitchToWorkspace(2), ms(2));
    policy.action(SEAT, Action::SwitchToWorkspaceUp, ms(3));
    assert_eq!(
        policy.take_output_switches(),
        [switch(1, 3, 2, (-1, 0)), switch(1, 2, 0, (0, -1))]
    );
    assert_eq!(policy.take_output_switches(), []);
}

#[test]
fn a_switch_that_goes_nowhere_isnt_noted() {
    let mut policy = two_rows(WrapStyle::NoWrap);

    policy.action(SEAT, Action::SwitchToWorkspace(0), ms(1));
    policy.action(SEAT, Action::SwitchToWorkspaceLeft, ms(2));

    assert_eq!(policy.take_output_switches(), []);
}

#[test]
fn a_switch_that_wraps_is_noted_one_cell_the_way_of_its_binding() {
    let mut policy = two_rows(WrapStyle::Toroidal);
    policy.action(SEAT, Action::SwitchToWorkspaceLeft, ms(1));
    assert_eq!(policy.take_output_switches(), [switch(1, 0, 1, (-1, 0))]);

    // On to the next row, which is a column back and a row down in the grid.
    let mut policy = two_rows(WrapStyle::Classic);
    policy.action(SEAT, Action::SwitchToWorkspace(1), ms(1));
    policy.take_output_switches();
    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(2));
    assert_eq!(policy.take_output_switches(), [switch(1, 1, 2, (1, 0))]);
}

#[test]
fn a_window_carried_past_the_grids_edge_notes_its_bindings_way() {
    let mut policy = two_rows(WrapStyle::Toroidal);
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    policy.action(SEAT, Action::MoveToWorkspaceUp, ms(2));

    assert_eq!(policy.take_output_switches(), [switch(1, 0, 2, (0, -1))]);
}

#[test]
fn a_switch_of_one_output_is_noted_for_it_alone() {
    let mut policy = two_outputs(true);

    policy.switch_output_workspace(OutputId(2), 1);

    assert_eq!(policy.take_output_switches(), [switch(2, 0, 1, (1, 0))]);
}

#[test]
fn a_switch_of_every_output_is_noted_for_each_that_changes() {
    let mut policy = two_outputs(true);
    policy.switch_output_workspace(OutputId(2), 2);
    policy.take_output_switches();

    policy.switch_workspace(2);
    assert_eq!(policy.take_output_switches(), [switch(1, 0, 2, (2, 0))]);

    policy.switch_workspace(0);
    assert_eq!(
        policy.take_output_switches(),
        [switch(1, 2, 0, (-2, 0)), switch(2, 2, 0, (-2, 0))]
    );
}

#[test]
fn turning_the_key_off_and_an_output_that_comes_note_no_switch() {
    let mut policy = two_outputs(true);
    policy.switch_output_workspace(OutputId(2), 1);
    policy.take_output_switches();

    set_per_output(&mut policy, false);
    // The right output's, the Active output, where window 2 had the focus.
    assert_eq!(active(&policy), (1, 1));
    policy.add_output(output(3, 3200, 0, 1280, 1024));

    assert_eq!(policy.take_output_switches(), []);
}

#[test]
fn the_output_showing_a_window_is_its_main_output_or_the_primary_one() {
    let policy = two_outputs(true);

    assert_eq!(policy.output_showing(id(1)), Some(OutputId(1)));
    assert_eq!(policy.output_showing(id(2)), Some(OutputId(2)));
    // A window policy doesn't manage is on no output.
    assert_eq!(policy.output_showing(id(9)), Some(OutputId(1)));
}
