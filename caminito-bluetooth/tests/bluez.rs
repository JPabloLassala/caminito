//! BlueZ's model (255) against python-dbusmock's `bluez5` template on a private bus: what it
//! lists, how it follows BlueZ while running, and what its calls do to the mock.

mod mock;

use caminito_bluetooth::bluez::{Bluez, Device};

use mock::Mock;

const HEADSET: &str = "11:22:33:44:55:66";
const MOUSE: &str = "AA:BB:CC:DD:EE:FF";

/// The model, once it has read the mock's objects.
fn model(mock: &Mock) -> Bluez {
    let bluez = Bluez::new(&mock.connection);
    mock.wait_until("BlueZ's objects", || bluez.is_ready());
    bluez
}

fn device(bluez: &Bluez, path: &str) -> Option<Device> {
    bluez
        .devices()
        .into_iter()
        .find(|device| device.path == path)
}

#[test]
fn the_adapters_and_their_devices_are_listed() {
    mock::run(|mock| {
        mock.add_adapter("hci0", "laptop");
        mock.add_adapter("hci1", "dongle");
        let headset = mock.add_device("hci0", HEADSET, "Headset");
        mock.pair("hci0", HEADSET);
        let mouse = mock.add_device("hci1", MOUSE, "Mouse");

        let bluez = model(mock);

        let adapters = bluez.adapters();
        let paths: Vec<_> = adapters
            .iter()
            .map(|adapter| adapter.path.as_str())
            .collect();
        assert_eq!(paths, ["/org/bluez/hci0", "/org/bluez/hci1"]);
        assert_eq!(adapters[0].alias, "laptop");
        assert!(adapters[0].powered);
        assert!(!adapters[0].discoverable);

        let devices = bluez.devices();
        assert_eq!(devices.len(), 2);
        let headset = device(&bluez, &headset).unwrap();
        assert_eq!(headset.alias, "Headset");
        assert_eq!(headset.address, HEADSET);
        assert_eq!(headset.adapter, "/org/bluez/hci0");
        assert_eq!(headset.icon.as_deref(), Some("phone"));
        assert!(headset.paired);
        assert!(!headset.connected);
        assert_eq!(headset.battery, None);
        let mouse = device(&bluez, &mouse).unwrap();
        assert_eq!(mouse.adapter, "/org/bluez/hci1");
        assert!(!mouse.paired);
    });
}

#[test]
fn a_device_added_and_removed_while_running_is_followed() {
    mock::run(|mock| {
        mock.add_adapter("hci0", "laptop");
        let bluez = model(mock);
        assert!(bluez.devices().is_empty());

        let headset = mock.add_device("hci0", HEADSET, "Headset");
        mock.wait_until("the headset to appear", || {
            device(&bluez, &headset).is_some()
        });

        mock.pair("hci0", HEADSET);
        mock.wait_until("the headset to be paired", || {
            device(&bluez, &headset).is_some_and(|device| device.paired)
        });

        mock.remove_device("/org/bluez/hci0", &headset);
        mock.wait_until("the headset to go", || bluez.devices().is_empty());
    });
}

#[test]
fn an_adapter_added_while_running_is_followed() {
    mock::run(|mock| {
        let bluez = model(mock);
        assert!(bluez.adapters().is_empty());

        mock.add_adapter("hci0", "laptop");
        mock.wait_until("the adapter to appear", || bluez.adapters().len() == 1);
    });
}

#[test]
fn the_battery_is_read_and_followed() {
    mock::run(|mock| {
        mock.add_adapter("hci0", "laptop");
        let headset = mock.add_device("hci0", HEADSET, "Headset");
        let mouse = mock.add_device("hci0", MOUSE, "Mouse");
        mock.add_battery(&headset, 80);

        let bluez = model(mock);
        assert_eq!(device(&bluez, &headset).unwrap().battery, Some(80));
        assert_eq!(device(&bluez, &mouse).unwrap().battery, None);

        mock.set_battery(&headset, 15);
        mock.wait_until("the headset's battery to fall", || {
            device(&bluez, &headset).unwrap().battery == Some(15)
        });

        mock.add_battery(&mouse, 55);
        mock.wait_until("the mouse's battery to appear", || {
            device(&bluez, &mouse).unwrap().battery == Some(55)
        });
    });
}

