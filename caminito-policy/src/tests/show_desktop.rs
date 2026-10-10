//! Showing the desktop (94), with Marco's cases: `meta_screen_show_desktop` and
//! `meta_screen_unshow_desktop` (screen.c:2850-2895), `maybe_leave_show_desktop_mode`
//! (window.c:183-208) and what `meta_window_showing_on_its_workspace` hides
//! (window.c:1385-1412).

use super::*;
use crate::ShowDesktopSkipList;

/// Three windows on the first workspace, 3 on top and focused.
fn three_windows() -> Policy<TestWindow> {
    let mut policy = policy();
    for w in 1..=3 {
        policy.map_window(TestWindow::new(w, 400, 300), ms(w));
    }
    policy
}

fn is_hidden(policy: &Policy<TestWindow>, w: u64) -> bool {
    !policy.is_showing(id(w)) && window(policy, w).last_outputs() == Some(&[][..])
}

#[test]
fn showing_the_desktop_hides_the_workspaces_windows_without_minimising_them() {
    let mut policy = three_windows();

    policy.set_showing_desktop(true);

    assert!(policy.is_showing_desktop());
    for w in 1..=3 {
        assert!(is_hidden(&policy, w), "window {w} is hidden");
        assert!(!policy.is_minimized(id(w)), "window {w} isn't minimised");
        assert!(!window(&policy, w).is_activated());
    }
    assert_eq!(policy.focus(SEAT), None);
}

#[test]
fn showing_the_desktop_leaves_the_other_workspaces_windows() {
    let mut policy = three_windows();
    policy.move_to_workspace(id(1), 1);

    policy.set_showing_desktop(true);
    policy.switch_workspace(1);

    assert!(!policy.is_showing_desktop());
    assert!(policy.is_showing(id(1)));
    assert_eq!(policy.focus(SEAT), Some(id(1)));

    // The first workspace still shows its desktop when the user is back on it.
    policy.switch_workspace(0);
    assert!(policy.is_showing_desktop());
    assert!(is_hidden(&policy, 2) && is_hidden(&policy, 3));
    assert_eq!(policy.focus(SEAT), None);
}

#[test]
fn a_second_toggle_brings_the_windows_back_in_their_stacking_order_with_the_focus_they_had() {
    let mut policy = three_windows();
    policy.click(SEAT, id(2), ms(4));
    assert_eq!(policy.stacking(0), [id(1), id(3), id(2)]);

    policy.action(SEAT, Action::ShowDesktop, ms(5));
    assert!(policy.is_showing_desktop());
    policy.action(SEAT, Action::ShowDesktop, ms(6));

    assert!(!policy.is_showing_desktop());
    assert_eq!(policy.stacking(0), [id(1), id(3), id(2)]);
    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert!(window(&policy, 2).is_activated());
    for w in 1..=3 {
        assert!(policy.is_showing(id(w)), "window {w} is back");
        assert!(!policy.is_minimized(id(w)));
        assert_eq!(window(&policy, w).last_outputs(), Some(&[OutputId(1)][..]));
    }
}

#[test]
fn the_focus_comes_back_to_the_window_that_had_it_even_under_another() {
    let mut policy = three_windows();
    policy.raise(id(1));
    assert_eq!(policy.stacking(0), [id(2), id(3), id(1)]);
    assert_eq!(policy.focus(SEAT), Some(id(3)));

    policy.set_showing_desktop(true);
    policy.set_showing_desktop(false);

    assert_eq!(policy.stacking(0), [id(2), id(3), id(1)]);
    assert_eq!(policy.focus(SEAT), Some(id(3)));
}

#[test]
fn a_stale_activation_leaves_the_desktop_showing() {
    let mut policy = three_windows();
    policy.click(SEAT, id(3), ms(10));
    policy.set_showing_desktop(true);

    policy.activate(SEAT, id(1), ms(5));

    assert!(policy.is_showing_desktop());
    assert!(!policy.is_minimized(id(2)) && !policy.is_minimized(id(3)));
    assert_eq!(policy.focus(SEAT), None);
}

#[test]
fn a_window_that_was_minimised_stays_minimised_when_the_others_come_back() {
    let mut policy = three_windows();
    policy.minimize(id(3));

    policy.set_showing_desktop(true);
    policy.set_showing_desktop(false);

    assert!(policy.is_minimized(id(3)));
    assert!(!policy.is_showing(id(3)));
    assert_eq!(policy.focus(SEAT), Some(id(2)));
}

