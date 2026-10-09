//! Idle (128): the blank at `idle-delay`, the lock `lock-delay` after it, activity that ends
//! the blank, ext-idle-notify's clients, and what holds idle off: an idle-inhibit surface
//! while it shows, and mate-session's `IsInhibited`, but neither while the session is
//! locked. Each test moves Caminito's clock on itself, so no test waits for a timer. The
//! timers' rules are unit-tested beside them; these go through the seams.
//!
//! The windows are xdg toplevels only: XWayland's clients hold idle off over D-Bus (127),
//! not through idle-inhibit, and the keys and the pointer are the seat's whichever window
//! has them (45).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use caminito_test_client::idle::IdleEvent;
use serde_json::{Value as Json, json};
use zbus::zvariant::ObjectPath;

use super::fixture::{
    Bus, ClientId, ClientWindow, Fixture, KEY_A, KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT, Kind,
    WindowSpec,
};
use crate::scene::SceneLayer;
use crate::settings::Value;
use crate::settings::translate::{CAMINITO, POWER_MANAGER, SCREENSAVER, SESSION};

const MINUTE: Duration = Duration::from_secs(60);
const SECOND: Duration = Duration::from_secs(1);

/// What an empty output shows, and a blanked one.
const CLEAR: u32 = 0x333333;
const BLACK: u32 = 0x000000;

/// Where the tests look at the output: where no window is, away from the cursor.
const EMPTY_SPOT: (i32, i32) = (1800, 1000);

fn setting(schema: &'static str, key: &str, value: Value) -> (&'static str, String, Value) {
    (schema, key.into(), value)
}

