use super::*;
use crate::{LayerSurfaceId, MinimizedPlacement, ResizeEdges, SwitchEntry, SwitchList};

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
    assert!(list.contains(&SwitchEntry::Window(id(1))));
    assert!(!list.contains(&SwitchEntry::Window(id(3))));
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
    assert!(policy.switch_list(SEAT, SwitchList::Group).is_empty());
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
    assert_eq!(switcher.entries, [id(3), id(2), id(1)]);
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
    assert_eq!(switcher.entries, [id(5), id(4)]);
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
    assert_eq!(switcher.entries, [id(3), id(1)]);
    assert_eq!(switcher.entries[switcher.selected], id(1));

    // The last entry closing gives the selection to the first.
    policy.unmap_window(id(1));
    let switcher = policy.switcher().unwrap();
    assert_eq!(switcher.entries, [id(3)]);
    assert_eq!(switcher.selected, 0);
}

#[test]
fn an_unselected_window_that_closes_leaves_the_grid_and_keeps_the_selection_on_its_window() {
    let mut policy = three();
    policy.action(SEAT, SWITCH, ms(10));
    policy.unmap_window(id(3));

    let switcher = policy.switcher().unwrap();
    assert_eq!(switcher.entries, [id(2), id(1)]);
    assert_eq!(switcher.entries[switcher.selected], id(2));

    policy.unmap_window(id(1));
    let switcher = policy.switcher().unwrap();
    assert_eq!(switcher.entries, [id(2)]);
    assert_eq!(switcher.entries[switcher.selected], id(2));
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

    assert_eq!(cancelled.switcher().unwrap().entries, [id(3), id(2), id(1)]);
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
    assert_eq!(policy.switcher().unwrap().entries, [id(3), id(2), id(1)]);
    policy.switcher_commit(ms(11));

    assert!(policy.switcher().is_none());
    assert_eq!(policy.active_workspace(), 1);
    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

/// A `cycle-*` action over `list`.
fn cycle(list: SwitchList, backward: bool) -> Action {
    Action::Cycle { list, backward }
}

const CYCLE: Action = Action::Cycle {
    list: SwitchList::Windows,
    backward: false,
};

#[test]
fn cycle_windows_selects_the_next_window_with_no_popup_and_raises_it_at_once() {
    // `alt-tab-raise-windows` is off: a cycle raises all the same, as Marco's does
    // (`do_choose_window`, core/keybindings.c:3117-3122).
    let mut policy = three();
    policy.action(SEAT, CYCLE, ms(10));

    let switcher = policy.switcher().expect("the cycle is under way");
    assert!(!switcher.popup);
    assert_eq!(switcher.entries, [id(3), id(2), id(1)]);
    assert_eq!(switcher.selected, 1);
    assert_eq!(policy.stacking(0), [id(1), id(3), id(2)]);
    // The focus moves when the modifier is let go.
    assert_eq!(policy.focus(SEAT), Some(id(3)));

    policy.switcher_commit(ms(11));
    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert_eq!(policy.stacking(0), [id(1), id(3), id(2)]);
}

#[test]
fn cycle_windows_backward_selects_the_last_window() {
    let mut policy = three();
    policy.action(SEAT, cycle(SwitchList::Windows, true), ms(10));

    assert_eq!(policy.switcher().unwrap().selected, 2);
    assert_eq!(policy.stacking(0), [id(2), id(3), id(1)]);

    policy.switcher_commit(ms(11));
    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn each_press_of_a_cycle_raises_the_next_window_alone() {
    let mut policy = three();
    policy.action(SEAT, CYCLE, ms(10));
    policy.action(SEAT, CYCLE, ms(11));

    // 2 is back where it was, under 3.
    assert_eq!(policy.switcher().unwrap().selected, 2);
    assert_eq!(policy.stacking(0), [id(2), id(3), id(1)]);

    policy.action(SEAT, cycle(SwitchList::Windows, true), ms(12));
    assert_eq!(policy.switcher().unwrap().selected, 1);
    assert_eq!(policy.stacking(0), [id(1), id(3), id(2)]);
}

#[test]
fn a_cancelled_cycle_leaves_the_focus_and_the_stacking_as_they_were() {
    let mut policy = three();
    policy.action(SEAT, CYCLE, ms(10));
    policy.switcher_cancel(ms(11));

    assert!(policy.switcher().is_none());
    assert_eq!(policy.focus(SEAT), Some(id(3)));
    assert_eq!(policy.stacking(0), [id(1), id(2), id(3)]);
}

#[test]
fn a_cycle_shows_a_minimised_window_only_while_it_is_selected() {
    let mut policy = three();
    policy.minimize(id(3)); // 2 has the focus: 2, 1, then 3 minimised
    assert_eq!(policy.focus(SEAT), Some(id(2)));

    policy.action(SEAT, cycle(SwitchList::Windows, true), ms(10));
    assert_eq!(policy.switcher().unwrap().selected, 2);
    assert!(!policy.is_minimized(id(3)));

    policy.action(SEAT, CYCLE, ms(11));
    assert_eq!(policy.switcher().unwrap().selected, 0);
    assert!(policy.is_minimized(id(3)));

    policy.action(SEAT, cycle(SwitchList::Windows, true), ms(12));
    assert!(!policy.is_minimized(id(3)));
    policy.switcher_cancel(ms(13));
    assert!(policy.is_minimized(id(3)));
    assert_eq!(policy.focus(SEAT), Some(id(2)));
}

#[test]
fn a_cycle_that_commits_to_a_minimised_window_restores_it() {
    let mut policy = three();
    policy.minimize(id(3));
    policy.action(SEAT, cycle(SwitchList::Windows, true), ms(10));
    policy.switcher_commit(ms(11));

    assert!(!policy.is_minimized(id(3)));
    assert_eq!(policy.focus(SEAT), Some(id(3)));
}

#[test]
fn cycle_group_walks_the_focused_applications_windows() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300).with_pid(10), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300).with_pid(20), ms(2));
    policy.map_window(TestWindow::new(3, 400, 300).with_pid(10), ms(3));

    policy.action(SEAT, cycle(SwitchList::Group, false), ms(10));

    let switcher = policy.switcher().unwrap();
    assert_eq!(switcher.list, SwitchList::Group);
    assert_eq!(switcher.entries, [id(3), id(1)]);
    assert_eq!(switcher.selected, 1);

    policy.switcher_commit(ms(11));
    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn cycle_panels_walks_docks_and_the_desktop() {
    let mut policy = three();
    for w in 4..=5 {
        policy.map_window(
            TestWindow::new(w, 1920, 30).with_type(WindowType::Dock),
            ms(w),
        );
    }
    policy.action(SEAT, cycle(SwitchList::Panels, false), ms(10));

    let switcher = policy.switcher().unwrap();
    assert_eq!(switcher.entries, [id(5), id(4)]);
    assert_eq!(switcher.selected, 0);

    policy.action(SEAT, cycle(SwitchList::Panels, true), ms(11));
    policy.switcher_commit(ms(12));
    assert_eq!(policy.focus(SEAT), Some(id(4)));
}

