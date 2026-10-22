//! The Application overview (CMN-185): the Overview opened for the focused window's
//! Application, which lists only that application's windows on each output's active
//! workspace. The rest of it is the Overview's, and `overview`'s tests.

use super::*;
use crate::{Interaction, OverviewScope};

fn four_workspaces() -> Config {
    Config {
        workspace_count: 4,
        ..Config::default()
    }
}

/// Four workspaces on two outputs, 1 of 1920 × 1080 on the left and 2 of 1280 × 1024 on
/// the right. Process 10 has windows 1 and 3 on the left and 4 on the right, and process
/// 20 has window 2 on the left and 5 on the right. Window 3 is focused.
fn two_processes() -> Policy<TestWindow> {
    let mut policy = Policy::new(four_workspaces());
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy.add_output(output(2, 1920, 0, 1280, 1024));
    for (w, pid, x) in [
        (1, 10, 100),
        (2, 20, 600),
        (4, 10, 2100),
        (5, 20, 2500),
        (3, 10, 300),
    ] {
        let window = TestWindow::new(w, 400, 300)
            .with_pid(pid)
            .with_requested_position(x, 100);
        policy.map_window(window, ms(w));
    }
    policy.activate(SEAT, id(3), ms(6));
    assert_eq!(policy.focus(SEAT), Some(id(3)));
    policy
}

fn toggle(policy: &mut Policy<TestWindow>) {
    policy.action(SEAT, Action::ApplicationOverview, ms(100));
}

fn toggle_overview(policy: &mut Policy<TestWindow>) {
    policy.action(SEAT, Action::Overview, ms(100));
}

/// What the open Overview lists on `output`: its workspace and its windows.
fn listed_on(policy: &Policy<TestWindow>, output: u64) -> (usize, Vec<WindowId>) {
    let open = policy.overview().expect("the Overview is open");
    let on = open
        .on(OutputId(output))
        .expect("the Overview is on the output");
    (on.workspace, on.windows)
}

fn scope(policy: &Policy<TestWindow>) -> Option<OverviewScope> {
    policy.overview().map(|open| open.scope())
}

// Which windows.

#[test]
fn the_shortcut_lists_the_focused_windows_processs_windows_on_each_output() {
    let mut policy = two_processes();

    toggle(&mut policy);

    assert_eq!(scope(&policy), Some(OverviewScope::Application));
    assert_eq!(listed_on(&policy, 1), (0, vec![id(1), id(3)]));
    assert_eq!(listed_on(&policy, 2), (0, vec![id(4)]));
    assert_eq!(policy.overview().unwrap().selected, Some(id(3)));
    assert_eq!(policy.interaction(SEAT), Some(Interaction::Overview));
}

#[test]
fn windows_with_no_pid_belong_together_by_their_app_id() {
    let mut policy = Policy::new(four_workspaces());
    policy.add_output(output(1, 0, 0, 1920, 1080));
    for (w, app_id) in [(1, "a"), (2, "b"), (3, "a")] {
        let window = TestWindow::new(w, 400, 300).with_app_id(app_id);
        policy.map_window(window, ms(w));
    }

    toggle(&mut policy);

    assert_eq!(listed_on(&policy, 1), (0, vec![id(1), id(3)]));
}

#[test]
fn two_processes_with_one_app_id_are_two_applications() {
    let mut policy = Policy::new(four_workspaces());
    policy.add_output(output(1, 0, 0, 1920, 1080));
    for (w, pid) in [(1, 10), (2, 20)] {
        let window = TestWindow::new(w, 400, 300).with_pid(pid).with_app_id("a");
        policy.map_window(window, ms(w));
    }

    toggle(&mut policy);

    assert_eq!(listed_on(&policy, 1), (0, vec![id(2)]));
}

#[test]
fn it_lists_the_applications_minimised_and_sticky_windows_and_not_another_workspaces() {
    let mut policy = two_processes();
    policy.minimize(id(1));
    policy.move_to_workspace(id(4), 1);
    let sticky = TestWindow::new(6, 400, 300).with_pid(10);
    policy.map_window(sticky, ms(6));
    policy.set_sticky(id(6), true);
    policy.activate(SEAT, id(3), ms(7));

    toggle(&mut policy);

    assert_eq!(listed_on(&policy, 1), (0, vec![id(1), id(6), id(3)]));
    assert_eq!(listed_on(&policy, 2), (0, vec![]));
}

