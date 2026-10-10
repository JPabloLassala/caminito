//! `com.canonical.AppMenu.Registrar` (113, ADR 0017): the menu an X11 client registers for
//! its window reaches `GlobalMenu` while that window has the focus. Each test runs a bus of
//! its own, as `global_menu`'s do. The windows are XWayland's only: the registrar is how an
//! X11 client says where its menu is, and a Wayland client's ways are 110's and 112's.

use std::collections::HashMap;

use caminito_test_client::dbus::{CaminitoProxy, GlobalMenu, RegistrarProxy, registrar_proxy};
use zbus::blocking::Connection;
use zbus::zvariant::ObjectPath;

use super::fixture::{Bus, ClientId, ClientWindow, Fixture, Kind, WindowSpec};
use super::global_menu::{
    a_caller, an_applet, connect, global_menu, no_menu, on_its_own_bus, registrar_is_owned,
};
use crate::dbus::global_menu::REGISTRAR_NAME;

/// Where VS Code serves its window's dbusmenu.
const MENU: &str = "/com/canonical/menu/1";

/// Caminito on a bus of the test's own, an X11 client, and a caller that reads `GlobalMenu`.
fn with_an_x11_client() -> (
    Fixture,
    tempfile::TempDir,
    Bus,
    ClientId,
    CaminitoProxy<'static>,
) {
    let (mut f, dir, bus) = on_its_own_bus();
    let id = f.connect(Kind::X11);
    let caller = a_caller(&mut f, &bus);
    (f, dir, bus, id, caller)
}

/// Maps a window of the application `app_id`.
fn map(f: &mut Fixture, id: ClientId, app_id: &'static str) -> ClientWindow {
    f.map(
        id,
        WindowSpec {
            app_id: Some(app_id),
            ..WindowSpec::default()
        },
    )
}

/// An app's connection to the bus, and the registrar as it sees it. Caminito's own name
/// stands for the registrar's, which it has only while an applet is on the bus (112).
fn an_app(f: &mut Fixture, bus: &Bus) -> (Connection, RegistrarProxy<'static>) {
    let app = connect(bus);
    let destination = crate::dbus::caminito::NAME.to_owned();
    let registrar = {
        let app = app.clone();
        f.dbus_call(move || registrar_proxy(&app, &destination).unwrap())
    };
    (app, registrar)
}

/// Calls `RegisterWindow` for `window` with the menu at `path`.
fn register(f: &mut Fixture, registrar: &RegistrarProxy<'static>, window: u32, path: &'static str) {
    let registrar = registrar.clone();
    let path = ObjectPath::try_from(path).unwrap();
    f.dbus_call(move || registrar.register_window(window, &path).unwrap());
}

/// Calls `UnregisterWindow` for `window`.
fn unregister(f: &mut Fixture, registrar: &RegistrarProxy<'static>, window: u32) {
    let registrar = registrar.clone();
    f.dbus_call(move || registrar.unregister_window(window).unwrap());
}

/// Calls `GetMenuForWindow`.
fn menu_for_window(
    f: &mut Fixture,
    registrar: &RegistrarProxy<'static>,
    window: u32,
) -> (String, String) {
    let registrar = registrar.clone();
    let (service, path) = f.dbus_call(move || registrar.get_menu_for_window(window).unwrap());
    (service, path.to_string())
}

/// The registrar's answer for a window that has no menu.
fn no_registration() -> (String, String) {
    (String::new(), "/".to_owned())
}

/// `app`'s unique name on the bus.
fn unique_name(app: &Connection) -> String {
    app.unique_name().unwrap().to_string()
}

/// A `GlobalMenu` of the `dbusmenu` kind, which `app` serves at `path`.
fn dbusmenu(app: &Connection, path: &str, application: &str) -> GlobalMenu {
    let bus_name = unique_name(app);
    let paths = HashMap::from([("menu".to_owned(), path.to_owned())]);
    ("dbusmenu".into(), bus_name, paths, application.into())
}

#[test]
fn a_registered_windows_menu_is_published_while_it_has_the_focus() {
    let (mut f, _dir, bus, id, caller) = with_an_x11_client();
    let code = map(&mut f, id, "code");
    let (app, registrar) = an_app(&mut f, &bus);

    register(&mut f, &registrar, code.x11(), MENU);

    assert_eq!(global_menu(&mut f, &caller), dbusmenu(&app, MENU, "code"));

    map(&mut f, id, "mate-calc");

    assert_eq!(global_menu(&mut f, &caller), no_menu("mate-calc"));
}

#[test]
fn a_dialog_without_a_registered_menu_has_its_parents() {
    let (mut f, _dir, bus, id, caller) = with_an_x11_client();
    let code = map(&mut f, id, "code");
    let (app, registrar) = an_app(&mut f, &bus);
    register(&mut f, &registrar, code.x11(), MENU);

    // VS Code's Open File dialog, transient for its window and with no menu of its own.
    let open = f.map(
        id,
        WindowSpec {
            app_id: Some("code"),
            parent: Some(code),
            ..WindowSpec::default()
        },
    );

    assert!(f.has_keyboard(open));
    assert_eq!(global_menu(&mut f, &caller), dbusmenu(&app, MENU, "code"));
}

