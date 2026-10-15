//! A private bus of each test's own, with python-dbusmock's `bluez5` template on it as BlueZ.
//! The test runs on a main context of its own too, so that the tests, each on its thread,
//! don't share GLib's default one.

#![allow(dead_code, reason = "each test binary uses a part of the fixture")]

use std::cell::Cell;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use gio::glib::variant::{ObjectPath, ToVariant};
use gio::glib::{self, Variant};

/// How long a test waits for what it expects before it fails.
const PATIENCE: Duration = Duration::from_secs(20);

/// The bus and the mock BlueZ on it, stopped when dropped.
pub struct Mock {
    /// The bus's address, for a process started on it.
    pub address: String,
    pub connection: gio::DBusConnection,
    context: glib::MainContext,
    daemon: Child,
    bluez: Option<Child>,
}

/// Runs `test` on a main context of its own, with a private bus.
pub fn run(test: impl FnOnce(&Mock)) {
    let context = glib::MainContext::new();
    context
        .with_thread_default(|| test(&Mock::start(context.clone())))
        .expect("a new context is free to acquire");
}

impl Mock {
    fn start(context: glib::MainContext) -> Mock {
        let mut daemon = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon starts");
        let mut address = String::new();
        BufReader::new(daemon.stdout.take().expect("its output is piped"))
            .read_line(&mut address)
            .expect("dbus-daemon prints its address");
        let address = address.trim_end().to_owned();
        let connection = gio::DBusConnection::for_address_sync(
            &address,
            gio::DBusConnectionFlags::AUTHENTICATION_CLIENT
                | gio::DBusConnectionFlags::MESSAGE_BUS_CONNECTION,
            None,
            gio::Cancellable::NONE,
        )
        .expect("the private bus takes a connection");
        let mut mock = Mock {
            address,
            connection,
            context,
            daemon,
            bluez: None,
        };
        mock.start_bluez();
        mock
    }

    /// Starts the mock BlueZ, which owns `org.bluez` on the bus as the real one owns it on
    /// the system bus.
    fn start_bluez(&mut self) {
        self.bluez = Some(
            Command::new("python3")
                .args(["-m", "dbusmock", "--template", "bluez5"])
                .env("DBUS_SYSTEM_BUS_ADDRESS", &self.address)
                .env("DBUS_SESSION_BUS_ADDRESS", &self.address)
                .stdout(Stdio::null())
                .spawn()
                .expect("python3 runs python-dbusmock"),
        );
        self.wait_for_name("org.bluez");
    }

    /// Waits until something owns `name` on the bus.
    pub fn wait_for_name(&self, name: &str) {
        let appeared = Rc::new(Cell::new(false));
        let watcher = gio::bus_watch_name_on_connection(
            &self.connection,
            name,
            gio::BusNameWatcherFlags::NONE,
            {
                let appeared = appeared.clone();
                move |_, _, _| appeared.set(true)
            },
            |_, _| {},
        );
        self.wait_until(&format!("{name} on the bus"), || appeared.get());
        gio::bus_unwatch_name(watcher);
    }

    /// Runs the main context until `condition` holds, and fails the test if it doesn't in
    /// time.
    pub fn wait_until(&self, what: &str, condition: impl Fn() -> bool) {
        // The source must be Send, though it runs on this thread.
        let late = Arc::new(AtomicBool::new(false));
        let timeout = glib::timeout_source_new(PATIENCE, None, glib::Priority::DEFAULT, {
            let late = late.clone();
            move || {
                late.store(true, Ordering::Relaxed);
                glib::ControlFlow::Break
            }
        });
        timeout.attach(Some(&self.context));
        while !condition() {
            assert!(!late.load(Ordering::Relaxed), "waited too long for {what}");
            self.context.iteration(true);
        }
        timeout.destroy();
    }

    /// Runs `future` on the test's main context to its end.
    pub fn block_on<F: std::future::Future>(&self, future: F) -> F::Output {
        self.context.block_on(future)
    }

