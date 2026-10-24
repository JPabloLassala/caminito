//! Intellihide (CMN-224, ADR 0047): when a panel that asks for it is hidden, and when the
//! pointer at its edge reveals it.

use super::*;
use crate::{IntellihidePanel, LayerSurfaceId, PanelEdge};

const PANEL: LayerSurfaceId = LayerSurfaceId(7);

fn at(x: f64, y: f64) -> Point<f64, Logical> {
    Point::new(x, y)
}

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Logical> {
    Rectangle::new((x, y).into(), (w, h).into())
}

/// A panel along the bottom of the first output, as wide as it and 28 high.
fn bottom_panel() -> IntellihidePanel {
    IntellihidePanel {
        id: PANEL,
        output: OutputId(1),
        rect: rect(0, 1052, 1920, 28),
        edge: PanelEdge::Bottom,
        held: false,
    }
}

/// A policy with a window high on its output, and a bottom panel that asks for intellihide.
fn with_a_panel() -> Policy<TestWindow> {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(100, 100),
        ms(1),
    );
    policy.set_intellihide_panels(&[bottom_panel()]);
    policy
}

fn move_window(policy: &mut Policy<TestWindow>, window: u64, x: i32, y: i32) {
    policy.request_geometry(id(window), Some(Point::from((x, y))), None);
}

/// Moves the pointer to the bottom edge of the first output, under the panel.
fn rest_at_the_edge(policy: &mut Policy<TestWindow>, now: Duration) {
    policy.intellihide_pointer(at(500., 1079.5), now);
}

/// A policy whose window covers the panel, as a maximised one does.
fn with_a_panel_covered() -> Policy<TestWindow> {
    let mut policy = with_a_panel();
    policy.set_maximized(id(1), true);
    assert!(policy.is_intellihide_hidden(PANEL));
    policy
}

#[test]
fn a_panel_nothing_overlaps_is_shown() {
    let policy = with_a_panel();

    assert!(!policy.is_intellihide_hidden(PANEL));
}

#[test]
fn a_window_moved_over_the_panel_hides_it() {
    let mut policy = with_a_panel();

    move_window(&mut policy, 1, 100, 1000);

    assert!(policy.is_intellihide_hidden(PANEL));
}

#[test]
fn a_window_moved_away_shows_it_again() {
    let mut policy = with_a_panel();
    move_window(&mut policy, 1, 100, 1000);

    move_window(&mut policy, 1, 100, 100);

    assert!(!policy.is_intellihide_hidden(PANEL));
}

#[test]
fn a_window_that_only_touches_the_panel_does_not_overlap_it() {
    let mut policy = with_a_panel();

    // 300 high, so the window's frame ends at 1052 where the panel starts.
    move_window(&mut policy, 1, 100, 752);

    assert!(!policy.is_intellihide_hidden(PANEL));
}

#[test]
fn a_maximised_window_hides_the_panel_and_unmaximising_shows_it() {
    let mut policy = with_a_panel();

    policy.set_maximized(id(1), true);
    assert!(policy.is_intellihide_hidden(PANEL));

    policy.set_maximized(id(1), false);
    assert!(!policy.is_intellihide_hidden(PANEL));
}

#[test]
fn a_window_on_another_workspace_does_not_count() {
    let mut policy = with_a_panel_covered();

    policy.move_to_workspace(id(1), 1);

    assert!(!policy.is_intellihide_hidden(PANEL));
    // Its workspace comes back, and so does the hiding.
    policy.switch_workspace(1);
    assert!(policy.is_intellihide_hidden(PANEL));
}

#[test]
fn a_sticky_window_counts_on_every_workspace() {
    let mut policy = with_a_panel_covered();
    policy.set_sticky(id(1), true);

    policy.switch_workspace(1);

    assert!(policy.is_intellihide_hidden(PANEL));
}

#[test]
fn a_minimised_window_does_not_count() {
    let mut policy = with_a_panel_covered();

    policy.minimize(id(1));

    assert!(!policy.is_intellihide_hidden(PANEL));
}

#[test]
fn the_desktop_and_a_dock_do_not_count() {
    let mut policy = with_a_panel();
    policy.map_window(
        TestWindow::new(2, 1920, 1080)
            .with_type(WindowType::Desktop)
            .with_requested_position(0, 0),
        ms(2),
    );
    policy.map_window(
        TestWindow::new(3, 600, 60)
            .with_type(WindowType::Dock)
            .with_requested_position(300, 1020),
        ms(3),
    );

    assert!(!policy.is_intellihide_hidden(PANEL));
}

