//! Marco's `placement-mode` and `center-new-windows` (154), and the windows they leave
//! alone. Expected positions are worked by hand from `place.c`'s arithmetic on a 1920×1080
//! output.

use super::*;

use crate::{GrabKey, GrabStep, MapStates, PlacementMode};

fn placed(policy: &Policy<TestWindow>, w: u64) -> (i32, i32) {
    let geometry = policy.geometry(id(w)).expect("window is managed");
    (geometry.loc.x, geometry.loc.y)
}

fn at(x: f64, y: f64) -> Point<f64, Logical> {
    Point::new(x, y)
}

fn right(step: GrabStep) -> GrabKey {
    GrabKey::Arrow { x: 1, y: 0, step }
}

fn config(placement_mode: PlacementMode, center_new_windows: bool) -> Config {
    Config {
        placement_mode,
        center_new_windows,
        ..Config::default()
    }
}

/// `policy()` with these settings, and the pointer at `(x, y)`.
fn policy_with(mode: PlacementMode, center: bool, x: f64, y: f64) -> Policy<TestWindow> {
    let mut policy = policy();
    policy.settings_changed(config(mode, center));
    policy.set_pointer(at(x, y));
    policy
}

// `automatic`, whose other cases are `placement.rs`'s.

#[test]
fn automatic_placement_keeps_clear_of_a_panel_along_the_outputs_edge() {
    let mut policy = policy();
    policy.set_outputs(vec![reserving(output(1, 0, 0, 1920, 1080), 0, 0, 24, 0)]);
    policy.set_pointer(at(1000., 10.));

    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    // The center tile of the 1920×1056 work area: 1920 % 401 / 2, 24 + 1056 % 301 / 3.
    assert_eq!(placed(&policy, 1), (158, 24 + 51));
}

// `pointer`.

#[test]
fn pointer_placement_centres_the_frame_on_the_pointer() {
    let mut policy = policy_with(PlacementMode::Pointer, false, 1000., 500.);
    policy.map_window(TestWindow::new(1, 400, 300).with_frame(5, 5, 25, 5), ms(1));

    // The 410×330 frame at (1000 - 205, 500 - 165), and the window inside it.
    assert_eq!(placed(&policy, 1), (795 + 5, 335 + 25));
    assert_eq!(policy.placing(SEAT), None);
}

#[test]
fn pointer_placement_ignores_the_windows_already_there() {
    let mut policy = policy_with(PlacementMode::Pointer, false, 1000., 500.);
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(800, 350),
        ms(1),
    );

    policy.map_window(TestWindow::new(2, 400, 300), ms(2));

    assert_eq!(placed(&policy, 2), (800, 350));
}

#[test]
fn pointer_placement_keeps_the_window_in_the_work_area() {
    let mut policy = policy();
    policy.set_outputs(vec![reserving(output(1, 0, 0, 1920, 1080), 0, 0, 0, 24)]);
    policy.settings_changed(config(PlacementMode::Pointer, false));
    policy.set_pointer(at(1900., 1070.));

    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    // Pushed in from the right edge and from the panel along the bottom.
    assert_eq!(placed(&policy, 1), (1920 - 400, 1056 - 300));
}

#[test]
fn pointer_placement_uses_the_output_the_pointer_is_on() {
    let mut policy = policy_with(PlacementMode::Pointer, false, 3150., 1000.);
    policy.add_output(output(2, 1920, 0, 1280, 1024));

    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    // Kept within the second output, not the first one's.
    assert_eq!(placed(&policy, 1), (1920 + 1280 - 400, 1024 - 300));
}

#[test]
fn pointer_placement_doesnt_maximise_a_window_too_big_for_its_work_area() {
    let mut policy = policy_with(PlacementMode::Pointer, false, 1000., 500.);
    policy.map_window(
        TestWindow::new(1, 2000, 1200).with_frame(5, 5, 25, 5),
        ms(1),
    );

    assert!(!policy.is_maximized(id(1)));
    assert_eq!(placed(&policy, 1), (5, 25));
}

// `manual`.

#[test]
fn manual_placement_starts_at_the_pointer_and_follows_it_until_put_down() {
    let mut policy = policy_with(PlacementMode::Manual, false, 1000., 500.);
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    assert_eq!(placed(&policy, 1), (800, 350));
    assert_eq!(policy.placing(SEAT), Some(id(1)));
    assert_eq!(policy.grab(SEAT), Some(id(1)));

    policy.grab_motion(SEAT, at(1100., 550.), false);
    assert_eq!(placed(&policy, 1), (900, 400));

    policy.end_grab(SEAT);
    assert_eq!(policy.placing(SEAT), None);
    policy.grab_motion(SEAT, at(1300., 550.), false);
    assert_eq!(placed(&policy, 1), (900, 400));
    let (geometry, _) = window(&policy, 1).last_geometry().unwrap();
    assert_eq!(geometry.loc, (900, 400).into());
}

