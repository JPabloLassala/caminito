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
    policy.grab_motion(SEAT, at(200.5, 120.));
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
    policy.grab_motion(SEAT, at(2150., 150.));

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
    policy.grab_motion(SEAT, at(800., 500.));

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
    policy.grab_motion(SEAT, at(50., 80.));
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
    policy.grab_motion(SEAT, at(10., 10.));

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