/// Two outputs side by side, a panel along the bottom of each, and a window high on the
/// first, with `workspaces-per-output` as `per_output` says.
fn two_panels(per_output: bool) -> Policy<TestWindow> {
    let mut policy = Policy::new(Config {
        workspaces_per_output: per_output,
        ..Config::default()
    });
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy.add_output(output(2, 1920, 0, 1280, 1024));
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(100, 100),
        ms(1),
    );
    policy.map_window(
        TestWindow::new(2, 400, 300).with_requested_position(2100, 100),
        ms(2),
    );
    policy.set_intellihide_panels(&[
        bottom_panel(),
        IntellihidePanel {
            id: LayerSurfaceId(8),
            output: OutputId(2),
            rect: rect(1920, 996, 1280, 28),
            edge: PanelEdge::Bottom,
            held: false,
        },
    ]);
    policy
}

#[test]
fn a_window_over_one_outputs_panel_leaves_the_other_outputs_panel_shown() {
    let mut policy = two_panels(true);

    move_window(&mut policy, 1, 100, 1000);

    assert!(policy.is_intellihide_hidden(PANEL));
    assert!(!policy.is_intellihide_hidden(LayerSurfaceId(8)));
}

#[test]
fn a_window_on_the_active_workspace_of_another_output_does_not_count() {
    let mut policy = two_panels(true);
    move_window(&mut policy, 2, 2100, 800);
    assert!(policy.is_intellihide_hidden(LayerSurfaceId(8)));

    // The second output shows its workspace 1, so window 2 on 0 is not on it.
    policy.switch_output_workspace(OutputId(2), 1);

    assert!(!policy.is_intellihide_hidden(LayerSurfaceId(8)));
    // The first output still shows 0, where window 1 is.
    move_window(&mut policy, 1, 100, 1000);
    assert!(policy.is_intellihide_hidden(PANEL));
}

#[test]
fn the_pointer_resting_at_the_edge_reveals_a_hidden_panel_after_250_ms() {
    let mut policy = with_a_panel_covered();

    rest_at_the_edge(&mut policy, ms(1000));

    assert_eq!(policy.deadline(), Some(ms(1250)));
    policy.elapse(ms(1249));
    assert!(policy.is_intellihide_hidden(PANEL));
    policy.elapse(ms(1250));
    assert!(!policy.is_intellihide_hidden(PANEL));
    assert_eq!(policy.deadline(), None);
}

#[test]
fn motion_along_the_edge_does_not_start_the_rest_again() {
    let mut policy = with_a_panel_covered();
    rest_at_the_edge(&mut policy, ms(1000));

    policy.intellihide_pointer(at(900., 1079.5), ms(1200));

    assert_eq!(policy.deadline(), Some(ms(1250)));
}

#[test]
fn leaving_the_edge_before_the_rest_is_over_reveals_nothing() {
    let mut policy = with_a_panel_covered();
    rest_at_the_edge(&mut policy, ms(1000));

    policy.intellihide_pointer(at(500., 600.), ms(1100));

    assert_eq!(policy.deadline(), None);
    policy.elapse(ms(2000));
    assert!(policy.is_intellihide_hidden(PANEL));
}

#[test]
fn the_edge_beside_the_panel_reveals_nothing() {
    let mut policy = with_a_panel();
    policy.set_intellihide_panels(&[IntellihidePanel {
        rect: rect(460, 1052, 1000, 28),
        ..bottom_panel()
    }]);
    policy.set_maximized(id(1), true);

    policy.intellihide_pointer(at(100., 1079.5), ms(1000));

    assert_eq!(policy.deadline(), None);
}

#[test]
fn the_pointer_one_pixel_above_the_edge_is_not_at_it() {
    let mut policy = with_a_panel_covered();

    policy.intellihide_pointer(at(500., 1078.), ms(1000));

    assert_eq!(policy.deadline(), None);
}

#[test]
fn leaving_the_panel_hides_it_again_while_something_overlaps() {
    let mut policy = with_a_panel_covered();
    rest_at_the_edge(&mut policy, ms(1000));
    policy.elapse(ms(1250));
    assert!(!policy.is_intellihide_hidden(PANEL));

    // Along the panel, it stays.
    policy.intellihide_pointer(at(700., 1060.), ms(1300));
    assert!(!policy.is_intellihide_hidden(PANEL));

    policy.intellihide_pointer(at(700., 900.), ms(1400));
    assert!(policy.is_intellihide_hidden(PANEL));
}

#[test]
fn a_panel_that_floats_off_the_edge_is_revealed_and_kept_by_the_gap_too() {
    let mut policy = with_a_panel();
    policy.set_intellihide_panels(&[IntellihidePanel {
        rect: rect(460, 1032, 1000, 40),
        ..bottom_panel()
    }]);
    policy.set_maximized(id(1), true);
    policy.intellihide_pointer(at(700., 1079.5), ms(1000));
    policy.elapse(ms(1250));
    assert!(!policy.is_intellihide_hidden(PANEL));

    // The 8 pixels between the panel and the edge are still the panel's.
    policy.intellihide_pointer(at(700., 1076.), ms(1300));
    policy.intellihide_pointer(at(700., 1079.5), ms(1400));

    assert!(!policy.is_intellihide_hidden(PANEL));
}

