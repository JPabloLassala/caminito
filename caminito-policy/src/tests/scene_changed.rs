//! `take_scene_changed` (179): policy says when what the Scene reads from it changed, so
//! that no caller has to queue a redraw for what it asked of policy.

use super::*;
use crate::{Crossing, FocusMode, PointerTarget};

#[test]
fn a_change_is_said_once() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));

    assert!(policy.take_scene_changed());
    assert!(!policy.take_scene_changed());
}

#[test]
fn an_auto_raise_changes_what_the_scene_reads() {
    let mut policy = policy();
    policy.settings_changed(Config {
        focus_mode: FocusMode::Sloppy,
        auto_raise: true,
        auto_raise_delay: ms(500),
        ..Config::default()
    });
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    let entered = PointerTarget::Window(id(1));
    policy.pointer_entered(SEAT, entered, Crossing::Motion, ms(1000));
    policy.take_scene_changed();

    policy.elapse(ms(1499));
    assert!(!policy.take_scene_changed());

    policy.elapse(ms(1500));
    assert!(policy.take_scene_changed());
}

#[test]
fn a_tile_preview_changes_what_the_scene_reads() {
    let mut policy = Policy::new(Config {
        allow_tiling: true,
        ..Config::default()
    });
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    let pointer = |x: f64, y: f64| Point::<f64, Logical>::from((x, y));
    assert!(policy.begin_move(SEAT, id(1), pointer(0., 400.)));
    policy.take_scene_changed();

    // The pointer stays where the move began, in the left side's zone: no window moves,
    // and none is told of the preview.
    policy.grab_motion(SEAT, pointer(0., 400.), false);

    assert!(policy.tile_preview(SEAT).is_some());
    assert!(policy.take_scene_changed());
}

#[test]
fn what_the_scene_doesnt_read_changes_nothing() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.take_scene_changed();

    policy.set_pointer(Point::from((10., 10.)));
    policy.set_primary_output(Some(OutputId(1)));

    assert!(!policy.take_scene_changed());
}
