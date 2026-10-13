//! The Window switcher's lists and Show Desktop with Per-output workspaces (ADR 0032, 224):
//! the lists have the windows showing on every output, and each output's workspace shows its
//! desktop for itself.

use super::per_output_workspaces::two_outputs;
use super::*;
use crate::SwitchList;

/// `two_outputs`, with the left output on workspace 0 and the right one on workspace 1.
/// Window 1 shows on the left and window 4 on the right, which has the focus. Window 3 is
/// on the left output's workspace 1 and window 2 on the right one's workspace 0, and
/// neither shows. From the most recently used: 4, 1, 2, 3.
fn two_workspaces_showing(on: bool) -> Policy<TestWindow> {
    let mut policy = two_outputs(on);
    policy.map_window(
        TestWindow::new(3, 400, 300).with_requested_position(100, 500),
        ms(3),
    );
    policy.move_to_workspace(id(3), 1);
    policy.map_window(
        TestWindow::new(4, 400, 300).with_requested_position(2100, 500),
        ms(4),
    );
    policy.move_to_workspace(id(4), 1);
    policy.switch_output_workspace(OutputId(2), 1);
    policy.click(SEAT, id(1), ms(5));
    policy.click(SEAT, id(4), ms(6));
    policy
}

fn showing(policy: &Policy<TestWindow>) -> Vec<u64> {
    (1..=4).filter(|w| policy.is_showing(id(*w))).collect()
}

#[test]
fn the_switcher_lists_the_windows_showing_on_every_output() {
    let policy = two_workspaces_showing(true);
    assert_eq!(showing(&policy), [1, 4]);

    assert_eq!(
        policy.switch_list(SEAT, SwitchList::Windows),
        [id(4), id(1)]
    );
}

#[test]
fn the_all_list_has_every_workspace_of_every_output() {
    let policy = two_workspaces_showing(true);

    assert_eq!(
        policy.switch_list(SEAT, SwitchList::WindowsAll),
        [id(4), id(1), id(2), id(3)]
    );
}

#[test]
fn the_group_and_panel_lists_have_what_shows_on_every_output() {
    let mut policy = two_workspaces_showing(true);
    for (w, x, workspace) in [(5, 0, 0), (6, 0, 1), (7, 1920, 0), (8, 1920, 1)] {
        policy.map_window(
            TestWindow::new(w, 400, 30)
                .with_type(WindowType::Dock)
                .with_requested_position(x, 0)
                .with_requested_workspace(workspace),
            ms(10 + w),
        );
    }

    // No window has a pid or an app id, so each is an application of its own.
    assert_eq!(policy.switch_list(SEAT, SwitchList::Group), [id(4)]);
    let mut panels = policy.switch_list(SEAT, SwitchList::Panels);
    panels.sort();
    assert_eq!(panels, [id(5), id(8)]);
}

#[test]
fn the_switcher_reaches_a_window_on_the_other_output() {
    let mut policy = two_workspaces_showing(true);
    let switch = Action::Switch {
        list: SwitchList::Windows,
        backward: false,
    };

    policy.action(SEAT, switch, ms(7));
    policy.switcher_commit(ms(8));

    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert_eq!(policy.active_output(SEAT), Some(OutputId(1)));
    assert_eq!(showing(&policy), [1, 4]);
}

#[test]
fn show_desktop_hides_the_windows_of_every_outputs_active_workspace() {
    let mut policy = two_workspaces_showing(true);

    policy.action(SEAT, Action::ShowDesktop, ms(7));

    assert!(policy.is_showing_desktop());
    assert_eq!(showing(&policy), [0u64; 0]);
    assert_eq!(policy.focus(SEAT), None);
    for w in 1..=4 {
        assert!(!policy.is_minimized(id(w)), "window {w} isn't minimised");
    }
}

#[test]
fn an_output_switched_away_and_back_shows_its_desktop_again() {
    let mut policy = two_workspaces_showing(true);
    policy.set_showing_desktop(true);

    policy.switch_output_workspace(OutputId(2), 0);

    assert!(!policy.is_showing_desktop());
    assert_eq!(showing(&policy), [2]);

    policy.switch_output_workspace(OutputId(2), 1);

    assert!(policy.is_showing_desktop());
    assert_eq!(showing(&policy), [0u64; 0]);
}

#[test]
fn another_outputs_workspace_of_the_same_number_keeps_its_windows() {
    let mut policy = two_workspaces_showing(true);
    policy.set_showing_desktop(true);

    // The right output's workspace 1 shows its desktop. The left one's never did.
    policy.switch_output_workspace(OutputId(1), 1);

    assert_eq!(showing(&policy), [3]);
    assert!(!policy.is_showing_desktop());
}

#[test]
fn a_window_activated_on_one_output_ends_show_desktop_there_alone() {
    // Both outputs show workspace 0.
    let mut policy = two_outputs(true);
    policy.set_showing_desktop(true);

    policy.activate(SEAT, id(1), ms(7));

    assert!(!policy.is_showing_desktop());
    assert!(policy.is_showing(id(1)));
    assert_eq!(policy.focus(SEAT), Some(id(1)));
    // The other output's workspace 0 is another workspace: its window is hidden still, and
    // isn't one of those the activation minimises.
    assert!(!policy.is_showing(id(2)));
    assert!(!policy.is_minimized(id(2)));

    // The next toggle shows the desktop on both again, and the one after ends it.
    policy.action(SEAT, Action::ShowDesktop, ms(8));
    assert!(policy.is_showing_desktop());
    policy.action(SEAT, Action::ShowDesktop, ms(9));
    assert!(policy.is_showing(id(1)) && policy.is_showing(id(2)));
}

