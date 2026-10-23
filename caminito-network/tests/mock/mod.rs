//! A private bus of each test's own, with python-dbusmock's `networkmanager` template on it as
//! NetworkManager, and an AgentManager added to it (`networkmanager.py` beside this file). A
//! test may start python-dbusmock's `notification_daemon` template on it too, as the session's
//! notification server: the bus is both the system bus and the session bus. The test runs on a
//! main context of its own too, so that the tests, each on its thread, don't share GLib's
//! default one.

#![allow(dead_code, reason = "each test binary uses a part of the fixture")]

use std::cell::Cell;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use gio::glib::variant::ToVariant;
use gio::glib::{self, Variant};

/// How long a test waits for what it expects before it fails.
const PATIENCE: Duration = Duration::from_secs(20);
/// How often a test checks again for what another process does.
const POLL: Duration = Duration::from_millis(20);

pub const NETWORK_MANAGER: &str = "org.freedesktop.NetworkManager";
pub const NOTIFICATIONS: &str = "org.freedesktop.Notifications";
const NOTIFICATIONS_PATH: &str = "/org/freedesktop/Notifications";
pub const AGENT_MANAGER: &str = "/org/freedesktop/NetworkManager/AgentManager";

/// The bus and the mock NetworkManager on it, stopped when dropped.
pub struct Mock {
    /// The bus's address, for a process started on it.
    pub address: String,
    /// The test's connection to the bus.
    pub connection: gio::DBusConnection,
    context: glib::MainContext,
    daemon: Child,
    network_manager: Option<Child>,
    notification_daemon: Option<Child>,
}

