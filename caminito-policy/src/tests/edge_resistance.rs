//! Edge resistance and snap-move during interactive moves and resizes (157), as Marco's
//! `edge-resistance.c` has them: a window stops at the edges of the work areas and of the
//! other windows on its workspace until the pointer goes 32 (an output's) or 16 (a
//! window's) pixels past, and with Shift it snaps to the nearest edges instead.

use super::*;

use crate::ResizeEdges;

fn at(x: f64, y: f64) -> Point<f64, Logical> {
    Point::new(x, y)
}

/// A policy whose 1920×1080 output has a panel of 30 along its bottom, with a 400×300
/// window at 100,100 being moved from 200,200.
fn moving_beside_a_panel() -> Policy<TestWindow> {
    let mut policy = policy();
    policy.add_output(reserving(output(1, 0, 0, 1920, 1080), 0, 0, 0, 30));
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(100, 100),
        ms(1),
    );
    assert!(policy.begin_move(SEAT, id(1), at(200., 200.)));
    policy
}

fn loc(policy: &Policy<TestWindow>, w: u64) -> Point<i32, Logical> {
    window(policy, w)
        .last_geometry()
        .expect("was configured")
        .0
        .loc
}

#[test]
fn a_move_stops_at_the_work_areas_edge_until_the_pointer_goes_32_past() {
    let mut policy = moving_beside_a_panel();

    // The window's bottom would be 10 past the work area's, at 1060.
    policy.grab_motion(SEAT, at(200., 860.), false);
    assert_eq!(loc(&policy, 1), (100, 750).into());

    policy.grab_motion(SEAT, at(200., 881.), false);
    assert_eq!(loc(&policy, 1), (100, 750).into());

    // 32 past: it goes on.
    policy.grab_motion(SEAT, at(200., 882.), false);
    assert_eq!(loc(&policy, 1), (100, 782).into());
}

#[test]
fn a_resize_stops_at_the_outputs_edge_until_the_pointer_goes_32_past() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(100, 100),
        ms(1),
    );
    assert!(policy.begin_resize(SEAT, id(1), ResizeEdges::BOTTOM_RIGHT, at(500., 400.)));

    policy.grab_motion(SEAT, at(1930., 400.), false);
    assert_eq!(sent(&policy, 1), rect(100, 100, 1820, 300));

    policy.grab_motion(SEAT, at(1952., 400.), false);
    assert_eq!(sent(&policy, 1), rect(100, 100, 1852, 300));
}

#[test]
fn a_frame_stops_at_the_edge_not_its_window() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300)
            .with_frame(5, 5, 25, 5)
            .with_requested_position(100, 100),
        ms(1),
    );
    assert!(policy.begin_move(SEAT, id(1), at(200., 200.)));

    // The frame's left side would be 10 past the output's, at -10.
    policy.grab_motion(SEAT, at(105., 200.), false);

    assert_eq!(loc(&policy, 1), (5, 100).into());
}

/// A policy with a 400×300 window at 100,100 and a second at 1000,100 to its right.
fn two_windows_side_by_side() -> Policy<TestWindow> {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(100, 100),
        ms(1),
    );
    policy.map_window(
        TestWindow::new(2, 400, 300).with_requested_position(1000, 100),
        ms(2),
    );
    policy
}

#[test]
fn a_move_stops_at_another_windows_edge_until_the_pointer_goes_16_past() {
    let mut policy = two_windows_side_by_side();
    assert!(policy.begin_move(SEAT, id(1), at(200., 200.)));

    // Its right side would be 10 into the other window, at 1010.
    policy.grab_motion(SEAT, at(710., 200.), false);
    assert_eq!(loc(&policy, 1), (600, 100).into());

    policy.grab_motion(SEAT, at(716., 200.), false);
    assert_eq!(loc(&policy, 1), (616, 100).into());
}

#[test]
fn a_resize_stops_at_another_windows_edge() {
    let mut policy = two_windows_side_by_side();
    assert!(policy.begin_resize(SEAT, id(1), ResizeEdges::BOTTOM_RIGHT, at(500., 400.)));

    policy.grab_motion(SEAT, at(1010., 400.), false);

    assert_eq!(sent(&policy, 1), rect(100, 100, 900, 300));
}