#[test]
fn the_menu_goes_with_its_window() {
    let (mut f, _dir, bus, id, caller) = with_an_x11_client();
    map(&mut f, id, "mate-calc");
    let code = map(&mut f, id, "code");
    let (_app, registrar) = an_app(&mut f, &bus);
    register(&mut f, &registrar, code.x11(), MENU);

    let window = code.x11();
    f.x11_call(id, move |x| x.destroy(window)).unwrap();
    f.dispatch_until(|f| f.policy_id(code).is_none());

    assert_eq!(global_menu(&mut f, &caller), no_menu("mate-calc"));
    // The X server may give the id to another window, which the menu isn't of.
    f.dispatch_until_mut(|f| menu_for_window(f, &registrar, window) == no_registration());
}

#[test]
fn the_menu_goes_with_the_connection_that_registered_it() {
    let (mut f, _dir, bus, id, caller) = with_an_x11_client();
    let code = map(&mut f, id, "code");
    let (app, registrar) = an_app(&mut f, &bus);
    register(&mut f, &registrar, code.x11(), MENU);
    assert_eq!(global_menu(&mut f, &caller), dbusmenu(&app, MENU, "code"));

    drop(registrar);
    drop(app);

    f.dispatch_until_mut(|f| global_menu(f, &caller) == no_menu("code"));
}

#[test]
fn a_window_is_unregistered_by_the_caller_that_registered_it() {
    let (mut f, _dir, bus, id, caller) = with_an_x11_client();
    let code = map(&mut f, id, "code");
    let (app, registrar) = an_app(&mut f, &bus);
    let (_other, others_registrar) = an_app(&mut f, &bus);
    register(&mut f, &registrar, code.x11(), MENU);

    unregister(&mut f, &others_registrar, code.x11());

    assert_eq!(global_menu(&mut f, &caller), dbusmenu(&app, MENU, "code"));

    unregister(&mut f, &registrar, code.x11());

    assert_eq!(global_menu(&mut f, &caller), no_menu("code"));
    assert_eq!(
        menu_for_window(&mut f, &registrar, code.x11()),
        no_registration()
    );
}

#[test]
fn the_registrar_says_where_a_windows_menu_is() {
    let (mut f, _dir, bus, id, _caller) = with_an_x11_client();
    let code = map(&mut f, id, "code");
    let (app, registrar) = an_app(&mut f, &bus);

    register(&mut f, &registrar, code.x11(), MENU);

    let service = unique_name(&app);
    assert_eq!(
        menu_for_window(&mut f, &registrar, code.x11()),
        (service, MENU.to_owned())
    );
}

#[test]
fn a_menu_registered_for_no_x_window_is_kept_and_never_shown() {
    let (mut f, _dir, bus, id, caller) = with_an_x11_client();
    map(&mut f, id, "mate-calc");
    // An Electron app on native Wayland registers a number that counts its windows, which
    // is no X window's id.
    let xdg = f.connect(Kind::Xdg);
    let slack = map(&mut f, xdg, "slack");
    let (app, registrar) = an_app(&mut f, &bus);

    register(&mut f, &registrar, 1, MENU);

    assert!(f.has_keyboard(slack));
    assert_eq!(global_menu(&mut f, &caller), no_menu("slack"));
    let service = unique_name(&app);
    assert_eq!(
        menu_for_window(&mut f, &registrar, 1),
        (service, MENU.to_owned())
    );
}

#[test]
fn the_registrar_answers_at_its_name_while_an_applet_is_on_the_bus() {
    let (mut f, _dir, bus, id, caller) = with_an_x11_client();
    let code = map(&mut f, id, "code");
    let _applet = an_applet(&bus);
    f.dispatch_until_mut(|f| registrar_is_owned(f, &bus));
    let app = connect(&bus);
    let registrar = {
        let app = app.clone();
        f.dbus_call(move || registrar_proxy(&app, REGISTRAR_NAME).unwrap())
    };

    register(&mut f, &registrar, code.x11(), MENU);

    assert_eq!(global_menu(&mut f, &caller), dbusmenu(&app, MENU, "code"));
}

#[test]
fn registering_a_menu_for_the_focused_window_is_signalled() {
    let (mut f, _dir, bus, id, _caller) = with_an_x11_client();
    let code = map(&mut f, id, "code");
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
    let (app, registrar) = an_app(&mut f, &bus);

    register(&mut f, &registrar, code.x11(), MENU);

    let registered = dbusmenu(&app, MENU, "code");
    let menu = {
        let registered = registered.clone();
        f.dbus_call(move || {
            changes
                .find_map(|change| change.get().ok().filter(|menu| *menu == registered))
                .unwrap()
        })
    };
    assert_eq!(menu, registered);
}

#[test]
fn the_menus_go_with_xwayland() {
    let (mut f, _dir, bus, id, _caller) = with_an_x11_client();
    let code = map(&mut f, id, "code");
    let (_app, registrar) = an_app(&mut f, &bus);
    register(&mut f, &registrar, code.x11(), MENU);

    f.crash_xwayland();

    // The next Xwayland gives the ids out again, and the app's connection may outlast it.
    assert_eq!(
        menu_for_window(&mut f, &registrar, code.x11()),
        no_registration()
    );
}
