//! The Screensaver shim (12, 62, 127): `org.mate.ScreenSaver` and
//! `org.freedesktop.ScreenSaver` on the test binary's private bus. `Lock` locks, `SetActive`
//! and `SimulateUserActivity` drive idle's blank, `GetActive` follows the blank and the lock,
//! and the inhibitors hold idle off. The tests that blank move Caminito's clock on
//! themselves, as idle's do (128). No test maps a window: the shim's inhibitors are on no
//! surface.

use std::sync::mpsc;
use std::time::Duration;

use caminito_test_client::dbus::{
    ScreenSaverProxy, freedesktop_screensaver_proxy, screensaver_proxy, session,
};
use serde_json::{Value as Json, json};
use zbus::blocking::Connection;
use zbus::blocking::fdo::DBusProxy;
use zbus::fdo::RequestNameFlags;

use super::fixture::{Fixture, KEY_A, Kind, Logs};
use crate::settings::Value;
use crate::settings::translate::{CAMINITO, LOCKDOWN, SCREENSAVER, SESSION};

const ACCESS_DENIED: &str = "org.freedesktop.DBus.Error.AccessDenied";

const MINUTE: Duration = Duration::from_secs(60);
const SECOND: Duration = Duration::from_secs(1);

/// The D-Bus error's name, for a call that was refused.
fn error_name(result: zbus::Result<()>) -> String {
    match result {
        Err(zbus::Error::MethodError(name, _, _)) => name.to_string(),
        other => panic!("the call wasn't refused: {other:?}"),
    }
}

/// The inhibitors Caminito lists, as `caller`.
fn inhibitors(f: &mut Fixture, caller: &Connection, bus: &str) -> Vec<String> {
    let shim = screensaver_proxy(caller, bus).unwrap();
    f.dbus_call(move || shim.get_inhibitors().unwrap())
}

fn setting(schema: &'static str, key: &str, value: Value) -> (&'static str, String, Value) {
    (schema, key.into(), value)
}

