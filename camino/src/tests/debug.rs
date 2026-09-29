//! The debug interface (14, 32): `org.mate.Camino.Debug` on the test binary's private bus,
//! and frame debug's outlines, with an xdg toplevel and an XWayland window (45).

use camino_test_client::dbus::debug_proxy;

use super::fixture::{ClientId, ClientWindow, Decoration, Fixture, WindowSpec, on_both_kinds};

/// Maps a window with `title` and `app_id` that asks for `decoration`.
fn titled(
    f: &mut Fixture,
    client: ClientId,
    decoration: Decoration,
    title: &'static str,
    app_id: &'static str,
) -> ClientWindow {
    f.map(
        client,
        WindowSpec {
            decoration,
            title: Some(title),
            app_id: Some(app_id),
            ..WindowSpec::default()
        },
    )
}

/// The dump with its X window ids, which differ from run to run, as `[X id]`.
fn without_x_ids(dump: &str) -> String {
    let lines = dump.lines().map(|line| {
        let key = "\"window\": ";
        match line.find(key) {
            Some(at) => format!("{}{key}\"[X id]\",", &line[..at]),
            None => line.to_owned(),
        }
    });
    lines.collect::<Vec<_>>().join("\n")
}

on_both_kinds! {
    fn the_state_dump_shows_an_ssd_and_a_csd_window_on_two_workspaces(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        f.add_output((2560, 1440), 2.0);
        // The refresh pass applies the X11 scale of 2 before the X windows map, rather
        // than while they do.
        f.dispatch();
        let ssd = titled(&mut f, client, Decoration::Server, "Caja", "caja");
        let csd = titled(
            &mut f,
            client,
            Decoration::Client,
            "Files",
            "org.gnome.Nautilus",
        );
        let csd = f.pid(csd);
        f.state.camino.policy.move_to_workspace(csd, 1);
        let ssd = f.pid(ssd);
        f.state
            .camino
            .policy
            .activate(crate::state::SEAT, ssd, f.state.camino.now());
        f.dispatch();

        let bus = f.start_dbus();
        let dump = f.dbus_call(move || debug_proxy(&bus).unwrap().dump_state().unwrap());

        insta::assert_snapshot!(format!("{kind:?}").to_lowercase(), without_x_ids(&dump));
    }
}

on_both_kinds! {
    fn frame_debug_adds_the_outlines_and_turning_it_off_removes_them(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        titled(&mut f, client, Decoration::Server, "Caja", "caja");
        let output = f.state.camino.outputs[0].clone();
        let before = f.state.scene_element_count(&output);
        let bus = f.start_dbus();

        let toggle = |f: &mut Fixture| {
            let bus = bus.clone();
            f.dbus_call(move || debug_proxy(&bus).unwrap().toggle_frame_debug().unwrap())
        };
        assert!(toggle(&mut f));
        // Four sides of an outline for each of the four strips, the resize border and the
        // title, and the scale's label.
        assert_eq!(f.state.scene_element_count(&output), before + 6 * 4 + 1);

        assert!(!toggle(&mut f));
        assert_eq!(f.state.scene_element_count(&output), before);
    }
}

on_both_kinds! {
    fn frame_debug_outlines_nothing_for_a_csd_window(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        titled(
            &mut f,
            client,
            Decoration::Client,
            "Files",
            "org.gnome.Nautilus",
        );
        let output = f.state.camino.outputs[0].clone();
        let before = f.state.scene_element_count(&output);

        f.state.camino.toggle_frame_debug();
        f.dispatch();

        assert_eq!(f.state.scene_element_count(&output), before);
    }
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
