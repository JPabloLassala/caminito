//! Edge and corner tiling (158), as Marco's `update_move` and `meta_window_tile` have it: a
//! window dropped with the pointer at an output's side fills that half of the work area, at
//! a corner that quarter, and at the top all of it, maximised; a tiled window goes back to
//! its floating geometry when it's untiled.

use super::*;

use crate::{FrameState, Functions, ResizeEdges, Tile, TiledEdges};

fn at(x: f64, y: f64) -> Point<f64, Logical> {
    Point::new(x, y)
}

fn tiling_config() -> Config {
    Config {
        allow_tiling: true,
        ..Config::default()
    }
}

/// A policy with tiling on and one 1920×1080 output, with a 400×300 window at 500,400.
fn tiling() -> Policy<TestWindow> {
    tiling_with(tiling_config(), TestWindow::new(1, 400, 300))
}

fn tiling_with(config: Config, window: TestWindow) -> Policy<TestWindow> {
    let mut policy = Policy::new(config);
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy.map_window(window.with_requested_position(500, 400), ms(1));
    policy
}

/// Drags window 1 from 600,410 to `x`,`y` and lets go there.
fn drop_at(policy: &mut Policy<TestWindow>, x: f64, y: f64) {
    assert!(policy.begin_move(SEAT, id(1), at(600., 410.)));
    policy.grab_motion(SEAT, at(x, y), false);
    policy.end_grab(SEAT);
}

fn last(policy: &Policy<TestWindow>) -> (Rectangle<i32, Logical>, WindowStates) {
    window(policy, 1).last_geometry().expect("was configured")
}

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Logical> {
    Rectangle::new((x, y).into(), (w, h).into())
}

#[test]
fn a_window_dropped_at_the_left_edge_fills_the_left_half_of_the_work_area() {
    let mut policy = tiling();

    drop_at(&mut policy, 0., 500.);

    assert_eq!(policy.tile(id(1)), Some(Tile::Left));
    let (geometry, states) = last(&policy);
    assert_eq!(geometry, rect(0, 0, 960, 1080));
    assert_eq!(states.tile, Some(Tile::Left));
}

#[test]
fn each_side_and_corner_tiles_to_its_half_or_quarter() {
    let cases = [
        ((1919., 500.), Tile::Right, rect(960, 0, 960, 1080)),
        ((0., 0.), Tile::TopLeft, rect(0, 0, 960, 540)),
        ((1919., 5.), Tile::TopRight, rect(960, 0, 960, 540)),
        ((3., 1079.), Tile::BottomLeft, rect(0, 540, 960, 540)),
        ((1910., 1070.), Tile::BottomRight, rect(960, 540, 960, 540)),
    ];
    for ((x, y), tile, expected) in cases {
        let mut policy = tiling();

        drop_at(&mut policy, x, y);

        assert_eq!(policy.tile(id(1)), Some(tile), "dropped at {x},{y}");
        let (geometry, states) = last(&policy);
        assert_eq!(geometry, expected, "{tile:?}");
        assert_eq!(states.tile, Some(tile));
    }
}

#[test]
fn a_framed_window_tiles_frame_and_all() {
    let mut policy = tiling_with(
        tiling_config(),
        TestWindow::new(1, 400, 300).with_frame(5, 5, 25, 5),
    );

    drop_at(&mut policy, 1919., 500.);

    // The frame fills the right half, 960,0 960×1080.
    assert_eq!(last(&policy).0, rect(965, 25, 950, 1050));
}

#[test]
fn a_window_dropped_at_the_top_is_maximised() {
    let mut policy = tiling();

    drop_at(&mut policy, 960., 0.);

    assert!(policy.is_maximized(id(1)));
    assert_eq!(policy.tile(id(1)), None);
    assert_eq!(last(&policy).0, rect(0, 0, 1920, 1080));
}

