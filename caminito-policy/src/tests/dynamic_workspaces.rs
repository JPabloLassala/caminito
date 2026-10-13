//! `dynamic-workspaces` (151), Marco master's key: workspaces are added and removed as
//! windows come and go, as its `dynamic_workspaces_idle_callback` does (screen.c:1228-1358
//! at Marco `8522ed1`).

use super::*;
use crate::{Direction, MAX_WORKSPACES, OutputSwitch};

/// A policy with one output and `dynamic-workspaces` on, from Marco's four workspaces.
fn dynamic() -> Policy<TestWindow> {
    let mut policy = policy();
    policy.settings_changed(Config {
        dynamic_workspaces: true,
        ..Config::default()
    });
    policy
}

#[test]
fn a_window_opened_on_the_last_empty_workspace_adds_one() {
    let mut policy = dynamic();
    assert_eq!(policy.workspace_count(), 1);

    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    assert_eq!(policy.workspace_count(), 2);
    assert_eq!(policy.workspace_of(id(1)), Some(0));
    assert_eq!(policy.stacking(1), []);
    assert_eq!(policy.workspace_names(), ["Workspace 1", "Workspace 2"]);
}

#[test]
fn the_last_window_leaving_a_workspace_removes_it() {
    let mut policy = dynamic();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.move_to_workspace(id(2), 1);
    assert_eq!(policy.workspace_count(), 3);

    policy.move_to_workspace(id(2), 0);
    assert_eq!(policy.workspace_count(), 2);

    policy.unmap_window(id(1));
    assert_eq!(policy.workspace_count(), 2);
    policy.unmap_window(id(2));
    assert_eq!(policy.workspace_count(), 1);
}

#[test]
fn the_windows_after_an_emptied_workspace_move_down_with_the_active_workspace_and_focus() {
    let mut policy = dynamic();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.switch_workspace(1);
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.switch_workspace(2);
    policy.map_window(TestWindow::new(3, 400, 300), ms(3));
    assert_eq!(policy.workspace_count(), 4);

    policy.unmap_window(id(1));

    assert_eq!(policy.workspace_count(), 3);
    assert_eq!(policy.stacking(0), [id(2)]);
    assert_eq!(policy.stacking(1), [id(3)]);
    assert_eq!(policy.stacking(2), []);
    assert_eq!(policy.active_workspace(), 1);
    assert_eq!(policy.focus(SEAT), Some(id(3)));
    assert_eq!(window(&policy, 3).last_outputs(), Some(&[OutputId(1)][..]));
}

#[test]
fn an_emptied_active_workspace_shows_the_windows_of_the_next_as_in_marco() {
    let mut policy = dynamic();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.switch_workspace(1);
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.map_window(TestWindow::new(3, 400, 300), ms(3));
    policy.move_to_workspace(id(3), 2);

    policy.unmap_window(id(2));

    assert_eq!(policy.workspace_count(), 3);
    assert_eq!(policy.active_workspace(), 1);
    assert_eq!(policy.stacking(1), [id(3)]);
    assert_eq!(policy.focus(SEAT), Some(id(3)));
}

