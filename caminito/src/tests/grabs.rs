//! Interactive move and resize, from client requests and from Marco's Alt+drag and
//! Alt+middle-drag (22). Move and resize requests are xdg's; Alt+drag and Alt+middle-drag
//! run with an xdg toplevel and an XWayland window (45).

use caminito_test_client::{ResizeEdge, format_configures, format_input};
use insta::assert_snapshot;
use smithay::utils::{Point, Rectangle};

use super::fixture::{
    BTN_LEFT, BTN_MIDDLE, ClientWindow, Fixture, KEY_LEFTALT, Kind, WINDOW_COLOR, WindowSpec,
    on_both_kinds,
};
use crate::state::SEAT;

/// Presses the left button `(dx, dy)` inside an xdg toplevel, and returns the press's
/// serial as the client saw it.
fn press_inside(f: &mut Fixture, window: ClientWindow, dx: i32, dy: i32) -> u32 {
    let point = (f.geometry(window).loc + Point::from((dx, dy))).to_f64();
    f.pointer_move(point.x, point.y);
    f.pointer_button(BTN_LEFT, true);
    f.roundtrip(window.client());
    f.client(window.client())
        .state
        .last_press_serial
        .expect("the client saw the press")
}

#[test]
fn a_move_request_moves_the_window_with_the_pointer_until_release() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    let start = f.geometry(window);

    let serial = press_inside(&mut f, window, 50, 10);
    f.client(id).start_move(window.xdg(), serial);
    f.roundtrip(id);
    f.pointer_move(f64::from(start.loc.x + 150), f64::from(start.loc.y + 60));
    assert_eq!(f.geometry(window).loc, start.loc + Point::from((100, 50)));

    f.pointer_button(BTN_LEFT, false);
    f.pointer_move(0., 0.);
    assert_eq!(f.geometry(window).loc, start.loc + Point::from((100, 50)));
    assert_eq!(f.state.caminito.policy.grab(SEAT), None);
}

#[test]
fn a_move_request_without_a_held_button_is_refused() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));

    let serial = press_inside(&mut f, window, 50, 10);
    f.pointer_button(BTN_LEFT, false);
    f.client(id).start_move(window.xdg(), serial);
    f.roundtrip(id);

    assert_eq!(f.state.caminito.policy.grab(SEAT), None);
}

#[test]
fn a_resize_request_configures_the_size_under_the_pointer() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    let start = f.geometry(window);

    let serial = press_inside(&mut f, window, 399, 299);
    f.client(id)
        .start_resize(window.xdg(), serial, ResizeEdge::BottomRight);
    f.roundtrip(id);
    f.pointer_move(f64::from(start.loc.x + 499), f64::from(start.loc.y + 349));
    f.roundtrip(id);
    f.client(id)
        .ack_and_commit_buffer(window.xdg(), (500, 350), WINDOW_COLOR);
    f.pointer_button(BTN_LEFT, false);
    f.roundtrip(id);

    assert_eq!(
        f.geometry(window),
        Rectangle::new(start.loc, (500, 350).into())
    );
    assert_snapshot!(format_configures(
        &f.client(id).window(window.xdg()).configures
    ));
}

on_both_kinds! {
    fn alt_drag_moves_the_window(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let window = f.map(client, WindowSpec::plain((400, 300)));
        let start = f.geometry(window);
        let point = (start.loc + Point::from((50, 10))).to_f64();
        f.pointer_move(point.x, point.y);
        f.key(KEY_LEFTALT, true);

        f.pointer_button(BTN_LEFT, true);
        f.pointer_move(point.x + 100., point.y + 50.);
        f.pointer_button(BTN_LEFT, false);
        f.key(KEY_LEFTALT, false);
        f.sync(window);

        assert_eq!(f.geometry(window).loc, start.loc + Point::from((100, 50)));
    }
}

#[test]
fn the_client_sees_nothing_of_an_alt_drag_but_the_alt_release() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    let start = f.geometry(window);
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

    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    pointer leaves window 0
    pointer enters window 0 at 50, 10
    key 56 released
    ");
}

