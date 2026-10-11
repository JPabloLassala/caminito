//! The window and workspace actions of Marco's keybindings (149), on the focused window.

use super::*;

/// Two windows at `(100, 100)` and `(300, 300)`, which overlap, with the second on top.
fn two_overlapping(policy: &mut Policy<TestWindow>) {
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(100, 100),
        ms(1),
    );
    policy.map_window(
        TestWindow::new(2, 400, 300).with_requested_position(300, 300),
        ms(2),
    );
}

#[test]
fn raise_or_lower_raises_a_window_that_another_in_its_layer_overlaps() {
    let mut policy = policy();
    two_overlapping(&mut policy);
    policy.activate(SEAT, id(1), ms(3));
    policy.action(SEAT, Action::Lower, ms(4));
    assert_eq!(policy.stacking(0), [id(1), id(2)]);

    policy.action(SEAT, Action::RaiseOrLower, ms(7));

    assert_eq!(policy.stacking(0), [id(2), id(1)]);
    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn raise_or_lower_lowers_the_topmost_window_and_it_keeps_the_focus() {
    let mut policy = policy();
    two_overlapping(&mut policy);
    assert_eq!(policy.focus(SEAT), Some(id(2)));

    policy.action(SEAT, Action::RaiseOrLower, ms(3));

    assert_eq!(policy.stacking(0), [id(2), id(1)]);
    assert_eq!(policy.focus(SEAT), Some(id(2)));
}

#[test]
fn raise_or_lower_lowers_a_window_that_only_another_layer_or_a_hidden_window_overlaps() {
    let mut policy = policy();
    // Below it, a window beside it.
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(1000, 100),
        ms(1),
    );
    policy.map_window(
        TestWindow::new(2, 400, 300).with_requested_position(100, 100),
        ms(2),
    );
    // Above it: a window beside it, a minimised one over it, and a dock over it, which is
    // in the layer above.
    policy.map_window(
        TestWindow::new(3, 400, 300).with_requested_position(1200, 600),
        ms(3),
    );
    policy.map_window(
        TestWindow::new(4, 400, 300).with_requested_position(150, 150),
        ms(4),
    );
    policy.minimize(id(4));
    policy.map_window(
        TestWindow::new(5, 1920, 400).with_type(WindowType::Dock),
        ms(5),
    );
    policy.activate(SEAT, id(2), ms(6));
    policy.raise(id(3));
    policy.raise(id(4));
    assert_eq!(policy.stacking(0), [id(1), id(2), id(3), id(4), id(5)]);
    assert_eq!(policy.focus(SEAT), Some(id(2)));

    policy.action(SEAT, Action::RaiseOrLower, ms(7));

    assert_eq!(policy.stacking(0), [id(2), id(1), id(3), id(4), id(5)]);
    assert_eq!(policy.focus(SEAT), Some(id(2)));
}

#[test]
fn lower_keeps_the_focus_unlike_the_titlebars() {
    let mut policy = policy();
    two_overlapping(&mut policy);

    policy.action(SEAT, Action::Lower, ms(3));

    assert_eq!(policy.stacking(0), [id(2), id(1)]);
    assert_eq!(policy.focus(SEAT), Some(id(2)));

    policy.action(SEAT, Action::Raise, ms(4));
    assert_eq!(policy.stacking(0), [id(1), id(2)]);
}

#[test]
fn moving_to_a_numbered_workspace_leaves_the_active_workspace_and_the_focus_behind() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));

    policy.action(SEAT, Action::MoveToWorkspace(2), ms(3));

    assert_eq!(policy.workspace_of(id(2)), Some(2));
    assert_eq!(policy.active_workspace(), 0);
    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn moving_to_the_workspace_on_the_right_takes_the_active_workspace_and_the_focus_along() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.activate(SEAT, id(1), ms(3));

    policy.action(SEAT, Action::MoveToWorkspaceRight, ms(4));

    assert_eq!(policy.workspace_of(id(1)), Some(1));
    assert_eq!(policy.active_workspace(), 1);
    assert_eq!(policy.focus(SEAT), Some(id(1)));

    policy.action(SEAT, Action::MoveToWorkspaceLeft, ms(5));
    assert_eq!(policy.workspace_of(id(1)), Some(0));
    assert_eq!(policy.active_workspace(), 0);
    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn moving_to_a_workspace_that_isnt_there_does_nothing() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    // Four workspaces, numbered from 0, and nothing left of the first.
    policy.action(SEAT, Action::MoveToWorkspace(4), ms(2));
    policy.action(SEAT, Action::MoveToWorkspaceLeft, ms(3));
    policy.action(SEAT, Action::SwitchToWorkspace(4), ms(4));

    assert_eq!(policy.workspace_of(id(1)), Some(0));
    assert_eq!(policy.active_workspace(), 0);
    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn the_desktop_and_docks_dont_move_to_another_workspace() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 1920, 1080).with_type(WindowType::Desktop),
        ms(1),
    );
    policy.map_window(
        TestWindow::new(2, 1920, 30).with_type(WindowType::Dock),
        ms(2),
    );
    for w in 1..=2 {
        policy.click(SEAT, id(w), ms(3));
        policy.activate(SEAT, id(w), ms(3));

        policy.action(SEAT, Action::MoveToWorkspace(1), ms(4));
        policy.action(SEAT, Action::MoveToWorkspaceRight, ms(5));

        assert_eq!(policy.workspace_of(id(w)), Some(0));
        assert_eq!(policy.active_workspace(), 0);
    }
}

