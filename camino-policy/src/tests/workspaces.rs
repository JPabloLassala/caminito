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
