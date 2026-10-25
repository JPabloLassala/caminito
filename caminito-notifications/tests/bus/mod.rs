//! A private bus of each test's own, and a main context of each test's own too, so that the
//! tests, each on its thread, don't share GLib's default one. The bus is the session bus the
//! server owns the notification name on.

#![allow(dead_code, reason = "each test binary uses a part of the fixture")]

use std::cell::Cell;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use gio::glib;

/// How long a test waits for what it expects before it fails.
const PATIENCE: Duration = Duration::from_secs(20);
/// How often a test checks again for what another process does.
const POLL: Duration = Duration::from_millis(20);

/// The bus, stopped when dropped.
pub struct Bus {
    /// The bus's address.
    pub address: String,
    context: glib::MainContext,
    daemon: Child,
}

/// Runs `test` on a main context of its own, with a private bus.
pub fn run(test: impl FnOnce(&Bus)) {
    let context = glib::MainContext::new();
    context
        .with_thread_default(|| test(&Bus::start(context.clone())))
        .expect("a new context is free to acquire");
}

impl Bus {
    fn start(context: glib::MainContext) -> Bus {
        let mut daemon = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon starts");
        let mut address = String::new();
        BufReader::new(daemon.stdout.take().expect("its output is piped"))
            .read_line(&mut address)
            .expect("dbus-daemon prints its address");
        Bus {
            address: address.trim_end().to_owned(),
            context,
            daemon,
        }
    }

    /// A new connection to the bus, as another process would have.
    pub fn connect(&self) -> gio::DBusConnection {
        gio::DBusConnection::for_address_sync(
            &self.address,
            gio::DBusConnectionFlags::AUTHENTICATION_CLIENT
                | gio::DBusConnectionFlags::MESSAGE_BUS_CONNECTION,
            None,
            gio::Cancellable::NONE,
        )
        .expect("the private bus takes a connection")
    }

    /// Waits until `connection` owns `name` on the bus.
    pub fn wait_for_owner(&self, name: &str, connection: &gio::DBusConnection) {
        let owner = connection
            .unique_name()
            .expect("a bus connection has a unique name")
            .to_string();
        let current = Rc::new(Cell::new(false));
        let watcher = gio::bus_watch_name_on_connection(
            &self.connect(),
            name,
            gio::BusNameWatcherFlags::NONE,
            {
                let current = current.clone();
                move |_, _, now| current.set(now == owner)
            },
            {
                let current = current.clone();
                move |_, _| current.set(false)
            },
        );
        self.wait_until(&format!("{name} to be owned"), || current.get());
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
        // What arrives over the bus may wake nothing here: the condition is checked again
        // every so often too.
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
}

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}
