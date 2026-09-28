use super::*;

use crate::ResizeEdges;

#[test]
fn shading_an_ssd_window_keeps_it_focused_and_unshading_restores_it() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300).with_frame(5, 5, 25, 5), ms(1));
    let (placed, _) = window(&policy, 1).last_geometry().unwrap();

    policy.set_shaded(id(1), true);
    assert!(policy.is_shaded(id(1)));
    assert_eq!(policy.focus(SEAT), Some(id(1)));
    let (geometry, states) = window(&policy, 1).last_geometry().unwrap();
    assert_eq!(geometry, placed);
    assert!(states.shaded);

    policy.set_shaded(id(1), false);
    assert_eq!(
        window(&policy, 1).last_geometry(),
        Some((placed, WindowStates::default()))
    );
}

#[test]
fn a_csd_window_cant_be_shaded() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    policy.set_shaded(id(1), true);

    assert!(!policy.is_shaded(id(1)));
}

#[test]
fn a_shaded_window_that_turns_csd_is_unshaded() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300).with_frame(5, 5, 25, 5), ms(1));
    policy.set_shaded(id(1), true);

    policy.window_mut(id(1)).unwrap().ssd = false;
    policy.frame_changed(id(1));

    assert!(!policy.is_shaded(id(1)));
}

#[test]
fn a_shaded_window_is_only_its_titlebar_on_the_outputs() {
    let mut policy = Policy::new(Config::default());
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy.add_output(output(2, 0, 1080, 1920, 1080));
    // The frame reaches down onto the second output.
    policy.map_window(
        TestWindow::new(1, 400, 300)
            .with_frame(5, 5, 25, 5)
            .with_requested_position(100, 900),
        ms(1),
    );
    assert_eq!(
        window(&policy, 1).last_outputs(),
        Some(&[OutputId(1), OutputId(2)][..])
    );

    policy.set_shaded(id(1), true);

    assert_eq!(window(&policy, 1).last_outputs(), Some(&[OutputId(1)][..]));
}

#[test]
fn a_shaded_window_moves_but_doesnt_resize() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300).with_frame(5, 5, 25, 5), ms(1));
    policy.set_shaded(id(1), true);

    assert!(!policy.begin_resize(SEAT, id(1), ResizeEdges::BOTTOM_RIGHT, Point::new(0., 0.)));
    assert!(policy.begin_move(SEAT, id(1), Point::new(0., 0.)));
}

#[test]
fn going_fullscreen_or_maximising_vertically_unshades_as_marco_does() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300).with_frame(5, 5, 25, 5), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300).with_frame(5, 5, 25, 5), ms(2));
    policy.map_window(TestWindow::new(3, 400, 300).with_frame(5, 5, 25, 5), ms(3));
    for w in 1..=3 {
        policy.set_shaded(id(w), true);
    }

    policy.set_fullscreen(id(1), true, None);
    policy.set_maximized_vertically(id(2), true);
    policy.set_maximized_horizontally(id(3), true);

    assert!(!policy.is_shaded(id(1)));
    assert!(!policy.is_shaded(id(2)));
    assert!(policy.is_shaded(id(3)));
}
