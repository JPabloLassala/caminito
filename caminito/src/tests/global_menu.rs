//! The global menu (110, ADR 0017): the `GlobalMenu` property of `org.mate.Caminito`, which
//! follows the focus, and `gtk_shell1`'s `global_menu_bar` capability, which follows the
//! Global menu applet's bus name and outlasts it by a grace period (114), as the appmenu
//! global and the registrar's name do (112). Each test runs a bus of its own, with a
//! stand-in that owns the applet's name, which only one could on the tests' bus. The windows
//! are xdg toplevels only: an XWayland window has no `gtk_surface1` and no
//! `org_kde_kwin_appmenu`, and its menu is ticket 113's.

use std::collections::HashMap;

use caminito_test_client::dbus::{CaminitoProxy, GlobalMenu};
use caminito_test_client::gtk_shell::DbusProperties;
use caminito_test_client::layer_shell::{KeyboardInteractivity, LayerSpec};
use zbus::blocking::Connection;

use super::fixture::{
    Bus, ClientId, ClientWindow, Fixture, KEY_LEFTALT, KEY_TAB, Kind, WindowSpec,
};
use crate::dbus::global_menu::{APPLET_NAME, GRACE, REGISTRAR_NAME};

/// `gtk_shell1`'s `global_menu_bar` capability.
const GLOBAL_MENU_BAR: u32 = 2;

/// Caminito with one output on a bus of the test's own, which lasts as long as the directory
/// and the bus do.
fn on_its_own_bus() -> (Fixture, tempfile::TempDir, Bus) {
    let dir = tempfile::tempdir().unwrap();
    let bus = Bus::start(&dir.path().join("bus"));
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.0);
    f.start_dbus_at(&bus.address);
    (f, dir, bus)
}

/// A connection to the test's bus.
fn connect(bus: &Bus) -> Connection {
    zbus::blocking::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .build()
        .unwrap()
}

/// A stand-in for the Global menu applet: a connection that owns its name until it's
/// dropped.
fn an_applet(bus: &Bus) -> Connection {
    let applet = connect(bus);
    applet.request_name(APPLET_NAME).unwrap();
    applet
}

/// A client that has bound `gtk_shell1`, as GTK does when it starts.
fn a_gtk_client(f: &mut Fixture) -> ClientId {
    let id = f.connect(Kind::Xdg);
    f.client(id).bind_gtk_shell();
    f.roundtrip(id);
    id
}

/// A client that has found the appmenu global, as a Qt app or Firefox does that starts
/// while the applet is there.
fn a_qt_client(f: &mut Fixture) -> ClientId {
    let id = f.connect(Kind::Xdg);
    wait_for_appmenu_manager(f, id, true);
    id
}

/// Dispatches until `id` is offered the appmenu global, or isn't any more.
#[track_caller]
fn wait_for_appmenu_manager(f: &mut Fixture, id: ClientId, offered: bool) {
    f.dispatch_until_mut(|f| {
        f.roundtrip(id);
        f.client(id).has_appmenu_manager() == offered
    });
}

/// Whether the registrar's name has an owner.
fn registrar_is_owned(f: &mut Fixture, bus: &Bus) -> bool {
    let connection = connect(bus);
    f.dbus_call(move || {
        let bus = zbus::blocking::fdo::DBusProxy::new(&connection).unwrap();
        bus.name_has_owner(REGISTRAR_NAME.try_into().unwrap())
            .unwrap()
    })
}

/// Dispatches until `id` was last sent `capabilities`.
#[track_caller]
fn wait_for_capabilities(f: &mut Fixture, id: ClientId, capabilities: u32) {
    f.dispatch_until_mut(|f| f.client(id).gtk_capabilities().last() == Some(&capabilities));
}

/// The debug dump's `global_menu_switch` member.
fn switch(f: &Fixture) -> String {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    dump["global_menu_switch"].as_str().unwrap().to_owned()
}

/// Dispatches until Caminito has heard that the applet's name lost its owner.
#[track_caller]
fn wait_for_the_applet_to_leave(f: &mut Fixture) {
    f.dispatch_until(|f| switch(f) == "leaving");
}

/// The grace period passes: Caminito is given the time its timer would give it when it
/// fires, with no waiting.
fn the_grace_period_passes(f: &mut Fixture) {
    let now = f.state.caminito.now() + GRACE;
    f.state.caminito.global_menu_time_passed(now);
}

