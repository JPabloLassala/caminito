//! The connection notifications (CMN-199): the ones nm-applet sent, through the session's
//! `org.freedesktop.Notifications`, with the connection's icon.
//!
//! - "Connection failed", with the reason NetworkManager gives, in words.
//! - "Disconnected", when a connection that was up goes down without the user's doing.
//! - "Connected to *X*", for an activation the user didn't start, as an auto-connect after
//!   resume.
//!
//! Each connection has one notification at a time, replaced as its state moves.
//!
//! The reason is the active connection's `StateChanged` reason, except where that is only
//! `DEVICE_DISCONNECTED`, as NetworkManager gives it for every Wi-Fi or wired connection that
//! fails or drops (nm-act-request.c, `device_state_changed`). Then it's the reason of the
//! device's own `StateChanged`, which says why: no secrets, the supplicant, DHCP, the carrier.
//!
//! What the user did is read from NetworkManager where it says so: a deactivation through
//! `DeactivateConnection` or the device's `Disconnect` has the reason `USER_DISCONNECTED`,
//! Wi-Fi or airplane mode turned off shows in `WirelessEnabled` and `WirelessHardwareEnabled`,
//! and sleep, a removed connection or a new activation on the device show in the device's
//! reason. An activation doesn't say who started it, so the Network applet tells the notifier
//! first, with [`INTERFACE`]'s `UserActivating`, on the session bus (ADR 0044). An activation
//! of a connection that no one named there is NetworkManager's own.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use gio::glib::variant::{ObjectPath, ToVariant};
use gio::glib::{self, Variant};

use crate::DOMAIN;

/// Where the notifier is on the session bus, under the program's name.
pub const PATH: &str = "/org/mate/Caminito/Network/Notifier";
/// The notifier's interface, which the Network applet calls.
pub const INTERFACE: &str = "org.mate.Caminito.Network.Notifier";

const INTERFACE_XML: &str = r#"<node>
  <interface name="org.mate.Caminito.Network.Notifier">
    <!-- The user is activating the connection uuid: its activation is no news to them. -->
    <method name="UserActivating">
      <arg name="uuid" type="s" direction="in"/>
    </method>
  </interface>
</node>"#;

const NETWORK_MANAGER: &str = "org.freedesktop.NetworkManager";
const MANAGER_PATH: &str = "/org/freedesktop/NetworkManager";
const MANAGER: &str = "org.freedesktop.NetworkManager";
/// Where NetworkManager's ObjectManager is.
const ROOT: &str = "/org/freedesktop";
const ACTIVE: &str = "org.freedesktop.NetworkManager.Connection.Active";
const DEVICE: &str = "org.freedesktop.NetworkManager.Device";
const OBJECT_MANAGER: &str = "org.freedesktop.DBus.ObjectManager";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";

const NOTIFICATIONS: &str = "org.freedesktop.Notifications";
const NOTIFICATIONS_PATH: &str = "/org/freedesktop/Notifications";

/// NetworkManager's `NMActiveConnectionState`.
const ACTIVATED: u32 = 2;
const DEACTIVATED: u32 = 4;

/// NetworkManager's `NMState` while the machine sleeps.
const ASLEEP: u32 = 10;

/// NetworkManager's `NMActiveConnectionStateReason`.
mod active_reason {
    pub const NONE: u32 = 1;
    pub const USER_DISCONNECTED: u32 = 2;
    pub const DEVICE_DISCONNECTED: u32 = 3;
    pub const SERVICE_STOPPED: u32 = 4;
    pub const IP_CONFIG_INVALID: u32 = 5;
    pub const CONNECT_TIMEOUT: u32 = 6;
    pub const SERVICE_START_TIMEOUT: u32 = 7;
    pub const SERVICE_START_FAILED: u32 = 8;
    pub const NO_SECRETS: u32 = 9;
    pub const LOGIN_FAILED: u32 = 10;
    pub const CONNECTION_REMOVED: u32 = 11;
    pub const DEPENDENCY_FAILED: u32 = 12;
    pub const DEVICE_REALIZE_FAILED: u32 = 13;
    pub const DEVICE_REMOVED: u32 = 14;
}

