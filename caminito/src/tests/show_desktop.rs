//! Showing the desktop (94): `org.mate.Caminito`'s `ShowingDesktop`, which the panel's Show
//! Desktop button reads, writes and follows, and Ctrl+Alt+D, which is the same thing. The
//! policy cases are `caminito-policy`'s; these are that the bus, the key and the scene agree.
//! The tests of the key, the signal and the workspaces map an xdg window only: nothing in
//! them depends on the kind of window, which the two tests on both kinds cover.

use std::sync::mpsc;
use std::thread;

use caminito_test_client::dbus::caminito_proxy;
use smithay::output::Output;

use super::fixture::{
    ClientWindow, Fixture, KEY_D, KEY_LEFT, KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT, Kind, WindowSpec,
    on_both_kinds,
};
use crate::dbus::caminito::PATH;

/// A window of `kind` on the first workspace, with Caminito on the bus at the name returned.
fn one_window(kind: Kind) -> (Fixture, ClientWindow, Output, String) {
    let (mut f, client) = Fixture::connected(kind);
    let window = f.map(client, WindowSpec::plain((400, 300)));
    let output = f.state.caminito.outputs[0].clone();
    let bus = f.start_dbus();
    (f, window, output, bus)
}

fn read(f: &mut Fixture, bus: &str) -> bool {
    let bus = bus.to_owned();
    f.dbus_call(move || caminito_proxy(&bus).unwrap().showing_desktop().unwrap())
}

fn write(f: &mut Fixture, bus: &str, showing: bool) {
    let bus = bus.to_owned();
    f.dbus_call(move || {
        let proxy = caminito_proxy(&bus).unwrap();
        proxy.set_showing_desktop(showing).unwrap();
    });
}

/// `showing_desktop`, from the dump.
fn dumped(f: &Fixture) -> bool {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    dump["showing_desktop"].as_bool().unwrap()
}

/// Whether the scene has only the cursor.
fn desktop_is_bare(f: &mut Fixture, output: &Output) -> bool {
    f.state.scene_element_count(output) == 1
}

/// Listens on another thread for `org.mate.Caminito` at `bus` to signal `ShowingDesktop`
/// `count` times, and returns a handle to what each signal said, in order. It returns
/// once the thread listens. It reads the signals themselves, not a proxy's cache of the
/// property, so that one sent twice is heard twice.
fn listen(f: &mut Fixture, bus: &str, count: usize) -> thread::JoinHandle<Vec<bool>> {
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
                let showing = args.changed_properties().get("ShowingDesktop")?;
                Some(bool::try_from(showing).unwrap())
            })
            .take(count)
            .collect()
    });
    f.dispatch_until(|_| is_listening.try_recv().is_ok());
    listener
}

/// What a listener from `listen` heard.
fn heard(f: &mut Fixture, listener: thread::JoinHandle<Vec<bool>>) -> Vec<bool> {
    f.dispatch_until(|_| listener.is_finished());
    listener.join().unwrap()
}

on_both_kinds! {
    fn writing_the_property_hides_the_windows_and_writing_it_again_brings_them_back(kind: Kind) {
        let (mut f, window, output, bus) = one_window(kind);
        assert!(!read(&mut f, &bus));
        assert!(f.has_keyboard(window));

        write(&mut f, &bus, true);

        assert!(read(&mut f, &bus));
        assert!(dumped(&f));
        assert!(desktop_is_bare(&mut f, &output));
        assert!(!f.has_keyboard(window));

        write(&mut f, &bus, false);

        assert!(!read(&mut f, &bus));
        assert!(!dumped(&f));
        assert!(!desktop_is_bare(&mut f, &output));
        assert!(f.has_keyboard(window));
    }
}

on_both_kinds! {
    fn a_new_window_ends_it_and_the_property_is_signalled(kind: Kind) {
        let (mut f, window, _output, bus) = one_window(kind);
        write(&mut f, &bus, true);
        let listener = listen(&mut f, &bus, 1);

        let new = f.map(window.client(), WindowSpec::plain((300, 200)));

        assert_eq!(heard(&mut f, listener), [false]);
        assert!(!read(&mut f, &bus));
        assert!(f.has_keyboard(new));
    }
}

/// zbus signals a property its setter changed, and the refresh pass mustn't as well.
#[test]
fn writing_the_property_signals_it_once() {
    let (mut f, _window, _output, bus) = one_window(Kind::Xdg);
    let listener = listen(&mut f, &bus, 2);

    write(&mut f, &bus, true);
    write(&mut f, &bus, false);

    assert_eq!(heard(&mut f, listener), [true, false]);
}

/// The key is the same for both kinds of window, so this maps one.
#[test]
fn ctrl_alt_d_changes_the_property_and_signals_it_once() {
    let (mut f, window, output, bus) = one_window(Kind::Xdg);
    let listener = listen(&mut f, &bus, 2);

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_D]);
    assert!(read(&mut f, &bus));
    assert!(desktop_is_bare(&mut f, &output));

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_D]);
    assert!(!read(&mut f, &bus));
    assert!(f.has_keyboard(window));

    // A signal sent twice for the first press would be heard as `[true, true]`.
    assert_eq!(heard(&mut f, listener), [true, false]);
}

/// What the panel's button does: it follows the key, and the key undoes what it wrote.
#[test]
fn the_key_and_the_property_undo_each_other() {
    let (mut f, window, output, bus) = one_window(Kind::Xdg);

    write(&mut f, &bus, true);
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_D]);
    assert!(!read(&mut f, &bus));
    assert!(f.has_keyboard(window));

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_D]);
    write(&mut f, &bus, false);
    assert!(!desktop_is_bare(&mut f, &output));
}

/// Workspaces are the same for both kinds of window, so this maps one.
#[test]
fn switching_workspace_signals_the_new_workspaces_value() {
    let (mut f, _window, _output, bus) = one_window(Kind::Xdg);
    write(&mut f, &bus, true);
    let listener = listen(&mut f, &bus, 2);

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    assert!(!read(&mut f, &bus));

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFT]);
    assert!(read(&mut f, &bus));

    assert_eq!(heard(&mut f, listener), [false, true]);
}

#[test]
fn switching_between_workspaces_that_agree_signals_nothing() {
    let (mut f, _window, _output, bus) = one_window(Kind::Xdg);
    let listener = listen(&mut f, &bus, 1);

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    assert!(!read(&mut f, &bus));
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_D]);

    // The switch sent none, or it would be heard first, as `false`.
    assert_eq!(heard(&mut f, listener), [true]);
}