#[test]
fn a_device_connects_and_disconnects() {
    mock::run(|mock| {
        mock.add_adapter("hci0", "laptop");
        let headset = mock.add_device("hci0", HEADSET, "Headset");
        mock.pair("hci0", HEADSET);
        let bluez = model(mock);

        mock.block_on(bluez.connect(&headset)).unwrap();
        mock.wait_until("the headset to connect", || {
            device(&bluez, &headset).unwrap().connected
        });

        mock.block_on(bluez.disconnect(&headset)).unwrap();
        mock.wait_until("the headset to disconnect", || {
            !device(&bluez, &headset).unwrap().connected
        });

        // BlueZ's error comes back with what was asked.
        let err = mock.block_on(bluez.disconnect(&headset)).unwrap_err();
        assert!(
            err.to_string()
                .starts_with(&format!("can't disconnect {headset}: ")),
            "{err}"
        );
    });
}

#[test]
fn a_device_and_an_adapter_are_renamed() {
    mock::run(|mock| {
        mock.add_adapter("hci0", "laptop");
        let headset = mock.add_device("hci0", HEADSET, "Headset");
        let bluez = model(mock);

        mock.block_on(bluez.set_alias(&headset, "My headphones"))
            .unwrap();
        mock.wait_until("the headset's new name", || {
            device(&bluez, &headset).unwrap().alias == "My headphones"
        });
        assert_eq!(
            mock.property(&headset, "org.bluez.Device1", "Alias").str(),
            Some("My headphones")
        );

        mock.block_on(bluez.set_alias("/org/bluez/hci0", "workstation"))
            .unwrap();
        mock.wait_until("the adapter's new name", || {
            bluez.adapters()[0].alias == "workstation"
        });
        assert_eq!(
            mock.property("/org/bluez/hci0", "org.bluez.Adapter1", "Alias")
                .str(),
            Some("workstation")
        );
    });
}

#[test]
fn a_removed_device_is_forgotten() {
    mock::run(|mock| {
        mock.add_adapter("hci0", "laptop");
        let headset = mock.add_device("hci0", HEADSET, "Headset");
        let mouse = mock.add_device("hci0", MOUSE, "Mouse");
        let bluez = model(mock);

        mock.block_on(bluez.remove(&headset)).unwrap();
        mock.wait_until("the headset to go", || device(&bluez, &headset).is_none());
        assert!(device(&bluez, &mouse).is_some());
    });
}

#[test]
fn an_adapter_is_powered_made_discoverable_and_scans() {
    mock::run(|mock| {
        let adapter = mock.add_adapter("hci0", "laptop");
        let bluez = model(mock);

        mock.block_on(bluez.set_powered(&adapter, false)).unwrap();
        mock.wait_until("the adapter to power off", || !bluez.adapters()[0].powered);
        assert_eq!(
            mock.property(&adapter, "org.bluez.Adapter1", "Powered")
                .get::<bool>(),
            Some(false)
        );

        mock.block_on(bluez.set_discoverable(&adapter, true))
            .unwrap();
        mock.wait_until("the adapter to be discoverable", || {
            bluez.adapters()[0].discoverable
        });

        mock.block_on(bluez.start_discovery(&adapter)).unwrap();
        mock.wait_until("the adapter to scan", || bluez.adapters()[0].discovering);
        mock.block_on(bluez.stop_discovery(&adapter)).unwrap();
        mock.wait_until("the adapter to stop", || !bluez.adapters()[0].discovering);
    });
}

#[test]
fn each_change_is_told_to_the_listeners() {
    mock::run(|mock| {
        mock.add_adapter("hci0", "laptop");
        let bluez = model(mock);
        let told = std::rc::Rc::new(std::cell::Cell::new(0));
        bluez.connect_changed({
            let told = told.clone();
            // A listener reads the model, which nothing holds borrowed while it's told.
            move |bluez| told.set(told.get() + bluez.devices().len())
        });

        mock.add_device("hci0", HEADSET, "Headset");
        mock.wait_until("the listener to hear of the headset", || told.get() == 1);
    });
}

#[test]
fn a_path_that_isnt_one_is_refused_before_it_reaches_the_bus() {
    mock::run(|mock| {
        let bluez = model(mock);
        let err = mock.block_on(bluez.connect("not a path")).unwrap_err();
        assert_eq!(
            err.to_string(),
            "can't reach \"not a path\": it isn't an object path"
        );
    });
}
