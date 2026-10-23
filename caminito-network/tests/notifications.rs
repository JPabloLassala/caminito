//! The connection notifications against python-dbusmock's `networkmanager` and
//! `notification_daemon` templates on a private bus. The mock NetworkManager doesn't move an
//! active connection through its states, so the test emits its `StateChanged` as NetworkManager
//! would, and reads what reached the notification server from the mock's calls.

mod mock;

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use caminito_network::notifications::{self, Notifier};
use gio::glib::Variant;
use gio::glib::variant::{ObjectPath, ToVariant};

use mock::Mock;

const MOCK: &str = "org.freedesktop.DBus.Mock";
/// The object the template's own methods are on.
const MOCK_PATH: &str = "/org/freedesktop";
const MANAGER_PATH: &str = "/org/freedesktop/NetworkManager";
const MANAGER: &str = "org.freedesktop.NetworkManager";
const ACTIVE: &str = "org.freedesktop.NetworkManager.Connection.Active";

/// NetworkManager's `NMActiveConnectionState`.
const ACTIVATING: u32 = 1;
const ACTIVATED: u32 = 2;
const DEACTIVATING: u32 = 3;
const DEACTIVATED: u32 = 4;

/// NetworkManager's `NMActiveConnectionStateReason`.
const NONE: u32 = 1;
const USER_DISCONNECTED: u32 = 2;
const DEVICE_DISCONNECTED: u32 = 3;
const IP_CONFIG_INVALID: u32 = 5;
const CONNECT_TIMEOUT: u32 = 6;
const NO_SECRETS: u32 = 9;
const LOGIN_FAILED: u32 = 10;

/// NetworkManager's `NMDeviceState` for a device with nothing up.
const DEVICE_DISCONNECTED_STATE: u32 = 30;

/// A notification, as the notification server had it.
#[derive(Debug, PartialEq, Eq)]
struct Notification {
    replaces: u32,
    icon: String,
    summary: String,
    body: String,
    category: String,
}

fn notifications(mock: &Mock) -> Vec<Notification> {
    // The mock gives each call's arguments as an array of variants.
    let argument = |call: &Variant, index| call.child_value(index).as_variant().unwrap();
    mock.notifies()
        .iter()
        .map(|call| Notification {
            replaces: argument(call, 1).get().unwrap(),
            icon: argument(call, 2).str().unwrap().to_owned(),
            summary: argument(call, 3).str().unwrap().to_owned(),
            body: argument(call, 4).str().unwrap().to_owned(),
            category: argument(call, 6)
                .get::<HashMap<String, Variant>>()
                .unwrap()
                .get("category")
                .and_then(|category| category.str().map(str::to_owned))
                .unwrap_or_default(),
        })
        .collect()
}

fn notification(icon: &str, summary: &str, body: &str, category: &str) -> Notification {
    Notification {
        replaces: 0,
        icon: icon.to_owned(),
        summary: summary.to_owned(),
        body: body.to_owned(),
        category: category.to_owned(),
    }
}

/// The mock NetworkManager with a Wi-Fi device, and the notifier following it.
struct Fixture {
    device: String,
    /// The notifier's unique name on the session bus, which the Network applet calls it at.
    name: String,
    _notifier: Notifier,
}

fn start(mock: &mut Mock) -> Fixture {
    mock.start_notification_daemon();
    let device = mock
        .call(
            MOCK_PATH,
            MOCK,
            "AddWiFiDevice",
            ("wlan0", "wlan0", DEVICE_DISCONNECTED_STATE).to_variant(),
        )
        .child_value(0)
        .str()
        .unwrap()
        .to_owned();
    let system = mock.connect();
    let session = mock.connect();
    let notifier = Notifier::new(&system, &session).unwrap();
    // NetworkManager answers the read after the notifier's subscriptions are in place, and
    // before any change the test makes next.
    mock.wait_until("the notifier to read NetworkManager's objects", || {
        !mock.calls(MOCK_PATH, "GetManagedObjects").is_empty()
    });
    Fixture {
        device,
        name: session.unique_name().unwrap().to_string(),
        _notifier: notifier,
    }
}

impl Fixture {
    /// A saved Wi-Fi connection to `ssid`, in range, as its path.
    fn network(&self, mock: &Mock, ssid: &str) -> String {
        mock.call(
            MOCK_PATH,
            MOCK,
            "AddAccessPoint",
            (
                self.device.as_str(),
                format!("ap_{ssid}"),
                ssid,
                "11:22:33:44:55:66",
                2u32,
                2437u32,
                54000u32,
                82u8,
                0u32,
            )
                .to_variant(),
        );
        mock.call(
            MOCK_PATH,
            MOCK,
            "AddWiFiConnection",
            (self.device.as_str(), format!("conn_{ssid}"), ssid, "").to_variant(),
        )
        .child_value(0)
        .str()
        .unwrap()
        .to_owned()
    }

