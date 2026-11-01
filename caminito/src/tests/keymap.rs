//! `org.mate.peripherals-keyboard-xkb.kbd` as the seat keyboard's keymap (54), live.
//! Xwayland follows the keymap `wl_keyboard` sends it, so a Wayland client stands for
//! either kind (45).

use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;

use caminito_test_client::dbus::caminito_proxy;
use zbus::zvariant::OwnedValue;

use super::fixture::{
    ClientId, Fixture, KEY_LEFTALT, KEY_LEFTSHIFT, KEY_NUMLOCK, Kind, WindowSpec,
};
use crate::settings::Value;
use crate::settings::translate::{KEYBOARD, KEYBOARD_XKB};

/// NumLock's bit in the locked modifiers: Mod2, in the default keymap.
const NUM_LOCK: u32 = 1 << 4;

pub(super) fn strings(values: &[&str]) -> Value {
    Value::StrList(values.iter().map(|&v| v.into()).collect())
}

/// A client with a window, which has the keyboard.
fn focused_client(f: &mut Fixture) -> ClientId {
    f.add_output((1920, 1080), 1.0);
    let id = f.connect(Kind::Xdg);
    f.client(id).bind_input();
    f.map(id, WindowSpec::plain((400, 300)));
    id
}

/// The names of the client's keymap's groups, such as `["English (US)", "Spanish"]`.
/// The symbols section's own name isn't read: libxkbcommon 1.6, Ubuntu 24.04's, writes it as
/// `(unnamed)`. It writes a group's name as `name[Group1]`, and libxkbcommon 1.13, Arch's, as
/// `name[1]`.
fn groups(f: &Fixture, id: ClientId) -> Vec<String> {
    let keymap = f.client_state(id).keymap.as_deref().expect("a keymap");
    keymap
        .lines()
        .filter_map(|line| line.trim().strip_prefix("name["))
        .filter_map(|line| line.split('"').nth(1))
        .map(str::to_owned)
        .collect()
}

/// What `KeyboardLayout` said: the short name, and whether there are several layouts.
fn read_keyboard_layout(value: HashMap<String, OwnedValue>) -> (String, bool) {
    let name = String::try_from(value["name"].try_clone().unwrap()).unwrap();
    let multiple = bool::try_from(&value["multiple"]).unwrap();
    (name, multiple)
}

/// `KeyboardLayout` at `bus`.
pub(super) fn published_keyboard_layout(f: &mut Fixture, bus: &str) -> (String, bool) {
    let bus = bus.to_owned();
    read_keyboard_layout(
        f.dbus_call(move || caminito_proxy(&bus).unwrap().keyboard_layout().unwrap()),
    )
}

/// Waits on another thread for `org.mate.Caminito` at `bus` to signal `KeyboardLayout` once,
/// and returns a handle to what it said. It returns once the thread listens.
pub(super) fn next_keyboard_layout_signalled(
    f: &mut Fixture,
    bus: &str,
) -> thread::JoinHandle<(String, bool)> {
    let bus = bus.to_owned();
    let (listening, is_listening) = mpsc::channel();
    let listener = thread::spawn(move || {
        let proxy = caminito_proxy(&bus).unwrap();
        // Reading it has the proxy listen for changes before this goes on, and zbus's
        // stream of changes starts with the value read.
        proxy.keyboard_layout().unwrap();
        let mut changes = proxy.receive_keyboard_layout_changed();
        changes.next().unwrap();
        listening.send(()).unwrap();
        read_keyboard_layout(changes.next().unwrap().get().unwrap())
    });
    f.dispatch_until(|_| is_listening.try_recv().is_ok());
    listener
}

/// Two layouts and the key that switches them, on one output.
pub(super) fn two_keyboard_layouts() -> Fixture {
    let mut f = Fixture::with_settings(vec![
        (KEYBOARD_XKB, "layouts".into(), strings(&["us", "es"])),
        (
            KEYBOARD_XKB,
            "options".into(),
            strings(&["grp\tgrp:alt_shift_toggle"]),
        ),
    ]);
    f.add_output((1920, 1080), 1.0);
    f
}

#[test]
fn the_published_layout_follows_a_switch_and_signals_it() {
    let mut f = two_keyboard_layouts();
    let bus = f.start_dbus();
    assert_eq!(published_keyboard_layout(&mut f, &bus), ("us".into(), true));

    let listener = next_keyboard_layout_signalled(&mut f, &bus);
    f.chord(&[KEY_LEFTALT, KEY_LEFTSHIFT]);
    f.dispatch_until(|_| listener.is_finished());

    assert_eq!(listener.join().unwrap(), ("es".into(), true));
    assert_eq!(published_keyboard_layout(&mut f, &bus), ("es".into(), true));
}