#[test]
fn nothing_holds_a_window_moving_away_from_an_edge() {
    let mut policy = two_windows_side_by_side();
    assert!(policy.begin_move(SEAT, id(2), at(1100., 200.)));
    // Stopped against the first window's right side.
    policy.grab_motion(SEAT, at(590., 200.), false);
    assert_eq!(loc(&policy, 2), (500, 100).into());

    policy.grab_motion(SEAT, at(605., 200.), false);

    assert_eq!(loc(&policy, 2), (505, 100).into());
}

#[test]
fn with_shift_a_move_snaps_its_nearest_side_to_the_nearest_edge() {
    let mut policy = two_windows_side_by_side();
    assert!(policy.begin_move(SEAT, id(1), at(200., 200.)));

    // The pointer would put it at 400..800: its right side is 200 from the other window,
    // its left 400 from the output's edge.
    policy.grab_motion(SEAT, at(500., 200.), true);

    assert_eq!(loc(&policy, 1), (600, 100).into());
}

#[test]
fn with_shift_a_resize_snaps_its_moving_side() {
    let mut policy = two_windows_side_by_side();
    assert!(policy.begin_resize(SEAT, id(1), ResizeEdges::BOTTOM_RIGHT, at(500., 400.)));

    policy.grab_motion(SEAT, at(800., 400.), true);

    assert_eq!(sent(&policy, 1), rect(100, 100, 900, 300));
}

#[test]
fn a_window_stops_at_the_shared_edge_of_two_outputs_side_by_side_with_either_side() {
    let mut policy = policy();
    policy.add_output(output(2, 1920, 0, 1280, 1024));
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(100, 100),
        ms(1),
    );
    assert!(policy.begin_move(SEAT, id(1), at(200., 200.)));

    // Its right side would be 20 past the shared edge.
    policy.grab_motion(SEAT, at(1640., 200.), false);
    assert_eq!(loc(&policy, 1), (1520, 100).into());
    policy.grab_motion(SEAT, at(1680., 200.), false);
    assert_eq!(loc(&policy, 1), (1580, 100).into());

    // Then its left side, 10 past it.
    policy.grab_motion(SEAT, at(2030., 200.), false);
    assert_eq!(loc(&policy, 1), (1920, 100).into());
}

#[test]
fn a_window_on_another_workspace_isnt_an_edge() {
    let mut policy = two_windows_side_by_side();
    policy.move_to_workspace(id(2), 1);
    assert!(policy.begin_move(SEAT, id(1), at(200., 200.)));

    policy.grab_motion(SEAT, at(710., 200.), false);

    assert_eq!(loc(&policy, 1), (610, 100).into());
}

#[test]
fn the_part_of_an_edge_a_window_above_covers_isnt_an_edge() {
    let mut policy = two_windows_side_by_side();
    // Above the second window, over its left side from y 50 to 450.
    policy.map_window(
        TestWindow::new(3, 200, 400).with_requested_position(900, 50),
        ms(3),
    );
    assert!(policy.begin_move(SEAT, id(1), at(200., 200.)));

    // Its right side would be 10 past the second window's left, but the third covers
    // that, and is 110 past the third's own left side.
    policy.grab_motion(SEAT, at(710., 200.), false);

    assert_eq!(loc(&policy, 1), (610, 100).into());
}

#[test]
fn a_desktop_windows_edges_dont_count() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(100, 100),
        ms(1),
    );
    policy.map_window(
        TestWindow::new(2, 400, 300)
            .with_type(WindowType::Desktop)
            .with_requested_position(1000, 100),
        ms(2),
    );
    assert!(policy.begin_move(SEAT, id(1), at(200., 200.)));

    policy.grab_motion(SEAT, at(710., 200.), false);

    assert_eq!(loc(&policy, 1), (610, 100).into());
}

fn sent(policy: &Policy<TestWindow>, w: u64) -> Rectangle<i32, Logical> {
    window(policy, w).last_geometry().expect("was configured").0
}

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Logical> {
    Rectangle::new((x, y).into(), (w, h).into())
}
