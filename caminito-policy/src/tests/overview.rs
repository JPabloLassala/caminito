//! The Overview (241, ADR 0033): the windows it lists, what opens and closes it, and what
//! a choice does. That none opens over another Interaction, and none over it, is
//! `interaction`'s.

use super::*;
use crate::Interaction;

fn four_workspaces() -> Config {
    Config {
        workspace_count: 4,
        ..Config::default()
    }
}

/// Four workspaces on one output, and three windows on the first, 3 focused and on top.
fn three() -> Policy<TestWindow> {
    let mut policy = Policy::new(four_workspaces());
    policy.add_output(output(1, 0, 0, 1920, 1080));
    for w in 1..=3 {
        policy.map_window(TestWindow::new(w, 400, 300), ms(w));
    }
    policy
}

fn toggle(policy: &mut Policy<TestWindow>) {
    policy.action(SEAT, Action::Overview, ms(100));
}

/// The windows the open Overview lists.
fn listed(policy: &Policy<TestWindow>) -> Vec<WindowId> {
    policy
        .overview()
        .expect("the Overview is open")
        .windows
        .clone()
}

// Opening and closing.

#[test]
fn the_shortcut_opens_the_overview_on_the_seats_active_output() {
    let mut policy = three();
    policy.add_output(output(2, 1920, 0, 1920, 1080));
    assert!(policy.overview().is_none());

    toggle(&mut policy);

    let overview = policy.overview().unwrap();
    assert_eq!(Some(overview.output), policy.active_output(SEAT));
    assert_eq!(policy.interaction(SEAT), Some(Interaction::Overview));
}

#[test]
fn the_shortcut_again_closes_it_and_changes_nothing() {
    let mut policy = three();
    policy.minimize(id(1));
    let (focus, stacking) = (policy.focus(SEAT), policy.active_stacking());
    toggle(&mut policy);

    toggle(&mut policy);

    assert!(policy.overview().is_none());
    assert_eq!(policy.interaction(SEAT), None);
    assert_eq!(policy.focus(SEAT), focus);
    assert_eq!(policy.active_stacking(), stacking);
    assert!(policy.is_minimized(id(1)));
}

#[test]
fn esc_or_a_click_on_the_empty_area_closes_it_and_changes_nothing() {
    let mut policy = three();
    policy.minimize(id(1));
    let (focus, stacking) = (policy.focus(SEAT), policy.active_stacking());
    toggle(&mut policy);

    // The compositor asks for the same close for both.
    policy.close_overview();

    assert!(policy.overview().is_none());
    assert_eq!(policy.focus(SEAT), focus);
    assert_eq!(policy.active_stacking(), stacking);
    assert!(policy.is_minimized(id(1)));
}

#[test]
fn it_opens_on_a_workspace_with_no_windows() {
    let mut policy = Policy::new(four_workspaces());
    policy.add_output(output(1, 0, 0, 1920, 1080));

    toggle(&mut policy);

    assert_eq!(listed(&policy), []);
}

#[test]
fn it_opens_with_no_output_nowhere() {
    let mut policy: Policy<TestWindow> = Policy::new(four_workspaces());

    toggle(&mut policy);

    assert!(policy.overview().is_none());
}

#[test]
fn a_focused_fullscreen_window_is_listed_like_any_other() {
    let mut policy = three();
    policy.set_fullscreen(id(3), true, None);

    toggle(&mut policy);

    assert_eq!(listed(&policy), [id(1), id(2), id(3)]);
}

#[test]
fn its_output_going_closes_it() {
    let mut policy = three();
    toggle(&mut policy);

    policy.remove_output(OutputId(1));

    assert!(policy.overview().is_none());
}

// The windows listed.

#[test]
fn it_lists_the_active_workspaces_windows_bottom_to_top() {
    let mut policy = three();
    policy.raise(id(1));

    toggle(&mut policy);

    assert_eq!(listed(&policy), [id(2), id(3), id(1)]);
}

#[test]
fn a_minimised_window_is_listed() {
    let mut policy = three();
    policy.minimize(id(2));

    toggle(&mut policy);

    assert!(listed(&policy).contains(&id(2)));
}

