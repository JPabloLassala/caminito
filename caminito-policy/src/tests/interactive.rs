use super::*;

use crate::ResizeEdges;

fn at(x: f64, y: f64) -> Point<f64, Logical> {
    Point::new(x, y)
}

#[test]
fn a_move_follows_the_pointer() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(100, 100),
        ms(1),
    );

    assert!(policy.begin_move(SEAT, id(1), at(150., 150.)));
    policy.grab_motion(SEAT, at(200.5, 120.), false);
    policy.end_grab(SEAT);

    assert_eq!(policy.geometry(id(1)).unwrap().loc, (151, 70).into());
    let (geometry, _) = window(&policy, 1).last_geometry().unwrap();
    assert_eq!(geometry.loc, (151, 70).into());
}

#[test]
fn a_move_onto_another_output_changes_the_windows_outputs() {
    let mut policy = policy();
    policy.add_output(output(2, 1920, 0, 1280, 1024));
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(100, 100),
        ms(1),
    );

    policy.begin_move(SEAT, id(1), at(150., 150.));
    policy.grab_motion(SEAT, at(2150., 150.), false);

    assert_eq!(window(&policy, 1).last_outputs(), Some(&[OutputId(2)][..]));
}

#[test]
fn maximised_and_fullscreen_windows_cant_be_moved_or_resized() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.set_maximized(id(1), true);
    policy.set_fullscreen(id(2), true, None);

    assert!(!policy.begin_move(SEAT, id(1), at(0., 0.)));
    assert!(!policy.begin_resize(SEAT, id(2), ResizeEdges::BOTTOM_RIGHT, at(0., 0.)));
}

#[test]
fn a_resize_asks_for_the_new_size_within_the_hints() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300)
            .with_requested_position(100, 100)
            .with_hints((200, 200), (600, 0)),
        ms(1),
    );

    assert!(policy.begin_resize(SEAT, id(1), ResizeEdges::BOTTOM_RIGHT, at(500., 400.)));
    policy.grab_motion(SEAT, at(800., 500.), false);

    let (geometry, states) = window(&policy, 1).last_geometry().unwrap();
    assert_eq!(
        geometry,
        Rectangle::new((100, 100).into(), (600, 400).into())
    );
    assert!(states.resizing);

    policy.end_grab(SEAT);
    let (_, states) = window(&policy, 1).last_geometry().unwrap();
    assert!(!states.resizing);
}

#[test]
fn a_resize_from_the_top_left_keeps_the_bottom_right_corner() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(100, 100),
        ms(1),
    );

    policy.begin_resize(SEAT, id(1), ResizeEdges::TOP_LEFT, at(100., 100.));
    policy.grab_motion(SEAT, at(50., 80.), false);
    let (geometry, _) = window(&policy, 1).last_geometry().unwrap();
    assert_eq!(geometry, Rectangle::new((50, 80).into(), (450, 320).into()));

    // The client settles on a smaller size than asked; the corner stays put.
    policy.window_mut(id(1)).unwrap().size = (440, 310).into();
    policy.commit(id(1));
    assert_eq!(
        policy.geometry(id(1)),
        Some(Rectangle::new((60, 90).into(), (440, 310).into()))
    );
}

#[test]
fn unmapping_a_grabbed_window_ends_the_grab() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.begin_move(SEAT, id(1), at(0., 0.));

    policy.unmap_window(id(1));
    policy.grab_motion(SEAT, at(10., 10.), false);

    assert!(policy.grab(SEAT).is_none());
}

#[test]
fn a_grab_ends_when_its_window_is_maximised_hidden_or_moved_away() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.map_window(TestWindow::new(3, 400, 300), ms(3));

    policy.begin_resize(SEAT, id(1), ResizeEdges::BOTTOM_RIGHT, at(0., 0.));
    policy.set_maximized(id(1), true);
    assert_eq!(policy.grab(SEAT), None);
    let (geometry, states) = window(&policy, 1).last_geometry().unwrap();
    assert_eq!(geometry, Rectangle::new((0, 0).into(), (1920, 1080).into()));
    assert!(!states.resizing);

    policy.begin_move(SEAT, id(2), at(0., 0.));
    policy.minimize(id(2));
    assert_eq!(policy.grab(SEAT), None);

    policy.begin_move(SEAT, id(3), at(0., 0.));
    policy.move_to_workspace(id(3), 1);
    assert_eq!(policy.grab(SEAT), None);
}

#[test]
fn a_seat_can_restart_a_grab_on_the_same_window() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.begin_move(SEAT, id(1), at(0., 0.));

    assert!(policy.begin_resize(SEAT, id(1), ResizeEdges::BOTTOM_RIGHT, at(0., 0.)));
    assert!(!policy.begin_move(SeatId(1), id(1), at(0., 0.)));
}

