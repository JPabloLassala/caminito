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

#[test]
fn utility_toolbar_menu_and_splash_windows_dont_take_focus_on_map() {
    // Marco's `window_state_on_map` (window.c:2031-2044), with docks and desktops.
    for window_type in [
        WindowType::Utility,
        WindowType::Toolbar,
        WindowType::Menu,
        WindowType::Splashscreen,
        WindowType::Dock,
        WindowType::Desktop,
    ] {
        let mut policy = policy();
        policy.map_window(TestWindow::new(1, 400, 300), ms(1));

        policy.map_window(TestWindow::new(2, 200, 100).with_type(window_type), ms(2));

        assert_eq!(policy.focus(SEAT), Some(id(1)), "{window_type:?}");
    }
}

#[test]
fn a_click_raises_a_dock_but_doesnt_focus_it() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 1920, 30).with_type(WindowType::Dock),
        ms(1),
    );
    policy.map_window(
        TestWindow::new(2, 1920, 30)
            .with_type(WindowType::Dock)
            .with_requested_position(0, 1050),
        ms(2),
    );
    policy.map_window(TestWindow::new(3, 400, 300), ms(3));

    policy.click(SEAT, id(1), ms(4));

    assert_eq!(policy.focus(SEAT), Some(id(3)));
    assert_eq!(policy.stacking(0), [id(3), id(2), id(1)]);
}

#[test]
fn a_window_that_takes_no_input_isnt_focused_when_it_maps_but_is_on_click() {
    // Marco's `window_state_on_map` (window.c:1997-2001).
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    policy.map_window(TestWindow::new(2, 200, 100).without_input(), ms(2));
    assert_eq!(policy.focus(SEAT), Some(id(1)));

    policy.click(SEAT, id(2), ms(3));
    assert_eq!(policy.focus(SEAT), Some(id(2)));
}

#[test]
fn the_recently_focused_windows_are_those_on_screen_from_the_focused_one() {
    let mut policy = policy();
    for w in 1..=4 {
        policy.map_window(TestWindow::new(w, 400, 300), ms(w));
    }
    policy.map_window(
        TestWindow::new(5, 1920, 30).with_type(WindowType::Dock),
        ms(5),
    );
    // A click doesn't give a dock the focus.
    policy.click(SEAT, id(5), ms(6));
    policy.click(SEAT, id(2), ms(7));
    // Most recently focused first: 2, 4, 3, 1.
    policy.minimize(id(3));
    policy.move_to_workspace(id(4), 1);

    let recent: Vec<_> = policy.recently_focused().collect();

    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert_eq!(recent, [id(2), id(1)]);
}
