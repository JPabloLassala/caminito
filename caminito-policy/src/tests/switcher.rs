use super::*;
use crate::{ResizeEdges, SwitchList};

const SWITCH: Action = Action::Switch {
    list: SwitchList::Windows,
    backward: false,
};
const SWITCH_BACK: Action = Action::Switch {
    list: SwitchList::Windows,
    backward: true,
};

/// Three windows, 3 the most recently used and focused.
fn three() -> Policy<TestWindow> {
    let mut policy = policy();
    for w in 1..=3 {
        policy.map_window(TestWindow::new(w, 400, 300), ms(w));
    }
    policy
}

/// `three`, with `alt-tab-raise-windows` set.
fn three_raising() -> Policy<TestWindow> {
    let mut policy = three();
    policy.settings_changed(Config {
        alt_tab_raise_windows: true,
        ..Config::default()
    });
    policy
}

fn pointer() -> Point<f64, Logical> {
    Point::from((0., 0.))
}

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

#[test]
fn the_switcher_opens_on_the_second_entry_and_changes_no_focus() {
    let mut policy = three();
    policy.action(SEAT, SWITCH, ms(10));

    let switcher = policy.switcher().expect("the switcher is open");
    assert_eq!(switcher.windows, [id(3), id(2), id(1)]);
    assert_eq!(switcher.selected, 1);
    assert_eq!(policy.focus(SEAT), Some(id(3)));
    assert_eq!(policy.stacking(0), [id(1), id(2), id(3)]);
}

#[test]
fn the_switcher_opens_backward_on_the_last_entry() {
    let mut policy = three();
    policy.action(SEAT, SWITCH_BACK, ms(10));

    assert_eq!(policy.switcher().unwrap().selected, 2);
}

#[test]
fn stepping_wraps_both_ways() {
    let mut policy = three();
    policy.action(SEAT, SWITCH, ms(10));
    policy.action(SEAT, SWITCH, ms(11));
    assert_eq!(policy.switcher().unwrap().selected, 2);
    policy.action(SEAT, SWITCH, ms(12));
    assert_eq!(policy.switcher().unwrap().selected, 0);
    policy.action(SEAT, SWITCH_BACK, ms(13));
    assert_eq!(policy.switcher().unwrap().selected, 2);
}

#[test]
fn an_open_switcher_steps_whatever_list_the_action_names() {
    let mut policy = three();
    policy.action(SEAT, SWITCH, ms(10));
    let panels = Action::Switch {
        list: SwitchList::Panels,
        backward: false,
    };
    policy.action(SEAT, panels, ms(11));

    let switcher = policy.switcher().unwrap();
    assert_eq!(switcher.list, SwitchList::Windows);
    assert_eq!(switcher.selected, 2);
}

#[test]
fn selecting_an_entry_ignores_an_index_past_the_end() {
    let mut policy = three();
    policy.action(SEAT, SWITCH, ms(10));
    policy.switcher_select(0);
    assert_eq!(policy.switcher().unwrap().selected, 0);
    policy.switcher_select(3);
    assert_eq!(policy.switcher().unwrap().selected, 0);
}

#[test]
fn with_no_window_to_list_nothing_happens() {
    let mut policy = policy();
    policy.action(SEAT, SWITCH, ms(1));

    assert!(policy.switcher().is_none());
    assert_eq!(policy.focus(SEAT), None);
}

#[test]
fn with_one_window_it_is_activated_and_no_switcher_opens() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.minimize(id(1));
    assert_eq!(policy.focus(SEAT), None);

    policy.action(SEAT, SWITCH, ms(2));

    assert!(policy.switcher().is_none());
    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert!(!policy.is_minimized(id(1)));
}