#[test]
fn a_window_that_skips_the_taskbar_isnt_listed() {
    let mut policy = three();
    policy.map_window(TestWindow::new(4, 200, 100).skipping_taskbar(), ms(4));

    toggle(&mut policy);

    assert_eq!(listed(&policy), [id(1), id(2), id(3)]);
}

#[test]
fn a_dock_and_the_desktop_arent_listed() {
    let mut policy = three();
    let dock = TestWindow::new(4, 1920, 24).with_type(WindowType::Dock);
    policy.map_window(dock, ms(4));
    let desktop = TestWindow::new(5, 1920, 1080).with_type(WindowType::Desktop);
    policy.map_window(desktop, ms(5));

    toggle(&mut policy);

    assert_eq!(listed(&policy), [id(1), id(2), id(3)]);
}

#[test]
fn another_workspaces_window_isnt_listed() {
    let mut policy = three();
    policy.move_to_workspace(id(2), 1);

    toggle(&mut policy);

    assert_eq!(listed(&policy), [id(1), id(3)]);
}

#[test]
fn a_sticky_window_is_listed_on_every_workspace() {
    let mut policy = three();
    policy.set_sticky(id(2), true);

    for workspace in 0..4 {
        policy.switch_workspace(workspace);
        toggle(&mut policy);

        let expected: &[WindowId] = if workspace == 0 {
            &[id(1), id(2), id(3)]
        } else {
            &[id(2)]
        };
        assert_eq!(listed(&policy), expected, "workspace {workspace}");
        toggle(&mut policy);
    }
}

#[test]
fn it_lists_the_windows_of_its_own_output() {
    let mut policy = Policy::new(Config {
        workspaces_per_output: true,
        ..four_workspaces()
    });
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy.add_output(output(2, 1920, 0, 1920, 1080));
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(
        TestWindow::new(2, 400, 300).with_requested_position(2000, 100),
        ms(2),
    );
    policy.activate(SEAT, id(1), ms(3));

    toggle(&mut policy);

    assert_eq!(policy.overview().unwrap().output, OutputId(1));
    assert_eq!(listed(&policy), [id(1)]);
}

#[test]
fn a_modal_dialog_stands_in_for_its_parent_as_in_the_window_switcher() {
    let mut policy = three();
    let dialog = TestWindow::new(4, 200, 100)
        .with_parent(3)
        .with_type(WindowType::ModalDialog);
    policy.map_window(dialog, ms(4));

    toggle(&mut policy);

    assert_eq!(listed(&policy), [id(1), id(2), id(4)]);
}

#[test]
fn a_listed_window_that_goes_leaves_the_list() {
    let mut policy = three();
    toggle(&mut policy);

    policy.unmap_window(id(2));

    assert_eq!(listed(&policy), [id(1), id(3)]);
}

// A choice.

#[test]
fn a_click_on_a_window_activates_it_and_closes_the_overview() {
    let mut policy = three();
    toggle(&mut policy);

    policy.overview_choose(id(1), ms(200));

    assert!(policy.overview().is_none());
    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert_eq!(policy.active_stacking(), [id(2), id(3), id(1)]);
}

#[test]
fn a_click_on_a_minimised_window_unminimises_it() {
    let mut policy = three();
    policy.minimize(id(2));
    toggle(&mut policy);

    policy.overview_choose(id(2), ms(200));

    assert!(policy.overview().is_none());
    assert!(!policy.is_minimized(id(2)));
    assert_eq!(policy.focus(SEAT), Some(id(2)));
}

#[test]
fn a_choice_of_a_window_that_isnt_listed_does_nothing() {
    let mut policy = three();
    policy.move_to_workspace(id(2), 1);
    toggle(&mut policy);

    policy.overview_choose(id(2), ms(200));

    assert!(policy.overview().is_some());
    assert_eq!(policy.active_workspace(), 0);
}

#[test]
fn a_choice_with_the_overview_closed_does_nothing() {
    let mut policy = three();

    policy.overview_choose(id(1), ms(200));

    assert_eq!(policy.focus(SEAT), Some(id(3)));
}

// The Scene.

#[test]
fn opening_and_closing_change_what_the_scene_reads() {
    let mut policy = three();
    policy.take_scene_changed();

    toggle(&mut policy);
    assert!(policy.take_scene_changed());
    assert!(!policy.take_scene_changed());

    toggle(&mut policy);
    assert!(policy.take_scene_changed());
}
