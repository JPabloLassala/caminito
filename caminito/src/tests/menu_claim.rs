//! The menu claim (115, ADR 0018): `ClaimGlobalMenu` on `org.mate.Caminito` ties the menu
//! its caller serves to the focused toplevel of the caller's process, or of that process's
//! parent, and `GlobalMenuOffered` says whether a claim is taken. Each test runs a bus of
//! its own, as `global_menu`'s do. The windows are xdg toplevels only: the claim is for a
//! window on native Wayland, since an XWayland window's menu is `app_menu_registrar`'s, and
//! a tie is to policy's window, of either kind.

use std::os::unix::process::parent_id;

use caminito_test_client::dbus::{CaminitoProxy, GlobalMenu};
use caminito_test_client::gtk_shell::DbusProperties;
use zbus::blocking::Connection;
use zbus::zvariant::ObjectPath;

use super::fixture::{
    Bus, ClientId, ClientWindow, Fixture, KEY_LEFTALT, KEY_TAB, Kind, WindowSpec,
};
use super::global_menu::{
    a_caller, an_applet, connect, global_menu, map_with_menu, no_menu, on_its_own_bus,
    plumas_global_menu, plumas_menu, the_grace_period_passes, wait_for_the_applet_to_leave,
};
use crate::dbus::menu_claim::parent_process;

/// Where a global menu extension serves its window's dbusmenu.
const MENU: &str = "/org/mate/Caminito/GlobalMenuExtension/menu";

/// A process that's neither the test's own nor its parent, nor any other: the kernel's
/// process ids stop at 2^22.
const ANOTHER_PROCESS: u32 = u32::MAX;

/// Reads `GlobalMenuOffered`.
fn is_offered(f: &mut Fixture, caller: &CaminitoProxy<'static>) -> bool {
    let caller = caller.clone();
    f.dbus_call(move || caller.global_menu_offered().unwrap())
}

/// Dispatches until `GlobalMenuOffered` reads `offered`.
#[track_caller]
fn wait_for_offered(f: &mut Fixture, caller: &CaminitoProxy<'static>, offered: bool) {
    f.dispatch_until_mut(|f| is_offered(f, caller) == offered);
}

/// Caminito on a bus of the test's own with an applet on it, so that the global menu is
/// offered, and a caller that reads the properties.
fn offering() -> (
    Fixture,
    tempfile::TempDir,
    Bus,
    Connection,
    CaminitoProxy<'static>,
) {
    let (mut f, dir, bus) = on_its_own_bus();
    let applet = an_applet(&bus);
    let caller = a_caller(&mut f, &bus);
    wait_for_offered(&mut f, &caller, true);
    (f, dir, bus, applet, caller)
}

/// A stand-in for an extension host: a caller on a connection of its own, which it's on
/// the bus with until it's dropped. Like the fixture's clients, it's of the test's process.
fn a_host(f: &mut Fixture, bus: &Bus) -> CaminitoProxy<'static> {
    a_caller(f, bus)
}

/// Calls `ClaimGlobalMenu` with the menu at `path`. The reply is empty, whatever came of
/// the claim.
fn claim(f: &mut Fixture, host: &CaminitoProxy<'static>, path: &'static str) {
    let host = host.clone();
    let path = ObjectPath::try_from(path).unwrap();
    f.dbus_call(move || host.claim_global_menu(&path).unwrap());
}

/// The `GlobalMenu` of a window of `application` that's tied to `host`'s menu at `path`.
fn claimed(host: &CaminitoProxy<'static>, path: &str, application: &str) -> GlobalMenu {
    let bus_name = host.inner().connection().unique_name().unwrap();
    let paths = [("menu".to_owned(), path.to_owned())].into();
    (
        "dbusmenu".into(),
        bus_name.to_string(),
        paths,
        application.into(),
    )
}

/// Maps a window of the application `app_id` that says nothing of a menu.
fn map(f: &mut Fixture, id: ClientId, app_id: &'static str) -> ClientWindow {
    map_with_menu(f, id, app_id, None)
}

