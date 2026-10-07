//! Input devices' settings (53): each device gets its kind's `DeviceSettings` when it's
//! added, and again when they change. They're applied one `Setting` at a time through
//! `ConfigurableDevice`, which libinput's devices implement (56) and the tests' recording
//! double does, so the translation and its re-application are tested without hardware.

use crate::settings::translate::{
    AccelProfile, ClickMethod, DeviceSettings, Devices, ScrollMethod,
};
use crate::state::Caminito;

/// What a device is, for the settings it takes and for the default scale's rule (50).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    /// A pointer that isn't a touchpad.
    Mouse,
    /// A pointer that libinput can tap on, as msd tells touchpads apart.
    Touchpad,
    /// A laptop's lid. It takes no settings, but tells the default scale's rule (50) that
    /// the built-in panel is a laptop's (56).
    LidSwitch,
    /// A device msd sets nothing on, such as a keyboard or another switch.
    Other,
}

/// One setting, as libinput's device configuration has it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Setting {
    /// Whether the device sends events at all.
    SendEvents(bool),
    /// Whether the primary and secondary buttons are swapped.
    LeftHanded(bool),
    /// `None` is the device's default speed.
    AccelSpeed(Option<f64>),
    AccelProfile(AccelProfile),
    /// Whether scrolling is reversed.
    NaturalScroll(bool),
    /// Whether pressing left and right together is a middle click.
    MiddleEmulation(bool),
    /// Whether a tap is a click.
    Tap(bool),
    DisableWhileTyping(bool),
    ClickMethod(ClickMethod),
    ScrollMethod(ScrollMethod),
}

/// A device settings are applied to.
pub trait ConfigurableDevice {
    /// The id Smithay's `Device::id` gives the device's events.
    fn id(&self) -> String;
    /// Which settings the device takes.
    fn kind(&self) -> DeviceKind;
    /// Applies one setting. A device that lacks it ignores it, as msd does.
    fn apply(&mut self, setting: Setting);
}

/// The settings `settings` has for a device, in the order they're applied.
pub fn settings_of(settings: &DeviceSettings) -> Vec<Setting> {
    [
        settings.send_events.map(Setting::SendEvents),
        Some(Setting::LeftHanded(settings.left_handed)),
        Some(Setting::AccelSpeed(settings.accel_speed)),
        Some(Setting::AccelProfile(settings.accel_profile)),
        Some(Setting::NaturalScroll(settings.natural_scroll)),
        settings.middle_emulation.map(Setting::MiddleEmulation),
        settings.tap.map(Setting::Tap),
        settings
            .disable_while_typing
            .map(Setting::DisableWhileTyping),
        settings.click_method.map(Setting::ClickMethod),
        settings.scroll_method.map(Setting::ScrollMethod),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// The devices plugged in, and the settings they were last given.
pub struct InputDevices {
    devices: Vec<Box<dyn ConfigurableDevice>>,
    settings: Devices,
}

impl InputDevices {
    /// No devices yet, which will get `settings`.
    pub fn new(settings: Devices) -> Self {
        Self {
            devices: Vec::new(),
            settings,
        }
    }

    /// Adds a device, and gives it its kind's settings.
    pub fn add(&mut self, mut device: Box<dyn ConfigurableDevice>) {
        if let Some(settings) = settings_for(&self.settings, device.kind()) {
            apply_all(device.as_mut(), &settings_of(settings));
        }
        self.devices.push(device);
    }

    /// Forgets the device with `id`, which was unplugged.
    pub fn remove(&mut self, id: &str) {
        self.devices.retain(|device| device.id() != id);
    }

    /// New settings: each kind whose settings changed gets them again.
    pub fn settings_changed(&mut self, settings: Devices) {
        let old = std::mem::replace(&mut self.settings, settings);
        for kind in [DeviceKind::Mouse, DeviceKind::Touchpad] {
            let (Some(new), Some(old)) =
                (settings_for(&self.settings, kind), settings_for(&old, kind))
            else {
                continue;
            };
            if new == old {
                continue;
            }
            let applied = settings_of(new);
            for device in self.devices.iter_mut().filter(|d| d.kind() == kind) {
                apply_all(device.as_mut(), &applied);
            }
        }
    }

    /// Whether a lid switch is plugged in, which makes the built-in panel a laptop's (50).
    pub fn has_lid_switch(&self) -> bool {
        self.devices
            .iter()
            .any(|device| device.kind() == DeviceKind::LidSwitch)
    }

    /// Whether the device with `id` passes on horizontal scrolling. A device Caminito wasn't
    /// told of, such as the winit window, does.
    pub fn scrolls_horizontally(&self, id: &str) -> bool {
        self.devices
            .iter()
            .find(|device| device.id() == id)
            .and_then(|device| settings_for(&self.settings, device.kind()))
            .is_none_or(|settings| settings.horizontal_scroll)
    }
}

fn apply_all(device: &mut dyn ConfigurableDevice, settings: &[Setting]) {
    for setting in settings {
        device.apply(*setting);
    }
}

/// The settings `devices` has for `kind`, if it takes any.
fn settings_for(devices: &Devices, kind: DeviceKind) -> Option<&DeviceSettings> {
    match kind {
        DeviceKind::Mouse => Some(&devices.mouse),
        DeviceKind::Touchpad => Some(&devices.touchpad),
        DeviceKind::LidSwitch | DeviceKind::Other => None,
    }
}

impl Caminito {
    /// A device was plugged in: it gets its kind's settings now and on each change. A lid
    /// switch reaches the outputs' scales in the refresh pass.
    pub fn add_input_device(&mut self, device: Box<dyn ConfigurableDevice>) {
        self.input_devices.add(device);
    }

    /// The device with `id` was unplugged.
    pub fn remove_input_device(&mut self, id: &str) {
        self.input_devices.remove(id);
    }
}

#[cfg(test)]
mod tests;