#[test]
fn a_panel_shown_because_nothing_overlaps_stays_shown_when_the_pointer_leaves() {
    let mut policy = with_a_panel();

    policy.intellihide_pointer(at(700., 1060.), ms(1000));
    policy.intellihide_pointer(at(700., 100.), ms(1100));

    assert!(!policy.is_intellihide_hidden(PANEL));
}

#[test]
fn a_window_moving_over_the_panel_hides_it_even_with_the_pointer_gone() {
    let mut policy = with_a_panel();
    policy.intellihide_pointer(at(700., 100.), ms(1000));

    move_window(&mut policy, 1, 100, 1000);

    assert!(policy.is_intellihide_hidden(PANEL));
}

#[test]
fn a_window_moving_over_the_panel_under_the_pointer_hides_it_once_the_pointer_leaves() {
    let mut policy = with_a_panel();
    policy.intellihide_pointer(at(700., 1060.), ms(1000));

    // The panel is not taken out from under the pointer that is on it.
    move_window(&mut policy, 1, 100, 1000);
    assert!(!policy.is_intellihide_hidden(PANEL));

    policy.intellihide_pointer(at(700., 100.), ms(2000));
    assert!(policy.is_intellihide_hidden(PANEL));
}

#[test]
fn a_panel_with_a_popup_open_is_not_hidden() {
    let mut policy = with_a_panel_covered();

    policy.set_intellihide_panels(&[IntellihidePanel {
        held: true,
        ..bottom_panel()
    }]);
    assert!(!policy.is_intellihide_hidden(PANEL));

    policy.set_intellihide_panels(&[bottom_panel()]);
    assert!(policy.is_intellihide_hidden(PANEL));
}

#[test]
fn a_popup_closing_with_the_pointer_gone_from_the_panel_hides_it_again() {
    let mut policy = with_a_panel_covered();
    rest_at_the_edge(&mut policy, ms(1000));
    policy.elapse(ms(1250));
    policy.set_intellihide_panels(&[IntellihidePanel {
        held: true,
        ..bottom_panel()
    }]);
    // On to the panel's menu, above it.
    policy.intellihide_pointer(at(700., 900.), ms(1400));
    assert!(!policy.is_intellihide_hidden(PANEL));

    // An item is chosen and the menu closes, with no motion after it.
    policy.set_intellihide_panels(&[bottom_panel()]);

    assert!(policy.is_intellihide_hidden(PANEL));
}

#[test]
fn a_popup_closing_with_the_pointer_on_the_panel_leaves_it_shown() {
    let mut policy = with_a_panel_covered();
    rest_at_the_edge(&mut policy, ms(1000));
    policy.elapse(ms(1250));
    policy.set_intellihide_panels(&[IntellihidePanel {
        held: true,
        ..bottom_panel()
    }]);
    policy.intellihide_pointer(at(700., 1060.), ms(1400));

    policy.set_intellihide_panels(&[bottom_panel()]);

    assert!(!policy.is_intellihide_hidden(PANEL));
}

#[test]
fn a_panel_that_stopped_asking_is_forgotten_and_never_hidden() {
    let mut policy = with_a_panel_covered();
    rest_at_the_edge(&mut policy, ms(1000));

    policy.set_intellihide_panels(&[]);

    assert!(!policy.is_intellihide_hidden(PANEL));
    assert_eq!(policy.deadline(), None);
}

#[test]
fn a_panel_asking_again_starts_with_the_pointer_far_from_it() {
    let mut policy = with_a_panel_covered();
    rest_at_the_edge(&mut policy, ms(1000));
    policy.elapse(ms(1250));
    policy.set_intellihide_panels(&[]);
    policy.intellihide_pointer(at(500., 100.), ms(2000));

    policy.set_intellihide_panels(&[bottom_panel()]);

    assert!(policy.is_intellihide_hidden(PANEL));
}

#[test]
fn the_other_edges_reveal_at_their_own_sides() {
    let top = IntellihidePanel {
        rect: rect(0, 0, 1920, 28),
        edge: PanelEdge::Top,
        ..bottom_panel()
    };
    let left = IntellihidePanel {
        rect: rect(0, 0, 40, 1080),
        edge: PanelEdge::Left,
        ..bottom_panel()
    };
    let right = IntellihidePanel {
        rect: rect(1880, 0, 40, 1080),
        edge: PanelEdge::Right,
        ..bottom_panel()
    };
    for (panel, edge) in [
        (top, at(500., 0.)),
        (left, at(0., 500.)),
        (right, at(1919.5, 500.)),
    ] {
        let mut policy = with_a_panel();
        policy.set_intellihide_panels(&[panel]);
        policy.set_maximized(id(1), true);
        assert!(policy.is_intellihide_hidden(PANEL), "{:?}", panel.edge);

        policy.intellihide_pointer(edge, ms(1000));
        policy.elapse(ms(1250));

        assert!(!policy.is_intellihide_hidden(PANEL), "{:?}", panel.edge);
    }
}
