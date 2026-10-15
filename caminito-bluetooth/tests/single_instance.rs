//! `caminito-bluetooth` is one process through the session (255): a second start passes its
//! command line to the first, which opens the window, and exits.
//!
//! GTK needs a display to start, so the test is ignored unless asked for, and tools/check
//! runs it under Xvfb. Each test's bus is its own, as both the session and the system bus.

mod mock;

use std::process::{Child, Command};

use gio::glib::variant::ToVariant;

const APPLICATION: &str = "org.mate.Caminito.Bluetooth";

/// The program, started on the test's bus.
fn start(mock: &mock::Mock, arguments: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_caminito-bluetooth"));
    command
        .args(arguments)
        .env("DBUS_SESSION_BUS_ADDRESS", &mock.address)
        .env("DBUS_SYSTEM_BUS_ADDRESS", &mock.address)
        // The test's bus has no accessibility bus to find.
        .env("GTK_A11Y", "none");
    command
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

/// The windows the application exports, as GtkApplication numbers them.
fn windows(mock: &mock::Mock) -> String {
    mock.connection
        .call_sync(
            Some(APPLICATION),
            "/org/mate/Caminito/Bluetooth/window",
            "org.freedesktop.DBus.Introspectable",
            "Introspect",
            None,
            None,
            gio::DBusCallFlags::NONE,
            -1,
            gio::Cancellable::NONE,
        )
        .expect("the application answers")
        .child_value(0)
        .str()
        .expect("introspection is a string")
        .to_owned()
}

struct Killed(Child);

impl Drop for Killed {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
#[ignore = "GTK needs a display: tools/check runs it under Xvfb"]
fn a_second_start_reaches_the_first_which_opens_the_window() {
    mock::run(|mock| {
        let mut first = Killed(
            start(mock, &["--background"])
                .spawn()
                .expect("caminito-bluetooth starts"),
        );
        mock.wait_for_name(APPLICATION);
        assert_eq!(owner_pid(mock), first.0.id());
        assert!(!windows(mock).contains("<node name=\"1\""));

        // The second start waits for the first to handle its command line, so the window
        // is there once it has exited.
        let second = start(mock, &[])
            .status()
            .expect("caminito-bluetooth starts");
        assert!(second.success());
        assert!(windows(mock).contains("<node name=\"1\""));

        // And a second --background doesn't stay either.
        let third = start(mock, &["--background"])
            .status()
            .expect("caminito-bluetooth starts");
        assert!(third.success());
        assert_eq!(owner_pid(mock), first.0.id());
        assert!(
            first.0.try_wait().unwrap().is_none(),
            "the first is still running"
        );
    });
}
