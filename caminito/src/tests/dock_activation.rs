//! `caminito_dock_activation_manager_v1` (CMN-245, ADR 0050): Caminito tells one Dock applet
//! which Dock item to activate: the applet on the pointer's output, else on the focused
//! window's output, else any one. An applet is a layer surface's, so its client is
//! `Kind::Xdg`; mate-panel has every Dock panel in one client, as the tests have.

use caminito_test_client::dock_activation::DockActivationId;
use caminito_test_client::layer_shell::LayerSpec;

use super::fixture::{ClientId, Fixture, Kind, WindowSpec};
use super::per_output_workspaces::put_at;
use crate::state::SEAT;

/// Three 1920 × 1080 outputs side by side, and a panel client that has bound them.
fn three_outputs() -> (Fixture, ClientId) {
    let mut f = Fixture::new();
    for _ in 0..3 {
        f.add_output((1920, 1080), 1.0);
    }
    let panel = f.connect(Kind::Xdg);
    f.roundtrip(panel);
    f.client(panel).bind_outputs();
    f.roundtrip(panel);
    (f, panel)
}

/// A Dock panel along the top of the output at `output`, and its applet's object.
fn dock_on(f: &mut Fixture, panel: ClientId, output: usize) -> DockActivationId {
    let spec = LayerSpec {
        output: Some(output),
        ..LayerSpec::top_bar(48)
    };
    let layer = f.map_layer(panel, spec, 0xff00_00ff);
    let applet = f.client(panel).bind_dock_activation(layer);
    f.roundtrip(panel);
    applet
}

/// Asks Caminito to activate the Dock item at `index`, and returns whether it told an
/// applet, once the panel client has seen what it sent.
fn activate(f: &mut Fixture, panel: ClientId, index: u32) -> bool {
    let told = f.state.caminito.activate_dock_item(index);
    f.roundtrip(panel);
    told
}

/// The x of the middle of the output at `output`.
fn middle_of(output: usize) -> f64 {
    1920.0 * output as f64 + 960.0
}

#[test]
fn the_compositor_offers_the_protocol() {
    let (mut f, panel) = Fixture::connected(Kind::Xdg);
    f.roundtrip(panel);

    assert!(f.client(panel).has_dock_activation_manager());
}

#[test]
fn with_no_applet_nothing_is_told() {
    let (mut f, panel) = three_outputs();
    f.map_layer(panel, LayerSpec::top_bar(48), 0xff00_00ff);

    assert!(!f.state.caminito.has_dock_applet());
    assert!(!activate(&mut f, panel, 0));
}

#[test]
fn an_applet_is_told_the_index() {
    let (mut f, panel) = three_outputs();
    let applet = dock_on(&mut f, panel, 0);

    assert!(f.state.caminito.has_dock_applet());
    assert!(activate(&mut f, panel, 2));
    assert!(activate(&mut f, panel, 0));

    assert_eq!(f.client(panel).dock_activations(applet), [2, 0]);
}

#[test]
fn the_applet_on_the_pointers_output_is_told() {
    let (mut f, panel) = three_outputs();
    let left = dock_on(&mut f, panel, 0);
    let middle = dock_on(&mut f, panel, 1);

    f.pointer_move(middle_of(1), 500.0);
    assert!(activate(&mut f, panel, 3));
    f.pointer_move(middle_of(0), 500.0);
    assert!(activate(&mut f, panel, 4));

    assert_eq!(f.client(panel).dock_activations(middle), [3]);
    assert_eq!(f.client(panel).dock_activations(left), [4]);
}

#[test]
fn with_none_on_the_pointers_output_the_applet_on_the_focused_windows_is_told() {
    let (mut f, panel) = three_outputs();
    let left = dock_on(&mut f, panel, 0);
    let right = dock_on(&mut f, panel, 2);
    let client = f.connect(Kind::Xdg);
    let window = f.map(client, WindowSpec::plain((400, 300)));
    put_at(&mut f, window, (3840 + 200, 300), 0);
    assert_eq!(f.state.caminito.policy.focus(SEAT), Some(f.pid(window)));

    f.pointer_move(middle_of(1), 500.0);
    assert!(activate(&mut f, panel, 1));

    assert_eq!(f.client(panel).dock_activations(right), [1]);
    assert_eq!(f.client(panel).dock_activations(left), [] as [u32; 0]);
}

#[test]
fn with_none_on_the_pointers_output_or_the_focused_windows_one_applet_is_told() {
    let (mut f, panel) = three_outputs();
    let left = dock_on(&mut f, panel, 0);
    let right = dock_on(&mut f, panel, 2);
    assert_eq!(f.state.caminito.policy.focus(SEAT), None);

    f.pointer_move(middle_of(1), 500.0);
    assert!(activate(&mut f, panel, 5));

    let mut told = [left, right].map(|applet| f.client(panel).dock_activations(applet).to_vec());
    told.sort();
    assert_eq!(told, [vec![], vec![5]]);
}

#[test]
fn an_applet_that_destroys_its_object_is_told_nothing() {
    let (mut f, panel) = three_outputs();
    let left = dock_on(&mut f, panel, 0);
    let right = dock_on(&mut f, panel, 1);
    f.client(panel).destroy_dock_activation(right);
    f.roundtrip(panel);

    f.pointer_move(middle_of(1), 500.0);
    assert!(activate(&mut f, panel, 0));
    f.client(panel).destroy_dock_activation(left);
    f.roundtrip(panel);

    assert_eq!(f.client(panel).dock_activations(left), [0]);
    assert!(!f.state.caminito.has_dock_applet());
    assert!(!activate(&mut f, panel, 1));
    assert_eq!(f.client(panel).dock_activations(right), [] as [u32; 0]);
}

#[test]
fn a_panel_client_that_disconnects_leaves_no_applet() {
    let (mut f, panel) = three_outputs();
    dock_on(&mut f, panel, 0);

    f.disconnect(panel);

    assert!(!f.state.caminito.has_dock_applet());
    assert!(!f.state.caminito.activate_dock_item(0));
}