/// NetworkManager's `NMDeviceState`.
mod device_state {
    pub const UNMANAGED: u32 = 10;
    pub const UNAVAILABLE: u32 = 20;
    pub const DISCONNECTED: u32 = 30;
    pub const PREPARE: u32 = 40;
    pub const ACTIVATED: u32 = 100;
    pub const DEACTIVATING: u32 = 110;
    pub const FAILED: u32 = 120;
}

/// NetworkManager's `NMDeviceStateReason`, the ones the notifier tells apart.
mod device_reason {
    pub const NONE: u32 = 0;
    pub const UNKNOWN: u32 = 1;
    pub const CONFIG_FAILED: u32 = 4;
    pub const IP_CONFIG_UNAVAILABLE: u32 = 5;
    pub const IP_CONFIG_EXPIRED: u32 = 6;
    pub const NO_SECRETS: u32 = 7;
    pub const SUPPLICANT_DISCONNECT: u32 = 8;
    pub const SUPPLICANT_CONFIG_FAILED: u32 = 9;
    pub const SUPPLICANT_FAILED: u32 = 10;
    pub const SUPPLICANT_TIMEOUT: u32 = 11;
    pub const DHCP_START_FAILED: u32 = 15;
    pub const DHCP_ERROR: u32 = 16;
    pub const DHCP_FAILED: u32 = 17;
    pub const REMOVED: u32 = 36;
    pub const SLEEPING: u32 = 37;
    pub const CONNECTION_REMOVED: u32 = 38;
    pub const USER_REQUESTED: u32 = 39;
    pub const CARRIER: u32 = 40;
    pub const SSID_NOT_FOUND: u32 = 53;
    pub const NEW_ACTIVATION: u32 = 60;
    pub const IP_ADDRESS_DUPLICATE: u32 = 64;
    /// Networking turned off; `UNMANAGED_SLEEPING` before NetworkManager 1.56.
    pub const UNMANAGED_MANAGER_DISABLED: u32 = 73;
}

/// How long after a connection goes down the notifier decides whether that was news.
/// NetworkManager sends its property changes at idle, after the active connection's
/// `StateChanged`: Wi-Fi turned off or the machine going to sleep shows only then. Whatever
/// shows in this time is kept, since the machine may sleep before it's over.
const SETTLE: Duration = Duration::from_secs(1);
/// How long the user's word that they're activating a connection holds, in case
/// NetworkManager refused the activation and none comes.
const HINT_LIFETIME: Duration = Duration::from_secs(60);

/// Why a connection went down, as NetworkManager gave it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cause {
    /// The active connection's reason, `NMActiveConnectionStateReason`.
    Active(u32),
    /// Its device's reason, `NMDeviceStateReason`.
    Device(u32),
}

impl Cause {
    /// The user did it, or it gave way to another activation: it's no news.
    fn expected(self) -> bool {
        use active_reason as a;
        use device_reason as d;
        match self {
            Cause::Active(reason) => matches!(reason, a::USER_DISCONNECTED | a::CONNECTION_REMOVED),
            Cause::Device(reason) => matches!(
                reason,
                d::USER_REQUESTED
                    | d::SLEEPING
                    | d::CONNECTION_REMOVED
                    | d::NEW_ACTIVATION
                    | d::UNMANAGED_MANAGER_DISABLED
            ),
        }
    }

