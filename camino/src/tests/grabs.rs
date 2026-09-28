//! Interactive move and resize, from client requests and from Marco's Alt+drag and
//! Alt+middle-drag (22).

use camino_test_client::{ResizeEdge, WindowId, format_configures, format_input};
use insta::assert_snapshot;
use smithay::utils::{Logical, Point, Rectangle};

use super::fixture::{BTN_LEFT, BTN_MIDDLE, ClientId, Fixture, KEY_LEFTALT, WINDOW_COLOR};
use crate::state::SEAT;

fn geometry(f: &mut Fixture, id: ClientId, window: WindowId) -> Rectangle<i32, Logical> {
    let policy_id = f.policy_id(id, window).unwrap();
    f.state.camino.policy.geometry(policy_id).unwrap()
}

/// Presses the left button `(dx, dy)` inside the window, and returns the press's serial as
/// the client saw it.
fn press_inside(f: &mut Fixture, id: ClientId, window: WindowId, dx: i32, dy: i32) -> u32 {
    let point = (geometry(f, id, window).loc + Point::from((dx, dy))).to_f64();
    f.pointer_move(point.x, point.y);
    f.pointer_button(BTN_LEFT, true);
    f.roundtrip(id);
    f.client(id)
        .state
        .last_press_serial
        .expect("the client saw the press")
}

#[test]
fn a_move_request_moves_the_window_with_the_pointer_until_release() {
    let (mut f, id) = Fixture::connected();
    f.client(id).bind_input();
    let window = f.map_window(id, (400, 300));
    let start = geometry(&mut f, id, window);

    let serial = press_inside(&mut f, id, window, 50, 10);
    f.client(id).start_move(window, serial);
    f.roundtrip(id);
    f.pointer_move(f64::from(start.loc.x + 150), f64::from(start.loc.y + 60));
    assert_eq!(
        geometry(&mut f, id, window).loc,
        start.loc + Point::from((100, 50))
    );

    f.pointer_button(BTN_LEFT, false);
    f.pointer_move(0., 0.);
    assert_eq!(
        geometry(&mut f, id, window).loc,
        start.loc + Point::from((100, 50))
    );
    assert_eq!(f.state.camino.policy.grab(SEAT), None);
}

#[test]
fn a_move_request_without_a_held_button_is_refused() {
    let (mut f, id) = Fixture::connected();
    f.client(id).bind_input();
    let window = f.map_window(id, (400, 300));

    let serial = press_inside(&mut f, id, window, 50, 10);
    f.pointer_button(BTN_LEFT, false);
    f.client(id).start_move(window, serial);
    f.roundtrip(id);

    assert_eq!(f.state.camino.policy.grab(SEAT), None);
}

#[test]
fn a_resize_request_configures_the_size_under_the_pointer() {
    let (mut f, id) = Fixture::connected();
    f.client(id).bind_input();
    let window = f.map_window(id, (400, 300));
    let start = geometry(&mut f, id, window);

    let serial = press_inside(&mut f, id, window, 399, 299);
    f.client(id)
        .start_resize(window, serial, ResizeEdge::BottomRight);
    f.roundtrip(id);
    f.pointer_move(f64::from(start.loc.x + 499), f64::from(start.loc.y + 349));
    f.roundtrip(id);
    f.client(id)
        .ack_and_commit_buffer(window, (500, 350), WINDOW_COLOR);
    f.pointer_button(BTN_LEFT, false);
    f.roundtrip(id);

    assert_eq!(
        geometry(&mut f, id, window),
        Rectangle::new(start.loc, (500, 350).into())
    );
    assert_snapshot!(format_configures(&f.client(id).window(window).configures));
}

#[test]
fn alt_drag_moves_the_window_without_the_client_seeing_the_button() {
    let (mut f, id) = Fixture::connected();
    f.client(id).bind_input();
    let window = f.map_window(id, (400, 300));
    let start = geometry(&mut f, id, window);
    let point = (start.loc + Point::from((50, 10))).to_f64();
    f.pointer_move(point.x, point.y);
    f.key(KEY_LEFTALT, true);
    f.roundtrip(id);
    f.client(id).state.input.clear();

    f.pointer_button(BTN_LEFT, true);
    f.pointer_move(point.x + 100., point.y + 50.);
    f.pointer_button(BTN_LEFT, false);
    f.key(KEY_LEFTALT, false);
    f.roundtrip(id);

    assert_eq!(
        geometry(&mut f, id, window).loc,
        start.loc + Point::from((100, 50))
    );
    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    pointer leaves window 0
    pointer enters window 0 at 50, 10
    key 56 released
    ");
}

#[test]
fn alt_middle_drag_resizes_from_the_nearest_corner() {
    let (mut f, id) = Fixture::connected();
    f.client(id).bind_input();
    let window = f.map_window(id, (400, 300));
    let start = geometry(&mut f, id, window);
    // Nearer the top-left corner than any other.
    let point = (start.loc + Point::from((20, 30))).to_f64();
    f.pointer_move(point.x, point.y);
    f.key(KEY_LEFTALT, true);

    f.pointer_button(BTN_MIDDLE, true);
    f.pointer_move(point.x - 100., point.y - 50.);
    f.roundtrip(id);
    f.client(id)
        .ack_and_commit_buffer(window, (500, 350), WINDOW_COLOR);
    f.pointer_button(BTN_MIDDLE, false);
    f.key(KEY_LEFTALT, false);
    f.roundtrip(id);

    // The top-left corner followed the pointer; the bottom-right stayed put.
    assert_eq!(
        geometry(&mut f, id, window),
        Rectangle::new(start.loc - Point::from((100, 50)), (500, 350).into())
    );
}

#[test]
fn keys_held_when_the_keyboard_is_lost_are_released() {
    let (mut f, id) = Fixture::connected();
    f.client(id).bind_input();
    let window = f.map_window(id, (400, 300));
    let start = geometry(&mut f, id, window);
    f.key(KEY_LEFTALT, true);

    // The host takes the keyboard while Alt is down, and gives it back after it's up.
    f.state.release_pressed_keys();
    let point = (start.loc + Point::from((50, 10))).to_f64();
    f.pointer_move(point.x, point.y);
    f.pointer_button(BTN_LEFT, true);
    f.pointer_move(point.x + 100., point.y + 50.);
    f.pointer_button(BTN_LEFT, false);

    assert_eq!(geometry(&mut f, id, window).loc, start.loc);
}
