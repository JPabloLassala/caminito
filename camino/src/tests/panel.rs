//! Alt+F1 and Alt+F2 (95): the calls to `org.mate.Panel`, the one popup grab its client
//! gets without a valid serial, and what an unpatched panel leaves. Each test runs a bus of
//! its own, with a stand-in that owns the panel's name, which only one could on the tests'
//! bus. The keys and the grabs are a Wayland client's, so the windows are xdg toplevels
//! only (45).

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use camino_test_client::layer_shell::{LayerId, LayerSpec};
use camino_test_client::{
    Anchor, ConstraintAdjustment, Gravity, InputEvent, PopupId, Positioner, SurfaceRef,
    format_input,
};
use insta::assert_snapshot;
use zbus::blocking::Connection;

use super::fixture::{Bus, ClientId, Fixture, KEY_F1, KEY_F2, KEY_LEFTALT, Kind, Logs, WindowSpec};

const BAR_COLOR: u32 = 0xff00ff00;
const RED: u32 = 0xffff0000;

/// A serial no input event had: what GTK sends for a menu no key or button opened.
const NO_SERIAL: u32 = 0;

/// The calls a stand-in for the panel got, each with its timestamp.
type Calls = Arc<Mutex<Vec<(&'static str, u32)>>>;

/// `org.mate.Panel`, as mate-panel's carried patch exports it.
struct Interface {
    calls: Calls,
    /// Whether the methods fail, as the calls to a panel that hasn't them do.
    fails: bool,
}

impl Interface {
    fn called(&self, method: &'static str, time: u32) -> zbus::fdo::Result<()> {
        self.calls.lock().unwrap().push((method, time));
        if self.fails {
            return Err(zbus::fdo::Error::UnknownMethod(method.into()));
        }
        Ok(())
    }
}

#[zbus::interface(name = "org.mate.Panel")]
impl Interface {
    fn main_menu(&self, time: u32) -> zbus::fdo::Result<()> {
        self.called("MainMenu", time)
    }

    fn run_dialog(&self, time: u32) -> zbus::fdo::Result<()> {
        self.called("RunDialog", time)
    }
}

/// A stand-in for the panel on a test's bus. Its connection is the test's process's, as
/// the fixture's Wayland clients are, so any of them is the panel's client to Camino.
struct Panel {
    calls: Calls,
    _connection: Connection,
}

impl Panel {
    fn on(bus: &Bus, fails: bool) -> Self {
        let calls = Calls::default();
        let interface = Interface {
            calls: calls.clone(),
            fails,
        };
        let connection = zbus::blocking::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .name("org.mate.Panel")
            .unwrap()
            .serve_at("/org/mate/Panel", interface)
            .unwrap()
            .build()
            .unwrap();
        Panel {
            calls,
            _connection: connection,
        }
    }

    fn calls(&self) -> Vec<(&'static str, u32)> {
        self.calls.lock().unwrap().clone()
    }
}

/// Camino with one output on a bus of the test's own, which lasts as long as the directory
/// and the bus do.
fn on_its_own_bus() -> (Fixture, tempfile::TempDir, Bus) {
    let dir = tempfile::tempdir().unwrap();
    let bus = Bus::start(&dir.path().join("bus"));
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.0);
    f.start_dbus_at(&bus.address);
    (f, dir, bus)
}

/// A client with a bar at the top, as the panel's is, that has seen no input.
fn a_bar(f: &mut Fixture, id: ClientId) -> LayerId {
    f.client(id).bind_input();
    f.roundtrip(id);
    f.map_layer(id, LayerSpec::top_bar(28), BAR_COLOR)
}

/// Opens a menu below the bar with a grab at `serial`, and returns whether it took the
/// keyboard, which the grab gives it. A menu that was refused is dismissed.
fn menu_takes_the_keyboard(f: &mut Fixture, id: ClientId, bar: LayerId, serial: u32) -> bool {
    let menu = Positioner {
        size: (300, 200),
        anchor_rect: (90, 0, 20, 28),
        anchor: Anchor::BottomLeft,
        gravity: Gravity::BottomRight,
        constraint_adjustment: ConstraintAdjustment::None,
    };
    f.client(id).state.input.clear();
    let popup = f.client(id).create_layer_popup(bar, menu, Some(serial));
    f.roundtrip(id);
    let entered = entered(f, id, popup);
    assert_ne!(entered, f.client(id).popup(popup).done);
    if entered {
        f.client(id).map_popup(popup, RED);
        f.roundtrip(id);
    }
    entered
}

