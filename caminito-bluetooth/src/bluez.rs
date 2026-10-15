//! The model of BlueZ (255): the adapters, devices and battery levels that `org.bluez`'s
//! object manager publishes, followed as they appear, change and go, and the calls that act
//! on them. It speaks gio's D-Bus, not zbus (ADR 0035), on the connection it's given: the
//! system bus in a session, a private bus in the tests.
//!
//! The settings window (257, 258) and the pairing agent (256) build on it. The Bluetooth
//! applet reads the same objects itself, in C (ADR 0035).

mod objects;

use std::cell::RefCell;
use std::fmt;
use std::rc::{Rc, Weak};

use gio::glib::variant::{ObjectPath, ToVariant};
use gio::glib::{self, Variant};

use objects::{ADAPTER, DEVICE, Objects};

/// BlueZ's well-known name.
const BLUEZ: &str = "org.bluez";
const OBJECT_MANAGER: &str = "org.freedesktop.DBus.ObjectManager";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";

/// A Bluetooth adapter, as `org.bluez.Adapter1` describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Adapter {
    /// Its object path, such as `/org/bluez/hci0`.
    pub path: String,
    pub address: String,
    /// The name other devices see, which the user can change.
    pub alias: String,
    pub powered: bool,
    pub discoverable: bool,
    pub discovering: bool,
}

/// A device an adapter knows of, paired or seen in a scan, as `org.bluez.Device1` and
/// `org.bluez.Battery1` describe it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    /// Its object path, such as `/org/bluez/hci0/dev_11_22_33_44_55_66`.
    pub path: String,
    /// Its adapter's object path.
    pub adapter: String,
    pub address: String,
    /// The name the device gives itself, which it may not.
    pub name: Option<String>,
    /// The name to show: the user's own, else the device's, else its address.
    pub alias: String,
    /// A freedesktop icon name, such as `audio-headset`.
    pub icon: Option<String>,
    pub paired: bool,
    pub trusted: bool,
    pub connected: bool,
    /// Its battery's percentage, when it reports one.
    pub battery: Option<u8>,
}

/// A call to BlueZ that failed.
#[derive(Debug)]
pub enum Error {
    /// The path asked for isn't an object path.
    Path(String),
    /// BlueZ, or the bus, answered with an error.
    Call {
        /// What was asked, as in "connect /org/bluez/hci0/dev_…".
        action: String,
        source: glib::Error,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Path(path) => write!(f, "can't reach {path:?}: it isn't an object path"),
            Error::Call { action, source } => write!(f, "can't {action}: {source}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Path(_) => None,
            Error::Call { source, .. } => Some(source),
        }
    }
}

type Listener = Rc<dyn Fn(&Bluez)>;

struct Inner {
    connection: gio::DBusConnection,
    objects: Objects,
    /// Whether BlueZ is on the bus and its objects have been read.
    ready: bool,
    listeners: Vec<Listener>,
    /// Held for as long as the model lives: dropping them unsubscribes.
    subscriptions: Vec<gio::SignalSubscription>,
    /// Stops watching BlueZ's name. The watcher's id is a closure's because gio's glob
    /// export of it is shadowed by another `WatcherId`, so it can't be named.
    unwatch: Option<Box<dyn FnOnce()>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Some(unwatch) = self.unwatch.take() {
            unwatch();
        }
    }
}

/// BlueZ's adapters and devices, kept up to date on the thread-default main context it was
/// made on. Clones share one model.
#[derive(Clone)]
pub struct Bluez {
    inner: Rc<RefCell<Inner>>,
}