    /// Adds an adapter, such as `hci0`, and returns its path.
    pub fn add_adapter(&self, name: &str, alias: &str) -> String {
        let path = self
            .mock(
                "/",
                "org.bluez.Mock",
                "AddAdapter",
                &(name, alias).to_variant(),
            )
            .child_value(0)
            .str()
            .expect("AddAdapter returns the path")
            .to_owned();
        // The template's StartDiscovery reads a filter that only SetDiscoveryFilter stores,
        // and fails without one (python-dbusmock 0.31). BlueZ's needs none.
        let filter: HashMap<String, Variant> = HashMap::new();
        self.mock(
            &path,
            "org.bluez.Adapter1",
            "SetDiscoveryFilter",
            &(filter,).to_variant(),
        );
        path
    }

    /// Adds an unpaired device to the adapter `adapter` names, and returns its path.
    pub fn add_device(&self, adapter: &str, address: &str, alias: &str) -> String {
        self.mock(
            "/",
            "org.bluez.Mock",
            "AddDevice",
            &(adapter, address, alias).to_variant(),
        )
        .child_value(0)
        .str()
        .expect("AddDevice returns the path")
        .to_owned()
    }

    /// Marks a device as paired.
    pub fn pair(&self, adapter: &str, address: &str) {
        self.mock(
            "/",
            "org.bluez.Mock",
            "PairDevice",
            &(adapter, address).to_variant(),
        );
    }

    /// Gives the device at `device` a battery, as BlueZ adds `Battery1` once it learns of
    /// one. The template has none, so the interface and its signal are the fixture's.
    pub fn add_battery(&self, device: &str, percentage: u8) {
        let properties = HashMap::from([("Percentage".to_owned(), percentage.to_variant())]);
        self.mock(
            device,
            "org.freedesktop.DBus.Mock",
            "AddProperties",
            &("org.bluez.Battery1", properties.clone()).to_variant(),
        );
        let interfaces = HashMap::from([("org.bluez.Battery1".to_owned(), properties)]);
        let arguments = vec![
            Variant::from_variant(&object_path(device).to_variant()),
            Variant::from_variant(&interfaces.to_variant()),
        ];
        self.mock(
            "/",
            "org.freedesktop.DBus.Mock",
            "EmitSignal",
            &(
                "org.freedesktop.DBus.ObjectManager",
                "InterfacesAdded",
                "oa{sa{sv}}",
                arguments,
            )
                .to_variant(),
        );
    }

    /// Changes the battery level of the device at `device`.
    pub fn set_battery(&self, device: &str, percentage: u8) {
        let properties = HashMap::from([("Percentage".to_owned(), percentage.to_variant())]);
        self.mock(
            device,
            "org.freedesktop.DBus.Mock",
            "UpdateProperties",
            &("org.bluez.Battery1", properties).to_variant(),
        );
    }

    /// Forgets a device behind the program's back, as `bluetoothctl remove` would.
    pub fn remove_device(&self, adapter: &str, device: &str) {
        self.mock(
            adapter,
            "org.bluez.Adapter1",
            "RemoveDevice",
            &(object_path(device),).to_variant(),
        );
    }

    /// A property, as the mock holds it.
    pub fn property(&self, path: &str, interface: &str, name: &str) -> Variant {
        self.mock(
            path,
            "org.freedesktop.DBus.Properties",
            "Get",
            &(interface, name).to_variant(),
        )
        .child_value(0)
        .as_variant()
        .expect("Get returns a variant")
    }

    fn mock(&self, path: &str, interface: &str, method: &str, parameters: &Variant) -> Variant {
        self.connection
            .call_sync(
                Some("org.bluez"),
                path,
                interface,
                method,
                Some(parameters),
                None,
                gio::DBusCallFlags::NONE,
                -1,
                gio::Cancellable::NONE,
            )
            .unwrap_or_else(|err| panic!("the mock's {method} answers: {err}"))
    }
}

impl Drop for Mock {
    fn drop(&mut self) {
        if let Some(bluez) = &mut self.bluez {
            let _ = bluez.kill();
            let _ = bluez.wait();
        }
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

fn object_path(path: &str) -> ObjectPath {
    ObjectPath::try_from(path.to_owned()).expect("the mock's paths are object paths")
}