    /// The reason, as a sentence.
    fn words(self) -> &'static str {
        use active_reason as a;
        use device_reason as d;
        match self {
            Cause::Active(a::DEVICE_DISCONNECTED) => "The network device disconnected.",
            Cause::Active(a::SERVICE_STOPPED) => "The VPN service stopped.",
            Cause::Active(a::IP_CONFIG_INVALID) => "It got no valid IP address.",
            Cause::Active(a::CONNECT_TIMEOUT) => "The attempt timed out.",
            Cause::Active(a::SERVICE_START_TIMEOUT) => "The VPN service took too long to start.",
            Cause::Active(a::SERVICE_START_FAILED) => "The VPN service couldn't start.",
            Cause::Active(a::NO_SECRETS) | Cause::Device(d::NO_SECRETS) => {
                "It needs a password, and none was given."
            }
            Cause::Active(a::LOGIN_FAILED) => "The login failed.",
            Cause::Active(a::DEPENDENCY_FAILED) => "The connection it depends on failed.",
            Cause::Active(a::DEVICE_REALIZE_FAILED) => "Its virtual device couldn't be created.",
            Cause::Active(a::DEVICE_REMOVED) | Cause::Device(d::REMOVED) => {
                "Its network device was removed."
            }
            Cause::Device(d::CONFIG_FAILED) => "Its network device couldn't be set up.",
            Cause::Device(d::IP_CONFIG_UNAVAILABLE) => "It got no IP address.",
            Cause::Device(d::IP_CONFIG_EXPIRED) => "Its IP address expired.",
            // The access point leaves during the handshake when the password is wrong.
            Cause::Device(d::SUPPLICANT_DISCONNECT) => {
                "The network turned it away. The password may be wrong."
            }
            Cause::Device(d::SUPPLICANT_CONFIG_FAILED) => {
                "Its security settings couldn't be applied."
            }
            Cause::Device(d::SUPPLICANT_FAILED) => "The authentication failed.",
            Cause::Device(d::SUPPLICANT_TIMEOUT) => {
                "The authentication took too long. The password may be wrong."
            }
            Cause::Device(d::DHCP_START_FAILED | d::DHCP_ERROR | d::DHCP_FAILED) => {
                "The network gave it no address."
            }
            Cause::Device(d::CARRIER) => "The cable was unplugged.",
            Cause::Device(d::SSID_NOT_FOUND) => "The network isn't in range.",
            Cause::Device(d::IP_ADDRESS_DUPLICATE) => "Another device has its IP address.",
            _ => "The reason is unknown.",
        }
    }
}

/// Exporting the notifier failed.
#[derive(Debug)]
pub enum Error {
    /// The session bus refused the object, as when another is at its path.
    Export(glib::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Export(err) => write!(f, "can't export the notifier: {err}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Export(err) => Some(err),
        }
    }
}

/// How a connection reaches its network, for its icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Wifi,
    Wired,
    Vpn,
    Other,
}

impl Kind {
    fn of(kind: &str, vpn: bool) -> Kind {
        match kind {
            "802-11-wireless" => Kind::Wifi,
            "802-3-ethernet" => Kind::Wired,
            "vpn" | "wireguard" => Kind::Vpn,
            _ if vpn => Kind::Vpn,
            _ => Kind::Other,
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Kind::Wifi => "network-wireless",
            Kind::Wired => "network-wired",
            Kind::Vpn => "network-vpn",
            Kind::Other => "network-transmit-receive",
        }
    }
}

/// An active connection, as the notifier follows it.
#[derive(Clone, Debug)]
struct Active {
    uuid: String,
    /// The connection's name.
    name: String,
    kind: Kind,
    /// Its device, or a VPN's base device. NetworkManager empties the list once it's
    /// deactivated, so it's kept from when it appeared.
    devices: Vec<String>,
    /// It has been up: going down now is a disconnection, not a failure.
    activated: bool,
    /// The user started it, through the Network applet.
    user_started: bool,
    /// The last reason NetworkManager gave for a change, other than none.
    reason: u32,
}

impl Active {
    /// The active connection from its properties, and its state.
    fn from_properties(properties: &HashMap<String, Variant>) -> Option<(Active, u32)> {
        let string = |key: &str| {
            properties
                .get(key)
                .and_then(|value| value.str().map(str::to_owned))
        };
        let devices = properties
            .get("Devices")
            .and_then(Variant::get::<Vec<ObjectPath>>)
            .unwrap_or_default()
            .iter()
            .map(|device| device.as_str().to_owned())
            .collect();
        let vpn = properties
            .get("Vpn")
            .and_then(Variant::get::<bool>)
            .unwrap_or(false);
        let state = properties.get("State").and_then(Variant::get::<u32>)?;
        let active = Active {
            uuid: string("Uuid")?,
            name: string("Id").unwrap_or_default(),
            kind: Kind::of(&string("Type").unwrap_or_default(), vpn),
            devices,
            activated: false,
            user_started: false,
            reason: active_reason::NONE,
        };
        Some((active, state))
    }
}

