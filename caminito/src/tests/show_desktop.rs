//! Showing the desktop (94): `org.mate.Caminito`'s `ShowingDesktop`, which the panel's Show
//! Desktop button reads, writes and follows, and Ctrl+Alt+D, which is the same thing. The
//! policy cases are `caminito-policy`'s; these are that the bus, the key and the scene agree.
//! The tests of the key, the signal and the workspaces map an xdg window only: nothing in
//! them depends on the kind of window, which the two tests on both kinds cover.

use std::sync::mpsc;
use std::thread;

use caminito_policy::ShowDesktopSkipList;
use caminito_test_client::dbus::caminito_proxy;
use smithay::output::Output;

use super::fixture::{
    ClientWindow, Fixture, KEY_D, KEY_LEFT, KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT, Kind, WindowSpec,
    on_both_kinds,
};
use super::per_output_workspaces::{two_outputs_with, window_at};
use crate::dbus::caminito::PATH;
use crate::output::output_id;
use crate::settings::Value;
use crate::settings::translate::MARCO_GENERAL;
use crate::state::SEAT;

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

// Per-output workspaces (224): the property is true while every output's active workspace
// shows its desktop. On one kind, as the workspaces are the same for both.

/// Two outputs with `workspaces-per-output` as `per_output` says, a window on each, both on
/// the first workspace, and Caminito on the bus at the name returned.
fn a_window_on_each_output(per_output: bool) -> (Fixture, [ClientWindow; 2], Output, String) {
    let (mut f, client, _left, right) = two_outputs_with(Kind::Xdg, per_output, vec![]);
    let on_left = window_at(&mut f, client, (100, 100), 0);
    let on_right = window_at(&mut f, client, (2100, 100), 0);
    let bus = f.start_dbus();
    (f, [on_left, on_right], right, bus)
}

/// Switches `output`, and with `workspaces-per-output` off every output, to `workspace`.
fn switch(f: &mut Fixture, output: &Output, workspace: usize) {
    let policy = &mut f.state.caminito.policy;
    policy.switch_output_workspace(output_id(output), workspace);
    f.dispatch();
}

#[test]
fn the_property_is_true_while_every_outputs_active_workspace_shows_its_desktop() {
    let (mut f, [on_left, on_right], right, bus) = a_window_on_each_output(true);
    let listener = listen(&mut f, &bus, 4);

    write(&mut f, &bus, true);
    assert!(read(&mut f, &bus));
    assert!(!showing(&f, on_left) && !showing(&f, on_right));

    // The right output goes to a workspace that doesn't show its desktop, and comes back.
    switch(&mut f, &right, 1);
    assert!(!read(&mut f, &bus));
    assert!(!showing(&f, on_left));
    switch(&mut f, &right, 0);
    assert!(read(&mut f, &bus));

    // A window activated on the left output ends it there, and the right one keeps it.
    let (id, now) = (f.pid(on_left), f.state.caminito.now());
    f.state.caminito.policy.activate(SEAT, id, now);
    f.dispatch();
    assert!(!read(&mut f, &bus));
    assert!(showing(&f, on_left) && !showing(&f, on_right));
    assert!(!f.state.caminito.policy.is_minimized(f.pid(on_right)));

    assert_eq!(heard(&mut f, listener), [true, false, true, false]);
}

#[test]
fn with_the_key_off_the_property_is_the_one_workspaces() {
    let (mut f, [on_left, on_right], right, bus) = a_window_on_each_output(false);
    let listener = listen(&mut f, &bus, 4);

    write(&mut f, &bus, true);
    assert!(read(&mut f, &bus));
    assert!(!showing(&f, on_left) && !showing(&f, on_right));

    // A switch of one output is a switch of both.
    switch(&mut f, &right, 1);
    assert!(!read(&mut f, &bus));
    switch(&mut f, &right, 0);
    assert!(read(&mut f, &bus));

    // A window activated ends it for both outputs, and the other window is minimised.
    let (id, now) = (f.pid(on_left), f.state.caminito.now());
    f.state.caminito.policy.activate(SEAT, id, now);
    f.dispatch();
    assert!(!read(&mut f, &bus));
    assert!(showing(&f, on_left));
    assert!(f.state.caminito.policy.is_minimized(f.pid(on_right)));

    assert_eq!(heard(&mut f, listener), [true, false, true, false]);
}