#[test]
fn an_output_with_none_of_its_windows_shows_an_empty_layout() {
    let mut policy = two_processes();
    policy.unmap_window(id(4));
    policy.activate(SEAT, id(3), ms(11));

    toggle(&mut policy);

    assert_eq!(listed_on(&policy, 2), (0, vec![]));
}

// What opens it.

#[test]
fn with_no_window_focused_nothing_opens() {
    let mut policy = two_processes();
    for w in 1..=5 {
        policy.minimize(id(w));
    }
    assert_eq!(policy.focus(SEAT), None);

    toggle(&mut policy);
    assert!(policy.overview().is_none());

    policy.overview_on(SEAT, Some(OutputId(2)), OverviewScope::Application);
    assert!(policy.overview().is_none());
}

#[test]
fn a_hot_corner_opens_it_on_its_output() {
    let mut policy = two_processes();

    policy.overview_on(SEAT, Some(OutputId(1)), OverviewScope::Application);

    let open = policy.overview().unwrap();
    assert_eq!(open.scope(), OverviewScope::Application);
    assert_eq!(open.output, OutputId(1));
    assert_eq!(listed_on(&policy, 1), (0, vec![id(1), id(3)]));
    assert_eq!(policy.active_output(SEAT), Some(OutputId(1)));
}

#[test]
fn its_own_shortcut_and_hot_corner_close_it() {
    let mut policy = two_processes();
    toggle(&mut policy);
    toggle(&mut policy);
    assert!(policy.overview().is_none());

    toggle(&mut policy);
    policy.overview_on(SEAT, Some(OutputId(2)), OverviewScope::Application);
    assert!(policy.overview().is_none());
}

// The application is fixed at opening.

#[test]
fn focusing_another_applications_window_doesnt_change_the_list() {
    let mut policy = two_processes();
    toggle(&mut policy);

    policy.activate(SEAT, id(2), ms(200));

    assert_eq!(listed_on(&policy, 1), (0, vec![id(1), id(3)]));
}

#[test]
fn a_switch_of_workspace_lists_the_applications_windows_of_the_new_one() {
    let mut policy = two_processes();
    let other = TestWindow::new(6, 400, 300).with_pid(20);
    policy.map_window(other, ms(6));
    let own = TestWindow::new(7, 400, 300).with_pid(10);
    policy.map_window(own, ms(7));
    policy.move_to_workspace(id(6), 1);
    policy.move_to_workspace(id(7), 1);
    policy.activate(SEAT, id(3), ms(8));
    toggle(&mut policy);

    policy.overview_switch_on(OutputId(1), 1);

    assert_eq!(listed_on(&policy, 1), (1, vec![id(7)]));
    // The selection's output's focused window then is of the application.
    assert_eq!(policy.overview().unwrap().selected, Some(id(7)));
}

#[test]
fn a_switch_to_a_workspace_where_another_application_is_focused_selects_none() {
    let mut policy = two_processes();
    let other = TestWindow::new(6, 400, 300).with_pid(20);
    policy.map_window(other, ms(6));
    policy.move_to_workspace(id(6), 1);
    policy.activate(SEAT, id(3), ms(7));
    toggle(&mut policy);

    policy.overview_switch_on(OutputId(1), 1);

    assert_eq!(listed_on(&policy, 1), (1, vec![]));
    assert_eq!(policy.overview().unwrap().selected, None);
    assert_eq!(scope(&policy), Some(OverviewScope::Application));
}

#[test]
fn the_focused_window_going_keeps_its_application() {
    let mut policy = two_processes();
    toggle(&mut policy);

    policy.unmap_window(id(3));
    policy.unmap_window(id(1));

    assert_eq!(listed_on(&policy, 1), (0, vec![]));
    assert_eq!(listed_on(&policy, 2), (0, vec![id(4)]));
    let joins = TestWindow::new(6, 400, 300).with_pid(10);
    policy.map_window(joins, ms(202));
    let other = TestWindow::new(7, 400, 300).with_pid(20);
    policy.map_window(other, ms(203));
    let (_, left) = listed_on(&policy, 1);
    let (_, right) = listed_on(&policy, 2);
    assert!(left.contains(&id(6)) || right.contains(&id(6)));
    assert!(!left.contains(&id(7)) && !right.contains(&id(7)));
}

