//! The settings window's state (257) against python-dbusmock's `bluez5` template on a private
//! bus: This computer and its adapter chooser, the paired devices and the selection, and the
//! computer discoverable while the window is open. The window draws what it says and calls it.

mod mock;

use caminito_bluetooth::bluez::Bluez;
use caminito_bluetooth::settings::{Selection, Settings};

use mock::Mock;

const HEADSET: &str = "11:22:33:44:55:66";
const MOUSE: &str = "AA:BB:CC:DD:EE:FF";
const PHONE: &str = "00:11:22:AA:BB:CC";

/// The window's state, once BlueZ's objects are read.
fn settings(mock: &Mock) -> Settings {
    let bluez = Bluez::new(&mock.connection);
    mock.wait_until("BlueZ's objects", || bluez.is_ready());
    Settings::new(&bluez)
}

fn discoverable(mock: &Mock, adapter: &str) -> bool {
    mock.property(adapter, "org.bluez.Adapter1", "Discoverable")
        .get::<bool>()
        .unwrap()
}

fn aliases(settings: &Settings) -> Vec<String> {
    settings
        .devices()
        .into_iter()
        .map(|device| device.alias)
        .collect()
}

#[test]
fn the_adapter_chooser_is_there_with_two_adapters_and_not_with_one() {
    mock::run(|mock| {
        mock.add_adapter("hci0", "laptop");
        let settings = settings(mock);
        assert!(settings.chooser().is_none());
        assert_eq!(settings.adapter().unwrap().alias, "laptop");

        mock.add_adapter("hci1", "dongle");
        mock.wait_until("the second adapter", || settings.chooser().is_some());
        let chooser: Vec<_> = settings
            .chooser()
            .unwrap()
            .into_iter()
            .map(|adapter| adapter.alias)
            .collect();
        assert_eq!(chooser, ["laptop", "dongle"]);

        settings.choose("/org/bluez/hci1");
        assert_eq!(settings.adapter().unwrap().alias, "dongle");
    });
}

#[test]
fn the_window_lists_the_paired_devices_of_every_adapter_by_name() {
    mock::run(|mock| {
        mock.add_adapter("hci0", "laptop");
        mock.add_adapter("hci1", "dongle");
        mock.add_device("hci0", HEADSET, "headset");
        mock.pair("hci0", HEADSET);
        mock.add_device("hci1", MOUSE, "Mouse");
        mock.pair("hci1", MOUSE);
        // Seen in a scan, not paired: Add New Device's (258), not the list's.
        mock.add_device("hci0", PHONE, "Phone");

        let settings = settings(mock);
        assert_eq!(aliases(&settings), ["headset", "Mouse"]);
    });
}

#[test]
fn the_computer_is_discoverable_while_the_window_is_open() {
    mock::run(|mock| {
        let adapter = mock.add_adapter("hci0", "laptop");
        let settings = settings(mock);
        assert!(!discoverable(mock, &adapter));

        settings.open();
        mock.wait_until("the adapter to be discoverable", || {
            discoverable(mock, &adapter)
        });

        // As BlueZ ends it when its DiscoverableTimeout runs out.
        mock.set_property(&adapter, "org.bluez.Adapter1", "Discoverable", false);
        mock.wait_until("the adapter to be discoverable again", || {
            discoverable(mock, &adapter)
        });

        mock.block_on(settings.close()).unwrap();
        assert!(!discoverable(mock, &adapter));
    });
}

#[test]
fn choosing_another_adapter_makes_it_discoverable_in_the_first_ones_place() {
    mock::run(|mock| {
        let first = mock.add_adapter("hci0", "laptop");
        let second = mock.add_adapter("hci1", "dongle");
        let settings = settings(mock);
        settings.open();
        mock.wait_until("the first to be discoverable", || {
            discoverable(mock, &first)
        });

        settings.choose(&second);
        mock.wait_until("the second to be discoverable and not the first", || {
            discoverable(mock, &second) && !discoverable(mock, &first)
        });

        mock.block_on(settings.close()).unwrap();
        assert!(!discoverable(mock, &second));
    });
}