/// Whether the keyboard entered `popup`.
fn entered(f: &mut Fixture, id: ClientId, popup: PopupId) -> bool {
    let enter = InputEvent::KeyboardEnter {
        surface: SurfaceRef::Popup(popup),
    };
    f.client(id).state.input.contains(&enter)
}

/// Presses Alt+F1 and returns once the stand-in has the call.
fn open_the_main_menu(f: &mut Fixture, panel: &Panel) {
    let calls = panel.calls().len();
    f.chord(&[KEY_LEFTALT, KEY_F1]);
    f.dispatch_until(|_| panel.calls().len() > calls);
}

#[test]
fn alt_f1_and_alt_f2_call_the_panel_and_reach_no_client() {
    let (mut f, _dir, bus) = on_its_own_bus();
    let panel = Panel::on(&bus, false);
    let id = f.connect(Kind::Xdg);
    f.client(id).bind_input();
    f.map(id, WindowSpec::plain((400, 300)));
    f.roundtrip(id);
    f.client(id).state.input.clear();

    f.chord(&[KEY_LEFTALT, KEY_F1]);
    f.dispatch_until(|_| panel.calls().len() == 1);
    f.chord(&[KEY_LEFTALT, KEY_F2]);
    f.dispatch_until(|_| panel.calls().len() == 2);

    // The fixture's keys are 10 ms apart, and each call has its own key's time: the
    // second and the sixth key event.
    assert_eq!(panel.calls(), [("MainMenu", 20), ("RunDialog", 60)]);
    f.roundtrip(id);
    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    key 56 pressed
    key 56 released
    key 56 pressed
    key 56 released
    ");
}

#[test]
fn the_panels_menu_takes_one_grab_without_a_valid_serial() {
    let (mut f, _dir, bus) = on_its_own_bus();
    let panel = Panel::on(&bus, false);
    let id = f.connect(Kind::Xdg);
    let bar = a_bar(&mut f, id);

    open_the_main_menu(&mut f, &panel);

    assert!(menu_takes_the_keyboard(&mut f, id, bar, NO_SERIAL));
    // A press outside it ends the grab and dismisses the menu, well within the second.
    f.click(1900., 1060.);
    assert!(!menu_takes_the_keyboard(&mut f, id, bar, NO_SERIAL));
}

#[test]
fn the_grab_is_refused_once_the_second_has_passed() {
    let (mut f, _dir, bus) = on_its_own_bus();
    let panel = Panel::on(&bus, false);
    let id = f.connect(Kind::Xdg);
    let bar = a_bar(&mut f, id);

    open_the_main_menu(&mut f, &panel);
    // What's tested is that the time ran out, so the test waits it out: the second the
    // panel's client has, and a tenth more for the clock and the loop.
    f.dispatch_for(Duration::from_millis(1100));

    assert!(!menu_takes_the_keyboard(&mut f, id, bar, NO_SERIAL));
}

#[test]
fn the_grab_is_refused_to_another_process_and_kept_for_the_panels() {
    let (mut f, _dir, bus) = on_its_own_bus();
    let panel = Panel::on(&bus, false);
    let id = f.connect(Kind::Xdg);
    let bar = a_bar(&mut f, id);
    // No process of the test's has init's pid.
    let other = f.connect_as_process(1);
    f.client(other).bind_input();
    f.roundtrip(other);
    let others_bar = f.map_layer(other, LayerSpec::top_bar(28), BAR_COLOR);

    open_the_main_menu(&mut f, &panel);

    assert!(!menu_takes_the_keyboard(
        &mut f, other, others_bar, NO_SERIAL
    ));
    assert!(menu_takes_the_keyboard(&mut f, id, bar, NO_SERIAL));
}

#[test]
fn no_grab_without_a_valid_serial_before_the_key() {
    let (mut f, _dir, bus) = on_its_own_bus();
    let _panel = Panel::on(&bus, false);
    let id = f.connect(Kind::Xdg);
    let bar = a_bar(&mut f, id);

    assert!(!menu_takes_the_keyboard(&mut f, id, bar, NO_SERIAL));
}

