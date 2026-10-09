//! The Keyboard monitor for Orca (135): `org.freedesktop.a11y.KeyboardMonitor`, with
//! callers on the test binary's private bus. The keys are checked as a Wayland client sees
//! them, which Xwayland follows too, so the windows are xdg toplevels only (45).

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use caminito_test_client::dbus::{KeyEventIterator, KeyboardMonitorProxy, keyboard_monitor_proxy};
use caminito_test_client::format_input;
use insta::assert_snapshot;
use zbus::fdo::{RequestNameFlags, RequestNameReply};

use super::fixture::{
    ClientId, ClientWindow, Fixture, KEY_A, KEY_D, KEY_F4, KEY_F9, KEY_INSERT, KEY_LEFTALT, Kind,
    WindowSpec,
};
use crate::dbus::a11y::ORCA_NAME;
use crate::settings::Value;
use crate::settings::translate::KEYBOARD;

/// The keysyms the tests press, from `xkbcommon-keysyms.h`.
const XK_A: u32 = 0x61;
const XK_D: u32 = 0x64;
const XK_F4: u32 = 0xffc1;
const XK_F9: u32 = 0xffc6;
const XK_INSERT: u32 = 0xff63;
/// Alt's bit in the modifiers, in the default keymap.
const MOD1: u32 = 1 << 3;

/// One `KeyEvent`, as the monitor signalled it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KeyEvent {
    released: bool,
    state: u32,
    keysym: u32,
    unichar: u32,
    keycode: u16,
}

/// A caller of the Keyboard monitor on a connection of its own, and the events it's
/// signalled. One that owns Orca's name holds it, so the tests that need it take turns.
struct Orca {
    monitor: KeyboardMonitorProxy<'static>,
    events: Option<KeyEventIterator>,
    _name: Option<MutexGuard<'static, ()>>,
}

impl Orca {
    /// A caller that owns Orca's name, listening for events from now on. The name is given
    /// up when it's dropped.
    fn new(f: &mut Fixture, bus: &str) -> Self {
        static ONE_ORCA: Mutex<()> = Mutex::new(());
        // A test that failed with the name did no harm to it.
        let turn = ONE_ORCA.lock().unwrap_or_else(|failed| failed.into_inner());
        let mut orca = Self::without_the_name(f, bus);
        // The bus frees the name some time after the last owner's connection closes.
        let connection = orca.monitor.inner().connection().clone();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let reply =
                connection.request_name_with_flags(ORCA_NAME, RequestNameFlags::DoNotQueue.into());
            if let Ok(RequestNameReply::PrimaryOwner) = reply {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the last Orca's name wasn't freed within 5 s"
            );
            std::thread::yield_now();
        }
        orca._name = Some(turn);
        orca
    }

    /// A caller that doesn't own Orca's name.
    fn without_the_name(f: &mut Fixture, bus: &str) -> Self {
        let bus = bus.to_owned();
        let (monitor, events) = f.dbus_call(move || {
            let connection = caminito_test_client::dbus::session().unwrap();
            let monitor = keyboard_monitor_proxy(&connection, &bus).unwrap();
            let events = monitor.receive_key_event().unwrap();
            (monitor, events)
        });
        Self {
            monitor,
            events: Some(events),
            _name: None,
        }
    }

    /// Makes a call on the monitor, off the event loop.
    fn call<T: Send + 'static>(
        &self,
        f: &mut Fixture,
        call: impl FnOnce(&KeyboardMonitorProxy<'static>) -> zbus::Result<T> + Send + 'static,
    ) -> zbus::Result<T> {
        let monitor = self.monitor.clone();
        f.dbus_call(move || call(&monitor))
    }

    fn watch(&self, f: &mut Fixture) {
        self.call(f, |monitor| monitor.watch_keyboard()).unwrap();
    }

    fn grab_keyboard(&self, f: &mut Fixture) {
        self.call(f, |monitor| monitor.grab_keyboard()).unwrap();
    }

    fn set_key_grabs(&self, f: &mut Fixture, modifiers: &[u32], keystrokes: &[(u32, u32)]) {
        let (modifiers, keystrokes) = (modifiers.to_vec(), keystrokes.to_vec());
        self.call(f, move |monitor| {
            monitor.set_key_grabs(&modifiers, &keystrokes)
        })
        .unwrap();
    }

    /// The next event the caller is signalled. It waits for one, so a test of a key that
    /// mustn't be signalled presses one that is after it.
    fn next_event(&mut self, f: &mut Fixture) -> KeyEvent {
        let mut events = self.events.take().unwrap();
        let (event, events) = f.dbus_call(move || {
            let signal = events.next().unwrap();
            let args = signal.args().unwrap();
            let event = KeyEvent {
                released: args.released,
                state: args.state,
                keysym: args.keysym,
                unichar: args.unichar,
                keycode: args.keycode,
            };
            (event, events)
        });
        self.events = Some(events);
        event
    }

    /// The next events' keysyms, each with whether it was a release.
    fn next_keys(&mut self, f: &mut Fixture, count: usize) -> Vec<(u32, bool)> {
        (0..count)
            .map(|_| {
                let event = self.next_event(f);
                (event.keysym, event.released)
            })
            .collect()
    }
}

