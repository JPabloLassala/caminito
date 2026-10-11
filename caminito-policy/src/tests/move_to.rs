//! The bindings that move the focused window without a grab (160): to a corner, a side or
//! the centre of its output's work area, and to the output beside its own, as Marco's
//! `handle_move_to_corner_backend`, `handle_move_to_center` and `handle_move_to_monitor`
//! have them.

use super::*;

use crate::{Anchor, Direction};

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Logical> {
    Rectangle::new((x, y).into(), (w, h).into())
}

/// A policy with one 1920×1080 output, a panel 24 high along its top and one 30 high along
/// its bottom, and a focused 400×300 window whose Frame is 408×334 at 500,400.
fn with_panels() -> Policy<TestWindow> {
    let mut policy = Policy::new(Config::default());
    policy.add_output(reserving(output(1, 0, 0, 1920, 1080), 0, 0, 24, 30));
    policy.map_window(
        TestWindow::new(1, 400, 300)
            .with_frame(4, 4, 30, 4)
            .with_requested_position(504, 430),
        ms(1),
    );
    assert_eq!(policy.frame_rect(id(1)), Some(rect(500, 400, 408, 334)));
    policy
}

fn press(policy: &mut Policy<TestWindow>, action: Action) {
    policy.action(SEAT, action, ms(10));
}

fn frame(policy: &Policy<TestWindow>, window: u64) -> Rectangle<i32, Logical> {
    policy.frame_rect(id(window)).expect("window is managed")
}

/// The geometry policy last asked of a window.
fn sent(policy: &Policy<TestWindow>, id: u64) -> Rectangle<i32, Logical> {
    window(policy, id)
        .last_geometry()
        .expect("was configured")
        .0
}

/// Maps window 2, a desktop or a dock 300×30 at 100,200, and gives it the focus.
fn focus_a_desktop_or_dock(policy: &mut Policy<TestWindow>, window_type: WindowType) {
    policy.map_window(
        TestWindow::new(2, 300, 30)
            .with_type(window_type)
            .with_requested_position(100, 200),
        ms(2),
    );
    policy.click(SEAT, id(2), ms(3));
    policy.activate(SEAT, id(2), ms(3));
    assert_eq!(policy.focus(SEAT), Some(id(2)));
}

#[test]
fn each_corner_side_and_the_centre_puts_the_frame_there_in_the_work_area() {
    // The work area is 1920×1026 at 0,24, so the Frame's right side is at 1920 from
    // x = 1512, and its bottom at 1050 from y = 716.
    let cases = [
        (Anchor::NorthWest, (0, 24)),
        (Anchor::NorthEast, (1512, 24)),
        (Anchor::SouthWest, (0, 716)),
        (Anchor::SouthEast, (1512, 716)),
        // A side moves the window along one axis alone.
        (Anchor::North, (500, 24)),
        (Anchor::South, (500, 716)),
        (Anchor::East, (1512, 400)),
        (Anchor::West, (0, 400)),
        (Anchor::Centre, (756, 370)),
    ];
    for (anchor, (x, y)) in cases {
        let mut policy = with_panels();

        press(&mut policy, Action::MoveTo(anchor));

        assert_eq!(frame(&policy, 1), rect(x, y, 408, 334), "{anchor:?}");
        // The window geometry sent is the Frame's less its extents.
        assert_eq!(
            sent(&policy, 1),
            rect(x + 4, y + 30, 400, 300),
            "{anchor:?}"
        );
    }
}

#[test]
fn a_fullscreen_or_tiled_window_the_desktop_and_a_dock_stay_where_they_are() {
    let mut fullscreen = with_panels();
    press(&mut fullscreen, Action::ToggleFullscreen);
    press(&mut fullscreen, Action::MoveTo(Anchor::SouthEast));
    assert_eq!(sent(&fullscreen, 1), rect(0, 0, 1920, 1080));

    // A tiled window keeps its tile (158).
    let mut tiled = with_panels();
    press(&mut tiled, Action::Tile(crate::Tile::Left));
    press(&mut tiled, Action::MoveTo(Anchor::East));
    assert_eq!(tiled.tile(id(1)), Some(crate::Tile::Left));
    assert_eq!(sent(&tiled, 1), rect(4, 54, 952, 992));

    for window_type in [WindowType::Desktop, WindowType::Dock] {
        let mut policy = with_panels();
        focus_a_desktop_or_dock(&mut policy, window_type);
        press(&mut policy, Action::MoveTo(Anchor::Centre));
        assert_eq!(frame(&policy, 2).loc, (100, 200).into(), "{window_type:?}");
    }
}