#[test]
fn an_unpowered_adapter_is_made_discoverable_once_it_powers_on() {
    mock::run(|mock| {
        let adapter = mock.add_adapter("hci0", "laptop");
        mock.set_property(&adapter, "org.bluez.Adapter1", "Powered", false);
        let settings = settings(mock);
        settings.open();

        mock.set_property(&adapter, "org.bluez.Adapter1", "Powered", true);
        mock.wait_until("the adapter to be discoverable", || {
            discoverable(mock, &adapter)
        });
        mock.block_on(settings.close()).unwrap();
        assert!(!discoverable(mock, &adapter));
    });
}

#[test]
fn the_adapter_and_a_device_are_renamed() {
    mock::run(|mock| {
        let adapter = mock.add_adapter("hci0", "laptop");
        let headset = mock.add_device("hci0", HEADSET, "Headset");
        mock.pair("hci0", HEADSET);
        let settings = settings(mock);

        mock.block_on(settings.rename(&adapter, "workstation"))
            .unwrap();
        mock.wait_until("the adapter's new name", || {
            settings.adapter().unwrap().alias == "workstation"
        });
        mock.block_on(settings.rename(&headset, "My headphones"))
            .unwrap();
        mock.wait_until("the headset's new name", || {
            aliases(&settings) == ["My headphones"]
        });
        assert_eq!(
            mock.property(&headset, "org.bluez.Device1", "Alias").str(),
            Some("My headphones")
        );
    });
}

#[test]
fn a_device_connects_and_disconnects() {
    mock::run(|mock| {
        mock.add_adapter("hci0", "laptop");
        let headset = mock.add_device("hci0", HEADSET, "Headset");
        mock.pair("hci0", HEADSET);
        let settings = settings(mock);
        let connected = || settings.devices()[0].connected;

        mock.block_on(settings.connect(&headset)).unwrap();
        mock.wait_until("the headset to connect", connected);
        mock.block_on(settings.disconnect(&headset)).unwrap();
        mock.wait_until("the headset to disconnect", || !connected());
    });
}

#[test]
fn forgetting_the_selected_device_selects_this_computer() {
    mock::run(|mock| {
        mock.add_adapter("hci0", "laptop");
        let headset = mock.add_device("hci0", HEADSET, "Headset");
        mock.pair("hci0", HEADSET);
        let mouse = mock.add_device("hci0", MOUSE, "Mouse");
        mock.pair("hci0", MOUSE);
        let settings = settings(mock);
        assert_eq!(settings.selection(), Selection::Computer);

        settings.select(Selection::Device(headset.clone()));
        assert_eq!(settings.selection(), Selection::Device(headset.clone()));

        mock.block_on(settings.forget(&headset)).unwrap();
        mock.wait_until("the headset to go", || aliases(&settings) == ["Mouse"]);
        assert_eq!(settings.selection(), Selection::Computer);

        // And a device forgotten elsewhere, as with bluetoothctl.
        settings.select(Selection::Device(mouse.clone()));
        mock.remove_device("/org/bluez/hci0", &mouse);
        mock.wait_until("the mouse to go", || settings.devices().is_empty());
        assert_eq!(settings.selection(), Selection::Computer);
    });
}

#[test]
fn each_change_is_told_to_the_window() {
    mock::run(|mock| {
        mock.add_adapter("hci0", "laptop");
        let settings = settings(mock);
        let told = std::rc::Rc::new(std::cell::Cell::new(0));
        settings.connect_changed({
            let told = told.clone();
            move |settings| told.set(settings.devices().len())
        });

        mock.add_device("hci0", HEADSET, "Headset");
        mock.pair("hci0", HEADSET);
        mock.wait_until("the window to hear of the headset", || told.get() == 1);

        settings.select(Selection::Device(settings.devices()[0].path.clone()));
        told.set(0);
        settings.select(Selection::Computer);
        assert_eq!(told.get(), 1, "a new selection is told too");
    });
}