/// What pluma's window says of its menus, were pluma a `GtkApplication`.
fn plumas_menu() -> DbusProperties {
    DbusProperties {
        application_id: Some("org.mate.pluma"),
        app_menu_path: None,
        menubar_path: Some("/org/mate/pluma/menus/menubar"),
        window_object_path: Some("/org/mate/pluma/window/1"),
        application_object_path: Some("/org/mate/pluma"),
        unique_bus_name: Some(":1.42"),
    }
}

/// `plumas_menu` as the property has it.
fn plumas_global_menu() -> GlobalMenu {
    let paths = [
        ("menubar", "/org/mate/pluma/menus/menubar"),
        ("window", "/org/mate/pluma/window/1"),
        ("application", "/org/mate/pluma"),
    ];
    gtk_menu(":1.42", &paths, "pluma")
}

/// What a GTK4 app with an app menu too says of its window.
fn a_gtk4_menu() -> DbusProperties {
    DbusProperties {
        application_id: Some("org.gnome.TextEditor"),
        app_menu_path: Some("/org/gnome/TextEditor/menus/appmenu"),
        menubar_path: Some("/org/gnome/TextEditor/menus/menubar"),
        window_object_path: Some("/org/gnome/TextEditor/window/3"),
        application_object_path: Some("/org/gnome/TextEditor"),
        unique_bus_name: Some(":1.57"),
    }
}

/// A `GlobalMenu` of the `gtk` kind.
fn gtk_menu(bus_name: &str, paths: &[(&str, &str)], application: &str) -> GlobalMenu {
    let paths: HashMap<_, _> = paths
        .iter()
        .map(|(key, path)| ((*key).to_owned(), (*path).to_owned()))
        .collect();
    ("gtk".into(), bus_name.into(), paths, application.into())
}

/// A `GlobalMenu` of the `dbusmenu` kind.
fn dbusmenu(bus_name: &str, path: &str, application: &str) -> GlobalMenu {
    let paths = HashMap::from([("menu".to_owned(), path.to_owned())]);
    (
        "dbusmenu".into(),
        bus_name.into(),
        paths,
        application.into(),
    )
}

/// The `GlobalMenu` of a focused window that has no menu.
fn no_menu(application: &str) -> GlobalMenu {
    (
        String::new(),
        String::new(),
        HashMap::new(),
        application.into(),
    )
}

/// Maps a window of the application `app_id` that says `menu` of itself, if any.
fn map_with_menu(
    f: &mut Fixture,
    id: ClientId,
    app_id: &'static str,
    menu: Option<&DbusProperties>,
) -> ClientWindow {
    let window = f.map(
        id,
        WindowSpec {
            app_id: Some(app_id),
            ..WindowSpec::default()
        },
    );
    if let Some(menu) = menu {
        f.client(id).set_dbus_properties(window.xdg(), menu);
        f.sync(window);
    }
    window
}

/// A caller that reads `GlobalMenu`, on a connection of its own.
fn a_caller(f: &mut Fixture, bus: &Bus) -> CaminitoProxy<'static> {
    let connection = connect(bus);
    // Without a cache, so that each read asks Caminito.
    f.dbus_call(move || {
        CaminitoProxy::builder(&connection)
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .unwrap()
    })
}

/// Reads `GlobalMenu`.
fn global_menu(f: &mut Fixture, caller: &CaminitoProxy<'static>) -> GlobalMenu {
    let caller = caller.clone();
    f.dbus_call(move || caller.global_menu().unwrap())
}

#[test]
fn the_capability_follows_the_applets_bus_name() {
    let (mut f, _dir, bus) = on_its_own_bus();
    let early = a_gtk_client(&mut f);
    // No applet: GTK is told nothing, and keeps its menubars.
    assert_eq!(f.client(early).gtk_capabilities(), &[] as &[u32]);

    let applet = an_applet(&bus);

    wait_for_capabilities(&mut f, early, GLOBAL_MENU_BAR);
    // A client that starts while the applet is there is told when it binds.
    let late = a_gtk_client(&mut f);
    assert_eq!(f.client(late).gtk_capabilities(), &[GLOBAL_MENU_BAR]);

    drop(applet);
    wait_for_the_applet_to_leave(&mut f);
    the_grace_period_passes(&mut f);

    wait_for_capabilities(&mut f, early, 0);
    wait_for_capabilities(&mut f, late, 0);
    assert_eq!(f.client(early).gtk_capabilities(), &[GLOBAL_MENU_BAR, 0]);
}

