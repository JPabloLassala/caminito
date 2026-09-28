//! The debug interface (14, 32): `org.mate.Camino.Debug` on the test binary's private bus,
//! and frame debug's outlines.

use camino_test_client::dbus::debug_proxy;
use camino_test_client::{DecorationMode, WindowId};

use super::fixture::{ClientId, Fixture, WINDOW_COLOR};

/// Maps a window with `title` and `app_id` that asks for `mode`.
fn map_titled(
    f: &mut Fixture,
    id: ClientId,
    mode: DecorationMode,
    title: &str,
    app_id: &str,
) -> WindowId {
    let window = f.client(id).create_window();
    f.client(id).set_title(window, title);
    f.client(id).set_app_id(window, app_id);
    f.client(id).request_decoration(window, Some(mode));
    f.client(id).commit(window);
    f.roundtrip(id);
    f.client(id)
        .ack_and_commit_buffer(window, (400, 300), WINDOW_COLOR);
    f.roundtrip(id);
    f.dispatch();
    window
}

#[test]
fn the_state_dump_shows_an_ssd_and_a_csd_window_on_two_workspaces() {
    let (mut f, id) = Fixture::connected();
    f.add_output((2560, 1440), 2.0);
    let ssd = map_titled(&mut f, id, DecorationMode::ServerSide, "Caja", "caja");
    let csd = map_titled(
        &mut f,
        id,
        DecorationMode::ClientSide,
        "Files",
        "org.gnome.Nautilus",
    );
    let csd = f.policy_id(id, csd).unwrap();
    f.state.camino.policy.move_to_workspace(csd, 1);
    let ssd = f.policy_id(id, ssd).unwrap();
    f.state
        .camino
        .policy
        .activate(crate::state::SEAT, ssd, f.state.camino.now());
    f.dispatch();

    let bus = f.start_dbus();
    let dump = f.dbus_call(move || debug_proxy(&bus).unwrap().dump_state().unwrap());

    insta::assert_snapshot!(dump);
}

#[test]
fn frame_debug_adds_the_outlines_and_turning_it_off_removes_them() {
    let (mut f, id) = Fixture::connected();
    map_titled(&mut f, id, DecorationMode::ServerSide, "Caja", "caja");
    let output = f.state.camino.outputs[0].clone();
    let before = f.state.scene_element_count(&output);
    let bus = f.start_dbus();

    let toggle = |f: &mut Fixture| {
        let bus = bus.clone();
        f.dbus_call(move || debug_proxy(&bus).unwrap().toggle_frame_debug().unwrap())
    };
    assert!(toggle(&mut f));
    f.dispatch();
    // Four sides of an outline for each of the four strips, the resize border and the
    // title, and the scale's label.
    assert_eq!(f.state.scene_element_count(&output), before + 6 * 4 + 1);

    assert!(!toggle(&mut f));
    f.dispatch();
    assert_eq!(f.state.scene_element_count(&output), before);
}

#[test]
fn frame_debug_outlines_nothing_for_a_csd_window() {
    let (mut f, id) = Fixture::connected();
    map_titled(
        &mut f,
        id,
        DecorationMode::ClientSide,
        "Files",
        "org.gnome.Nautilus",
    );
    let output = f.state.camino.outputs[0].clone();
    let before = f.state.scene_element_count(&output);

    f.state.camino.toggle_frame_debug();
    f.dispatch();

    assert_eq!(f.state.scene_element_count(&output), before);
}

#[test]
fn camino_owns_its_debug_name() {
    let mut f = Fixture::new();
    f.state
        .camino
        .start_dbus(crate::dbus::Names::WellKnown)
        .unwrap();

    let dump = f.dbus_call(|| {
        debug_proxy("org.mate.Camino.Debug")
            .unwrap()
            .dump_state()
            .unwrap()
    });

    assert!(dump.starts_with('{'));
}
