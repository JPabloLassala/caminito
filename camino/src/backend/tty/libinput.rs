//! libinput's devices as `ConfigurableDevice`s (53, 56): told apart as msd tells them
//! apart, and given each `Setting` through libinput's device configuration.

use smithay::reexports::input::event::switch::Switch;
use smithay::reexports::input::{
    self, AccelProfile as LibinputAccelProfile, ClickMethod as LibinputClickMethod,
    DeviceCapability, DeviceConfigError, ScrollMethod as LibinputScrollMethod, SendEventsMode,
};

use crate::input::devices::{ConfigurableDevice, DeviceKind, Setting};
use crate::settings::translate::{AccelProfile, ClickMethod, ScrollMethod};

impl ConfigurableDevice for input::Device {
    fn id(&self) -> String {
        smithay::backend::input::Device::id(self)
    }

    fn kind(&self) -> DeviceKind {
        // msd takes a device it can tap on for a touchpad, whatever else it is.
        if self.config_tap_finger_count() > 0 {
            DeviceKind::Touchpad
        } else if self.has_capability(DeviceCapability::Pointer) {
            DeviceKind::Mouse
        } else if self.has_capability(DeviceCapability::Switch)
            && self.switch_has_switch(Switch::Lid) == Ok(true)
        {
            DeviceKind::LidSwitch
        } else {
            DeviceKind::Other
        }
    }

    fn apply(&mut self, setting: Setting) {
        let result = match setting {
            Setting::SendEvents(enabled) => self.config_send_events_set_mode(if enabled {
                SendEventsMode::ENABLED
            } else {
                SendEventsMode::DISABLED
            }),
            Setting::LeftHanded(enabled) => self.config_left_handed_set(enabled),
            Setting::AccelSpeed(speed) => {
                let speed = speed.unwrap_or_else(|| self.config_accel_default_speed());
                self.config_accel_set_speed(speed)
            }
            Setting::AccelProfile(profile) => {
                let profile = match profile {
                    AccelProfile::Default => self.config_accel_default_profile(),
                    AccelProfile::Adaptive => Some(LibinputAccelProfile::Adaptive),
                    AccelProfile::Flat => Some(LibinputAccelProfile::Flat),
                };
                match profile {
                    Some(profile) => self.config_accel_set_profile(profile),
                    // A device without acceleration has no default profile.
                    None => Err(DeviceConfigError::Unsupported),
                }
            }
            Setting::NaturalScroll(enabled) => {
                self.config_scroll_set_natural_scroll_enabled(enabled)
            }
            Setting::MiddleEmulation(enabled) => self.config_middle_emulation_set_enabled(enabled),
            Setting::Tap(enabled) => self.config_tap_set_enabled(enabled),
            Setting::DisableWhileTyping(enabled) => self.config_dwt_set_enabled(enabled),
            Setting::ClickMethod(method) => self.config_click_set_method(match method {
                ClickMethod::ButtonAreas => LibinputClickMethod::ButtonAreas,
                ClickMethod::Clickfinger => LibinputClickMethod::Clickfinger,
            }),
            Setting::ScrollMethod(method) => self.config_scroll_set_method(match method {
                ScrollMethod::NoScroll => LibinputScrollMethod::NoScroll,
                ScrollMethod::TwoFinger => LibinputScrollMethod::TwoFinger,
                ScrollMethod::Edge => LibinputScrollMethod::Edge,
            }),
        };
        // A device that lacks a setting ignores it, as msd does.
        if let Err(DeviceConfigError::Invalid) = result {
            tracing::debug!("{} won't take {setting:?}", self.name());
        }
    }
}
