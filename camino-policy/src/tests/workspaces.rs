use super::*;

#[test]
fn switching_workspace_hides_the_old_windows_and_focuses_the_new_ones() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(2));
    policy.map_window(TestWindow::new(2, 400, 300), ms(3));
    assert_eq!(policy.workspace_of(id(2)), Some(1));
    assert_eq!(window(&policy, 1).last_outputs(), Some(&[][..]));

    policy.action(SEAT, Action::SwitchToWorkspaceLeft, ms(4));

    assert_eq!(policy.active_workspace(), 0);
    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert_eq!(window(&policy, 1).last_outputs(), Some(&[OutputId(1)][..]));
    assert_eq!(window(&policy, 2).last_outputs(), Some(&[][..]));
    assert!(!window(&policy, 2).is_activated());
}

#[test]
fn switching_to_an_empty_workspace_leaves_no_focus() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(2));

    assert_eq!(policy.focus(SEAT), None);
    assert!(!window(&policy, 1).is_activated());
}

#[test]
fn workspace_switching_doesnt_wrap() {
    let mut policy = policy();
    policy.action(SEAT, Action::SwitchToWorkspaceLeft, ms(1));
    assert_eq!(policy.active_workspace(), 0);

    for t in 2..10 {
        policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(t));
    }
    assert_eq!(policy.active_workspace(), 3);
}

#[test]
fn a_moved_window_takes_its_transient_family_along() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 200, 100).with_parent(1), ms(2));
    policy.map_window(TestWindow::new(3, 200, 100).with_parent(2), ms(3));
    policy.map_window(TestWindow::new(4, 400, 300), ms(4));

    policy.move_to_workspace(id(2), 2);

    assert_eq!(policy.stacking(0), [id(4)]);
    assert_eq!(policy.stacking(2), [id(1), id(2), id(3)]);
    assert_eq!(policy.focus(SEAT), Some(id(4)));
}

#[test]
fn a_transient_maps_on_its_parents_workspace() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.move_to_workspace(id(1), 1);

    policy.map_window(TestWindow::new(2, 200, 100).with_parent(1), ms(2));

    assert_eq!(policy.workspace_of(id(2)), Some(1));
    assert_eq!(policy.focus(SEAT), None);
}

#[test]
fn an_adopted_transient_moves_to_its_parents_workspace() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(2, 200, 100).with_parent(1), ms(1));
    policy.switch_workspace(1);

    policy.map_window(TestWindow::new(1, 400, 300), ms(2));

    assert_eq!(policy.stacking(1), [id(1), id(2)]);
    assert_eq!(policy.stacking(0), []);
}

#[test]
fn activating_a_window_switches_to_its_workspace() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.move_to_workspace(id(1), 3);

    policy.activate(SEAT, id(1), ms(3));

    assert_eq!(policy.active_workspace(), 3);
    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

fn workspaces(count: usize) -> Config {
    Config {
        workspace_count: count,
        workspace_names: (1..=count).map(|n| format!("Desk {n}")).collect(),
    }
}

#[test]
fn a_larger_workspace_count_adds_empty_workspaces_at_the_end() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.move_to_workspace(id(1), 3);

    policy.settings_changed(workspaces(6));

    assert_eq!(policy.workspace_count(), 6);
    assert_eq!(policy.stacking(3), [id(1)]);
    assert_eq!(policy.stacking(5), []);
    policy.switch_workspace(5);
    assert_eq!(policy.active_workspace(), 5);
}

#[test]
fn a_smaller_workspace_count_moves_windows_to_the_new_last_workspace() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.map_window(TestWindow::new(3, 400, 300), ms(3));
    policy.move_to_workspace(id(2), 2);
    policy.move_to_workspace(id(3), 3);
    policy.move_to_workspace(id(1), 1);

    policy.settings_changed(workspaces(2));

    assert_eq!(policy.workspace_count(), 2);
    assert_eq!(policy.stacking(1), [id(1), id(2), id(3)]);
    assert_eq!(policy.workspace_of(id(3)), Some(1));
    assert_eq!(policy.active_workspace(), 0);
}

#[test]
fn the_active_workspace_follows_its_windows_when_it_is_removed() {
    let mut policy = policy();
    policy.switch_workspace(3);
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    policy.settings_changed(workspaces(2));

    assert_eq!(policy.active_workspace(), 1);
    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert_eq!(window(&policy, 1).last_outputs(), Some(&[OutputId(1)][..]));
}

#[test]
fn windows_on_a_kept_active_workspace_stay_put_and_focused() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    policy.settings_changed(workspaces(1));

    assert_eq!(policy.active_workspace(), 0);
    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn workspace_names_follow_the_settings() {
    let mut policy = policy();

    policy.settings_changed(workspaces(3));

    assert_eq!(policy.workspace_names(), ["Desk 1", "Desk 2", "Desk 3"]);
}

#[test]
fn a_window_that_asks_for_another_workspace_maps_there_without_focus() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    policy.map_window(
        TestWindow::new(2, 400, 300).with_requested_workspace(2),
        ms(2),
    );

    assert_eq!(policy.workspace_of(id(2)), Some(2));
    assert_eq!(policy.active_workspace(), 0);
    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert!(!window(&policy, 2).is_activated());
    assert_eq!(window(&policy, 2).last_outputs(), Some(&[][..]));
}

#[test]
fn a_window_that_asks_for_a_workspace_that_doesnt_exist_maps_on_the_active_one() {
    let mut policy = policy();

    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_workspace(9),
        ms(1),
    );

    assert_eq!(policy.workspace_of(id(1)), Some(0));
    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn a_transient_that_asks_for_another_workspace_maps_on_its_parents() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    policy.map_window(
        TestWindow::new(2, 200, 100)
            .with_parent(1)
            .with_requested_workspace(3),
        ms(2),
    );

    assert_eq!(policy.workspace_of(id(2)), Some(0));
    assert_eq!(policy.focus(SEAT), Some(id(2)));
}
