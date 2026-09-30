//! Input devices' settings through the recording double (53): each device gets its kind's
//! settings when it's added, and again whenever they change. No window is involved.

use std::cell::RefCell;
use std::rc::Rc;

use insta::assert_snapshot;

use super::fixture::Fixture;
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
    f.state.camino.add_input_device(Box::new(RecordingDevice {
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
    f.dispatch_until(|f| f.state.camino.settings.devices.touchpad.tap == Some(true));
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
    f.dispatch_until(|f| f.state.camino.settings.devices.touchpad.send_events == Some(false));

    let touchpad = plug(&mut f, "event5", DeviceKind::Touchpad);
    assert!(touchpad.borrow().contains(&Setting::SendEvents(false)));
}

#[test]
fn a_removed_device_gets_no_more_settings() {
    let mut f = Fixture::new();
    let kept = plug(&mut f, "event5", DeviceKind::Touchpad);
    let removed = plug(&mut f, "event6", DeviceKind::Touchpad);
    removed.borrow_mut().clear();
    f.state.camino.remove_input_device("event6");

    f.set_setting(TOUCHPAD, "tap-to-click", Value::Bool(true));
    f.dispatch_until(|_| kept.borrow().contains(&Setting::Tap(true)));
    assert_eq!(format(&removed), "");
}