#[test]
fn a_new_window_ends_it_and_the_hidden_windows_are_minimised() {
    let mut policy = three_windows();
    policy.set_showing_desktop(true);

    policy.map_window(TestWindow::new(4, 400, 300), ms(4));

    assert!(!policy.is_showing_desktop());
    assert!(policy.is_showing(id(4)));
    assert_eq!(policy.focus(SEAT), Some(id(4)));
    for w in 1..=3 {
        assert!(policy.is_minimized(id(w)), "window {w} is minimised");
        assert!(is_hidden(&policy, w));
    }
}

#[test]
fn a_new_window_is_placed_as_if_the_hidden_windows_werent_there() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    let first = policy.geometry(id(1)).unwrap().loc;
    policy.set_showing_desktop(true);

    policy.map_window(TestWindow::new(2, 400, 300), ms(2));

    assert_eq!(policy.geometry(id(2)).unwrap().loc, first);
}

#[test]
fn activating_a_hidden_window_ends_it_and_minimises_the_rest() {
    let mut policy = three_windows();
    policy.set_showing_desktop(true);

    policy.activate(SEAT, id(1), ms(4));

    assert!(!policy.is_showing_desktop());
    assert!(policy.is_showing(id(1)));
    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert!(policy.is_minimized(id(2)) && policy.is_minimized(id(3)));
}

#[test]
fn switching_windows_ends_it_as_activating_does() {
    let mut policy = three_windows();
    policy.set_showing_desktop(true);

    policy.action(
        SEAT,
        Action::Switch {
            list: crate::SwitchList::Windows,
            backward: false,
        },
        ms(4),
    );
    policy.switcher_commit(ms(4));

    assert!(!policy.is_showing_desktop());
    assert_eq!(policy.focus(SEAT), Some(id(3)));
    assert!(policy.is_minimized(id(1)) && policy.is_minimized(id(2)));
}

#[test]
fn activating_a_window_on_another_workspace_leaves_this_ones_desktop_showing() {
    let mut policy = three_windows();
    policy.move_to_workspace(id(1), 1);
    policy.set_showing_desktop(true);

    policy.activate(SEAT, id(1), ms(4));
    assert!(!policy.is_showing_desktop());

    policy.switch_workspace(0);
    assert!(policy.is_showing_desktop());
    assert!(!policy.is_minimized(id(2)) && !policy.is_minimized(id(3)));
}

#[test]
fn a_window_mapped_on_another_workspace_leaves_this_ones_desktop_showing() {
    let mut policy = three_windows();
    policy.set_showing_desktop(true);

    policy.map_window(
        TestWindow::new(4, 400, 300).with_requested_workspace(1),
        ms(4),
    );

    assert!(policy.is_showing_desktop());
    assert!(!policy.is_minimized(id(1)));
}

#[test]
fn a_window_mapped_on_a_workspace_that_shows_its_desktop_ends_it_there() {
    let mut policy = three_windows();
    policy.set_showing_desktop(true);
    policy.switch_workspace(1);

    policy.map_window(
        TestWindow::new(4, 400, 300).with_requested_workspace(0),
        ms(4),
    );
    policy.switch_workspace(0);

    assert!(!policy.is_showing_desktop());
    assert!(policy.is_showing(id(4)));
    assert!(policy.is_minimized(id(1)));
}

#[test]
fn the_desktop_and_docks_stay_and_the_desktop_takes_the_focus() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 1920, 1080).with_type(WindowType::Desktop),
        ms(1),
    );
    policy.map_window(
        TestWindow::new(2, 1920, 30).with_type(WindowType::Dock),
        ms(2),
    );
    policy.map_window(TestWindow::new(3, 400, 300), ms(3));
    assert_eq!(policy.focus(SEAT), Some(id(3)));

    policy.set_showing_desktop(true);

    assert!(policy.is_showing(id(1)) && policy.is_showing(id(2)));
    assert!(is_hidden(&policy, 3));
    assert_eq!(policy.focus(SEAT), Some(id(1)));

    policy.set_showing_desktop(false);
    assert_eq!(policy.focus(SEAT), Some(id(3)));
}

