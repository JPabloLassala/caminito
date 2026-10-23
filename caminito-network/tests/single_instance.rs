//! `caminito-network` is one process through a Session (ADR 0041): a second start finds the
//! first on the session bus and exits. Outside Caminito it exits at once, so that nm-applet
//! stays the secret agent under X11 MATE.
//!
//! GTK needs a display to start, so the test of a second start is ignored unless asked for,
//! and tools/check runs it under Xvfb. Each test's bus is its own, as both the session and
//! the system bus.

mod mock;

use std::process::{Child, Command};
use std::time::{Duration, Instant};

use gio::glib::variant::ToVariant;

const APPLICATION: &str = "org.mate.Caminito.Network";

/// The program, started on the test's bus, under the window manager `wm`.
fn start(mock: &mock::Mock, wm: Option<&str>) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_caminito-network"));
    command
        .env("DBUS_SESSION_BUS_ADDRESS", &mock.address)
        .env("DBUS_SYSTEM_BUS_ADDRESS", &mock.address)
        .env_remove("MATE_WM_NAME")
        // The test's bus has no accessibility bus to find, and GTK would start the desktop's
        // portal on it.
        .env("GTK_A11Y", "none")
        .env("GDK_DEBUG", "no-portals");
    if let Some(wm) = wm {
        command.env("MATE_WM_NAME", wm);
    }
    command
}

/// Whether something owns `name` on the test's bus.
fn owned(mock: &mock::Mock, name: &str) -> bool {
    mock.connection
        .call_sync(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "NameHasOwner",
            Some(&(name,).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            -1,
            gio::Cancellable::NONE,
        )
        .expect("the bus answers")
        .child_value(0)
        .get::<bool>()
        .expect("NameHasOwner answers a boolean")
}

/// The process that owns the application's name.
fn owner_pid(mock: &mock::Mock) -> u32 {
    let owner = mock
        .connection
        .call_sync(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "GetNameOwner",
            Some(&(APPLICATION,).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            -1,
            gio::Cancellable::NONE,
        )
        .expect("the application owns its name")
        .child_value(0);
    mock.connection
        .call_sync(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "GetConnectionUnixProcessID",
            Some(&(owner.str().expect("a name is a string"),).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            -1,
            gio::Cancellable::NONE,
        )
        .expect("the bus knows its process")
        .child_value(0)
        .get::<u32>()
        .expect("a process id is a u32")
}

/// Waits for `child` to exit, and fails the test if it hasn't in a few seconds.
fn exits(mut child: Child) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().expect("the child can be waited for") {
            return status.success();
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("caminito-network is still running");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

struct Killed(Child);

impl Drop for Killed {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn it_exits_at_once_outside_caminito() {
    mock::run(|mock| {
        for wm in [None, Some("Marco")] {
            let child = start(mock, wm).spawn().expect("caminito-network starts");
            assert!(exits(child), "it exits cleanly under {wm:?}");
            assert!(!owned(mock, APPLICATION));
            assert!(
                mock.calls(mock::AGENT_MANAGER, "RegisterWithCapabilities")
                    .is_empty()
            );
        }
    });
}

#[test]
#[ignore = "GTK needs a display: tools/check runs it under Xvfb"]
fn a_second_start_exits_and_the_first_stays_the_agent() {
    mock::run(|mock| {
        let mut first = Killed(
            start(mock, Some("Caminito"))
                .spawn()
                .expect("caminito-network starts"),
        );
        mock.wait_for_name(APPLICATION);
        mock.wait_until("the agent to register", || {
            !mock
                .calls(mock::AGENT_MANAGER, "RegisterWithCapabilities")
                .is_empty()
        });

        let second = start(mock, Some("Caminito"))
            .spawn()
            .expect("caminito-network starts");
        assert!(exits(second));
        assert_eq!(owner_pid(mock), first.0.id());
        assert!(
            first.0.try_wait().unwrap().is_none(),
            "the first is still running"
        );
        assert_eq!(
            mock.calls(mock::AGENT_MANAGER, "RegisterWithCapabilities")
                .len(),
            1
        );
    });
}
