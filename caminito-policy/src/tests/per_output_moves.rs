//! Moves, sticky windows, transient families and activation with Per-output workspaces
//! (ADR 0032, 223): each goes by the window's own output, and a family by its root's.

use super::per_output_workspaces::{active, two_outputs};
use super::*;
use crate::{ActivationSource, Direction, MenuEntry, MenuOp, MenuSelection};

/// `two_outputs`, and a dialog of window 1, window 3, moved to the right output. The
/// dialog has the focus.
fn dialog_on_the_other_output(on: bool) -> Policy<TestWindow> {
    let mut policy = two_outputs(on);
    policy.map_window(
        TestWindow::new(3, 200, 100)
            .with_type(WindowType::Dialog)
            .with_parent(1)
            .with_requested_position(200, 200),
        ms(3),
    );
    policy.request_geometry(id(3), Some(Point::from((2100, 500))), None);
    assert_eq!(policy.focus(SEAT), Some(id(3)));
    policy
}

/// Whether the menu open on a window has `op` among its entries, and not greyed.
fn offers(policy: &Policy<TestWindow>, op: MenuOp) -> bool {
    let menu = policy.window_menu().expect("the window menu is open");
    menu.entries
        .iter()
        .any(|entry| matches!(entry, MenuEntry::Item(item) if item.op == op && item.sensitive))
}

/// Chooses `op` in the open menu.
fn choose(policy: &mut Policy<TestWindow>, op: MenuOp) {
    let menu = policy.window_menu().expect("the window menu is open");
    let index = menu
        .entries
        .iter()
        .position(|entry| matches!(entry, MenuEntry::Item(item) if item.op == op))
        .unwrap_or_else(|| panic!("the menu has no {op:?}"));
    policy.window_menu_activate(MenuSelection::Top(index), ms(50));
}

#[test]
fn move_to_workspace_n_moves_a_window_within_its_own_output() {
    let mut policy = two_outputs(true);
    policy.switch_output_workspace(OutputId(1), 2);
    assert_eq!(policy.focus(SEAT), Some(id(2)));

    policy.action(SEAT, Action::MoveToWorkspace(1), ms(3));

    assert_eq!(policy.workspace_of(id(2)), Some(1));
    assert!(!policy.is_showing(id(2)));
    assert_eq!(active(&policy), (2, 0));
    // It's the right output's workspace 1 that has it.
    policy.switch_output_workspace(OutputId(1), 1);
    assert!(!policy.is_showing(id(2)));
    policy.switch_output_workspace(OutputId(2), 1);
    assert!(policy.is_showing(id(2)));
}

#[test]
fn a_beside_move_on_the_second_output_switches_only_the_second() {
    let mut policy = two_outputs(true);
    policy.switch_output_workspace(OutputId(1), 2);
    assert_eq!(policy.focus(SEAT), Some(id(2)));

    policy.action(SEAT, Action::MoveToWorkspaceRight, ms(3));

    // Beside the right output's workspace 0, and not the left one's 2.
    assert_eq!(policy.workspace_of(id(2)), Some(1));
    assert_eq!(active(&policy), (2, 1));
    assert!(policy.is_showing(id(2)));
    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert_eq!(policy.active_output(SEAT), Some(OutputId(2)));
}

#[test]
fn a_beside_move_of_a_dialog_switches_its_roots_output() {
    let mut policy = dialog_on_the_other_output(true);
    policy.switch_output_workspace(OutputId(2), 2);
    assert_eq!(policy.focus(SEAT), Some(id(3)));

    policy.action(SEAT, Action::MoveToWorkspaceRight, ms(4));

    // Beside the left output's workspace 0, where the family is.
    assert_eq!(policy.workspace_of(id(1)), Some(1));
    assert_eq!(policy.workspace_of(id(3)), Some(1));
    assert_eq!(active(&policy), (1, 2));
    assert!(policy.is_showing(id(1)));
    assert!(policy.is_showing(id(3)));
    assert_eq!(policy.focus(SEAT), Some(id(3)));
}

#[test]
fn with_the_key_off_a_beside_move_switches_every_output() {
    let mut policy = two_outputs(false);

    policy.action(SEAT, Action::MoveToWorkspaceRight, ms(3));

    assert_eq!(policy.workspace_of(id(2)), Some(1));
    assert_eq!(active(&policy), (1, 1));
    assert!(!policy.is_showing(id(1)));
    assert_eq!(policy.focus(SEAT), Some(id(2)));
}