#[test]
fn the_capability_stays_for_the_grace_period_after_the_applet_leaves() {
    let (mut f, _dir, bus) = on_its_own_bus();
    let applet = an_applet(&bus);
    let early = a_gtk_client(&mut f);
    wait_for_capabilities(&mut f, early, GLOBAL_MENU_BAR);

    drop(applet);
    wait_for_the_applet_to_leave(&mut f);

    f.roundtrip(early);
    assert_eq!(f.client(early).gtk_capabilities(), &[GLOBAL_MENU_BAR]);
    // A client that starts meanwhile, as the panel's own does, is told as before.
    let late = a_gtk_client(&mut f);
    assert_eq!(f.client(late).gtk_capabilities(), &[GLOBAL_MENU_BAR]);
}

#[test]
fn a_restart_of_the_panel_within_the_grace_period_changes_nothing() {
    let (mut f, _dir, bus) = on_its_own_bus();
    let applet = an_applet(&bus);
    let id = a_gtk_client(&mut f);
    wait_for_capabilities(&mut f, id, GLOBAL_MENU_BAR);
    drop(applet);
    wait_for_the_applet_to_leave(&mut f);

    let _restarted = an_applet(&bus);
    f.dispatch_until(|f| switch(f) == "on");
    the_grace_period_passes(&mut f);

    f.roundtrip(id);
    assert_eq!(switch(&f), "on");
    assert_eq!(f.client(id).gtk_capabilities(), &[GLOBAL_MENU_BAR]);
}

#[test]
fn an_applet_that_was_there_before_caminito_counts() {
    let dir = tempfile::tempdir().unwrap();
    let bus = Bus::start(&dir.path().join("bus"));
    let _applet = an_applet(&bus);
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.0);
    f.start_dbus_at(&bus.address);
    let id = f.connect(Kind::Xdg);
    f.client(id).bind_gtk_shell();

    wait_for_capabilities(&mut f, id, GLOBAL_MENU_BAR);
}

#[test]
fn the_property_follows_the_focus() {
    let (mut f, _dir, bus) = on_its_own_bus();
    let id = f.connect(Kind::Xdg);
    let caller = a_caller(&mut f, &bus);
    assert_eq!(global_menu(&mut f, &caller), no_menu(""));

    map_with_menu(&mut f, id, "pluma", Some(&plumas_menu()));

    assert_eq!(global_menu(&mut f, &caller), plumas_global_menu());

    map_with_menu(&mut f, id, "org.gnome.TextEditor", Some(&a_gtk4_menu()));

    let paths = [
        ("app-menu", "/org/gnome/TextEditor/menus/appmenu"),
        ("menubar", "/org/gnome/TextEditor/menus/menubar"),
        ("window", "/org/gnome/TextEditor/window/3"),
        ("application", "/org/gnome/TextEditor"),
    ];
    let gtk4 = gtk_menu(":1.57", &paths, "org.gnome.TextEditor");
    assert_eq!(global_menu(&mut f, &caller), gtk4);

    f.chord(&[KEY_LEFTALT, KEY_TAB]);

    assert_eq!(global_menu(&mut f, &caller), plumas_global_menu());
}

#[test]
fn a_change_of_the_property_is_signalled() {
    let (mut f, _dir, bus) = on_its_own_bus();
    let id = f.connect(Kind::Xdg);
    // A proxy with a cache, as the applet's is: it hears of the changes, and reads none.
    let connection = connect(&bus);
    let listener = f.dbus_call(move || CaminitoProxy::new(&connection).unwrap());
    let mut changes = listener.receive_global_menu_changed();
    // The stream starts with the value as it is, so what follows is a change.
    let (first, mut changes) = f.dbus_call(move || {
        let first = changes.next().unwrap().get().unwrap();
        (first, changes)
    });
    assert_eq!(first, no_menu(""));

    // GTK says where the menus are once the window is mapped and focused.
    map_with_menu(&mut f, id, "pluma", Some(&plumas_menu()));

    let menu = f.dbus_call(move || {
        let pluma = plumas_global_menu();
        changes
            .find_map(|change| change.get().ok().filter(|menu| *menu == pluma))
            .unwrap()
    });
    assert_eq!(menu, plumas_global_menu());
}

#[test]
fn a_window_without_a_menu_has_its_application_alone() {
    let (mut f, _dir, bus) = on_its_own_bus();
    let id = f.connect(Kind::Xdg);
    let caller = a_caller(&mut f, &bus);
    map_with_menu(&mut f, id, "pluma", Some(&plumas_menu()));

    map_with_menu(&mut f, id, "mate-calc", None);

    assert_eq!(global_menu(&mut f, &caller), no_menu("mate-calc"));
}

