//! The Bluetooth settings window's state (257): what its list and its detail pane show, and
//! what the window does through BlueZ. The list is This computer, then the paired devices of
//! every adapter; the pane shows the selection. This computer is one adapter, the first
//! unless the user chose another in the chooser, which is there only with two or more.
//!
//! While the window is open, that adapter is discoverable: made so when the window opens,
//! when it's chosen, when it powers on, and again when BlueZ's `DiscoverableTimeout` ends it.
//! The adapter's timeout is left as the user set it, since bluetoothd keeps it across
//! reboots. When the window closes, or another adapter is chosen, it's hidden again.
//!
//! The binary draws it in GTK; the tests drive it against python-dbusmock's BlueZ.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use gio::glib;

use crate::DOMAIN;
use crate::bluez::{self, Adapter, Bluez, Device};

/// A battery below this percentage is low, and its bar is red.
pub const LOW_BATTERY: u8 = 20;

/// What the detail pane shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Selection {
    /// This computer, pinned at the top of the list.
    #[default]
    Computer,
    /// The paired device at this path.
    Device(String),
}

type Listener = Rc<dyn Fn(&Settings)>;

#[derive(Default)]
struct State {
    /// Whether the window is open.
    open: bool,
    /// The adapter the user chose, while there is one at its path.
    chosen: Option<String>,
    selection: Selection,
    /// The adapter the window holds discoverable, while it's open and powered.
    held: Option<String>,
    /// Whether a call to make it discoverable hasn't been answered yet.
    asking: bool,
    /// Whether BlueZ refused it, so that it isn't asked again at every change.
    refused: bool,
    listeners: Vec<Listener>,
}

/// The settings window's state over BlueZ's model. Clones share one state.
#[derive(Clone)]
pub struct Settings {
    bluez: Bluez,
    state: Rc<RefCell<State>>,
}

impl Settings {
    /// The state over `bluez`, which it follows: a device that goes isn't selected any more,
    /// and a chosen adapter that goes isn't chosen.
    pub fn new(bluez: &Bluez) -> Settings {
        let settings = Settings {
            bluez: bluez.clone(),
            state: Rc::default(),
        };
        let weak = Rc::downgrade(&settings.state);
        bluez.connect_changed(move |bluez| {
            if let Some(settings) = upgrade(bluez, &weak) {
                settings.follow();
            }
        });
        settings
    }

    /// This computer's adapter: the chosen one, else the first. None without an adapter.
    pub fn adapter(&self) -> Option<Adapter> {
        let adapters = self.bluez.adapters();
        let chosen = self.state.borrow().chosen.clone();
        let index = adapters
            .iter()
            .position(|adapter| Some(&adapter.path) == chosen.as_ref())
            .unwrap_or(0);
        adapters.into_iter().nth(index)
    }

    /// The adapters to choose This computer's from, when there are two or more.
    pub fn chooser(&self) -> Option<Vec<Adapter>> {
        Some(self.bluez.adapters()).filter(|adapters| adapters.len() >= 2)
    }

    /// Makes the adapter at `adapter` This computer's.
    pub fn choose(&self, adapter: &str) {
        self.state.borrow_mut().chosen = Some(adapter.to_owned());
        self.hold_discoverable();
        self.tell();
    }

    /// The paired devices of every adapter, by name.
    pub fn devices(&self) -> Vec<Device> {
        let mut devices: Vec<_> = self
            .bluez
            .devices()
            .into_iter()
            .filter(|device| device.paired)
            .collect();
        devices.sort_by_cached_key(|device| (device.alias.to_lowercase(), device.path.clone()));
        devices
    }

    /// What the detail pane shows.
    pub fn selection(&self) -> Selection {
        self.state.borrow().selection.clone()
    }

    /// Shows `selection` in the detail pane.
    pub fn select(&self, selection: Selection) {
        self.state.borrow_mut().selection = selection;
        self.tell();
    }

    /// The window opened: This computer becomes discoverable.
    pub fn open(&self) {
        self.state.borrow_mut().open = true;
        self.hold_discoverable();
    }

    /// The window closed: This computer is hidden again.
    pub async fn close(&self) -> Result<(), bluez::Error> {
        let held = {
            let mut state = self.state.borrow_mut();
            state.open = false;
            state.refused = false;
            state.held.take()
        };
        match held {
            Some(adapter) if self.is_discoverable(&adapter) => {
                self.bluez.set_discoverable(&adapter, false).await
            }
            // A call still on its way hides it once it's answered.
            _ => Ok(()),
        }
    }

    /// Gives the adapter or device at `path` the name `alias`.
    pub async fn rename(&self, path: &str, alias: &str) -> Result<(), bluez::Error> {
        self.bluez.set_alias(path, alias).await
    }

    /// Connects the device at `device`.
    pub async fn connect(&self, device: &str) -> Result<(), bluez::Error> {
        self.bluez.connect(device).await
    }

    /// Disconnects the device at `device`.
    pub async fn disconnect(&self, device: &str) -> Result<(), bluez::Error> {
        self.bluez.disconnect(device).await
    }