/// Caminito with one output, on the bus, with one minute to idle, blanking, a lock
/// `lock_delay` minutes after the blank if there is one, a locker that runs until its stdin
/// closes, and `more`. Returns Caminito's unique name, and the shim there.
fn on_the_bus(
    lock_delay: Option<i32>,
    more: Vec<(&'static str, String, Value)>,
) -> (Fixture, String, ScreenSaverProxy<'static>) {
    let mut settings = vec![
        setting(SESSION, "idle-delay", Value::Int(1)),
        setting(SCREENSAVER, "idle-activation-enabled", Value::Bool(true)),
        setting(
            SCREENSAVER,
            "lock-enabled",
            Value::Bool(lock_delay.is_some()),
        ),
        setting(
            SCREENSAVER,
            "lock-delay",
            Value::Int(lock_delay.unwrap_or(0)),
        ),
        setting(
            CAMINITO,
            "locker-command",
            Value::StrList(vec!["sh".into(), "-c".into(), "exec cat >/dev/null".into()]),
        ),
    ];
    settings.extend(more);
    let mut f = Fixture::with_settings(settings);
    f.add_output((1920, 1080), 1.0);
    f.dispatch();
    let bus = f.start_dbus();
    let shim = screensaver_proxy(&session().unwrap(), &bus).unwrap();
    (f, bus, shim)
}

/// Calls the shim off the loop.
fn call<T: Send + 'static>(
    f: &mut Fixture,
    shim: &ScreenSaverProxy<'static>,
    call: impl FnOnce(&ScreenSaverProxy<'static>) -> T + Send + 'static,
) -> T {
    let shim = shim.clone();
    f.dbus_call(move || call(&shim))
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

/// What `GetActive` and `GetActiveTime` answer.
fn active(f: &mut Fixture, shim: &ScreenSaverProxy<'static>) -> (bool, u32) {
    call(f, shim, |shim| {
        (shim.get_active().unwrap(), shim.get_active_time().unwrap())
    })
}

#[test]
fn lock_locks_the_session_and_returns_at_once() {
    let (mut f, _, shim) = on_the_bus(None, Vec::new());

    call(&mut f, &shim, |shim| shim.lock().unwrap());

    // Caminito's own lock, which waits for the locker it spawned.
    assert_eq!(f.lock_state(), "pending");
}

#[test]
fn lock_is_refused_with_disable_lock_screen() {
    let lockdown = setting(LOCKDOWN, "disable-lock-screen", Value::Bool(true));
    let (mut f, _, shim) = on_the_bus(None, vec![lockdown]);

    let locked = call(&mut f, &shim, |shim| shim.lock());

    assert_eq!(error_name(locked), ACCESS_DENIED);
    assert_eq!(f.lock_state(), "unlocked");
}

#[test]
fn unlock_is_refused_and_the_session_stays_locked() {
    let (mut f, _, shim) = on_the_bus(None, Vec::new());
    call(&mut f, &shim, |shim| shim.lock().unwrap());

    let unlocked = call(&mut f, &shim, |shim| shim.unlock());

    assert_eq!(error_name(unlocked), ACCESS_DENIED);
    assert_eq!(f.lock_state(), "pending");
}

#[test]
fn set_active_blanks_at_once_and_starts_the_lock_delay() {
    let (mut f, _, shim) = on_the_bus(Some(1), Vec::new());
    f.advance_clock(30 * SECOND);

    call(&mut f, &shim, |shim| shim.set_active(true).unwrap());

    assert!(blanked(&f));
    f.advance_clock(MINUTE - SECOND);
    assert_eq!(f.lock_state(), "unlocked");
    f.advance_clock(SECOND);
    assert_eq!(f.lock_state(), "pending");
}

#[test]
fn set_active_false_unblanks_and_restarts_the_count() {
    let (mut f, _, shim) = on_the_bus(None, Vec::new());
    call(&mut f, &shim, |shim| shim.set_active(true).unwrap());

    call(&mut f, &shim, |shim| shim.set_active(false).unwrap());

    assert!(!blanked(&f));
    f.advance_clock(MINUTE - SECOND);
    assert!(!blanked(&f));
    f.advance_clock(SECOND);
    assert!(blanked(&f));
}

/// `SetActive(false)` unblanks only a session that isn't locked, and never unlocks (12).
#[test]
fn set_active_false_leaves_a_locked_session_as_it_is() {
    let (mut f, _, shim) = on_the_bus(None, Vec::new());
    call(&mut f, &shim, |shim| {
        shim.lock().unwrap();
        shim.set_active(true).unwrap();
    });

    call(&mut f, &shim, |shim| shim.set_active(false).unwrap());

    assert_eq!(f.lock_state(), "pending");
    assert!(blanked(&f));
}

#[test]
fn the_session_is_active_while_blanked() {
    let (mut f, _, shim) = on_the_bus(None, Vec::new());
    assert_eq!(active(&mut f, &shim), (false, 0));

    f.advance_clock(MINUTE);
    assert_eq!(active(&mut f, &shim), (true, 0));
    f.advance_clock(30 * SECOND);
    let (is_active, seconds) = active(&mut f, &shim);
    assert!(is_active);
    // The real clock goes on too, between the blank and the call.
    assert!((30..=31).contains(&seconds), "{seconds}");

    f.key(KEY_A, true);
    f.key(KEY_A, false);
    assert_eq!(active(&mut f, &shim), (false, 0));
}

#[test]
fn the_session_is_active_while_locked() {
    let (mut f, _, shim) = on_the_bus(None, Vec::new());
    call(&mut f, &shim, |shim| shim.lock().unwrap());
    assert!(!blanked(&f));

    f.advance_clock(10 * SECOND);

    let (is_active, seconds) = active(&mut f, &shim);
    assert!(is_active);
    assert!((10..=11).contains(&seconds), "{seconds}");
}

#[test]
fn simulate_user_activity_unblanks_and_restarts_the_count() {
    let (mut f, _, shim) = on_the_bus(None, Vec::new());
    f.advance_clock(MINUTE);
    assert!(blanked(&f));

    call(&mut f, &shim, |shim| shim.simulate_user_activity().unwrap());

    assert!(!blanked(&f));
    f.advance_clock(MINUTE - SECOND);
    assert!(!blanked(&f));
    f.advance_clock(SECOND);
    assert!(blanked(&f));
}

#[test]
fn simulate_user_activity_never_unlocks() {
    let (mut f, _, shim) = on_the_bus(Some(0), Vec::new());
    f.advance_clock(MINUTE);
    assert_eq!(f.lock_state(), "pending");

    call(&mut f, &shim, |shim| shim.simulate_user_activity().unwrap());

    assert!(!blanked(&f));
    assert_eq!(f.lock_state(), "pending");
    assert!(active(&mut f, &shim).0);
}

/// Each blank and each lock that changes whether the session is active is signalled once:
/// a blank over the lock, and the lock showing as it ends, aren't changes.
#[test]
fn active_changed_is_emitted_once_per_change() {
    let (mut f, _, shim) = on_the_bus(None, Vec::new());
    let changes = shim.receive_active_changed().unwrap();
    let (heard, changed) = mpsc::channel();
    std::thread::spawn(move || {
        for change in changes {
            if heard.send(change.args().unwrap().new_value).is_err() {
                break;
            }
        }
    });
    let next_change = |f: &mut Fixture| {
        let mut change = None;
        f.dispatch_until_mut(|_| {
            change = changed.try_recv().ok();
            change.is_some()
        });
        change.unwrap()
    };

    f.advance_clock(MINUTE);
    assert!(next_change(&mut f), "the idle blank");
    call(&mut f, &shim, |shim| shim.simulate_user_activity().unwrap());
    assert!(!next_change(&mut f), "the activity");
    let locker = f.connect(Kind::Xdg);
    let lock = f.client(locker).lock_session();
    f.roundtrip(locker);
    assert!(next_change(&mut f), "the lock");
    call(&mut f, &shim, |shim| shim.set_active(true).unwrap());
    f.key(KEY_A, true);
    f.key(KEY_A, false);
    f.client(locker).unlock_and_destroy(lock);
    f.roundtrip(locker);

    // Had the blank or the key been signalled, this would be its `true`.
    assert!(!next_change(&mut f), "the locker's unlock");
}

/// The inhibitor that `GetInhibitors` lists, after `inhibit` took one over either name.
fn an_inhibitor_holds_idle_off(inhibit: impl FnOnce(&Connection, &str) -> u32 + Send + 'static) {
    let (mut f, bus, shim) = on_the_bus(None, Vec::new());
    let caller = session().unwrap();
    let cookie = f.dbus_call({
        let (caller, bus) = (caller.clone(), bus.clone());
        move || inhibit(&caller, &bus)
    });
    f.dispatch_until(|f| idle(f)["inhibited"] == true);

    f.advance_clock(60 * MINUTE);

    assert!(!blanked(&f));
    let listed = call(&mut f, &shim, |shim| shim.get_inhibitors().unwrap());
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert!(
        listed[0].starts_with("Application=\"totem\";"),
        "{listed:?}"
    );
    assert_eq!(
        idle(&f)["inhibitors"],
        json!([{ "source": "screensaver", "application": "totem" }])
    );

    let holder = screensaver_proxy(&caller, &bus).unwrap();
    f.dbus_call(move || holder.un_inhibit(cookie).unwrap());
    f.dispatch_until(|f| idle(f)["inhibited"] == false);
    f.advance_clock(MINUTE);
    assert!(blanked(&f));
}

#[test]
fn an_inhibitor_over_org_mate_screensaver_holds_idle_off() {
    an_inhibitor_holds_idle_off(|caller, bus| {
        let shim = screensaver_proxy(caller, bus).unwrap();
        shim.inhibit("totem", "playing a video").unwrap()
    });
}

#[test]
fn an_inhibitor_over_org_freedesktop_screensaver_holds_idle_off() {
    an_inhibitor_holds_idle_off(|caller, bus| {
        let shim = freedesktop_screensaver_proxy(caller, bus, "/ScreenSaver").unwrap();
        shim.inhibit("totem", "playing a video").unwrap()
    });
}

#[test]
fn an_inhibitor_goes_with_its_caller() {
    let (mut f, bus, shim) = on_the_bus(None, Vec::new());
    f.dbus_call(move || {
        let leaves = session().unwrap();
        let shim = freedesktop_screensaver_proxy(&leaves, &bus, "/org/freedesktop/ScreenSaver");
        shim.unwrap().inhibit("firefox", "playing a video").unwrap();
        // `leaves` ends here.
    });

    f.dispatch_until(|f| idle(f)["inhibited"] == false && idle(f)["inhibitors"] == json!([]));

    assert!(call(&mut f, &shim, |shim| shim.get_inhibitors().unwrap()).is_empty());
    f.advance_clock(MINUTE);
    assert!(blanked(&f));
}

#[test]
fn a_throttle_holds_nothing_off() {
    let (mut f, _, shim) = on_the_bus(None, Vec::new());
    call(&mut f, &shim, |shim| {
        shim.throttle("mate-power-manager", "on battery").unwrap()
    });

    f.advance_clock(MINUTE);

    assert!(blanked(&f));
}

/// An inhibitor that comes while the session is locked holds nothing off, as no inhibitor
/// does then (12).
#[test]
fn while_locked_an_inhibitor_holds_nothing_off() {
    let (mut f, _, shim) = on_the_bus(None, Vec::new());
    call(&mut f, &shim, |shim| {
        shim.lock().unwrap();
        shim.inhibit("totem", "playing a video").unwrap()
    });

    f.advance_clock(MINUTE);

    assert_eq!(idle(&f)["inhibited"], false);
    assert!(blanked(&f));
}

#[test]
fn inhibit_gives_distinct_cookies_and_uninhibit_takes_one_back() {
    let mut f = Fixture::new();
    let bus = f.start_dbus();
    let caller = session().unwrap();
    let shim = screensaver_proxy(&caller, &bus).unwrap();

    let (caja, atril) = f.dbus_call({
        let shim = shim.clone();
        move || {
            (
                shim.inhibit("caja", "copying files").unwrap(),
                shim.inhibit("atril", "presenting").unwrap(),
            )
        }
    });
    assert_ne!(caja, atril);
    assert_ne!(caja, 0);
    assert_ne!(atril, 0);
    let listed = inhibitors(&mut f, &caller, &bus);
    assert_eq!(listed.len(), 2, "{listed:?}");

    f.dbus_call(move || shim.un_inhibit(caja).unwrap());

    let listed = inhibitors(&mut f, &caller, &bus);
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert!(
        listed[0].starts_with("Application=\"atril\"; Since=\"")
            && listed[0].ends_with("Z\"; Reason=\"presenting\";"),
        "{listed:?}"
    );
}

#[test]
fn both_names_share_one_set_of_inhibitors() {
    let mut f = Fixture::new();
    let bus = f.start_dbus();
    let caller = session().unwrap();
    let at = |path| freedesktop_screensaver_proxy(&caller, &bus, path).unwrap();
    let (long, short) = (at("/org/freedesktop/ScreenSaver"), at("/ScreenSaver"));
    let mate = screensaver_proxy(&caller, &bus).unwrap();

    let cookies = f.dbus_call({
        let (long, short) = (long.clone(), short.clone());
        move || {
            [
                long.inhibit("firefox", "playing a video").unwrap(),
                short.inhibit("vlc", "playing a video").unwrap(),
                mate.inhibit("caja", "copying files").unwrap(),
            ]
        }
    });
    assert_eq!(inhibitors(&mut f, &caller, &bus).len(), 3);
    assert!(cookies[0] != cookies[1] && cookies[1] != cookies[2] && cookies[0] != cookies[2]);

    // A cookie from one name is taken back at another.
    f.dbus_call(move || {
        long.un_inhibit(cookies[2]).unwrap();
        short.un_inhibit(cookies[0]).unwrap();
    });

    let listed = inhibitors(&mut f, &caller, &bus);
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert!(listed[0].starts_with("Application=\"vlc\";"), "{listed:?}");
}

#[test]
fn a_caller_that_leaves_the_bus_loses_its_cookies() {
    let mut f = Fixture::new();
    let bus = f.start_dbus();
    let stays = session().unwrap();
    let leaves = session().unwrap();

    f.dbus_call({
        let (stays, bus) = (stays.clone(), bus.clone());
        move || {
            let shim = screensaver_proxy(&leaves, &bus).unwrap();
            shim.inhibit("totem", "playing a video").unwrap();
            shim.inhibit("totem", "still playing").unwrap();
            shim.throttle("totem", "on battery").unwrap();
            let shim = screensaver_proxy(&stays, &bus).unwrap();
            shim.inhibit("caja", "copying files").unwrap();
            // `leaves`, and the proxy that holds it, end here.
        }
    });

    f.dispatch_until_mut(|f| {
        let listed = inhibitors(f, &stays, &bus);
        listed.len() == 1 && listed[0].starts_with("Application=\"caja\";")
    });
}

#[test]
fn a_cookie_is_only_its_own_callers_to_take_back() {
    let mut f = Fixture::new();
    let bus = f.start_dbus();
    let caller = session().unwrap();
    let other = session().unwrap();
    let shim = screensaver_proxy(&caller, &bus).unwrap();
    let cookie = f.dbus_call(move || shim.inhibit("caja", "copying files").unwrap());

    f.dbus_call({
        let shim = screensaver_proxy(&other, &bus).unwrap();
        move || shim.un_inhibit(cookie).unwrap()
    });

    assert_eq!(inhibitors(&mut f, &caller, &bus).len(), 1);
}

#[test]
fn throttles_have_their_own_cookies_and_inhibit_nothing() {
    let mut f = Fixture::new();
    let bus = f.start_dbus();
    let caller = session().unwrap();
    let shim = screensaver_proxy(&caller, &bus).unwrap();

    let (throttle, inhibitor) = f.dbus_call({
        let shim = shim.clone();
        move || {
            let throttle = shim.throttle("mate-power-manager", "on battery").unwrap();
            let inhibitor = shim.inhibit("caja", "copying files").unwrap();
            // A throttle's cookie isn't an inhibitor's.
            shim.un_inhibit(throttle).unwrap();
            shim.un_throttle(inhibitor).unwrap();
            (throttle, inhibitor)
        }
    });
    assert_ne!(throttle, inhibitor);
    let listed = inhibitors(&mut f, &caller, &bus);
    assert_eq!(listed.len(), 1, "{listed:?}");

    f.dbus_call(move || {
        shim.un_throttle(throttle).unwrap();
        shim.un_inhibit(inhibitor).unwrap();
    });

    assert!(inhibitors(&mut f, &caller, &bus).is_empty());
}

#[test]
fn the_calls_with_no_effect_are_answered() {
    let mut f = Fixture::new();
    let bus = f.start_dbus();

    f.dbus_call(move || {
        let shim = screensaver_proxy(&session().unwrap(), &bus).unwrap();
        shim.cycle().unwrap();
        shim.show_message("Update", "Restart to finish", "system-software-update")
            .unwrap();
    });
}

#[test]
fn caminito_owns_the_screensaver_names() {
    let mut f = Fixture::new();
    let caminito = f.start_dbus_with_names();
    assert!(f.state.caminito.dbus_ready());

    let bus = DBusProxy::new(&session().unwrap()).unwrap();
    for name in ["org.mate.ScreenSaver", "org.freedesktop.ScreenSaver"] {
        let owner = bus.get_name_owner(name.try_into().unwrap()).unwrap();
        assert_eq!(owner.as_str(), caminito, "{name}");
        // Caminito keeps it: a second screensaver can't take it.
        let flags = RequestNameFlags::ReplaceExisting | RequestNameFlags::DoNotQueue;
        let taken = session().unwrap().request_name_with_flags(name, flags);
        assert!(matches!(taken, Err(zbus::Error::NameTaken)), "{taken:?}");
    }

    let active = f.dbus_call(|| {
        let shim = screensaver_proxy(&session().unwrap(), "org.mate.ScreenSaver").unwrap();
        shim.get_active().unwrap()
    });
    assert!(!active);
}

#[test]
fn a_name_that_is_taken_is_logged_and_caminito_is_still_ready() {
    let logs = Logs::capture();
    let mut f = Fixture::new();
    f.reserve_names();
    let screensaver = session().unwrap();
    // It would let Caminito replace it, if Caminito asked to.
    screensaver
        .request_name_with_flags(
            "org.mate.ScreenSaver",
            RequestNameFlags::AllowReplacement.into(),
        )
        .unwrap();
    let screensaver_name = screensaver.unique_name().unwrap().to_string();

    assert!(!f.state.caminito.dbus_ready());
    let caminito = f.start_dbus_with_names();

    assert!(f.state.caminito.dbus_ready());
    let logs = logs.text();
    assert!(
        logs.contains(&format!(
            "can't own org.mate.ScreenSaver: {screensaver_name} has it"
        )),
        "{logs}"
    );
    // The other name is Caminito's, and the first is left with its owner.
    let bus = DBusProxy::new(&session().unwrap()).unwrap();
    let owner = |name: &'static str| bus.get_name_owner(name.try_into().unwrap()).unwrap();
    assert_eq!(owner("org.freedesktop.ScreenSaver").as_str(), caminito);
    assert_eq!(owner("org.mate.ScreenSaver").as_str(), screensaver_name);
}
