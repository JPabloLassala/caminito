//! `begin-move` and `begin-resize`'s keyboard grab (156), as Marco's
//! `process_keyboard_move_grab` and `process_keyboard_resize_grab` have it: the arrows step
//! the window by 10 pixels, or 1 with Ctrl, Shift snaps it, Escape puts it back and any other
//! key puts it down, with the pointer warped to the window as it goes.

use super::*;

use crate::{GrabKey, GrabStep, ResizeEdges};

fn at(x: f64, y: f64) -> Point<f64, Logical> {
    Point::new(x, y)
}

const RIGHT: GrabKey = arrow(1, 0, GrabStep::Normal);
const LEFT: GrabKey = arrow(-1, 0, GrabStep::Normal);
const UP: GrabKey = arrow(0, -1, GrabStep::Normal);
const DOWN: GrabKey = arrow(0, 1, GrabStep::Normal);

const fn arrow(x: i32, y: i32, step: GrabStep) -> GrabKey {
    GrabKey::Arrow { x, y, step }
}

/// A policy with a focused 400×300 window at 100,100.
fn with_a_window() -> Policy<TestWindow> {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(100, 100),
        ms(1),
    );
    policy
}

/// What policy last asked of the window.
fn asked(policy: &Policy<TestWindow>) -> Rectangle<i32, Logical> {
    window(policy, 1).last_geometry().expect("was configured").0
}

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Logical> {
    Rectangle::new((x, y).into(), (w, h).into())
}

#[test]
fn a_keyboard_move_steps_the_window_ten_pixels_with_each_arrow() {
    let mut policy = with_a_window();
    assert!(policy.begin_keyboard_move(SEAT, id(1)));
    assert_eq!(policy.keyboard_grab(SEAT), Some(id(1)));

    policy.grab_key(SEAT, RIGHT);
    assert_eq!(asked(&policy).loc, (110, 100).into());
    policy.grab_key(SEAT, DOWN);
    assert_eq!(asked(&policy).loc, (110, 110).into());
    policy.grab_key(SEAT, LEFT);
    assert_eq!(asked(&policy).loc, (100, 110).into());
    policy.grab_key(SEAT, UP);
    assert_eq!(asked(&policy).loc, (100, 100).into());
    // A keypad diagonal goes both ways.
    policy.grab_key(SEAT, arrow(1, 1, GrabStep::Normal));
    assert_eq!(asked(&policy).loc, (110, 110).into());
}

#[test]
fn ctrl_steps_a_keyboard_move_one_pixel() {
    let mut policy = with_a_window();
    policy.begin_keyboard_move(SEAT, id(1));

    policy.grab_key(SEAT, arrow(1, 0, GrabStep::Fine));
    policy.grab_key(SEAT, arrow(0, -1, GrabStep::Fine));

    assert_eq!(asked(&policy).loc, (101, 99).into());
}

#[test]
fn a_keyboard_move_warps_the_pointer_to_the_middle_of_the_window_as_it_goes() {
    let mut policy = with_a_window();
    policy.begin_keyboard_move(SEAT, id(1));
    assert_eq!(policy.take_pointer_warp(SEAT), Some(at(300., 250.)));
    assert_eq!(policy.take_pointer_warp(SEAT), None);

    policy.grab_key(SEAT, RIGHT);

    assert_eq!(policy.take_pointer_warp(SEAT), Some(at(310., 250.)));
}

#[test]
fn the_pointer_moves_a_keyboard_move_from_where_it_was_warped() {
    let mut policy = with_a_window();
    policy.begin_keyboard_move(SEAT, id(1));
    policy.grab_key(SEAT, RIGHT);
    let warped = policy.take_pointer_warp(SEAT).unwrap();

    policy.grab_motion(SEAT, warped + at(40., 30.), false);

    assert_eq!(asked(&policy).loc, (150, 130).into());
}