    /// Forgets the device at `device`. If it's selected, This computer is, once it's gone.
    pub async fn forget(&self, device: &str) -> Result<(), bluez::Error> {
        self.bluez.remove(device).await
    }

    /// Calls `listener` after each change: BlueZ's, the selection's or the chosen adapter's.
    pub fn connect_changed(&self, listener: impl Fn(&Settings) + 'static) {
        self.state.borrow_mut().listeners.push(Rc::new(listener));
    }

    /// Follows a change of BlueZ's.
    fn follow(&self) {
        let adapters = self.bluez.adapters();
        let devices = self.devices();
        {
            let mut state = self.state.borrow_mut();
            if let Some(chosen) = &state.chosen
                && !adapters.iter().any(|adapter| &adapter.path == chosen)
            {
                state.chosen = None;
            }
            if let Selection::Device(selected) = &state.selection
                && !devices.iter().any(|device| &device.path == selected)
            {
                state.selection = Selection::Computer;
            }
        }
        self.hold_discoverable();
        self.tell();
    }

    /// Makes This computer's adapter discoverable while the window is open and it's powered,
    /// and hides the one held before it, if it's another.
    fn hold_discoverable(&self) {
        let target = self.adapter().filter(|adapter| adapter.powered);
        let mut state = self.state.borrow_mut();
        if !state.open || state.asking {
            return;
        }
        let target_path = target.as_ref().map(|adapter| adapter.path.clone());
        if state.held != target_path {
            if let Some(held) = state.held.take() {
                self.hide(held);
            }
            state.held = target_path;
            state.refused = false;
        }
        let Some(target) = target else {
            return;
        };
        if target.discoverable || state.refused {
            return;
        }
        state.asking = true;
        drop(state);

        let bluez = self.bluez.clone();
        let weak = Rc::downgrade(&self.state);
        glib::spawn_future_local(async move {
            let answer = bluez.set_discoverable(&target.path, true).await;
            let Some(settings) = upgrade(&bluez, &weak) else {
                return;
            };
            let open = {
                let mut state = settings.state.borrow_mut();
                state.asking = false;
                state.refused = answer.is_err();
                state.open
            };
            if let Err(err) = answer {
                glib::g_warning!(DOMAIN, "{err}");
            } else if !open {
                // The window closed while BlueZ was asked.
                settings.hide(target.path);
            }
            settings.hold_discoverable();
        });
    }

    /// Hides the adapter at `adapter`, if it's discoverable.
    fn hide(&self, adapter: String) {
        if !self.is_discoverable(&adapter) {
            return;
        }
        let bluez = self.bluez.clone();
        glib::spawn_future_local(async move {
            if let Err(err) = bluez.set_discoverable(&adapter, false).await {
                glib::g_warning!(DOMAIN, "{err}");
            }
        });
    }

    fn is_discoverable(&self, adapter: &str) -> bool {
        self.bluez
            .adapters()
            .iter()
            .any(|known| known.path == adapter && known.discoverable)
    }

    /// Tells the listeners, with nothing borrowed so that they can read the state or act.
    fn tell(&self) {
        let listeners = self.state.borrow().listeners.clone();
        for listener in listeners {
            listener(self);
        }
    }
}

fn upgrade(bluez: &Bluez, weak: &Weak<RefCell<State>>) -> Option<Settings> {
    weak.upgrade().map(|state| Settings {
        bluez: bluez.clone(),
        state,
    })
}

/// What kind of device `device` is, from the icon BlueZ gives it for its class.
pub fn kind(device: &Device) -> &'static str {
    match device.icon.as_deref() {
        Some("audio-headset") => "Headset",
        Some("audio-headphones") => "Headphones",
        Some("audio-card") => "Audio device",
        Some("input-keyboard") => "Keyboard",
        Some("input-mouse") => "Mouse",
        Some("input-tablet") => "Tablet",
        Some("input-gaming") => "Game controller",
        Some("phone") => "Phone",
        Some("computer") => "Computer",
        Some("video-display") => "Display",
        Some("camera-video" | "camera-photo") => "Camera",
        Some("printer") => "Printer",
        Some("scanner") => "Scanner",
        Some("multimedia-player") => "Media player",
        Some("network-wireless") => "Network device",
        _ => "Device",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_icon(icon: Option<&str>) -> Device {
        Device {
            path: "/org/bluez/hci0/dev_11_22_33_44_55_66".to_owned(),
            adapter: "/org/bluez/hci0".to_owned(),
            address: "11:22:33:44:55:66".to_owned(),
            name: None,
            alias: "11:22:33:44:55:66".to_owned(),
            icon: icon.map(str::to_owned),
            paired: true,
            trusted: true,
            connected: false,
            battery: None,
        }
    }

    #[test]
    fn a_device_is_named_by_its_icon_and_an_unknown_one_is_a_device() {
        assert_eq!(kind(&with_icon(Some("audio-headphones"))), "Headphones");
        assert_eq!(kind(&with_icon(Some("input-keyboard"))), "Keyboard");
        assert_eq!(kind(&with_icon(Some("camera-photo"))), "Camera");
        assert_eq!(kind(&with_icon(Some("something-new"))), "Device");
        assert_eq!(kind(&with_icon(None)), "Device");
    }
}
