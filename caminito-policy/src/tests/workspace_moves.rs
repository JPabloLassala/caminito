//! Up, down and the previous workspace, and `wrap-style` (150): the moves through the
//! Workspace grid that the bindings make.

use super::*;
use crate::WrapStyle;

/// A policy with `count` workspaces in `rows` rows, wrapping by `wrap`.
fn grid(count: usize, rows: usize, wrap: WrapStyle) -> Policy<TestWindow> {
    let mut policy = policy();
    policy.settings_changed(Config {
        workspace_count: count,
        workspace_layout: WorkspaceLayout {
            rows,
            ..WorkspaceLayout::default()
        },
        wrap_style: wrap,
        ..Config::default()
    });
    policy
}

/// The active workspace after `action`, taken at `t` ms.
fn after(policy: &mut Policy<TestWindow>, action: Action, t: u64) -> usize {
    policy.action(SEAT, action, ms(t));
    policy.active_workspace()
}

#[test]
fn up_and_down_in_one_row_stay_put() {
    let mut policy = grid(4, 1, WrapStyle::NoWrap);
    policy.switch_workspace(2);
    assert_eq!(after(&mut policy, Action::SwitchToWorkspaceUp, 1), 2);
    assert_eq!(after(&mut policy, Action::SwitchToWorkspaceDown, 2), 2);
}

#[test]
fn up_and_down_go_by_the_grid() {
    // 0 1
    // 2 3
    let mut policy = grid(4, 2, WrapStyle::NoWrap);
    assert_eq!(after(&mut policy, Action::SwitchToWorkspaceUp, 1), 0);
    assert_eq!(after(&mut policy, Action::SwitchToWorkspaceDown, 2), 2);
    assert_eq!(after(&mut policy, Action::SwitchToWorkspaceDown, 3), 2);
    policy.switch_workspace(3);
    assert_eq!(after(&mut policy, Action::SwitchToWorkspaceUp, 4), 1);
}

#[test]
fn down_into_an_empty_cell_stays_put() {
    // 0 1 2
    // 3 4
    let mut policy = grid(5, 2, WrapStyle::NoWrap);
    policy.switch_workspace(2);
    assert_eq!(after(&mut policy, Action::SwitchToWorkspaceDown, 1), 2);
    policy.switch_workspace(1);
    assert_eq!(after(&mut policy, Action::SwitchToWorkspaceDown, 2), 4);
}

#[test]
fn each_wrap_style_at_each_edge() {
    // 0 1
    // 2 3
    let cases = [
        // From, the move, then where no wrap, classic and toroidal go.
        (1, Action::SwitchToWorkspaceRight, [1, 2, 0]),
        (2, Action::SwitchToWorkspaceLeft, [2, 1, 3]),
        (1, Action::SwitchToWorkspaceUp, [1, 2, 3]),
        (2, Action::SwitchToWorkspaceDown, [2, 1, 0]),
        (3, Action::SwitchToWorkspaceRight, [3, 0, 2]),
        (0, Action::SwitchToWorkspaceUp, [0, 3, 2]),
    ];
    for (from, action, to) in cases {
        let wraps = [WrapStyle::NoWrap, WrapStyle::Classic, WrapStyle::Toroidal];
        for (wrap, to) in wraps.into_iter().zip(to) {
            let mut policy = grid(4, 2, wrap);
            policy.switch_workspace(from);
            assert_eq!(
                after(&mut policy, action, 1),
                to,
                "{action:?} from {from}, {wrap:?}"
            );
        }
    }
}

#[test]
fn a_wrap_style_change_applies_at_once() {
    let mut policy = grid(4, 1, WrapStyle::NoWrap);
    policy.switch_workspace(3);
    assert_eq!(after(&mut policy, Action::SwitchToWorkspaceRight, 1), 3);
    policy.settings_changed(Config {
        wrap_style: WrapStyle::Toroidal,
        ..Config::default()
    });
    assert_eq!(after(&mut policy, Action::SwitchToWorkspaceRight, 2), 0);
}

#[test]
fn prev_does_nothing_before_any_switch() {
    let mut policy = policy();
    assert_eq!(after(&mut policy, Action::SwitchToWorkspacePrev, 1), 0);
}

#[test]
fn prev_goes_back_to_the_workspace_a_binding_left() {
    let mut policy = grid(4, 2, WrapStyle::NoWrap);
    assert_eq!(after(&mut policy, Action::SwitchToWorkspaceDown, 1), 2);
    assert_eq!(after(&mut policy, Action::SwitchToWorkspacePrev, 2), 0);
    // And back again, since going back is a switch too.
    assert_eq!(after(&mut policy, Action::SwitchToWorkspacePrev, 3), 2);
    assert_eq!(after(&mut policy, Action::SwitchToWorkspaceRight, 4), 3);
    assert_eq!(after(&mut policy, Action::SwitchToWorkspacePrev, 5), 2);
}

