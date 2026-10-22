//! Key remaps (CMN-186, ADR 0038): a combination a rule takes reaches the focused client as
//! the rule's target, with the target's modifiers alone, or doesn't reach it at all. A
//! target of several steps is sent as each in turn, the last held while the trigger is
//! (CMN-187). A Remap group's rule replaces All applications' for its app ids, a System
//! shortcut is never remapped and runs as a target, AltGr is never Alt, and no remap
//! applies on the lock screen or during an Interaction.
//!
//! The Wayland tests read the keys a client got with the modifiers it had for each, in
//! xkb's masks for the test keymap: Shift is 1, Ctrl 4, Alt 8 and AltGr 128. The modifiers a
//! key changes are sent before it, so Alt's press is seen with Alt and its release without.
//! The stage acts on the focus's app id and the keys, the same for both kinds of window, so
//! only one test maps an XWayland window, to show that its client, which gets its keys from
//! Caminito as a Wayland client does, gets the remaps too.
//!
//! A modifier that starts a rule for the focus is held back from the client (CMN-188), so a
//! client gets no Alt for a remapped Alt+T, and gets it before a key no rule takes.

use std::time::Duration;

use smithay::utils::Point;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::KeyButMask;

use super::accelerators::Caller;
use super::fixture::{
    BTN_LEFT, ClientId, ClientWindow, Fixture, KEY_BACKSPACE, KEY_C, KEY_F, KEY_H, KEY_HOME,
    KEY_LEFTALT, KEY_LEFTCTRL, KEY_LEFTSHIFT, KEY_M, KEY_Q, KEY_RIGHTALT, KEY_T, KEY_TAB, KEY_Z,
    Kind, WindowSpec,
};
use super::lock::lock_with_a_surface;
use crate::settings::Value;
use crate::settings::translate::{CAMINITO, KEYBOARD_XKB, RemapGroupValue};
use crate::state::SEAT;
use caminito_policy::Interaction;

const SHIFT: u32 = 1;
const CTRL: u32 = 4;
const ALT: u32 = 8;
const ALTGR: u32 = 128;

/// A Remap group of app ids, its rules as `("Alt+T", &["Ctrl+T"])`, or none at all.
fn group(app_ids: &[&str], no_remaps: bool, rules: &[(&str, &[&str])]) -> RemapGroupValue {
    let rules = rules
        .iter()
        .map(|(trigger, steps)| {
            let steps = steps.iter().map(|step| (*step).to_owned()).collect();
            ((*trigger).to_owned(), steps)
        })
        .collect();
    let app_ids = app_ids.iter().map(|id| (*id).to_owned()).collect();
    (String::new(), app_ids, no_remaps, rules)
}

/// All applications' group with `rules`.
fn all(rules: &[(&str, &[&str])]) -> RemapGroupValue {
    group(&[], false, rules)
}

/// `key-remaps` set to `groups`, All applications' first.
fn key_remaps(groups: Vec<RemapGroupValue>) -> (&'static str, String, Value) {
    (CAMINITO, "key-remaps".into(), Value::RemapGroups(groups))
}