#[test]
fn return_puts_a_keyboard_move_down_where_it_is() {
    let mut policy = with_a_window();
    policy.begin_keyboard_move(SEAT, id(1));
    policy.grab_key(SEAT, RIGHT);

    policy.grab_key(SEAT, GrabKey::Other);

    assert_eq!(policy.grab(SEAT), None);
    assert_eq!(asked(&policy).loc, (110, 100).into());
}

#[test]
fn escape_puts_a_keyboard_move_back_where_it_began() {
    let mut policy = with_a_window();
    policy.begin_keyboard_move(SEAT, id(1));
    policy.grab_key(SEAT, RIGHT);
    policy.grab_motion(SEAT, at(600., 600.), false);

    policy.grab_key(SEAT, GrabKey::Escape);

    assert_eq!(policy.grab(SEAT), None);
    assert_eq!(asked(&policy).loc, (100, 100).into());
}

#[test]
fn a_keyboard_move_stops_at_an_edge_it_would_step_past_and_goes_on_with_the_next_key() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(1515, 100),
        ms(1),
    );
    policy.begin_keyboard_move(SEAT, id(1));

    // Its right side would go from 1915 to 1925, past the output's at 1920.
    policy.grab_key(SEAT, RIGHT);
    assert_eq!(asked(&policy).loc, (1520, 100).into());

    policy.grab_key(SEAT, RIGHT);
    assert_eq!(asked(&policy).loc, (1530, 100).into());
}

#[test]
fn shift_snaps_a_keyboard_move_to_the_next_edge_its_way() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(2, 300, 300).with_requested_position(1000, 100),
        ms(1),
    );
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(100, 100),
        ms(2),
    );
    policy.begin_keyboard_move(SEAT, id(1));

    // Its right side, at 500, snaps to the other window's left at 1000.
    policy.grab_key(SEAT, arrow(1, 0, GrabStep::Snap));
    assert_eq!(asked(&policy).loc, (600, 100).into());

    // Then to that window's right, at 1300, which is nearer than its left side's next
    // edge.
    policy.grab_key(SEAT, arrow(1, 0, GrabStep::Snap));
    assert_eq!(asked(&policy).loc, (900, 100).into());
}

#[test]
fn the_first_arrow_of_a_keyboard_resize_picks_the_edge_and_the_next_ones_move_it() {
    let mut policy = with_a_window();
    assert!(policy.begin_keyboard_resize(SEAT, id(1)));
    assert_eq!(policy.take_pointer_warp(SEAT), Some(at(300., 250.)));

    policy.grab_key(SEAT, RIGHT);
    assert_eq!(asked(&policy), rect(100, 100, 400, 300));
    assert_eq!(policy.take_pointer_warp(SEAT), Some(at(499., 250.)));

    policy.grab_key(SEAT, RIGHT);
    assert_eq!(asked(&policy), rect(100, 100, 410, 300));
    assert_eq!(policy.take_pointer_warp(SEAT), Some(at(509., 250.)));
    policy.grab_key(SEAT, LEFT);
    assert_eq!(asked(&policy), rect(100, 100, 400, 300));

    // An arrow across the edge picks the edge it points at.
    policy.grab_key(SEAT, UP);
    assert_eq!(asked(&policy), rect(100, 100, 400, 300));
    assert_eq!(policy.take_pointer_warp(SEAT), Some(at(300., 100.)));
    policy.grab_key(SEAT, UP);
    assert_eq!(asked(&policy), rect(100, 90, 400, 310));
    policy.grab_key(SEAT, arrow(0, 1, GrabStep::Fine));
    assert_eq!(asked(&policy), rect(100, 91, 400, 309));
}

#[test]
fn a_keyboard_resize_of_the_left_edge_keeps_the_right_one() {
    let mut policy = with_a_window();
    policy.begin_keyboard_resize(SEAT, id(1));

    policy.grab_key(SEAT, LEFT);
    policy.grab_key(SEAT, LEFT);

    assert_eq!(asked(&policy), rect(90, 100, 410, 300));
}

