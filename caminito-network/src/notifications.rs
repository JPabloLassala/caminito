//! The connection notifications (CMN-199): the ones nm-applet sent, through the session's
//! `org.freedesktop.Notifications`, with the connection's icon.
//!
//! - "Connection failed", with the reason NetworkManager gives in the active connection's
//!   `StateChanged`, in words.
//! - "Disconnected", when a connection that was up goes down without the user's doing.
//! - "Connected to *X*", for an activation the user didn't start, as an auto-connect after
//!   resume.
//!
//! Each connection has one notification at a time, replaced as its state moves.
//!
//! What the user did is read from NetworkManager where it says so: a deactivation through
//! `DeactivateConnection` or the device's `Disconnect` has the reason `USER_DISCONNECTED`, and
//! Wi-Fi or airplane mode turned off shows in `WirelessEnabled`. An activation doesn't say who
//! started it, so the Network applet tells the notifier first, with [`INTERFACE`]'s
//! `UserActivating`, on the session bus. An activation of a connection that no one named there
//! is NetworkManager's own.

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
const OBJECT_MANAGER: &str = "org.freedesktop.DBus.ObjectManager";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";

const NOTIFICATIONS: &str = "org.freedesktop.Notifications";
const NOTIFICATIONS_PATH: &str = "/org/freedesktop/Notifications";

/// NetworkManager's `NMActiveConnectionState`.
const ACTIVATED: u32 = 2;
const DEACTIVATED: u32 = 4;

/// NetworkManager's `NMState` while the machine sleeps.
const ASLEEP: u32 = 10;

/// NetworkManager's `NMActiveConnectionStateReason`, the ones the notifier tells apart.
const REASON_NONE: u32 = 1;
const USER_DISCONNECTED: u32 = 2;
const DEVICE_DISCONNECTED: u32 = 3;
const CONNECTION_REMOVED: u32 = 11;
const DEPENDENCY_FAILED: u32 = 12;

/// How long after a connection goes down the notifier decides whether that was news.
/// NetworkManager sends its property changes at idle, after the active connection's
/// `StateChanged`: Wi-Fi turned off, or the device's next activation, shows only then.
const SETTLE: Duration = Duration::from_secs(1);
/// How long the user's word that they're activating a connection holds, in case
/// NetworkManager refused the activation and none comes.
const HINT_LIFETIME: Duration = Duration::from_secs(60);

/// The words for an active connection's `StateChanged` reason, as a sentence.
pub fn reason(code: u32) -> &'static str {
    match code {
        DEVICE_DISCONNECTED => "The network device disconnected.",
        4 => "The VPN service stopped.",
        5 => "It got no valid IP address.",
        6 => "The attempt timed out.",
        7 => "The VPN service took too long to start.",
        8 => "The VPN service couldn't start.",
        9 => "It needs a password, and none was given.",
        10 => "The login failed.",
        DEPENDENCY_FAILED => "The connection it depends on failed.",
        13 => "Its virtual device couldn't be created.",
        14 => "Its network device was removed.",
        _ => "The reason is unknown.",
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
            reason: REASON_NONE,
        };
        Some((active, state))
    }
}

/// What of NetworkManager's own state tells whether a connection went down by the user's doing.
#[derive(Clone, Copy, Debug)]
struct Manager {
    wireless_enabled: bool,
    networking_enabled: bool,
    asleep: bool,
}

impl Manager {
    fn from_properties(properties: &HashMap<String, Variant>) -> Manager {
        let mut manager = Manager {
            wireless_enabled: true,
            networking_enabled: true,
            asleep: false,
        };
        manager.update(properties);
        manager
    }

