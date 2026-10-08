//! AccessX in the keyboard path (134): `org.mate.accessibility-keyboard` read at start and
//! followed live, a latched modifier in the client's modifiers and in a binding's, the
//! beeps on the bell event, and a gesture's key and notification. The filter itself is
//! unit-tested beside its code. Keys reach clients through `wl_keyboard`, which Xwayland
//! follows too, so a Wayland client stands for either kind (45).

use std::sync::{Arc, Mutex};

use caminito_test_client::format_input;
use insta::assert_snapshot;
use serde_json::{Value as Json, json};

use super::fixture::{
    Bus, ClientId, ClientWindow, Fixture, KEY_A, KEY_F4, KEY_LEFTALT, KEY_LEFTSHIFT, Kind,
    WindowSpec,
};
use crate::settings::Value;
use crate::settings::translate::A11Y_KEYBOARD;

/// Shift's bit in the modifiers, in the default keymap.
const SHIFT: u32 = 1 << 0;

fn on(key: &str) -> (&'static str, String, Value) {
    (A11Y_KEYBOARD, key.into(), Value::Bool(true))
}

/// A client with a window, which has the keyboard, and nothing received yet.
fn focused_client(f: &mut Fixture) -> (ClientId, ClientWindow) {
    f.add_output((1920, 1080), 1.0);
    let id = f.connect(Kind::Xdg);
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    f.client(id).state.input.clear();
    (id, window)
}

/// Presses and releases `key`.
fn tap(f: &mut Fixture, key: u32) {
    f.key(key, true);
    f.key(key, false);
}

/// The modifiers `id` was last told are down.
fn depressed(f: &mut Fixture, id: ClientId) -> u32 {
    f.roundtrip(id);
    f.client_state(id).depressed_modifiers
}

#[test]
fn a_latched_modifier_reaches_the_focused_client_in_its_modifiers() {
    // On at start: the keys are read before the first key.
    let mut f = Fixture::with_settings(vec![on("stickykeys-enable")]);
    let (id, _) = focused_client(&mut f);

    tap(&mut f, KEY_LEFTSHIFT);
    assert_eq!(depressed(&mut f, id), SHIFT);

    f.key(KEY_A, true);
    assert_eq!(depressed(&mut f, id), 0);
    f.key(KEY_A, false);
    f.roundtrip(id);
    // Shift is let go once the key it latched for is pressed.
    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    key 42 pressed
    key 30 pressed
    key 42 released
    key 30 released
    ");
}

#[test]
fn a_binding_fires_with_a_latched_modifier() {
    let mut f = Fixture::with_settings(vec![on("stickykeys-enable")]);
    let (id, window) = focused_client(&mut f);

    tap(&mut f, KEY_LEFTALT);
    tap(&mut f, KEY_F4);
    f.roundtrip(id);

    assert!(f.client(id).window(window.xdg()).close_requested);
    assert_eq!(depressed(&mut f, id), 0);
}

#[test]
fn sticky_keys_turned_on_and_off_live_are_followed() {
    let mut f = Fixture::new();
    let (id, _) = focused_client(&mut f);
    tap(&mut f, KEY_LEFTSHIFT);
    assert_eq!(depressed(&mut f, id), 0);

    f.set_setting(A11Y_KEYBOARD, "stickykeys-enable", Value::Bool(true));
    f.dispatch_until(|f| f.state.caminito.settings.accessx.sticky_keys);
    tap(&mut f, KEY_LEFTSHIFT);
    assert_eq!(depressed(&mut f, id), SHIFT);

    // Turned off, they let go of what they held.
    f.set_setting(A11Y_KEYBOARD, "stickykeys-enable", Value::Bool(false));
    f.dispatch_until(|f| !f.state.caminito.settings.accessx.sticky_keys);
    assert_eq!(depressed(&mut f, id), 0);
}

#[test]
fn a_key_slow_keys_hold_back_reaches_the_client_once_its_delay_is_over() {
    let mut f = Fixture::with_settings(vec![
        on("slowkeys-enable"),
        (A11Y_KEYBOARD, "slowkeys-delay".into(), Value::Int(50)),
    ]);
    let (id, _) = focused_client(&mut f);

    f.key(KEY_A, true);
    f.roundtrip(id);
    assert_snapshot!(format_input(&f.client(id).state.input), @"");

    f.dispatch_until_mut(|f| {
        f.roundtrip(id);
        !f.client_state(id).input.is_empty()
    });
    f.key(KEY_A, false);
    f.roundtrip(id);
    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    key 30 pressed
    key 30 released
    ");
}

#[test]
fn a_beep_rings_the_bell_event_for_the_focused_window() {
    let mut f = Fixture::with_settings(vec![on("bouncekeys-enable"), on("bouncekeys-beep-reject")]);
    let (id, window) = focused_client(&mut f);

    // Well within the 300 ms of the delay.
    tap(&mut f, KEY_A);
    tap(&mut f, KEY_A);
    f.roundtrip(id);

    let dump: Json = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    let last = json!({ "source": "accessx", "window": f.pid(window).0 });
    assert_eq!(dump["bell"], json!({ "count": 1, "last": last }));
    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    key 30 pressed
    key 30 released
    ");
}

/// The notifications a stand-in for the notification daemon was asked to show: each one's
/// application, summary and body.
type Notifications = Arc<Mutex<Vec<(String, String, String)>>>;

struct Daemon(Notifications);

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Daemon {
    #[expect(clippy::too_many_arguments, reason = "the method's own signature")]
    fn notify(
        &self,
        app_name: String,
        _replaces_id: u32,
        _app_icon: String,
        summary: String,
        body: String,
        _actions: Vec<String>,
        _hints: std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
        _expire_timeout: i32,
    ) -> u32 {
        self.0.lock().unwrap().push((app_name, summary, body));
        1
    }
}

#[test]
fn shift_five_times_turns_sticky_keys_on_writes_its_key_and_posts_a_notification() {
    let dir = tempfile::tempdir().unwrap();
    let bus = Bus::start(&dir.path().join("bus"));
    let notifications = Notifications::default();
    let _daemon = zbus::blocking::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .name("org.freedesktop.Notifications")
        .unwrap()
        .serve_at(
            "/org/freedesktop/Notifications",
            Daemon(notifications.clone()),
        )
        .unwrap()
        .build()
        .unwrap();
    let mut f = Fixture::with_settings(vec![on("enable")]);
    f.start_dbus_at(&bus.address);
    let (id, _) = focused_client(&mut f);

    for _ in 0..5 {
        tap(&mut f, KEY_LEFTSHIFT);
    }
    // On at once, before the key comes back from the settings.
    tap(&mut f, KEY_LEFTSHIFT);
    assert_eq!(depressed(&mut f, id), SHIFT);

    f.dispatch_until(|f| f.state.caminito.settings.accessx.sticky_keys);
    f.dispatch_until(|_| !notifications.lock().unwrap().is_empty());
    assert_eq!(
        *notifications.lock().unwrap(),
        [(
            "Caminito".to_owned(),
            "Sticky Keys turned on".to_owned(),
            "Shift was pressed five times in a row.".to_owned()
        )]
    );
    // The key coming back changed nothing: Shift is still latched.
    assert_eq!(depressed(&mut f, id), SHIFT);
}

#[test]
fn shift_five_times_toggles_nothing_without_enable() {
    let mut f = Fixture::new();
    let (id, _) = focused_client(&mut f);

    for _ in 0..6 {
        tap(&mut f, KEY_LEFTSHIFT);
    }

    // The sixth would have latched.
    assert_eq!(depressed(&mut f, id), 0);
}
