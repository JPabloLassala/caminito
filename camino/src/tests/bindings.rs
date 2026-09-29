//! The keyboard path's bindings stage: Marco's default bindings, hard-coded until M8 (22).
//! They check the configures and keys a Wayland client sees, so they map xdg toplevels
//! only (45).

use camino_test_client::{ToplevelState, format_input};
use insta::assert_snapshot;

use super::fixture::{
    ClientWindow, Fixture, KEY_F4, KEY_F10, KEY_LEFT, KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT,
    KEY_TAB, Kind, WindowSpec,
};
use crate::state::SEAT;

fn is_activated(f: &mut Fixture, window: ClientWindow) -> bool {
    let configure = f
        .client(window.client())
        .window(window.xdg())
        .configures
        .last()
        .unwrap()
        .clone();
    configure.states.contains(&ToplevelState::Activated)
}

#[test]
fn switching_workspaces_unmaps_and_remaps_their_windows() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    f.client(id).bind_outputs();
    f.roundtrip(id);
    let window = f.map(id, WindowSpec::plain((400, 300)));
    let output = f.state.camino.outputs[0].clone();
    f.client(id).state.input.clear();

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    f.roundtrip(id);
    assert_eq!(f.state.camino.policy.active_workspace(), 1);
    assert!(!is_activated(&mut f, window));
    // Only the cursor.
    assert_eq!(f.state.scene_element_count(&output), 1);

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFT]);
    f.roundtrip(id);
    assert_eq!(f.state.camino.policy.active_workspace(), 0);
    assert!(is_activated(&mut f, window));

    assert_snapshot!(f.client(id).window(window.xdg()).output_events.join("\n"), @r"
    enter headless-1
    leave headless-1
    enter headless-1
    ");
    // The bound keys never reach the client. The modifiers do while it has the keyboard:
    // it loses it mid-chord on the way out, and gets it back mid-chord on the way in.
    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    key 29 pressed
    key 56 pressed
    keyboard leaves window 0
    keyboard enters window 0
    key 56 released
    key 29 released
    ");
}

#[test]
fn alt_f4_asks_the_focused_window_to_close() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));

    f.chord(&[KEY_LEFTALT, KEY_F4]);
    f.roundtrip(id);

    assert!(f.client(id).window(window.xdg()).close_requested);
}

#[test]
fn alt_f10_toggles_maximised() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    let policy_id = f.pid(window);

    f.chord(&[KEY_LEFTALT, KEY_F10]);
    f.roundtrip(id);
    assert!(f.state.camino.policy.is_maximized(policy_id));

    f.chord(&[KEY_LEFTALT, KEY_F10]);
    f.roundtrip(id);
    assert!(!f.state.camino.policy.is_maximized(policy_id));
}

#[test]
fn alt_tab_focuses_the_previously_focused_window() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    let a = f.map(id, WindowSpec::plain((400, 300)));
    let b = f.map(id, WindowSpec::plain((400, 300)));
    let (pa, pb) = (f.policy_id(a), f.policy_id(b));
    assert_eq!(f.state.camino.policy.focus(SEAT), pb);

    f.chord(&[KEY_LEFTALT, KEY_TAB]);
    f.roundtrip(id);
    assert_eq!(f.state.camino.policy.focus(SEAT), pa);
    assert!(is_activated(&mut f, a));

    f.chord(&[KEY_LEFTALT, KEY_TAB]);
    f.roundtrip(id);
    assert_eq!(f.state.camino.policy.focus(SEAT), pb);
}

#[test]
fn an_inhibitor_on_the_focused_surface_passes_bindings_to_the_client() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    f.client(id).inhibit_shortcuts(window.xdg());
    f.roundtrip(id);
    f.client(id).state.input.clear();

    f.chord(&[KEY_LEFTALT, KEY_F4]);
    f.roundtrip(id);

    assert!(!f.client(id).window(window.xdg()).close_requested);
    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    key 56 pressed
    key 62 pressed
    key 62 released
    key 56 released
    ");
}
