//! Input devices' settings through the recording double (53): each device gets its kind's
//! settings when it's added, and again whenever they change. No window is involved.

use std::cell::RefCell;
use std::rc::Rc;

use insta::assert_snapshot;

use super::fixture::{Fixture, Kind, WindowSpec};
use crate::input::devices::{ConfigurableDevice, DeviceKind, Setting};
use crate::settings::Value;
use crate::settings::translate::{MOUSE, TOUCHPAD};

/// A device that records every setting it's given.
struct RecordingDevice {
    id: String,
    kind: DeviceKind,
    log: Rc<RefCell<Vec<Setting>>>,
}

impl ConfigurableDevice for RecordingDevice {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn kind(&self) -> DeviceKind {
        self.kind
    }

    fn apply(&mut self, setting: Setting) {
        self.log.borrow_mut().push(setting);
    }
}

type Log = Rc<RefCell<Vec<Setting>>>;

/// Plugs in a device of `kind`, and returns what it's given.
fn plug(f: &mut Fixture, id: &str, kind: DeviceKind) -> Log {
    let log = Log::default();
    f.state.caminito.add_input_device(Box::new(RecordingDevice {
        id: id.into(),
        kind,
        log: log.clone(),
    }));
    log
}

fn format(log: &Log) -> String {
    log.borrow()
        .iter()
        .map(|setting| format!("{setting:?}\n"))
        .collect()
}

