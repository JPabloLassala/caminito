use smithay::utils::FrameExtents;

use super::*;

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Logical> {
    Rectangle::new((x, y).into(), (w, h).into())
}

#[test]
fn maximising_fills_the_work_area_with_the_frame_and_unmaximising_restores() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300).with_frame(5, 5, 25, 5), ms(1));
    let (placed, _) = window(&policy, 1).last_geometry().unwrap();

    policy.set_maximized(id(1), true);
    let (geometry, states) = window(&policy, 1).last_geometry().unwrap();
    assert_eq!(geometry, rect(5, 25, 1910, 1050));
    assert!(states.maximized);

    policy.set_maximized(id(1), false);
    assert_eq!(
        window(&policy, 1).last_geometry(),
        Some((placed, WindowStates::default()))
    );
}

#[test]
fn a_maximised_frame_fills_the_work_area_with_its_maximised_borders() {
    let mut policy = policy();
    let mut w = TestWindow::new(1, 400, 300).with_frame(5, 5, 25, 5);
    w.maximized_extents = FrameExtents::new(0, 0, 20, 0);
    policy.map_window(w, ms(1));
    let (placed, _) = window(&policy, 1).last_geometry().unwrap();

    policy.set_maximized(id(1), true);
    let (geometry, _) = window(&policy, 1).last_geometry().unwrap();
    assert_eq!(geometry, rect(0, 20, 1920, 1060));

    // The floating frame is the one to restore.
    policy.set_maximized(id(1), false);
    let (geometry, _) = window(&policy, 1).last_geometry().unwrap();
    assert_eq!(geometry, placed);
}

#[test]
fn toggle_maximized_acts_on_the_seats_focused_window() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));

    policy.action(SEAT, Action::ToggleMaximized, ms(3));
    assert!(policy.is_maximized(id(2)));
    assert!(!policy.is_maximized(id(1)));

    policy.action(SEAT, Action::ToggleMaximized, ms(4));
    assert!(!policy.is_maximized(id(2)));
}

#[test]
fn a_framed_window_too_big_for_the_work_area_maps_maximised() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 1915, 1060).with_frame(5, 5, 25, 5),
        ms(1),
    );
    policy.map_window(TestWindow::new(2, 1915, 1060), ms(2));

    assert!(policy.is_maximized(id(1)));
    // Marco assumes a window it doesn't frame doesn't mean to be maximised, so a CSD window isn't.
    assert!(!policy.is_maximized(id(2)));
}

#[test]
fn fullscreen_covers_the_output_without_the_frame_and_leaving_restores() {
    let mut policy = policy();
    policy.add_output(output(2, 1920, 0, 1280, 1024));
    policy.map_window(TestWindow::new(1, 400, 300).with_frame(5, 5, 25, 5), ms(1));
    let (placed, _) = window(&policy, 1).last_geometry().unwrap();

    policy.set_fullscreen(id(1), true, None);
    let (geometry, states) = window(&policy, 1).last_geometry().unwrap();
    assert_eq!(geometry, rect(0, 0, 1920, 1080));
    assert!(states.fullscreen);

    policy.set_fullscreen(id(1), true, Some(OutputId(2)));
    assert_eq!(
        window(&policy, 1).last_geometry().unwrap().0,
        rect(1920, 0, 1280, 1024)
    );
    assert_eq!(window(&policy, 1).last_outputs(), Some(&[OutputId(2)][..]));

    policy.set_fullscreen(id(1), false, None);
    assert_eq!(
        window(&policy, 1).last_geometry(),
        Some((placed, WindowStates::default()))
    );
}

#[test]
fn a_maximised_window_leaving_fullscreen_is_maximised_again() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.set_maximized(id(1), true);

    policy.set_fullscreen(id(1), true, None);
    policy.set_fullscreen(id(1), false, None);

    let (geometry, states) = window(&policy, 1).last_geometry().unwrap();
    assert_eq!(geometry, rect(0, 0, 1920, 1080));
    assert!(states.maximized && !states.fullscreen);
}

#[test]
fn a_focused_fullscreen_window_goes_above_docks_until_it_loses_focus() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 1920, 30).with_type(WindowType::Dock),
        ms(1),
    );
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.map_window(TestWindow::new(3, 400, 300), ms(3));
    policy.click(SEAT, id(2), ms(4));

    policy.set_fullscreen(id(2), true, None);
    assert_eq!(policy.layer(id(2)), Some(Layer::Fullscreen));
    assert_eq!(policy.stacking(0), [id(3), id(1), id(2)]);

    policy.click(SEAT, id(3), ms(5));
    assert_eq!(policy.layer(id(2)), Some(Layer::Normal));
    assert_eq!(policy.stacking(0), [id(2), id(3), id(1)]);
}

#[test]
fn a_fullscreen_window_stays_up_while_its_transient_has_focus() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 1920, 30).with_type(WindowType::Dock),
        ms(1),
    );
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.set_fullscreen(id(2), true, None);

    policy.map_window(TestWindow::new(3, 200, 100).with_parent(2), ms(3));

    assert_eq!(policy.focus(SEAT), Some(id(3)));
    assert_eq!(policy.stacking(0), [id(1), id(2), id(3)]);
}

#[test]
fn minimising_hides_the_window_and_its_transients_and_moves_focus() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.map_window(TestWindow::new(3, 200, 100).with_parent(2), ms(3));

    policy.minimize(id(2));

    assert!(policy.is_minimized(id(2)));
    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert_eq!(window(&policy, 2).last_outputs(), Some(&[][..]));
    assert_eq!(window(&policy, 3).last_outputs(), Some(&[][..]));
    assert_eq!(policy.stacking(0), [id(1), id(2), id(3)]);
}

#[test]
fn a_minimised_window_cant_be_clicked_and_activating_restores_it() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.minimize(id(2));

    policy.click(SEAT, id(2), ms(3));
    assert_eq!(policy.focus(SEAT), Some(id(1)));

    policy.activate(SEAT, id(2), ms(4));
    assert!(!policy.is_minimized(id(2)));
    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert_eq!(window(&policy, 2).last_outputs(), Some(&[OutputId(1)][..]));
}

#[test]
fn maximising_on_an_output_smaller_than_the_frame_keeps_a_positive_size() {
    let mut policy = Policy::new(Config::default());
    policy.add_output(output(1, 0, 0, 1920, 20));
    policy.map_window(TestWindow::new(1, 400, 300).with_frame(5, 5, 25, 5), ms(1));

    policy.set_maximized(id(1), true);

    let (geometry, _) = window(&policy, 1).last_geometry().unwrap();
    assert_eq!(geometry.size, (1910, 1).into());
}