#[test]
fn cycle_panels_with_one_panel_activates_it_at_once() {
    // Marco's grab would hold it selected until the modifier went up
    // (`do_choose_window`, core/keybindings.c:3053-3063): with one entry that's the same.
    let mut policy = three();
    policy.map_window(
        TestWindow::new(4, 1920, 30).with_type(WindowType::Dock),
        ms(4),
    );
    policy.action(SEAT, cycle(SwitchList::Panels, false), ms(10));

    assert!(policy.switcher().is_none());
    assert_eq!(policy.focus(SEAT), Some(id(4)));

    // And again from the panel itself, where it stays.
    policy.action(SEAT, cycle(SwitchList::Panels, false), ms(11));
    assert!(policy.switcher().is_none());
    assert_eq!(policy.focus(SEAT), Some(id(4)));
}

/// `policy` with `alt-tab-minimized-placement` set.
fn placing_minimised(
    mut policy: Policy<TestWindow>,
    placement: MinimizedPlacement,
) -> Policy<TestWindow> {
    policy.settings_changed(Config {
        alt_tab_minimized_placement: placement,
        ..Config::default()
    });
    policy
}

#[test]
fn alt_tab_minimized_placement_places_the_minimised_windows_in_every_list() {
    // 3 is the most recently used, then 2, which has the focus once 3 is minimised.
    let minimised = |placement| {
        let mut policy = placing_minimised(three(), placement);
        policy.minimize(id(3));
        policy
    };

    let end = minimised(MinimizedPlacement::End);
    assert_eq!(
        end.switch_list(SEAT, SwitchList::Windows),
        [id(2), id(1), id(3)]
    );

    let hidden = minimised(MinimizedPlacement::Hidden);
    assert_eq!(
        hidden.switch_list(SEAT, SwitchList::Windows),
        [id(2), id(1)]
    );
    assert_eq!(
        hidden.switch_list(SEAT, SwitchList::WindowsAll),
        [id(2), id(1)]
    );

    // Among the others by use: 2 took the focus after 3, and 1 never had it since.
    let mut mru = minimised(MinimizedPlacement::Mru);
    assert_eq!(
        mru.switch_list(SEAT, SwitchList::Windows),
        [id(2), id(3), id(1)]
    );
    // The cycle's list is the same one.
    mru.action(SEAT, CYCLE, ms(10));
    assert_eq!(mru.switcher().unwrap().entries, [id(2), id(3), id(1)]);
}

/// Two docks, 4 and 5, over `three`, and the Layer surfaces 1 and 2.
fn panels() -> Policy<TestWindow> {
    let mut policy = three();
    for w in 4..=5 {
        policy.map_window(
            TestWindow::new(w, 1920, 30).with_type(WindowType::Dock),
            ms(w),
        );
    }
    policy.set_layers(&[LayerSurfaceId(1), LayerSurfaceId(2)], None);
    policy
}