impl Bluez {
    /// Follows BlueZ on `connection`. Its objects are read once BlueZ is on the bus, and
    /// forgotten when it leaves; [`Bluez::connect_changed`] tells of each.
    pub fn new(connection: &gio::DBusConnection) -> Bluez {
        let bluez = Bluez {
            inner: Rc::new(RefCell::new(Inner {
                connection: connection.clone(),
                objects: Objects::default(),
                ready: false,
                listeners: Vec::new(),
                subscriptions: Vec::new(),
                unwatch: None,
            })),
        };

        // Subscribed before the objects are read, so no change falls between the two. The
        // answer to GetManagedObjects replaces whatever came before it.
        let subscriptions = vec![
            bluez.subscribe(Some(OBJECT_MANAGER), "InterfacesAdded", |objects, _, p| {
                objects.interfaces_added(p)
            }),
            bluez.subscribe(
                Some(OBJECT_MANAGER),
                "InterfacesRemoved",
                |objects, _, p| objects.interfaces_removed(p),
            ),
            bluez.subscribe(Some(PROPERTIES), "PropertiesChanged", |objects, path, p| {
                objects.properties_changed(path, p)
            }),
        ];

        let appeared = Rc::downgrade(&bluez.inner);
        let vanished = Rc::downgrade(&bluez.inner);
        let watcher = gio::bus_watch_name_on_connection(
            connection,
            BLUEZ,
            gio::BusNameWatcherFlags::NONE,
            move |_, _, _| {
                if let Some(bluez) = upgrade(&appeared) {
                    glib::spawn_future_local(async move { bluez.read_objects().await });
                }
            },
            move |_, _| {
                if let Some(bluez) = upgrade(&vanished) {
                    bluez.update(|inner| {
                        inner.objects.clear();
                        inner.ready = false;
                    });
                }
            },
        );

        let mut inner = bluez.inner.borrow_mut();
        inner.subscriptions = subscriptions;
        inner.unwatch = Some(Box::new(move || gio::bus_unwatch_name(watcher)));
        drop(inner);
        bluez
    }

    /// Whether BlueZ is running and its objects have been read.
    pub fn is_ready(&self) -> bool {
        self.inner.borrow().ready
    }

    /// Every adapter, in the order of their paths.
    pub fn adapters(&self) -> Vec<Adapter> {
        self.inner.borrow().objects.adapters()
    }

    /// Every device of every adapter, in the order of their paths.
    pub fn devices(&self) -> Vec<Device> {
        self.inner.borrow().objects.devices()
    }

    /// Calls `listener` after each change: BlueZ arriving or leaving, an object appearing
    /// or going, a property changing.
    pub fn connect_changed(&self, listener: impl Fn(&Bluez) + 'static) {
        self.inner.borrow_mut().listeners.push(Rc::new(listener));
    }

    /// Connects the device at `device`, with every profile it allows.
    pub async fn connect(&self, device: &str) -> Result<(), Error> {
        self.call(device, DEVICE, "Connect", None, "connect").await
    }

    /// Disconnects the device at `device`.
    pub async fn disconnect(&self, device: &str) -> Result<(), Error> {
        self.call(device, DEVICE, "Disconnect", None, "disconnect")
            .await
    }

    /// Gives the device or adapter at `path` the name `alias`. An empty one gives back its
    /// own name.
    pub async fn set_alias(&self, path: &str, alias: &str) -> Result<(), Error> {
        let interface = if self.adapters().iter().any(|adapter| adapter.path == path) {
            ADAPTER
        } else {
            DEVICE
        };
        self.set(path, interface, "Alias", alias.to_variant()).await
    }

    /// Forgets the device at `device`: BlueZ unpairs it and removes it from its adapter.
    pub async fn remove(&self, device: &str) -> Result<(), Error> {
        let adapter = self
            .devices()
            .into_iter()
            .find(|known| known.path == device)
            .map(|known| known.adapter)
            .unwrap_or_else(|| parent(device).to_owned());
        let parameters = (object_path(device)?,).to_variant();
        self.call(
            &adapter,
            ADAPTER,
            "RemoveDevice",
            Some(&parameters),
            "remove",
        )
        .await
    }

    /// Powers the adapter at `adapter` on or off.
    pub async fn set_powered(&self, adapter: &str, powered: bool) -> Result<(), Error> {
        self.set(adapter, ADAPTER, "Powered", powered.to_variant())
            .await
    }

