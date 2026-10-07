use super::*;
use crate::SwitchList;

#[test]
fn switch_windows_lists_the_active_workspace_most_recently_used_first() {
    let mut policy = policy();
    for w in 1..=3 {
        policy.map_window(TestWindow::new(w, 400, 300), ms(w));
    }
    policy.click(SEAT, id(1), ms(4));
    policy.map_window(TestWindow::new(4, 400, 300), ms(5));
    policy.move_to_workspace(id(4), 1);

    assert_eq!(
        policy.switch_list(SEAT, SwitchList::Windows),
        [id(1), id(3), id(2)]
    );
}

#[test]
fn minimised_windows_come_after_the_others() {
    let mut policy = policy();
    for w in 1..=3 {
        policy.map_window(TestWindow::new(w, 400, 300), ms(w));
    }
    policy.minimize(id(3));

    // 3 was the most recently used, and is last for being minimised.
    assert_eq!(
        policy.switch_list(SEAT, SwitchList::Windows),
        [id(2), id(1), id(3)]
    );
}

#[test]
fn a_window_with_a_modal_dialog_is_listed_as_its_dialog() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.map_window(
        TestWindow::new(3, 200, 100)
            .with_type(WindowType::ModalDialog)
            .with_parent(1)
            .skipping_taskbar(),
        ms(3),
    );

    assert_eq!(
        policy.switch_list(SEAT, SwitchList::Windows),
        [id(3), id(2)]
    );
}

#[test]
fn a_modal_dialog_that_does_not_skip_the_taskbar_is_listed_once() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(
        TestWindow::new(2, 200, 100)
            .with_type(WindowType::ModalDialog)
            .with_parent(1),
        ms(2),
    );

    assert_eq!(policy.switch_list(SEAT, SwitchList::Windows), [id(2)]);
}

#[test]
fn a_minimised_modal_dialog_leaves_its_parent_listed_as_itself() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.map_window(
        TestWindow::new(3, 200, 100)
            .with_type(WindowType::ModalDialog)
            .with_parent(1)
            .skipping_taskbar(),
        ms(3),
    );
    policy.minimize(id(3));

    let list = policy.switch_list(SEAT, SwitchList::Windows);
    assert!(list.contains(&id(1)));
    assert!(!list.contains(&id(3)));
}

#[test]
fn windows_that_skip_the_taskbar_are_left_out() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300).skipping_taskbar(), ms(2));

    assert_eq!(policy.switch_list(SEAT, SwitchList::Windows), [id(1)]);
}

#[test]
fn docks_and_the_desktop_are_left_out_of_all_but_switch_panels() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(
        TestWindow::new(2, 1920, 30).with_type(WindowType::Dock),
        ms(2),
    );
    policy.map_window(
        TestWindow::new(3, 1920, 1080).with_type(WindowType::Desktop),
        ms(3),
    );
    policy.click(SEAT, id(1), ms(4));

    assert_eq!(policy.switch_list(SEAT, SwitchList::Windows), [id(1)]);
    assert_eq!(policy.switch_list(SEAT, SwitchList::WindowsAll), [id(1)]);
    assert_eq!(policy.switch_list(SEAT, SwitchList::Group), [id(1)]);
    let mut panels = policy.switch_list(SEAT, SwitchList::Panels);
    panels.sort();
    assert_eq!(panels, [id(2), id(3)]);
}

#[test]
fn switch_windows_all_lists_every_workspace() {
    let mut policy = policy();
    policy.settings_changed(Config {
        workspace_count: 2,
        ..Config::default()
    });
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.move_to_workspace(id(1), 1);

    assert_eq!(policy.switch_list(SEAT, SwitchList::Windows), [id(2)]);
    assert_eq!(
        policy.switch_list(SEAT, SwitchList::WindowsAll),
        [id(2), id(1)]
    );
}

#[test]
fn switch_group_lists_the_focused_applications_windows() {
    let mut policy = policy();
    // By pid.
    policy.map_window(TestWindow::new(1, 400, 300).with_pid(10), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300).with_pid(20), ms(2));
    policy.map_window(TestWindow::new(3, 400, 300).with_pid(10), ms(3));
    policy.click(SEAT, id(1), ms(4));
    assert_eq!(policy.switch_list(SEAT, SwitchList::Group), [id(1), id(3)]);

    // By app id, when there is no pid.
    let mut policy = self::policy();
    policy.map_window(TestWindow::new(1, 400, 300).with_app_id("a"), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300).with_app_id("b"), ms(2));
    policy.map_window(TestWindow::new(3, 400, 300).with_app_id("a"), ms(3));
    policy.click(SEAT, id(1), ms(4));
    assert_eq!(policy.switch_list(SEAT, SwitchList::Group), [id(1), id(3)]);

    // The focused window alone, with neither.
    let mut policy = self::policy();
    for w in 1..=2 {
        policy.map_window(TestWindow::new(w, 400, 300), ms(w));
    }
    policy.click(SEAT, id(1), ms(3));
    assert_eq!(policy.switch_list(SEAT, SwitchList::Group), [id(1)]);
}

#[test]
fn switch_group_lists_nothing_with_no_focused_window() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300).without_input(), ms(1));

    assert_eq!(policy.focus(SEAT), None);
    assert_eq!(policy.switch_list(SEAT, SwitchList::Group), []);
}

#[test]
fn windows_that_never_had_the_focus_come_after_the_others_topmost_first() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300).without_input(), ms(2));
    policy.map_window(TestWindow::new(3, 400, 300).without_input(), ms(3));

    let top_down: Vec<_> = policy.stacking(0).iter().rev().copied().collect();
    let unfocused: Vec<_> = top_down.into_iter().filter(|w| *w != id(1)).collect();
    let mut expected = vec![id(1)];
    expected.extend(unfocused);
    assert_eq!(policy.switch_list(SEAT, SwitchList::Windows), expected);
}

#[test]
fn a_window_hidden_by_show_desktop_is_still_listed() {
    let mut policy = policy();
    for w in 1..=2 {
        policy.map_window(TestWindow::new(w, 400, 300), ms(w));
    }
    policy.set_showing_desktop(true);

    assert_eq!(
        policy.switch_list(SEAT, SwitchList::Windows),
        [id(2), id(1)]
    );
}