#[test]
fn the_active_workspace_removed_activates_the_first() {
    let mut policy = dynamic();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.move_to_workspace(id(2), 1);
    // The empty workspace after the others.
    policy.switch_workspace(2);
    assert_eq!(policy.focus(SEAT), None);

    policy.unmap_window(id(2));

    assert_eq!(policy.workspace_count(), 2);
    assert_eq!(policy.active_workspace(), 0);
    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn the_active_workspace_removed_is_a_switch_to_the_first() {
    let mut policy = dynamic();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.move_to_workspace(id(2), 1);
    policy.switch_workspace(2);
    policy.take_workspace_switches();

    policy.unmap_window(id(2));

    assert_eq!(policy.active_workspace(), 0);
    assert_eq!(policy.take_workspace_switches(), [Direction::Left]);
}

#[test]
fn the_active_workspace_following_its_windows_down_is_no_switch() {
    let mut policy = dynamic();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.move_to_workspace(id(2), 1);
    policy.switch_workspace(1);
    policy.take_workspace_switches();

    policy.unmap_window(id(1));

    assert_eq!(policy.active_workspace(), 0);
    assert_eq!(policy.take_workspace_switches(), []);
}

// What the compositor's Workspace slide is told of it (236).

#[test]
fn the_active_workspace_removed_is_noted_for_its_output_as_a_switch_to_the_first() {
    let mut policy = dynamic();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.move_to_workspace(id(2), 1);
    policy.switch_workspace(2);
    policy.take_output_switches();

    policy.unmap_window(id(2));

    let to_the_first = OutputSwitch {
        output: OutputId(1),
        from: 2,
        to: 0,
        cells: (-2, 0),
    };
    assert_eq!(policy.take_output_switches(), [to_the_first]);
}

#[test]
fn the_active_workspace_following_its_windows_down_is_noted_for_no_output() {
    let mut policy = dynamic();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.move_to_workspace(id(2), 1);
    policy.switch_workspace(1);
    policy.take_output_switches();

    policy.unmap_window(id(1));

    assert_eq!(policy.active_workspace(), 0);
    assert_eq!(policy.take_output_switches(), []);
}

#[test]
fn a_workspace_removed_is_noted_for_the_output_that_showed_it_alone() {
    let mut policy = policy();
    policy.add_output(output(2, 1920, 0, 1280, 1024));
    policy.settings_changed(Config {
        dynamic_workspaces: true,
        workspaces_per_output: true,
        ..Config::default()
    });
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.move_to_workspace(id(2), 1);
    // The second output shows the empty workspace after the others.
    policy.switch_output_workspace(OutputId(2), 2);
    policy.take_output_switches();

    policy.unmap_window(id(2));

    assert_eq!(policy.active_workspace_on(OutputId(1)), 0);
    assert_eq!(policy.active_workspace_on(OutputId(2)), 0);
    let to_the_first = OutputSwitch {
        output: OutputId(2),
        from: 2,
        to: 0,
        cells: (-2, 0),
    };
    assert_eq!(policy.take_output_switches(), [to_the_first]);
}

#[test]
fn the_count_stays_within_1_and_36() {
    let mut policy = dynamic();
    for n in 1..=40 {
        policy.map_window(TestWindow::new(n, 400, 300), ms(n));
        policy.move_to_workspace(id(n), (n - 1) as usize);
    }
    assert_eq!(policy.workspace_count(), MAX_WORKSPACES);
    // With no empty workspace left, the 37th to 40th go to the last there is.
    assert_eq!(policy.workspace_of(id(40)), Some(35));

    for n in 1..=40 {
        policy.unmap_window(id(n));
    }
    assert_eq!(policy.workspace_count(), 1);
}

#[test]
fn sticky_windows_and_docks_leave_a_workspace_empty() {
    let mut policy = dynamic();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_type(WindowType::Dock),
        ms(1),
    );
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.set_sticky(id(2), true);
    policy.map_window(TestWindow::new(3, 400, 300), ms(3));
    assert_eq!(policy.workspace_count(), 2);

    policy.unmap_window(id(3));

    assert_eq!(policy.workspace_count(), 1);
}

#[test]
fn num_workspaces_is_left_unread_while_the_key_is_on() {
    let mut policy = dynamic();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    policy.settings_changed(Config {
        workspace_count: 6,
        dynamic_workspaces: true,
        ..Config::default()
    });

    assert_eq!(policy.workspace_count(), 2);
}

#[test]
fn turning_the_key_off_keeps_the_count_and_on_again_trims_it() {
    let mut policy = dynamic();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    // Caminito writes the count back to `num-workspaces`, which is what comes with the key
    // turned off.
    let off = Config {
        workspace_count: 2,
        ..Config::default()
    };
    policy.settings_changed(off.clone());
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.move_to_workspace(id(2), 1);
    assert_eq!(policy.workspace_count(), 2);

    policy.unmap_window(id(1));
    assert_eq!(policy.workspace_count(), 2);
    assert_eq!(policy.workspace_of(id(2)), Some(1));

    policy.settings_changed(Config {
        dynamic_workspaces: true,
        ..off
    });
    assert_eq!(policy.workspace_count(), 2);
    assert_eq!(policy.workspace_of(id(2)), Some(0));
}

#[test]
fn names_stay_with_their_number() {
    let mut policy = dynamic();
    policy.settings_changed(Config {
        workspace_names: vec!["Web".into(), "Mail".into(), "Chat".into()],
        dynamic_workspaces: true,
        ..Config::default()
    });
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.switch_workspace(1);
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    assert_eq!(policy.workspace_names(), ["Web", "Mail", "Chat"]);

    policy.unmap_window(id(1));

    // Marco keeps a name with its number, not with the windows of its workspace.
    assert_eq!(policy.workspace_names(), ["Web", "Mail"]);
    assert_eq!(policy.stacking(0), [id(2)]);
}