/// What of NetworkManager's own state tells whether a connection went down by the user's doing.
#[derive(Clone, Copy, Debug)]
struct Manager {
    wireless_enabled: bool,
    /// The radio's hardware switch, airplane mode's key on many laptops.
    wireless_hardware_enabled: bool,
    networking_enabled: bool,
    asleep: bool,
}

impl Default for Manager {
    fn default() -> Manager {
        Manager {
            wireless_enabled: true,
            wireless_hardware_enabled: true,
            networking_enabled: true,
            asleep: false,
        }
    }
}

impl Manager {
    fn update(&mut self, properties: &HashMap<String, Variant>) {
        let flag = |key: &str| properties.get(key).and_then(Variant::get::<bool>);
        if let Some(enabled) = flag("WirelessEnabled") {
            self.wireless_enabled = enabled;
        }
        if let Some(enabled) = flag("WirelessHardwareEnabled") {
            self.wireless_hardware_enabled = enabled;
        }
        if let Some(enabled) = flag("NetworkingEnabled") {
            self.networking_enabled = enabled;
        }
        if let Some(state) = properties.get("State").and_then(Variant::get::<u32>) {
            self.asleep = state == ASLEEP;
        }
    }

    /// The machine sleeps, networking is off, or Wi-Fi is, for a connection that goes with
    /// it: its going down is no news.
    fn expects_down(self, active: &Active) -> bool {
        let wifi_off = !self.wireless_enabled || !self.wireless_hardware_enabled;
        // A VPN goes down with its base: on Wi-Fi, that's the device going.
        let with_its_base = matches!(
            active.reason,
            active_reason::DEVICE_DISCONNECTED | active_reason::DEPENDENCY_FAILED
        );
        self.asleep
            || !self.networking_enabled
            || (wifi_off
                && (active.kind == Kind::Wifi || (active.kind == Kind::Vpn && with_its_base)))
    }
}

/// An active connection that went down, waiting out [`SETTLE`].
struct Ended {
    active: Active,
    /// Its device's reason, once the device has said.
    device_reason: Option<u32>,
    /// Something in the wait showed it to be no news.
    expected: bool,
}

impl Ended {
    fn cause(&self) -> Cause {
        match self.device_reason {
            Some(reason) if self.active.reason == active_reason::DEVICE_DISCONNECTED => {
                Cause::Device(reason)
            }
            _ => Cause::Active(self.active.reason),
        }
    }
}

/// One notification's text.
#[derive(Clone, Debug)]
struct Note {
    icon: &'static str,
    summary: String,
    body: String,
    /// The notification spec's category.
    category: &'static str,
}

/// What a connection's notification is to become once the server answers the `Notify` on
/// its way.
enum Queued {
    Show(Note),
    Close,
}

/// A connection's notification on the server.
#[derive(Default)]
struct Slot {
    /// The server's id for it, while it's shown.
    id: Option<u32>,
    /// A `Notify` is on its way, whose answer gives the id.
    in_flight: bool,
    queued: Option<Queued>,
}

struct State {
    system: gio::DBusConnection,
    session: gio::DBusConnection,
    /// NetworkManager's state, once it's on the bus and its objects have been read.
    manager: Option<Manager>,
    /// The active connections, by path.
    active: HashMap<String, Active>,
    /// Why each device last went down, by path, until it starts its next activation.
    devices: HashMap<String, u32>,
    /// The connections that went down and wait out [`SETTLE`], by number.
    ended: HashMap<u64, Ended>,
    last_ended: u64,
    /// The connections the user said they're activating, by UUID, with when they said so.
    hints: HashMap<String, Instant>,
    /// Each connection's notification, by UUID.
    slots: HashMap<String, Slot>,
    /// Counts NetworkManager's arrivals and departures, so that the answer to a read sent
    /// before it restarted doesn't replace what its new instance said since.
    generation: u64,
}