#[test]
fn switch_panels_from_a_normal_window_starts_on_the_first_entry() {
    let mut policy = three();
    for w in 4..=5 {
        policy.map_window(
            TestWindow::new(w, 1920, 30).with_type(WindowType::Dock),
            ms(w),
        );
    }
    let panels = Action::Switch {
        list: SwitchList::Panels,
        backward: false,
    };
    policy.action(SEAT, panels, ms(10));

    let switcher = policy.switcher().unwrap();
    assert_eq!(switcher.windows, [id(5), id(4)]);
    assert_eq!(switcher.selected, 0);
    assert_eq!(policy.focus(SEAT), Some(id(3)));
}

#[test]
fn commit_activates_the_selected_window_and_restores_a_minimised_one() {
    let mut policy = three();
    policy.minimize(id(2));
    policy.action(SEAT, SWITCH, ms(10)); // 3, 1, then 2 minimised
    policy.switcher_select(2);
    policy.switcher_commit(ms(11));

    assert!(policy.switcher().is_none());
    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert!(!policy.is_minimized(id(2)));
    assert_eq!(policy.stacking(0).last(), Some(&id(2)));
}

#[test]
fn cancel_leaves_focus_and_stacking_as_they_were() {
    let mut policy = three();
    let before = policy.stacking(0).to_vec();
    policy.action(SEAT, SWITCH, ms(10));
    policy.switcher_step(false);
    policy.switcher_cancel(ms(11));

    assert!(policy.switcher().is_none());
    assert_eq!(policy.focus(SEAT), Some(id(3)));
    assert_eq!(policy.stacking(0), before);
}

#[test]
fn a_selected_window_that_closes_gives_the_selection_to_the_next() {
    let mut policy = three();
    policy.action(SEAT, SWITCH, ms(10));
    policy.unmap_window(id(2));

    let switcher = policy.switcher().unwrap();
    assert_eq!(switcher.windows, [id(3), id(1)]);
    assert_eq!(switcher.windows[switcher.selected], id(1));

    // The last entry closing gives the selection to the first.
    policy.unmap_window(id(1));
    let switcher = policy.switcher().unwrap();
    assert_eq!(switcher.windows, [id(3)]);
    assert_eq!(switcher.selected, 0);
}

#[test]
fn an_unselected_window_that_closes_leaves_the_grid_and_keeps_the_selection_on_its_window() {
    let mut policy = three();
    policy.action(SEAT, SWITCH, ms(10));
    policy.unmap_window(id(3));

    let switcher = policy.switcher().unwrap();
    assert_eq!(switcher.windows, [id(2), id(1)]);
    assert_eq!(switcher.windows[switcher.selected], id(2));

    policy.unmap_window(id(1));
    let switcher = policy.switcher().unwrap();
    assert_eq!(switcher.windows, [id(2)]);
    assert_eq!(switcher.windows[switcher.selected], id(2));
}

#[test]
fn the_last_listed_window_closing_closes_the_switcher() {
    let mut policy = three();
    policy.action(SEAT, SWITCH, ms(10));
    for w in 1..=3 {
        assert!(policy.switcher().is_some());
        policy.unmap_window(id(w));
    }

    assert!(policy.switcher().is_none());
}

#[test]
fn a_window_that_opens_meanwhile_is_not_listed_nor_focused_until_the_switcher_closes() {
    let mut cancelled = three();
    cancelled.action(SEAT, SWITCH, ms(10));
    cancelled.map_window(TestWindow::new(4, 400, 300), ms(11));

    assert_eq!(cancelled.switcher().unwrap().windows, [id(3), id(2), id(1)]);
    assert_eq!(cancelled.focus(SEAT), Some(id(3)));
    cancelled.switcher_cancel(ms(12));
    assert_eq!(cancelled.focus(SEAT), Some(id(4)));

    // After a commit it is mapped after the selected window was activated, so it has the focus.
    let mut committed = three();
    committed.action(SEAT, SWITCH, ms(10));
    committed.map_window(TestWindow::new(4, 400, 300), ms(11));
    committed.switcher_commit(ms(12));
    assert_eq!(committed.focus(SEAT), Some(id(4)));
    assert_eq!(committed.recently_focused().nth(1), Some(id(2)));
}