#[test]
fn the_window_menus_moves_are_from_its_windows_outputs_active_workspace() {
    let mut policy = two_outputs(true);
    // The right output shows the last workspace, with window 2 on it.
    policy.action(SEAT, Action::MoveToWorkspace(3), ms(3));
    policy.switch_output_workspace(OutputId(2), 3);

    assert!(policy.open_window_menu(SEAT, id(2)));
    assert!(offers(&policy, MenuOp::MoveLeft));
    assert!(!offers(&policy, MenuOp::MoveRight));

    choose(&mut policy, MenuOp::MoveLeft);
    assert_eq!(policy.workspace_of(id(2)), Some(2));
    assert_eq!(active(&policy), (0, 3));

    // Window 1's are from the left output's workspace 0.
    assert!(policy.open_window_menu(SEAT, id(1)));
    assert!(offers(&policy, MenuOp::MoveRight));
    assert!(!offers(&policy, MenuOp::MoveLeft));
}

#[test]
fn the_window_menus_moves_of_a_dialog_are_from_its_roots_outputs_active_workspace() {
    let mut policy = dialog_on_the_other_output(true);
    policy.switch_output_workspace(OutputId(2), 3);

    assert!(policy.open_window_menu(SEAT, id(3)));

    assert!(offers(&policy, MenuOp::MoveRight));
    assert!(!offers(&policy, MenuOp::MoveLeft));
}

#[test]
fn move_to_monitor_puts_the_window_on_the_target_outputs_active_workspace() {
    let mut policy = two_outputs(true);
    policy.switch_output_workspace(OutputId(2), 1);
    policy.activate(SEAT, id(1), ms(3));

    policy.action(SEAT, Action::MoveToMonitor(Direction::Right), ms(4));

    assert_eq!(policy.output_showing(id(1)), Some(OutputId(2)));
    assert_eq!(policy.workspace_of(id(1)), Some(1));
    assert!(policy.is_showing(id(1)));
    assert_eq!(active(&policy), (0, 1));
}

#[test]
fn with_the_key_off_move_to_monitor_keeps_the_windows_workspace() {
    let mut policy = two_outputs(false);
    policy.activate(SEAT, id(1), ms(3));

    policy.action(SEAT, Action::MoveToMonitor(Direction::Right), ms(4));

    assert_eq!(policy.workspace_of(id(1)), Some(0));
    assert!(policy.is_showing(id(1)));
}

#[test]
fn a_sticky_window_is_on_every_workspace_of_its_main_output() {
    let mut policy = two_outputs(true);
    policy.set_sticky(id(1), true);

    for workspace in [1, 2, 0] {
        policy.switch_output_workspace(OutputId(1), workspace);
        assert!(policy.is_showing(id(1)), "workspace {workspace}");
    }

    // Dragged across, it's sticky on the other output.
    policy.request_geometry(id(1), Some(Point::from((2000, 100))), None);
    for workspace in [1, 2, 0] {
        policy.switch_output_workspace(OutputId(2), workspace);
        assert!(policy.is_sticky(id(1)));
        assert!(policy.is_showing(id(1)), "workspace {workspace}");
    }
}

#[test]
fn unsticking_puts_a_window_on_its_outputs_active_workspace() {
    let mut policy = two_outputs(true);
    policy.set_sticky(id(2), true);
    policy.switch_output_workspace(OutputId(1), 1);
    policy.switch_output_workspace(OutputId(2), 2);

    policy.set_sticky(id(2), false);

    assert_eq!(policy.workspace_of(id(2)), Some(2));
    assert!(policy.is_showing(id(2)));
    policy.switch_output_workspace(OutputId(2), 0);
    assert!(!policy.is_showing(id(2)));
}

#[test]
fn unsticking_a_dialog_puts_its_family_on_its_roots_outputs_active_workspace() {
    let mut policy = dialog_on_the_other_output(true);
    policy.set_sticky(id(1), true);
    assert!(policy.is_sticky(id(3)));
    policy.switch_output_workspace(OutputId(1), 1);
    policy.switch_output_workspace(OutputId(2), 2);
    assert!(policy.is_showing(id(3)));

    policy.set_sticky(id(3), false);

    assert_eq!(policy.workspace_of(id(1)), Some(1));
    assert_eq!(policy.workspace_of(id(3)), Some(1));
    assert!(policy.is_showing(id(3)));
}

