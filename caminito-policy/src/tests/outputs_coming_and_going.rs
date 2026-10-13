//! Outputs coming and going with per-output workspaces, and the one workspace count every
//! output has (ADR 0032, 225).

use super::per_output_workspaces::{active, two_outputs};
use super::*;

/// The right output of `two_outputs`.
fn right() -> Output {
    output(2, 1920, 0, 1280, 1024)
}

/// Maps window `n` on the right output of `two_outputs`, and puts it on `workspace`.
fn map_on_the_right(policy: &mut Policy<TestWindow>, n: u64, workspace: usize) {
    map_at(policy, n, 2000 + 20 * n as i32, workspace);
}

/// Maps window `n` at `x`, and puts it on `workspace`.
fn map_at(policy: &mut Policy<TestWindow>, n: u64, x: i32, workspace: usize) {
    policy.map_window(
        TestWindow::new(n, 400, 300).with_requested_position(x, 100),
        ms(n),
    );
    policy.move_to_workspace(id(n), workspace);
}

/// `two_outputs`, with windows 2 to 4 on the first three workspaces of the right output,
/// which shows the second, and window 5 on the left output's second, which it doesn't show.
fn windows_on_three_workspaces(on: bool) -> Policy<TestWindow> {
    let mut policy = two_outputs(on);
    map_on_the_right(&mut policy, 3, 1);
    map_on_the_right(&mut policy, 4, 2);
    map_at(&mut policy, 5, 300, 1);
    policy.activate(SEAT, id(1), ms(10));
    policy.switch_output_workspace(OutputId(2), 1);
    policy
}

#[test]
fn the_windows_of_an_output_that_goes_keep_their_numbers_on_the_primary_output() {
    let mut policy = windows_on_three_workspaces(true);
    assert_eq!(active(&policy), (0, 1));

    policy.remove_output(OutputId(2));

    for (window, workspace) in [(2, 0), (3, 1), (4, 2)] {
        assert_eq!(policy.workspace_of(id(window)), Some(workspace));
        assert_eq!(policy.main_output_of(id(window)), Some(OutputId(1)));
    }
    // The primary output shows what it did, and the newcomer of that number.
    assert_eq!(policy.active_workspace_on(OutputId(1)), 0);
    assert_eq!(policy.active_stacking(), [id(2), id(1)]);
    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert_eq!(policy.stacking(1), [id(3), id(5)]);
    assert_eq!(policy.stacking(2), [id(4)]);
}

#[test]
fn the_output_added_again_shows_workspace_1_and_no_window_moves_back() {
    let mut policy = windows_on_three_workspaces(true);
    // The left output shows another workspace than the first, which the one that comes
    // doesn't take.
    policy.switch_output_workspace(OutputId(1), 2);
    policy.remove_output(OutputId(2));
    let places: Vec<_> = (1..=5).map(|n| policy.geometry(id(n)).unwrap()).collect();

    policy.add_output(right());

    assert_eq!(active(&policy), (2, 0));
    for (n, place) in (1..=5).zip(places) {
        assert_eq!(policy.geometry(id(n)).unwrap(), place);
        assert_eq!(policy.main_output_of(id(n)), Some(OutputId(1)));
    }
    for (window, workspace) in [(1, 0), (2, 0), (3, 1), (4, 2), (5, 1)] {
        assert_eq!(policy.workspace_of(id(window)), Some(workspace));
    }
}

#[test]
fn a_new_output_starts_on_workspace_1_whatever_the_others_show() {
    let mut policy = two_outputs(true);
    // Every output's, as ext-workspace's `activate` still switches them.
    policy.switch_workspace(2);

    policy.add_output(output(3, 3200, 0, 1920, 1080));

    assert_eq!(active(&policy), (2, 2));
    assert_eq!(policy.active_workspace_on(OutputId(3)), 0);
    // And it has no previous workspace to go back to.
    policy.press_on_output(SEAT, OutputId(3));
    policy.action(SEAT, Action::SwitchToWorkspacePrev, ms(3));
    assert_eq!(policy.active_workspace_on(OutputId(3)), 0);
}

#[test]
fn an_output_that_changes_keeps_its_workspace() {
    let mut policy = two_outputs(true);
    policy.switch_output_workspace(OutputId(2), 2);

    policy.add_output(output(2, 1920, 0, 2560, 1440));

    assert_eq!(active(&policy), (0, 2));
}