/// The notifier, following NetworkManager on the system bus and exported on the session bus,
/// for as long as it lives.
pub struct Notifier {
    session: gio::DBusConnection,
    registration: Option<gio::RegistrationId>,
    _subscriptions: Vec<gio::SignalSubscription>,
    /// Stops watching NetworkManager's name. A closure, since the watcher's id is of a type
    /// gio doesn't export.
    unwatch: Option<Box<dyn FnOnce()>>,
    _state: Rc<RefCell<State>>,
}

impl Drop for Notifier {
    fn drop(&mut self) {
        if let Some(registration) = self.registration.take() {
            let _ = self.session.unregister_object(registration);
        }
        if let Some(unwatch) = self.unwatch.take() {
            unwatch();
        }
    }
}

impl Notifier {
    /// Follows NetworkManager on `system`, and notifies through the notification server on
    /// `session`, where it's exported at [`PATH`].
    pub fn new(
        system: &gio::DBusConnection,
        session: &gio::DBusConnection,
    ) -> Result<Notifier, Error> {
        let state = Rc::new(RefCell::new(State {
            system: system.clone(),
            session: session.clone(),
            manager: None,
            active: HashMap::new(),
            devices: HashMap::new(),
            ended: HashMap::new(),
            last_ended: 0,
            hints: HashMap::new(),
            slots: HashMap::new(),
            generation: 0,
        }));

        let node = gio::DBusNodeInfo::for_xml(INTERFACE_XML).expect("the interface is valid");
        let interface = node
            .lookup_interface(INTERFACE)
            .expect("the node has the notifier's interface");
        let registration = session
            .register_object(PATH, &interface)
            .method_call({
                let state = Rc::downgrade(&state);
                move |_, _, _, _, method, parameters, invocation| match (method, state.upgrade()) {
                    ("UserActivating", Some(state)) => {
                        let uuid = parameters.child_value(0);
                        user_activating(&state, uuid.str().expect("the UUID is a string"));
                        invocation.return_value(None);
                    }
                    _ => invocation.return_dbus_error(
                        "org.freedesktop.DBus.Error.UnknownMethod",
                        &format!("the notifier has no method {method}"),
                    ),
                }
            })
            .build()
            .map_err(Error::Export)?;

        // Subscribed before the objects are read, so no change falls between the two.
        let subscriptions = vec![
            subscribe(
                &state,
                system,
                NETWORK_MANAGER,
                OBJECT_MANAGER,
                "InterfacesAdded",
            ),
            subscribe(
                &state,
                system,
                NETWORK_MANAGER,
                OBJECT_MANAGER,
                "InterfacesRemoved",
            ),
            subscribe(
                &state,
                system,
                NETWORK_MANAGER,
                PROPERTIES,
                "PropertiesChanged",
            ),
            subscribe(&state, system, NETWORK_MANAGER, ACTIVE, "StateChanged"),
            subscribe(&state, system, NETWORK_MANAGER, DEVICE, "StateChanged"),
            subscribe(
                &state,
                session,
                NOTIFICATIONS,
                NOTIFICATIONS,
                "NotificationClosed",
            ),
        ];

        let appeared = Rc::downgrade(&state);
        let vanished = Rc::downgrade(&state);
        let watcher = gio::bus_watch_name_on_connection(
            system,
            NETWORK_MANAGER,
            gio::BusNameWatcherFlags::NONE,
            move |_, _, _| {
                if let Some(state) = appeared.upgrade() {
                    let generation = next_generation(&state);
                    glib::spawn_future_local(read_objects(Rc::downgrade(&state), generation));
                }
            },
            move |_, _| {
                if let Some(state) = vanished.upgrade() {
                    next_generation(&state);
                    let mut state = state.borrow_mut();
                    state.manager = None;
                    state.active.clear();
                    state.devices.clear();
                }
            },
        );

        Ok(Notifier {
            session: session.clone(),
            registration: Some(registration),
            _subscriptions: subscriptions,
            unwatch: Some(Box::new(move || gio::bus_unwatch_name(watcher))),
            _state: state,
        })
    }
}

fn next_generation(state: &Rc<RefCell<State>>) -> u64 {
    let mut state = state.borrow_mut();
    state.generation += 1;
    state.generation
}

/// An object's interfaces and their properties, as D-Bus sends them.
type Interfaces = HashMap<String, HashMap<String, Variant>>;