#[test]
fn a_window_that_opened_meanwhile_and_went_is_not_focused_when_the_switcher_closes() {
    let mut policy = three();
    policy.action(SEAT, SWITCH, ms(10));
    policy.map_window(TestWindow::new(4, 400, 300), ms(11));
    policy.unmap_window(id(4));
    policy.switcher_cancel(ms(12));

    assert_eq!(policy.focus(SEAT), Some(id(3)));
}

#[test]
fn the_selected_window_is_raised_while_open_with_alt_tab_raise_windows() {
    let mut policy = three_raising();
    policy.action(SEAT, SWITCH, ms(10));
    assert_eq!(policy.stacking(0), [id(1), id(3), id(2)]);

    // Only the selected window is out of place.
    policy.switcher_step(false);
    assert_eq!(policy.stacking(0), [id(2), id(3), id(1)]);
    assert_eq!(policy.focus(SEAT), Some(id(3)));
}

#[test]
fn the_selected_window_is_raised_while_open_with_alt_tab_raise_windows_and_goes_back_on_cancel() {
    let mut policy = three_raising();
    let before = policy.stacking(0).to_vec();
    policy.action(SEAT, SWITCH, ms(10));
    policy.switcher_step(false);
    policy.switcher_cancel(ms(11));

    assert_eq!(policy.stacking(0), before);
    assert_eq!(policy.focus(SEAT), Some(id(3)));
}

#[test]
fn committing_with_alt_tab_raise_windows_activates_the_selected_window_on_the_old_stacking() {
    let mut policy = three_raising();
    policy.action(SEAT, SWITCH, ms(10));
    policy.switcher_step(false);
    policy.switcher_commit(ms(11));

    assert_eq!(policy.stacking(0), [id(2), id(3), id(1)]);
    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn without_alt_tab_raise_windows_the_stacking_does_not_change_while_open() {
    let mut policy = three();
    policy.action(SEAT, SWITCH, ms(10));
    policy.switcher_step(false);

    assert_eq!(policy.stacking(0), [id(1), id(2), id(3)]);
}

#[test]
fn nothing_opens_during_a_move() {
    let mut policy = three();
    assert!(policy.begin_move(SEAT, id(3), pointer()));

    policy.action(SEAT, SWITCH, ms(10));

    assert!(policy.switcher().is_none());
}

#[test]
fn no_move_begins_while_the_switcher_is_open() {
    let mut policy = three();
    policy.action(SEAT, SWITCH, ms(10));

    assert!(!policy.begin_move(SEAT, id(3), pointer()));
    assert!(!policy.begin_resize(SEAT, id(3), ResizeEdges::BOTTOM_RIGHT, pointer()));
    assert_eq!(policy.grab(SEAT), None);
}

#[test]
fn switching_workspace_closes_the_switcher_without_activating() {
    let mut policy = three();
    policy.action(SEAT, SWITCH, ms(10));
    policy.switch_workspace(1);

    assert!(policy.switcher().is_none());
    assert_eq!(policy.active_workspace(), 1);

    policy.switch_workspace(0);
    assert_eq!(policy.focus(SEAT), Some(id(3)));
}

#[test]
fn committing_a_window_on_another_workspace_switches_to_it() {
    let mut policy = three();
    policy.move_to_workspace(id(1), 1);
    let all = Action::Switch {
        list: SwitchList::WindowsAll,
        backward: true,
    };
    policy.action(SEAT, all, ms(10));
    assert_eq!(policy.switcher().unwrap().windows, [id(3), id(2), id(1)]);
    policy.switcher_commit(ms(11));

    assert!(policy.switcher().is_none());
    assert_eq!(policy.active_workspace(), 1);
    assert_eq!(policy.focus(SEAT), Some(id(1)));
}