#[test]
fn a_maximised_window_isnt_moved_and_keeps_the_place_it_goes_back_to() {
    let mut policy = with_panels();
    press(&mut policy, Action::Maximize);

    press(&mut policy, Action::MoveTo(Anchor::West));

    assert!(policy.is_maximized(id(1)));
    press(&mut policy, Action::Unmaximize);
    assert_eq!(frame(&policy, 1), rect(500, 400, 408, 334));
}

#[test]
fn a_window_maximised_one_way_moves_the_other_way_alone() {
    let mut policy = with_panels();
    press(&mut policy, Action::ToggleMaximizedVertically);

    press(&mut policy, Action::MoveTo(Anchor::SouthEast));

    assert_eq!(sent(&policy, 1), rect(1516, 54, 400, 992));
    // Unmaximised, it's where the binding put it along x, and where it was along y.
    press(&mut policy, Action::ToggleMaximizedVertically);
    assert_eq!(sent(&policy, 1), rect(1516, 430, 400, 300));
}

#[test]
fn a_window_taller_than_the_work_area_keeps_its_top_in_it() {
    // Marco keeps the titlebar in the work area (`constrain_titlebar_visible`).
    for anchor in [Anchor::South, Anchor::SouthWest, Anchor::Centre] {
        let mut policy = Policy::new(Config::default());
        policy.add_output(reserving(output(1, 0, 0, 1920, 1080), 0, 0, 24, 30));
        policy.map_window(
            TestWindow::new(1, 400, 1200).with_requested_position(500, 100),
            ms(1),
        );

        press(&mut policy, Action::MoveTo(anchor));

        assert_eq!(frame(&policy, 1).loc.y, 24, "{anchor:?}");
    }
}

/// A policy with output 1, 1920×1080 at the origin, output 2 of the same size on its
/// right and output 3 of the same size above it, and a focused 400×300 window at 500,400
/// on output 1.
fn three_outputs() -> Policy<TestWindow> {
    let mut policy = policy();
    policy.add_output(output(2, 1920, 0, 1920, 1080));
    policy.add_output(output(3, 0, -1080, 1920, 1080));
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(500, 400),
        ms(1),
    );
    policy
}

/// A policy with output 1, 1920×1080 at the origin, and on its right output 2, 1280×1024
/// with a panel 24 high along its top, and `window` focused.
fn a_smaller_output_on_the_right(window: TestWindow) -> Policy<TestWindow> {
    let mut policy = policy();
    policy.add_output(reserving(output(2, 1920, 0, 1280, 1024), 0, 0, 24, 0));
    policy.map_window(window, ms(1));
    policy
}

#[test]
fn each_monitor_direction_moves_the_window_to_the_output_beside_its_own() {
    let mut policy = three_outputs();

    press(&mut policy, Action::MoveToMonitor(Direction::Right));
    assert_eq!(sent(&policy, 1), rect(2420, 400, 400, 300));
    assert_eq!(window(&policy, 1).last_outputs(), Some(&[OutputId(2)][..]));
    press(&mut policy, Action::MoveToMonitor(Direction::Left));
    assert_eq!(sent(&policy, 1), rect(500, 400, 400, 300));

    press(&mut policy, Action::MoveToMonitor(Direction::Up));
    assert_eq!(sent(&policy, 1), rect(500, -680, 400, 300));
    press(&mut policy, Action::MoveToMonitor(Direction::Down));
    assert_eq!(sent(&policy, 1), rect(500, 400, 400, 300));
}

#[test]
fn with_no_output_that_way_the_window_stays() {
    for direction in [Direction::Left, Direction::Down] {
        let mut policy = three_outputs();
        press(&mut policy, Action::MoveToMonitor(direction));
        assert_eq!(sent(&policy, 1), rect(500, 400, 400, 300), "{direction:?}");
    }
    // Output 3 is above output 1, and shares no side with output 2.
    let mut policy = three_outputs();
    press(&mut policy, Action::MoveToMonitor(Direction::Right));
    press(&mut policy, Action::MoveToMonitor(Direction::Up));
    assert_eq!(sent(&policy, 1), rect(2420, 400, 400, 300));
}

