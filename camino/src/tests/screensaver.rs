//! The Screensaver shim's shell (12, 62): `org.mate.ScreenSaver` and
//! `org.freedesktop.ScreenSaver` on the test binary's private bus. No test maps a window:
//! the shim has nothing to do with them until M7.

use camino_test_client::dbus::{freedesktop_screensaver_proxy, screensaver_proxy, session};
use zbus::blocking::Connection;
use zbus::blocking::fdo::DBusProxy;
use zbus::fdo::RequestNameFlags;

use super::fixture::{Fixture, Logs};

const ACCESS_DENIED: &str = "org.freedesktop.DBus.Error.AccessDenied";
const NOT_SUPPORTED: &str = "org.freedesktop.DBus.Error.NotSupported";

/// The D-Bus error's name, for a call that was refused.
fn error_name(result: zbus::Result<()>) -> String {
    match result {
        Err(zbus::Error::MethodError(name, _, _)) => name.to_string(),
        other => panic!("the call wasn't refused: {other:?}"),
    }
}

/// The inhibitors Camino lists, as `caller`.
fn inhibitors(f: &mut Fixture, caller: &Connection, bus: &str) -> Vec<String> {
    let shim = screensaver_proxy(caller, bus).unwrap();
    f.dbus_call(move || shim.get_inhibitors().unwrap())
}

#[test]
fn the_session_is_never_active() {
    let mut f = Fixture::new();
    let bus = f.start_dbus();

    let (active, seconds) = f.dbus_call(move || {
        let shim = screensaver_proxy(&session().unwrap(), &bus).unwrap();
        shim.set_active(true).unwrap();
        (shim.get_active().unwrap(), shim.get_active_time().unwrap())
    });

    assert!(!active);
    assert_eq!(seconds, 0);
}

#[test]
fn lock_is_refused_and_logged() {
    let logs = Logs::capture();
    let mut f = Fixture::new();
    let bus = f.start_dbus();
    let caller = session().unwrap();
    let name = caller.unique_name().unwrap().to_string();

    let locked = f.dbus_call(move || screensaver_proxy(&caller, &bus).unwrap().lock());

    assert_eq!(error_name(locked), NOT_SUPPORTED);
    let logs = logs.text();
    assert!(
        logs.contains(&format!("can't lock the session for {name}")),
        "{logs}"
    );
}

#[test]
fn unlock_is_refused() {
    let mut f = Fixture::new();
    let bus = f.start_dbus();

    let unlocked = f.dbus_call(move || {
        screensaver_proxy(&session().unwrap(), &bus)
            .unwrap()
            .unlock()
    });

    assert_eq!(error_name(unlocked), ACCESS_DENIED);
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
        shim.simulate_user_activity().unwrap();
        shim.cycle().unwrap();
        shim.show_message("Update", "Restart to finish", "system-software-update")
            .unwrap();
    });
}

#[test]
fn camino_owns_the_screensaver_names() {
    let mut f = Fixture::new();
    let camino = f.start_dbus_with_names();
    assert!(f.state.camino.dbus_ready());

    let bus = DBusProxy::new(&session().unwrap()).unwrap();
    for name in ["org.mate.ScreenSaver", "org.freedesktop.ScreenSaver"] {
        let owner = bus.get_name_owner(name.try_into().unwrap()).unwrap();
        assert_eq!(owner.as_str(), camino, "{name}");
        // Camino keeps it: a second screensaver can't take it.
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
fn a_name_that_is_taken_is_logged_and_camino_is_still_ready() {
    let logs = Logs::capture();
    let mut f = Fixture::new();
    f.reserve_names();
    let screensaver = session().unwrap();
    // It would let Camino replace it, if Camino asked to.
    screensaver
        .request_name_with_flags(
            "org.mate.ScreenSaver",
            RequestNameFlags::AllowReplacement.into(),
        )
        .unwrap();
    let screensaver_name = screensaver.unique_name().unwrap().to_string();

    assert!(!f.state.camino.dbus_ready());
    let camino = f.start_dbus_with_names();

    assert!(f.state.camino.dbus_ready());
    let logs = logs.text();
    assert!(
        logs.contains(&format!(
            "can't own org.mate.ScreenSaver: {screensaver_name} has it"
        )),
        "{logs}"
    );
    // The other name is Camino's, and the first is left with its owner.
    let bus = DBusProxy::new(&session().unwrap()).unwrap();
    let owner = |name: &'static str| bus.get_name_owner(name.try_into().unwrap()).unwrap();
    assert_eq!(owner("org.freedesktop.ScreenSaver").as_str(), camino);
    assert_eq!(owner("org.mate.ScreenSaver").as_str(), screensaver_name);
}