/// One minute to idle, blanking, no lock, the outputs never powered off (129's), and
/// `more`.
fn settings(more: Vec<(&'static str, String, Value)>) -> Vec<(&'static str, String, Value)> {
    let mut settings = vec![
        setting(SESSION, "idle-delay", Value::Int(1)),
        setting(SCREENSAVER, "idle-activation-enabled", Value::Bool(true)),
        setting(SCREENSAVER, "lock-enabled", Value::Bool(false)),
        setting(POWER_MANAGER, "sleep-display-ac", Value::Int(0)),
        // A locker that runs until its stdin closes, as the fixture's drop does.
        setting(
            CAMINITO,
            "locker-command",
            Value::StrList(vec!["sh".into(), "-c".into(), "exec cat >/dev/null".into()]),
        ),
    ];
    settings.extend(more);
    settings
}

/// Locking `minutes` after the blank.
fn locking_after(minutes: i32) -> Vec<(&'static str, String, Value)> {
    vec![
        setting(SCREENSAVER, "lock-enabled", Value::Bool(true)),
        setting(SCREENSAVER, "lock-delay", Value::Int(minutes)),
    ]
}

/// Caminito with one output, on [`settings`] with `more`.
fn fixture(more: Vec<(&'static str, String, Value)>) -> Fixture {
    let mut f = Fixture::with_settings(settings(more));
    f.add_output((1920, 1080), 1.0);
    f.dispatch();
    f
}

/// The dump's `idle`.
fn idle(f: &Fixture) -> Json {
    let dump: Json =
        serde_json::from_str(&f.state.caminito.state_dump()).expect("the dump is JSON");
    dump["idle"].clone()
}

fn blanked(f: &Fixture) -> bool {
    idle(f)["blanked"]
        .as_bool()
        .expect("the dump says whether it's blanked")
}

/// A client with a window, which has the keyboard.
fn a_window(f: &mut Fixture) -> (ClientId, ClientWindow) {
    let id = f.connect(Kind::Xdg);
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    (id, window)
}

/// Holds idle off while `window` shows, and returns once Caminito has the inhibitor.
fn inhibit_idle(f: &mut Fixture, id: ClientId, window: ClientWindow) {
    let surface = f.client(id).window(window.xdg()).surface.clone();
    f.client(id).inhibit_idle(&surface);
    f.roundtrip(id);
}

#[test]
fn the_outputs_blank_at_the_idle_delay() {
    let mut f = fixture(Vec::new());
    f.wait_for_pixel(EMPTY_SPOT.0, EMPTY_SPOT.1, CLEAR);

    f.advance_clock(MINUTE - SECOND);
    assert!(!blanked(&f));

    f.advance_clock(SECOND);
    assert!(blanked(&f));
    f.wait_for_pixel(EMPTY_SPOT.0, EMPTY_SPOT.1, BLACK);
}

/// Over the cursor and the lock too, and nothing below it is drawn.
#[test]
fn the_blank_is_all_the_scene_has() {
    let mut f = fixture(Vec::new());
    a_window(&mut f);
    let locker = f.connect(Kind::Xdg);
    f.client(locker).lock_session();
    f.roundtrip(locker);
    let output = f.state.caminito.outputs[0].clone();

    f.advance_clock(MINUTE);

    assert_eq!(f.state.scene_layers(&output), [SceneLayer::Blank]);
}

#[test]
fn nothing_blanks_with_idle_activation_off() {
    let mut f = fixture(vec![setting(
        SCREENSAVER,
        "idle-activation-enabled",
        Value::Bool(false),
    )]);

    f.advance_clock(60 * MINUTE);

    assert!(!blanked(&f));
    f.wait_for_pixel(EMPTY_SPOT.0, EMPTY_SPOT.1, CLEAR);
}

#[test]
fn a_key_unblanks_and_restarts_the_count() {
    let mut f = fixture(Vec::new());
    f.advance_clock(MINUTE);

    f.key(KEY_A, true);
    f.key(KEY_A, false);

    assert!(!blanked(&f));
    f.wait_for_pixel(EMPTY_SPOT.0, EMPTY_SPOT.1, CLEAR);
    f.advance_clock(MINUTE - SECOND);
    assert!(!blanked(&f));
    f.advance_clock(SECOND);
    assert!(blanked(&f));
}

#[test]
fn a_pointer_motion_unblanks_and_restarts_the_count() {
    let mut f = fixture(Vec::new());
    f.advance_clock(30 * SECOND);
    f.pointer_move(500.0, 500.0);
    f.advance_clock(30 * SECOND);
    assert!(!blanked(&f), "the motion restarted the count");
    f.advance_clock(30 * SECOND);
    assert!(blanked(&f));

    f.pointer_move_by(1.0, 0.0);

    assert!(!blanked(&f));
    f.advance_clock(MINUTE - SECOND);
    assert!(!blanked(&f));
}

/// The key that ends a blank goes to the window with the keyboard, as any key does.
#[test]
fn the_key_that_unblanks_reaches_the_client() {
    let mut f = fixture(Vec::new());
    let (id, _) = a_window(&mut f);
    f.advance_clock(MINUTE);
    f.client(id).state.input.clear();

    f.key(KEY_A, true);
    f.roundtrip(id);

    let pressed = caminito_test_client::InputEvent::Key {
        key: KEY_A,
        pressed: true,
    };
    assert_eq!(f.client(id).state.input, [pressed]);
}

#[test]
fn the_lock_comes_lock_delay_after_the_blank() {
    let mut f = fixture(locking_after(1));
    f.advance_clock(MINUTE);
    assert!(blanked(&f));
    assert_eq!(f.lock_state(), "unlocked");

    f.advance_clock(MINUTE - SECOND);
    assert_eq!(f.lock_state(), "unlocked");
    f.advance_clock(SECOND);

    assert_eq!(f.lock_state(), "pending");
    assert!(blanked(&f), "the lock comes under the blank");
}

/// A surface that comes under the pointer moves the focus, which isn't activity: the
/// locker's lock surface, which comes under the blank, doesn't end it.
#[test]
fn a_surface_that_comes_under_the_pointer_leaves_the_blank() {
    let mut f = fixture(Vec::new());
    f.pointer_move(500.0, 500.0);
    f.advance_clock(MINUTE);
    let locker = f.connect(Kind::Xdg);
    f.client(locker).bind_outputs();
    let lock = f.client(locker).lock_session();
    f.roundtrip(locker);

    let surface = f.client(locker).create_lock_surface(lock, 0);
    f.roundtrip(locker);
    f.client(locker).map_lock_surface(surface, 0xff12_3456);
    f.roundtrip(locker);

    assert_eq!(f.lock_state(), "locked");
    assert!(blanked(&f));
}

#[test]
fn nothing_locks_with_lock_enabled_off() {
    let mut f = fixture(Vec::new());

    f.advance_clock(60 * MINUTE);

    assert!(blanked(&f));
    assert_eq!(f.lock_state(), "unlocked");
}

/// mate-screensaver's lock delay is a grace period: activity in it unblanks with no lock.
#[test]
fn activity_before_the_lock_delay_asks_for_no_lock() {
    let mut f = fixture(locking_after(1));
    f.advance_clock(MINUTE + 30 * SECOND);

    f.pointer_move(500.0, 500.0);
    f.advance_clock(30 * SECOND);

    assert_eq!(f.lock_state(), "unlocked");
}

/// Activity shows the lock; it never unlocks (12).
#[test]
fn activity_after_the_lock_unblanks_and_leaves_it_locked() {
    let mut f = fixture(locking_after(0));
    f.advance_clock(MINUTE);
    assert_eq!(f.lock_state(), "pending");

    f.key(KEY_A, true);
    f.key(KEY_A, false);

    assert!(!blanked(&f));
    assert_eq!(f.lock_state(), "pending");
}

#[test]
fn an_idle_notify_client_hears_idled_and_resumed_with_idle_activation_off() {
    let mut f = fixture(vec![setting(
        SCREENSAVER,
        "idle-activation-enabled",
        Value::Bool(false),
    )]);
    let id = f.connect(Kind::Xdg);
    let notification = f.client(id).idle_notification(30_000);
    f.roundtrip(id);
    assert_eq!(idle(&f)["notifications"], 1);

    f.advance_clock(29 * SECOND);
    f.roundtrip(id);
    assert_eq!(f.client(id).idle_events(notification), []);
    f.advance_clock(SECOND);
    f.roundtrip(id);
    assert_eq!(f.client(id).idle_events(notification), [IdleEvent::Idled]);

    f.pointer_move(500.0, 500.0);
    f.roundtrip(id);

    assert_eq!(
        f.client(id).idle_events(notification),
        [IdleEvent::Idled, IdleEvent::Resumed]
    );
    f.client(id).destroy_idle_notification(notification);
    f.roundtrip(id);
    assert_eq!(idle(&f)["notifications"], 0);
}

#[test]
fn an_idle_inhibitor_on_a_window_that_shows_holds_idle_off() {
    let mut f = fixture(Vec::new());
    let (id, window) = a_window(&mut f);
    let notification = f.client(id).idle_notification(1_000);

    inhibit_idle(&mut f, id, window);

    let policy_id = f.policy_id(window).unwrap().0;
    assert_eq!(
        idle(&f)["inhibitors"],
        json!([{ "source": "idle-inhibit", "window": policy_id, "shown": true }])
    );
    assert_eq!(idle(&f)["inhibited"], true);
    f.advance_clock(60 * MINUTE);
    f.roundtrip(id);
    assert!(!blanked(&f));
    assert_eq!(f.client(id).idle_events(notification), []);
}

/// The count starts again when the inhibitor goes, as when it's minimised or destroyed.
#[test]
fn an_idle_inhibitor_on_a_minimised_window_holds_nothing_off() {
    let mut f = fixture(Vec::new());
    let (id, window) = a_window(&mut f);
    inhibit_idle(&mut f, id, window);
    f.advance_clock(10 * MINUTE);

    f.client(id).set_minimized(window.xdg());
    f.roundtrip(id);

    assert_eq!(idle(&f)["inhibitors"][0]["shown"], false);
    assert_eq!(idle(&f)["inhibited"], false);
    f.advance_clock(MINUTE - SECOND);
    assert!(!blanked(&f));
    f.advance_clock(SECOND);
    assert!(blanked(&f));
}

#[test]
fn an_idle_inhibitor_on_another_workspace_holds_nothing_off() {
    let mut f = fixture(Vec::new());
    let (id, window) = a_window(&mut f);
    inhibit_idle(&mut f, id, window);

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);

    assert_eq!(idle(&f)["inhibitors"][0]["shown"], false);
    f.advance_clock(MINUTE);
    assert!(blanked(&f));
}

#[test]
fn an_idle_inhibitor_destroyed_holds_nothing_off() {
    let mut f = fixture(Vec::new());
    let (id, window) = a_window(&mut f);
    let surface = f.client(id).window(window.xdg()).surface.clone();
    let inhibitor = f.client(id).inhibit_idle(&surface);
    f.roundtrip(id);

    f.client(id).uninhibit_idle(inhibitor);
    f.roundtrip(id);

    assert_eq!(idle(&f)["inhibitors"], json!([]));
    f.advance_clock(MINUTE);
    assert!(blanked(&f));
}

/// No client surface shows while locked, so blanking runs on its timer under the lock (12).
#[test]
fn while_locked_an_idle_inhibitor_holds_nothing_off() {
    let mut f = fixture(Vec::new());
    let (id, window) = a_window(&mut f);
    inhibit_idle(&mut f, id, window);
    let locker = f.connect(Kind::Xdg);

    let lock = f.client(locker).lock_session();
    f.roundtrip(locker);
    assert_eq!(f.lock_state(), "locked");

    assert_eq!(idle(&f)["inhibited"], false);
    f.advance_clock(MINUTE);
    assert!(blanked(&f));

    // And it holds idle off again once the session is unlocked.
    f.client(locker).unlock_and_destroy(lock);
    f.roundtrip(locker);
    assert_eq!(f.lock_state(), "unlocked");
    assert_eq!(idle(&f)["inhibited"], true);
}

/// A stand-in for mate-session on a test's bus: `IsInhibited`, and the signals it sends
/// when its inhibitors change.
struct SessionManager {
    inhibited: Arc<AtomicBool>,
    connection: zbus::blocking::Connection,
}

const SESSION_MANAGER_PATH: &str = "/org/gnome/SessionManager";

/// `org.gnome.SessionManager`'s method Caminito calls.
struct SessionManagerInterface(Arc<AtomicBool>);

#[zbus::interface(name = "org.gnome.SessionManager")]
impl SessionManagerInterface {
    /// Whether an inhibitor holds any of `flags` off. Its one inhibitor holds idle off, 8.
    fn is_inhibited(&self, flags: u32) -> bool {
        flags & 8 != 0 && self.0.load(Ordering::SeqCst)
    }
}

impl SessionManager {
    /// mate-session on `bus`, with its inhibitor holding idle off from the start if
    /// `inhibited`.
    fn on(bus: &Bus, inhibited: bool) -> Self {
        let inhibited = Arc::new(AtomicBool::new(inhibited));
        let connection = zbus::blocking::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .name("org.gnome.SessionManager")
            .unwrap()
            .serve_at(
                SESSION_MANAGER_PATH,
                SessionManagerInterface(inhibited.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        SessionManager {
            inhibited,
            connection,
        }
    }

    /// An application asks for idle to be held off, or stops asking, as `Inhibit` and
    /// `Uninhibit` would have it.
    fn set_inhibited(&self, inhibited: bool) {
        self.inhibited.store(inhibited, Ordering::SeqCst);
        let signal = if inhibited {
            "InhibitorAdded"
        } else {
            "InhibitorRemoved"
        };
        let inhibitor = ObjectPath::try_from("/org/gnome/SessionManager/Inhibitor1").unwrap();
        self.connection
            .emit_signal(
                None::<()>,
                SESSION_MANAGER_PATH,
                "org.gnome.SessionManager",
                signal,
                &(inhibitor,),
            )
            .unwrap();
    }
}

#[test]
fn mate_sessions_inhibitor_holds_idle_off() {
    let dir = tempfile::tempdir().unwrap();
    let bus = Bus::start(&dir.path().join("bus"));
    let session = SessionManager::on(&bus, false);
    let mut f = fixture(Vec::new());
    f.start_dbus_at(&bus.address);

    session.set_inhibited(true);
    f.dispatch_until(|f| idle(f)["inhibited"] == true);

    assert_eq!(
        idle(&f)["inhibitors"],
        json!([{ "source": "mate-session" }])
    );
    f.advance_clock(60 * MINUTE);
    assert!(!blanked(&f));

    session.set_inhibited(false);
    f.dispatch_until(|f| idle(f)["inhibited"] == false);
    f.advance_clock(MINUTE);
    assert!(blanked(&f));
}

/// mate-session is asked as it comes onto the bus, after Caminito, with no signal.
#[test]
fn mate_session_is_asked_when_it_comes() {
    let dir = tempfile::tempdir().unwrap();
    let bus = Bus::start(&dir.path().join("bus"));
    let mut f = fixture(Vec::new());
    f.start_dbus_at(&bus.address);
    f.dispatch();

    let _session = SessionManager::on(&bus, true);

    f.dispatch_until(|f| idle(f)["inhibited"] == true);
}

#[test]
fn while_locked_mate_sessions_inhibitor_holds_nothing_off() {
    let dir = tempfile::tempdir().unwrap();
    let bus = Bus::start(&dir.path().join("bus"));
    let session = SessionManager::on(&bus, false);
    let mut f = fixture(Vec::new());
    f.start_dbus_at(&bus.address);
    session.set_inhibited(true);
    f.dispatch_until(|f| idle(f)["inhibited"] == true);

    assert!(f.state.request_lock());
    f.dispatch();

    assert_eq!(idle(&f)["inhibited"], false);
    f.advance_clock(MINUTE);
    assert!(blanked(&f));
}

// Each key applies without a restart.

#[test]
fn a_new_idle_delay_applies_at_once() {
    let mut f = fixture(Vec::new());

    f.set_setting(SESSION, "idle-delay", Value::Int(2));
    f.dispatch_until(|f| f.state.caminito.settings.idle.delay == 2 * MINUTE);

    f.advance_clock(MINUTE);
    assert!(!blanked(&f));
    f.advance_clock(MINUTE);
    assert!(blanked(&f));
}

#[test]
fn idle_activation_turned_on_blanks_a_session_that_is_idle() {
    let mut f = fixture(vec![setting(
        SCREENSAVER,
        "idle-activation-enabled",
        Value::Bool(false),
    )]);
    f.advance_clock(2 * MINUTE);

    f.set_setting(SCREENSAVER, "idle-activation-enabled", Value::Bool(true));
    f.dispatch_until(|f| f.state.caminito.settings.idle.blank);

    assert!(blanked(&f));
}

#[test]
fn lock_enabled_turned_on_locks_a_blank() {
    let mut f = fixture(Vec::new());
    f.advance_clock(MINUTE);

    f.set_setting(SCREENSAVER, "lock-enabled", Value::Bool(true));
    f.dispatch_until(|f| f.state.caminito.settings.idle.lock.is_some());

    assert_eq!(f.lock_state(), "pending");
}

#[test]
fn a_new_lock_delay_counts_from_the_blank() {
    let mut f = fixture(locking_after(10));
    f.advance_clock(MINUTE);
    f.advance_clock(MINUTE);
    assert_eq!(f.lock_state(), "unlocked");

    f.set_setting(SCREENSAVER, "lock-delay", Value::Int(2));
    f.dispatch_until(|f| f.state.caminito.settings.idle.lock == Some(2 * MINUTE));
    assert_eq!(f.lock_state(), "unlocked");
    f.advance_clock(MINUTE);

    assert_eq!(f.lock_state(), "pending");
}