#[test]
fn the_zones_reach_twice_the_drag_threshold_into_the_work_area_and_the_top_to_its_edge() {
    let mut policy = tiling();
    // A panel of 28 along the top.
    policy.add_output(reserving(output(1, 0, 0, 1920, 1080), 0, 0, 28, 0));
    assert!(policy.begin_move(SEAT, id(1), at(600., 410.)));

    // The drag threshold is 8, so a side's zone is 16 wide.
    let cases = [
        ((15., 500.), Some(rect(0, 28, 960, 1052))),
        ((16., 500.), None),
        ((1904., 500.), Some(rect(960, 28, 960, 1052))),
        ((1903., 500.), None),
        // A corner reaches 16 under the work area's top and over its bottom.
        ((5., 43.), Some(rect(0, 28, 960, 526))),
        ((5., 44.), Some(rect(0, 28, 960, 1052))),
        ((5., 1065.), Some(rect(0, 554, 960, 526))),
        ((5., 1064.), Some(rect(0, 28, 960, 1052))),
        // The top's zone is the panel, down to the work area's top.
        ((960., 28.), Some(rect(0, 28, 1920, 1052))),
        ((960., 29.), None),
    ];
    for ((x, y), preview) in cases {
        policy.grab_motion(SEAT, at(x, y), false);
        assert_eq!(policy.tile_preview(SEAT), preview, "at {x},{y}");
    }
}

#[test]
fn the_zones_follow_the_drag_threshold() {
    let config = Config {
        drag_threshold: 20,
        ..tiling_config()
    };
    let mut policy = tiling_with(config, TestWindow::new(1, 400, 300));
    assert!(policy.begin_move(SEAT, id(1), at(600., 410.)));

    policy.grab_motion(SEAT, at(39., 500.), false);
    assert_eq!(policy.tile_preview(SEAT), Some(rect(0, 0, 960, 1080)));
    policy.grab_motion(SEAT, at(40., 500.), false);
    assert_eq!(policy.tile_preview(SEAT), None);
}

#[test]
fn the_preview_shows_while_the_pointer_is_in_a_zone_and_goes_on_release() {
    let mut policy = tiling();
    assert!(policy.begin_move(SEAT, id(1), at(600., 410.)));
    assert_eq!(policy.tile_preview(SEAT), None);

    policy.grab_motion(SEAT, at(0., 500.), false);
    assert_eq!(policy.tile_preview(SEAT), Some(rect(0, 0, 960, 1080)));
    assert_eq!(policy.tile(id(1)), None, "nothing tiles before the release");

    policy.grab_motion(SEAT, at(700., 500.), false);
    assert_eq!(policy.tile_preview(SEAT), None);

    policy.grab_motion(SEAT, at(1919., 500.), false);
    policy.end_grab(SEAT);
    assert_eq!(policy.tile_preview(SEAT), None);
    assert_eq!(policy.tile(id(1)), Some(Tile::Right));
}

#[test]
fn moving_away_from_the_edge_before_letting_go_tiles_nothing() {
    let mut policy = tiling();
    assert!(policy.begin_move(SEAT, id(1), at(600., 410.)));
    policy.grab_motion(SEAT, at(0., 500.), false);
    policy.grab_motion(SEAT, at(300., 500.), false);
    policy.end_grab(SEAT);

    assert_eq!(policy.tile(id(1)), None);
    assert_eq!(last(&policy).0, rect(200, 490, 400, 300));
}

#[test]
fn a_snapping_move_tiles_nowhere() {
    let mut policy = tiling();
    assert!(policy.begin_move(SEAT, id(1), at(600., 410.)));

    policy.grab_motion(SEAT, at(0., 500.), true);
    assert_eq!(policy.tile_preview(SEAT), None);
    policy.end_grab(SEAT);

    assert_eq!(policy.tile(id(1)), None);
}

#[test]
fn a_cancelled_move_tiles_nothing() {
    let mut policy = tiling();
    assert!(policy.begin_move(SEAT, id(1), at(600., 410.)));
    policy.grab_motion(SEAT, at(0., 500.), false);

    policy.cancel_grab(SEAT);

    assert_eq!(policy.tile(id(1)), None);
    assert_eq!(last(&policy).0, rect(500, 400, 400, 300));
}

#[test]
fn with_allow_tiling_off_nothing_tiles_or_maximises() {
    let mut policy = tiling_with(Config::default(), TestWindow::new(1, 400, 300));
    assert!(policy.begin_move(SEAT, id(1), at(600., 410.)));

    for (x, y) in [(0., 500.), (0., 0.), (960., 0.)] {
        policy.grab_motion(SEAT, at(x, y), false);
        assert_eq!(policy.tile_preview(SEAT), None, "at {x},{y}");
    }
    policy.end_grab(SEAT);

    assert_eq!(policy.tile(id(1)), None);
    assert!(!policy.is_maximized(id(1)));
}

