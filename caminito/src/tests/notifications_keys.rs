//! Super+N and Do Not Disturb's shortcut (CMN-263): the calls to caminito-notifications's
//! private API, `org.mate.Caminito.Notifications`, which a stand-in on a bus of the test's own
//! serves. The key never reaches the focused client.

use std::sync::{Arc, Mutex};

use insta::assert_snapshot;
use zbus::blocking::Connection;

use super::fixture::{Bus, Fixture, KEY_D, KEY_LEFTMETA, KEY_N, Kind, WindowSpec};
use caminito_test_client::format_input;

use crate::settings::Value;
use crate::settings::translate::CAMINITO;

/// The methods the stand-in was called with, in order.
type Calls = Arc<Mutex<Vec<&'static str>>>;

/// The private API's calls Caminito makes.
struct Interface {
    calls: Calls,
}

#[zbus::interface(name = "org.mate.Caminito.Notifications")]
impl Interface {
    fn open_center(&self) {
        self.calls.lock().unwrap().push("OpenCenter");
    }

    fn toggle_do_not_disturb(&self) {
        self.calls.lock().unwrap().push("ToggleDoNotDisturb");
    }
}

/// A stand-in for caminito-notifications on the test's bus.
struct Daemon {
    calls: Calls,
    _connection: Connection,
}

impl Daemon {
    fn on(bus: &Bus) -> Self {
        let calls = Calls::default();
        let interface = Interface {
            calls: calls.clone(),
        };
        let connection = zbus::blocking::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .name("org.mate.Caminito.Notifications")
            .unwrap()
            .serve_at("/org/mate/Caminito/Notifications", interface)
            .unwrap()
            .build()
            .unwrap();
        Daemon {
            calls,
            _connection: connection,
        }
    }

    fn calls(&self) -> Vec<&'static str> {
        self.calls.lock().unwrap().clone()
    }
}

#[test]
fn super_n_opens_the_notification_center_and_the_shortcut_toggles_do_not_disturb() {
    let dir = tempfile::tempdir().unwrap();
    let bus = Bus::start(&dir.path().join("bus"));
    let mut f = Fixture::with_settings(vec![(
        CAMINITO,
        "toggle-do-not-disturb".into(),
        Value::Str("<Super>d".into()),
    )]);
    f.add_output((1920, 1080), 1.0);
    f.start_dbus_at(&bus.address);
    let daemon = Daemon::on(&bus);
    let id = f.connect(Kind::Xdg);
    f.client(id).bind_input();
    f.map(id, WindowSpec::plain((400, 300)));
    f.roundtrip(id);
    f.client(id).state.input.clear();

    f.chord(&[KEY_LEFTMETA, KEY_N]);
    f.dispatch_until(|_| daemon.calls().len() == 1);
    f.chord(&[KEY_LEFTMETA, KEY_D]);
    f.dispatch_until(|_| daemon.calls().len() == 2);

    assert_eq!(daemon.calls(), ["OpenCenter", "ToggleDoNotDisturb"]);
    f.roundtrip(id);
    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    key 125 pressed
    key 125 released
    key 125 pressed
    key 125 released
    ");
}
