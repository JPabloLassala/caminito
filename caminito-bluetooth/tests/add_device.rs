//! Add New Device (258) against python-dbusmock's `bluez5` template on a private bus: the
//! computer's adapter looks for devices while the page is shown and not otherwise, the
//! devices it finds that have a name are listed, and a click pairs one, which then connects
//! and is one of the paired devices. The window draws what the state says and calls it.

mod mock;

use caminito_bluetooth::bluez::Bluez;
use caminito_bluetooth::settings::{Attempt, Selection, Settings};

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

fn discovering(mock: &Mock, adapter: &str) -> bool {
    mock.property(adapter, "org.bluez.Adapter1", "Discovering")
        .get::<bool>()
        .unwrap()
}

fn found(settings: &Settings) -> Vec<String> {
    settings
        .new_devices()
        .into_iter()
        .map(|device| device.alias)
        .collect()
}

#[test]
fn the_scan_starts_when_the_page_is_shown_and_stops_when_it_is_left() {
    mock::run(|mock| {
        let adapter = mock.add_adapter("hci0", "laptop");
        let settings = settings(mock);
        settings.open();
        assert!(
            !discovering(mock, &adapter),
            "the computer's page doesn't scan"
        );

        settings.select(Selection::AddDevice);
        mock.wait_until("the scan to start", || discovering(mock, &adapter));

        settings.select(Selection::Computer);
        mock.wait_until("the scan to stop", || !discovering(mock, &adapter));

        // And once more, so that nothing is left over from the first time.
        settings.select(Selection::AddDevice);
        mock.wait_until("the scan to start again", || discovering(mock, &adapter));
        mock.block_on(settings.close()).unwrap();
        assert!(!discovering(mock, &adapter), "closing the window stops it");
    });
}

#[test]
fn the_page_selected_before_the_window_opens_scans_once_it_does() {
    mock::run(|mock| {
        let adapter = mock.add_adapter("hci0", "laptop");
        let settings = settings(mock);
        // --scan: the window is built on Add New Device.
        settings.select(Selection::AddDevice);
        settings.open();
        mock.wait_until("the scan to start", || discovering(mock, &adapter));
        mock.block_on(settings.close()).unwrap();
        assert!(!discovering(mock, &adapter));
    });
}

#[test]
fn no_scan_runs_while_the_adapter_is_off_and_one_starts_when_it_powers_on() {
    mock::run(|mock| {
        let adapter = mock.add_adapter("hci0", "laptop");
        mock.set_property(&adapter, "org.bluez.Adapter1", "Powered", false);
        let settings = settings(mock);
        settings.open();
        settings.select(Selection::AddDevice);
        mock.wait_until("the settings to know of the page", || {
            settings.selection() == Selection::AddDevice
        });
        assert!(!discovering(mock, &adapter));

        mock.set_property(&adapter, "org.bluez.Adapter1", "Powered", true);
        mock.wait_until("the scan to start", || discovering(mock, &adapter));
        mock.block_on(settings.close()).unwrap();
    });
}

#[test]
fn the_scan_follows_the_adapter_that_is_this_computer() {
    mock::run(|mock| {
        let first = mock.add_adapter("hci0", "laptop");
        let second = mock.add_adapter("hci1", "dongle");
        let settings = settings(mock);
        settings.open();
        settings.select(Selection::AddDevice);
        mock.wait_until("the first to scan", || discovering(mock, &first));

        settings.choose(&second);
        mock.wait_until("the second to scan and not the first", || {
            discovering(mock, &second) && !discovering(mock, &first)
        });
        mock.block_on(settings.close()).unwrap();
        assert!(!discovering(mock, &second));
    });
}

#[test]
fn a_device_found_is_listed_when_it_has_a_name_and_isnt_paired() {
    mock::run(|mock| {
        mock.add_adapter("hci0", "laptop");
        mock.add_device("hci0", HEADSET, "Headset");
        mock.pair("hci0", HEADSET);
        let settings = settings(mock);
        settings.open();
        settings.select(Selection::AddDevice);
        assert!(found(&settings).is_empty(), "a paired device is not new");

        mock.add_device("hci0", PHONE, "Pixel 7");
        mock.wait_until("the phone to be listed", || found(&settings) == ["Pixel 7"]);
        let phone = &settings.new_devices()[0];
        assert_eq!(phone.icon.as_deref(), Some("phone"), "with its icon");
        assert_eq!(settings.devices().len(), 1, "and not among the paired");
        mock.block_on(settings.close()).unwrap();
    });
}