#[test]
fn dragging_the_titlebar_of_a_maximised_window_unmaximises_it_under_the_pointer() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300)
            .with_frame(5, 5, 25, 5)
            .with_requested_position(100, 100),
        ms(1),
    );
    policy.set_maximized(id(1), true);

    // The maximised frame fills the 1920×1080 output; the pointer presses its middle.
    assert!(policy.begin_titlebar_move(SEAT, id(1), at(960., 10.), at(960., 10.)));

    assert!(!policy.is_maximized_horizontally(id(1)));
    assert!(!policy.is_maximized_vertically(id(1)));
    // Restored to its floating size, positioned so the pointer keeps the same half-way
    // point across the (now much narrower) titlebar: frame width 410, so 205 either side
    // of the pointer, minus the frame's 5px left border. The press is as far down the
    // frame as it was down the maximised one, 10 of 1080 and so 3 of 330, not down at
    // the floating geometry's 100.
    let (geometry, _) = window(&policy, 1).last_geometry().unwrap();
    assert_eq!(
        geometry,
        Rectangle::new((760, 32).into(), (400, 300).into())
    );

    // The same grab keeps following the pointer, as an ordinary move does.
    policy.grab_motion(SEAT, at(970., 10.), false);
    let (geometry, _) = window(&policy, 1).last_geometry().unwrap();
    assert_eq!(geometry.loc, (770, 32).into());
}

#[test]
fn a_catch_up_after_crossing_the_threshold_keeps_the_pointers_fraction_across_the_titlebar() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300)
            .with_frame(5, 5, 25, 5)
            .with_requested_position(100, 100),
        ms(1),
    );
    policy.set_maximized(id(1), true);

    // Pressed a quarter of the way across the maximised titlebar (480 of 1920), with the
    // threshold crossed 80px to the right and 20px down from the press. `grab.rs`'s
    // `motion()` applies that same catch-up with a `grab_motion` call right after starting
    // the grab, so it's simulated here too.
    assert!(policy.begin_titlebar_move(SEAT, id(1), at(400., 10.), at(480., 30.)));
    policy.grab_motion(SEAT, at(480., 30.), false);

    assert!(!policy.is_maximized(id(1)));
    let (geometry, _) = window(&policy, 1).last_geometry().unwrap();
    assert_eq!(
        geometry,
        Rectangle::new((383, 52).into(), (400, 300).into())
    );
}

#[test]
fn dragging_the_titlebar_of_a_half_maximised_window_unmaximises_both_axes() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(100, 100),
        ms(1),
    );
    policy.set_maximized_horizontally(id(1), true);
    assert!(policy.is_maximized_horizontally(id(1)));

    assert!(policy.begin_titlebar_move(SEAT, id(1), at(500., 110.), at(500., 110.)));

    assert!(!policy.is_maximized_horizontally(id(1)));
    assert!(!policy.is_maximized_vertically(id(1)));

    // Without unmaximising first, `pin_maximized_axes` would keep the horizontal axis at
    // its maximised size and position for the rest of the grab; now it moves freely too.
    policy.grab_motion(SEAT, at(600., 160.), false);
    let (geometry, _) = window(&policy, 1).last_geometry().unwrap();
    assert_eq!(
        geometry,
        Rectangle::new((496, 150).into(), (400, 300).into())
    );
}

#[test]
fn begin_titlebar_move_leaves_an_ordinary_window_exactly_as_begin_move_would() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(100, 100),
        ms(1),
    );

    assert!(policy.begin_titlebar_move(SEAT, id(1), at(150., 150.), at(150., 150.)));
    policy.grab_motion(SEAT, at(200.5, 120.), false);
    policy.end_grab(SEAT);

    assert_eq!(policy.geometry(id(1)).unwrap().loc, (151, 70).into());
}

#[test]
fn a_move_its_client_asks_for_holds_a_maximised_window_then_unmaximises_it_under_the_pointer() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(100, 500),
        ms(1),
    );
    policy.set_maximized(id(1), true);

    // A client that draws its own titlebar asks for the move at the press, so a click
    // mustn't unmaximise: the window stays until the pointer is twice the drag threshold
    // away, as a tiled one does.
    assert!(policy.begin_requested_move(SEAT, id(1), at(960., 10.)));
    policy.grab_motion(SEAT, at(960., 25.), false);
    assert!(policy.is_maximized(id(1)));
    assert_eq!(policy.grab(SEAT), Some(id(1)));

    // Half of the floating 400 either side of the pointer, and the press 10 of 1080 down
    // it, 3 of 300, then the 16 the pointer has gone.
    policy.grab_motion(SEAT, at(960., 26.), false);
    assert!(!policy.is_maximized_horizontally(id(1)));
    assert!(!policy.is_maximized_vertically(id(1)));
    let (geometry, _) = window(&policy, 1).last_geometry().unwrap();
    assert_eq!(
        geometry,
        Rectangle::new((760, 23).into(), (400, 300).into())
    );

    // Maximised again mid-drag, the grab ends as any other does.
    policy.set_maximized(id(1), true);
    assert_eq!(policy.grab(SEAT), None);
}

#[test]
fn a_requested_move_let_go_of_before_the_threshold_leaves_the_window_maximised() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.set_maximized(id(1), true);

    assert!(policy.begin_requested_move(SEAT, id(1), at(960., 10.)));
    policy.grab_motion(SEAT, at(965., 12.), false);
    policy.end_grab(SEAT);

    assert!(policy.is_maximized(id(1)));
    let (geometry, _) = window(&policy, 1).last_geometry().unwrap();
    assert_eq!(geometry, Rectangle::new((0, 0).into(), (1920, 1080).into()));
}

#[test]
fn a_requested_move_never_starts_on_a_fullscreen_window() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.set_fullscreen(id(1), true, None);

    assert!(!policy.begin_requested_move(SEAT, id(1), at(960., 10.)));
}
