//! Pointer and keyboard input: focus, click to focus and raise, and keys reaching the
//! focused client (22).

use camino_test_client::{ToplevelState, WindowId, format_input};
use insta::assert_snapshot;
use smithay::utils::{Logical, Point};

use crate::state::SEAT;

use super::fixture::{ClientId, Fixture, KEY_A};

/// A point `(dx, dy)` inside a window's geometry.
fn inside(
    f: &mut Fixture,
    id: ClientId,
    window: WindowId,
    dx: i32,
    dy: i32,
) -> Point<f64, Logical> {
    let policy_id = f.policy_id(id, window).unwrap();
    let geometry = f.state.camino.policy.geometry(policy_id).unwrap();
    (geometry.loc + Point::from((dx, dy))).to_f64()
}

#[test]
fn pointer_focus_follows_the_surface_under_the_pointer() {
    let (mut f, id) = Fixture::connected();
    f.client(id).bind_input();
    let a = f.map_window(id, (400, 300));
    let b = f.map_window(id, (400, 300));
    let (in_a, in_b) = (inside(&mut f, id, a, 10, 20), inside(&mut f, id, b, 30, 40));
    f.client(id).state.input.clear();

    f.pointer_move(in_a.x, in_a.y);
    f.pointer_move(in_b.x, in_b.y);
    f.pointer_move(1919., 1079.);
    f.roundtrip(id);

    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    pointer enters window 0 at 10, 20
    pointer leaves window 0
    pointer enters window 1 at 30, 40
    pointer leaves window 1
    ");
}

#[test]
fn a_click_focuses_and_raises_the_window() {
    let (mut f, id) = Fixture::connected();
    f.client(id).bind_input();
    // Too big to place side by side, so the second cascades over the first.
    let a = f.map_window(id, (1200, 900));
    let b = f.map_window(id, (1200, 900));
    let (pa, pb) = (f.policy_id(id, a).unwrap(), f.policy_id(id, b).unwrap());
    assert_eq!(f.state.camino.policy.stacking(0), [pa, pb]);
    f.client(id).state.input.clear();

    // A's bottom-right corner, which B doesn't cover.
    let point = inside(&mut f, id, a, 1199, 899);
    f.click(point.x, point.y);
    f.roundtrip(id);

    let policy = &f.state.camino.policy;
    assert_eq!(policy.focus(SEAT), Some(pa));
    assert_eq!(policy.stacking(0), [pb, pa]);
    // The pointer starts at the origin, over B.
    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    pointer leaves window 1
    pointer enters window 0 at 1199, 899
    keyboard leaves window 1
    keyboard enters window 0
    button 0x110 pressed
    button 0x110 released
    ");
    let mut activated = |w| {
        let configure = f.client(id).window(w).configures.last().unwrap().clone();
        configure.states.contains(&ToplevelState::Activated)
    };
    assert!(activated(a));
    assert!(!activated(b));
}

#[test]
fn a_new_window_takes_keyboard_focus() {
    let (mut f, id) = Fixture::connected();
    f.client(id).bind_input();
    f.roundtrip(id);

    f.map_window(id, (400, 300));
    f.map_window(id, (400, 300));
    f.roundtrip(id);

    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    keyboard enters window 0
    keyboard leaves window 0
    keyboard enters window 1
    ");
}

#[test]
fn unbound_keys_reach_the_focused_client() {
    let (mut f, id) = Fixture::connected();
    f.client(id).bind_input();
    f.map_window(id, (400, 300));
    f.client(id).state.input.clear();

    f.key(KEY_A, true);
    f.key(KEY_A, false);
    f.roundtrip(id);

    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    key 30 pressed
    key 30 released
    ");
}

#[test]
fn clicking_empty_space_keeps_focus() {
    let (mut f, id) = Fixture::connected();
    f.client(id).bind_input();
    let a = f.map_window(id, (400, 300));
    let pa = f.policy_id(id, a);

    f.click(1919., 1079.);
    f.roundtrip(id);

    assert_eq!(f.state.camino.policy.focus(SEAT), pa);
}

#[test]
fn a_locked_pointer_stays_put_over_its_window() {
    let (mut f, id) = Fixture::connected();
    f.client(id).bind_input();
    let window = f.map_window(id, (400, 300));
    let point = inside(&mut f, id, window, 50, 50);
    f.pointer_move(point.x, point.y);
    f.client(id).lock_pointer(window);
    f.roundtrip(id);
    assert!(f.client(id).state.pointer_locked);

    f.pointer_move_by(500., 500.);

    let pointer = f.state.camino.seat.get_pointer().unwrap();
    assert_eq!(pointer.current_location(), point);
}

#[test]
fn a_mouse_moves_the_pointer_within_the_outputs() {
    let (mut f, _) = Fixture::connected();
    f.pointer_move(100., 100.);

    f.pointer_move_by(5000., -50.);

    let pointer = f.state.camino.seat.get_pointer().unwrap();
    assert_eq!(pointer.current_location(), (1919., 50.).into());
}