#[test]
fn a_fixed_size_window_isnt_maximised_but_is_made_fullscreen() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_hints((400, 300), (400, 300)),
        ms(1),
    );

    for action in [
        Action::Maximize,
        Action::ToggleMaximized,
        Action::ToggleMaximizedHorizontally,
        Action::ToggleMaximizedVertically,
    ] {
        policy.action(SEAT, action, ms(2));
    }

    assert!(!policy.is_maximized_horizontally(id(1)));
    assert!(!policy.is_maximized_vertically(id(1)));
    // Not as in Marco (177).
    policy.action(SEAT, Action::ToggleFullscreen, ms(2));
    assert!(policy.is_fullscreen(id(1)));
    policy.action(SEAT, Action::ToggleFullscreen, ms(2));
    assert!(!policy.is_fullscreen(id(1)));
    // It can still be minimised.
    policy.action(SEAT, Action::Minimize, ms(3));
    assert!(policy.is_minimized(id(1)));
}

#[test]
fn the_window_state_actions_maximise_unmaximise_and_toggle_the_focused_window() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    policy.action(SEAT, Action::Maximize, ms(2));
    assert!(policy.is_maximized(id(1)));
    policy.action(SEAT, Action::Maximize, ms(3));
    assert!(policy.is_maximized(id(1)));
    policy.action(SEAT, Action::Unmaximize, ms(4));
    assert!(!policy.is_maximized_horizontally(id(1)));
    assert!(!policy.is_maximized_vertically(id(1)));

    policy.action(SEAT, Action::ToggleMaximizedVertically, ms(5));
    assert!(policy.is_maximized_vertically(id(1)));
    assert!(!policy.is_maximized_horizontally(id(1)));
    policy.action(SEAT, Action::ToggleMaximizedHorizontally, ms(6));
    assert!(policy.is_maximized(id(1)));
    // Unmaximise undoes either way alone, too.
    policy.action(SEAT, Action::ToggleMaximizedHorizontally, ms(7));
    policy.action(SEAT, Action::Unmaximize, ms(8));
    assert!(!policy.is_maximized_vertically(id(1)));

    policy.action(SEAT, Action::ToggleFullscreen, ms(9));
    assert!(policy.is_fullscreen(id(1)));
    policy.action(SEAT, Action::ToggleFullscreen, ms(10));
    assert!(!policy.is_fullscreen(id(1)));
}

#[test]
fn toggle_shaded_shades_an_ssd_window_and_leaves_a_csd_one() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.action(SEAT, Action::ToggleShaded, ms(2));
    assert!(!policy.is_shaded(id(1)));

    policy.map_window(TestWindow::new(2, 400, 300).with_frame(4, 4, 24, 4), ms(3));
    policy.action(SEAT, Action::ToggleShaded, ms(4));
    assert!(policy.is_shaded(id(2)));
    policy.action(SEAT, Action::ToggleShaded, ms(5));
    assert!(!policy.is_shaded(id(2)));
}

#[test]
fn only_a_normal_window_or_a_dialog_is_minimised_maximised_or_made_fullscreen() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_type(WindowType::Utility),
        ms(1),
    );
    policy.click(SEAT, id(1), ms(2));
    assert_eq!(policy.focus(SEAT), Some(id(1)));

    for action in [
        Action::Minimize,
        Action::Maximize,
        Action::ToggleMaximized,
        Action::ToggleFullscreen,
    ] {
        policy.action(SEAT, action, ms(2));
    }
    assert!(!policy.is_minimized(id(1)));
    assert!(!policy.is_maximized_horizontally(id(1)));
    assert!(!policy.is_fullscreen(id(1)));

    // A resizable utility window is maximised one way, as Marco checks only that it resizes.
    policy.action(SEAT, Action::ToggleMaximizedVertically, ms(3));
    assert!(policy.is_maximized_vertically(id(1)));
}

#[test]
fn the_window_actions_do_nothing_with_no_focused_window() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(2));
    assert_eq!(policy.focus(SEAT), None);

    for action in [
        Action::Minimize,
        Action::Maximize,
        Action::ToggleFullscreen,
        Action::RaiseOrLower,
        Action::MoveToWorkspace(0),
        Action::MoveToWorkspaceLeft,
    ] {
        policy.action(SEAT, action, ms(3));
    }

    assert_eq!(policy.active_workspace(), 1);
    assert!(!policy.is_minimized(id(1)));
    assert!(!policy.is_maximized(id(1)));
    assert!(!policy.is_fullscreen(id(1)));
}