#[test]
fn a_cancelled_manual_placement_goes_back_to_where_it_started() {
    let mut policy = policy_with(PlacementMode::Manual, false, 1000., 500.);
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.grab_motion(SEAT, at(1300., 700.), false);
    policy.grab_key(SEAT, right(GrabStep::Normal));

    policy.grab_key(SEAT, GrabKey::Escape);

    assert_eq!(placed(&policy, 1), (800, 350));
    assert_eq!(policy.grab(SEAT), None);
}

#[test]
fn a_nudged_manual_placement_goes_on_following_the_pointer_from_where_it_was_nudged() {
    let mut policy = policy_with(PlacementMode::Manual, false, 1000., 500.);
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    policy.grab_key(SEAT, right(GrabStep::Normal));
    policy.grab_key(
        SEAT,
        GrabKey::Arrow {
            x: 0,
            y: -1,
            step: GrabStep::Fine,
        },
    );
    assert_eq!(placed(&policy, 1), (810, 349));
    // A manual placement doesn't warp the pointer (154).
    assert_eq!(policy.take_pointer_warp(SEAT), None);

    policy.grab_motion(SEAT, at(1100., 500.), false);
    assert_eq!(placed(&policy, 1), (910, 349));
}

#[test]
fn manual_placement_ignores_the_windows_already_there() {
    let mut policy = policy_with(PlacementMode::Manual, false, 1000., 500.);
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(800, 350),
        ms(1),
    );

    policy.map_window(TestWindow::new(2, 400, 300), ms(2));

    assert_eq!(placed(&policy, 2), (800, 350));
    assert_eq!(policy.placing(SEAT), Some(id(2)));
}

#[test]
fn manual_placement_starts_in_the_work_area_and_can_leave_its_output() {
    let mut policy = policy_with(PlacementMode::Manual, false, 1900., 1070.);
    policy.add_output(output(2, 1920, 0, 1280, 1024));
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    assert_eq!(placed(&policy, 1), (1520, 780));

    policy.grab_motion(SEAT, at(2500., 600.), false);

    assert_eq!(placed(&policy, 1), (2120, 310));
    assert_eq!(window(&policy, 1).last_outputs(), Some(&[OutputId(2)][..]));
}

#[test]
fn manual_placement_waits_for_no_other_grab() {
    let mut policy = policy_with(PlacementMode::Manual, false, 1000., 500.);
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(100, 100),
        ms(1),
    );
    assert_eq!(policy.placing(SEAT), None);
    assert!(policy.begin_move(SEAT, id(1), at(150., 150.)));

    policy.map_window(TestWindow::new(2, 400, 300), ms(2));

    // Placed at the pointer all the same, but the seat goes on moving the first window.
    assert_eq!(placed(&policy, 2), (800, 350));
    assert_eq!(policy.placing(SEAT), None);
    assert_eq!(policy.grab(SEAT), Some(id(1)));
}

// `center-new-windows`.

#[test]
fn center_new_windows_centres_the_frame_in_the_work_area() {
    let mut policy = policy();
    policy.set_outputs(vec![reserving(output(1, 0, 0, 1920, 1080), 0, 0, 24, 0)]);
    policy.settings_changed(config(PlacementMode::Automatic, true));

    policy.map_window(TestWindow::new(1, 400, 300).with_frame(5, 5, 25, 5), ms(1));

    // The 410×330 frame at ((1920 - 410) / 2, 24 + (1056 - 330) / 2).
    assert_eq!(placed(&policy, 1), (755 + 5, 387 + 25));
}

#[test]
fn center_new_windows_centres_over_the_windows_already_there() {
    let mut policy = policy_with(PlacementMode::Automatic, true, 0., 0.);
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));

    assert_eq!(placed(&policy, 1), (760, 390));
    assert_eq!(placed(&policy, 2), (760, 390));
}

#[test]
fn center_new_windows_uses_the_output_of_the_focused_window() {
    let mut policy = policy_with(PlacementMode::Automatic, true, 0., 0.);
    policy.add_output(output(2, 1920, 0, 1280, 1024));
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(2000, 500),
        ms(1),
    );

    policy.map_window(TestWindow::new(2, 400, 300), ms(2));

    assert_eq!(placed(&policy, 2), (1920 + 440, 362));
}

#[test]
fn center_new_windows_puts_a_window_taller_than_the_work_area_in_its_corner() {
    let mut policy = policy_with(PlacementMode::Automatic, true, 0., 0.);
    policy.add_output(output(2, 1920, 0, 1280, 1024));
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(2000, 500),
        ms(1),
    );

    policy.map_window(TestWindow::new(2, 400, 1100), ms(2));

    // As Marco's origin placement, which skips the cascade with `center-new-windows` on.
    assert_eq!(placed(&policy, 2), (1920, 0));
    assert!(!policy.is_maximized(id(2)));
}

