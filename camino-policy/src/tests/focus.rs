use super::*;

#[test]
fn mapped_window_is_stacked_on_the_active_workspace_and_focused() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    assert_eq!(policy.active_workspace(), 0);
    assert_eq!(policy.stacking(0), [id(1)]);
    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert!(window(&policy, 1).is_activated());
}

#[test]
fn click_focuses_and_raises() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));

    policy.click(SEAT, id(1), ms(3));

    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert_eq!(policy.stacking(0), [id(2), id(1)]);
    assert!(window(&policy, 1).is_activated());
    assert!(!window(&policy, 2).is_activated());
}

#[test]
fn click_older_than_the_last_focus_change_is_ignored() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(10));

    policy.click(SEAT, id(1), ms(5));

    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert_eq!(policy.stacking(0), [id(1), id(2)]);
}

#[test]
fn each_seat_has_its_own_focus() {
    let mut policy = policy();
    let other = SeatId(1);
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));

    policy.click(other, id(1), ms(3));

    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert_eq!(policy.focus(other), Some(id(1)));
    assert!(window(&policy, 1).is_activated());
    assert!(window(&policy, 2).is_activated());
}

#[test]
fn unmapping_the_focused_window_focuses_the_next_in_stacking_order() {
    let mut policy = policy();
    for w in 1..=3 {
        policy.map_window(TestWindow::new(w, 400, 300), ms(w));
    }
    policy.click(SEAT, id(1), ms(4));

    let unmapped = policy.unmap_window(id(1));

    assert_eq!(unmapped.map(|w| w.id), Some(id(1)));
    assert_eq!(policy.stacking(0), [id(2), id(3)]);
    assert_eq!(policy.focus(SEAT), Some(id(3)));
    assert!(window(&policy, 3).is_activated());
}

#[test]
fn unmapping_an_unfocused_window_keeps_focus() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));

    policy.unmap_window(id(1));

    assert_eq!(policy.focus(SEAT), Some(id(2)));
}

#[test]
fn unmapping_the_last_window_leaves_no_focus() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    policy.unmap_window(id(1));

    assert_eq!(policy.focus(SEAT), None);
    assert_eq!(policy.stacking(0), []);
}

#[test]
fn focus_moves_past_docks_and_settles_for_the_desktop() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 1920, 1080).with_type(WindowType::Desktop),
        ms(1),
    );
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.map_window(
        TestWindow::new(3, 1920, 30).with_type(WindowType::Dock),
        ms(3),
    );
    policy.map_window(TestWindow::new(4, 400, 300), ms(4));

    policy.unmap_window(id(4));
    assert_eq!(policy.focus(SEAT), Some(id(2)));

    policy.unmap_window(id(2));
    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn docks_desktops_and_splash_screens_dont_take_focus_on_map() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(
        TestWindow::new(2, 1920, 1080).with_type(WindowType::Desktop),
        ms(2),
    );
    policy.map_window(
        TestWindow::new(3, 1920, 30).with_type(WindowType::Dock),
        ms(3),
    );
    policy.map_window(
        TestWindow::new(4, 300, 200).with_type(WindowType::Splashscreen),
        ms(4),
    );

    assert_eq!(policy.focus(SEAT), Some(id(1)));
}
