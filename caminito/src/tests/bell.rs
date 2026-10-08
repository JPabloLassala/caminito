//! The one bell event (105): `gtk_shell1`, xdg-system-bell and XKB each ring it, for the
//! window the bell names or, when it names none, for the focused window, and the debug dump
//! shows how many have rung and the last one. `gtk_shell1` and xdg-system-bell are Wayland's,
//! and XKB's bell is X's, so each test maps only the kind of window its source is for.

use serde_json::{Value, json};

use super::fixture::{ClientId, ClientWindow, Fixture, Kind, WindowSpec};

/// The dump's bell.
fn bell(f: &Fixture) -> Value {
    let dump: Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    dump["bell"].clone()
}

/// The window the dump has focused.
fn focus(f: &Fixture) -> Value {
    let dump: Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    dump["focus"].clone()
}

/// `count` bells rung, the last from `source` for `window`.
fn rung(count: u64, source: &str, window: Value) -> Value {
    json!({ "count": count, "last": { "source": source, "window": window } })
}

/// Two windows of one client of `kind`, the second focused, so a bell that took the focused
/// window where it should take another, or none, shows.
fn two_windows(kind: Kind) -> (Fixture, ClientId, ClientWindow, ClientWindow) {
    let (mut f, client) = Fixture::connected(kind);
    let first = f.map(client, WindowSpec::default());
    let second = f.map(client, WindowSpec::default());
    assert_eq!(focus(&f), json!(f.pid(second).0));
    (f, client, first, second)
}

#[test]
fn no_bell_has_rung_at_first() {
    let f = Fixture::new();

    assert_eq!(bell(&f), json!({ "count": 0, "last": null }));
}

#[test]
fn a_gtk_shell_bell_for_a_surface_is_for_its_window() {
    let (mut f, client, first, _) = two_windows(Kind::Xdg);

    f.client(client).system_bell(Some(first.xdg()));
    f.roundtrip(client);

    assert_eq!(bell(&f), rung(1, "gtk_shell1", json!(f.pid(first).0)));
}

#[test]
fn a_gtk_shell_bell_for_no_surface_is_for_the_focused_window() {
    let (mut f, client, _, second) = two_windows(Kind::Xdg);

    f.client(client).system_bell(None);
    f.roundtrip(client);

    assert_eq!(bell(&f), rung(1, "gtk_shell1", json!(f.pid(second).0)));
}

#[test]
fn an_xdg_system_bell_for_a_surface_is_for_its_window() {
    let (mut f, client, first, _) = two_windows(Kind::Xdg);

    f.client(client).ring_system_bell(Some(first.xdg()));
    f.roundtrip(client);

    assert_eq!(bell(&f), rung(1, "xdg_system_bell", json!(f.pid(first).0)));
}

#[test]
fn an_xdg_system_bell_for_no_surface_is_for_the_focused_window() {
    let (mut f, client, _, second) = two_windows(Kind::Xdg);

    f.client(client).ring_system_bell(None);
    f.roundtrip(client);

    assert_eq!(bell(&f), rung(1, "xdg_system_bell", json!(f.pid(second).0)));
}

#[test]
fn a_bell_for_a_destroyed_surface_is_for_no_window() {
    let (mut f, client, first, _) = two_windows(Kind::Xdg);
    // The first bell makes the window's `gtk_surface1`, which outlives its surface: GTK3
    // has no request to destroy one at version 1.
    f.client(client).system_bell(Some(first.xdg()));
    f.client(client).destroy_window(first.xdg());
    f.roundtrip(client);

    f.client(client).system_bell(Some(first.xdg()));
    f.roundtrip(client);

    assert_eq!(bell(&f), rung(2, "gtk_shell1", Value::Null));
}

#[test]
fn a_bell_for_a_surface_that_isnt_a_windows_is_for_no_window() {
    let (mut f, client, _, _) = two_windows(Kind::Xdg);
    let surface = f.client(client).create_bare_surface();

    f.client(client).ring_system_bell_for_surface(&surface);
    f.roundtrip(client);

    assert_eq!(bell(&f), rung(1, "xdg_system_bell", Value::Null));
}

#[test]
fn a_bell_for_a_windows_subsurface_is_for_the_window() {
    let (mut f, client, first, _) = two_windows(Kind::Xdg);
    let subsurface = f
        .client(client)
        .add_subsurface(first.xdg(), (10, 10), (20, 20), 0xff0000ff);
    f.roundtrip(client);

    f.client(client).ring_system_bell_for_surface(&subsurface);
    f.roundtrip(client);

    assert_eq!(bell(&f), rung(1, "xdg_system_bell", json!(f.pid(first).0)));
}

#[test]
fn an_x_bell_for_a_window_is_for_that_window() {
    let (mut f, client, first, _) = two_windows(Kind::X11);
    let window = first.x11();

    f.x11_call(client, move |c| c.bell(Some(window)).unwrap());
    f.dispatch_until(|f| bell(f)["count"] == 1);

    assert_eq!(bell(&f), rung(1, "xkb", json!(f.pid(first).0)));
}

#[test]
fn an_x_bell_for_no_window_is_for_the_focused_window() {
    let (mut f, client, _, second) = two_windows(Kind::X11);

    f.x11_call(client, |c| c.bell(None).unwrap());
    f.dispatch_until(|f| bell(f)["count"] == 1);

    assert_eq!(bell(&f), rung(1, "xkb", json!(f.pid(second).0)));
}