/// Caminito with `settings` and one output.
fn caminito(settings: Vec<(&'static str, String, Value)>) -> Fixture {
    let mut f = Fixture::with_settings(settings);
    f.add_output((1920, 1080), 1.0);
    f.dispatch();
    f
}

/// A Wayland client that listens to the keyboard.
fn listening(f: &mut Fixture) -> ClientId {
    let id = f.connect(Kind::Xdg);
    f.client(id).bind_input();
    f.roundtrip(id);
    id
}

/// Maps a window of `client` with `app_id`, which gets the keyboard, and forgets the keys
/// the client got so far.
fn focused(f: &mut Fixture, client: ClientId, app_id: Option<&'static str>) -> ClientWindow {
    let window = f.map(
        client,
        WindowSpec {
            app_id,
            ..WindowSpec::plain((400, 300))
        },
    );
    f.roundtrip(client);
    assert!(f.has_keyboard(window));
    f.client(client).state.keys.clear();
    window
}

/// The keys `client` got since the last call, as `(key, pressed, modifiers)`.
fn keys(f: &mut Fixture, client: ClientId) -> Vec<(u32, bool, u32)> {
    f.roundtrip(client);
    std::mem::take(&mut f.client(client).state.keys)
}

/// What a client gets for `KEY_T` with `modifier` held, when it isn't remapped.
fn unchanged(modifier: u32, mask: u32) -> Vec<(u32, bool, u32)> {
    vec![
        (modifier, true, mask),
        (KEY_T, true, mask),
        (KEY_T, false, mask),
        (modifier, false, 0),
    ]
}

#[test]
fn alt_t_reaches_a_client_as_ctrl_t_with_no_alt_and_its_own_ctrl_t_is_unchanged() {
    let mut f = caminito(vec![key_remaps(vec![all(&[("Alt+T", &["Ctrl+T"])])])]);
    let client = listening(&mut f);
    focused(&mut f, client, Some("org.mate.Pluma"));

    // Alt, which starts the rule, is held back from the client (CMN-188).
    f.chord(&[KEY_LEFTALT, KEY_T]);
    assert_eq!(
        keys(&mut f, client),
        [(KEY_T, true, CTRL), (KEY_T, false, CTRL)]
    );

    f.chord(&[KEY_LEFTCTRL, KEY_T]);
    assert_eq!(keys(&mut f, client), unchanged(KEY_LEFTCTRL, CTRL));
}

#[test]
fn a_remap_groups_rule_replaces_all_applications_for_its_app_ids_alone() {
    let mut f = caminito(vec![key_remaps(vec![
        all(&[("Alt+C", &["Ctrl+C"])]),
        group(&["kitty"], false, &[("Alt+C", &["Ctrl+Shift+C"])]),
    ])]);
    let client = listening(&mut f);

    focused(&mut f, client, Some("kitty"));
    f.chord(&[KEY_LEFTALT, KEY_C]);
    assert_eq!(
        keys(&mut f, client),
        [(KEY_C, true, CTRL | SHIFT), (KEY_C, false, CTRL | SHIFT)]
    );

    focused(&mut f, client, Some("firefox"));
    f.chord(&[KEY_LEFTALT, KEY_C]);
    assert_eq!(
        keys(&mut f, client),
        [(KEY_C, true, CTRL), (KEY_C, false, CTRL)]
    );
}

#[test]
fn a_group_with_no_remaps_gets_every_combination_unchanged() {
    let mut f = caminito(vec![key_remaps(vec![
        all(&[("Alt+T", &["Ctrl+T"])]),
        group(&["org.remmina.Remmina"], true, &[]),
    ])]);
    let client = listening(&mut f);
    focused(&mut f, client, Some("org.remmina.Remmina"));

    f.chord(&[KEY_LEFTALT, KEY_T]);
    assert_eq!(keys(&mut f, client), unchanged(KEY_LEFTALT, ALT));
}

#[test]
fn a_focus_with_no_app_id_gets_all_applications_remaps() {
    let mut f = caminito(vec![key_remaps(vec![
        all(&[("Alt+T", &["Ctrl+T"])]),
        group(&["kitty"], true, &[]),
    ])]);
    let client = listening(&mut f);
    focused(&mut f, client, None);

    f.chord(&[KEY_LEFTALT, KEY_T]);
    assert_eq!(keys(&mut f, client)[0], (KEY_T, true, CTRL));
}

#[test]
fn a_rule_to_nothing_swallows_the_combination() {
    let mut f = caminito(vec![key_remaps(vec![all(&[("Alt+H", &[])])])]);
    let client = listening(&mut f);
    focused(&mut f, client, None);

    f.chord(&[KEY_LEFTALT, KEY_H]);
    assert_eq!(keys(&mut f, client), []);
}

/// Caminito with Alt+BackSpace sent as Shift+Home then BackSpace, and a client of no app id
/// with the keyboard.
fn delete_to_line_start() -> (Fixture, ClientId) {
    let mut f = caminito(vec![key_remaps(vec![all(&[(
        "Alt+BackSpace",
        &["Shift+Home", "BackSpace"],
    )])])]);
    let client = listening(&mut f);
    focused(&mut f, client, None);
    (f, client)
}

#[test]
fn a_sequence_sends_each_step_with_its_own_modifiers_and_holds_the_last_while_the_trigger_is() {
    let (mut f, client) = delete_to_line_start();

    f.key(KEY_LEFTALT, true);
    f.key(KEY_BACKSPACE, true);
    assert_eq!(
        keys(&mut f, client),
        [
            (KEY_HOME, true, SHIFT),
            (KEY_HOME, false, SHIFT),
            (KEY_BACKSPACE, true, 0),
        ]
    );

    f.key(KEY_BACKSPACE, false);
    assert_eq!(keys(&mut f, client), [(KEY_BACKSPACE, false, 0)]);

    // Alt is still held back, and a key no rule takes gets it first (CMN-188).
    f.key(KEY_T, true);
    f.key(KEY_T, false);
    f.key(KEY_LEFTALT, false);
    assert_eq!(
        keys(&mut f, client),
        [
            (KEY_LEFTALT, true, ALT),
            (KEY_T, true, ALT),
            (KEY_T, false, ALT),
            (KEY_LEFTALT, false, 0)
        ]
    );
}

#[test]
fn a_held_trigger_leaves_the_last_step_for_the_client_to_repeat_and_sends_nothing_else() {
    let (mut f, client) = delete_to_line_start();
    f.key(KEY_LEFTALT, true);
    f.key(KEY_BACKSPACE, true);
    keys(&mut f, client);

    // The client repeats the one key it holds, at its own rate: Caminito sends no more.
    // Two seconds are past the keyboard's repeat delay, 500 ms, and dozens of repeats at
    // its rate, 30 a second, so a repeat Caminito sent would have come.
    f.advance_clock(Duration::from_secs(2));
    f.dispatch();
    assert_eq!(keys(&mut f, client), []);

    f.key(KEY_BACKSPACE, false);
    assert_eq!(keys(&mut f, client), [(KEY_BACKSPACE, false, 0)]);
}

#[test]
fn the_trigger_pressed_twice_with_alt_held_sends_the_sequence_twice() {
    let (mut f, client) = delete_to_line_start();
    let sequence = [
        (KEY_HOME, true, SHIFT),
        (KEY_HOME, false, SHIFT),
        (KEY_BACKSPACE, true, 0),
        (KEY_BACKSPACE, false, 0),
    ];

    f.key(KEY_LEFTALT, true);
    f.key(KEY_BACKSPACE, true);
    f.key(KEY_BACKSPACE, false);
    assert_eq!(keys(&mut f, client), sequence);
    f.key(KEY_BACKSPACE, true);
    f.key(KEY_BACKSPACE, false);
    assert_eq!(keys(&mut f, client), sequence);
}

#[test]
fn another_key_pressed_while_the_trigger_is_held_lets_go_of_the_last_step_first() {
    let (mut f, client) = delete_to_line_start();
    f.key(KEY_LEFTALT, true);
    f.key(KEY_BACKSPACE, true);
    keys(&mut f, client);

    // The Alt held back (CMN-188) comes before T.
    f.key(KEY_T, true);
    assert_eq!(
        keys(&mut f, client),
        [
            (KEY_BACKSPACE, false, 0),
            (KEY_LEFTALT, true, ALT),
            (KEY_T, true, ALT)
        ]
    );

    // The trigger's release has nothing left to let go of.
    f.key(KEY_BACKSPACE, false);
    assert_eq!(keys(&mut f, client), []);
}

#[test]
fn alt_released_before_the_trigger_lets_go_of_the_last_step_with_its_own_modifiers() {
    let mut f = caminito(vec![key_remaps(vec![all(&[("Alt+T", &["Ctrl+T"])])])]);
    let client = listening(&mut f);
    focused(&mut f, client, None);
    f.key(KEY_LEFTALT, true);
    f.key(KEY_T, true);
    keys(&mut f, client);

    // The client would otherwise repeat T with no Ctrl. It never saw the Alt held back
    // (CMN-188), so it gets no release of it.
    f.key(KEY_LEFTALT, false);
    assert_eq!(keys(&mut f, client), [(KEY_T, false, CTRL)]);

    f.key(KEY_T, false);
    assert_eq!(keys(&mut f, client), []);
}

#[test]
fn a_step_that_is_a_system_shortcut_runs_it_in_its_place() {
    let mut f = caminito(vec![key_remaps(vec![all(&[(
        "Alt+Q",
        &["Ctrl+T", "Alt+F4", "Ctrl+H"],
    )])])]);
    let client = listening(&mut f);
    let window = focused(&mut f, client, None);

    f.key(KEY_LEFTALT, true);
    f.key(KEY_Q, true);
    f.roundtrip(client);
    assert!(f.client(client).window(window.xdg()).close_requested);
    assert_eq!(
        keys(&mut f, client),
        [
            (KEY_T, true, CTRL),
            (KEY_T, false, CTRL),
            (KEY_H, true, CTRL),
        ]
    );
}

#[test]
fn a_system_shortcut_is_never_remapped() {
    let mut f = caminito(vec![key_remaps(vec![all(&[("Alt+Tab", &["Ctrl+T"])])])]);
    let client = listening(&mut f);
    focused(&mut f, client, None);
    focused(&mut f, client, None);

    f.key(KEY_LEFTALT, true);
    f.key(KEY_TAB, true);
    assert_eq!(
        f.state.caminito.policy.interaction(SEAT),
        Some(Interaction::WindowSwitcher)
    );
    assert!(!keys(&mut f, client).iter().any(|(key, ..)| *key == KEY_T));
}

#[test]
fn a_target_that_is_a_system_shortcut_runs_it() {
    let mut f = caminito(vec![key_remaps(vec![all(&[("Alt+Q", &["Alt+F4"])])])]);
    let client = listening(&mut f);
    let window = focused(&mut f, client, None);

    f.chord(&[KEY_LEFTALT, KEY_Q]);
    f.roundtrip(client);

    assert!(f.client(client).window(window.xdg()).close_requested);
    assert!(!keys(&mut f, client).iter().any(|(key, ..)| *key == KEY_Q));
}

#[test]
fn a_target_that_is_an_accelerator_grab_signals_its_grabber() {
    let mut f = caminito(vec![key_remaps(vec![all(&[("Alt+M", &["Ctrl+Alt+A"])])])]);
    let client = listening(&mut f);
    focused(&mut f, client, None);
    let bus = f.start_dbus();
    let mut caller = Caller::new(&mut f, &bus);
    let [ctrl_alt_a] = caller.grab(&mut f, &["<Control><Alt>a"])[..] else {
        panic!("one action id");
    };

    f.chord(&[KEY_LEFTALT, KEY_M]);

    assert_eq!(caller.next_activation(&mut f).0, ctrl_alt_a);
    assert!(!keys(&mut f, client).iter().any(|(key, ..)| *key == KEY_M));
}

#[test]
fn a_key_no_rule_takes_gets_the_held_back_alt_first() {
    let mut f = caminito(vec![key_remaps(vec![all(&[("Alt+T", &["Ctrl+T"])])])]);
    let client = listening(&mut f);
    focused(&mut f, client, None);

    f.key(KEY_LEFTALT, true);
    assert_eq!(keys(&mut f, client), []);

    f.key(KEY_F, true);
    f.key(KEY_F, false);
    f.key(KEY_LEFTALT, false);
    assert_eq!(
        keys(&mut f, client),
        [
            (KEY_LEFTALT, true, ALT),
            (KEY_F, true, ALT),
            (KEY_F, false, ALT),
            (KEY_LEFTALT, false, 0),
        ]
    );
}

#[test]
fn a_held_back_alt_released_alone_reaches_the_client_as_a_tap() {
    let mut f = caminito(vec![key_remaps(vec![all(&[("Alt+T", &["Ctrl+T"])])])]);
    let client = listening(&mut f);
    focused(&mut f, client, None);

    f.chord(&[KEY_LEFTALT]);
    assert_eq!(
        keys(&mut f, client),
        [(KEY_LEFTALT, true, ALT), (KEY_LEFTALT, false, 0)]
    );
}

#[test]
fn alt_reaches_the_client_as_it_is_pressed_when_no_rule_for_the_focus_starts_with_it() {
    let mut f = caminito(vec![key_remaps(vec![
        all(&[("Alt+T", &["Ctrl+T"])]),
        group(&["org.remmina.Remmina"], true, &[]),
        group(&["kitty"], false, &[("Ctrl+Q", &[])]),
    ])]);
    let client = listening(&mut f);

    focused(&mut f, client, Some("org.remmina.Remmina"));
    f.key(KEY_LEFTALT, true);
    assert_eq!(keys(&mut f, client), [(KEY_LEFTALT, true, ALT)]);
    f.key(KEY_LEFTALT, false);

    // Ctrl starts no rule of All applications', nor Alt one of the group's.
    focused(&mut f, client, None);
    f.key(KEY_LEFTCTRL, true);
    assert_eq!(keys(&mut f, client), [(KEY_LEFTCTRL, true, CTRL)]);
    f.key(KEY_LEFTCTRL, false);
}

#[test]
fn a_rule_taken_while_alt_is_held_keeps_it_held_back_for_the_next() {
    let mut f = caminito(vec![key_remaps(vec![all(&[
        ("Alt+T", &["Ctrl+T"]),
        ("Alt+C", &["Ctrl+C"]),
    ])])]);
    let client = listening(&mut f);
    focused(&mut f, client, None);

    f.key(KEY_LEFTALT, true);
    f.chord(&[KEY_T]);
    f.chord(&[KEY_C]);
    f.chord(&[KEY_F]);
    f.key(KEY_LEFTALT, false);
    assert_eq!(
        keys(&mut f, client),
        [
            (KEY_T, true, CTRL),
            (KEY_T, false, CTRL),
            (KEY_C, true, CTRL),
            (KEY_C, false, CTRL),
            (KEY_LEFTALT, true, ALT),
            (KEY_F, true, ALT),
            (KEY_F, false, ALT),
            (KEY_LEFTALT, false, 0),
        ]
    );
}

#[test]
fn another_keys_release_while_alt_is_held_back_never_shows_the_client_the_alt() {
    let mut f = caminito(vec![key_remaps(vec![all(&[(
        "Alt+Shift+Z",
        &["Ctrl+Shift+Z"],
    )])])]);
    let client = listening(&mut f);
    focused(&mut f, client, None);

    f.key(KEY_LEFTSHIFT, true);
    f.key(KEY_LEFTALT, true);
    f.chord(&[KEY_Z]);
    f.key(KEY_LEFTSHIFT, false);
    f.key(KEY_LEFTALT, false);
    assert_eq!(
        keys(&mut f, client),
        [
            (KEY_LEFTSHIFT, true, SHIFT),
            (KEY_Z, true, CTRL | SHIFT),
            (KEY_Z, false, CTRL | SHIFT),
            (KEY_LEFTSHIFT, false, 0),
        ]
    );
}

#[test]
fn a_window_that_takes_the_focus_while_alt_is_held_back_gets_no_alt_tap() {
    let mut f = caminito(vec![key_remaps(vec![all(&[("Alt+T", &["Ctrl+T"])])])]);
    let first = listening(&mut f);
    focused(&mut f, first, None);
    let second = listening(&mut f);

    f.key(KEY_LEFTALT, true);
    focused(&mut f, second, None);
    f.key(KEY_LEFTALT, false);

    // It's told of the Alt as it enters, as of any modifier held then, and of its release.
    assert_eq!(keys(&mut f, second), [(KEY_LEFTALT, false, 0)]);
    assert_eq!(keys(&mut f, first), []);
}

#[test]
fn a_scroll_has_the_held_back_alt_sent_first() {
    let mut f = caminito(vec![key_remaps(vec![all(&[("Alt+T", &["Ctrl+T"])])])]);
    let client = listening(&mut f);
    let window = focused(&mut f, client, None);
    let start = f.geometry(window);
    let point = (start.loc + Point::from((50, 50))).to_f64();
    f.pointer_move(point.x, point.y);

    f.key(KEY_LEFTALT, true);
    f.scroll(120.);
    assert_eq!(keys(&mut f, client), [(KEY_LEFTALT, true, ALT)]);
}

#[test]
fn alt_drag_moves_a_window_while_alt_is_held_back() {
    let mut f = caminito(vec![key_remaps(vec![all(&[("Alt+T", &["Ctrl+T"])])])]);
    let client = listening(&mut f);
    let window = focused(&mut f, client, None);
    let start = f.geometry(window);
    let point = (start.loc + Point::from((50, 10))).to_f64();
    f.pointer_move(point.x, point.y);

    f.key(KEY_LEFTALT, true);
    assert_eq!(keys(&mut f, client), []);
    f.pointer_button(BTN_LEFT, true);
    // The click has the Alt sent first, as a client's own Alt+click needs.
    assert_eq!(keys(&mut f, client), [(KEY_LEFTALT, true, ALT)]);
    f.pointer_move(point.x + 100., point.y + 50.);
    f.pointer_button(BTN_LEFT, false);
    f.key(KEY_LEFTALT, false);
    f.sync(window);

    assert_eq!(f.geometry(window).loc, start.loc + Point::from((100, 50)));
    assert_eq!(keys(&mut f, client), [(KEY_LEFTALT, false, 0)]);
}

#[test]
fn alt_tab_opens_the_window_switcher_while_alt_is_held_back() {
    let mut f = caminito(vec![key_remaps(vec![all(&[("Alt+T", &["Ctrl+T"])])])]);
    let client = listening(&mut f);
    focused(&mut f, client, None);
    focused(&mut f, client, None);

    f.key(KEY_LEFTALT, true);
    f.key(KEY_TAB, true);

    assert_eq!(
        f.state.caminito.policy.interaction(SEAT),
        Some(Interaction::WindowSwitcher)
    );
    // The binding has the Alt sent first, as it was before.
    assert_eq!(keys(&mut f, client)[0], (KEY_LEFTALT, true, ALT));
}

#[test]
fn altgr_q_never_matches_an_alt_q_rule() {
    // RightAlt is AltGr in the German layout.
    let layouts = Value::StrList(vec!["de".into()]);
    let mut f = caminito(vec![
        (KEYBOARD_XKB, "layouts".into(), layouts),
        key_remaps(vec![all(&[("Alt+Q", &["Ctrl+Q"])])]),
    ]);
    let client = listening(&mut f);
    focused(&mut f, client, None);

    f.chord(&[KEY_RIGHTALT, KEY_Q]);
    let got = keys(&mut f, client);
    assert_eq!(got[1..3], [(KEY_Q, true, ALTGR), (KEY_Q, false, ALTGR)]);

    // Left Alt is Alt there still.
    f.chord(&[KEY_LEFTALT, KEY_Q]);
    assert_eq!(keys(&mut f, client)[0], (KEY_Q, true, CTRL));
}

#[test]
fn no_remap_applies_on_the_lock_screen() {
    let mut f = caminito(vec![key_remaps(vec![all(&[("Alt+T", &["Ctrl+T"])])])]);
    let locker = listening(&mut f);
    lock_with_a_surface(&mut f, locker);
    f.dispatch_until(|f| f.state.caminito.lock.is_locked());
    f.client(locker).state.keys.clear();

    f.chord(&[KEY_LEFTALT, KEY_T]);
    assert_eq!(keys(&mut f, locker), unchanged(KEY_LEFTALT, ALT));
}

#[test]
fn no_remap_applies_while_the_window_switcher_is_open() {
    let mut f = caminito(vec![key_remaps(vec![all(&[("Alt+T", &["Ctrl+T"])])])]);
    let client = listening(&mut f);
    focused(&mut f, client, None);
    focused(&mut f, client, None);

    f.key(KEY_LEFTALT, true);
    f.key(KEY_TAB, true);
    f.key(KEY_TAB, false);
    assert_eq!(
        f.state.caminito.policy.interaction(SEAT),
        Some(Interaction::WindowSwitcher)
    );
    f.key(KEY_T, true);
    f.key(KEY_T, false);

    // The Window switcher has the key, and no client gets it, Ctrl+T or other.
    assert!(!keys(&mut f, client).iter().any(|(key, ..)| *key == KEY_T));
}

#[test]
fn setting_key_remaps_applies_to_the_next_key_pressed() {
    let mut f = caminito(Vec::new());
    let client = listening(&mut f);
    focused(&mut f, client, None);
    f.chord(&[KEY_LEFTALT, KEY_T]);
    assert_eq!(keys(&mut f, client), unchanged(KEY_LEFTALT, ALT));

    let (schema, key, value) = key_remaps(vec![all(&[("Alt+T", &["Ctrl+T"])])]);
    f.set_setting(schema, &key, value);
    f.dispatch_until(|f| !f.state.caminito.settings.key_remaps.is_empty());

    f.chord(&[KEY_LEFTALT, KEY_T]);
    assert_eq!(keys(&mut f, client)[0], (KEY_T, true, CTRL));
}

#[test]
fn an_xwayland_client_gets_the_remaps_too() {
    let mut f = caminito(vec![key_remaps(vec![all(&[("Alt+T", &["Ctrl+T"])])])]);
    let x = f.connect(Kind::X11);
    let window = f.map(x, WindowSpec::default());
    let id = window.x11();
    f.x11_call(x, move |x| x.watch_keys(id)).unwrap();
    f.dispatch_until(|f| f.has_keyboard(window));
    f.x11_call(x, |x| x.events()).unwrap();

    f.chord(&[KEY_LEFTALT, KEY_T]);
    f.sync(window);

    let events = f.x11_call(x, |x| x.events()).unwrap();
    // X's keycodes are evdev's plus 8.
    let presses: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::KeyPress(key) if key.event == id => Some((key.detail, key.state)),
            _ => None,
        })
        .collect();
    let t = (KEY_T + 8) as u8;
    assert_eq!(presses, [(t, KeyButMask::CONTROL)], "{events:?}");
}