#[test]
fn a_window_that_only_names_its_actions_has_no_menu() {
    let (mut f, _dir, bus) = on_its_own_bus();
    let id = f.connect(Kind::Xdg);
    let caller = a_caller(&mut f, &bus);
    // A `GtkApplicationWindow` with neither a menubar nor an app menu still says where
    // its actions are.
    let actions_only = DbusProperties {
        menubar_path: None,
        ..plumas_menu()
    };

    map_with_menu(&mut f, id, "mate-calc", Some(&actions_only));

    assert_eq!(global_menu(&mut f, &caller), no_menu("mate-calc"));
}

/// A dialog of `parent`'s application, with no menu of its own.
fn a_dialog_of(parent: ClientWindow) -> WindowSpec {
    WindowSpec {
        app_id: Some("pluma"),
        parent: Some(parent),
        ..WindowSpec::default()
    }
}

#[test]
fn a_dialog_without_a_menu_has_its_parents() {
    let (mut f, _dir, bus) = on_its_own_bus();
    let id = f.connect(Kind::Xdg);
    let caller = a_caller(&mut f, &bus);
    let pluma = map_with_menu(&mut f, id, "pluma", Some(&plumas_menu()));

    let find = f.map(id, a_dialog_of(pluma));

    assert!(f.has_keyboard(find));
    assert_eq!(global_menu(&mut f, &caller), plumas_global_menu());

    // And so on up: a dialog of the dialog.
    let confirm = f.map(id, a_dialog_of(find));

    assert!(f.has_keyboard(confirm));
    assert_eq!(global_menu(&mut f, &caller), plumas_global_menu());
}

#[test]
fn a_dialog_with_a_menu_has_its_own() {
    let (mut f, _dir, bus) = on_its_own_bus();
    let id = f.connect(Kind::Xdg);
    let caller = a_caller(&mut f, &bus);
    let pluma = map_with_menu(&mut f, id, "pluma", Some(&plumas_menu()));

    let dialog = f.map(id, a_dialog_of(pluma));
    f.client(id)
        .set_dbus_properties(dialog.xdg(), &a_gtk4_menu());
    f.sync(dialog);

    let (kind, bus_name, _, application) = global_menu(&mut f, &caller);
    assert_eq!((kind.as_str(), bus_name.as_str()), ("gtk", ":1.57"));
    assert_eq!(application, "pluma");
}

#[test]
fn a_dialog_whose_parent_has_no_menu_has_its_application_alone() {
    let (mut f, _dir, bus) = on_its_own_bus();
    let id = f.connect(Kind::Xdg);
    let caller = a_caller(&mut f, &bus);
    let calc = map_with_menu(&mut f, id, "mate-calc", None);

    f.map(id, a_dialog_of(calc));

    assert_eq!(global_menu(&mut f, &caller), no_menu("pluma"));
}

#[test]
fn the_menu_goes_with_its_window() {
    let (mut f, _dir, bus) = on_its_own_bus();
    let id = f.connect(Kind::Xdg);
    let caller = a_caller(&mut f, &bus);
    map_with_menu(&mut f, id, "mate-calc", None);
    let pluma = map_with_menu(&mut f, id, "pluma", Some(&plumas_menu()));

    f.client(id).destroy_window(pluma.xdg());
    f.roundtrip(id);

    assert_eq!(global_menu(&mut f, &caller), no_menu("mate-calc"));
}

#[test]
fn the_panel_taking_the_keyboard_leaves_the_menu() {
    let (mut f, _dir, bus) = on_its_own_bus();
    let id = f.connect(Kind::Xdg);
    let caller = a_caller(&mut f, &bus);
    let pluma = map_with_menu(&mut f, id, "pluma", Some(&plumas_menu()));
    // A bar that takes the keyboard when it's clicked, as the panel's does for a menu of
    // the applet's.
    let panel = f.connect(Kind::Xdg);
    let bar = LayerSpec {
        keyboard: KeyboardInteractivity::OnDemand,
        ..LayerSpec::top_bar(28)
    };
    f.map_layer(panel, bar, 0xff00ff00);

    f.click(100.0, 10.0);

    assert!(!f.has_keyboard(pluma));
    assert_eq!(global_menu(&mut f, &caller), plumas_global_menu());
}

