//! Powering the outputs off when idle (129): at mate-power-manager's `sleep-display-ac`, or
//! `sleep-display-battery` while a stand-in UPower says `OnBattery`, never at 0, and not
//! while an inhibitor holds idle off, unless the session is locked. Activity powers them on,
//! and no client sees an output go or come. The headless backend stands in for the tty's
//! DPMS: it records whether the monitors are powered, and keeps their modes as they were.
//! Each test moves Caminito's clock on itself.

use std::time::Duration;

use serde_json::Value as Json;

use super::fixture::{Bus, ClientId, ClientWindow, Fixture, KEY_A, Kind, WindowSpec};
use crate::scene::SceneLayer;
use crate::settings::Value;
use crate::settings::translate::{CAMINITO, POWER_MANAGER, SCREENSAVER, SESSION};

const MINUTE: Duration = Duration::from_secs(60);
const SECOND: Duration = Duration::from_secs(1);

/// What an empty output shows, and a blanked or powered-off one.
const CLEAR: u32 = 0x333333;
const BLACK: u32 = 0x000000;

/// Where the tests look at the output: where no window is, away from the cursor.
const EMPTY_SPOT: (i32, i32) = (1800, 1000);

const UPOWER: &str = "org.freedesktop.UPower";
const UPOWER_PATH: &str = "/org/freedesktop/UPower";

fn setting(schema: &'static str, key: &str, value: Value) -> (&'static str, String, Value) {
    (schema, key.into(), value)
}