#[test]
fn one_layout_is_published_as_one_and_a_second_one_set_live_is_signalled() {
    let mut f = Fixture::with_settings(vec![(
        KEYBOARD_XKB,
        "layouts".into(),
        strings(&["de\tnodeadkeys"]),
    )]);
    f.add_output((1920, 1080), 1.0);
    let bus = f.start_dbus();
    assert_eq!(
        published_keyboard_layout(&mut f, &bus),
        ("de".into(), false)
    );

    let listener = next_keyboard_layout_signalled(&mut f, &bus);
    f.set_setting(KEYBOARD_XKB, "layouts", strings(&["de", "fr"]));
    f.dispatch_until(|_| listener.is_finished());

    assert_eq!(listener.join().unwrap(), ("de".into(), true));
}

#[test]
fn a_layout_that_cant_compile_leaves_the_published_name_of_the_one_kept() {
    let mut f = Fixture::with_settings(vec![(KEYBOARD_XKB, "layouts".into(), strings(&["es"]))]);
    let id = focused_client(&mut f);
    let bus = f.start_dbus();

    f.set_setting(KEYBOARD_XKB, "layouts", strings(&["no-such-layout"]));
    // Settings arrive in order, so once the rate has, so has the layout.
    f.set_setting(KEYBOARD, "rate", Value::Int(50));
    f.dispatch_until(|f| f.client_state(id).repeat_info == Some((50, 500)));

    assert_eq!(
        published_keyboard_layout(&mut f, &bus),
        ("es".into(), false)
    );
}

#[test]
fn layouts_at_start_up_are_the_keymap_a_client_gets() {
    let mut f = Fixture::with_settings(vec![(
        KEYBOARD_XKB,
        "layouts".into(),
        strings(&["de\tnodeadkeys"]),
    )]);
    let id = focused_client(&mut f);
    f.roundtrip(id);
    assert_eq!(groups(&f, id), ["German (no dead keys)"]);
}

#[test]
fn setting_layouts_live_sends_a_client_a_keymap_with_them() {
    let mut f = Fixture::new();
    let id = focused_client(&mut f);
    f.roundtrip(id);
    assert_eq!(groups(&f, id), ["English (US)"]);

    f.set_setting(KEYBOARD_XKB, "layouts", strings(&["us", "es"]));
    f.dispatch_until(|f| groups(f, id) == ["English (US)", "Spanish"]);
}

#[test]
fn a_grp_options_keys_switch_the_group() {
    let mut f = Fixture::with_settings(vec![
        (KEYBOARD_XKB, "layouts".into(), strings(&["us", "es"])),
        (
            KEYBOARD_XKB,
            "options".into(),
            strings(&["grp\tgrp:alt_shift_toggle"]),
        ),
    ]);
    let id = focused_client(&mut f);
    f.roundtrip(id);
    assert_eq!(f.client_state(id).group, 0);

    f.chord(&[KEY_LEFTALT, KEY_LEFTSHIFT]);
    f.roundtrip(id);
    assert_eq!(f.client_state(id).group, 1);

    f.chord(&[KEY_LEFTALT, KEY_LEFTSHIFT]);
    f.roundtrip(id);
    assert_eq!(f.client_state(id).group, 0);
}

#[test]
fn a_layout_xkbcommon_cant_compile_keeps_the_keymap() {
    let mut f = Fixture::new();
    let id = focused_client(&mut f);
    f.set_setting(KEYBOARD_XKB, "layouts", strings(&["es"]));
    f.dispatch_until(|f| groups(f, id) == ["Spanish"]);

    f.set_setting(KEYBOARD_XKB, "layouts", strings(&["no-such-layout"]));
    // Settings arrive in order, so once the rate has, so has the layout.
    f.set_setting(KEYBOARD, "rate", Value::Int(50));
    f.dispatch_until(|f| f.client_state(id).repeat_info == Some((50, 500)));
    assert_eq!(groups(&f, id), ["Spanish"]);
}

#[test]
fn a_new_keymap_keeps_numlock() {
    let mut f = Fixture::new();
    let id = focused_client(&mut f);
    f.chord(&[KEY_NUMLOCK]);
    f.roundtrip(id);
    assert_eq!(f.client_state(id).locked_modifiers, NUM_LOCK);

    f.set_setting(KEYBOARD_XKB, "layouts", strings(&["es"]));
    f.dispatch_until(|f| groups(f, id) == ["Spanish"]);
    f.roundtrip(id);
    assert_eq!(f.client_state(id).locked_modifiers, NUM_LOCK);
}
