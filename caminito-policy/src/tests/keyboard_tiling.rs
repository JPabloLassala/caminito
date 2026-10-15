//! The tiling keybindings (159), as Marco's `handle_toggle_tiled` has them: each binding
//! tiles the focused window to its half or quarter of the work area of the output it's
//! on, and repeated presses cycle its width through Marco's sizes, or untile it.

use super::*;

use crate::Tile;

/// A policy with one 1920×1080 output and a focused 400×300 window at 500,400.
fn keyboard_tiling() -> Policy<TestWindow> {
    keyboard_tiling_with(Config::default())
}

fn keyboard_tiling_with(config: Config) -> Policy<TestWindow> {
    let mut policy = Policy::new(config);
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(500, 400),
        ms(1),
    );
    policy
}

fn press(policy: &mut Policy<TestWindow>, tile: Tile) {
    policy.action(SEAT, Action::Tile(tile), ms(10));
}

fn geometry(policy: &Policy<TestWindow>) -> Rectangle<i32, Logical> {
    window(policy, 1).last_geometry().expect("was configured").0
}

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Logical> {
    Rectangle::new((x, y).into(), (w, h).into())
}

#[test]
fn each_binding_tiles_the_focused_window_to_its_half_or_quarter() {
    let cases = [
        (Tile::Left, rect(0, 0, 960, 1080)),
        (Tile::Right, rect(960, 0, 960, 1080)),
        (Tile::TopLeft, rect(0, 0, 960, 540)),
        (Tile::TopRight, rect(960, 0, 960, 540)),
        (Tile::BottomLeft, rect(0, 540, 960, 540)),
        (Tile::BottomRight, rect(960, 540, 960, 540)),
    ];
    for (tile, expected) in cases {
        let mut policy = keyboard_tiling();

        press(&mut policy, tile);

        assert_eq!(policy.tile(id(1)), Some(tile));
        assert_eq!(geometry(&policy), expected, "{tile:?}");
        assert_eq!(
            window(&policy, 1).last_geometry().unwrap().1.tile,
            Some(tile)
        );
    }
}

#[test]
fn with_cycling_each_press_of_a_side_steps_its_width_through_marcos_sizes_then_untiles() {
    let mut policy = keyboard_tiling();
    // Half, a third, a quarter, all, three quarters and two thirds of 1920, rounded down.
    let widths = [960, 640, 480, 1920, 1440, 1280];
    for width in widths {
        press(&mut policy, Tile::Right);
        assert_eq!(policy.tile(id(1)), Some(Tile::Right));
        assert_eq!(geometry(&policy), rect(1920 - width, 0, width, 1080));
    }

    press(&mut policy, Tile::Right);

    assert_eq!(policy.tile(id(1)), None);
    assert_eq!(geometry(&policy), rect(500, 400, 400, 300));
}

#[test]
fn with_cycling_a_corner_cycles_its_width_too() {
    let mut policy = keyboard_tiling();
    press(&mut policy, Tile::BottomLeft);
    press(&mut policy, Tile::BottomLeft);

    assert_eq!(geometry(&policy), rect(0, 540, 640, 540));
}

#[test]
fn with_cycling_another_binding_starts_again_at_half() {
    let mut policy = keyboard_tiling();
    press(&mut policy, Tile::Left);
    press(&mut policy, Tile::Left);
    assert_eq!(geometry(&policy).size.w, 640);

    press(&mut policy, Tile::Right);
    assert_eq!(geometry(&policy), rect(960, 0, 960, 1080));
    press(&mut policy, Tile::Left);
    assert_eq!(geometry(&policy), rect(0, 0, 960, 1080));
}

#[test]
fn without_cycling_a_second_press_untiles_and_another_binding_retiles_at_half() {
    let mut policy = keyboard_tiling_with(Config {
        allow_tile_cycling: false,
        ..Config::default()
    });
    press(&mut policy, Tile::Left);
    assert_eq!(geometry(&policy), rect(0, 0, 960, 1080));

    press(&mut policy, Tile::Left);
    assert_eq!(policy.tile(id(1)), None);
    assert_eq!(geometry(&policy), rect(500, 400, 400, 300));

    press(&mut policy, Tile::TopRight);
    press(&mut policy, Tile::Left);
    assert_eq!(policy.tile(id(1)), Some(Tile::Left));
    assert_eq!(geometry(&policy), rect(0, 0, 960, 1080));
}