#[test]
fn on_an_output_of_another_size_the_window_keeps_its_share_of_the_output() {
    // Marco scales the place in the monitor and the size by the monitors' sizes, rounding
    // down (`meta_window_transform_to_monitor`): 1280/1920 along x, 1024/1080 along y.
    let mut policy = a_smaller_output_on_the_right(
        TestWindow::new(1, 400, 300).with_requested_position(500, 100),
    );

    press(&mut policy, Action::MoveToMonitor(Direction::Right));

    assert_eq!(sent(&policy, 1), rect(1920 + 333, 94, 266, 284));
}

#[test]
fn a_window_bigger_than_the_target_work_area_goes_to_its_top_left() {
    // A window of fixed size keeps it, and is 1600 wide in a work area 1280 wide.
    let mut policy = a_smaller_output_on_the_right(
        TestWindow::new(1, 1600, 900)
            .with_hints((1600, 900), (1600, 900))
            .with_requested_position(100, 100),
    );
    press(&mut policy, Action::MoveToMonitor(Direction::Right));
    assert_eq!(sent(&policy, 1), rect(1920, 94, 1600, 900));

    // And a window that would lie under the panel there goes below it.
    let mut policy = a_smaller_output_on_the_right(
        TestWindow::new(1, 400, 300)
            .with_frame(4, 4, 30, 4)
            .with_requested_position(504, 40),
    );
    press(&mut policy, Action::MoveToMonitor(Direction::Right));
    assert_eq!(policy.frame_rect(id(1)).unwrap().loc.y, 24);
}

#[test]
fn a_maximised_window_is_maximised_on_the_other_output_and_goes_back_to_its_share_of_it() {
    let mut policy = a_smaller_output_on_the_right(
        TestWindow::new(1, 400, 300).with_requested_position(500, 100),
    );
    press(&mut policy, Action::Maximize);

    press(&mut policy, Action::MoveToMonitor(Direction::Right));

    assert!(policy.is_maximized(id(1)));
    assert_eq!(sent(&policy, 1), rect(1920, 24, 1280, 1000));
    press(&mut policy, Action::Unmaximize);
    assert_eq!(sent(&policy, 1), rect(1920 + 333, 94, 266, 284));
}

#[test]
fn a_fullscreen_window_is_fullscreen_on_the_other_output_and_a_tiled_one_keeps_its_tile() {
    let window = || TestWindow::new(1, 400, 300).with_requested_position(500, 100);
    let mut fullscreen = a_smaller_output_on_the_right(window());
    press(&mut fullscreen, Action::ToggleFullscreen);
    press(&mut fullscreen, Action::MoveToMonitor(Direction::Right));
    assert!(fullscreen.is_fullscreen(id(1)));
    assert_eq!(sent(&fullscreen, 1), rect(1920, 0, 1280, 1024));

    let mut tiled = a_smaller_output_on_the_right(window());
    press(&mut tiled, Action::Tile(crate::Tile::Left));
    press(&mut tiled, Action::MoveToMonitor(Direction::Right));
    assert_eq!(tiled.tile(id(1)), Some(crate::Tile::Left));
    assert_eq!(sent(&tiled, 1), rect(1920, 24, 640, 1000));
}

#[test]
fn the_desktop_and_a_dock_arent_moved_to_another_output() {
    for window_type in [WindowType::Desktop, WindowType::Dock] {
        let mut policy = three_outputs();
        focus_a_desktop_or_dock(&mut policy, window_type);

        press(&mut policy, Action::MoveToMonitor(Direction::Right));

        assert_eq!(frame(&policy, 2).loc, (100, 200).into(), "{window_type:?}");
    }
}

#[test]
fn a_side_to_the_left_or_right_leaves_a_window_under_the_panel_at_its_height() {
    // A panel that comes after the window doesn't move it, so its top is above the work
    // area, and only a binding that moves it up or down brings it in.
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(500, 0),
        ms(1),
    );
    policy.add_output(reserving(output(1, 0, 0, 1920, 1080), 0, 0, 24, 30));
    assert_eq!(frame(&policy, 1).loc, (500, 0).into());

    press(&mut policy, Action::MoveTo(Anchor::East));

    assert_eq!(frame(&policy, 1).loc, (1520, 0).into());
}