#[test]
fn a_dialog_on_the_second_output_shows_and_hides_with_its_root() {
    let mut policy = dialog_on_the_other_output(true);
    // Dragged across alone, it keeps its family's workspace.
    assert_eq!(policy.workspace_of(id(3)), Some(0));
    assert!(policy.is_showing(id(3)));

    // Its root's output switches away, and the dialog's own hasn't.
    policy.switch_output_workspace(OutputId(1), 1);
    assert_eq!(active(&policy), (1, 0));
    assert!(!policy.is_showing(id(1)));
    assert!(!policy.is_showing(id(3)));
    assert_eq!(window(&policy, 3).last_outputs(), Some(&[][..]));
    // The seat's Active output is the dialog's own, so its focus goes to a window there.
    assert_eq!(policy.focus(SEAT), Some(id(2)));

    // The dialog's own output shows the family's workspace or another: it's the root's
    // that says.
    policy.switch_output_workspace(OutputId(2), 1);
    assert!(!policy.is_showing(id(3)));
    policy.switch_output_workspace(OutputId(1), 0);
    assert_eq!(active(&policy), (0, 1));
    assert!(policy.is_showing(id(1)));
    assert!(policy.is_showing(id(3)));
    assert_eq!(window(&policy, 3).last_outputs(), Some(&[OutputId(2)][..]));
}

#[test]
fn a_dialog_stays_through_its_own_outputs_switch_with_the_focus() {
    let mut policy = dialog_on_the_other_output(true);
    // The Active output is the focused window's own Main output.
    assert_eq!(policy.active_output(SEAT), Some(OutputId(2)));

    policy.action(SEAT, Action::SwitchToWorkspace(1), ms(4));

    assert_eq!(active(&policy), (0, 1));
    assert!(policy.is_showing(id(3)));
    assert!(!policy.is_showing(id(2)));
    assert_eq!(policy.focus(SEAT), Some(id(3)));
}

#[test]
fn a_root_dragged_across_takes_its_dialog_left_behind() {
    let mut policy = two_outputs(true);
    policy.map_window(
        TestWindow::new(3, 200, 100)
            .with_type(WindowType::Dialog)
            .with_parent(1)
            .with_requested_position(200, 200),
        ms(3),
    );
    policy.switch_output_workspace(OutputId(2), 1);

    policy.request_geometry(id(1), Some(Point::from((2000, 100))), None);

    // The dialog is on the left output still, and shows by the right one's workspace.
    assert_eq!(policy.workspace_of(id(3)), Some(1));
    assert!(policy.is_showing(id(3)));
    policy.switch_output_workspace(OutputId(1), 2);
    assert!(policy.is_showing(id(3)));
    policy.switch_output_workspace(OutputId(2), 0);
    assert!(!policy.is_showing(id(3)));
}

#[test]
fn activating_a_hidden_window_on_the_second_output_leaves_the_first_alone() {
    let mut policy = two_outputs(true);
    policy.move_to_workspace(id(2), 1);
    policy.activate(SEAT, id(1), ms(3));
    assert!(!policy.is_showing(id(2)));

    policy.activate(SEAT, id(2), ms(4));

    assert_eq!(active(&policy), (0, 1));
    assert!(policy.is_showing(id(1)));
    assert!(policy.is_showing(id(2)));
    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert_eq!(policy.active_output(SEAT), Some(OutputId(2)));
}

#[test]
fn a_clients_activation_request_switches_the_windows_output_alone() {
    let mut policy = two_outputs(true);
    policy.move_to_workspace(id(2), 1);
    policy.activate(SEAT, id(1), ms(3));

    policy.request_activation_from(SEAT, id(2), ActivationSource::Pager, ms(4));

    assert_eq!(active(&policy), (0, 1));
    assert_eq!(policy.focus(SEAT), Some(id(2)));
}

#[test]
fn activating_a_window_that_shows_switches_nothing() {
    let mut policy = two_outputs(true);
    policy.switch_output_workspace(OutputId(1), 1);

    // Window 2 is on workspace 0, which its own output shows and the left one doesn't.
    policy.activate(SEAT, id(2), ms(3));

    assert_eq!(active(&policy), (1, 0));
}

#[test]
fn activating_a_hidden_dialog_switches_its_roots_output() {
    let mut policy = dialog_on_the_other_output(true);
    policy.move_to_workspace(id(1), 1);
    policy.switch_output_workspace(OutputId(2), 2);
    assert!(!policy.is_showing(id(3)));

    policy.activate(SEAT, id(3), ms(4));

    assert_eq!(active(&policy), (1, 2));
    assert!(policy.is_showing(id(1)));
    assert!(policy.is_showing(id(3)));
    assert_eq!(policy.focus(SEAT), Some(id(3)));
}

#[test]
fn with_the_key_off_activation_switches_every_output() {
    let mut policy = two_outputs(false);
    policy.move_to_workspace(id(2), 1);

    policy.activate(SEAT, id(2), ms(3));

    assert_eq!(active(&policy), (1, 1));
    assert!(!policy.is_showing(id(1)));
    assert_eq!(policy.focus(SEAT), Some(id(2)));
}
