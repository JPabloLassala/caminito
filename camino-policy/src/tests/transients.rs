use super::*;

#[test]
fn a_transient_stays_above_its_raised_parent() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(
        TestWindow::new(2, 200, 100)
            .with_type(WindowType::Dialog)
            .with_parent(1),
        ms(2),
    );
    policy.map_window(TestWindow::new(3, 400, 300), ms(3));

    policy.click(SEAT, id(1), ms(4));

    assert_eq!(policy.stacking(0), [id(3), id(1), id(2)]);
    assert_eq!(policy.parent(id(2)), Some(id(1)));
}

#[test]
fn clicking_a_transient_raises_its_parent_too() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 200, 100).with_parent(1), ms(2));
    policy.map_window(TestWindow::new(3, 400, 300), ms(3));

    policy.click(SEAT, id(2), ms(4));

    assert_eq!(policy.stacking(0), [id(3), id(1), id(2)]);
    assert_eq!(policy.focus(SEAT), Some(id(2)));
}

#[test]
fn a_transient_mapped_before_its_parent_is_adopted_when_the_parent_maps() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(2, 200, 100).with_parent(1), ms(1));
    assert_eq!(policy.parent(id(2)), None);

    policy.map_window(TestWindow::new(1, 400, 300), ms(2));

    assert_eq!(policy.parent(id(2)), Some(id(1)));
    assert_eq!(policy.stacking(0), [id(1), id(2)]);
}

#[test]
fn unmapping_a_focused_transient_focuses_its_parent() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(3, 400, 300), ms(2));
    policy.map_window(TestWindow::new(2, 200, 100).with_parent(1), ms(3));
    assert_eq!(policy.stacking(0), [id(1), id(3), id(2)]);

    policy.unmap_window(id(2));

    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn a_transient_takes_its_parents_layer() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 1920, 30).with_type(WindowType::Dock),
        ms(1),
    );
    policy.map_window(TestWindow::new(2, 200, 100).with_parent(1), ms(2));
    policy.map_window(TestWindow::new(3, 400, 300), ms(3));

    assert_eq!(policy.layer(id(2)), Some(Layer::Top));
    assert_eq!(policy.stacking(0), [id(3), id(1), id(2)]);
}

#[test]
fn transient_cycles_are_broken() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300).with_parent(2), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300).with_parent(1), ms(2));

    assert_eq!(policy.parent(id(2)), Some(id(1)));
    assert_eq!(policy.parent(id(1)), None);
}