/// Reads NetworkManager's objects as it comes on the bus. The connections already up are no
/// news: only what moves after is.
async fn read_objects(state: Weak<RefCell<State>>, generation: u64) {
    let Some(system) = state.upgrade().map(|state| state.borrow().system.clone()) else {
        return;
    };
    let answer = system
        .call_future(
            Some(NETWORK_MANAGER),
            ROOT,
            OBJECT_MANAGER,
            "GetManagedObjects",
            None,
            Some(glib::VariantTy::new("(a{oa{sa{sv}}})").expect("the type is valid")),
            gio::DBusCallFlags::NONE,
            -1,
        )
        .await;
    let Some(state) = state.upgrade() else {
        return;
    };
    let mut state = state.borrow_mut();
    match answer {
        Ok(_) if state.generation != generation => {}
        Ok(managed) => {
            let objects: HashMap<ObjectPath, Interfaces> = managed
                .child_value(0)
                .get()
                .expect("the objects are a{oa{sa{sv}}}");
            let mut manager = Manager::default();
            for (path, interfaces) in objects {
                if path.as_str() == MANAGER_PATH
                    && let Some(properties) = interfaces.get(MANAGER)
                {
                    manager.update(properties);
                }
                if let Some((mut active, connection_state)) =
                    interfaces.get(ACTIVE).and_then(Active::from_properties)
                {
                    active.activated = connection_state == ACTIVATED;
                    // A change that came before the answer knows better.
                    state
                        .active
                        .entry(path.as_str().to_owned())
                        .or_insert(active);
                }
            }
            state.manager = Some(manager);
        }
        Err(err) => glib::g_warning!(DOMAIN, "can't read NetworkManager's objects: {err}"),
    }
}

fn subscribe(
    state: &Rc<RefCell<State>>,
    connection: &gio::DBusConnection,
    sender: &str,
    interface: &str,
    member: &str,
) -> gio::SignalSubscription {
    let weak = Rc::downgrade(state);
    connection.subscribe_to_signal(
        Some(sender),
        Some(interface),
        Some(member),
        None,
        None,
        gio::DBusSignalFlags::NONE,
        move |signal| {
            if let Some(state) = weak.upgrade() {
                signal_received(
                    &state,
                    signal.interface_name,
                    signal.signal_name,
                    signal.object_path,
                    signal.parameters,
                );
            }
        },
    )
}

fn signal_received(
    state: &Rc<RefCell<State>>,
    interface: &str,
    member: &str,
    path: &str,
    parameters: &Variant,
) {
    match (interface, member) {
        (OBJECT_MANAGER, "InterfacesAdded") => {
            let Some((path, interfaces)) = parameters.get::<(ObjectPath, Interfaces)>() else {
                return;
            };
            if let Some((active, connection_state)) =
                interfaces.get(ACTIVE).and_then(Active::from_properties)
            {
                added(state, path.as_str(), active, connection_state);
            }
        }
        (OBJECT_MANAGER, "InterfacesRemoved") => {
            if let Some((path, interfaces)) = parameters.get::<(ObjectPath, Vec<String>)>()
                && interfaces.iter().any(|interface| interface == ACTIVE)
            {
                state.borrow_mut().active.remove(path.as_str());
            }
        }
        (PROPERTIES, "PropertiesChanged") => {
            if path != MANAGER_PATH {
                return;
            }
            let Some((interface, changed, _)) =
                parameters.get::<(String, HashMap<String, Variant>, Vec<String>)>()
            else {
                return;
            };
            if interface == MANAGER {
                manager_changed(state, &changed);
            }
        }
        (ACTIVE, "StateChanged") => {
            if let Some((connection_state, reason)) = parameters.get::<(u32, u32)>() {
                state_changed(state, path, connection_state, reason);
            }
        }
        (DEVICE, "StateChanged") => {
            if let Some((new_state, _, reason)) = parameters.get::<(u32, u32, u32)>() {
                device_changed(state, path, new_state, reason);
            }
        }
        (NOTIFICATIONS, "NotificationClosed") => {
            if let Some((id, _)) = parameters.get::<(u32, u32)>() {
                for slot in state.borrow_mut().slots.values_mut() {
                    if slot.id == Some(id) {
                        slot.id = None;
                    }
                }
            }
        }
        _ => {}
    }
}

