//! Marco's automatic placement (`place.c`). Expected positions are worked by hand from Marco's
//! arithmetic on a 1920×1080 output.

use super::*;

fn placed(policy: &Policy<TestWindow>, w: u64) -> (i32, i32) {
    let (geometry, _) = window(policy, w)
        .last_geometry()
        .expect("window was placed");
    (geometry.loc.x, geometry.loc.y)
}

#[test]
fn the_first_window_goes_on_the_center_tile() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    // x: 1920 % 401 / 2, y: 1080 % 301 / 3.
    assert_eq!(placed(&policy, 1), (158, 59));
    let (geometry, states) = window(&policy, 1).last_geometry().unwrap();
    assert_eq!(geometry.size, (400, 300).into());
    assert_eq!(states, WindowStates::default());
}

#[test]
fn placement_uses_the_frame() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300).with_frame(5, 5, 25, 5), ms(1));

    // The 410×330 frame at (1920 % 411 / 2, 1080 % 331 / 3).
    assert_eq!(placed(&policy, 1), (138 + 5, 29 + 25));
}

#[test]
fn later_windows_go_below_the_first_window_they_fit_under() {
    let mut policy = policy();
    for w in 1..=3 {
        policy.map_window(TestWindow::new(w, 400, 300), ms(w));
    }

    assert_eq!(placed(&policy, 2), (158, 359));
    assert_eq!(placed(&policy, 3), (158, 659));
}

#[test]
fn a_window_goes_right_of_one_when_nothing_fits_below() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 600), ms(1));
    policy.map_window(TestWindow::new(2, 400, 600), ms(2));

    assert_eq!(placed(&policy, 1), (158, 159));
    assert_eq!(placed(&policy, 2), (558, 159));
}

#[test]
fn windows_cascade_from_the_corner_when_nothing_fits() {
    let mut policy = policy();
    for w in 1..=3 {
        policy.map_window(TestWindow::new(w, 1000, 700).with_frame(5, 5, 25, 5), ms(w));
    }

    assert_eq!(placed(&policy, 1), (454 + 5, 116 + 25));
    // The frame goes in the corner, then at the client origin of the window in the way.
    assert_eq!(placed(&policy, 2), (5, 25));
    assert_eq!(placed(&policy, 3), (10, 50));
}

#[test]
fn frameless_windows_cascade_by_the_fuzz() {
    let mut policy = policy();
    for w in 1..=3 {
        policy.map_window(TestWindow::new(w, 1000, 700), ms(w));
    }

    assert_eq!(placed(&policy, 2), (0, 0));
    assert_eq!(placed(&policy, 3), (15, 15));
}

#[test]
fn dialogs_dont_block_placement() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_type(WindowType::Dialog),
        ms(1),
    );
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));

    assert_eq!(placed(&policy, 2), (158, 59));
}

#[test]
fn a_dialog_is_centered_over_its_parent_a_third_of_the_way_down() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 800, 600), ms(1));
    policy.map_window(
        TestWindow::new(2, 200, 100)
            .with_type(WindowType::Dialog)
            .with_parent(1)
            .with_frame(5, 5, 25, 5),
        ms(2),
    );

    assert_eq!(placed(&policy, 1), (159, 159));
    assert_eq!(placed(&policy, 2), (159 + 300, 159 + 166 + 25));
}

#[test]
fn a_dialog_over_a_parent_at_the_edge_is_kept_on_screen() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(1700, 50),
        ms(1),
    );
    policy.map_window(
        TestWindow::new(2, 300, 100)
            .with_type(WindowType::ModalDialog)
            .with_parent(1),
        ms(2),
    );

    assert_eq!(placed(&policy, 1), (1700, 50));
    assert_eq!(placed(&policy, 2), (1620, 116));
}

#[test]
fn parentless_dialogs_and_splash_screens_are_centered_on_the_output() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 200, 100).with_type(WindowType::Dialog),
        ms(1),
    );
    policy.map_window(
        TestWindow::new(2, 300, 200).with_type(WindowType::Splashscreen),
        ms(2),
    );

    assert_eq!(placed(&policy, 1), (860, 490));
    assert_eq!(placed(&policy, 2), (810, 440));
}

#[test]
fn a_requested_position_is_honoured() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(50, 60),
        ms(1),
    );

    assert_eq!(placed(&policy, 1), (50, 60));
}

#[test]
fn windows_are_placed_on_the_output_of_the_focused_window() {
    let mut policy = policy();
    policy.add_output(output(2, 1920, 0, 1280, 1024));
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(2000, 500),
        ms(1),
    );

    policy.map_window(TestWindow::new(2, 400, 300), ms(2));

    // The center tile of the second output: 1280 % 401 / 2, 1024 % 301 / 3.
    assert_eq!(placed(&policy, 2), (1920 + 38, 40));
}

#[test]
fn windows_on_other_workspaces_dont_block_placement() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.move_to_workspace(id(1), 1);

    policy.map_window(TestWindow::new(2, 400, 300), ms(2));

    assert_eq!(placed(&policy, 2), (158, 59));
}