/// One minute to the blank, two to the power-off on AC and three on battery, no lock, and
/// `more`.
fn settings(more: Vec<(&'static str, String, Value)>) -> Vec<(&'static str, String, Value)> {
    let mut settings = vec![
        setting(SESSION, "idle-delay", Value::Int(1)),
        setting(SCREENSAVER, "idle-activation-enabled", Value::Bool(true)),
        setting(SCREENSAVER, "lock-enabled", Value::Bool(false)),
        setting(POWER_MANAGER, "sleep-display-ac", Value::Int(120)),
        setting(POWER_MANAGER, "sleep-display-battery", Value::Int(180)),
        // A locker that runs until its stdin closes, as the fixture's drop does.
        setting(
            CAMINITO,
            "locker-command",
            Value::StrList(vec!["sh".into(), "-c".into(), "exec cat >/dev/null".into()]),
        ),
    ];
    settings.extend(more);
    settings
}

/// Caminito with one output, on [`settings`] with `more`.
fn fixture(more: Vec<(&'static str, String, Value)>) -> Fixture {
    let mut f = Fixture::with_settings(settings(more));
    f.add_output((1920, 1080), 1.0);
    f.dispatch();
    f
}

/// The dump's `idle`.
fn idle(f: &Fixture) -> Json {
    let dump: Json =
        serde_json::from_str(&f.state.caminito.state_dump()).expect("the dump is JSON");
    dump["idle"].clone()
}

/// Whether the outputs are powered off, as the dump says and the backend has them.
fn powered_off(f: &mut Fixture) -> bool {
    let dumped = idle(f)["powered_off"]
        .as_bool()
        .expect("the dump says whether the outputs are powered off");
    let backend = f.state.backend.headless_mut().is_powered_off();
    assert_eq!(dumped, backend, "the dump and the backend agree");
    dumped
}

/// A client with a window, which has the keyboard.
fn a_window(f: &mut Fixture) -> (ClientId, ClientWindow) {
    let id = f.connect(Kind::Xdg);
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    (id, window)
}

#[test]
fn the_outputs_power_off_at_sleep_display_ac() {
    let mut f = fixture(Vec::new());

    f.advance_clock(MINUTE);
    assert!(!powered_off(&mut f), "the blank comes first");
    f.advance_clock(MINUTE - SECOND);
    assert!(!powered_off(&mut f));

    f.advance_clock(SECOND);
    assert!(powered_off(&mut f));
}

#[test]
fn a_key_powers_the_outputs_on_and_restarts_the_count() {
    let mut f = fixture(Vec::new());
    f.advance_clock(2 * MINUTE);

    f.key(KEY_A, true);
    f.key(KEY_A, false);

    assert!(!powered_off(&mut f));
    f.wait_for_pixel(EMPTY_SPOT.0, EMPTY_SPOT.1, CLEAR);
    f.advance_clock(2 * MINUTE - SECOND);
    assert!(!powered_off(&mut f));
    f.advance_clock(SECOND);
    assert!(powered_off(&mut f));
}

#[test]
fn a_pointer_motion_powers_the_outputs_on() {
    let mut f = fixture(Vec::new());
    f.pointer_move(500.0, 500.0);
    f.advance_clock(2 * MINUTE);

    f.pointer_move_by(1.0, 0.0);

    assert!(!powered_off(&mut f));
}

#[test]
fn a_sleep_display_of_0_never_powers_off() {
    let mut f = fixture(vec![setting(
        POWER_MANAGER,
        "sleep-display-ac",
        Value::Int(0),
    )]);
    f.advance_clock(600 * MINUTE);
    assert!(!powered_off(&mut f));
}

/// mate-power-manager's DPMS doesn't ask whether the screensaver blanks: the outputs go off
/// with nothing but the clear colour on them made black.
#[test]
fn the_outputs_power_off_with_idle_activation_off() {
    let mut f = fixture(vec![setting(
        SCREENSAVER,
        "idle-activation-enabled",
        Value::Bool(false),
    )]);
    let output = f.state.caminito.outputs[0].clone();

    f.advance_clock(2 * MINUTE);

    assert!(powered_off(&mut f));
    assert_eq!(idle(&f)["blanked"], false);
    // Nested, powering off is the blank and nothing more (129).
    assert_eq!(f.state.scene_layers(&output), [SceneLayer::Blank]);
    f.wait_for_pixel(EMPTY_SPOT.0, EMPTY_SPOT.1, BLACK);
}

#[test]
fn an_idle_inhibitor_holds_the_power_off_off() {
    let mut f = fixture(Vec::new());
    let (id, window) = a_window(&mut f);
    let surface = f.client(id).window(window.xdg()).surface.clone();
    f.client(id).inhibit_idle(&surface);
    f.roundtrip(id);
    assert_eq!(idle(&f)["inhibited"], true);

    f.advance_clock(600 * MINUTE);

    assert!(!powered_off(&mut f));
}

/// The timers run without inhibitors while the session is locked (12).
#[test]
fn while_locked_an_idle_inhibitor_holds_nothing_off() {
    let mut f = fixture(Vec::new());
    let (id, window) = a_window(&mut f);
    let surface = f.client(id).window(window.xdg()).surface.clone();
    f.client(id).inhibit_idle(&surface);
    f.roundtrip(id);

    assert!(f.state.request_lock());
    f.dispatch();
    assert_eq!(idle(&f)["inhibited"], false);

    f.advance_clock(2 * MINUTE);
    assert!(powered_off(&mut f));
}

/// Across a power-off and a power-on, two monitors keep their outputs, their modes and
/// their places: no `wl_output` global goes or comes, output management sends nothing, and
/// a window on them neither leaves nor enters one.
#[test]
fn no_client_sees_an_output_go_or_come() {
    let mut f = Fixture::with_settings(settings(Vec::new()));
    let aoc = f.plug((527, 296), (1920, 1080));
    let lg = f.plug((600, 340), (3840, 2160));
    f.dispatch();
    let (id, window) = a_window(&mut f);
    f.client(id).bind_outputs();
    f.client(id).bind_output_management();
    f.roundtrip(id);
    let globals = f.client(id).state.globals.clone();
    let serial = f.client(id).output_serial();
    let heads = caminito_test_client::output_management::format_heads(
        &f.client(id).output_management().heads,
    );
    let output_events = f.client(id).window(window.xdg()).output_events.clone();
    let mode_sets = (f.mode_sets(&aoc), f.mode_sets(&lg));
    let layout = f.state.caminito.state_dump();

    f.advance_clock(2 * MINUTE);
    assert!(powered_off(&mut f));
    f.key(KEY_A, true);
    f.key(KEY_A, false);
    assert!(!powered_off(&mut f));
    f.roundtrip(id);

    assert_eq!(f.client(id).state.globals, globals);
    assert_eq!(f.client(id).output_serial(), serial);
    assert_eq!(
        caminito_test_client::output_management::format_heads(
            &f.client(id).output_management().heads
        ),
        heads
    );
    assert_eq!(
        f.client(id).window(window.xdg()).output_events,
        output_events
    );
    assert_eq!((f.mode_sets(&aoc), f.mode_sets(&lg)), mode_sets);
    let outputs = |dump: &str| serde_json::from_str::<Json>(dump).unwrap()["outputs"].clone();
    assert_eq!(outputs(&f.state.caminito.state_dump()), outputs(&layout));
}

// UPower's `OnBattery` picks the key.

/// `org.freedesktop.UPower`'s property Caminito reads.
struct UPowerInterface {
    on_battery: bool,
}

#[zbus::interface(name = "org.freedesktop.UPower")]
impl UPowerInterface {
    #[zbus(property)]
    fn on_battery(&self) -> bool {
        self.on_battery
    }
}

/// A stand-in UPower on a bus of the test's own, the system bus as Caminito sees it.
struct UPower {
    connection: zbus::blocking::Connection,
}

impl UPower {
    fn on(bus: &Bus, on_battery: bool) -> Self {
        let connection = zbus::blocking::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .name(UPOWER)
            .unwrap()
            .serve_at(UPOWER_PATH, UPowerInterface { on_battery })
            .unwrap()
            .build()
            .unwrap();
        UPower { connection }
    }

    /// The machine is unplugged, or plugged in, as UPower says with `PropertiesChanged`.
    fn set_on_battery(&self, on_battery: bool) {
        let iface = self
            .connection
            .object_server()
            .interface::<_, UPowerInterface>(UPOWER_PATH)
            .unwrap();
        iface.get_mut().on_battery = on_battery;
        zbus::block_on(iface.get().on_battery_changed(iface.signal_emitter())).unwrap();
    }
}

/// Caminito on [`settings`], following UPower on a bus of the test's own.
fn with_upower(on_battery: bool) -> (Fixture, Bus, UPower, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let bus = Bus::start(&dir.path().join("bus"));
    let upower = UPower::on(&bus, on_battery);
    let mut f = fixture(Vec::new());
    f.start_upower_at(&bus.address);
    (f, bus, upower, dir)
}

#[test]
fn on_battery_the_outputs_power_off_at_sleep_display_battery() {
    let (mut f, _bus, _upower, _dir) = with_upower(true);
    f.dispatch_until(|f| idle(f)["on_battery"] == true);

    f.advance_clock(2 * MINUTE);
    assert!(!powered_off(&mut f));
    f.advance_clock(MINUTE);
    assert!(powered_off(&mut f));
}

#[test]
fn on_ac_the_outputs_power_off_at_sleep_display_ac() {
    let (mut f, _bus, _upower, _dir) = with_upower(false);
    f.dispatch();
    assert_eq!(idle(&f)["on_battery"], false);

    f.advance_clock(2 * MINUTE);
    assert!(powered_off(&mut f));
}

/// Plugged in past AC's time, the outputs power off at once; unplugged again, the
/// battery's time applies to the next count.
#[test]
fn a_change_of_on_battery_applies_at_once() {
    let (mut f, _bus, upower, _dir) = with_upower(true);
    f.dispatch_until(|f| idle(f)["on_battery"] == true);
    f.advance_clock(2 * MINUTE + 30 * SECOND);
    assert!(!powered_off(&mut f));

    upower.set_on_battery(false);
    f.dispatch_until_mut(powered_off);

    upower.set_on_battery(true);
    f.dispatch_until(|f| idle(f)["on_battery"] == true);
    f.key(KEY_A, true);
    f.key(KEY_A, false);
    f.advance_clock(2 * MINUTE + 30 * SECOND);
    assert!(!powered_off(&mut f));
    f.advance_clock(30 * SECOND);
    assert!(powered_off(&mut f));
}

/// UPower is asked as it comes onto the bus, after Caminito.
#[test]
fn upower_is_asked_when_it_comes() {
    let dir = tempfile::tempdir().unwrap();
    let bus = Bus::start(&dir.path().join("bus"));
    let mut f = fixture(Vec::new());
    f.start_upower_at(&bus.address);
    f.dispatch();
    assert_eq!(idle(&f)["on_battery"], false);

    let _upower = UPower::on(&bus, true);

    f.dispatch_until(|f| idle(f)["on_battery"] == true);
}

#[test]
fn a_new_sleep_display_applies_at_once() {
    let mut f = fixture(Vec::new());
    f.advance_clock(MINUTE);

    f.set_setting(POWER_MANAGER, "sleep-display-ac", Value::Int(30));
    f.dispatch_until_mut(powered_off);
}