#[test]
fn with_allow_top_tiling_off_the_top_does_nothing_and_the_sides_still_tile() {
    let config = Config {
        allow_top_tiling: false,
        ..tiling_config()
    };
    let mut policy = tiling_with(config.clone(), TestWindow::new(1, 400, 300));

    drop_at(&mut policy, 960., 0.);
    assert!(!policy.is_maximized(id(1)));

    let mut policy = tiling_with(config, TestWindow::new(1, 400, 300));
    drop_at(&mut policy, 0., 0.);
    assert_eq!(policy.tile(id(1)), Some(Tile::TopLeft));
}

#[test]
fn a_window_too_big_for_half_the_work_area_only_maximises_at_the_top() {
    let window = TestWindow::new(1, 1000, 300).with_hints((1000, 200), (0, 0));
    let mut policy = tiling_with(tiling_config(), window);
    assert!(policy.begin_move(SEAT, id(1), at(600., 410.)));

    policy.grab_motion(SEAT, at(0., 500.), false);
    assert_eq!(policy.tile_preview(SEAT), None);
    policy.grab_motion(SEAT, at(960., 0.), false);
    assert_eq!(policy.tile_preview(SEAT), Some(rect(0, 0, 1920, 1080)));
}

#[test]
fn a_window_of_fixed_size_neither_tiles_nor_maximises() {
    let window = TestWindow::new(1, 400, 300).with_hints((400, 300), (400, 300));
    let mut policy = tiling_with(tiling_config(), window);
    assert!(policy.begin_move(SEAT, id(1), at(600., 410.)));

    for (x, y) in [(0., 500.), (960., 0.)] {
        policy.grab_motion(SEAT, at(x, y), false);
        assert_eq!(policy.tile_preview(SEAT), None, "at {x},{y}");
    }
}

#[test]
fn a_tile_tells_the_client_the_edges_it_touches() {
    assert_eq!(
        Tile::Left.edges(),
        TiledEdges {
            left: true,
            right: false,
            top: true,
            bottom: true
        }
    );
    assert_eq!(
        Tile::BottomRight.edges(),
        TiledEdges {
            left: false,
            right: true,
            top: false,
            bottom: true
        }
    );
}

#[test]
fn a_side_tile_has_its_frame_drawn_tiled_and_a_corner_doesnt() {
    let states = |tile| WindowStates {
        tile,
        ..WindowStates::default()
    };
    assert_eq!(
        states(Some(Tile::Right)).frame_state(true, Functions::default()),
        FrameState {
            focused: true,
            tiled_right: true,
            ..FrameState::default()
        }
    );
    assert_eq!(
        states(Some(Tile::TopLeft)).frame_state(false, Functions::default()),
        FrameState::default()
    );
}

#[test]
fn a_tiled_windows_own_geometry_requests_are_refused() {
    let mut policy = tiling();
    drop_at(&mut policy, 0., 500.);

    policy.request_geometry(id(1), Some((300, 300).into()), Some((200, 200).into()));

    assert_eq!(last(&policy).0, rect(0, 0, 960, 1080));
}

#[test]
fn maximising_and_unmaximising_a_tiled_window_takes_it_back_to_where_the_move_began() {
    let mut policy = tiling();
    drop_at(&mut policy, 0., 500.);

    policy.set_maximized(id(1), true);
    assert_eq!(policy.tile(id(1)), None);
    assert_eq!(last(&policy).0, rect(0, 0, 1920, 1080));

    policy.set_maximized(id(1), false);
    assert_eq!(
        last(&policy),
        (rect(500, 400, 400, 300), WindowStates::default())
    );
}