impl Drop for Orca {
    /// Gives Orca's name up before the next test's turn: a test that failed waiting for an
    /// event leaves the connection open, with a thread still waiting on it.
    fn drop(&mut self) {
        if self._name.is_some() {
            let _ = self.monitor.inner().connection().release_name(ORCA_NAME);
        }
    }
}

/// A fixture on the bus, and a client that records its input, with a window that has the
/// keyboard.
fn with_a_focused_window() -> (Fixture, ClientId, ClientWindow, String) {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    f.client(id).state.input.clear();
    let bus = f.start_dbus();
    (f, id, window, bus)
}

/// What `id` was sent of the keyboard since the last look.
fn client_keys(f: &mut Fixture, id: ClientId) -> String {
    f.roundtrip(id);
    let input = format_input(&f.client(id).state.input);
    f.client(id).state.input.clear();
    input
}

#[test]
fn a_caller_without_orcas_name_is_refused_and_one_with_it_isnt() {
    let (mut f, _, _, bus) = with_a_focused_window();
    let stranger = Orca::without_the_name(&mut f, &bus);

    let refused = [
        stranger.call(&mut f, |monitor| monitor.watch_keyboard()),
        stranger.call(&mut f, |monitor| monitor.grab_keyboard()),
        stranger.call(&mut f, |monitor| monitor.set_key_grabs(&[XK_INSERT], &[])),
    ];

    for refusal in refused {
        let Err(zbus::Error::MethodError(name, _, _)) = refusal else {
            panic!("refused, not {refusal:?}");
        };
        assert_eq!(name.as_str(), "org.freedesktop.DBus.Error.AccessDenied");
    }
    let orca = Orca::new(&mut f, &bus);
    assert!(
        orca.call(&mut f, |monitor| monitor.watch_keyboard())
            .is_ok()
    );
    assert!(orca.call(&mut f, |monitor| monitor.grab_keyboard()).is_ok());
}

