use super::*;

fn two_outputs() -> Policy<TestWindow> {
    let mut policy = policy();
    policy.add_output(output(2, 1920, 0, 1280, 1024));
    policy
}

#[test]
fn windows_on_a_removed_output_move_onto_a_remaining_one() {
    let mut policy = two_outputs();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(3000, 900),
        ms(1),
    );

    policy.remove_output(OutputId(2));

    assert_eq!(policy.geometry(id(1)).unwrap().loc, (1520, 780).into());
    assert_eq!(window(&policy, 1).last_outputs(), Some(&[OutputId(1)][..]));
}

#[test]
fn a_maximised_window_follows_its_output_away() {
    let mut policy = two_outputs();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(2000, 100),
        ms(1),
    );
    policy.set_maximized(id(1), true);
    assert_eq!(
        window(&policy, 1).last_geometry().unwrap().0,
        Rectangle::new((1920, 0).into(), (1280, 1024).into())
    );

    policy.remove_output(OutputId(2));

    assert_eq!(
        window(&policy, 1).last_geometry().unwrap().0,
        Rectangle::new((0, 0).into(), (1920, 1080).into())
    );
}

#[test]
fn a_resized_output_resizes_its_fullscreen_window() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.set_fullscreen(id(1), true, None);

    policy.add_output(output(1, 0, 0, 2560, 1440));

    assert_eq!(
        window(&policy, 1).last_geometry().unwrap().0,
        Rectangle::new((0, 0).into(), (2560, 1440).into())
    );
}

#[test]
fn without_outputs_windows_keep_their_place_and_are_on_none() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    let loc = policy.geometry(id(1)).unwrap().loc;

    policy.remove_output(OutputId(1));

    assert_eq!(policy.geometry(id(1)).unwrap().loc, loc);
    assert_eq!(window(&policy, 1).last_outputs(), Some(&[][..]));
}

#[test]
fn a_window_maximised_on_a_removed_output_unmaximises_onto_a_remaining_one() {
    let mut policy = two_outputs();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(3000, 900),
        ms(1),
    );
    policy.set_maximized(id(1), true);

    policy.remove_output(OutputId(2));
    policy.set_maximized(id(1), false);

    assert_eq!(policy.geometry(id(1)).unwrap().loc, (1520, 780).into());
    assert_eq!(window(&policy, 1).last_outputs(), Some(&[OutputId(1)][..]));
}

#[test]
fn windows_move_with_their_output_when_it_moves() {
    let mut policy = two_outputs();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(2000, 100),
        ms(1),
    );
    policy.map_window(
        TestWindow::new(2, 400, 300).with_requested_position(100, 100),
        ms(2),
    );

    policy.add_output(output(2, 0, 1080, 1280, 1024));

    assert_eq!(policy.geometry(id(1)).unwrap().loc, (80, 1180).into());
    assert_eq!(policy.geometry(id(2)).unwrap().loc, (100, 100).into());
}

#[test]
fn a_maximised_window_restores_onto_its_moved_output() {
    let mut policy = two_outputs();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(2000, 100),
        ms(1),
    );
    policy.set_maximized(id(1), true);

    policy.add_output(output(2, 0, 1080, 1280, 1024));
    policy.set_maximized(id(1), false);

    assert_eq!(policy.geometry(id(1)).unwrap().loc, (80, 1180).into());
}

#[test]
fn removing_the_left_output_and_moving_the_right_one_to_the_origin_keeps_windows_on_it() {
    let mut policy = two_outputs();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(2000, 100),
        ms(1),
    );

    policy.set_outputs(vec![output(2, 0, 0, 1280, 1024)]);

    assert_eq!(policy.geometry(id(1)).unwrap().loc, (80, 100).into());
    assert_eq!(window(&policy, 1).last_outputs(), Some(&[OutputId(2)][..]));
}