#[test]
fn a_window_mapped_on_one_output_ends_show_desktop_there_alone() {
    let mut policy = two_outputs(true);
    policy.set_showing_desktop(true);

    policy.map_window(
        TestWindow::new(3, 400, 300).with_requested_position(2100, 500),
        ms(7),
    );

    assert!(policy.is_showing(id(3)));
    assert!(policy.is_minimized(id(2)));
    assert!(!policy.is_showing(id(1)));
    assert!(!policy.is_minimized(id(1)));
}

#[test]
fn a_sticky_window_is_hidden_on_the_workspace_its_output_shows() {
    let mut policy = two_workspaces_showing(true);
    policy.set_sticky(id(2), true);
    assert_eq!(showing(&policy), [1, 2, 4]);

    policy.set_showing_desktop(true);

    assert_eq!(showing(&policy), [0u64; 0]);

    // The right output's workspace 2 doesn't show its desktop, and the left one's
    // workspace 1 has nothing to do with the sticky window.
    policy.switch_output_workspace(OutputId(2), 2);
    assert_eq!(showing(&policy), [2]);
    policy.switch_output_workspace(OutputId(2), 1);
    policy.switch_output_workspace(OutputId(1), 1);
    assert_eq!(showing(&policy), [3]);
}

#[test]
fn the_end_of_show_desktop_keeps_a_lost_focus_on_the_active_output() {
    let mut policy = two_outputs(true);
    assert_eq!(policy.focus(SEAT), Some(id(2)));
    policy.set_showing_desktop(true);
    policy.unmap_window(id(2));

    policy.set_showing_desktop(false);

    // The window that had the focus is gone, and the right output has no other.
    assert!(policy.is_showing(id(1)));
    assert_eq!(policy.focus(SEAT), None);
    assert_eq!(policy.active_output(SEAT), Some(OutputId(2)));
}

#[test]
fn turning_the_key_off_keeps_the_desktop_the_active_output_shows() {
    let mut policy = two_workspaces_showing(true);
    policy.set_showing_desktop(true);
    // The left output goes to a workspace that shows its windows.
    policy.switch_output_workspace(OutputId(1), 1);
    assert_eq!(policy.active_output(SEAT), Some(OutputId(2)));

    policy.settings_changed(Config::default());

    // Every output shows the right one's workspace 1, with its desktop.
    assert!(policy.is_showing_desktop());
    assert_eq!(showing(&policy), [0u64; 0]);
}

#[test]
fn turning_the_key_on_keeps_the_desktop_on_every_output() {
    let mut policy = two_outputs(false);
    policy.set_showing_desktop(true);

    policy.settings_changed(Config {
        workspaces_per_output: true,
        ..Config::default()
    });

    assert!(policy.is_showing_desktop());
    assert_eq!(showing(&policy), [0u64; 0]);
}

#[test]
fn with_the_key_off_the_lists_and_show_desktop_are_one_for_every_output() {
    let mut policy = two_outputs(false);
    policy.map_window(
        TestWindow::new(3, 400, 300).with_requested_position(100, 500),
        ms(3),
    );
    policy.move_to_workspace(id(3), 1);

    assert_eq!(
        policy.switch_list(SEAT, SwitchList::Windows),
        [id(2), id(1)]
    );
    assert_eq!(
        policy.switch_list(SEAT, SwitchList::WindowsAll),
        [id(2), id(3), id(1)]
    );

    policy.set_showing_desktop(true);
    assert!(policy.is_showing_desktop());
    assert_eq!(showing(&policy), [0u64; 0]);

    // A window activated on one output ends it for the one workspace, and the other
    // output's window is minimised with the rest, as Marco does.
    policy.activate(SEAT, id(1), ms(7));
    assert!(!policy.is_showing_desktop());
    assert!(policy.is_showing(id(1)));
    assert!(policy.is_minimized(id(2)));
}

#[test]
fn with_the_key_off_the_end_of_show_desktop_focuses_a_window_on_any_output() {
    let mut policy = two_outputs(false);
    policy.set_showing_desktop(true);
    policy.unmap_window(id(2));

    policy.set_showing_desktop(false);

    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

/// `two_outputs`, and window 3, a desktop window on the left output.
fn with_a_desktop_window() -> Policy<TestWindow> {
    let mut policy = two_outputs(true);
    policy.map_window(
        TestWindow::new(3, 1920, 1080)
            .with_type(WindowType::Desktop)
            .with_requested_position(0, 0),
        ms(3),
    );
    policy
}

#[test]
fn show_desktop_focuses_the_desktop_window_of_the_active_output() {
    let mut policy = with_a_desktop_window();
    policy.click(SEAT, id(1), ms(4));

    policy.set_showing_desktop(true);

    assert_eq!(policy.focus(SEAT), Some(id(3)));
}

#[test]
fn show_desktop_leaves_the_focus_off_another_outputs_desktop_window() {
    let mut policy = with_a_desktop_window();
    policy.click(SEAT, id(2), ms(4));

    policy.set_showing_desktop(true);

    assert_eq!(policy.focus(SEAT), None);
    assert_eq!(policy.active_output(SEAT), Some(OutputId(2)));
}

#[test]
fn an_output_that_goes_takes_the_desktops_it_showed_with_it() {
    let mut policy = two_workspaces_showing(true);
    policy.set_showing_desktop(true);

    policy.remove_output(OutputId(2));

    // The left output's workspace 0 shows its desktop still, and its workspace 1 has the
    // windows of both outputs' and never showed one.
    assert!(policy.is_showing_desktop());
    assert_eq!(showing(&policy), [0u64; 0]);
    policy.switch_output_workspace(OutputId(1), 1);
    assert_eq!(showing(&policy), [3, 4]);
}