#[test]
fn when_none_of_its_windows_is_left_it_stays_open_empty() {
    let mut policy = two_processes();
    toggle(&mut policy);

    for w in [1, 3, 4] {
        policy.unmap_window(id(w));
    }

    assert_eq!(listed_on(&policy, 1), (0, vec![]));
    assert_eq!(listed_on(&policy, 2), (0, vec![]));
    assert_eq!(scope(&policy), Some(OverviewScope::Application));
}

// The scopes switch in place.

#[test]
fn the_overviews_shortcut_changes_it_to_the_overview_in_place() {
    let mut policy = two_processes();
    toggle(&mut policy);

    toggle_overview(&mut policy);

    assert_eq!(scope(&policy), Some(OverviewScope::Workspace));
    assert_eq!(listed_on(&policy, 1), (0, vec![id(1), id(2), id(3)]));
    assert_eq!(listed_on(&policy, 2), (0, vec![id(4), id(5)]));
    assert_eq!(policy.overview().unwrap().selected, Some(id(3)));
}

#[test]
fn its_shortcut_changes_the_overview_to_it_in_place() {
    let mut policy = two_processes();
    toggle_overview(&mut policy);

    toggle(&mut policy);

    assert_eq!(scope(&policy), Some(OverviewScope::Application));
    assert_eq!(listed_on(&policy, 1), (0, vec![id(1), id(3)]));
    assert_eq!(listed_on(&policy, 2), (0, vec![id(4)]));
    assert_eq!(policy.overview().unwrap().selected, Some(id(3)));
}

#[test]
fn a_change_of_scope_keeps_the_selection_while_its_window_is_listed() {
    let mut policy = two_processes();
    toggle_overview(&mut policy);
    policy.set_overview_selection(Some(id(1)));

    toggle(&mut policy);
    assert_eq!(policy.overview().unwrap().selected, Some(id(1)));

    toggle_overview(&mut policy);
    policy.set_overview_selection(Some(id(2)));
    toggle(&mut policy);
    assert_eq!(policy.overview().unwrap().selected, None);
}

#[test]
fn a_change_of_scope_keeps_the_selections_output() {
    let mut policy = two_processes();
    toggle_overview(&mut policy);
    policy.overview_switch_on(OutputId(2), 0);
    assert_eq!(policy.overview().unwrap().output, OutputId(2));

    toggle(&mut policy);

    assert_eq!(policy.overview().unwrap().output, OutputId(2));
    assert_eq!(policy.active_output(SEAT), Some(OutputId(2)));
}

#[test]
fn a_hot_corner_of_the_other_scope_changes_it_in_place() {
    let mut policy = two_processes();
    toggle_overview(&mut policy);

    policy.overview_on(SEAT, Some(OutputId(1)), OverviewScope::Application);
    assert_eq!(scope(&policy), Some(OverviewScope::Application));

    policy.overview_on(SEAT, Some(OutputId(1)), OverviewScope::Workspace);
    assert_eq!(scope(&policy), Some(OverviewScope::Workspace));
}

#[test]
fn to_the_application_overview_it_takes_the_focused_windows_application() {
    let mut policy = two_processes();
    toggle(&mut policy);
    toggle_overview(&mut policy);
    // The focus moves to process 20's window after the Application overview opened for 10.
    policy.activate(SEAT, id(2), ms(200));

    toggle(&mut policy);

    assert_eq!(listed_on(&policy, 1), (0, vec![id(2)]));
    assert_eq!(listed_on(&policy, 2), (0, vec![id(5)]));
}

#[test]
fn with_no_window_focused_the_overview_stays_the_overview() {
    let mut policy = two_processes();
    for w in 1..=5 {
        policy.minimize(id(w));
    }
    toggle_overview(&mut policy);

    toggle(&mut policy);

    assert_eq!(scope(&policy), Some(OverviewScope::Workspace));
    assert_eq!(listed_on(&policy, 1), (0, vec![id(1), id(2), id(3)]));
}

#[test]
fn a_modal_dialog_of_another_process_stands_in_for_the_applications_window() {
    let mut policy = two_processes();
    let dialog = TestWindow::new(6, 200, 100)
        .with_pid(30)
        .with_parent(1)
        .with_type(WindowType::ModalDialog);
    policy.map_window(dialog, ms(7));
    policy.activate(SEAT, id(3), ms(8));

    toggle(&mut policy);

    assert_eq!(listed_on(&policy, 1), (0, vec![id(6), id(3)]));
}