#[test]
fn a_dialog_of_the_desktop_shows_and_doesnt_end_it() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 1920, 1080).with_type(WindowType::Desktop),
        ms(1),
    );
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.set_showing_desktop(true);

    policy.map_window(
        TestWindow::new(3, 300, 200)
            .with_type(WindowType::Dialog)
            .with_parent(1),
        ms(3),
    );

    assert!(policy.is_showing_desktop());
    assert!(policy.is_showing(id(3)));
    assert_eq!(policy.focus(SEAT), Some(id(3)));
    assert!(is_hidden(&policy, 2));
    assert!(!policy.is_minimized(id(2)));
}

#[test]
fn a_new_dialog_of_a_hidden_window_comes_back_with_its_parents() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.set_showing_desktop(true);

    policy.map_window(
        TestWindow::new(3, 300, 200)
            .with_type(WindowType::Dialog)
            .with_parent(1),
        ms(3),
    );

    assert!(!policy.is_showing_desktop());
    assert!(policy.is_showing(id(3)) && policy.is_showing(id(1)));
    assert!(policy.is_minimized(id(2)));
}

#[test]
fn only_windows_marco_lets_the_user_minimise_are_minimised_when_it_ends() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(
        TestWindow::new(2, 200, 300).with_type(WindowType::Utility),
        ms(2),
    );
    policy.set_showing_desktop(true);
    assert!(is_hidden(&policy, 2));

    policy.map_window(TestWindow::new(3, 400, 300), ms(3));

    assert!(policy.is_minimized(id(1)));
    assert!(!policy.is_minimized(id(2)));
    assert!(policy.is_showing(id(2)));
}

#[test]
fn a_window_moved_to_a_workspace_that_shows_its_desktop_is_hidden_there() {
    let mut policy = three_windows();
    policy.set_showing_desktop(true);
    policy.switch_workspace(1);
    policy.map_window(TestWindow::new(4, 400, 300), ms(4));

    policy.move_to_workspace(id(4), 0);
    policy.switch_workspace(0);

    assert!(policy.is_showing_desktop());
    assert!(is_hidden(&policy, 4));
}

#[test]
fn a_hidden_window_cant_be_moved_by_a_grab() {
    let mut policy = three_windows();
    assert!(policy.begin_move(SEAT, id(3), (10., 10.).into()));

    policy.set_showing_desktop(true);

    assert_eq!(policy.grab(SEAT), None);
    assert!(!policy.begin_move(SEAT, id(3), (10., 10.).into()));
}

#[test]
fn a_removed_workspace_stops_showing_its_desktop() {
    let mut policy = three_windows();
    policy.switch_workspace(3);
    policy.set_showing_desktop(true);

    policy.settings_changed(Config {
        workspace_count: 2,
        workspace_names: Vec::new(),
        ..Config::default()
    });
    assert!(!policy.is_showing_desktop());

    policy.settings_changed(Config::default());
    policy.switch_workspace(3);
    assert!(!policy.is_showing_desktop());
}

#[test]
fn asking_for_what_it_already_is_changes_nothing() {
    let mut policy = three_windows();

    policy.set_showing_desktop(false);
    assert_eq!(policy.focus(SEAT), Some(id(3)));

    policy.set_showing_desktop(true);
    policy.set_showing_desktop(true);
    assert!(policy.is_showing_desktop());
}

// `show-desktop-skip-list` (164).

/// A policy with `show-desktop-skip-list` set to `list`.
fn skipping(list: &str) -> Policy<TestWindow> {
    let mut policy = Policy::new(Config {
        show_desktop_skip_list: ShowDesktopSkipList::parse(list),
        ..Config::default()
    });
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy
}

#[test]
fn a_listed_window_stays_with_its_place_and_focus_and_the_others_come_back_round_it() {
    let mut policy = skipping("Conky");
    policy.map_window(
        TestWindow::new(1, 400, 300).with_app_id("mate-terminal"),
        ms(1),
    );
    policy.map_window(TestWindow::new(2, 400, 300).with_app_id("Conky"), ms(2));
    let geometry = window(&policy, 2).last_geometry();

    policy.action(SEAT, Action::ShowDesktop, ms(3));

    assert!(is_hidden(&policy, 1));
    assert!(policy.is_showing(id(2)));
    assert_eq!(window(&policy, 2).last_outputs(), Some(&[OutputId(1)][..]));
    assert_eq!(window(&policy, 2).last_geometry(), geometry);
    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert!(window(&policy, 2).is_activated());

    policy.action(SEAT, Action::ShowDesktop, ms(4));

    assert!(policy.is_showing(id(1)) && policy.is_showing(id(2)));
    assert_eq!(policy.stacking(0), [id(1), id(2)]);
    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert_eq!(window(&policy, 2).last_geometry(), geometry);
}

