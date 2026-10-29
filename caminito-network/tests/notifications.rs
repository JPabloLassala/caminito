//! The connection notifications against python-dbusmock's `networkmanager` and
//! `notification_daemon` templates on a private bus. The mock NetworkManager doesn't move an
//! active connection or a device through its states, so the test emits their `StateChanged`
//! as NetworkManager would, with its reasons: a Wi-Fi connection's own reason is only
//! `DEVICE_DISCONNECTED` or `USER_DISCONNECTED` (nm-act-request.c), and why it went is the
//! device's. It reads what reached the notification server from the mock's calls.

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
const DEVICE: &str = "org.freedesktop.NetworkManager.Device";

/// NetworkManager's `NMActiveConnectionState`.
const ACTIVATING: u32 = 1;
const ACTIVATED: u32 = 2;
const DEACTIVATING: u32 = 3;
const DEACTIVATED: u32 = 4;

/// NetworkManager's `NMActiveConnectionStateReason`.
const NONE: u32 = 1;
const USER_DISCONNECTED: u32 = 2;
const DEVICE_DISCONNECTED: u32 = 3;
const CONNECT_TIMEOUT: u32 = 6;
const LOGIN_FAILED: u32 = 10;

/// NetworkManager's `NMDeviceState`.
mod device {
    pub const UNMANAGED: u32 = 10;
    pub const DISCONNECTED: u32 = 30;
    pub const IP_CONFIG: u32 = 70;
    pub const ACTIVATED: u32 = 100;
    pub const DEACTIVATING: u32 = 110;
    pub const FAILED: u32 = 120;
}

/// NetworkManager's `NMDeviceStateReason`.
mod device_reason {
    pub const NONE: u32 = 0;
    pub const NO_SECRETS: u32 = 7;
    pub const SUPPLICANT_DISCONNECT: u32 = 8;
    pub const DHCP_FAILED: u32 = 17;
    pub const SLEEPING: u32 = 37;
    pub const USER_REQUESTED: u32 = 39;
    pub const SSID_NOT_FOUND: u32 = 53;
    pub const NEW_ACTIVATION: u32 = 60;
}

/// NetworkManager's `NMState`.
const ASLEEP: u32 = 10;
const CONNECTED_GLOBAL: u32 = 70;

/// A notification, as the notification server had it.
#[derive(Debug, PartialEq, Eq)]
struct Notification {
    replaces: u32,
    icon: String,
    summary: String,
    body: String,
    category: String,
    /// The `desktop-entry` hint, which makes it a Notifying app of its own.
    desktop_entry: String,
}

