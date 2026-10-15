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
//! Add New Device (258) is a third selection. While it's shown, and the window is open, This
//! computer's adapter looks for devices, and stops when the page is left or the window
//! closes. The page lists what it finds that has a name and isn't paired, and [`Settings::pair`]
//! pairs one: the agent's dialogs come up as BlueZ asks, then the device is trusted and
//! connected, and it's one of the paired devices.
//!
//! The binary draws it in GTK; the tests drive it against python-dbusmock's BlueZ.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
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
    /// Add New Device.
    AddDevice,
}

/// What became of the last try to pair a device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Attempt {
    /// It's being paired.
    Pairing,
    /// It failed, for this reason: BlueZ's own words.
    Failed(String),
}

/// A pairing that didn't go through, or went through only in part.
#[derive(Debug)]
pub enum PairError {
    /// Another device is being paired, and BlueZ's agent answers one at a time.
    Busy,
    /// BlueZ didn't pair it.
    Pair(bluez::Error),
    /// It's paired and trusted, but wouldn't connect.
    Connect(bluez::Error),
}

impl fmt::Display for PairError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PairError::Busy => write!(f, "another device is being paired"),
            PairError::Pair(err) | PairError::Connect(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for PairError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PairError::Busy => None,
            PairError::Pair(err) | PairError::Connect(err) => Some(err),
        }
    }
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
    /// The adapter that is looking for devices for Add New Device.
    scanning: Option<String>,
    /// Whether a call to start or stop the scan hasn't been answered yet.
    scan_busy: bool,
    /// The adapter BlueZ refused to scan with, so that it isn't asked again at every change.
    scan_refused: Option<String>,
    /// The devices being paired or that failed to be, by path.
    attempts: BTreeMap<String, Attempt>,
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
        self.hold_scan();
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

    /// The devices Add New Device lists: those This computer's adapter has found that have a
    /// name and aren't paired, by name. BlueZ gives none a name it hasn't heard, only an
    /// address, and an address says nothing to pick one by.
    pub fn new_devices(&self) -> Vec<Device> {
        let Some(adapter) = self.adapter() else {
            return Vec::new();
        };
        let mut devices: Vec<_> = self
            .bluez
            .devices()
            .into_iter()
            .filter(|device| {
                device.adapter == adapter.path
                    && !device.paired
                    && device.name.as_deref().is_some_and(|name| !name.is_empty())
            })
            .collect();
        devices.sort_by_cached_key(|device| (device.alias.to_lowercase(), device.path.clone()));
        devices
    }

    /// Whether a device is being paired. BlueZ's agent answers one at a time, so one is.
    pub fn is_pairing(&self) -> bool {
        self.state
            .borrow()
            .attempts
            .values()
            .any(|attempt| *attempt == Attempt::Pairing)
    }

    /// What became of the last try to pair the device at `device`, while Add New Device is
    /// shown: it's being paired, or it failed.
    pub fn attempt(&self, device: &str) -> Option<Attempt> {
        self.state.borrow().attempts.get(device).cloned()
    }

    /// What the detail pane shows.
    pub fn selection(&self) -> Selection {
        self.state.borrow().selection.clone()
    }

    /// Shows `selection` in the detail pane.
    pub fn select(&self, selection: Selection) {
        {
            let mut state = self.state.borrow_mut();
            // A failure belongs to a visit to Add New Device, not to the next.
            if (state.selection == Selection::AddDevice) != (selection == Selection::AddDevice) {
                state
                    .attempts
                    .retain(|_, attempt| matches!(attempt, Attempt::Pairing));
            }
            state.selection = selection;
        }
        self.hold_scan();
        self.tell();
    }

    /// The window opened: This computer becomes discoverable.
    pub fn open(&self) {
        self.state.borrow_mut().open = true;
        self.hold_discoverable();
        self.hold_scan();
    }

    /// The window closed: This computer is hidden again, and no longer looks for devices.
    pub async fn close(&self) -> Result<(), bluez::Error> {
        let (held, scanning) = {
            let mut state = self.state.borrow_mut();
            state.open = false;
            state.refused = false;
            let scanning = if state.scan_busy {
                None
            } else {
                state.scanning.take()
            };
            (state.held.take(), scanning)
        };
        let hidden = match held {
            Some(adapter) if self.is_discoverable(&adapter) => {
                self.bluez.set_discoverable(&adapter, false).await
            }
            // A call still on its way hides it once it's answered.
            _ => Ok(()),
        };
        let stopped = match scanning {
            Some(adapter) if self.is_powered(&adapter) => self.bluez.stop_discovery(&adapter).await,
            // BlueZ ends a scan when its adapter powers off or goes. And one still being
            // started is stopped once it's answered.
            _ => Ok(()),
        };
        hidden.and(stopped)
    }

    /// Pairs the device at `device`, which Add New Device lists. BlueZ asks the pairing
    /// agent for what it needs, so this takes as long as the user does. A device that's
    /// paired is trusted, since pairing is the one consent the window asks for, then
    /// connected, and is one of the paired devices. Meanwhile, and if it fails,
    /// [`Settings::attempt`] says so.
    pub async fn pair(&self, device: &str) -> Result<(), PairError> {
        {
            if self.is_pairing() {
                return Err(PairError::Busy);
            }
            self.state
                .borrow_mut()
                .attempts
                .insert(device.to_owned(), Attempt::Pairing);
        }
        self.tell();
        let outcome = self.pair_and_connect(device).await;
        {
            let mut state = self.state.borrow_mut();
            match &outcome {
                Err(PairError::Pair(err)) => {
                    state
                        .attempts
                        .insert(device.to_owned(), Attempt::Failed(reason(err)));
                }
                _ => {
                    state.attempts.remove(device);
                }
            }
        }
        self.tell();
        outcome
    }

    async fn pair_and_connect(&self, device: &str) -> Result<(), PairError> {
        self.bluez.pair(device).await.map_err(PairError::Pair)?;
        // The agent trusts a device it asked about. One that needed no answer, as most
        // headsets, was never asked about.
        if let Err(err) = self.bluez.set_trusted(device, true).await {
            glib::g_warning!(DOMAIN, "{err}");
        }
        self.bluez.connect(device).await.map_err(PairError::Connect)
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
            // A device that's gone takes its failure with it. One being paired is kept, as
            // BlueZ may not have announced the device again by the time it's paired.
            let known = self.bluez.devices();
            state.attempts.retain(|path, attempt| {
                *attempt == Attempt::Pairing || known.iter().any(|device| &device.path == path)
            });
        }
        self.hold_discoverable();
        self.hold_scan();
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

    /// Makes This computer's adapter look for devices while the window is open, powered and
    /// showing Add New Device, and stops it when it isn't, or when it's another adapter's
    /// turn. One call to BlueZ is out at a time, and each answer looks again.
    fn hold_scan(&self) {
        let target = self
            .adapter()
            .filter(|adapter| adapter.powered)
            .map(|adapter| adapter.path);
        let mut state = self.state.borrow_mut();
        let wanted = target.filter(|_| state.open && state.selection == Selection::AddDevice);
        // BlueZ ends a scan when its adapter powers off or goes.
        if let Some(held) = state.scanning.clone()
            && !self.is_powered(&held)
        {
            state.scanning = None;
        }
        if state.scan_refused != wanted {
            state.scan_refused = None;
        }
        if state.scan_busy || state.scanning == wanted {
            return;
        }
        // The calls hold the state, not a weak reference to it: the window's closing drops
        // its own, and an answer to a start that comes after must still stop the scan.
        let settings = self.clone();
        if let Some(held) = state.scanning.take() {
            state.scan_busy = true;
            drop(state);
            glib::spawn_future_local(async move {
                let answer = settings.bluez.stop_discovery(&held).await;
                settings.state.borrow_mut().scan_busy = false;
                if let Err(err) = answer {
                    glib::g_warning!(DOMAIN, "{err}");
                }
                settings.hold_scan();
            });
        } else if let Some(target) = wanted
            && state.scan_refused.as_ref() != Some(&target)
        {
            state.scan_busy = true;
            drop(state);
            glib::spawn_future_local(async move {
                let answer = settings.bluez.start_discovery(&target).await;
                {
                    let mut state = settings.state.borrow_mut();
                    state.scan_busy = false;
                    match &answer {
                        Ok(()) => state.scanning = Some(target.clone()),
                        Err(_) => state.scan_refused = Some(target.clone()),
                    }
                }
                if let Err(err) = answer {
                    glib::g_warning!(DOMAIN, "{err}");
                }
                settings.hold_scan();
            });
        }
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

    fn is_powered(&self, adapter: &str) -> bool {
        self.bluez
            .adapters()
            .iter()
            .any(|known| known.path == adapter && known.powered)
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

/// BlueZ's own words for why a call failed, as `Authentication Failed`, without the D-Bus
/// error's name in front.
fn reason(err: &bluez::Error) -> String {
    match err {
        bluez::Error::Call { source, .. } => {
            let mut source = source.clone();
            gio::DBusError::strip_remote_error(&mut source);
            source.message().to_owned()
        }
        other => other.to_string(),
    }
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