#[test]
fn escape_puts_a_keyboard_resize_back_as_it_began() {
    let mut policy = with_a_window();
    policy.begin_keyboard_resize(SEAT, id(1));
    policy.grab_key(SEAT, LEFT);
    policy.grab_key(SEAT, LEFT);

    policy.grab_key(SEAT, GrabKey::Escape);

    assert_eq!(policy.grab(SEAT), None);
    assert_eq!(asked(&policy), rect(100, 100, 400, 300));
}

#[test]
fn return_puts_a_keyboard_resize_down_at_its_size() {
    let mut policy = with_a_window();
    policy.begin_keyboard_resize(SEAT, id(1));
    policy.grab_key(SEAT, DOWN);
    policy.grab_key(SEAT, DOWN);

    policy.grab_key(SEAT, GrabKey::Other);

    assert_eq!(policy.grab(SEAT), None);
    assert_eq!(asked(&policy), rect(100, 100, 400, 310));
}

#[test]
fn the_pointer_picks_a_keyboard_resizes_edge_by_the_way_it_goes_first() {
    let mut policy = with_a_window();
    policy.begin_keyboard_resize(SEAT, id(1));
    let warped = policy.take_pointer_warp(SEAT).unwrap();

    policy.grab_motion(SEAT, warped + at(5., 3.), false);
    assert_eq!(policy.grab_edges(SEAT), Some(ResizeEdges::BOTTOM_RIGHT));
    let corner = policy.take_pointer_warp(SEAT).unwrap();
    assert_eq!(corner, at(499., 399.));

    policy.grab_motion(SEAT, corner + at(40., 30.), false);
    assert_eq!(asked(&policy), rect(100, 100, 440, 330));
}

#[test]
fn a_keyboard_resize_stops_at_an_edge_it_would_step_past() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(1515, 100),
        ms(1),
    );
    policy.begin_keyboard_resize(SEAT, id(1));
    policy.grab_key(SEAT, RIGHT);

    policy.grab_key(SEAT, RIGHT);

    assert_eq!(asked(&policy), rect(1515, 100, 405, 300));
}

#[test]
fn maximised_windows_and_windows_that_cant_grow_arent_moved_or_resized_by_the_keys() {
    let mut policy = with_a_window();
    policy.map_window(
        TestWindow::new(2, 400, 300).with_hints((400, 300), (400, 300)),
        ms(2),
    );
    policy.set_maximized(id(1), true);

    assert!(!policy.begin_keyboard_move(SEAT, id(1)));
    assert!(!policy.begin_keyboard_resize(SEAT, id(1)));
    assert!(!policy.begin_keyboard_resize(SEAT, id(2)));
    assert!(policy.begin_keyboard_move(SEAT, id(2)));
}

#[test]
fn begin_move_and_begin_resize_grab_the_focused_window() {
    let mut policy = with_a_window();

    policy.action(SEAT, Action::BeginMove, ms(2));
    assert_eq!(policy.keyboard_grab(SEAT), Some(id(1)));
    assert_eq!(policy.grab_edges(SEAT), None);
    policy.grab_key(SEAT, GrabKey::Other);

    policy.action(SEAT, Action::BeginResize, ms(3));
    assert_eq!(policy.keyboard_grab(SEAT), Some(id(1)));
    assert_eq!(policy.grab_edges(SEAT), Some(ResizeEdges::default()));
}

#[test]
fn a_keyboard_grab_doesnt_start_over_another_grab() {
    let mut policy = with_a_window();
    assert!(policy.begin_move(SEAT, id(1), at(150., 150.)));

    assert!(!policy.begin_keyboard_resize(SEAT, id(1)));
    assert_eq!(policy.keyboard_grab(SEAT), None);
}
