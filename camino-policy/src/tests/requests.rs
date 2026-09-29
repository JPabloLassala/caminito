//! What a client asks of its window's geometry and stacking, as X11 clients do through
//! `ConfigureRequest` and `_NET_WM_STATE` (11).

use super::*;

fn geometry(policy: &Policy<TestWindow>, w: u64) -> Rectangle<i32, Logical> {
    window(policy, w)
        .last_geometry()
        .expect("window was placed")
        .0
}

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Logical> {
    Rectangle::new((x, y).into(), (w, h).into())
}

#[test]
fn a_floating_window_goes_where_it_asks_at_the_size_it_asks() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    policy.request_geometry(id(1), Some((100, 200).into()), Some((500, 400).into()));

    assert_eq!(geometry(&policy, 1), rect(100, 200, 500, 400));
}

#[test]
fn a_request_for_only_a_size_or_only_a_position_keeps_the_rest() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(10, 20),
        ms(1),
    );

    policy.request_geometry(id(1), None, Some((500, 400).into()));
    assert_eq!(geometry(&policy, 1), rect(10, 20, 500, 400));

    policy.request_geometry(id(1), Some((30, 40).into()), None);
    assert_eq!(geometry(&policy, 1), rect(30, 40, 500, 400));
}

#[test]
fn a_request_is_clamped_to_the_work_area_frame_and_all() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300).with_frame(5, 5, 25, 5), ms(1));

    policy.request_geometry(id(1), Some((1800, -50).into()), None);

    // The frame's right edge at 1920 and its top at 0.
    assert_eq!(geometry(&policy, 1), rect(1920 - 5 - 400, 25, 400, 300));
}

#[test]
fn a_request_for_neither_leaves_a_window_off_the_work_area_where_it_is() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(1800, 50),
        ms(1),
    );

    policy.request_geometry(id(1), None, None);

    assert_eq!(geometry(&policy, 1), rect(1800, 50, 400, 300));
}

#[test]
fn a_requested_size_is_kept_within_the_size_hints() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_hints((200, 100), (600, 500)),
        ms(1),
    );

    policy.request_geometry(id(1), None, Some((1000, 50).into()));

    assert_eq!(geometry(&policy, 1).size, (600, 100).into());
}

#[test]
fn maximised_or_fullscreen_windows_refuse_requests() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.map_window(TestWindow::new(3, 400, 300), ms(3));
    policy.set_maximized(id(1), true);
    policy.set_maximized_vertically(id(2), true);
    policy.set_fullscreen(id(3), true, None);
    let before: Vec<_> = (1..=3).map(|w| geometry(&policy, w)).collect();

    for w in 1..=3 {
        policy.request_geometry(id(w), Some((10, 10).into()), Some((100, 100).into()));
    }

    let after: Vec<_> = (1..=3).map(|w| geometry(&policy, w)).collect();
    assert_eq!(after, before);
}

#[test]
fn a_request_is_ignored_while_the_window_is_moved_or_resized() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    let before = geometry(&policy, 1);
    policy.begin_move(SEAT, id(1), (0., 0.).into());

    policy.request_geometry(id(1), Some((10, 10).into()), Some((100, 100).into()));

    assert_eq!(geometry(&policy, 1), before);
}

#[test]
fn above_below_and_sticky_are_recorded() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    policy.set_above(id(1), true);
    policy.set_below(id(1), true);
    policy.set_sticky(id(1), true);
    assert!(policy.is_above(id(1)) && policy.is_below(id(1)) && policy.is_sticky(id(1)));

    policy.set_above(id(1), false);
    assert!(!policy.is_above(id(1)));
    // They take effect in M8 (17).
    assert_eq!(policy.layer(id(1)), Some(Layer::Normal));
}