#[test]
fn a_watch_gets_a_key_event_for_each_key_and_the_focused_client_still_gets_the_key() {
    let (mut f, id, _, bus) = with_a_focused_window();
    let mut orca = Orca::new(&mut f, &bus);
    orca.watch(&mut f);

    f.chord(&[KEY_A]);

    let press = orca.next_event(&mut f);
    let release = orca.next_event(&mut f);
    // xkb's key code, evdev's plus 8.
    let a = |released| KeyEvent {
        released,
        state: 0,
        keysym: XK_A,
        unichar: u32::from('a'),
        keycode: (KEY_A + 8) as u16,
    };
    assert_eq!([press, release], [a(false), a(true)]);
    assert_snapshot!(client_keys(&mut f, id), @r"
    key 30 pressed
    key 30 released
    ");
}

#[test]
fn a_key_event_carries_the_modifiers_held() {
    let (mut f, _, _, bus) = with_a_focused_window();
    let mut orca = Orca::new(&mut f, &bus);
    orca.watch(&mut f);

    f.chord(&[KEY_LEFTALT, KEY_F9]);

    let events: Vec<_> = (0..4).map(|_| orca.next_event(&mut f)).collect();
    let f9 = events.iter().find(|event| event.keysym == XK_F9).unwrap();
    assert_eq!(f9.state, MOD1);
}

#[test]
fn an_unwatched_caller_is_signalled_no_more() {
    let (mut f, _, _, bus) = with_a_focused_window();
    let mut orca = Orca::new(&mut f, &bus);
    orca.watch(&mut f);
    orca.call(&mut f, |monitor| monitor.unwatch_keyboard())
        .unwrap();
    orca.set_key_grabs(&mut f, &[], &[(XK_F9, 0)]);

    f.chord(&[KEY_A]);
    f.chord(&[KEY_F9]);

    // A wasn't signalled before F9.
    assert_eq!(orca.next_keys(&mut f, 2), [(XK_F9, false), (XK_F9, true)]);
}

#[test]
fn a_grab_of_the_keyboard_gets_the_keys_and_the_client_doesnt() {
    let (mut f, id, _, bus) = with_a_focused_window();
    let mut orca = Orca::new(&mut f, &bus);
    orca.grab_keyboard(&mut f);

    f.chord(&[KEY_A]);
    f.chord(&[KEY_D]);

    let keys = orca.next_keys(&mut f, 4);
    assert_eq!(
        keys,
        [(XK_A, false), (XK_A, true), (XK_D, false), (XK_D, true)]
    );
    assert_snapshot!(client_keys(&mut f, id), @"");

    // Let go, the keys are the client's again.
    orca.call(&mut f, |monitor| monitor.ungrab_keyboard())
        .unwrap();
    f.chord(&[KEY_A]);
    assert_snapshot!(client_keys(&mut f, id), @r"
    key 30 pressed
    key 30 released
    ");
}

#[test]
fn a_key_grab_gets_its_key_and_the_client_doesnt() {
    let (mut f, id, _, bus) = with_a_focused_window();
    let mut orca = Orca::new(&mut f, &bus);
    orca.set_key_grabs(&mut f, &[], &[(XK_A, 0)]);

    f.chord(&[KEY_D]);
    f.chord(&[KEY_A]);
    // With a modifier held, A isn't the keystroke.
    f.chord(&[KEY_LEFTALT, KEY_A]);
    f.chord(&[KEY_A]);

    // D, unwatched and not grabbed, wasn't signalled, nor Alt and A.
    let keys = orca.next_keys(&mut f, 4);
    assert_eq!(
        keys,
        [(XK_A, false), (XK_A, true), (XK_A, false), (XK_A, true)]
    );
    assert_snapshot!(client_keys(&mut f, id), @r"
    key 32 pressed
    key 32 released
    key 56 pressed
    key 30 pressed
    key 30 released
    key 56 released
    ");
}

/// Sets the keyboard's repeat delay, which is how soon a grabbed modifier's second press
/// must come.
fn set_repeat_delay(f: &mut Fixture, delay: i32) {
    f.set_setting(KEYBOARD, "delay", Value::Int(delay));
    f.dispatch_until(|f| f.state.caminito.settings.keyboard.repeat_delay == delay);
}

#[test]
fn a_grabbed_modifier_pressed_alone_reaches_no_client_nor_do_the_keys_pressed_with_it() {
    let (mut f, id, _, bus) = with_a_focused_window();
    set_repeat_delay(&mut f, 100);
    let mut orca = Orca::new(&mut f, &bus);
    orca.set_key_grabs(&mut f, &[XK_INSERT], &[]);

    f.chord(&[KEY_INSERT]);
    // Past the delay, it's a press alone again.
    f.wait_between_inputs(200);
    f.chord(&[KEY_INSERT, KEY_A]);

    let keys = orca.next_keys(&mut f, 6);
    assert_eq!(
        keys,
        [
            (XK_INSERT, false),
            (XK_INSERT, true),
            (XK_INSERT, false),
            (XK_A, false),
            (XK_A, true),
            (XK_INSERT, true),
        ]
    );
    assert_snapshot!(client_keys(&mut f, id), @"");
}

#[test]
fn a_grabbed_modifier_pressed_again_within_the_repeat_delay_reaches_the_client() {
    let (mut f, id, _, bus) = with_a_focused_window();
    set_repeat_delay(&mut f, 100);
    let mut orca = Orca::new(&mut f, &bus);
    orca.set_key_grabs(&mut f, &[XK_INSERT], &[]);

    // The fixture's input events are 10 ms apart: the second press is 20 ms after the
    // first.
    f.chord(&[KEY_INSERT]);
    f.chord(&[KEY_INSERT]);
    // Held as itself, Insert grabs nothing else.
    f.key(KEY_INSERT, true);
    f.chord(&[KEY_A]);
    f.key(KEY_INSERT, false);

    assert_snapshot!(client_keys(&mut f, id), @r"
    key 110 pressed
    key 110 released
    key 110 pressed
    key 30 pressed
    key 30 released
    key 110 released
    ");
    // The monitor hears of the second press too, as Mutter's does.
    let keys = orca.next_keys(&mut f, 4);
    assert_eq!(
        keys,
        [
            (XK_INSERT, false),
            (XK_INSERT, true),
            (XK_INSERT, false),
            (XK_INSERT, true)
        ]
    );
}

#[test]
fn a_grabbed_key_that_is_also_a_binding_goes_to_the_monitor() {
    let (mut f, id, window, bus) = with_a_focused_window();
    let mut orca = Orca::new(&mut f, &bus);
    orca.set_key_grabs(&mut f, &[], &[(XK_F4, MOD1)]);

    f.chord(&[KEY_LEFTALT, KEY_F4]);

    let f4 = orca.next_event(&mut f);
    assert_eq!((f4.keysym, f4.state, f4.released), (XK_F4, MOD1, false));
    assert!(!f.close_requested(window));
    assert_snapshot!(client_keys(&mut f, id), @r"
    key 56 pressed
    key 56 released
    ");
}

#[test]
fn the_grabs_are_gone_when_the_caller_leaves_the_bus() {
    let (mut f, id, _, bus) = with_a_focused_window();
    let orca = Orca::new(&mut f, &bus);
    orca.grab_keyboard(&mut f);
    f.chord(&[KEY_A]);
    assert_snapshot!(client_keys(&mut f, id), @"");

    drop(orca);

    // The bus reports it some time after: until then the keyboard is still grabbed.
    f.dispatch_until_mut(|f| {
        f.chord(&[KEY_A]);
        !client_keys(f, id).is_empty()
    });
    f.chord(&[KEY_D]);
    assert_snapshot!(client_keys(&mut f, id), @r"
    key 32 pressed
    key 32 released
    ");
}

#[test]
fn while_locked_the_events_still_come_and_the_locker_still_gets_the_keys() {
    let (mut f, _, _, bus) = with_a_focused_window();
    let locker = f.connect(Kind::Xdg);
    f.client(locker).bind_input();
    f.client(locker).bind_outputs();
    let lock = f.client(locker).lock_session();
    f.roundtrip(locker);
    let surface = f.client(locker).create_lock_surface(lock, 0);
    f.roundtrip(locker);
    f.client(locker).map_lock_surface(surface, 0xff12_3456);
    f.roundtrip(locker);
    assert_eq!(f.lock_state(), "locked");
    f.client(locker).state.input.clear();
    let mut orca = Orca::new(&mut f, &bus);
    orca.watch(&mut f);

    f.chord(&[KEY_A]);

    assert_eq!(orca.next_keys(&mut f, 2), [(XK_A, false), (XK_A, true)]);
    assert_snapshot!(client_keys(&mut f, locker), @r"
    key 30 pressed
    key 30 released
    ");
}