#[test]
fn dragging_a_tiled_windows_titlebar_untiles_it_under_the_pointer() {
    let mut policy = tiling();
    drop_at(&mut policy, 0., 500.);

    // Pressed a quarter of the way across the 960 wide tile, then dragged 10 to the right.
    assert!(policy.begin_titlebar_move(SEAT, id(1), at(240., 10.), at(240., 10.)));
    policy.grab_motion(SEAT, at(250., 10.), false);

    assert_eq!(policy.tile(id(1)), None);
    // Its floating size, with a quarter of it left of the pointer, and the press as far
    // down it as it was down the tile: 10 of 1080, 3 of 300.
    assert_eq!(
        last(&policy),
        (rect(150, 7, 400, 300), WindowStates::default())
    );
}

#[test]
fn a_move_of_a_tiled_window_holds_it_until_twice_the_drag_threshold_then_untiles_it() {
    let mut policy = tiling();
    drop_at(&mut policy, 0., 500.);
    assert!(policy.begin_move(SEAT, id(1), at(480., 500.)));

    policy.grab_motion(SEAT, at(495., 500.), false);
    assert_eq!(policy.tile(id(1)), Some(Tile::Left));
    assert_eq!(last(&policy).0, rect(0, 0, 960, 1080));

    // The pointer is 496 of the tile's 960 across, so as far across the floating 400, and
    // it was pressed 500 of 1080 down, so as far down the floating 300.
    policy.grab_motion(SEAT, at(496., 500.), false);
    assert_eq!(policy.tile(id(1)), None);
    assert_eq!(last(&policy).0, rect(289, 361, 400, 300));
}

#[test]
fn an_untiled_window_can_be_tiled_again_in_the_same_move() {
    let mut policy = tiling();
    drop_at(&mut policy, 0., 500.);
    assert!(policy.begin_move(SEAT, id(1), at(480., 500.)));

    policy.grab_motion(SEAT, at(700., 500.), false);
    policy.grab_motion(SEAT, at(1919., 500.), false);
    policy.end_grab(SEAT);

    assert_eq!(policy.tile(id(1)), Some(Tile::Right));
    assert_eq!(last(&policy).0, rect(960, 0, 960, 1080));
}

/// Resizes window 1, tiled and drawn at its tile's size, by `edges`, from `from` to `to`.
fn resize_tiled(
    policy: &mut Policy<TestWindow>,
    edges: ResizeEdges,
    from: (f64, f64),
    to: (f64, f64),
) {
    let size = last(policy).0.size;
    policy.window_mut(id(1)).unwrap().size = size;
    policy.commit(id(1));
    assert!(policy.begin_resize(SEAT, id(1), edges, at(from.0, from.1)));
    policy.grab_motion(SEAT, at(to.0, to.1), false);
    policy.end_grab(SEAT);
}

const RIGHT: ResizeEdges = ResizeEdges {
    right: true,
    top: false,
    bottom: false,
    left: false,
};

const BOTTOM: ResizeEdges = ResizeEdges {
    bottom: true,
    top: false,
    left: false,
    right: false,
};

#[test]
fn a_tiled_window_resized_by_its_free_side_stays_tiled_at_the_new_width() {
    let mut policy = tiling();
    drop_at(&mut policy, 0., 500.);

    resize_tiled(&mut policy, RIGHT, (960., 500.), (900., 500.));

    assert_eq!(policy.tile(id(1)), Some(Tile::Left));
    let (geometry, states) = last(&policy);
    assert_eq!(geometry, rect(0, 0, 900, 1080));
    assert_eq!(states.tile, Some(Tile::Left));
}

#[test]
fn a_tiled_window_resized_by_a_side_along_the_work_area_doesnt_change() {
    // Marco's `can_resize_tiled` (constraints.c:986-1060) frees only a tile's inner sides:
    // a side tile's left or right, and a corner's and its top or bottom, alone or together.
    let cases = [
        ((0., 500.), BOTTOM, (500., 1079.), (500., 900.)),
        (
            (0., 500.),
            ResizeEdges::BOTTOM_RIGHT,
            (959., 1079.),
            (900., 900.),
        ),
        ((0., 0.), ResizeEdges::TOP_LEFT, (0., 0.), (50., 50.)),
    ];
    for (drop, edges, from, to) in cases {
        let mut policy = tiling();
        drop_at(&mut policy, drop.0, drop.1);
        let tiled = last(&policy).0;

        resize_tiled(&mut policy, edges, from, to);

        assert_eq!(last(&policy).0, tiled, "{edges:?}");
        assert!(policy.tile(id(1)).is_some());
    }
}