#[test]
fn an_empty_list_hides_every_window() {
    let mut policy = skipping("");
    policy.map_window(TestWindow::new(1, 400, 300).with_app_id("Conky"), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));

    policy.set_showing_desktop(true);

    assert!(is_hidden(&policy, 1) && is_hidden(&policy, 2));
    assert_eq!(policy.focus(SEAT), None);
}

#[test]
fn an_entry_that_matches_no_window_hides_them_all() {
    let mut policy = skipping("xclock");
    policy.map_window(TestWindow::new(1, 400, 300).with_app_id("Conky"), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));

    policy.set_showing_desktop(true);

    assert!(is_hidden(&policy, 1) && is_hidden(&policy, 2));
}

#[test]
fn entries_are_split_on_commas_trimmed_and_matched_in_any_case() {
    let mut policy = skipping(" xclock , CONKY,,");
    policy.map_window(TestWindow::new(1, 400, 300).with_app_id("conky"), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300).with_app_id("XClock"), ms(2));
    policy.map_window(
        TestWindow::new(3, 400, 300).with_app_id("xclock conky"),
        ms(3),
    );
    policy.map_window(TestWindow::new(4, 400, 300), ms(4));

    policy.set_showing_desktop(true);

    assert!(policy.is_showing(id(1)) && policy.is_showing(id(2)));
    assert!(is_hidden(&policy, 3) && is_hidden(&policy, 4));
}

#[test]
fn a_change_of_the_list_waits_for_the_next_show_desktop() {
    let mut policy = skipping("Conky");
    policy.map_window(
        TestWindow::new(1, 400, 300).with_app_id("mate-terminal"),
        ms(1),
    );
    policy.map_window(TestWindow::new(2, 400, 300).with_app_id("Conky"), ms(2));
    policy.set_showing_desktop(true);

    policy.settings_changed(Config {
        show_desktop_skip_list: ShowDesktopSkipList::parse("mate-terminal"),
        ..Config::default()
    });

    assert!(is_hidden(&policy, 1));
    assert!(policy.is_showing(id(2)));

    policy.set_showing_desktop(false);
    policy.set_showing_desktop(true);

    assert!(policy.is_showing(id(1)));
    assert!(is_hidden(&policy, 2));
}

#[test]
fn a_seat_on_a_listed_window_keeps_it_where_the_others_go_to_the_desktop() {
    let mut policy = skipping("Conky");
    policy.map_window(
        TestWindow::new(1, 1920, 1080).with_type(WindowType::Desktop),
        ms(1),
    );
    policy.map_window(TestWindow::new(2, 400, 300).with_app_id("Conky"), ms(2));
    assert_eq!(policy.focus(SEAT), Some(id(2)));

    policy.set_showing_desktop(true);

    assert_eq!(policy.focus(SEAT), Some(id(2)));
}

#[test]
fn a_listed_window_mapped_while_the_desktop_shows_leaves_the_others_hidden() {
    let mut policy = skipping("Conky");
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.set_showing_desktop(true);

    policy.map_window(TestWindow::new(2, 400, 300).with_app_id("Conky"), ms(2));

    assert!(policy.is_showing_desktop());
    assert!(policy.is_showing(id(2)));
    assert!(is_hidden(&policy, 1));
    assert!(!policy.is_minimized(id(1)));
}

#[test]
fn a_window_mapped_while_the_desktop_shows_doesnt_minimise_a_listed_one() {
    let mut policy = skipping("Conky");
    policy.map_window(TestWindow::new(1, 400, 300).with_app_id("Conky"), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.set_showing_desktop(true);

    policy.map_window(TestWindow::new(3, 400, 300), ms(3));

    assert!(!policy.is_showing_desktop());
    assert!(policy.is_minimized(id(2)));
    assert!(!policy.is_minimized(id(1)));
    assert!(policy.is_showing(id(1)) && policy.is_showing(id(3)));
}

#[test]
fn a_listed_windows_transients_stay_with_it() {
    let mut policy = skipping("Conky");
    policy.map_window(TestWindow::new(1, 400, 300).with_app_id("Conky"), ms(1));
    policy.map_window(
        TestWindow::new(2, 200, 100)
            .with_type(WindowType::Dialog)
            .with_parent(1),
        ms(2),
    );

    policy.set_showing_desktop(true);

    assert!(policy.is_showing(id(1)) && policy.is_showing(id(2)));
}
