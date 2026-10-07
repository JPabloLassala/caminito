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

/// A window with a modal dialog over it, and another window above both.
fn a_modal_dialog_over_its_parent() -> Policy<TestWindow> {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(
        TestWindow::new(2, 200, 100)
            .with_type(WindowType::ModalDialog)
            .with_parent(1),
        ms(2),
    );
    policy.map_window(TestWindow::new(3, 400, 300), ms(3));
    policy
}

#[test]
fn clicking_the_parent_of_a_modal_dialog_focuses_the_dialog() {
    let mut policy = a_modal_dialog_over_its_parent();

    policy.click(SEAT, id(1), ms(4));

    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert_eq!(policy.stacking(0), [id(3), id(1), id(2)]);
}

#[test]
fn activating_the_parent_of_a_modal_dialog_focuses_the_dialog() {
    let mut policy = a_modal_dialog_over_its_parent();

    policy.activate(SEAT, id(1), ms(4));

    assert_eq!(policy.focus(SEAT), Some(id(2)));
}

#[test]
fn focus_goes_to_the_last_of_a_chain_of_modal_dialogs() {
    let mut policy = a_modal_dialog_over_its_parent();
    policy.map_window(
        TestWindow::new(4, 100, 50)
            .with_type(WindowType::ModalDialog)
            .with_parent(2),
        ms(4),
    );
    policy.click(SEAT, id(3), ms(5));

    policy.click(SEAT, id(1), ms(6));

    assert_eq!(policy.focus(SEAT), Some(id(4)));
}

#[test]
fn a_dialog_that_stops_being_modal_lets_its_parent_be_focused() {
    let mut policy = a_modal_dialog_over_its_parent();
    policy.window_mut(id(2)).unwrap().window_type = WindowType::Dialog;
    policy.commit(id(2));

    policy.click(SEAT, id(1), ms(4));

    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn a_dialog_that_becomes_modal_takes_the_focus_its_parent_is_given() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(
        TestWindow::new(2, 200, 100)
            .with_type(WindowType::Dialog)
            .with_parent(1),
        ms(2),
    );
    policy.window_mut(id(2)).unwrap().window_type = WindowType::ModalDialog;
    policy.commit(id(2));

    policy.click(SEAT, id(1), ms(3));

    assert_eq!(policy.focus(SEAT), Some(id(2)));
}

#[test]
fn a_minimised_modal_dialog_leaves_its_parent_the_focus() {
    let mut policy = a_modal_dialog_over_its_parent();
    policy.minimize(id(2));

    policy.click(SEAT, id(1), ms(4));

    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn a_closed_transient_hands_the_focus_to_the_modal_dialog_over_its_parent() {
    let mut policy = a_modal_dialog_over_its_parent();
    policy.map_window(TestWindow::new(4, 200, 100).with_parent(1), ms(4));
    policy.click(SEAT, id(4), ms(5));
    assert_eq!(policy.focus(SEAT), Some(id(4)));

    policy.unmap_window(id(4));

    assert_eq!(policy.focus(SEAT), Some(id(2)));
}