/// A stand-in for `mate-panel --run-dialog` that writes its arguments to `ran`, in the
/// directory, and Camino set to run it.
fn a_run_dialog_command(f: &mut Fixture, dir: &tempfile::TempDir) -> std::path::PathBuf {
    let ran = dir.path().join("ran");
    let script = dir.path().join("mate-panel");
    let body = format!(
        "#!/bin/sh\necho \"$@\" >'{0}.partial' && mv '{0}.partial' '{0}'\n",
        ran.display()
    );
    fs::write(&script, body).unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    f.state.camino.set_panel_program(script);
    ran
}

#[test]
fn with_no_panel_on_the_bus_alt_f2_spawns_the_run_dialog() {
    let (mut f, dir, _bus) = on_its_own_bus();
    let ran = a_run_dialog_command(&mut f, &dir);

    f.chord(&[KEY_LEFTALT, KEY_F2]);
    f.dispatch_until(|_| ran.exists());

    assert_eq!(fs::read_to_string(ran).unwrap(), "--run-dialog\n");
}

#[test]
fn a_panel_whose_call_fails_has_alt_f2_spawn_the_run_dialog() {
    let (mut f, dir, bus) = on_its_own_bus();
    let panel = Panel::on(&bus, true);
    let ran = a_run_dialog_command(&mut f, &dir);

    f.chord(&[KEY_LEFTALT, KEY_F2]);
    f.dispatch_until(|_| ran.exists());

    assert_eq!(panel.calls(), [("RunDialog", 20)]);
}

#[test]
fn a_failed_call_ends_the_time_for_the_grab() {
    // Before the bus: zbus's tasks carry spans, and one made before the capture isn't one
    // the capture knows.
    let logs = Logs::capture();
    let (mut f, _dir, bus) = on_its_own_bus();
    let panel = Panel::on(&bus, true);
    let id = f.connect(Kind::Xdg);
    let bar = a_bar(&mut f, id);

    open_the_main_menu(&mut f, &panel);
    f.dispatch_until(|_| logs.text().contains("can't open the panel's main menu"));

    assert!(!menu_takes_the_keyboard(&mut f, id, bar, NO_SERIAL));
}

#[test]
fn with_no_panel_on_the_bus_alt_f1_logs_once() {
    let logs = Logs::capture();
    let (mut f, dir, _bus) = on_its_own_bus();
    // The second key's answer is seen by the run dialog it spawns after it.
    let ran = a_run_dialog_command(&mut f, &dir);

    f.chord(&[KEY_LEFTALT, KEY_F1]);
    f.chord(&[KEY_LEFTALT, KEY_F1]);
    f.chord(&[KEY_LEFTALT, KEY_F2]);
    f.dispatch_until(|_| ran.exists());

    let logs = logs.text();
    let logged = logs.matches("can't open the panel's main menu");
    assert_eq!(logged.count(), 1, "{logs}");
}

#[test]
fn alt_f1_while_another_clients_menu_is_open_leaves_that_menu_and_the_panel() {
    let (mut f, _dir, bus) = on_its_own_bus();
    let panel = Panel::on(&bus, false);
    let id = f.connect(Kind::Xdg);
    let bar = a_bar(&mut f, id);
    let app = f.connect(Kind::Xdg);
    f.client(app).bind_input();
    let window = f.map(app, WindowSpec::plain((400, 300)));
    let geometry = f.geometry(window);
    f.click(geometry.loc.x as f64 + 10., geometry.loc.y as f64 + 10.);
    f.roundtrip(app);
    let press = f.client(app).state.last_press_serial.unwrap();
    let place = Positioner {
        size: (300, 200),
        anchor_rect: (10, 20, 30, 40),
        anchor: Anchor::BottomRight,
        gravity: Gravity::BottomRight,
        constraint_adjustment: ConstraintAdjustment::None,
    };
    let menu = f.client(app).create_popup(window.xdg(), place);
    f.client(app).grab_popup(menu, press);
    f.roundtrip(app);
    f.client(app).map_popup(menu, RED);
    f.roundtrip(app);

    open_the_main_menu(&mut f, &panel);

    // The panel's menu is dismissed, and the panel is still connected to hear of it.
    assert!(!menu_takes_the_keyboard(&mut f, id, bar, NO_SERIAL));
    f.roundtrip(app);
    assert!(!f.client(app).popup(menu).done);
}
