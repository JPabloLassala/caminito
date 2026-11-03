//! The fullscreen layer with two outputs (ADR 0057): a fullscreen window covers its output's
//! panel while it or a transient of it was the last window focused on that output, where
//! Marco goes by the focus on any output.

use super::per_output_workspaces::two_outputs;
use super::*;

/// `two_outputs` with a dock and window 4, kept above, on the left output, and window 1
/// focused and fullscreen there. Window 2 is on the right output.
fn fullscreen_on_the_left() -> Policy<TestWindow> {
    let mut policy = two_outputs(false);
    policy.map_window(
        TestWindow::new(3, 1920, 30).with_type(WindowType::Dock),
        ms(3),
    );
    policy.map_window(
        TestWindow::new(4, 400, 300).with_requested_position(600, 100),
        ms(4),
    );
    policy.set_above(id(4), true);
    policy.click(SEAT, id(1), ms(5));
    policy.set_fullscreen(id(1), true, None);
    assert_eq!(policy.layer(id(1)), Some(Layer::Fullscreen));
    policy
}

#[test]
fn a_fullscreen_window_stays_over_the_panel_when_another_output_takes_the_focus() {
    let mut policy = fullscreen_on_the_left();

    policy.click(SEAT, id(2), ms(6));

    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert_eq!(policy.layer(id(1)), Some(Layer::Fullscreen));
    assert_eq!(policy.stacking(0), [id(2), id(3), id(4), id(1)]);
}

#[test]
fn a_window_focused_on_its_output_lowers_it_until_it_has_the_focus_again() {
    let mut policy = fullscreen_on_the_left();

    policy.click(SEAT, id(4), ms(6));
    assert_eq!(policy.layer(id(1)), Some(Layer::Normal));
    assert_eq!(policy.stacking(0), [id(2), id(1), id(3), id(4)]);

    policy.click(SEAT, id(2), ms(7));
    assert_eq!(policy.layer(id(1)), Some(Layer::Normal));
    assert_eq!(policy.stacking(0), [id(1), id(2), id(3), id(4)]);

    policy.click(SEAT, id(1), ms(8));
    assert_eq!(policy.layer(id(1)), Some(Layer::Fullscreen));
    assert_eq!(policy.stacking(0)[3], id(1));
}

#[test]
fn a_focused_transient_keeps_it_over_the_panel() {
    let mut policy = fullscreen_on_the_left();
    policy.map_window(
        TestWindow::new(5, 200, 100)
            .with_parent(1)
            .with_requested_position(200, 200),
        ms(6),
    );
    assert_eq!(policy.focus(SEAT), Some(id(5)));

    policy.click(SEAT, id(2), ms(7));

    assert_eq!(policy.layer(id(1)), Some(Layer::Fullscreen));
    assert_eq!(policy.stacking(0), [id(2), id(3), id(4), id(1), id(5)]);
}

#[test]
fn a_switch_to_an_empty_workspace_lowers_it_as_nothing_has_the_focus() {
    let mut policy = fullscreen_on_the_left();

    policy.switch_workspace(1);

    assert_eq!(policy.focus(SEAT), None);
    assert_eq!(policy.layer(id(1)), Some(Layer::Normal));
}