#[test]
fn the_only_output_keeps_its_workspace_through_going_and_coming_back() {
    let mut policy = Policy::<TestWindow>::new(Config {
        workspaces_per_output: true,
        ..Config::default()
    });
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.switch_output_workspace(OutputId(1), 1);
    policy.switch_output_workspace(OutputId(1), 2);

    policy.remove_output(OutputId(1));
    assert_eq!(policy.active_workspace(), 2);

    // As another monitor in its place, too.
    policy.add_output(output(7, 0, 0, 1920, 1080));
    assert_eq!(policy.active_workspace_on(OutputId(7)), 2);
    assert!(!policy.is_showing(id(1)));
    policy.action(SEAT, Action::SwitchToWorkspacePrev, ms(2));
    assert_eq!(policy.active_workspace_on(OutputId(7)), 1);
}

#[test]
fn every_output_replaced_at_once_keeps_the_primary_outputs_workspace() {
    let mut policy = two_outputs(true);
    policy.switch_output_workspace(OutputId(1), 2);
    policy.switch_output_workspace(OutputId(2), 1);

    policy.set_outputs(vec![output(3, 0, 0, 1920, 1080)]);

    assert_eq!(policy.active_workspace_on(OutputId(3)), 2);
}

#[test]
fn the_active_output_that_goes_leaves_the_primary_output_the_active_one() {
    let mut policy = two_outputs(true);
    policy.activate(SEAT, id(2), ms(3));
    assert_eq!(policy.active_output(SEAT), Some(OutputId(2)));

    policy.remove_output(OutputId(2));

    assert_eq!(policy.active_output(SEAT), Some(OutputId(1)));
    // Window 2 is on the workspace the primary output shows, and keeps the focus.
    assert_eq!(policy.focus(SEAT), Some(id(2)));
    policy.action(SEAT, Action::SwitchToWorkspace(1), ms(4));
    assert_eq!(policy.active_workspace_on(OutputId(1)), 1);
}

#[test]
fn the_focus_of_an_output_that_goes_is_the_primary_outputs_when_its_window_is_put_away() {
    let mut policy = two_outputs(true);
    map_on_the_right(&mut policy, 3, 1);
    policy.switch_output_workspace(OutputId(2), 1);
    policy.activate(SEAT, id(3), ms(4));

    policy.remove_output(OutputId(2));

    // The primary output shows workspace 0, where window 3 isn't.
    assert!(!policy.is_showing(id(3)));
    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert_eq!(policy.active_output(SEAT), Some(OutputId(1)));
}