/// The ties in the debug dump, each as its window's id and its caller's bus name.
fn ties(f: &Fixture) -> Vec<(u64, String)> {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    let claims = dump["menu_claims"].as_array().unwrap().iter();
    claims
        .map(|claim| {
            let window = claim["window"].as_u64().unwrap();
            (window, claim["caller"].as_str().unwrap().to_owned())
        })
        .collect()
}

#[test]
fn a_claim_from_the_clients_own_process_ties_its_focused_toplevel() {
    let (mut f, _dir, bus, _applet, caller) = offering();
    let id = f.connect(Kind::Xdg);
    map(&mut f, id, "code");
    let host = a_host(&mut f, &bus);
    assert_eq!(global_menu(&mut f, &caller), no_menu("code"));

    claim(&mut f, &host, MENU);

    assert_eq!(global_menu(&mut f, &caller), claimed(&host, MENU, "code"));
}

/// An extension host is a child of the process that has the windows.
#[test]
fn a_claim_from_a_child_of_the_clients_process_ties_the_same_way() {
    let (mut f, _dir, bus, _applet, caller) = offering();
    // The host is of the test's process, so the client is of that one's parent.
    let id = f.connect_as_process(parent_id());
    map(&mut f, id, "code");
    let host = a_host(&mut f, &bus);

    claim(&mut f, &host, MENU);

    assert_eq!(global_menu(&mut f, &caller), claimed(&host, MENU, "code"));
}

/// One step up, and no further.
#[test]
fn a_claim_from_a_grandchild_of_the_clients_process_is_dropped() {
    let (mut f, _dir, bus, _applet, caller) = offering();
    let grandparent = parent_process(parent_id()).unwrap();
    let id = f.connect_as_process(grandparent);
    map(&mut f, id, "code");
    let host = a_host(&mut f, &bus);

    claim(&mut f, &host, MENU);

    assert_eq!(ties(&f), []);
    assert_eq!(global_menu(&mut f, &caller), no_menu("code"));
}

#[test]
fn a_claim_for_the_focused_toplevel_is_signalled() {
    let (mut f, _dir, bus, _applet, _caller) = offering();
    let id = f.connect(Kind::Xdg);
    map(&mut f, id, "code");
    // A proxy with a cache, as the applet's is: it hears of the changes, and reads none.
    let connection = connect(&bus);
    let listener = f.dbus_call(move || CaminitoProxy::new(&connection).unwrap());
    let mut changes = listener.receive_global_menu_changed();
    // The stream starts with the value as it is, so what follows is a change.
    let (first, mut changes) = f.dbus_call(move || {
        let first = changes.next().unwrap().get().unwrap();
        (first, changes)
    });
    assert_eq!(first, no_menu("code"));
    let host = a_host(&mut f, &bus);

    claim(&mut f, &host, MENU);

    let menu = f.dbus_call(move || changes.next().unwrap().get().unwrap());
    assert_eq!(menu, claimed(&host, MENU, "code"));
}

#[test]
fn with_two_toplevels_of_one_client_the_menu_follows_the_focus() {
    let (mut f, _dir, bus, _applet, caller) = offering();
    let id = f.connect(Kind::Xdg);
    let (first_host, second_host) = (a_host(&mut f, &bus), a_host(&mut f, &bus));
    map(&mut f, id, "code");
    claim(&mut f, &first_host, MENU);
    map(&mut f, id, "code");
    assert_eq!(global_menu(&mut f, &caller), no_menu("code"));

    claim(&mut f, &second_host, MENU);

    let (first, second) = (
        claimed(&first_host, MENU, "code"),
        claimed(&second_host, MENU, "code"),
    );
    assert_eq!(global_menu(&mut f, &caller), second);

    f.chord(&[KEY_LEFTALT, KEY_TAB]);

    assert_eq!(global_menu(&mut f, &caller), first);

    f.chord(&[KEY_LEFTALT, KEY_TAB]);

    assert_eq!(global_menu(&mut f, &caller), second);
}

