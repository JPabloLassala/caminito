//! `ActivateApplication` on `org.mate.Caminito` (ADR 0054): caminito-notifications's click on a
//! notification without a default action, which brings the app forward as a click on its Task
//! item does: its most recently focused window, minimised or not.

use caminito_test_client::dbus::caminito_proxy;

use super::fixture::{ClientWindow, Fixture, Kind, WindowSpec};
use crate::state::SEAT;

/// Maps a window of the app `app_id`.
fn map_of(f: &mut Fixture, app_id: &'static str, title: &'static str) -> ClientWindow {
    let client = f.connect(Kind::Xdg);
    f.map(
        client,
        WindowSpec {
            title: Some(title),
            app_id: Some(app_id),
            ..WindowSpec::plain((400, 300))
        },
    )
}

fn activate_application(f: &mut Fixture, bus: &str, app: &'static str) {
    let bus = bus.to_owned();
    f.dbus_call(move || {
        caminito_proxy(&bus)
            .unwrap()
            .activate_application(app)
            .unwrap()
    });
    f.dispatch();
}

fn has_focus(f: &Fixture, window: ClientWindow) -> bool {
    f.state.caminito.policy.focus(SEAT) == Some(f.pid(window))
}

#[test]
fn it_focuses_the_apps_most_recently_focused_window() {
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.);
    let first = map_of(&mut f, "org.gnome.Evolution", "Inbox");
    let second = map_of(&mut f, "org.gnome.Evolution", "Compose");
    let other = map_of(&mut f, "pluma", "Notes");
    assert!(has_focus(&f, other));
    let bus = f.start_dbus();

    activate_application(&mut f, &bus, "org.gnome.Evolution");

    assert!(has_focus(&f, second));
    assert!(f.has_keyboard(second));
    assert!(!has_focus(&f, first));
}

#[test]
fn it_brings_back_a_minimised_window_and_matches_the_app_id_in_any_case() {
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.);
    let mail = map_of(&mut f, "org.gnome.Evolution", "Inbox");
    let other = map_of(&mut f, "pluma", "Notes");
    f.state.caminito.policy.minimize(f.pid(mail));
    f.dispatch();
    assert!(has_focus(&f, other));
    let bus = f.start_dbus();

    activate_application(&mut f, &bus, "Org.Gnome.Evolution");

    assert!(has_focus(&f, mail));
    assert!(!f.state.caminito.policy.is_minimized(f.pid(mail)));
}

#[test]
fn an_app_without_a_window_changes_nothing() {
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.);
    let other = map_of(&mut f, "pluma", "Notes");
    let bus = f.start_dbus();

    activate_application(&mut f, &bus, "org.gnome.Evolution");

    assert!(has_focus(&f, other));
}