#[test]
fn center_new_windows_leaves_pointer_placement_alone() {
    let mut policy = policy_with(PlacementMode::Pointer, true, 1000., 500.);
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    assert_eq!(placed(&policy, 1), (800, 350));
}

#[test]
fn center_new_windows_leaves_manual_placement_alone() {
    let mut policy = policy_with(PlacementMode::Manual, true, 1000., 500.);
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    assert_eq!(placed(&policy, 1), (800, 350));
    assert_eq!(policy.placing(SEAT), Some(id(1)));
}

// The windows that aren't placed.

#[test]
fn a_requested_position_is_honoured_in_every_mode() {
    for (mode, center) in [
        (PlacementMode::Pointer, false),
        (PlacementMode::Manual, false),
        (PlacementMode::Automatic, true),
    ] {
        let mut policy = policy_with(mode, center, 1000., 500.);
        policy.map_window(
            TestWindow::new(1, 400, 300).with_requested_position(50, 60),
            ms(1),
        );

        assert_eq!(placed(&policy, 1), (50, 60), "{mode:?}, centring {center}");
        assert_eq!(policy.placing(SEAT), None);
    }
}

#[test]
fn a_dialog_is_centred_over_its_parent_in_every_mode() {
    for (mode, center) in [
        (PlacementMode::Pointer, false),
        (PlacementMode::Manual, false),
        (PlacementMode::Automatic, true),
    ] {
        let mut policy = policy_with(mode, center, 1000., 500.);
        policy.map_window(
            TestWindow::new(1, 800, 600).with_requested_position(159, 159),
            ms(1),
        );
        policy.map_window(
            TestWindow::new(2, 200, 100)
                .with_type(WindowType::Dialog)
                .with_parent(1)
                .with_frame(5, 5, 25, 5),
            ms(2),
        );

        // As in `placement.rs`'s automatic mode.
        let expected = (159 + 300, 159 + 166 + 25);
        assert_eq!(placed(&policy, 2), expected, "{mode:?}, centring {center}");
        assert_eq!(policy.placing(SEAT), None);
    }
}

#[test]
fn a_parentless_dialog_is_centred_on_the_output_in_every_mode() {
    for (mode, center) in [
        (PlacementMode::Pointer, false),
        (PlacementMode::Manual, false),
        (PlacementMode::Automatic, true),
    ] {
        let mut policy = policy_with(mode, center, 100., 100.);
        policy.map_window(
            TestWindow::new(1, 200, 100).with_type(WindowType::Dialog),
            ms(1),
        );

        assert_eq!(
            placed(&policy, 1),
            (860, 490),
            "{mode:?}, centring {center}"
        );
        assert_eq!(policy.placing(SEAT), None);
    }
}

#[test]
fn a_window_mapped_maximised_or_fullscreen_goes_back_to_where_automatic_placement_puts_it() {
    let maximized = MapStates {
        maximized: true,
        fullscreen: None,
    };
    let fullscreen = MapStates {
        maximized: false,
        fullscreen: Some(None),
    };
    for states in [maximized, fullscreen] {
        for (mode, center) in [
            (PlacementMode::Pointer, false),
            (PlacementMode::Manual, false),
            (PlacementMode::Automatic, true),
        ] {
            let mut policy = policy_with(mode, center, 1000., 500.);
            policy.map_window_with(TestWindow::new(1, 400, 300), states, ms(1));
            assert!(policy.is_maximized(id(1)) || policy.is_fullscreen(id(1)));
            assert_eq!(policy.placing(SEAT), None, "{mode:?}, centring {center}");

            policy.set_maximized(id(1), false);
            policy.set_fullscreen(id(1), false, None);

            // The center tile, as in `placement.rs`.
            assert_eq!(placed(&policy, 1), (158, 59), "{mode:?}, centring {center}");
        }
    }
}

// The keys changing.

#[test]
fn a_new_placement_mode_applies_to_the_next_window() {
    let mut policy = policy_with(PlacementMode::Automatic, false, 1000., 500.);
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    assert_eq!(placed(&policy, 1), (158, 59));

    policy.settings_changed(config(PlacementMode::Pointer, false));
    assert_eq!(placed(&policy, 1), (158, 59));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    assert_eq!(placed(&policy, 2), (800, 350));

    policy.settings_changed(config(PlacementMode::Automatic, true));
    policy.map_window(TestWindow::new(3, 400, 300), ms(3));
    assert_eq!(placed(&policy, 3), (760, 390));
}