#[test]
fn a_touchpad_gets_the_touchpad_settings_when_its_added() {
    let mut f = Fixture::with_settings(vec![
        (TOUCHPAD, "tap-to-click".into(), Value::Bool(true)),
        (TOUCHPAD, "motion-acceleration".into(), Value::Double(10.0)),
    ]);
    let touchpad = plug(&mut f, "event5", DeviceKind::Touchpad);
    assert_snapshot!(format(&touchpad), @r"
    SendEvents(true)
    LeftHanded(false)
    AccelSpeed(Some(1.0))
    AccelProfile(Default)
    NaturalScroll(false)
    Tap(true)
    DisableWhileTyping(false)
    ClickMethod(ButtonAreas)
    ScrollMethod(TwoFinger)
    ");
}

#[test]
fn a_mouse_gets_the_mouse_settings_when_its_added() {
    let mut f = Fixture::with_settings(vec![(MOUSE, "left-handed".into(), Value::Bool(true))]);
    let mouse = plug(&mut f, "event3", DeviceKind::Mouse);
    assert_snapshot!(format(&mouse), @r"
    LeftHanded(true)
    AccelSpeed(None)
    AccelProfile(Default)
    NaturalScroll(false)
    MiddleEmulation(true)
    ");
}

#[test]
fn a_device_of_no_kind_gets_nothing() {
    let mut f = Fixture::new();
    let keyboard = plug(&mut f, "event2", DeviceKind::Other);
    f.set_setting(TOUCHPAD, "tap-to-click", Value::Bool(true));
    f.dispatch_until(|f| f.state.caminito.settings.devices.touchpad.tap == Some(true));
    assert_eq!(format(&keyboard), "");
}

#[test]
fn a_touchpad_setting_change_reaches_every_touchpad_and_no_mouse() {
    let mut f = Fixture::new();
    let touchpads = [
        plug(&mut f, "event5", DeviceKind::Touchpad),
        plug(&mut f, "event6", DeviceKind::Touchpad),
    ];
    let mouse = plug(&mut f, "event3", DeviceKind::Mouse);
    for log in touchpads.iter().chain([&mouse]) {
        log.borrow_mut().clear();
    }

    f.set_setting(TOUCHPAD, "tap-to-click", Value::Bool(true));
    f.dispatch_until(|_| touchpads.iter().all(|log| !log.borrow().is_empty()));

    for log in &touchpads {
        assert!(log.borrow().contains(&Setting::Tap(true)));
    }
    assert_eq!(format(&mouse), "");
}

#[test]
fn a_mouse_setting_change_reaches_every_mouse_and_the_touchpads_that_follow_it() {
    let mut f = Fixture::new();
    let mice = [
        plug(&mut f, "event3", DeviceKind::Mouse),
        plug(&mut f, "event4", DeviceKind::Mouse),
    ];
    // Its `left-handed` is `mouse` by default.
    let touchpad = plug(&mut f, "event5", DeviceKind::Touchpad);
    for log in mice.iter().chain([&touchpad]) {
        log.borrow_mut().clear();
    }

    f.set_setting(MOUSE, "left-handed", Value::Bool(true));
    f.dispatch_until(|_| {
        mice.iter()
            .chain([&touchpad])
            .all(|log| log.borrow().contains(&Setting::LeftHanded(true)))
    });
}

#[test]
fn a_device_added_after_a_change_gets_the_current_settings() {
    let mut f = Fixture::new();
    f.set_setting(TOUCHPAD, "touchpad-enabled", Value::Bool(false));
    f.dispatch_until(|f| f.state.caminito.settings.devices.touchpad.send_events == Some(false));

    let touchpad = plug(&mut f, "event5", DeviceKind::Touchpad);
    assert!(touchpad.borrow().contains(&Setting::SendEvents(false)));
}

#[test]
fn a_removed_device_gets_no_more_settings() {
    let mut f = Fixture::new();
    let kept = plug(&mut f, "event5", DeviceKind::Touchpad);
    let removed = plug(&mut f, "event6", DeviceKind::Touchpad);
    removed.borrow_mut().clear();
    f.state.caminito.remove_input_device("event6");

    f.set_setting(TOUCHPAD, "tap-to-click", Value::Bool(true));
    f.dispatch_until(|_| kept.borrow().contains(&Setting::Tap(true)));
    assert_eq!(format(&removed), "");
}

/// A 13.3" 2560×1600 laptop panel: 1.80 with a lid switch, 1.50 without (50).
const PANEL: ((i32, i32), (i32, i32)) = ((286, 179), (2560, 1600));

/// The scale of the first output, as the debug dump shows it.
fn scale(f: &Fixture) -> f64 {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    dump["outputs"][0]["scale"].as_f64().unwrap()
}

#[test]
fn a_lid_switch_gives_the_built_in_panel_a_laptops_scale() {
    let mut f = Fixture::new();
    f.plug_internal(PANEL.0, PANEL.1);
    assert_eq!(scale(&f), 1.5);

    plug(&mut f, "event1", DeviceKind::LidSwitch);
    f.dispatch();
    assert_eq!(scale(&f), 1.8);
}

#[test]
fn a_panel_plugged_in_after_the_lid_switch_gets_a_laptops_scale() {
    let mut f = Fixture::new();
    plug(&mut f, "event1", DeviceKind::LidSwitch);
    f.plug_internal(PANEL.0, PANEL.1);
    assert_eq!(scale(&f), 1.8);
}

#[test]
fn the_panel_loses_the_laptops_scale_when_the_lid_switch_goes() {
    let mut f = Fixture::new();
    plug(&mut f, "event1", DeviceKind::LidSwitch);
    f.plug_internal(PANEL.0, PANEL.1);
    f.state.caminito.remove_input_device("event1");
    f.dispatch();
    assert_eq!(scale(&f), 1.5);
}

#[test]
fn a_lid_switch_leaves_a_scale_output_management_set() {
    let mut f = Fixture::new();
    let panel = f.plug_internal(PANEL.0, PANEL.1);
    f.set_output_scale(&panel, 2.0);
    plug(&mut f, "event1", DeviceKind::LidSwitch);
    f.dispatch();
    assert_eq!(scale(&f), 2.0);
}

#[test]
fn a_lid_switch_gets_no_settings() {
    let mut f = Fixture::new();
    let lid = plug(&mut f, "event1", DeviceKind::LidSwitch);
    f.set_setting(MOUSE, "left-handed", Value::Bool(true));
    f.dispatch_until(|f| f.state.caminito.settings.devices.mouse.left_handed);
    assert_eq!(format(&lid), "");
}

/// Switching VTs, libinput unplugs every device and plugs it in again, and both come in one
/// dispatch. The panel keeps its scale throughout, so a maximized window isn't sized for
/// 1.50 and back.
#[test]
fn a_lid_switch_unplugged_and_plugged_again_in_one_dispatch_leaves_windows_alone() {
    let mut f = Fixture::new();
    plug(&mut f, "event1", DeviceKind::LidSwitch);
    f.plug_internal(PANEL.0, PANEL.1);
    let client = f.connect(Kind::Xdg);
    let window = f.map(client, WindowSpec::plain((200, 100))).xdg();
    f.client(client).set_maximized(window);
    f.roundtrip(client);
    let configures = f.client(client).window(window).configures.len();

    f.state.caminito.remove_input_device("event1");
    plug(&mut f, "event1", DeviceKind::LidSwitch);
    f.roundtrip(client);

    assert_eq!(scale(&f), 1.8);
    assert_eq!(f.client(client).window(window).configures.len(), configures);
}
