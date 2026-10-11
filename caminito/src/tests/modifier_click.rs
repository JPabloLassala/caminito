//! The clicks on a window with `mouse-button-modifier`'s modifiers held (162): the left
//! button moves it, and the middle and right ones resize it and open its menu, as
//! `resize-with-right-button` says which. Each runs with an xdg toplevel and an XWayland
//! window (45), but what a client sees of a press, which the xdg client records.

use smithay::utils::{Logical, Point, Rectangle};

use super::fixture::{
    BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, ClientId, ClientWindow, Fixture, KEY_ESC, KEY_LEFTALT,
    KEY_LEFTCTRL, KEY_LEFTMETA, KEY_LEFTSHIFT, Kind, WindowSpec, on_both_kinds,
};
use crate::settings::Value;
use crate::settings::translate::MARCO_GENERAL;
use crate::state::SEAT;

/// What came of a press and a drag.
#[derive(Debug, PartialEq)]
enum Did {
    Moved,
    Resized,
    /// The window menu opened, with its corner here.
    Menu(Point<i32, Logical>),
    Nothing,
    /// The window went somewhere else.
    Other(Rectangle<i32, Logical>),
}

fn modifier(value: &str) -> (&'static str, String, Value) {
    (
        MARCO_GENERAL,
        "mouse-button-modifier".into(),
        Value::Str(value.into()),
    )
}

fn resize_with_right_button(on: bool) -> (&'static str, String, Value) {
    (
        MARCO_GENERAL,
        "resize-with-right-button".into(),
        Value::Bool(on),
    )
}

