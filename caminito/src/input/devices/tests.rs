//! `InputDevices`' lookups, with devices that record nothing.

use super::*;
use crate::settings::translate::DeviceSettings;

/// A device that records nothing, for the lookups that don't need its settings.
struct Plain {
    id: &'static str,
    kind: DeviceKind,
}

impl ConfigurableDevice for Plain {
    fn id(&self) -> String {
        self.id.into()
    }

    fn kind(&self) -> DeviceKind {
        self.kind
    }

    fn apply(&mut self, _: Setting) {}
}

fn device_settings(horizontal_scroll: bool) -> DeviceSettings {
    DeviceSettings {
        send_events: None,
        left_handed: false,
        accel_speed: None,
        accel_profile: AccelProfile::Default,
        natural_scroll: false,
        middle_emulation: None,
        tap: None,
        disable_while_typing: None,
        click_method: None,
        scroll_method: None,
        horizontal_scroll,
    }
}

fn devices() -> InputDevices {
    let mut devices = InputDevices::new(Devices {
        mouse: device_settings(true),
        touchpad: device_settings(false),
    });
    for (id, kind) in [
        ("mouse", DeviceKind::Mouse),
        ("touchpad", DeviceKind::Touchpad),
        ("keyboard", DeviceKind::Other),
    ] {
        devices.add(Box::new(Plain { id, kind }));
    }
    devices
}

#[test]
fn a_device_whose_kind_has_horizontal_scrolling_off_drops_it() {
    assert!(!devices().scrolls_horizontally("touchpad"));
}

#[test]
fn a_device_whose_kind_has_horizontal_scrolling_on_keeps_it() {
    assert!(devices().scrolls_horizontally("mouse"));
}

#[test]
fn devices_of_no_kind_and_unknown_devices_keep_horizontal_scrolling() {
    let devices = devices();
    assert!(devices.scrolls_horizontally("keyboard"));
    assert!(devices.scrolls_horizontally("winit"));
}

#[test]
fn a_change_to_horizontal_scrolling_takes_effect() {
    let mut devices = devices();
    devices.settings_changed(Devices {
        mouse: device_settings(true),
        touchpad: device_settings(true),
    });
    assert!(devices.scrolls_horizontally("touchpad"));
}