#[test]
fn a_device_without_a_name_is_not_listed() {
    mock::run(|mock| {
        mock.add_adapter("hci0", "laptop");
        let settings = settings(mock);
        settings.open();
        settings.select(Selection::AddDevice);

        mock.announce_device("/org/bluez/hci0", MOUSE, None);
        // Announced after, so that once the second is listed the first has been heard of.
        mock.announce_device("/org/bluez/hci0", PHONE, Some("Pixel 7"));
        mock.wait_until("the phone to be listed", || found(&settings) == ["Pixel 7"]);
        assert_eq!(settings.new_devices().len(), 1);
        mock.block_on(settings.close()).unwrap();
    });
}

#[test]
fn only_what_the_computers_adapter_found_is_listed() {
    mock::run(|mock| {
        mock.add_adapter("hci0", "laptop");
        mock.add_adapter("hci1", "dongle");
        mock.add_device("hci1", MOUSE, "Mouse");
        let settings = settings(mock);
        settings.open();
        settings.select(Selection::AddDevice);
        mock.add_device("hci0", PHONE, "Pixel 7");
        mock.wait_until("the phone to be listed", || found(&settings) == ["Pixel 7"]);

        settings.choose("/org/bluez/hci1");
        assert_eq!(found(&settings), ["Mouse"]);
        mock.block_on(settings.close()).unwrap();
    });
}

#[test]
fn pairing_connects_the_device_trusts_it_and_moves_it_to_the_devices() {
    mock::run(|mock| {
        mock.add_adapter("hci0", "laptop");
        let phone = mock.add_device("hci0", PHONE, "Pixel 7");
        let settings = settings(mock);
        settings.open();
        settings.select(Selection::AddDevice);
        mock.wait_until("the phone to be listed", || found(&settings) == ["Pixel 7"]);

        mock.block_on(settings.pair(&phone)).unwrap();
        mock.wait_until("the phone to be a paired device", || {
            settings
                .devices()
                .iter()
                .any(|device| device.path == phone && device.connected && device.trusted)
        });
        assert!(
            settings.new_devices().is_empty(),
            "it has left Add New Device"
        );
        assert_eq!(settings.attempt(&phone), None);
        assert_eq!(
            mock.property(&phone, "org.bluez.Device1", "Trusted")
                .get::<bool>(),
            Some(true),
            "a pairing that asked nothing is trusted too"
        );
        mock.block_on(settings.close()).unwrap();
    });
}

#[test]
fn a_pairing_that_fails_is_told_on_the_device_and_tried_again() {
    mock::run(|mock| {
        mock.add_adapter("hci0", "laptop");
        let mouse = mock.add_device("hci0", MOUSE, "Mouse");
        mock.fail_pairing(
            &mouse,
            "org.bluez.Error.AuthenticationFailed",
            "Authentication Failed",
        );
        let settings = settings(mock);
        settings.open();
        settings.select(Selection::AddDevice);
        assert_eq!(found(&settings), ["Mouse"]);
        assert_eq!(settings.attempt(&mouse), None);

        let err = mock.block_on(settings.pair(&mouse)).unwrap_err();
        assert!(err.to_string().contains("pair"), "{err}");
        assert_eq!(
            settings.attempt(&mouse),
            Some(Attempt::Failed("Authentication Failed".to_owned()))
        );
        assert_eq!(found(&settings), ["Mouse"], "it can be tried again");
        assert!(settings.devices().is_empty());
        assert_eq!(
            mock.property(&mouse, "org.bluez.Device1", "Trusted")
                .get::<bool>(),
            Some(false),
            "a device that didn't pair isn't trusted"
        );

        // Leaving the page forgets the failure.
        settings.select(Selection::Computer);
        assert_eq!(settings.attempt(&mouse), None);
        mock.block_on(settings.close()).unwrap();
    });
}

#[test]
fn a_device_that_goes_takes_its_failure_with_it() {
    mock::run(|mock| {
        let adapter = mock.add_adapter("hci0", "laptop");
        let mouse = mock.add_device("hci0", MOUSE, "Mouse");
        mock.fail_pairing(&mouse, "org.bluez.Error.Failed", "Page Timeout");
        let settings = settings(mock);
        settings.open();
        settings.select(Selection::AddDevice);
        let _ = mock.block_on(settings.pair(&mouse));
        assert!(settings.attempt(&mouse).is_some());

        mock.remove_device(&adapter, &mouse);
        mock.wait_until("the mouse to go", || found(&settings).is_empty());
        assert_eq!(settings.attempt(&mouse), None);
        mock.block_on(settings.close()).unwrap();
    });
}