    /// Makes the adapter at `adapter` visible to other devices, or hides it.
    pub async fn set_discoverable(&self, adapter: &str, discoverable: bool) -> Result<(), Error> {
        self.set(adapter, ADAPTER, "Discoverable", discoverable.to_variant())
            .await
    }

    /// Starts the adapter at `adapter` looking for devices.
    pub async fn start_discovery(&self, adapter: &str) -> Result<(), Error> {
        self.call(
            adapter,
            ADAPTER,
            "StartDiscovery",
            None,
            "start discovery on",
        )
        .await
    }

    /// Stops the adapter at `adapter` looking for devices.
    pub async fn stop_discovery(&self, adapter: &str) -> Result<(), Error> {
        self.call(adapter, ADAPTER, "StopDiscovery", None, "stop discovery on")
            .await
    }

    async fn read_objects(&self) {
        let connection = self.inner.borrow().connection.clone();
        let answer = connection
            .call_future(
                Some(BLUEZ),
                "/",
                OBJECT_MANAGER,
                "GetManagedObjects",
                None,
                Some(glib::VariantTy::new("(a{oa{sa{sv}}})").expect("the type is valid")),
                gio::DBusCallFlags::NONE,
                -1,
            )
            .await;
        match answer {
            Ok(managed) => self.update(|inner| {
                inner.objects.replace_all(&managed);
                inner.ready = true;
            }),
            Err(err) => glib::g_warning!("caminito-bluetooth", "can't read BlueZ's objects: {err}"),
        }
    }

    async fn set(
        &self,
        path: &str,
        interface: &str,
        property: &str,
        value: Variant,
    ) -> Result<(), Error> {
        let parameters = (interface, property, value).to_variant();
        let action = format!("set {property} of");
        self.call(path, PROPERTIES, "Set", Some(&parameters), &action)
            .await
    }

    async fn call(
        &self,
        path: &str,
        interface: &str,
        method: &str,
        parameters: Option<&Variant>,
        action: &str,
    ) -> Result<(), Error> {
        object_path(path)?;
        let connection = self.inner.borrow().connection.clone();
        connection
            .call_future(
                Some(BLUEZ),
                path,
                interface,
                method,
                parameters,
                None,
                gio::DBusCallFlags::NONE,
                // Connecting can take as long as BlueZ's own page timeout and more.
                60_000,
            )
            .await
            .map(drop)
            .map_err(|source| Error::Call {
                action: format!("{action} {path}"),
                source,
            })
    }

    fn subscribe(
        &self,
        interface: Option<&str>,
        member: &str,
        apply: impl Fn(&mut Objects, &str, &Variant) + 'static,
    ) -> gio::SignalSubscription {
        let weak = Rc::downgrade(&self.inner);
        let connection = self.inner.borrow().connection.clone();
        connection.subscribe_to_signal(
            Some(BLUEZ),
            interface,
            Some(member),
            None,
            None,
            gio::DBusSignalFlags::NONE,
            move |signal| {
                if let Some(bluez) = upgrade(&weak) {
                    bluez.update(|inner| {
                        apply(&mut inner.objects, signal.object_path, signal.parameters)
                    });
                }
            },
        )
    }

    /// Changes the model, then tells the listeners, with nothing borrowed so that they can
    /// read it or act.
    fn update(&self, change: impl FnOnce(&mut Inner)) {
        let listeners = {
            let mut inner = self.inner.borrow_mut();
            change(&mut inner);
            inner.listeners.clone()
        };
        for listener in listeners {
            listener(self);
        }
    }
}

fn upgrade(weak: &Weak<RefCell<Inner>>) -> Option<Bluez> {
    weak.upgrade().map(|inner| Bluez { inner })
}

/// A device's adapter, from its path, for a device the model doesn't know.
fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(parent, _)| parent)
}

/// `path` as an object path, checked before it reaches gio, which would refuse it with a
/// critical warning and no answer.
fn object_path(path: &str) -> Result<ObjectPath, Error> {
    ObjectPath::try_from(path.to_owned()).map_err(|_| Error::Path(path.to_owned()))
}