/// Runs `test` on a main context of its own, with a private bus.
pub fn run(test: impl FnOnce(&mut Mock)) {
    let context = glib::MainContext::new();
    context
        .with_thread_default(|| test(&mut Mock::start(context.clone())))
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
        let connection = connect(&address);
        let mut mock = Mock {
            address,
            connection,
            context,
            daemon,
            network_manager: None,
            notification_daemon: None,
        };
        mock.start_network_manager();
        mock
    }

    /// Starts the mock NetworkManager, which owns its name on the bus as the real one owns it
    /// on the system bus.
    pub fn start_network_manager(&mut self) {
        let template = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/mock/networkmanager.py");
        self.network_manager = Some(
            Command::new("python3")
                .args(["-m", "dbusmock", "--template", template])
                .env("DBUS_SYSTEM_BUS_ADDRESS", &self.address)
                .env("DBUS_SESSION_BUS_ADDRESS", &self.address)
                .stdout(Stdio::null())
                .spawn()
                .expect("python3 runs python-dbusmock"),
        );
        self.wait_for_name(NETWORK_MANAGER);
    }

    /// Starts the mock notification server, which owns its name on the bus as the session's
    /// owns it on the session bus.
    pub fn start_notification_daemon(&mut self) {
        self.notification_daemon = Some(
            Command::new("python3")
                .args(["-m", "dbusmock", "--template", "notification_daemon"])
                .env("DBUS_SYSTEM_BUS_ADDRESS", &self.address)
                .env("DBUS_SESSION_BUS_ADDRESS", &self.address)
                .stdout(Stdio::null())
                .spawn()
                .expect("python3 runs python-dbusmock"),
        );
        self.wait_for_name(NOTIFICATIONS);
    }

    /// Stops the mock NetworkManager, as when the service stops or crashes, and waits until
    /// its name has left the bus.
    pub fn stop_network_manager(&mut self) {
        if let Some(mut network_manager) = self.network_manager.take() {
            let _ = network_manager.kill();
            let _ = network_manager.wait();
        }
        let vanished = Rc::new(Cell::new(false));
        let watcher = gio::bus_watch_name_on_connection(
            &self.connection,
            NETWORK_MANAGER,
            gio::BusNameWatcherFlags::NONE,
            |_, _, _| {},
            {
                let vanished = vanished.clone();
                move |_, _| vanished.set(true)
            },
        );
        self.wait_until("NetworkManager to leave the bus", || vanished.get());
        gio::bus_unwatch_name(watcher);
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

    /// A new connection to the bus, as another process would have.
    pub fn connect(&self) -> gio::DBusConnection {
        connect(&self.address)
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
        // What another process does, as the program registering, wakes nothing here: the
        // condition is checked again every so often too.
        let poll = glib::timeout_source_new(POLL, None, glib::Priority::DEFAULT, || {
            glib::ControlFlow::Continue
        });
        poll.attach(Some(&self.context));
        while !condition() {
            assert!(!late.load(Ordering::Relaxed), "waited too long for {what}");
            self.context.iteration(true);
        }
        poll.destroy();
        timeout.destroy();
    }

    /// Runs the main context for `time`, for a test that nothing happens in it.
    pub fn run_for(&self, time: Duration) {
        let over = Arc::new(AtomicBool::new(false));
        let timeout = glib::timeout_source_new(time, None, glib::Priority::DEFAULT, {
            let over = over.clone();
            move || {
                over.store(true, Ordering::Relaxed);
                glib::ControlFlow::Break
            }
        });
        timeout.attach(Some(&self.context));
        while !over.load(Ordering::Relaxed) {
            self.context.iteration(true);
        }
    }

    /// The arguments of each call the mock's object at `path` has had to `method`, oldest
    /// first.
    pub fn calls(&self, path: &str, method: &str) -> Vec<Variant> {
        self.calls_of(NETWORK_MANAGER, path, method)
    }

    /// The arguments of each `Notify` the mock notification server has had, oldest first.
    pub fn notifies(&self) -> Vec<Variant> {
        self.calls_of(NOTIFICATIONS, NOTIFICATIONS_PATH, "Notify")
    }

    /// The arguments of each `CloseNotification` the mock notification server has had.
    pub fn closes(&self) -> Vec<Variant> {
        self.calls_of(NOTIFICATIONS, NOTIFICATIONS_PATH, "CloseNotification")
    }

    /// Calls `method` of the mock NetworkManager's object at `path`, as the test drives it.
    pub fn call(&self, path: &str, interface: &str, method: &str, parameters: Variant) -> Variant {
        self.connection
            .call_sync(
                Some(NETWORK_MANAGER),
                path,
                interface,
                method,
                Some(&parameters),
                None,
                gio::DBusCallFlags::NONE,
                -1,
                gio::Cancellable::NONE,
            )
            .unwrap_or_else(|err| panic!("the mock's {method} answers: {err}"))
    }

    /// The arguments of each call to `method` that the mocked service `name` has had at
    /// `path`, oldest first.
    fn calls_of(&self, name: &str, path: &str, method: &str) -> Vec<Variant> {
        self.connection
            .call_sync(
                Some(name),
                path,
                "org.freedesktop.DBus.Mock",
                "GetMethodCalls",
                Some(&(method,).to_variant()),
                None,
                gio::DBusCallFlags::NONE,
                -1,
                gio::Cancellable::NONE,
            )
            .unwrap_or_else(|err| panic!("the mock's GetMethodCalls answers: {err}"))
            .child_value(0)
            .iter()
            .map(|call| call.child_value(1))
            .collect()
    }
}

impl Drop for Mock {
    fn drop(&mut self) {
        for child in [&mut self.network_manager, &mut self.notification_daemon]
            .into_iter()
            .flatten()
        {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

fn connect(address: &str) -> gio::DBusConnection {
    gio::DBusConnection::for_address_sync(
        address,
        gio::DBusConnectionFlags::AUTHENTICATION_CLIENT
            | gio::DBusConnectionFlags::MESSAGE_BUS_CONNECTION,
        None,
        gio::Cancellable::NONE,
    )
    .expect("the private bus takes a connection")
}
