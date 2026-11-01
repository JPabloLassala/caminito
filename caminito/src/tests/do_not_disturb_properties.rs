//! What caminito-notifications turns Do Not Disturb on for (ADR 0051, CMN-264), which
//! Caminito publishes on `org.mate.Caminito` and decides nothing about: whether the output
//! under the pointer has a fullscreen window and which it is, which windows are fullscreen on
//! any output (CMN-290), and whether a screencast is running, which is while any
//! ext-image-copy-capture session is open, a screenshot's included.

use std::sync::mpsc;
use std::thread;

use caminito_test_client::dbus::caminito_proxy;

use super::fixture::{Fixture, Kind, WindowSpec};
use super::per_output_workspaces::two_outputs_with;
use crate::dbus::caminito::PATH;

fn screencasting(f: &mut Fixture, bus: &str) -> bool {
    let bus = bus.to_owned();
    f.dbus_call(move || caminito_proxy(&bus).unwrap().screencasting().unwrap())
}

fn fullscreen(f: &mut Fixture, bus: &str) -> bool {
    let bus = bus.to_owned();
    f.dbus_call(move || {
        caminito_proxy(&bus)
            .unwrap()
            .pointer_output_fullscreen()
            .unwrap()
    })
}

/// `PointerOutputFullscreenWindow` at `bus`.
fn fullscreen_window(f: &mut Fixture, bus: &str) -> String {
    let bus = bus.to_owned();
    f.dbus_call(move || {
        caminito_proxy(&bus)
            .unwrap()
            .pointer_output_fullscreen_window()
            .unwrap()
    })
}

/// `FullscreenWindows` at `bus`.
fn fullscreen_windows(f: &mut Fixture, bus: &str) -> Vec<String> {
    let bus = bus.to_owned();
    f.dbus_call(move || caminito_proxy(&bus).unwrap().fullscreen_windows().unwrap())
}

/// Listens on another thread for `org.mate.Caminito` at `bus` to signal the boolean
/// `property` `count` times, and returns a handle to what each signal said, in order. It
/// returns once the thread listens.
fn listen(
    f: &mut Fixture,
    bus: &str,
    property: &'static str,
    count: usize,
) -> thread::JoinHandle<Vec<bool>> {
    listen_as(f, bus, property, count, |value| {
        bool::try_from(value).unwrap()
    })
}

/// The same for a property whose value `read` makes of what a signal said.
fn listen_as<T: Send + 'static>(
    f: &mut Fixture,
    bus: &str,
    property: &'static str,
    count: usize,
    read: fn(&zbus::zvariant::Value<'_>) -> T,
) -> thread::JoinHandle<Vec<T>> {
    let bus = bus.to_owned();
    let (listening, is_listening) = mpsc::channel();
    let listener = thread::spawn(move || {
        let connection = caminito_test_client::dbus::session().unwrap();
        let properties = zbus::blocking::fdo::PropertiesProxy::builder(&connection)
            .destination(bus)
            .unwrap()
            .path(PATH)
            .unwrap()
            .build()
            .unwrap();
        let changes = properties.receive_properties_changed().unwrap();
        listening.send(()).unwrap();
        changes
            .filter_map(|change| {
                let args = change.args().unwrap();
                let value = args.changed_properties().get(property)?;
                Some(read(value))
            })
            .take(count)
            .collect()
    });
    f.dispatch_until(|_| is_listening.try_recv().is_ok());
    listener
}

/// What a listener from `listen` or `listen_as` heard.
fn heard<T: Send + 'static>(f: &mut Fixture, listener: thread::JoinHandle<Vec<T>>) -> Vec<T> {
    f.dispatch_until(|_| listener.is_finished());
    listener.join().unwrap()
}

#[test]
fn screencasting_is_true_exactly_while_a_capture_session_is_open() {
    let mut f = Fixture::new();
    f.add_output((800, 600), 1.0);
    let bus = f.start_dbus();
    let tool = f.connect(Kind::Xdg);
    f.client(tool).bind_outputs();
    f.client(tool).bind_image_capture();
    f.roundtrip(tool);
    assert!(!screencasting(&mut f, &bus));
    let listener = listen(&mut f, &bus, "Screencasting", 2);

    let first = f.client(tool).capture_output(0, false);
    f.roundtrip(tool);
    let second = f.client(tool).capture_output(0, false);
    f.roundtrip(tool);
    assert!(screencasting(&mut f, &bus));
    f.client(tool).stop_capture(first);
    f.roundtrip(tool);
    assert!(screencasting(&mut f, &bus), "one session is still open");
    f.client(tool).stop_capture(second);
    f.roundtrip(tool);

    assert!(!screencasting(&mut f, &bus));
    assert_eq!(heard(&mut f, listener), [true, false]);
}