#[test]
fn a_window_tiled_by_a_drag_takes_one_press_to_start_the_cycle() {
    // Marco's drag leaves the tile at `META_TILE_CYCLE_NONE`, which the first press steps
    // to the half it's already at.
    let mut policy = keyboard_tiling_with(Config {
        allow_tiling: true,
        ..Config::default()
    });
    assert!(policy.begin_move(SEAT, id(1), Point::new(600., 410.)));
    policy.grab_motion(SEAT, Point::new(0., 500.), false);
    policy.end_grab(SEAT);
    assert_eq!(policy.tile(id(1)), Some(Tile::Left));

    press(&mut policy, Tile::Left);
    assert_eq!(geometry(&policy), rect(0, 0, 960, 1080));
    press(&mut policy, Tile::Left);
    assert_eq!(geometry(&policy), rect(0, 0, 640, 1080));
}

#[test]
fn a_maximised_window_is_tiled_and_untiling_it_restores_its_floating_geometry() {
    let mut policy = keyboard_tiling_with(Config {
        allow_tile_cycling: false,
        ..Config::default()
    });
    policy.action(SEAT, Action::ToggleMaximized, ms(5));

    press(&mut policy, Tile::Right);
    assert!(!policy.is_maximized(id(1)));
    assert_eq!(geometry(&policy), rect(960, 0, 960, 1080));

    press(&mut policy, Tile::Right);
    assert_eq!(geometry(&policy), rect(500, 400, 400, 300));
}

#[test]
fn a_window_on_a_second_output_is_tiled_in_that_outputs_work_area() {
    let mut policy = keyboard_tiling();
    policy.add_output(reserving(output(2, 1920, 0, 1280, 1024), 0, 0, 24, 0));
    policy.map_window(
        TestWindow::new(2, 400, 300).with_requested_position(2200, 300),
        ms(2),
    );
    assert_eq!(policy.focus(SEAT), Some(id(2)));

    press(&mut policy, Tile::Right);
    assert_eq!(
        window(&policy, 2).last_geometry().unwrap().0,
        rect(2560, 24, 640, 1000)
    );
    press(&mut policy, Tile::Right);
    assert_eq!(
        window(&policy, 2).last_geometry().unwrap().0,
        rect(2774, 24, 426, 1000)
    );
}

#[test]
fn a_window_that_cant_be_maximised_or_is_too_big_isnt_tiled() {
    let windows = [
        // Fixed size.
        TestWindow::new(1, 400, 300).with_hints((400, 300), (400, 300)),
        // Wider at least than half the work area.
        TestWindow::new(1, 1000, 300).with_hints((1000, 300), (0, 0)),
    ];
    for window in windows {
        let mut policy = Policy::new(Config::default());
        policy.add_output(output(1, 0, 0, 1920, 1080));
        policy.map_window(window.with_requested_position(100, 100), ms(1));

        press(&mut policy, Tile::Left);

        assert_eq!(policy.tile(id(1)), None);
    }
}

#[test]
fn a_press_tiles_a_resized_tile_afresh() {
    let mut policy = keyboard_tiling();
    press(&mut policy, Tile::Left);
    policy.window_mut(id(1)).unwrap().size = (960, 1080).into();
    policy.commit(id(1));
    let right = crate::ResizeEdges {
        right: true,
        ..crate::ResizeEdges::default()
    };
    assert!(policy.begin_resize(SEAT, id(1), right, Point::new(959., 500.)));
    policy.grab_motion(SEAT, Point::new(1059., 500.), false);
    policy.end_grab(SEAT);
    assert_eq!(geometry(&policy), rect(0, 0, 1060, 1080));

    press(&mut policy, Tile::Right);

    assert_eq!(geometry(&policy), rect(960, 0, 960, 1080));
}

#[test]
fn a_tiled_window_whose_frame_is_taller_than_its_tile_keeps_at_least_one_pixel() {
    // The case proptest shrank (238): the work area ends 29 high, and the frame's top and
    // bottom extents together are 30.
    let mut policy = Policy::new(Config::default());
    policy.set_outputs(vec![output(1, 0, 0, 20, 30)]);
    policy.map_window(TestWindow::new(1, 1, 1).with_frame(5, 5, 25, 5), ms(1));
    press(&mut policy, Tile::Left);

    policy.set_outputs(vec![reserving(output(1, 0, 0, 20, 30), 0, 0, 0, 1)]);

    assert_eq!(policy.tile(id(1)), Some(Tile::Left));
    assert_eq!(geometry(&policy), rect(5, 25, 1, 1));
}