    /// Starts activating `connection` on the device as NetworkManager does by itself, as its
    /// active connection's path.
    fn activate(&self, mock: &Mock, connection: &str, name: &str) -> String {
        mock.call(
            MOCK_PATH,
            MOCK,
            "AddActiveConnection",
            (
                vec![self.device.as_str()],
                connection,
                "/",
                name,
                ACTIVATING,
            )
                .to_variant(),
        )
        .child_value(0)
        .str()
        .unwrap()
        .to_owned()
    }

    /// Takes the active connection away, as NetworkManager does once it's deactivated.
    fn remove(&self, mock: &Mock, active: &str) {
        mock.call(
            MOCK_PATH,
            MOCK,
            "RemoveActiveConnection",
            (self.device.as_str(), active).to_variant(),
        );
    }

    /// Tells the notifier that the user is activating the connection `uuid`, as the Network
    /// applet does before it asks NetworkManager.
    fn user_activating(&self, mock: &Mock, uuid: &str) {
        let answered = Rc::new(Cell::new(false));
        let caller = mock.connect();
        caller.call(
            Some(&self.name),
            notifications::PATH,
            notifications::INTERFACE,
            "UserActivating",
            Some(&(uuid,).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            -1,
            gio::Cancellable::NONE,
            {
                let answered = answered.clone();
                move |reply| {
                    reply.expect("the notifier answers");
                    answered.set(true);
                }
            },
        );
        mock.wait_until("the notifier to hear of the activation", || answered.get());
    }
}

/// Emits the active connection's `StateChanged`, as NetworkManager does when it moves.
fn state_changed(mock: &Mock, active: &str, state: u32, reason: u32) {
    mock.call(
        active,
        MOCK,
        "EmitSignal",
        (
            ACTIVE,
            "StateChanged",
            "uu",
            vec![state.to_variant(), reason.to_variant()],
        )
            .to_variant(),
    );
}

/// The UUID of the saved connection at `connection`.
fn uuid(mock: &Mock, connection: &str) -> String {
    mock.call(
        connection,
        "org.freedesktop.NetworkManager.Settings.Connection",
        "GetSettings",
        ().to_variant(),
    )
    .child_value(0)
    .get::<HashMap<String, HashMap<String, Variant>>>()
    .unwrap()["connection"]["uuid"]
        .str()
        .unwrap()
        .to_owned()
}

/// Sets one of NetworkManager's properties, as when the user turns Wi-Fi off.
fn set_manager(mock: &Mock, property: &str, value: Variant) {
    mock.call(
        MOCK_PATH,
        MOCK,
        "SetProperty",
        (MANAGER_PATH, MANAGER, property, value).to_variant(),
    );
}

fn wait_for_notifications(mock: &Mock, count: usize) -> Vec<Notification> {
    mock.wait_until(&format!("{count} notifications"), || {
        mock.notifies().len() >= count
    });
    notifications(mock)
}

/// A connection that fails as it comes up, with `reason`, and what the notification says.
fn failure(reason: u32) -> Notification {
    let mut failed = None;
    mock::run(|mock| {
        let fixture = start(mock);
        let home = fixture.network(mock, "Home");
        let active = fixture.activate(mock, &home, "0");
        state_changed(mock, &active, DEACTIVATING, reason);
        state_changed(mock, &active, DEACTIVATED, reason);
        fixture.remove(mock, &active);
        let mut shown = wait_for_notifications(mock, 1);
        failed = shown.pop();
    });
    failed.unwrap()
}

#[test]
fn a_wrong_password_is_a_failed_connection_whose_login_failed() {
    assert_eq!(
        failure(LOGIN_FAILED),
        notification(
            "network-wireless",
            "Connection failed",
            "Couldn't connect to Home. The login failed.",
            "network.error",
        )
    );
}

#[test]
fn a_connection_without_its_password_fails_for_want_of_one() {
    assert_eq!(
        failure(NO_SECRETS).body,
        "Couldn't connect to Home. It needs a password, and none was given."
    );
}

#[test]
fn a_connection_that_takes_too_long_fails_with_a_timeout() {
    assert_eq!(
        failure(CONNECT_TIMEOUT).body,
        "Couldn't connect to Home. The attempt timed out."
    );
}

#[test]
fn a_connection_without_an_address_fails_for_its_ip_configuration() {
    assert_eq!(
        failure(IP_CONFIG_INVALID).body,
        "Couldn't connect to Home. It got no valid IP address."
    );
}

/// Long enough for the notifier's second after a connection goes down, and its call to the
/// notification server, for a test that no notification comes.
const QUIET: std::time::Duration = std::time::Duration::from_secs(3);

/// The user connects to `connection` through the Network applet, which tells the notifier
/// before it asks NetworkManager, as its active connection's path.
fn user_connects(mock: &Mock, fixture: &Fixture, connection: &str) -> String {
    fixture.user_activating(mock, &uuid(mock, connection));
    let active = mock.call(
        MANAGER_PATH,
        MANAGER,
        "ActivateConnection",
        (
            ObjectPath::try_from(connection).unwrap(),
            ObjectPath::try_from(fixture.device.as_str()).unwrap(),
            ObjectPath::try_from("/").unwrap(),
        )
            .to_variant(),
    );
    active.child_value(0).str().unwrap().to_owned()
}

#[test]
fn a_connection_the_user_cancels_as_it_comes_up_is_no_failure() {
    mock::run(|mock| {
        let fixture = start(mock);
        let home = fixture.network(mock, "Home");
        let active = fixture.activate(mock, &home, "0");
        state_changed(mock, &active, DEACTIVATED, USER_DISCONNECTED);
        fixture.remove(mock, &active);
        mock.run_for(QUIET);
        assert_eq!(notifications(mock), []);
    });
}

#[test]
fn an_auto_connect_tells_the_user_it_connected() {
    mock::run(|mock| {
        let fixture = start(mock);
        let home = fixture.network(mock, "Home");
        let active = fixture.activate(mock, &home, "0");
        state_changed(mock, &active, ACTIVATED, NONE);
        assert_eq!(
            wait_for_notifications(mock, 1),
            [notification(
                "network-wireless",
                "Connected to Home",
                "",
                "network.connected"
            )]
        );
    });
}

#[test]
fn a_connection_the_user_makes_is_no_news() {
    mock::run(|mock| {
        let fixture = start(mock);
        let home = fixture.network(mock, "Home");
        user_connects(mock, &fixture, &home);
        mock.run_for(QUIET);
        assert_eq!(notifications(mock), []);
    });
}

#[test]
fn a_dropped_connection_replaces_its_connected_notification() {
    mock::run(|mock| {
        let fixture = start(mock);
        let home = fixture.network(mock, "Home");
        let active = fixture.activate(mock, &home, "0");
        state_changed(mock, &active, ACTIVATED, NONE);
        wait_for_notifications(mock, 1);
        state_changed(mock, &active, DEACTIVATING, DEVICE_DISCONNECTED);
        state_changed(mock, &active, DEACTIVATED, DEVICE_DISCONNECTED);
        fixture.remove(mock, &active);
        let dropped = wait_for_notifications(mock, 2).pop().unwrap();
        assert_eq!(
            dropped,
            Notification {
                // The mock server numbers its notifications from 1.
                replaces: 1,
                ..notification(
                    "network-wireless",
                    "Disconnected",
                    "The connection to Home dropped.",
                    "network.disconnected",
                )
            }
        );
    });
}

#[test]
fn a_connection_the_user_disconnects_is_no_news() {
    mock::run(|mock| {
        let fixture = start(mock);
        let home = fixture.network(mock, "Home");
        let active = user_connects(mock, &fixture, &home);
        // NetworkManager's reason for DeactivateConnection, as for the device's Disconnect.
        state_changed(mock, &active, DEACTIVATING, USER_DISCONNECTED);
        state_changed(mock, &active, DEACTIVATED, USER_DISCONNECTED);
        mock.call(
            MANAGER_PATH,
            MANAGER,
            "DeactivateConnection",
            (ObjectPath::try_from(active.as_str()).unwrap(),).to_variant(),
        );
        mock.run_for(QUIET);
        assert_eq!(notifications(mock), []);
    });
}

#[test]
fn wifi_turned_off_is_no_news_of_its_connection() {
    mock::run(|mock| {
        let fixture = start(mock);
        let home = fixture.network(mock, "Home");
        let active = user_connects(mock, &fixture, &home);
        // NetworkManager's property change comes after the connection's state, at idle.
        state_changed(mock, &active, DEACTIVATED, DEVICE_DISCONNECTED);
        fixture.remove(mock, &active);
        set_manager(mock, "WirelessEnabled", false.to_variant());
        mock.run_for(QUIET);
        assert_eq!(notifications(mock), []);
    });
}

#[test]
fn a_network_the_user_switches_to_is_no_news_of_the_one_before() {
    mock::run(|mock| {
        let fixture = start(mock);
        let home = fixture.network(mock, "Home");
        let work = fixture.network(mock, "Work");
        let at_home = user_connects(mock, &fixture, &home);
        fixture.user_activating(mock, &uuid(mock, &work));
        let at_work = fixture.activate(mock, &work, "1");
        state_changed(mock, &at_home, DEACTIVATED, DEVICE_DISCONNECTED);
        fixture.remove(mock, &at_home);
        state_changed(mock, &at_work, ACTIVATED, NONE);
        mock.run_for(QUIET);
        assert_eq!(notifications(mock), []);
    });
}

#[test]
fn a_connection_the_user_brings_back_closes_its_notification() {
    mock::run(|mock| {
        let fixture = start(mock);
        let home = fixture.network(mock, "Home");
        let active = fixture.activate(mock, &home, "0");
        state_changed(mock, &active, DEACTIVATED, LOGIN_FAILED);
        fixture.remove(mock, &active);
        wait_for_notifications(mock, 1);
        user_connects(mock, &fixture, &home);
        mock.wait_until("the notification to close", || !mock.closes().is_empty());
        let closed = mock.closes()[0].child_value(0).as_variant().unwrap();
        // The template takes the id as an int32, where the spec has a uint32.
        assert_eq!(closed.get::<i32>(), Some(1));
    });
}
