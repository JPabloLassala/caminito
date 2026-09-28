use super::*;

#[test]
fn docks_stay_above_normal_windows() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 1920, 30).with_type(WindowType::Dock),
        ms(1),
    );
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.click(SEAT, id(2), ms(3));

    assert_eq!(policy.stacking(0), [id(2), id(1)]);
}

#[test]
fn the_desktop_stays_below_normal_windows() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(
        TestWindow::new(2, 1920, 1080).with_type(WindowType::Desktop),
        ms(2),
    );
    policy.click(SEAT, id(2), ms(3));

    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert_eq!(policy.stacking(0), [id(2), id(1)]);
}

#[test]
fn a_raised_window_goes_to_the_top_of_its_own_layer() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 1920, 1080).with_type(WindowType::Desktop),
        ms(1),
    );
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.map_window(TestWindow::new(3, 400, 300), ms(3));
    policy.map_window(
        TestWindow::new(4, 1920, 30).with_type(WindowType::Dock),
        ms(4),
    );

    policy.click(SEAT, id(2), ms(5));

    assert_eq!(policy.stacking(0), [id(1), id(3), id(2), id(4)]);
}