// `show-desktop-skip-list` (164): a native window listed by its app id, and an X11 one by
// `WM_CLASS`'s class, stay whichever way the desktop is shown. Policy's tests cover the
// list's other cases.

/// A window of `kind` that `show-desktop-skip-list` lists, mapped and focused over one it
/// doesn't, with Caminito on the bus at the name returned.
struct Skipping {
    f: Fixture,
    listed: ClientWindow,
    unlisted: ClientWindow,
    output: Output,
    bus: String,
    /// The scene's element count with both windows showing.
    both: usize,
}

fn skipping(kind: Kind) -> Skipping {
    let mut f = Fixture::with_settings(vec![(
        MARCO_GENERAL,
        "show-desktop-skip-list".into(),
        Value::Str("caja, Conky".into()),
    )]);
    f.add_output((1920, 1080), 1.0);
    let client = f.connect(kind);
    let unlisted = f.map(
        client,
        WindowSpec {
            app_id: Some("mate-terminal"),
            ..WindowSpec::plain((400, 300))
        },
    );
    let listed = f.map(
        client,
        WindowSpec {
            app_id: Some("conky"),
            ..WindowSpec::plain((300, 200))
        },
    );
    let output = f.state.caminito.outputs[0].clone();
    let bus = f.start_dbus();
    let both = f.state.scene_element_count(&output);
    Skipping {
        f,
        listed,
        unlisted,
        output,
        bus,
        both,
    }
}

fn showing(f: &Fixture, window: ClientWindow) -> bool {
    f.state.caminito.policy.is_showing(f.pid(window))
}

impl Skipping {
    /// That only the listed window shows, with the keyboard, as the scene draws it.
    fn assert_only_the_listed_window_shows(&mut self) {
        let Self {
            f,
            listed,
            unlisted,
            output,
            ..
        } = self;
        assert!(showing(f, *listed));
        assert!(!showing(f, *unlisted));
        assert!(f.has_keyboard(*listed));
        let count = f.state.scene_element_count(output);
        assert!(1 < count && count < self.both, "{count} of {}", self.both);
    }

    /// That both windows show again, the listed one with the keyboard.
    fn assert_both_show(&mut self) {
        let Self {
            f,
            listed,
            unlisted,
            output,
            ..
        } = self;
        assert!(showing(f, *listed) && showing(f, *unlisted));
        assert!(f.has_keyboard(*listed));
        assert_eq!(f.state.scene_element_count(output), self.both);
    }
}

on_both_kinds! {
    fn a_listed_window_stays_through_ctrl_alt_d(kind: Kind) {
        let mut s = skipping(kind);

        s.f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_D]);
        assert!(read(&mut s.f, &s.bus));
        s.assert_only_the_listed_window_shows();

        s.f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_D]);
        assert!(!read(&mut s.f, &s.bus));
        s.assert_both_show();
    }
}

on_both_kinds! {
    fn a_listed_window_stays_through_the_property(kind: Kind) {
        let mut s = skipping(kind);

        write(&mut s.f, &s.bus, true);
        s.assert_only_the_listed_window_shows();

        write(&mut s.f, &s.bus, false);
        s.assert_both_show();
    }
}

/// The key is read live, and the same for both kinds of window, so this maps one.
#[test]
fn a_change_of_the_list_applies_at_the_next_show_desktop() {
    let mut s = skipping(Kind::Xdg);
    s.f.set_setting(
        MARCO_GENERAL,
        "show-desktop-skip-list",
        Value::Str(String::new()),
    );
    s.f.dispatch_until(|f| {
        f.state.caminito.settings.policy.show_desktop_skip_list == ShowDesktopSkipList::default()
    });

    write(&mut s.f, &s.bus, true);

    assert!(desktop_is_bare(&mut s.f, &s.output));
    assert!(!s.f.has_keyboard(s.listed));
}
