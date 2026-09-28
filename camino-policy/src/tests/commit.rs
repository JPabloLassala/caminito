use super::*;

#[test]
fn a_committed_size_is_the_windows_size_without_a_new_request() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    let calls = window(&policy, 1).calls.len();

    policy.window_mut(id(1)).unwrap().size = (500, 350).into();
    policy.commit(id(1));

    assert_eq!(policy.geometry(id(1)).unwrap().size, (500, 350).into());
    assert_eq!(window(&policy, 1).calls.len(), calls);
}

#[test]
fn a_window_growing_onto_another_output_is_on_both() {
    let mut policy = policy();
    policy.add_output(output(2, 1920, 0, 1280, 1024));
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(1700, 0),
        ms(1),
    );
    assert_eq!(
        window(&policy, 1).last_outputs(),
        Some(&[OutputId(1), OutputId(2)][..])
    );

    policy.window_mut(id(1)).unwrap().size = (100, 300).into();
    policy.commit(id(1));

    assert_eq!(window(&policy, 1).last_outputs(), Some(&[OutputId(1)][..]));
}

#[test]
fn a_new_parent_restacks_and_moves_the_transient() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.move_to_workspace(id(1), 1);
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));

    policy.window_mut(id(2)).unwrap().parent = Some(id(1));
    policy.commit(id(2));

    assert_eq!(policy.parent(id(2)), Some(id(1)));
    assert_eq!(policy.stacking(1), [id(1), id(2)]);
}

#[test]
fn a_parent_that_would_make_a_cycle_is_refused() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300).with_parent(1), ms(2));

    policy.window_mut(id(1)).unwrap().parent = Some(id(2));
    policy.commit(id(1));

    assert_eq!(policy.parent(id(1)), None);
}

#[test]
fn a_new_type_changes_the_layer() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));

    policy.window_mut(id(1)).unwrap().window_type = WindowType::Dock;
    policy.commit(id(1));

    assert_eq!(policy.stacking(0), [id(2), id(1)]);
}

#[test]
fn a_new_frame_keeps_a_maximised_window_in_its_work_area() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.set_maximized(id(1), true);

    let w = policy.window_mut(id(1)).unwrap();
    w.ssd = true;
    w.extents = smithay::utils::FrameExtents::new(5, 5, 25, 5);
    policy.commit(id(1));

    let (geometry, _) = window(&policy, 1).last_geometry().unwrap();
    assert_eq!(
        geometry,
        Rectangle::new((5, 25).into(), (1910, 1050).into())
    );
}
