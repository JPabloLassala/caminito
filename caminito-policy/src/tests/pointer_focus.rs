//! The pointer focus modes, `raise-on-click` and auto-raise (153).

use super::*;
use crate::{Crossing, FocusMode, PointerTarget, SwitchList};

/// A policy in `mode`, with two windows mapped, 2 above and focused.
fn two_windows(mode: FocusMode) -> Policy<TestWindow> {
    let mut policy = policy();
    policy.settings_changed(Config {
        focus_mode: mode,
        ..Config::default()
    });
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy
}

fn enter(policy: &mut Policy<TestWindow>, target: PointerTarget, now: u64) {
    policy.pointer_entered(SEAT, target, Crossing::Motion, ms(now));
}

#[test]
fn sloppy_focuses_the_window_the_pointer_enters_without_raising_it() {
    let mut policy = two_windows(FocusMode::Sloppy);

    enter(&mut policy, PointerTarget::Window(id(1)), 3);

    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert_eq!(policy.stacking(0), [id(1), id(2)]);
}

#[test]
fn sloppy_follows_the_pointer_from_window_to_window() {
    let mut policy = two_windows(FocusMode::Sloppy);
    enter(&mut policy, PointerTarget::Window(id(1)), 3);

    enter(&mut policy, PointerTarget::Window(id(2)), 4);

    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert!(window(&policy, 2).is_activated());
    assert!(!window(&policy, 1).is_activated());
}

#[test]
fn sloppy_keeps_the_focus_when_the_pointer_leaves_for_the_desktop() {
    let mut policy = two_windows(FocusMode::Sloppy);
    enter(&mut policy, PointerTarget::Window(id(1)), 3);

    enter(&mut policy, PointerTarget::Desktop, 4);

    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert!(window(&policy, 1).is_activated());
}

#[test]
fn mouse_focuses_the_window_the_pointer_enters_and_follows_it_to_another() {
    let mut policy = two_windows(FocusMode::Mouse);

    enter(&mut policy, PointerTarget::Window(id(1)), 3);
    assert_eq!(policy.focus(SEAT), Some(id(1)));

    enter(&mut policy, PointerTarget::Window(id(2)), 4);
    assert_eq!(policy.focus(SEAT), Some(id(2)));
}

#[test]
fn mouse_takes_the_focus_away_when_the_pointer_leaves_for_the_desktop() {
    let mut policy = two_windows(FocusMode::Mouse);
    enter(&mut policy, PointerTarget::Window(id(1)), 3);

    enter(&mut policy, PointerTarget::Desktop, 4);

    assert_eq!(policy.focus(SEAT), None);
    assert!(!window(&policy, 1).is_activated());
    assert!(!window(&policy, 2).is_activated());
}

#[test]
fn mouse_takes_the_focus_away_when_the_pointer_enters_a_desktop_window() {
    let mut policy = two_windows(FocusMode::Mouse);
    policy.map_window(
        TestWindow::new(3, 1920, 1080).with_type(WindowType::Desktop),
        ms(3),
    );
    enter(&mut policy, PointerTarget::Window(id(1)), 4);

    enter(&mut policy, PointerTarget::Window(id(3)), 5);

    assert_eq!(policy.focus(SEAT), None);
}