#[test]
fn a_corner_tile_resizes_by_its_inner_side_and_its_inner_corner() {
    let mut policy = tiling();
    drop_at(&mut policy, 0., 0.);

    resize_tiled(
        &mut policy,
        ResizeEdges::BOTTOM_RIGHT,
        (959., 539.),
        (1059., 439.),
    );

    assert_eq!(policy.tile(id(1)), Some(Tile::TopLeft));
    assert_eq!(last(&policy).0, rect(0, 0, 1060, 440));
}

#[test]
fn a_resized_tile_keeps_its_size_until_it_is_tiled_again() {
    let mut policy = tiling();
    drop_at(&mut policy, 1919., 500.);
    resize_tiled(
        &mut policy,
        ResizeEdges {
            left: true,
            ..ResizeEdges::default()
        },
        (960., 500.),
        (1060., 500.),
    );
    assert_eq!(last(&policy).0, rect(1060, 0, 860, 1080));

    // As Marco's `tile_resized` keeps it (constraints.c:958-965).
    policy.add_output(reserving(output(1, 0, 0, 1920, 1080), 0, 0, 28, 0));
    assert_eq!(last(&policy).0, rect(1060, 0, 860, 1080));

    // A drag to the same side tiles it afresh.
    assert!(policy.begin_move(SEAT, id(1), at(1500., 410.)));
    policy.grab_motion(SEAT, at(1000., 500.), false);
    policy.grab_motion(SEAT, at(1919., 500.), false);
    policy.end_grab(SEAT);
    assert_eq!(last(&policy).0, rect(960, 28, 960, 1052));
}

#[test]
fn a_tiled_window_follows_its_work_area_when_it_changes() {
    let mut policy = tiling();
    drop_at(&mut policy, 1919., 1079.);

    policy.add_output(reserving(output(1, 0, 0, 1920, 1080), 0, 0, 28, 40));

    assert_eq!(policy.tile(id(1)), Some(Tile::BottomRight));
    assert_eq!(last(&policy).0, rect(960, 534, 960, 506));
}

#[test]
fn a_tiled_window_whose_output_goes_is_tiled_on_the_one_it_moves_to() {
    let mut policy = tiling();
    policy.add_output(output(2, 1920, 0, 1280, 1024));
    drop_at(&mut policy, 1920., 500.);
    assert_eq!(last(&policy).0, rect(1920, 0, 640, 1024));

    policy.remove_output(OutputId(2));

    assert_eq!(policy.tile(id(1)), Some(Tile::Left));
    assert_eq!(last(&policy).0, rect(0, 0, 960, 1080));
}

#[test]
fn at_an_edge_between_two_outputs_the_pointers_output_decides() {
    let mut policy = tiling();
    policy.add_output(output(2, 1920, 0, 1280, 1024));
    assert!(policy.begin_move(SEAT, id(1), at(600., 410.)));

    // Just inside the first output: its right half.
    policy.grab_motion(SEAT, at(1919., 500.), false);
    assert_eq!(policy.tile_preview(SEAT), Some(rect(960, 0, 960, 1080)));

    // Just past the shared edge: the second's left half.
    policy.grab_motion(SEAT, at(1920., 500.), false);
    assert_eq!(policy.tile_preview(SEAT), Some(rect(1920, 0, 640, 1024)));
}

#[test]
fn a_keyboard_move_untiles_a_tiled_window_round_the_pointer_and_tiles_nothing() {
    let mut policy = tiling();
    drop_at(&mut policy, 0., 500.);
    assert!(policy.begin_keyboard_move(SEAT, id(1)));
    // The middle of the left half.
    assert_eq!(policy.take_pointer_warp(SEAT), Some(at(480., 540.)));

    policy.grab_key(
        SEAT,
        crate::GrabKey::Arrow {
            x: 1,
            y: 0,
            step: crate::GrabStep::Normal,
        },
    );

    assert_eq!(policy.tile(id(1)), None);
    assert_eq!(last(&policy).0, rect(290, 390, 400, 300));
    policy.grab_motion(SEAT, at(0., 500.), false);
    assert_eq!(policy.tile_preview(SEAT), None);
}