#[test]
fn a_caller_may_hold_several_toplevels() {
    let (mut f, _dir, bus, _applet, caller) = offering();
    let id = f.connect(Kind::Xdg);
    let host = a_host(&mut f, &bus);
    map(&mut f, id, "code");
    claim(&mut f, &host, "/menu/1");
    map(&mut f, id, "code");

    claim(&mut f, &host, "/menu/2");

    assert_eq!(
        global_menu(&mut f, &caller),
        claimed(&host, "/menu/2", "code")
    );

    f.chord(&[KEY_LEFTALT, KEY_TAB]);

    assert_eq!(
        global_menu(&mut f, &caller),
        claimed(&host, "/menu/1", "code")
    );
}

#[test]
fn the_last_claim_on_a_toplevel_wins() {
    let (mut f, _dir, bus, _applet, caller) = offering();
    let id = f.connect(Kind::Xdg);
    map(&mut f, id, "code");
    let (first_host, second_host) = (a_host(&mut f, &bus), a_host(&mut f, &bus));
    claim(&mut f, &first_host, MENU);

    claim(&mut f, &second_host, MENU);

    let second = claimed(&second_host, MENU, "code");
    assert_eq!(global_menu(&mut f, &caller), second);

    // The first one's tie was replaced, and isn't waiting under the second's.
    drop(second_host);

    f.dispatch_until_mut(|f| global_menu(f, &caller) == no_menu("code"));
}

#[test]
fn a_claim_with_no_toplevel_focused_is_dropped() {
    let (mut f, _dir, bus, _applet, caller) = offering();
    let host = a_host(&mut f, &bus);

    claim(&mut f, &host, MENU);

    assert_eq!(ties(&f), []);
    assert_eq!(global_menu(&mut f, &caller), no_menu(""));
}

#[test]
fn a_claim_is_dropped_while_another_processs_toplevel_has_the_focus() {
    let (mut f, _dir, bus, _applet, caller) = offering();
    let own = f.connect(Kind::Xdg);
    map(&mut f, own, "code");
    let other = f.connect_as_process(ANOTHER_PROCESS);
    map(&mut f, other, "pluma");
    let host = a_host(&mut f, &bus);

    claim(&mut f, &host, MENU);

    assert_eq!(global_menu(&mut f, &caller), no_menu("pluma"));

    // Nor was it tied to the caller's own toplevel, which hadn't the focus.
    f.chord(&[KEY_LEFTALT, KEY_TAB]);

    assert_eq!(global_menu(&mut f, &caller), no_menu("code"));
}

#[test]
fn a_tie_ends_with_its_toplevel() {
    let (mut f, _dir, bus, _applet, caller) = offering();
    let id = f.connect(Kind::Xdg);
    map(&mut f, id, "mate-calc");
    let code = map(&mut f, id, "code");
    let host = a_host(&mut f, &bus);
    claim(&mut f, &host, MENU);
    let name = host.inner().connection().unique_name().unwrap().to_string();
    assert_eq!(ties(&f), [(f.policy_id(code).unwrap().0, name)]);

    f.client(id).destroy_window(code.xdg());
    f.roundtrip(id);

    assert_eq!(ties(&f), []);
    assert_eq!(global_menu(&mut f, &caller), no_menu("mate-calc"));
}

#[test]
fn a_tie_ends_when_its_caller_leaves_the_bus() {
    let (mut f, _dir, bus, _applet, caller) = offering();
    let id = f.connect(Kind::Xdg);
    map(&mut f, id, "code");
    let host = a_host(&mut f, &bus);
    claim(&mut f, &host, MENU);
    assert_eq!(global_menu(&mut f, &caller), claimed(&host, MENU, "code"));

    drop(host);

    f.dispatch_until_mut(|f| global_menu(f, &caller) == no_menu("code"));
    assert_eq!(ties(&f), []);
}

#[test]
fn a_menu_set_on_the_surface_is_shown_in_place_of_a_claim() {
    let (mut f, _dir, bus, _applet, caller) = offering();
    let id = f.connect(Kind::Xdg);
    let pluma = map_with_menu(&mut f, id, "pluma", Some(&plumas_menu()));
    let host = a_host(&mut f, &bus);

    claim(&mut f, &host, MENU);

    assert_eq!(global_menu(&mut f, &caller), plumas_global_menu());

    // The claim was kept, and shows once the surface's own menu is unset.
    f.client(id)
        .set_dbus_properties(pluma.xdg(), &DbusProperties::default());
    f.sync(pluma);

    assert_eq!(global_menu(&mut f, &caller), claimed(&host, MENU, "pluma"));
}