fn manager_changed(state: &Rc<RefCell<State>>, changed: &HashMap<String, Variant>) {
    let mut state = state.borrow_mut();
    let Some(manager) = &mut state.manager else {
        return;
    };
    manager.update(changed);
    let manager = *manager;
    for ended in state.ended.values_mut() {
        ended.expected |= manager.expects_down(&ended.active);
    }
}

/// A device's state moved: on its way down, its reason is why its connection went.
fn device_changed(state: &Rc<RefCell<State>>, path: &str, new_state: u32, reason: u32) {
    use device_state as s;
    let mut state = state.borrow_mut();
    match new_state {
        s::PREPARE..=s::ACTIVATED => {
            state.devices.remove(path);
        }
        s::UNMANAGED | s::UNAVAILABLE | s::DISCONNECTED | s::DEACTIVATING | s::FAILED => {
            // Failing, it goes on to DISCONNECTED with no reason: the failure's is the one.
            if matches!(reason, device_reason::NONE | device_reason::UNKNOWN) {
                return;
            }
            state.devices.insert(path.to_owned(), reason);
            for ended in state.ended.values_mut() {
                if ended.active.devices.iter().any(|device| device == path) {
                    ended.device_reason = Some(reason);
                }
            }
        }
        _ => {}
    }
}

/// The user's word, from the Network applet, that they're activating the connection `uuid`.
fn user_activating(state: &Rc<RefCell<State>>, uuid: &str) {
    let mut state = state.borrow_mut();
    // The applet may have heard back from NetworkManager first.
    let mut found = false;
    for active in state.active.values_mut() {
        if active.uuid == uuid && !active.activated {
            active.user_started = true;
            found = true;
        }
    }
    if !found {
        let now = Instant::now();
        state
            .hints
            .retain(|_, said| now.duration_since(*said) < HINT_LIFETIME);
        state.hints.insert(uuid.to_owned(), now);
    }
}

/// A new active connection: an activation that has just started, or that NetworkManager
/// reports already up.
fn added(state: &Rc<RefCell<State>>, path: &str, mut active: Active, connection_state: u32) {
    {
        let mut state = state.borrow_mut();
        if state.active.contains_key(path) {
            return;
        }
        active.user_started = state
            .hints
            .remove(&active.uuid)
            .is_some_and(|said| said.elapsed() < HINT_LIFETIME);
        state.active.insert(path.to_owned(), active);
    }
    if connection_state == ACTIVATED {
        state_changed(state, path, ACTIVATED, active_reason::NONE);
    }
}

fn state_changed(state: &Rc<RefCell<State>>, path: &str, connection_state: u32, reason: u32) {
    let mut borrowed = state.borrow_mut();
    let Some(active) = borrowed.active.get_mut(path) else {
        return;
    };
    if reason != active_reason::NONE {
        active.reason = reason;
    }
    match connection_state {
        ACTIVATED if !active.activated => {
            active.activated = true;
            let active = active.clone();
            drop(borrowed);
            if active.user_started {
                // What it said before is old news to the user who brought it back.
                close(state, &active.uuid);
            } else {
                let note = Note {
                    icon: active.kind.icon(),
                    summary: format!("Connected to {}", active.name),
                    body: String::new(),
                    category: "network.connected",
                };
                show(state, &active.uuid, note);
            }
        }
        DEACTIVATED => {
            // NetworkManager never brings an active connection back: a new activation is a
            // new one.
            let active = borrowed
                .active
                .remove(path)
                .expect("the active connection is followed");
            // The device may have told its reason before the connection did.
            let device_reason = active
                .devices
                .iter()
                .find_map(|device| borrowed.devices.get(device).copied());
            let expected = borrowed
                .manager
                .is_some_and(|manager| manager.expects_down(&active));
            borrowed.last_ended += 1;
            let id = borrowed.last_ended;
            borrowed.ended.insert(
                id,
                Ended {
                    active,
                    device_reason,
                    expected,
                },
            );
            drop(borrowed);
            let weak = Rc::downgrade(state);
            glib::spawn_future_local(async move {
                glib::timeout_future(SETTLE).await;
                if let Some(state) = weak.upgrade() {
                    went_down(&state, id);
                }
            });
        }
        _ => {}
    }
}

