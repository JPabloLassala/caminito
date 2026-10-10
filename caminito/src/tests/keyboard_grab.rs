//! `begin-move` (Alt+F7) and `begin-resize` (Alt+F8), the keyboard grab of Marco's (156):
//! policy's tests work out the steps and the edges; these check that the bindings, the keys,
//! the pointer and its warps reach it, on both kinds of window. X11's
//! `_NET_WM_MOVERESIZE` directions 9 and 10 start the same grab, which
//! `window_properties.rs` checks.

use caminito_test_client::format_input;
use smithay::utils::{Logical, Point};

use super::fixture::{
    BTN_LEFT, ClientWindow, Fixture, KEY_DOWN, KEY_ENTER, KEY_ESC, KEY_F7, KEY_F8, KEY_LEFTALT,
    KEY_LEFTCTRL, KEY_RIGHT, Kind, WindowSpec, on_both_kinds,
};
use crate::state::SEAT;

/// Presses and releases `key`.
fn tap(f: &mut Fixture, key: u32) {
    f.key(key, true);
    f.key(key, false);
}

fn pointer(f: &Fixture) -> Point<f64, Logical> {
    f.state
        .caminito
        .seat
        .get_pointer()
        .unwrap()
        .current_location()
}

fn grabbed(f: &Fixture) -> bool {
    f.state.caminito.policy.keyboard_grab(SEAT).is_some()
}

/// The middle of a window's visible frame, where Marco warps the pointer for a move.
fn middle(f: &Fixture, window: ClientWindow) -> Point<f64, Logical> {
    let frame = f.visible_frame(window);
    Point::from((
        frame.loc.x + frame.size.w / 2,
        frame.loc.y + frame.size.h / 2,
    ))
    .to_f64()
}

on_both_kinds! {
    fn alt_f7_and_the_arrows_move_the_window_and_return_puts_it_down(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let window = f.map(id, WindowSpec::plain((400, 300)));
        let start = f.geometry(window).loc;
        f.pointer_move(10., 10.);

        f.chord(&[KEY_LEFTALT, KEY_F7]);
        assert!(grabbed(&f));
        assert_eq!(pointer(&f), middle(&f, window));

        tap(&mut f, KEY_RIGHT);
        tap(&mut f, KEY_DOWN);
        f.key(KEY_LEFTCTRL, true);
        tap(&mut f, KEY_RIGHT);
        f.key(KEY_LEFTCTRL, false);
        f.sync(window);
        assert_eq!(f.geometry(window).loc, start + Point::from((11, 10)));
        assert_eq!(pointer(&f), middle(&f, window));

        // The pointer moves it too, from where it was warped.
        f.pointer_move_by(20., 5.);
        f.sync(window);
        assert_eq!(f.geometry(window).loc, start + Point::from((31, 15)));

        tap(&mut f, KEY_ENTER);
        assert!(!grabbed(&f));
        f.pointer_move_by(100., 100.);
        f.sync(window);
        assert_eq!(f.geometry(window).loc, start + Point::from((31, 15)));
    }
}

on_both_kinds! {
    fn escape_puts_a_keyboard_move_back_where_it_began(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let window = f.map(id, WindowSpec::plain((400, 300)));
        let start = f.geometry(window).loc;
        f.chord(&[KEY_LEFTALT, KEY_F7]);
        tap(&mut f, KEY_RIGHT);
        f.pointer_move_by(50., 50.);

        tap(&mut f, KEY_ESC);
        f.sync(window);

        assert!(!grabbed(&f));
        assert_eq!(f.geometry(window).loc, start);
    }
}

on_both_kinds! {
    fn alt_f8_picks_an_edge_with_the_first_arrow_and_resizes_with_the_next(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let window = f.map(id, WindowSpec::plain((400, 300)));
        let start = f.geometry(window);

        f.chord(&[KEY_LEFTALT, KEY_F8]);
        assert!(grabbed(&f));
        assert_eq!(pointer(&f), middle(&f, window));

        tap(&mut f, KEY_RIGHT);
        let frame = f.visible_frame(window);
        let right_edge = Point::from((frame.loc.x + frame.size.w - 1, frame.loc.y + frame.size.h / 2));
        assert_eq!(pointer(&f), right_edge.to_f64());

        tap(&mut f, KEY_RIGHT);
        tap(&mut f, KEY_RIGHT);
        f.draw_at_asked_size(window, (420, 300));
        assert_eq!(f.geometry(window).loc, start.loc);
        assert_eq!(pointer(&f), right_edge.to_f64() + Point::from((20., 0.)));

        tap(&mut f, KEY_ENTER);
        assert!(!grabbed(&f));
    }
}

on_both_kinds! {
    fn escape_puts_a_keyboard_resize_back_as_it_began(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let window = f.map(id, WindowSpec::plain((400, 300)));
        f.chord(&[KEY_LEFTALT, KEY_F8]);
        tap(&mut f, KEY_DOWN);
        tap(&mut f, KEY_DOWN);
        f.draw_at_asked_size(window, (400, 310));

        tap(&mut f, KEY_ESC);

        assert!(!grabbed(&f));
        f.draw_at_asked_size(window, (400, 300));
    }
}

on_both_kinds! {
    fn a_click_puts_a_keyboard_move_down(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let window = f.map(id, WindowSpec::plain((400, 300)));
        let start = f.geometry(window).loc;
        f.chord(&[KEY_LEFTALT, KEY_F7]);
        f.pointer_move_by(30., 0.);

        f.pointer_button(BTN_LEFT, true);
        f.pointer_button(BTN_LEFT, false);

        assert!(!grabbed(&f));
        f.pointer_move_by(30., 0.);
        f.sync(window);
        assert_eq!(f.geometry(window).loc, start + Point::from((30, 0)));
    }
}

// Only an xdg client tells the test what keys it got.
#[test]
fn the_keys_of_a_keyboard_move_dont_reach_the_client() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    f.map(id, WindowSpec::plain((400, 300)));
    f.roundtrip(id);
    f.client(id).state.input.clear();

    f.chord(&[KEY_LEFTALT, KEY_F7]);
    tap(&mut f, KEY_RIGHT);
    tap(&mut f, KEY_ENTER);
    f.roundtrip(id);

    // Alt, which it saw pressed, and nothing of F7, the arrow or Return. The pointer, warped
    // onto the window, enters it once the grab ends.
    insta::assert_snapshot!(format_input(&f.client(id).state.input), @r"
    key 56 pressed
    key 56 released
    pointer enters window 0 at 200, 150
    ");
}