#[test]
fn a_dialog_nobody_claimed_has_its_parents_claimed_menu() {
    let (mut f, _dir, bus, _applet, caller) = offering();
    let id = f.connect(Kind::Xdg);
    let code = map(&mut f, id, "code");
    let host = a_host(&mut f, &bus);
    claim(&mut f, &host, MENU);

    let dialog = WindowSpec {
        app_id: Some("code"),
        parent: Some(code),
        ..WindowSpec::default()
    };
    f.map(id, dialog);

    assert_eq!(global_menu(&mut f, &caller), claimed(&host, MENU, "code"));
}

#[test]
fn a_claim_made_while_the_global_menu_isnt_offered_is_dropped() {
    let (mut f, _dir, bus) = on_its_own_bus();
    let caller = a_caller(&mut f, &bus);
    assert!(!is_offered(&mut f, &caller));
    let id = f.connect(Kind::Xdg);
    map(&mut f, id, "code");
    let host = a_host(&mut f, &bus);

    claim(&mut f, &host, MENU);

    assert_eq!(ties(&f), []);
    // It isn't kept for when an applet comes, either.
    let _applet = an_applet(&bus);
    wait_for_offered(&mut f, &caller, true);
    assert_eq!(global_menu(&mut f, &caller), no_menu("code"));
}

#[test]
fn global_menu_offered_follows_the_applets_bus_name_with_the_grace_period() {
    let (mut f, _dir, bus, applet, caller) = offering();
    let id = f.connect(Kind::Xdg);
    map(&mut f, id, "code");
    let host = a_host(&mut f, &bus);

    drop(applet);
    wait_for_the_applet_to_leave(&mut f);

    // The grace period is part of it, and a claim is taken meanwhile.
    assert!(is_offered(&mut f, &caller));
    claim(&mut f, &host, MENU);
    assert_eq!(global_menu(&mut f, &caller), claimed(&host, MENU, "code"));

    the_grace_period_passes(&mut f);

    wait_for_offered(&mut f, &caller, false);
}

#[test]
fn the_ties_are_forgotten_when_the_global_menu_stops_being_offered() {
    let (mut f, _dir, bus, applet, caller) = offering();
    let id = f.connect(Kind::Xdg);
    map(&mut f, id, "code");
    let host = a_host(&mut f, &bus);
    claim(&mut f, &host, MENU);
    drop(applet);
    wait_for_the_applet_to_leave(&mut f);

    the_grace_period_passes(&mut f);
    wait_for_offered(&mut f, &caller, false);

    assert_eq!(ties(&f), []);
    // An applet that comes later finds the window with no menu, until its host claims.
    let _applet = an_applet(&bus);
    wait_for_offered(&mut f, &caller, true);
    assert_eq!(global_menu(&mut f, &caller), no_menu("code"));
}

#[test]
fn a_change_of_global_menu_offered_is_signalled() {
    let (mut f, _dir, bus) = on_its_own_bus();
    // A proxy with a cache, as the extension's is: it hears of the changes, and reads none.
    let connection = connect(&bus);
    let listener = f.dbus_call(move || CaminitoProxy::new(&connection).unwrap());
    let mut changes = listener.receive_global_menu_offered_changed();
    // The stream starts with the value as it is, so what follows is a change.
    let (first, mut changes) = f.dbus_call(move || {
        let first = changes.next().unwrap().get().unwrap();
        (first, changes)
    });
    assert!(!first);

    let applet = an_applet(&bus);

    let (offered, mut changes) = f.dbus_call(move || {
        let offered = changes.next().unwrap().get().unwrap();
        (offered, changes)
    });
    assert!(offered);

    drop(applet);
    wait_for_the_applet_to_leave(&mut f);
    the_grace_period_passes(&mut f);

    let offered = f.dbus_call(move || changes.next().unwrap().get().unwrap());
    assert!(!offered);
}