on_both_kinds! {
    fn alt_middle_drag_resizes_from_the_nearest_corner(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let window = f.map(client, WindowSpec::plain((400, 300)));
        // Alt+dragged into the middle of the output first, so that the resize doesn't end
        // near an edge that resists it (157).
        let grip = (f.geometry(window).loc + Point::from((200, 150))).to_f64();
        f.pointer_move(grip.x, grip.y);
        f.key(KEY_LEFTALT, true);
        f.pointer_button(BTN_LEFT, true);
        f.pointer_move(grip.x + 400., grip.y + 300.);
        f.pointer_button(BTN_LEFT, false);
        f.key(KEY_LEFTALT, false);
        f.sync(window);
        let start = f.geometry(window);
        // Nearer the top-left corner than any other.
        let point = (start.loc + Point::from((20, 30))).to_f64();
        f.pointer_move(point.x, point.y);
        f.key(KEY_LEFTALT, true);

        f.pointer_button(BTN_MIDDLE, true);
        f.pointer_move(point.x - 100., point.y - 50.);
        f.draw_at_asked_size(window, (500, 350));
        f.pointer_button(BTN_MIDDLE, false);
        f.key(KEY_LEFTALT, false);
        f.sync(window);

        // The top-left corner followed the pointer; the bottom-right stayed put.
        assert_eq!(
            f.geometry(window),
            Rectangle::new(start.loc - Point::from((100, 50)), (500, 350).into())
        );
    }
}

#[test]
fn an_x_window_that_takes_sync_requests_keeps_its_size_until_it_has_drawn_the_new_one() {
    let (mut f, client) = Fixture::connected(Kind::X11);
    let window = f.map(
        client,
        WindowSpec {
            sync_request: true,
            ..WindowSpec::plain((400, 300))
        },
    );
    let start = f.geometry(window);
    // Nearer the bottom-right corner than any other.
    let point = (start.loc + Point::from((380, 270))).to_f64();
    f.pointer_move(point.x, point.y);
    f.key(KEY_LEFTALT, true);
    f.pointer_button(BTN_MIDDLE, true);
    f.pointer_move(point.x + 100., point.y + 50.);
    f.sync(window);

    // X has resized the window, which hasn't drawn at its new size yet: Caminito shows it,
    // and its frame, at the size of what it last drew.
    let x = window.x11();
    assert_eq!(
        f.x11_call(client, move |x11| x11.size(x)).unwrap(),
        (500, 350)
    );
    assert_eq!(f.geometry(window), start);

    let acked = f.x11_call(client, move |x11| x11.ack_sync_requests(x));
    assert_eq!(acked.unwrap(), 1);
    f.dispatch_until(|f| f.geometry(window).size == (500, 350).into());
    f.pointer_button(BTN_MIDDLE, false);
    f.key(KEY_LEFTALT, false);
    f.sync(window);

    assert_eq!(
        f.geometry(window),
        Rectangle::new(start.loc, (500, 350).into())
    );
}

#[test]
fn an_x_window_released_before_it_has_drawn_its_new_size_still_waits_for_it() {
    let (mut f, client) = Fixture::connected(Kind::X11);
    let window = f.map(
        client,
        WindowSpec {
            sync_request: true,
            ..WindowSpec::plain((400, 300))
        },
    );
    let start = f.geometry(window);
    let point = (start.loc + Point::from((380, 270))).to_f64();
    f.pointer_move(point.x, point.y);
    f.key(KEY_LEFTALT, true);
    f.pointer_button(BTN_MIDDLE, true);
    f.pointer_move(point.x + 100., point.y + 50.);
    f.pointer_button(BTN_MIDDLE, false);
    f.key(KEY_LEFTALT, false);
    f.sync(window);

    assert_eq!(f.geometry(window), start);

    let x = window.x11();
    f.x11_call(client, move |x11| x11.ack_sync_requests(x))
        .unwrap();
    f.dispatch_until(|f| f.geometry(window).size == (500, 350).into());
}

on_both_kinds! {
    fn keys_held_when_the_keyboard_is_lost_are_released(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let window = f.map(client, WindowSpec::plain((400, 300)));
        let start = f.geometry(window);
        f.key(KEY_LEFTALT, true);

        // The host takes the keyboard while Alt is down, and gives it back after it's up.
        f.state.release_pressed_keys();
        let point = (start.loc + Point::from((50, 10))).to_f64();
        f.pointer_move(point.x, point.y);
        f.pointer_button(BTN_LEFT, true);
        f.pointer_move(point.x + 100., point.y + 50.);
        f.pointer_button(BTN_LEFT, false);

        assert_eq!(f.geometry(window).loc, start.loc);
    }
}