#[test]
fn pointer_output_fullscreen_follows_the_pointers_output_and_the_windows_state() {
    let (mut f, client, _left, _right) = two_outputs_with(Kind::Xdg, true, vec![]);
    let window = f.map(client, WindowSpec::plain((400, 300)));
    let id = f.pid(window);
    f.pointer_move(100., 100.);
    let bus = f.start_dbus();
    assert!(!fullscreen(&mut f, &bus));
    let listener = listen(&mut f, &bus, "PointerOutputFullscreen", 4);

    f.client(client).set_fullscreen(window.xdg(), None);
    f.dispatch_until(|f| f.state.caminito.policy.is_fullscreen(id));
    assert!(fullscreen(&mut f, &bus));
    // The other output has none.
    f.pointer_move(2500., 100.);
    assert!(!fullscreen(&mut f, &bus));
    f.pointer_move(100., 100.);
    assert!(fullscreen(&mut f, &bus));
    f.client(client).unset_fullscreen(window.xdg());
    f.dispatch_until(|f| !f.state.caminito.policy.is_fullscreen(id));

    assert!(!fullscreen(&mut f, &bus));
    assert_eq!(heard(&mut f, listener), [true, false, true, false]);
}

#[test]
fn the_fullscreen_window_is_named_by_its_identifier_wherever_the_pointer_is() {
    let (mut f, client, _left, _right) = two_outputs_with(Kind::Xdg, true, vec![]);
    let window = f.map(client, WindowSpec::plain((400, 300)));
    let id = f.pid(window);
    let identifier = f
        .state
        .caminito
        .toplevel_identifier(id)
        .expect("a managed window has an identifier");
    f.pointer_move(100., 100.);
    let bus = f.start_dbus();
    assert_eq!(fullscreen_window(&mut f, &bus), "");
    assert!(fullscreen_windows(&mut f, &bus).is_empty());
    let on_pointer_output = listen_as(&mut f, &bus, "PointerOutputFullscreenWindow", 4, |value| {
        String::try_from(value.try_clone().unwrap()).unwrap()
    });
    let on_any_output = listen_as(&mut f, &bus, "FullscreenWindows", 2, |value| {
        Vec::<String>::try_from(value.try_clone().unwrap()).unwrap()
    });

    f.client(client).set_fullscreen(window.xdg(), None);
    f.dispatch_until(|f| f.state.caminito.policy.is_fullscreen(id));
    assert_eq!(fullscreen_window(&mut f, &bus), identifier);
    assert_eq!(
        fullscreen_windows(&mut f, &bus),
        std::slice::from_ref(&identifier)
    );
    // The other output has none, and the window is fullscreen all the same.
    f.pointer_move(2500., 100.);
    assert_eq!(fullscreen_window(&mut f, &bus), "");
    assert_eq!(
        fullscreen_windows(&mut f, &bus),
        std::slice::from_ref(&identifier)
    );
    f.pointer_move(100., 100.);
    assert_eq!(fullscreen_window(&mut f, &bus), identifier);
    f.client(client).unset_fullscreen(window.xdg());
    f.dispatch_until(|f| !f.state.caminito.policy.is_fullscreen(id));

    assert_eq!(fullscreen_window(&mut f, &bus), "");
    assert!(fullscreen_windows(&mut f, &bus).is_empty());
    assert_eq!(
        heard(&mut f, on_pointer_output),
        [
            identifier.clone(),
            String::new(),
            identifier.clone(),
            String::new()
        ]
    );
    assert_eq!(
        heard(&mut f, on_any_output),
        [vec![identifier], Vec::<String>::new()]
    );
}

#[test]
fn a_fullscreen_window_is_listed_while_it_is_minimised() {
    let (mut f, client, _left, _right) = two_outputs_with(Kind::Xdg, true, vec![]);
    let window = f.map(client, WindowSpec::plain((400, 300)));
    let id = f.pid(window);
    let identifier = f.state.caminito.toplevel_identifier(id).unwrap();
    f.pointer_move(100., 100.);
    let bus = f.start_dbus();
    f.client(client).set_fullscreen(window.xdg(), None);
    f.dispatch_until(|f| f.state.caminito.policy.is_fullscreen(id));
    assert_eq!(fullscreen_windows(&mut f, &bus), [identifier.clone()]);

    f.client(client).set_minimized(window.xdg());
    f.dispatch_until(|f| f.state.caminito.policy.is_minimized(id));

    // It isn't on the pointer's output any more, but it is fullscreen still.
    assert_eq!(fullscreen_window(&mut f, &bus), "");
    assert_eq!(fullscreen_windows(&mut f, &bus), [identifier]);
}