#[test]
fn a_seat_with_no_focus_on_the_output_that_goes_gets_the_primary_outputs() {
    let mut policy = two_outputs(true);
    policy.activate(SEAT, id(2), ms(3));
    policy.move_to_workspace(id(2), 1);
    policy.action(SEAT, Action::SwitchToWorkspace(3), ms(4));
    assert_eq!(policy.focus(SEAT), None);
    assert_eq!(policy.active_output(SEAT), Some(OutputId(2)));

    policy.remove_output(OutputId(2));

    assert_eq!(policy.active_output(SEAT), Some(OutputId(1)));
    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn a_seat_with_no_focus_keeps_none_when_another_output_goes() {
    let mut policy = two_outputs(true);
    policy.activate(SEAT, id(1), ms(3));
    policy.action(SEAT, Action::SwitchToWorkspace(3), ms(4));
    assert_eq!(policy.focus(SEAT), None);

    policy.remove_output(OutputId(2));

    // Window 2 came to the primary output's workspace 0, which it doesn't show.
    assert_eq!(policy.active_workspace_on(OutputId(1)), 3);
    assert_eq!(policy.focus(SEAT), None);
}

#[test]
fn a_sticky_window_of_an_output_that_goes_is_on_every_workspace_of_the_primary_output() {
    let mut policy = two_outputs(true);
    policy.set_sticky(id(2), true);
    policy.switch_output_workspace(OutputId(1), 2);

    policy.remove_output(OutputId(2));

    assert!(policy.is_sticky(id(2)));
    assert!(policy.is_showing(id(2)));
    assert!(!policy.is_showing(id(1)));
}

// With the key off, as before.

#[test]
fn with_the_key_off_an_output_added_takes_the_workspace_the_others_show() {
    let mut policy = windows_on_three_workspaces(false);
    assert_eq!(active(&policy), (1, 1));

    policy.remove_output(OutputId(2));
    for (window, workspace) in [(1, 0), (2, 0), (3, 1), (4, 2), (5, 1)] {
        assert_eq!(policy.workspace_of(id(window)), Some(workspace));
    }
    assert_eq!(policy.active_stacking(), [id(3), id(5)]);

    policy.add_output(right());
    assert_eq!(active(&policy), (1, 1));
    policy.add_output(output(3, 3200, 0, 1920, 1080));
    assert_eq!(policy.active_workspace_on(OutputId(3)), 1);
}

#[test]
fn with_the_key_off_the_only_output_keeps_its_workspace_through_going_and_coming_back() {
    let mut policy = policy();
    policy.switch_workspace(2);

    policy.remove_output(OutputId(1));
    policy.add_output(output(1, 0, 0, 1920, 1080));

    assert_eq!(policy.active_workspace_on(OutputId(1)), 2);
}

// The count.

fn with_count(policy: &Policy<TestWindow>, workspace_count: usize) -> Config {
    Config {
        workspace_count,
        workspaces_per_output: policy.workspaces_per_output(),
        ..Config::default()
    }
}

#[test]
fn a_smaller_count_clamps_each_outputs_workspace_and_every_window() {
    let mut policy = two_outputs(true);
    map_on_the_right(&mut policy, 3, 3);
    map_at(&mut policy, 4, 300, 2);
    policy.switch_output_workspace(OutputId(1), 2);
    policy.switch_output_workspace(OutputId(1), 3);
    policy.switch_output_workspace(OutputId(2), 1);
    assert_eq!(active(&policy), (3, 1));

    policy.settings_changed(with_count(&policy, 3));

    assert_eq!(active(&policy), (2, 1));
    assert_eq!(policy.workspace_of(id(3)), Some(2));
    assert_eq!(policy.workspace_of(id(4)), Some(2));
    assert!(policy.is_showing(id(4)));
    assert!(!policy.is_showing(id(3)));

    policy.settings_changed(with_count(&policy, 1));

    assert_eq!(active(&policy), (0, 0));
    for n in 1..=4 {
        assert_eq!(policy.workspace_of(id(n)), Some(0));
        assert!(policy.is_showing(id(n)));
    }
}

#[test]
fn a_smaller_count_keeps_each_outputs_previous_workspace_that_is_left() {
    let mut policy = two_outputs(true);
    // The left output's previous workspace is 2, which goes, and the right one's is 0.
    policy.switch_output_workspace(OutputId(1), 2);
    policy.switch_output_workspace(OutputId(1), 3);
    policy.switch_output_workspace(OutputId(2), 1);

    policy.settings_changed(with_count(&policy, 2));
    assert_eq!(active(&policy), (1, 1));

    policy.press_on_output(SEAT, OutputId(2));
    policy.action(SEAT, Action::SwitchToWorkspacePrev, ms(5));
    assert_eq!(active(&policy), (1, 0));
    // The left one's is forgotten, as the one output's is (150).
    policy.press_on_output(SEAT, OutputId(1));
    policy.action(SEAT, Action::SwitchToWorkspacePrev, ms(6));
    assert_eq!(active(&policy), (1, 0));
}

#[test]
fn with_the_key_off_a_smaller_count_clamps_the_one_workspace() {
    let mut policy = two_outputs(false);
    map_on_the_right(&mut policy, 3, 3);
    policy.switch_workspace(3);

    policy.settings_changed(with_count(&policy, 2));

    assert_eq!(active(&policy), (1, 1));
    assert_eq!(policy.workspace_of(id(3)), Some(1));
}

// `dynamic-workspaces`.

/// Two outputs with `dynamic-workspaces` on and `workspaces-per-output` as `on` says:
/// windows 1 to 3 on the first three workspaces of the left one, and window 4 on the right
/// one's first.
fn dynamic(on: bool) -> Policy<TestWindow> {
    let mut policy = Policy::new(Config {
        dynamic_workspaces: true,
        workspaces_per_output: on,
        ..Config::default()
    });
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy.add_output(right());
    for n in 1..=3 {
        map_at(&mut policy, n, 100 * n as i32, n as usize - 1);
    }
    map_on_the_right(&mut policy, 4, 0);
    policy
}

#[test]
fn dynamic_workspaces_keep_one_count_for_every_output() {
    let policy = dynamic(true);

    // Three in use, by the left output alone, and the empty one after them.
    assert_eq!(policy.workspace_count(), 4);
    assert_eq!(policy.workspace_names().len(), 4);
}

#[test]
fn a_workspace_emptied_on_one_output_stays_while_another_has_a_window_there() {
    let mut policy = dynamic(true);
    map_on_the_right(&mut policy, 5, 1);
    policy.switch_output_workspace(OutputId(1), 2);
    policy.switch_output_workspace(OutputId(2), 2);

    policy.unmap_window(id(2));

    assert_eq!(policy.workspace_count(), 4);
    assert_eq!(policy.workspace_of(id(3)), Some(2));
    assert_eq!(active(&policy), (2, 2));
}

#[test]
fn a_workspace_empty_on_every_output_goes_and_every_output_is_renumbered() {
    let mut policy = dynamic(true);
    map_on_the_right(&mut policy, 5, 1);
    // The left output shows window 3's workspace, and the right one the empty one of that
    // number, with the one before as its previous.
    policy.switch_output_workspace(OutputId(1), 2);
    policy.switch_output_workspace(OutputId(2), 2);
    policy.unmap_window(id(2));

    policy.unmap_window(id(5));

    assert_eq!(policy.workspace_count(), 3);
    assert_eq!(policy.workspace_of(id(3)), Some(1));
    assert_eq!(active(&policy), (1, 1));
    assert!(policy.is_showing(id(3)));
    // The right output's previous workspace is the first still.
    policy.press_on_output(SEAT, OutputId(2));
    policy.action(SEAT, Action::SwitchToWorkspacePrev, ms(20));
    assert_eq!(active(&policy), (1, 0));
}

#[test]
fn a_window_on_any_output_keeps_its_workspaces_number_in_use() {
    let mut policy = dynamic(true);
    map_on_the_right(&mut policy, 5, 3);
    assert_eq!(policy.workspace_count(), 5);

    // The right output's window on the fourth is alone there, after two that are empty on
    // its own output.
    policy.unmap_window(id(3));

    assert_eq!(policy.workspace_count(), 4);
    assert_eq!(policy.workspace_of(id(5)), Some(2));
}

#[test]
fn a_workspace_removed_under_another_output_leaves_the_focus_where_it_is() {
    let mut policy = dynamic(true);
    // Window 4, on the right output's first workspace, is above every other.
    policy.set_above(id(4), true);
    // The right output shows the empty workspace after the others.
    policy.activate(SEAT, id(4), ms(10));
    policy.action(SEAT, Action::SwitchToWorkspace(3), ms(11));
    assert_eq!(active(&policy), (0, 3));
    policy.activate(SEAT, id(1), ms(12));

    // The count is one less, so the right output's workspace is removed, and it shows
    // the first, with window 4.
    policy.unmap_window(id(3));

    assert_eq!(policy.workspace_count(), 3);
    assert_eq!(active(&policy), (0, 0));
    assert!(policy.is_showing(id(4)));
    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn a_workspace_removed_under_the_active_output_gives_its_seat_that_outputs_focus() {
    let mut policy = dynamic(true);
    policy.set_above(id(1), true);
    policy.activate(SEAT, id(4), ms(10));
    policy.action(SEAT, Action::SwitchToWorkspace(3), ms(11));
    assert_eq!(policy.focus(SEAT), None);

    policy.unmap_window(id(3));

    assert_eq!(active(&policy), (0, 0));
    // Not window 1, the top one, which is on the other output.
    assert_eq!(policy.focus(SEAT), Some(id(4)));
}

#[test]
fn with_the_key_off_a_workspace_removed_gives_every_seat_the_default_focus() {
    let mut policy = dynamic(false);
    policy.set_above(id(4), true);
    policy.switch_workspace(3);
    assert_eq!(policy.focus(SEAT), None);

    policy.unmap_window(id(3));

    assert_eq!(active(&policy), (0, 0));
    assert_eq!(policy.focus(SEAT), Some(id(4)));
}