    fn update(&mut self, properties: &HashMap<String, Variant>) {
        let flag = |key: &str| properties.get(key).and_then(Variant::get::<bool>);
        if let Some(enabled) = flag("WirelessEnabled") {
            self.wireless_enabled = enabled;
        }
        if let Some(enabled) = flag("NetworkingEnabled") {
            self.networking_enabled = enabled;
        }
        if let Some(state) = properties.get("State").and_then(Variant::get::<u32>) {
            self.asleep = state == ASLEEP;
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

/// A connection's notification on the server.
#[derive(Default)]
struct Slot {
    /// The server's id for it, while it's shown.
    id: Option<u32>,
    /// A `Notify` is on its way, whose answer gives the id.
    in_flight: bool,
    /// What to show once that answer comes, or None to close it.
    queued: Option<Option<Note>>,
}

struct State {
    system: gio::DBusConnection,
    session: gio::DBusConnection,
    /// NetworkManager's state, once it's on the bus and its objects have been read.
    manager: Option<Manager>,
    /// The active connections, by path.
    active: HashMap<String, Active>,
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

/// A connection's settings or an object's interfaces, as D-Bus sends them.
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
            for (path, interfaces) in objects {
                if path.as_str() == MANAGER_PATH
                    && let Some(properties) = interfaces.get(MANAGER)
                {
                    state.manager = Some(Manager::from_properties(properties));
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
            state.manager.get_or_insert(Manager {
                wireless_enabled: true,
                networking_enabled: true,
                asleep: false,
            });
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
                    signal.signal_name,
                    signal.object_path,
                    signal.parameters,
                );
            }
        },
    )
}

fn signal_received(state: &Rc<RefCell<State>>, member: &str, path: &str, parameters: &Variant) {
    match member {
        "InterfacesAdded" => {
            let Some((path, interfaces)) = parameters.get::<(ObjectPath, Interfaces)>() else {
                return;
            };
            if let Some((active, connection_state)) =
                interfaces.get(ACTIVE).and_then(Active::from_properties)
            {
                added(state, path.as_str(), active, connection_state);
            }
        }
        "InterfacesRemoved" => {
            if let Some((path, interfaces)) = parameters.get::<(ObjectPath, Vec<String>)>()
                && interfaces.iter().any(|interface| interface == ACTIVE)
            {
                state.borrow_mut().active.remove(path.as_str());
            }
        }
        "PropertiesChanged" => {
            if path != MANAGER_PATH {
                return;
            }
            let Some((interface, changed, _)) =
                parameters.get::<(String, HashMap<String, Variant>, Vec<String>)>()
            else {
                return;
            };
            if interface == MANAGER
                && let Some(manager) = &mut state.borrow_mut().manager
            {
                manager.update(&changed);
            }
        }
        "StateChanged" => {
            if let Some((connection_state, reason)) = parameters.get::<(u32, u32)>() {
                state_changed(state, path, connection_state, reason);
            }
        }
        "NotificationClosed" => {
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

/// The user's word, from the Network applet, that they're activating the connection `uuid`.
fn user_activating(state: &Rc<RefCell<State>>, uuid: &str) {
    let mut state = state.borrow_mut();
    // The applet may have heard back from NetworkManager first.
    let coming_up = state
        .active
        .values_mut()
        .filter(|active| active.uuid == uuid && !active.activated);
    let mut found = false;
    for active in coming_up {
        active.user_started = true;
        found = true;
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
        state_changed(state, path, ACTIVATED, REASON_NONE);
    }
}

fn state_changed(state: &Rc<RefCell<State>>, path: &str, connection_state: u32, reason: u32) {
    let mut borrowed = state.borrow_mut();
    let Some(active) = borrowed.active.get_mut(path) else {
        return;
    };
    if reason != REASON_NONE {
        active.reason = reason;
    }
    match connection_state {
        ACTIVATED if !active.activated => {
            active.activated = true;
            let active = active.clone();
            drop(borrowed);
            if active.user_started {
                // What it said before is old news to the user who brought it back.
                show(state, &active.uuid, None);
            } else {
                let note = Note {
                    icon: active.kind.icon(),
                    summary: format!("Connected to {}", active.name),
                    body: String::new(),
                    category: "network.connected",
                };
                show(state, &active.uuid, Some(note));
            }
        }
        DEACTIVATED => {
            // NetworkManager never brings an active connection back: a new activation is a
            // new one.
            let ended = borrowed
                .active
                .remove(path)
                .expect("the active connection is followed");
            drop(borrowed);
            let weak = Rc::downgrade(state);
            glib::spawn_future_local(async move {
                glib::timeout_future(SETTLE).await;
                if let Some(state) = weak.upgrade() {
                    went_down(&state, ended);
                }
            });
        }
        _ => {}
    }
}

/// Notifies of a connection that went down, unless the user did it.
fn went_down(state: &Rc<RefCell<State>>, ended: Active) {
    let note = {
        let state = state.borrow();
        let Some(manager) = state.manager else {
            return;
        };
        let users = matches!(ended.reason, USER_DISCONNECTED | CONNECTION_REMOVED)
            || manager.asleep
            || !manager.networking_enabled
            || (!manager.wireless_enabled
                && (ended.kind == Kind::Wifi
                    || (ended.kind == Kind::Vpn
                        && matches!(ended.reason, DEVICE_DISCONNECTED | DEPENDENCY_FAILED))));
        // Another connection coming up on the device took its place: the user chose another
        // network, or NetworkManager roamed to one. A VPN names its base's device, which it
        // doesn't take the place of.
        let replaced = ended.kind != Kind::Vpn
            && state.active.values().any(|other| {
                other.kind != Kind::Vpn
                    && other
                        .devices
                        .iter()
                        .any(|device| ended.devices.contains(device))
            });
        if users || replaced {
            return;
        }
        if ended.activated {
            Note {
                icon: ended.kind.icon(),
                summary: "Disconnected".to_owned(),
                body: format!("The connection to {} dropped.", ended.name),
                category: "network.disconnected",
            }
        } else {
            Note {
                icon: ended.kind.icon(),
                summary: "Connection failed".to_owned(),
                body: format!(
                    "Couldn't connect to {}. {}",
                    ended.name,
                    reason(ended.reason)
                ),
                category: "network.error",
            }
        }
    };
    show(state, &ended.uuid, Some(note));
}

/// Shows `note` as the connection `uuid`'s notification, in place of the one it has, or
/// closes that one for None.
fn show(state: &Rc<RefCell<State>>, uuid: &str, note: Option<Note>) {
    let mut borrowed = state.borrow_mut();
    let session = borrowed.session.clone();
    let slot = borrowed.slots.entry(uuid.to_owned()).or_default();
    if slot.in_flight {
        // The id it replaces isn't known yet.
        slot.queued = Some(note);
        return;
    }
    let Some(note) = note else {
        if let Some(id) = slot.id.take() {
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
        return;
    };
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
        if let Some(note) = queued {
            show(&state, &uuid, note);
        }
    });
}