/// One 1920 × 1080 output and a 400 × 300 window from a client of `kind`, under a Caminito
/// with `settings` set from the start.
fn with_window(
    kind: Kind,
    settings: Vec<(&'static str, String, Value)>,
) -> (Fixture, ClientId, ClientWindow) {
    let mut f = Fixture::with_settings(settings);
    f.add_output((1920, 1080), 1.0);
    let client = f.connect(kind);
    if kind == Kind::Xdg {
        f.client(client).bind_input();
    }
    let window = f.map(client, WindowSpec::plain((400, 300)));
    (f, client, window)
}

/// Where the drags start: near the window's bottom right corner, which a resize moves.
const GRIP: (i32, i32) = (380, 270);

/// Holds `keys`, presses `button` near the window's bottom right corner, drags it by
/// (100, 50) and lets go of them all. Says what that did to the window, and closes the
/// window menu if it opened it.
fn drag(f: &mut Fixture, window: ClientWindow, keys: &[u32], button: u32) -> Did {
    let start = f.geometry(window);
    let point = (start.loc + Point::from(GRIP)).to_f64();
    f.pointer_move(point.x, point.y);
    for key in keys {
        f.key(*key, true);
    }
    f.pointer_button(button, true);
    f.pointer_move(point.x + 100., point.y + 50.);
    if f.state.caminito.policy.grab_edges(SEAT).is_some() {
        f.draw_at_asked_size(window, (start.size.w + 100, start.size.h + 50));
    }
    f.pointer_button(button, false);
    for key in keys.iter().rev() {
        f.key(*key, false);
    }
    f.sync(window);

    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    let menu = &dump["window_menu"];
    let end = f.geometry(window);
    if !menu.is_null() {
        assert_eq!(menu["window"], f.pid(window).0);
        assert_eq!(end, start, "the menu's press moved the window");
        let corner = |name: &str| menu["rect"][name].as_i64().unwrap() as i32;
        f.key(KEY_ESC, true);
        f.key(KEY_ESC, false);
        return Did::Menu((corner("x"), corner("y")).into());
    }
    let moved = Rectangle::new(start.loc + Point::from((100, 50)), start.size);
    let resized = Rectangle::new(start.loc, start.size + (100, 50).into());
    match end {
        end if end == start => Did::Nothing,
        end if end == moved => Did::Moved,
        end if end == resized => Did::Resized,
        end => Did::Other(end),
    }
}

/// Where the pointer is when `drag` presses its button.
fn grip(f: &Fixture, window: ClientWindow) -> Point<i32, Logical> {
    f.geometry(window).loc + Point::from(GRIP)
}

on_both_kinds! {
    fn with_marcos_defaults_alt_moves_with_the_left_button_resizes_with_the_right_and_opens_the_menu_with_the_middle(kind: Kind) {
        let (mut f, _, window) = with_window(kind, Vec::new());

        assert_eq!(drag(&mut f, window, &[KEY_LEFTALT], BTN_LEFT), Did::Moved);
        assert_eq!(drag(&mut f, window, &[KEY_LEFTALT], BTN_RIGHT), Did::Resized);
        // At the pointer, as Marco opens it.
        let at = grip(&f, window);
        assert_eq!(drag(&mut f, window, &[KEY_LEFTALT], BTN_MIDDLE), Did::Menu(at));
    }
}

on_both_kinds! {
    fn without_resize_with_right_button_the_middle_button_resizes_and_the_right_one_opens_the_menu(kind: Kind) {
        let (mut f, _, window) = with_window(kind, vec![resize_with_right_button(false)]);

        assert_eq!(drag(&mut f, window, &[KEY_LEFTALT], BTN_LEFT), Did::Moved);
        assert_eq!(drag(&mut f, window, &[KEY_LEFTALT], BTN_MIDDLE), Did::Resized);
        let at = grip(&f, window);
        assert_eq!(drag(&mut f, window, &[KEY_LEFTALT], BTN_RIGHT), Did::Menu(at));
    }
}

on_both_kinds! {
    fn the_modifier_set_to_super_takes_the_three_buttons_from_alt(kind: Kind) {
        let (mut f, _, window) = with_window(kind, vec![modifier("<Super>")]);

        for button in [BTN_LEFT, BTN_MIDDLE, BTN_RIGHT] {
            assert_eq!(drag(&mut f, window, &[KEY_LEFTALT], button), Did::Nothing);
        }
        assert_eq!(drag(&mut f, window, &[KEY_LEFTMETA], BTN_LEFT), Did::Moved);
        assert_eq!(drag(&mut f, window, &[KEY_LEFTMETA], BTN_RIGHT), Did::Resized);
        let at = grip(&f, window);
        assert_eq!(drag(&mut f, window, &[KEY_LEFTMETA], BTN_MIDDLE), Did::Menu(at));
    }
}

on_both_kinds! {
    fn a_modifier_of_two_keys_needs_both_and_no_other(kind: Kind) {
        let (mut f, _, window) = with_window(kind, vec![modifier("<Control><Alt>")]);
        let both = [KEY_LEFTCTRL, KEY_LEFTALT];

        assert_eq!(drag(&mut f, window, &[KEY_LEFTALT], BTN_LEFT), Did::Nothing);
        assert_eq!(drag(&mut f, window, &[KEY_LEFTCTRL], BTN_LEFT), Did::Nothing);
        assert_eq!(drag(&mut f, window, &both, BTN_LEFT), Did::Moved);
        assert_eq!(drag(&mut f, window, &both, BTN_RIGHT), Did::Resized);
        let with_super = [KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFTMETA];
        assert_eq!(drag(&mut f, window, &with_super, BTN_LEFT), Did::Nothing);
    }
}

on_both_kinds! {
    fn shift_with_the_modifier_still_moves_and_does_nothing_else(kind: Kind) {
        let (mut f, _, window) = with_window(kind, Vec::new());
        let keys = [KEY_LEFTALT, KEY_LEFTSHIFT];

        // Marco grabs the left button with Shift too, for a move that snaps: the window
        // goes to the output's edges, not where the pointer does.
        let start = f.geometry(window);
        let Did::Other(snapped) = drag(&mut f, window, &keys, BTN_LEFT) else {
            panic!("Alt+Shift and the left button didn't move the window with a snap");
        };
        assert_eq!(snapped.size, start.size);
        assert_eq!(drag(&mut f, window, &keys, BTN_RIGHT), Did::Nothing);
        assert_eq!(drag(&mut f, window, &keys, BTN_MIDDLE), Did::Nothing);
    }
}

on_both_kinds! {
    fn disabled_and_the_empty_string_turn_the_clicks_off(kind: Kind) {
        for value in ["disabled", ""] {
            let (mut f, _, window) = with_window(kind, vec![modifier(value)]);

            for button in [BTN_LEFT, BTN_MIDDLE, BTN_RIGHT] {
                assert_eq!(drag(&mut f, window, &[KEY_LEFTALT], button), Did::Nothing);
                assert_eq!(drag(&mut f, window, &[], button), Did::Nothing);
            }
        }
    }
}

on_both_kinds! {
    fn a_modifier_that_doesnt_parse_is_alt(kind: Kind) {
        let (mut f, _, window) = with_window(kind, vec![modifier("garbage")]);

        assert_eq!(drag(&mut f, window, &[KEY_LEFTALT], BTN_LEFT), Did::Moved);
    }
}

on_both_kinds! {
    fn a_change_of_either_key_applies_at_once(kind: Kind) {
        let (mut f, _, window) = with_window(kind, Vec::new());
        assert_eq!(drag(&mut f, window, &[KEY_LEFTALT], BTN_LEFT), Did::Moved);

        f.set_setting(MARCO_GENERAL, "mouse-button-modifier", Value::Str("<Super>".into()));
        f.dispatch_until(|f| {
            f.state.caminito.settings.modifier_click.modifier.to_string() == "<Super>"
        });
        assert_eq!(drag(&mut f, window, &[KEY_LEFTALT], BTN_LEFT), Did::Nothing);
        assert_eq!(drag(&mut f, window, &[KEY_LEFTMETA], BTN_LEFT), Did::Moved);
        assert_eq!(drag(&mut f, window, &[KEY_LEFTMETA], BTN_RIGHT), Did::Resized);

        f.set_setting(MARCO_GENERAL, "resize-with-right-button", Value::Bool(false));
        f.dispatch_until(|f| !f.state.caminito.settings.modifier_click.resize_with_right_button);
        assert_eq!(drag(&mut f, window, &[KEY_LEFTMETA], BTN_MIDDLE), Did::Resized);
        let at = grip(&f, window);
        assert_eq!(drag(&mut f, window, &[KEY_LEFTMETA], BTN_RIGHT), Did::Menu(at));
    }
}

/// Whether the xdg client saw a button pressed since it last forgot its input.
fn saw_a_press(f: &mut Fixture, client: ClientId) -> bool {
    f.roundtrip(client);
    f.client(client).state.last_press_serial.take().is_some()
}

#[test]
fn the_client_sees_no_press_of_the_three_buttons_with_the_modifier_held() {
    for settings in [Vec::new(), vec![modifier("<Super>")]] {
        let key = if settings.is_empty() {
            KEY_LEFTALT
        } else {
            KEY_LEFTMETA
        };
        let (mut f, client, window) = with_window(Kind::Xdg, settings);
        for button in [BTN_LEFT, BTN_MIDDLE, BTN_RIGHT] {
            drag(&mut f, window, &[key], button);
            assert!(!saw_a_press(&mut f, client), "button {button:#x}");
        }
        // And the same press without it is the client's.
        drag(&mut f, window, &[], BTN_RIGHT);
        assert!(saw_a_press(&mut f, client));
    }
}

#[test]
fn the_client_sees_no_press_that_would_resize_a_window_that_wont() {
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.0);
    let client = f.connect(Kind::Xdg);
    f.client(client).bind_input();
    let spec = WindowSpec {
        fixed_size: true,
        ..WindowSpec::plain((400, 300))
    };
    let window = f.map(client, spec);

    // Marco's grab of the button has the press whether the window resizes or not.
    assert_eq!(
        drag(&mut f, window, &[KEY_LEFTALT], BTN_RIGHT),
        Did::Nothing
    );
    assert!(!saw_a_press(&mut f, client));
}

#[test]
fn with_the_modifier_disabled_the_press_is_the_clients() {
    let (mut f, client, window) = with_window(Kind::Xdg, vec![modifier("disabled")]);

    for button in [BTN_LEFT, BTN_MIDDLE, BTN_RIGHT] {
        assert_eq!(drag(&mut f, window, &[KEY_LEFTALT], button), Did::Nothing);
        assert!(saw_a_press(&mut f, client), "button {button:#x}");
    }
}
