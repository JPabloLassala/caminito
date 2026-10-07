//! `GetActiveToplevel` on `org.mate.Caminito` (104): the window `mate-screenshot --window`
//! is of, named by its ext-foreign-toplevel-list identifier (65), as mate-screenshot's
//! carried patch asks for it over the bus. The caller is the test's own process, as the
//! fixture's clients are, so the windows of another are mapped with `map_for`.

use caminito_test_client::dbus::caminito_proxy;

use super::fixture::{
    ClientId, ClientWindow, Fixture, KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT, KEY_TAB, Kind,
    WindowSpec, on_both_kinds,
};

/// A process that isn't the caller's.
fn another_process() -> u32 {
    std::process::id() + 1
}

/// Maps a window of `kind` titled `title`, which Caminito takes for one of the process
/// `process`: its client's, or its `_NET_WM_PID`.
fn map_for(f: &mut Fixture, kind: Kind, process: u32, title: &'static str) -> ClientWindow {
    let spec = WindowSpec {
        title: Some(title),
        ..WindowSpec::default()
    };
    match kind {
        Kind::Xdg => {
            let client = f.connect_as_process(process);
            f.map(client, spec)
        }
        Kind::X11 => {
            let client = f.connect(Kind::X11);
            let spec = WindowSpec {
                cardinals: vec![("_NET_WM_PID", vec![process])],
                ..spec
            };
            f.map(client, spec)
        }
    }
}

/// A client that has bound ext-foreign-toplevel-list, as mate-screenshot does to find the
/// window it's told of.
fn lister(f: &mut Fixture) -> ClientId {
    let client = f.connect(Kind::Xdg);
    f.client(client).bind_foreign_toplevel_list();
    f.roundtrip(client);
    client
}

/// The title of the window `GetActiveToplevel` names to a caller at `bus`, as `lister`
/// finds it by the identifier, or `None` for the empty string.
fn active_toplevel(f: &mut Fixture, bus: &str, lister: ClientId) -> Option<String> {
    let bus = bus.to_owned();
    let identifier =
        f.dbus_call(move || caminito_proxy(&bus).unwrap().get_active_toplevel().unwrap());
    if identifier.is_empty() {
        return None;
    }
    f.roundtrip(lister);
    let listed = &f.client(lister).foreign_toplevels().listed;
    let named = listed
        .iter()
        .find(|toplevel| toplevel.identifier.as_deref() == Some(&identifier))
        .expect("the identifier is of a listed toplevel");
    assert!(!named.closed);
    named.title.clone()
}

on_both_kinds! {
    fn it_names_the_focused_window(kind: Kind) {
        let mut f = Fixture::new();
        f.add_output((1920, 1080), 1.);
        map_for(&mut f, kind, another_process(), "Caja");
        map_for(&mut f, kind, another_process(), "Pluma");
        let lister = lister(&mut f);
        let bus = f.start_dbus();

        assert_eq!(active_toplevel(&mut f, &bus, lister).as_deref(), Some("Pluma"));

        f.chord(&[KEY_LEFTALT, KEY_TAB]);

        assert_eq!(active_toplevel(&mut f, &bus, lister).as_deref(), Some("Caja"));
    }
}

on_both_kinds! {
    /// mate-screenshot's dialog has the focus when its *Take Screenshot* is clicked.
    fn with_the_callers_own_window_focused_it_names_the_one_focused_before(kind: Kind) {
        let mut f = Fixture::new();
        f.add_output((1920, 1080), 1.);
        map_for(&mut f, kind, another_process(), "Caja");
        map_for(&mut f, kind, another_process(), "Pluma");
        // Caja was focused after Pluma.
        f.chord(&[KEY_LEFTALT, KEY_TAB]);
        map_for(&mut f, kind, std::process::id(), "Take Screenshot");
        let lister = lister(&mut f);
        let bus = f.start_dbus();

        assert_eq!(active_toplevel(&mut f, &bus, lister).as_deref(), Some("Caja"));
    }
}

on_both_kinds! {
    fn with_no_other_window_it_names_none(kind: Kind) {
        let mut f = Fixture::new();
        f.add_output((1920, 1080), 1.);
        let lister = lister(&mut f);
        let bus = f.start_dbus();
        assert_eq!(active_toplevel(&mut f, &bus, lister), None);

        map_for(&mut f, kind, std::process::id(), "Take Screenshot");

        assert_eq!(active_toplevel(&mut f, &bus, lister), None);
    }
}

on_both_kinds! {
    /// A window screenshot is of a window on screen, as Marco's active window is.
    fn a_window_on_another_workspace_isnt_named(kind: Kind) {
        let mut f = Fixture::new();
        f.add_output((1920, 1080), 1.);
        map_for(&mut f, kind, another_process(), "Caja");
        let lister = lister(&mut f);
        let bus = f.start_dbus();

        f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);

        assert_eq!(active_toplevel(&mut f, &bus, lister), None);
    }
}
