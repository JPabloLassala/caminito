use super::*;

use crate::test_window::Call;

#[test]
fn close_asks_the_focused_window_to_close() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));

    policy.action(SEAT, Action::Close, ms(3));

    assert_eq!(window(&policy, 2).calls.last(), Some(&Call::Close));
    assert!(!window(&policy, 1).calls.contains(&Call::Close));
    assert_eq!(policy.stacking(0), [id(1), id(2)]);
}

#[test]
fn switch_windows_goes_back_to_the_previously_used_window() {
    let mut policy = policy();
    for w in 1..=3 {
        policy.map_window(TestWindow::new(w, 400, 300), ms(w));
    }
    policy.click(SEAT, id(1), ms(4));

    policy.action(SEAT, Action::SwitchWindows, ms(5));
    assert_eq!(policy.focus(SEAT), Some(id(3)));
    assert_eq!(policy.stacking(0).last(), Some(&id(3)));

    policy.action(SEAT, Action::SwitchWindows, ms(6));
    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn switch_windows_restores_a_minimised_window_and_skips_docks_and_other_workspaces() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.minimize(id(2));
    policy.map_window(TestWindow::new(3, 400, 300), ms(3));
    policy.move_to_workspace(id(3), 1);
    policy.map_window(
        TestWindow::new(4, 1920, 30).with_type(WindowType::Dock),
        ms(4),
    );
    policy.click(SEAT, id(4), ms(5));
    policy.click(SEAT, id(1), ms(6));
    // Most recently used first: 1, 4, 3, 2.

    policy.action(SEAT, Action::SwitchWindows, ms(7));

    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert!(!policy.is_minimized(id(2)));
}

#[test]
fn close_and_maximise_leave_the_desktop_and_docks_alone() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 1920, 1080).with_type(WindowType::Desktop),
        ms(1),
    );
    policy.map_window(
        TestWindow::new(2, 1920, 30).with_type(WindowType::Dock),
        ms(2),
    );
    policy.click(SEAT, id(1), ms(2));
    assert_eq!(policy.focus(SEAT), Some(id(1)));

    policy.action(SEAT, Action::Close, ms(3));
    policy.action(SEAT, Action::ToggleMaximized, ms(4));
    policy.click(SEAT, id(2), ms(5));
    policy.action(SEAT, Action::Close, ms(6));
    policy.action(SEAT, Action::ToggleMaximized, ms(7));

    for w in 1..=2 {
        assert!(!window(&policy, w).calls.contains(&Call::Close));
        assert!(!policy.is_maximized(id(w)));
    }
}

#[test]
fn close_asks_a_window_to_close_unless_its_the_desktop_or_a_dock() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(
        TestWindow::new(2, 1920, 30).with_type(WindowType::Dock),
        ms(2),
    );

    policy.close(id(1));
    policy.close(id(2));

    assert_eq!(window(&policy, 1).calls.last(), Some(&Call::Close));
    assert!(!window(&policy, 2).calls.contains(&Call::Close));
}