fn layer(id: u64) -> SwitchEntry {
    SwitchEntry::Layer(LayerSurfaceId(id))
}

fn dock(window: u64) -> SwitchEntry {
    SwitchEntry::Window(id(window))
}

#[test]
fn the_panels_list_has_the_layer_surfaces_after_the_windows() {
    let policy = panels();

    assert_eq!(
        policy.switch_list(SEAT, SwitchList::Panels),
        [dock(5), dock(4), layer(1), layer(2)]
    );
    for list in [
        SwitchList::Windows,
        SwitchList::WindowsAll,
        SwitchList::Group,
    ] {
        let listed = policy.switch_list(SEAT, list);
        assert!(listed.iter().all(|entry| entry.window().is_some()));
    }
}

#[test]
fn committing_to_a_layer_surface_names_it_and_changes_no_focus() {
    let mut policy = panels();
    policy.action(
        SEAT,
        Action::Switch {
            list: SwitchList::Panels,
            backward: true,
        },
        ms(10),
    );
    assert_eq!(policy.switcher().unwrap().selected, 3);
    policy.switcher_commit(ms(11));

    assert_eq!(policy.take_chosen(), Some(layer(2)));
    assert_eq!(policy.take_chosen(), None);
    assert_eq!(policy.focus(SEAT), Some(id(3)));
    assert_eq!(policy.stacking(0).last(), Some(&id(5)));
}

#[test]
fn committing_to_a_window_names_it_so_a_layer_surface_gives_the_keyboard_back() {
    let mut policy = three();
    policy.action(SEAT, SWITCH, ms(10));
    policy.switcher_commit(ms(11));

    assert_eq!(policy.take_chosen(), Some(dock(2)));
}

#[test]
fn a_cancelled_switcher_names_nothing() {
    let mut policy = panels();
    policy.action(SEAT, cycle(SwitchList::Panels, false), ms(10));
    policy.switcher_cancel(ms(11));

    assert_eq!(policy.take_chosen(), None);
}

#[test]
fn the_layer_surface_with_the_keyboard_is_first_and_the_next_is_selected() {
    let mut policy = panels();
    policy.set_layers(
        &[LayerSurfaceId(1), LayerSurfaceId(2)],
        Some(LayerSurfaceId(2)),
    );

    policy.action(SEAT, cycle(SwitchList::Panels, false), ms(10));
    let switcher = policy.switcher().unwrap();
    assert_eq!(switcher.entries, [layer(2), layer(1), dock(5), dock(4)]);
    assert_eq!(switcher.selected, 1);
}

#[test]
fn layer_surfaces_are_listed_by_when_each_last_had_the_keyboard() {
    let mut policy = policy();
    let all = [LayerSurfaceId(1), LayerSurfaceId(2), LayerSurfaceId(3)];
    policy.set_layers(&all, None);
    policy.set_layers(&all, Some(LayerSurfaceId(3)));
    policy.set_layers(&all, Some(LayerSurfaceId(2)));
    // A window took the keyboard back.
    policy.set_layers(&all, None);

    assert_eq!(
        policy.switch_list(SEAT, SwitchList::Panels),
        [layer(2), layer(3), layer(1)]
    );
}

#[test]
fn with_a_layer_surface_at_the_keyboard_switch_windows_starts_on_the_focused_window() {
    // The focused window keeps policy's focus while a panel has the keyboard, and it's the
    // window to come back to.
    let mut policy = panels();
    policy.set_layers(
        &[LayerSurfaceId(1), LayerSurfaceId(2)],
        Some(LayerSurfaceId(1)),
    );
    policy.action(SEAT, SWITCH, ms(10));

    let switcher = policy.switcher().unwrap();
    assert_eq!(switcher.entries[0], id(3));
    assert_eq!(switcher.selected, 0);
}

#[test]
fn one_layer_surface_alone_is_chosen_with_no_switcher() {
    let mut policy = three();
    policy.set_layers(&[LayerSurfaceId(7)], None);
    policy.action(SEAT, cycle(SwitchList::Panels, false), ms(10));

    assert!(policy.switcher().is_none());
    assert_eq!(policy.take_chosen(), Some(layer(7)));
    assert_eq!(policy.focus(SEAT), Some(id(3)));
}

#[test]
fn a_layer_surface_that_goes_away_leaves_the_open_switcher() {
    let mut policy = panels();
    policy.action(
        SEAT,
        Action::Switch {
            list: SwitchList::Panels,
            backward: true,
        },
        ms(10),
    );
    policy.set_layers(&[LayerSurfaceId(1)], None);

    let switcher = policy.switcher().unwrap();
    assert_eq!(switcher.entries, [dock(5), dock(4), layer(1)]);
    // The selected one was the last: the first is next.
    assert_eq!(switcher.selected, 0);
}