/// Notifies of the connection `id` that went down, unless that was no news.
fn went_down(state: &Rc<RefCell<State>>, id: u64) {
    let (uuid, note) = {
        let mut state = state.borrow_mut();
        let Some(ended) = state.ended.remove(&id) else {
            return;
        };
        // NetworkManager left: its connections went with it.
        let Some(manager) = state.manager else {
            return;
        };
        let cause = ended.cause();
        if ended.expected
            || manager.expects_down(&ended.active)
            || cause.expected()
            || Cause::Active(ended.active.reason).expected()
        {
            return;
        }
        let active = &ended.active;
        let note = if active.activated {
            Note {
                icon: active.kind.icon(),
                summary: "Disconnected".to_owned(),
                body: format!("The connection to {} dropped.", active.name),
                category: "network.disconnected",
            }
        } else {
            Note {
                icon: active.kind.icon(),
                summary: "Connection failed".to_owned(),
                body: format!("Couldn't connect to {}. {}", active.name, cause.words()),
                category: "network.error",
            }
        };
        (active.uuid.clone(), note)
    };
    show(state, &uuid, note);
}

/// Shows `note` as the connection `uuid`'s notification, in place of the one it has.
fn show(state: &Rc<RefCell<State>>, uuid: &str, note: Note) {
    let mut borrowed = state.borrow_mut();
    let session = borrowed.session.clone();
    let slot = borrowed.slots.entry(uuid.to_owned()).or_default();
    if slot.in_flight {
        // The id it replaces isn't known yet.
        slot.queued = Some(Queued::Show(note));
        return;
    }
    slot.in_flight = true;
    let replaces = slot.id.unwrap_or(0);
    drop(borrowed);

    let hints = HashMap::from([("category".to_owned(), note.category.to_variant())]);
    let actions: Vec<String> = Vec::new();
    let parameters = (
        "Network",
        replaces,
        note.icon,
        note.summary,
        note.body,
        actions,
        hints,
        -1i32,
    )
        .to_variant();
    let weak = Rc::downgrade(state);
    let uuid = uuid.to_owned();
    glib::spawn_future_local(async move {
        let answer = session
            .call_future(
                Some(NOTIFICATIONS),
                NOTIFICATIONS_PATH,
                NOTIFICATIONS,
                "Notify",
                Some(&parameters),
                Some(glib::VariantTy::new("(u)").expect("the type is valid")),
                gio::DBusCallFlags::NONE,
                -1,
            )
            .await;
        let Some(state) = weak.upgrade() else {
            return;
        };
        let queued = {
            let mut borrowed = state.borrow_mut();
            let slot = borrowed.slots.entry(uuid.clone()).or_default();
            slot.in_flight = false;
            match answer {
                Ok(id) => slot.id = id.child_value(0).get(),
                Err(err) => glib::g_warning!(DOMAIN, "can't show a notification: {err}"),
            }
            slot.queued.take()
        };
        match queued {
            Some(Queued::Show(note)) => show(&state, &uuid, note),
            Some(Queued::Close) => close(&state, &uuid),
            None => {}
        }
    });
}

/// Closes the connection `uuid`'s notification, if it has one.
fn close(state: &Rc<RefCell<State>>, uuid: &str) {
    let mut borrowed = state.borrow_mut();
    let session = borrowed.session.clone();
    let Some(slot) = borrowed.slots.get_mut(uuid) else {
        return;
    };
    if slot.in_flight {
        slot.queued = Some(Queued::Close);
        return;
    }
    let Some(id) = slot.id.take() else {
        return;
    };
    drop(borrowed);
    session.call(
        Some(NOTIFICATIONS),
        NOTIFICATIONS_PATH,
        NOTIFICATIONS,
        "CloseNotification",
        Some(&(id,).to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        -1,
        gio::Cancellable::NONE,
        |answer| {
            if let Err(err) = answer {
                glib::g_warning!(DOMAIN, "can't close a notification: {err}");
            }
        },
    );
}
