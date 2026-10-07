//! The work area: each output less what's reserved along its edges (64).

use super::*;

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Logical> {
    Rectangle::new((x, y).into(), (w, h).into())
}

/// A policy whose 1920×1080 output has a panel of 28 on top and a dock of 64 on the left.
fn with_a_panel_and_a_dock() -> Policy<TestWindow> {
    let mut policy = policy();
    policy.add_output(reserving(output(1, 0, 0, 1920, 1080), 64, 0, 28, 0));
    policy
}

fn sent(policy: &Policy<TestWindow>, w: u64) -> Rectangle<i32, Logical> {
    window(policy, w).last_geometry().expect("was configured").0
}

#[test]
fn the_work_area_is_the_output_less_its_top_and_left_reservations() {
    let policy = with_a_panel_and_a_dock();

    assert_eq!(
        policy.work_areas().collect::<Vec<_>>(),
        [rect(64, 28, 1856, 1052)]
    );
}

#[test]
fn every_edge_can_be_reserved() {
    let mut policy = policy();
    policy.add_output(reserving(output(1, 100, 50, 1920, 1080), 1, 2, 3, 4));

    assert_eq!(
        policy.work_areas().collect::<Vec<_>>(),
        [rect(101, 53, 1917, 1073)]
    );
}

#[test]
fn reservations_on_one_output_leave_the_other_whole() {
    let mut policy = with_a_panel_and_a_dock();
    policy.add_output(output(2, 1920, 0, 1280, 1024));

    assert_eq!(
        policy.work_areas().collect::<Vec<_>>(),
        [rect(64, 28, 1856, 1052), rect(1920, 0, 1280, 1024)]
    );
}

#[test]
fn reservations_larger_than_the_output_leave_a_work_area_of_one_pixel() {
    let mut policy = policy();
    policy.add_output(reserving(output(1, 0, 0, 1920, 1080), 1000, 1000, 2000, 0));

    let area = policy.work_areas().next().unwrap();
    assert_eq!(area.size, (1, 1).into());
    assert!(output(1, 0, 0, 1920, 1080).geometry.contains_rect(area));
}

#[test]
fn maximising_fills_the_work_area() {
    let mut policy = with_a_panel_and_a_dock();
    policy.map_window(TestWindow::new(1, 400, 300).with_frame(5, 5, 25, 5), ms(1));

    policy.set_maximized(id(1), true);

    assert_eq!(sent(&policy, 1), rect(64 + 5, 28 + 25, 1846, 1022));
}

#[test]
fn maximising_one_way_fills_the_work_area_that_way() {
    let mut policy = with_a_panel_and_a_dock();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    let placed = sent(&policy, 1);

    policy.set_maximized_vertically(id(1), true);

    assert_eq!(sent(&policy, 1), rect(placed.loc.x, 28, 400, 1052));
}

#[test]
fn placement_stays_in_the_work_area() {
    let mut policy = with_a_panel_and_a_dock();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 1800, 1000), ms(2));

    // The centre tile of the work area: x: 64 + 1856 % 401 / 2, y: 28 + 1052 % 301 / 3.
    assert_eq!(sent(&policy, 1).loc, (64 + 126, 28 + 49).into());
    // Nothing fits beside the first window, so the second cascades from the area's corner.
    assert_eq!(sent(&policy, 2).loc, (64, 28).into());
}

#[test]
fn a_framed_window_as_big_as_the_work_area_maps_maximised() {
    let mut policy = with_a_panel_and_a_dock();
    policy.map_window(
        TestWindow::new(1, 1846, 1022).with_frame(5, 5, 25, 5),
        ms(1),
    );

    assert!(policy.is_maximized(id(1)));
}

#[test]
fn a_change_reconfigures_maximised_windows_only() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(0, 0),
        ms(1),
    );
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.set_maximized(id(2), true);
    assert_eq!(sent(&policy, 2), rect(0, 0, 1920, 1080));
    let calls = window(&policy, 1).calls.len();

    // A panel maps on top of the first window, which isn't moved from under it.
    policy.add_output(reserving(output(1, 0, 0, 1920, 1080), 0, 0, 28, 0));

    assert_eq!(sent(&policy, 2), rect(0, 28, 1920, 1052));
    assert_eq!(sent(&policy, 1), rect(0, 0, 400, 300));
    assert_eq!(window(&policy, 1).calls.len(), calls);

    // The panel goes, and the space comes back.
    policy.add_output(output(1, 0, 0, 1920, 1080));

    assert_eq!(sent(&policy, 2), rect(0, 0, 1920, 1080));
    assert_eq!(window(&policy, 1).calls.len(), calls);
}

#[test]
fn a_maximised_window_unmaximises_to_where_it_was_whatever_was_reserved_meanwhile() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(0, 0),
        ms(1),
    );
    policy.set_maximized(id(1), true);

    policy.add_output(reserving(output(1, 0, 0, 1920, 1080), 0, 0, 28, 0));
    policy.set_maximized(id(1), false);

    assert_eq!(sent(&policy, 1), rect(0, 0, 400, 300));
}

#[test]
fn fullscreen_ignores_the_reservations() {
    let mut policy = with_a_panel_and_a_dock();
    policy.map_window(TestWindow::new(1, 400, 300).with_frame(5, 5, 25, 5), ms(1));

    policy.set_fullscreen(id(1), true, None);

    assert_eq!(sent(&policy, 1), rect(0, 0, 1920, 1080));

    // Nor does it follow them when they change.
    policy.add_output(reserving(output(1, 0, 0, 1920, 1080), 0, 0, 0, 40));

    assert_eq!(sent(&policy, 1), rect(0, 0, 1920, 1080));
}

#[test]
fn a_requested_geometry_is_kept_in_the_work_area() {
    let mut policy = with_a_panel_and_a_dock();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    policy.request_geometry(id(1), Some((0, 0).into()), Some((3000, 2000).into()));

    assert_eq!(sent(&policy, 1), rect(64, 28, 1856, 1052));
}