#[test]
fn the_appmenu_global_and_the_registrars_name_follow_the_applets_bus_name() {
    let (mut f, _dir, bus) = on_its_own_bus();
    let early = f.connect(Kind::Xdg);
    f.roundtrip(early);
    // No applet: a Qt app and Firefox find neither, and keep their menubars.
    assert!(!f.client(early).has_appmenu_manager());
    assert!(!registrar_is_owned(&mut f, &bus));

    let applet = an_applet(&bus);

    f.dispatch_until_mut(|f| registrar_is_owned(f, &bus));
    // The name is never there without the global, so there's no waiting for this.
    let late = f.connect(Kind::Xdg);
    f.roundtrip(late);
    assert!(f.client(late).has_appmenu_manager());
    wait_for_appmenu_manager(&mut f, early, true);

    drop(applet);
    wait_for_the_applet_to_leave(&mut f);
    // All three stay for the grace period.
    assert!(registrar_is_owned(&mut f, &bus));
    assert!(f.client(early).has_appmenu_manager());
    the_grace_period_passes(&mut f);

    wait_for_appmenu_manager(&mut f, early, false);
    // Nor is there for this: the name went first.
    assert!(!registrar_is_owned(&mut f, &bus));
    wait_for_appmenu_manager(&mut f, late, false);
}

#[test]
fn a_dbusmenu_address_reaches_the_property_when_its_window_has_the_focus() {
    let (mut f, _dir, bus) = on_its_own_bus();
    let _applet = an_applet(&bus);
    let id = a_qt_client(&mut f);
    let caller = a_caller(&mut f, &bus);
    let dolphin = map_with_menu(&mut f, id, "org.kde.dolphin", None);

    f.client(id)
        .set_appmenu_address(dolphin.xdg(), ":1.77", "/MenuBar/1");
    f.sync(dolphin);

    let dolphins = dbusmenu(":1.77", "/MenuBar/1", "org.kde.dolphin");
    assert_eq!(global_menu(&mut f, &caller), dolphins);

    map_with_menu(&mut f, id, "mate-calc", None);

    assert_eq!(global_menu(&mut f, &caller), no_menu("mate-calc"));

    f.chord(&[KEY_LEFTALT, KEY_TAB]);

    assert_eq!(global_menu(&mut f, &caller), dolphins);
}

#[test]
fn an_address_set_again_replaces_the_first() {
    let (mut f, _dir, bus) = on_its_own_bus();
    let _applet = an_applet(&bus);
    let id = a_qt_client(&mut f);
    let caller = a_caller(&mut f, &bus);
    let dolphin = map_with_menu(&mut f, id, "org.kde.dolphin", None);
    f.client(id)
        .set_appmenu_address(dolphin.xdg(), ":1.77", "/MenuBar/1");

    f.client(id)
        .set_appmenu_address(dolphin.xdg(), ":1.77", "/MenuBar/2");
    f.sync(dolphin);

    let dolphins = dbusmenu(":1.77", "/MenuBar/2", "org.kde.dolphin");
    assert_eq!(global_menu(&mut f, &caller), dolphins);
}

#[test]
fn a_released_appmenu_takes_the_menu_away() {
    let (mut f, _dir, bus) = on_its_own_bus();
    let _applet = an_applet(&bus);
    let id = a_qt_client(&mut f);
    let caller = a_caller(&mut f, &bus);
    let dolphin = map_with_menu(&mut f, id, "org.kde.dolphin", None);
    f.client(id)
        .set_appmenu_address(dolphin.xdg(), ":1.77", "/MenuBar/1");
    f.sync(dolphin);

    f.client(id).release_appmenu(dolphin.xdg());
    f.sync(dolphin);

    assert_eq!(global_menu(&mut f, &caller), no_menu("org.kde.dolphin"));
}

#[test]
fn an_address_that_is_no_bus_name_and_object_path_is_no_menu() {
    let (mut f, _dir, bus) = on_its_own_bus();
    let _applet = an_applet(&bus);
    let id = a_qt_client(&mut f);
    let caller = a_caller(&mut f, &bus);
    let dolphin = map_with_menu(&mut f, id, "org.kde.dolphin", None);

    f.client(id)
        .set_appmenu_address(dolphin.xdg(), ":1.77", "MenuBar 1");
    f.sync(dolphin);

    assert_eq!(global_menu(&mut f, &caller), no_menu("org.kde.dolphin"));

    f.client(id)
        .set_appmenu_address(dolphin.xdg(), "not a name", "/MenuBar/1");
    f.sync(dolphin);

    assert_eq!(global_menu(&mut f, &caller), no_menu("org.kde.dolphin"));
}