#[test]
fn mouse_keeps_the_focus_when_the_pointer_enters_a_dock_or_a_panel() {
    let mut policy = two_windows(FocusMode::Mouse);
    policy.map_window(
        TestWindow::new(3, 1920, 30).with_type(WindowType::Dock),
        ms(3),
    );
    enter(&mut policy, PointerTarget::Window(id(1)), 4);

    enter(&mut policy, PointerTarget::Window(id(3)), 5);
    assert_eq!(policy.focus(SEAT), Some(id(1)));

    enter(&mut policy, PointerTarget::Other, 6);
    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn click_mode_leaves_the_focus_to_clicks_whatever_the_pointer_crosses() {
    let mut policy = two_windows(FocusMode::Click);

    enter(&mut policy, PointerTarget::Window(id(1)), 3);
    assert_eq!(policy.focus(SEAT), Some(id(2)));

    enter(&mut policy, PointerTarget::Desktop, 4);
    assert_eq!(policy.focus(SEAT), Some(id(2)));
}

#[test]
fn entering_a_window_with_a_modal_dialog_focuses_the_dialog() {
    let mut policy = two_windows(FocusMode::Sloppy);
    policy.map_window(
        TestWindow::new(3, 200, 100)
            .with_parent(1)
            .with_type(WindowType::ModalDialog),
        ms(3),
    );
    enter(&mut policy, PointerTarget::Window(id(2)), 4);

    enter(&mut policy, PointerTarget::Window(id(1)), 5);

    assert_eq!(policy.focus(SEAT), Some(id(3)));
}

/// `two_windows` with `raise-on-click` off.
fn not_raising_on_click() -> Policy<TestWindow> {
    let mut policy = policy();
    policy.settings_changed(Config {
        raise_on_click: false,
        ..Config::default()
    });
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy
}

#[test]
fn without_raise_on_click_a_click_on_the_body_focuses_without_raising() {
    let mut policy = not_raising_on_click();

    policy.click(SEAT, id(1), ms(3));

    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert_eq!(policy.stacking(0), [id(1), id(2)]);
}

#[test]
fn without_raise_on_click_a_click_on_the_titlebar_raises_once_it_is_released() {
    let mut policy = not_raising_on_click();

    policy.click(SEAT, id(1), ms(3));
    assert_eq!(policy.stacking(0), [id(1), id(2)]);
    policy.titlebar_released(id(1));

    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert_eq!(policy.stacking(0), [id(2), id(1)]);
}

#[test]
fn without_raise_on_click_dragging_the_titlebar_moves_the_window_without_raising_it() {
    let mut policy = not_raising_on_click();
    let press = Point::from((10., 10.));

    policy.click(SEAT, id(1), ms(3));
    policy.begin_titlebar_move(SEAT, id(1), press, Point::from((20., 10.)));
    policy.grab_motion(SEAT, Point::from((60., 10.)), false);
    policy.end_grab(SEAT);

    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert_eq!(policy.stacking(0), [id(1), id(2)]);
}

#[test]
fn without_raise_on_click_a_move_that_hardly_moves_is_a_click_and_raises() {
    let mut policy = not_raising_on_click();

    policy.click(SEAT, id(1), ms(3));
    policy.begin_move(SEAT, id(1), Point::from((10., 10.)));
    policy.grab_motion(SEAT, Point::from((14., 13.)), false);
    policy.end_grab(SEAT);

    assert_eq!(policy.stacking(0), [id(2), id(1)]);
}

#[test]
fn without_raise_on_click_a_click_on_a_modal_dialogs_parent_focuses_the_dialog_and_raises_nothing()
{
    let mut policy = not_raising_on_click();
    policy.map_window(
        TestWindow::new(3, 200, 100)
            .with_parent(1)
            .with_type(WindowType::ModalDialog),
        ms(3),
    );
    policy.click(SEAT, id(2), ms(4));
    let stacking = policy.stacking(0).to_vec();

    policy.click(SEAT, id(1), ms(5));

    assert_eq!(policy.focus(SEAT), Some(id(3)));
    assert_eq!(policy.stacking(0), stacking);
}

/// A policy in sloppy mode with auto-raise after 500 ms, and two windows, 2 above.
fn auto_raising() -> Policy<TestWindow> {
    let mut policy = policy();
    policy.settings_changed(Config {
        focus_mode: FocusMode::Sloppy,
        auto_raise: true,
        auto_raise_delay: ms(500),
        ..Config::default()
    });
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy
}

#[test]
fn auto_raise_raises_the_window_the_pointer_focused_once_the_delay_is_up() {
    let mut policy = auto_raising();

    enter(&mut policy, PointerTarget::Window(id(1)), 1000);
    assert_eq!(policy.deadline(), Some(ms(1500)));

    policy.elapse(ms(1499));
    assert_eq!(policy.stacking(0), [id(1), id(2)]);

    policy.elapse(ms(1500));
    assert_eq!(policy.stacking(0), [id(2), id(1)]);
    assert_eq!(policy.deadline(), None);
}

#[test]
fn leaving_the_window_before_the_delay_is_up_cancels_its_auto_raise() {
    let mut policy = auto_raising();
    enter(&mut policy, PointerTarget::Window(id(1)), 1000);

    enter(&mut policy, PointerTarget::Desktop, 1200);
    policy.elapse(ms(1500));

    assert_eq!(policy.deadline(), None);
    assert_eq!(policy.stacking(0), [id(1), id(2)]);
    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn auto_raise_waits_for_a_pointer_focus_mode() {
    let mut policy = auto_raising();
    policy.settings_changed(Config {
        auto_raise: true,
        ..Config::default()
    });

    enter(&mut policy, PointerTarget::Window(id(1)), 1000);

    assert_eq!(policy.deadline(), None);
}

#[test]
fn a_change_of_focus_mode_applies_at_once() {
    let mut policy = two_windows(FocusMode::Click);
    enter(&mut policy, PointerTarget::Window(id(1)), 3);
    assert_eq!(policy.focus(SEAT), Some(id(2)));

    policy.settings_changed(Config {
        focus_mode: FocusMode::Sloppy,
        ..Config::default()
    });
    enter(&mut policy, PointerTarget::Window(id(2)), 4);
    enter(&mut policy, PointerTarget::Window(id(1)), 5);

    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn leaving_the_pointer_modes_drops_a_waiting_auto_raise() {
    let mut policy = auto_raising();
    enter(&mut policy, PointerTarget::Window(id(1)), 1000);

    policy.settings_changed(Config::default());

    assert_eq!(policy.deadline(), None);
    policy.elapse(ms(1500));
    assert_eq!(policy.stacking(0), [id(1), id(2)]);
}

#[test]
fn the_window_switcher_holds_the_pointer_focus_modes_off_while_it_is_open() {
    let mut policy = two_windows(FocusMode::Sloppy);
    policy.action(
        SEAT,
        Action::Switch {
            list: SwitchList::Windows,
            backward: false,
        },
        ms(3),
    );
    assert!(policy.switcher().is_some());

    enter(&mut policy, PointerTarget::Window(id(1)), 4);
    assert_eq!(policy.focus(SEAT), Some(id(2)));

    policy.switcher_cancel(ms(5));
    enter(&mut policy, PointerTarget::Window(id(2)), 6);
    enter(&mut policy, PointerTarget::Window(id(1)), 7);
    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn a_window_going_from_under_a_still_pointer_focuses_the_one_it_uncovers() {
    let mut policy = two_windows(FocusMode::Sloppy);
    policy.map_window(TestWindow::new(3, 400, 300), ms(3));
    enter(&mut policy, PointerTarget::Window(id(3)), 4);
    policy.click(SEAT, id(2), ms(5));

    policy.unmap_window(id(3));
    policy.pointer_entered(SEAT, PointerTarget::Window(id(1)), Crossing::Scene, ms(6));

    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn after_the_window_switcher_only_the_pointers_motion_moves_the_focus() {
    let mut policy = two_windows(FocusMode::Sloppy);
    enter(&mut policy, PointerTarget::Window(id(2)), 3);
    let switch = Action::Switch {
        list: SwitchList::Windows,
        backward: false,
    };
    policy.action(SEAT, switch, ms(4));
    policy.switcher_commit(ms(5));
    assert_eq!(policy.focus(SEAT), Some(id(1)));

    // Window 1, raised, now covers the still pointer.
    policy.pointer_entered(SEAT, PointerTarget::Window(id(1)), Crossing::Scene, ms(6));
    policy.pointer_entered(SEAT, PointerTarget::Window(id(2)), Crossing::Scene, ms(7));
    assert_eq!(policy.focus(SEAT), Some(id(1)));

    enter(&mut policy, PointerTarget::Desktop, 8);
    enter(&mut policy, PointerTarget::Window(id(2)), 9);
    assert_eq!(policy.focus(SEAT), Some(id(2)));
}