#[test]
fn prev_goes_back_to_the_workspace_the_pager_left() {
    let mut policy = policy();
    assert_eq!(after(&mut policy, Action::SwitchToWorkspace(3), 1), 3);
    assert_eq!(after(&mut policy, Action::SwitchToWorkspace(1), 2), 1);
    assert_eq!(after(&mut policy, Action::SwitchToWorkspacePrev, 3), 3);
}

#[test]
fn prev_goes_back_to_the_workspace_an_activation_left() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.move_to_workspace(id(1), 2);
    policy.switch_workspace(1);

    policy.activate(SEAT, id(1), ms(2));
    assert_eq!(policy.active_workspace(), 2);

    assert_eq!(after(&mut policy, Action::SwitchToWorkspacePrev, 3), 1);
}

#[test]
fn prev_goes_back_to_the_workspace_a_window_was_moved_from() {
    // Marco's move to the workspace beside activates it (`meta_workspace_activate_with_focus`),
    // which keeps the one before as any switch does.
    let mut policy = grid(4, 2, WrapStyle::NoWrap);
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    assert_eq!(after(&mut policy, Action::MoveToWorkspaceDown, 2), 2);
    assert_eq!(after(&mut policy, Action::SwitchToWorkspacePrev, 3), 0);
}

#[test]
fn moving_a_window_by_number_isn_t_a_switch() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.switch_workspace(1);
    policy.switch_workspace(0);
    policy.move_to_workspace(id(1), 3);
    assert_eq!(after(&mut policy, Action::SwitchToWorkspacePrev, 2), 1);
}

#[test]
fn prev_is_forgotten_when_the_count_shrinks_below_it() {
    let mut policy = policy();
    policy.switch_workspace(3);
    policy.switch_workspace(0);

    policy.settings_changed(Config {
        workspace_count: 2,
        ..Config::default()
    });
    assert_eq!(after(&mut policy, Action::SwitchToWorkspacePrev, 1), 0);

    // Until the next switch.
    policy.settings_changed(Config::default());
    policy.switch_workspace(1);
    assert_eq!(after(&mut policy, Action::SwitchToWorkspacePrev, 2), 0);
}

#[test]
fn prev_follows_its_windows_as_dynamic_workspaces_close_up() {
    let dynamic = Config {
        dynamic_workspaces: true,
        ..Config::default()
    };
    let mut policy = policy();
    policy.settings_changed(dynamic);
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.map_window(TestWindow::new(3, 400, 300), ms(3));
    policy.move_to_workspace(id(2), 1);
    policy.move_to_workspace(id(3), 2);
    // 1 | 2 | 3 | (empty)
    policy.switch_workspace(2);
    policy.switch_workspace(0);

    // Closing 2 takes 3 from the third workspace to the second.
    policy.unmap_window(id(2));
    assert_eq!(policy.workspace_of(id(3)), Some(1));
    assert_eq!(after(&mut policy, Action::SwitchToWorkspacePrev, 4), 1);
}

#[test]
fn move_to_workspace_down_carries_the_focused_window_and_keeps_its_focus() {
    // 0 1
    // 2 3
    let mut policy = grid(4, 2, WrapStyle::NoWrap);
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 200, 100).with_parent(1), ms(2));
    policy.map_window(TestWindow::new(3, 400, 300), ms(3));
    policy.activate(SEAT, id(1), ms(4));

    policy.action(SEAT, Action::MoveToWorkspaceDown, ms(5));

    assert_eq!(policy.active_workspace(), 2);
    assert_eq!(policy.stacking(2), [id(1), id(2)]);
    assert_eq!(policy.stacking(0), [id(3)]);
    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert!(window(&policy, 1).is_activated());

    policy.action(SEAT, Action::MoveToWorkspaceUp, ms(6));
    assert_eq!(policy.active_workspace(), 0);
    assert_eq!(policy.workspace_of(id(1)), Some(0));
    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn move_to_workspace_up_past_the_edge_moves_nothing_with_no_wrap() {
    let mut policy = grid(4, 2, WrapStyle::NoWrap);
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    policy.action(SEAT, Action::MoveToWorkspaceUp, ms(2));

    assert_eq!(policy.active_workspace(), 0);
    assert_eq!(policy.workspace_of(id(1)), Some(0));

    let mut policy = grid(4, 2, WrapStyle::Toroidal);
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.action(SEAT, Action::MoveToWorkspaceUp, ms(2));
    assert_eq!(policy.workspace_of(id(1)), Some(2));
    assert_eq!(policy.active_workspace(), 2);
}

#[test]
fn move_to_workspace_down_with_no_focus_or_on_the_desktop_does_nothing() {
    let mut policy = grid(4, 2, WrapStyle::NoWrap);
    policy.action(SEAT, Action::MoveToWorkspaceDown, ms(1));
    assert_eq!(policy.active_workspace(), 0);

    policy.map_window(
        TestWindow::new(1, 1920, 1080).with_type(WindowType::Desktop),
        ms(2),
    );
    policy.activate(SEAT, id(1), ms(3));
    policy.action(SEAT, Action::MoveToWorkspaceDown, ms(4));
    assert_eq!(policy.active_workspace(), 0);
    assert_eq!(policy.workspace_of(id(1)), Some(0));
}