fn notifications(mock: &Mock) -> Vec<Notification> {
    // The mock gives each call's arguments as an array of variants.
    let argument = |call: &Variant, index| call.child_value(index).as_variant().unwrap();
    mock.notifies()
        .iter()
        .map(|call| {
            let hints = argument(call, 6).get::<HashMap<String, Variant>>().unwrap();
            let hint = |key| {
                hints
                    .get(key)
                    .and_then(|hint| hint.str().map(str::to_owned))
                    .unwrap_or_default()
            };
            Notification {
                replaces: argument(call, 1).get().unwrap(),
                icon: argument(call, 2).str().unwrap().to_owned(),
                summary: argument(call, 3).str().unwrap().to_owned(),
                body: argument(call, 4).str().unwrap().to_owned(),
                category: hint("category"),
                desktop_entry: hint("desktop-entry"),
            }
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
        desktop_entry: "caminito-network".to_owned(),
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
            ("wlan0", "wlan0", device::DISCONNECTED).to_variant(),
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

    /// A saved VPN connection named `name`, as its path.
    fn vpn(&self, mock: &Mock, name: &str) -> String {
        let settings: HashMap<String, HashMap<String, Variant>> = HashMap::from([(
            "connection".to_owned(),
            HashMap::from([
                ("id".to_owned(), name.to_variant()),
                ("type".to_owned(), "vpn".to_variant()),
            ]),
        )]);
        mock.call(
            "/org/freedesktop/NetworkManager/Settings",
            "org.freedesktop.NetworkManager.Settings",
            "AddConnection",
            (settings,).to_variant(),
        )
        .child_value(0)
        .str()
        .unwrap()
        .to_owned()
    }

    /// Starts activating the VPN `connection` by itself, as its active connection's path.
    fn activate_vpn(&self, mock: &Mock, connection: &str) -> String {
        self.activate_vpn_on(mock, connection, &[])
    }

    /// Starts activating the VPN `connection` over `devices`, as its active connection's
    /// path.
    fn activate_vpn_on(&self, mock: &Mock, connection: &str, devices: &[&str]) -> String {
        mock.call(
            MOCK_PATH,
            MOCK,
            "AddActiveConnection",
            (devices.to_vec(), connection, "/", "vpn", ACTIVATING).to_variant(),
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

/// Emits the device's `StateChanged`, as NetworkManager does when it moves.
fn device_changed(mock: &Mock, fixture: &Fixture, state: u32, old: u32, reason: u32) {
    mock.call(
        &fixture.device,
        MOCK,
        "EmitSignal",
        (
            DEVICE,
            "StateChanged",
            "uuu",
            vec![state.to_variant(), old.to_variant(), reason.to_variant()],
        )
            .to_variant(),
    );
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

/// A Wi-Fi connection that fails as it comes up, its device with `reason`, and what the
/// notification says.
fn wifi_failure(reason: u32) -> Notification {
    let mut failed = None;
    mock::run(|mock| {
        let fixture = start(mock);
        let home = fixture.network(mock, "Home");
        let active = fixture.activate(mock, &home, "0");
        device_changed(mock, &fixture, device::FAILED, device::IP_CONFIG, reason);
        state_changed(mock, &active, DEACTIVATED, DEVICE_DISCONNECTED);
        // Failed, the device goes on to disconnected, with no reason.
        device_changed(
            mock,
            &fixture,
            device::DISCONNECTED,
            device::FAILED,
            device_reason::NONE,
        );
        fixture.remove(mock, &active);
        failed = wait_for_notifications(mock, 1).pop();
    });
    failed.unwrap()
}

/// A VPN that fails as it comes up with `reason`, and what the notification says.
fn vpn_failure(reason: u32) -> Notification {
    let mut failed = None;
    mock::run(|mock| {
        let fixture = start(mock);
        let office = fixture.vpn(mock, "Office");
        let active = fixture.activate_vpn(mock, &office);
        state_changed(mock, &active, DEACTIVATED, reason);
        failed = wait_for_notifications(mock, 1).pop();
    });
    failed.unwrap()
}

#[test]
fn a_wrong_wifi_password_is_a_failed_connection_the_network_turned_away() {
    assert_eq!(
        wifi_failure(device_reason::SUPPLICANT_DISCONNECT),
        notification(
            "network-wireless",
            "Connection failed",
            "Couldn't connect to Home. The network turned it away. The password may be wrong.",
            "network.error",
        )
    );
}

#[test]
fn a_connection_without_its_password_fails_for_want_of_one() {
    assert_eq!(
        wifi_failure(device_reason::NO_SECRETS).body,
        "Couldn't connect to Home. It needs a password, and none was given."
    );
}

#[test]
fn a_connection_without_an_address_fails_for_want_of_one() {
    assert_eq!(
        wifi_failure(device_reason::DHCP_FAILED).body,
        "Couldn't connect to Home. The network gave it no address."
    );
}

#[test]
fn a_network_out_of_range_fails_as_one() {
    assert_eq!(
        wifi_failure(device_reason::SSID_NOT_FOUND).body,
        "Couldn't connect to Home. The network isn't in range."
    );
}

#[test]
fn a_vpn_whose_login_is_refused_fails_with_its_login() {
    assert_eq!(
        vpn_failure(LOGIN_FAILED),
        notification(
            "network-vpn",
            "Connection failed",
            "Couldn't connect to Office. The login failed.",
            "network.error",
        )
    );
}

#[test]
fn a_vpn_that_takes_too_long_fails_with_a_timeout() {
    assert_eq!(
        vpn_failure(CONNECT_TIMEOUT).body,
        "Couldn't connect to Office. The attempt timed out."
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

/// The Wi-Fi connection `active` goes down as NetworkManager takes it down for the device's
/// `reason`, with the device's signal before the connection's or after it.
fn wifi_down(mock: &Mock, fixture: &Fixture, active: &str, reason: u32, device_first: bool) {
    // Only the user's request reaches the connection's own reason (nm-act-request.c).
    let own = if reason == device_reason::USER_REQUESTED {
        USER_DISCONNECTED
    } else {
        DEVICE_DISCONNECTED
    };
    if device_first {
        device_changed(
            mock,
            fixture,
            device::DEACTIVATING,
            device::ACTIVATED,
            reason,
        );
    }
    state_changed(mock, active, DEACTIVATING, own);
    state_changed(mock, active, DEACTIVATED, own);
    if !device_first {
        device_changed(mock, fixture, device::UNMANAGED, device::ACTIVATED, reason);
    }
    fixture.remove(mock, active);
}

#[test]
fn a_connection_the_user_cancels_as_it_comes_up_is_no_failure() {
    mock::run(|mock| {
        let fixture = start(mock);
        let home = fixture.network(mock, "Home");
        let active = fixture.activate(mock, &home, "0");
        wifi_down(mock, &fixture, &active, device_reason::USER_REQUESTED, true);
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
        wifi_down(
            mock,
            &fixture,
            &active,
            device_reason::SUPPLICANT_DISCONNECT,
            true,
        );
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
        // DeactivateConnection, as the device's Disconnect, is the device's USER_REQUESTED.
        device_changed(
            mock,
            &fixture,
            device::DEACTIVATING,
            device::ACTIVATED,
            device_reason::USER_REQUESTED,
        );
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
fn a_vpn_on_a_link_the_user_disconnects_is_no_news() {
    mock::run(|mock| {
        let fixture = start(mock);
        let home = fixture.network(mock, "Home");
        let at_home = user_connects(mock, &fixture, &home);
        let office = fixture.vpn(mock, "Office");
        let vpn = fixture.activate_vpn_on(mock, &office, &[fixture.device.as_str()]);
        state_changed(mock, &vpn, ACTIVATED, NONE);
        wait_for_notifications(mock, 1);
        wifi_down(
            mock,
            &fixture,
            &at_home,
            device_reason::USER_REQUESTED,
            false,
        );
        state_changed(mock, &vpn, DEACTIVATED, DEVICE_DISCONNECTED);
        mock.run_for(QUIET);
        assert_eq!(notifications(mock).len(), 1);
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
fn the_airplane_mode_switch_is_no_news_of_the_wifi_connection() {
    mock::run(|mock| {
        let fixture = start(mock);
        let home = fixture.network(mock, "Home");
        let active = user_connects(mock, &fixture, &home);
        state_changed(mock, &active, DEACTIVATED, DEVICE_DISCONNECTED);
        fixture.remove(mock, &active);
        set_manager(mock, "WirelessHardwareEnabled", false.to_variant());
        mock.run_for(QUIET);
        assert_eq!(notifications(mock), []);
    });
}

#[test]
fn sleep_is_no_news_of_the_connections_it_takes_down() {
    mock::run(|mock| {
        let fixture = start(mock);
        let home = fixture.network(mock, "Home");
        let active = user_connects(mock, &fixture, &home);
        // The device says why after the connection went.
        wifi_down(mock, &fixture, &active, device_reason::SLEEPING, false);
        mock.run_for(QUIET);
        assert_eq!(notifications(mock), []);
    });
}

#[test]
fn a_resume_before_the_notifier_decides_is_still_no_news_of_sleep() {
    mock::run(|mock| {
        let fixture = start(mock);
        let home = fixture.network(mock, "Home");
        let active = user_connects(mock, &fixture, &home);
        state_changed(mock, &active, DEACTIVATED, DEVICE_DISCONNECTED);
        fixture.remove(mock, &active);
        // The machine sleeps, and wakes, within the notifier's second, as its clock has it.
        set_manager(mock, "State", ASLEEP.to_variant());
        set_manager(mock, "State", CONNECTED_GLOBAL.to_variant());
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
        wifi_down(
            mock,
            &fixture,
            &at_home,
            device_reason::NEW_ACTIVATION,
            true,
        );
        let at_work = fixture.activate(mock, &work, "1");
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
        wifi_down(mock, &fixture, &active, device_reason::NO_SECRETS, true);
        wait_for_notifications(mock, 1);
        user_connects(mock, &fixture, &home);
        mock.wait_until("the notification to close", || !mock.closes().is_empty());
        let closed = mock.closes()[0].child_value(0).as_variant().unwrap();
        // The template takes the id as an int32, where the spec has a uint32.
        assert_eq!(closed.get::<i32>(), Some(1));
    });
}
