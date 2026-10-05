//! The privileged globals, which a client connected through a security context isn't
//! offered (06, 68). The clients are Wayland ones only: Xwayland is Camino's own child, and
//! nothing connects it through a context.

use camino_test_client::format_globals;
use insta::assert_snapshot;

use super::fixture::{ClientId, Fixture, Kind};

/// The globals that show or change what belongs to other clients, and security-context
/// itself.
const PRIVILEGED: [&str; 9] = [
    "zwlr_layer_shell_v1",
    "zwlr_foreign_toplevel_manager_v1",
    "xfce_foreign_toplevel_manager_private_v1",
    "ext_foreign_toplevel_list_v1",
    "ext_workspace_manager_v1",
    "zwlr_output_manager_v1",
    "ext_data_control_manager_v1",
    "zwlr_data_control_manager_v1",
    "wp_security_context_manager_v1",
];

fn sees(f: &Fixture, client: ClientId, interface: &str) -> bool {
    let globals = &f.client_state(client).globals;
    globals.iter().any(|global| global.interface == interface)
}

#[test]
fn a_sandboxed_client_is_offered_no_privileged_global() {
    let (mut f, ordinary) = Fixture::connected(Kind::Xdg);
    let sandboxed = f.connect_sandboxed(ordinary);

    for interface in PRIVILEGED {
        assert!(
            sees(&f, ordinary, interface),
            "an ordinary client sees {interface}"
        );
        assert!(
            !sees(&f, sandboxed, interface),
            "a sandboxed client sees {interface}"
        );
    }
    for interface in ["xdg_wm_base", "wl_data_device_manager"] {
        assert!(
            sees(&f, sandboxed, interface),
            "a sandboxed client sees {interface}"
        );
    }
}

#[test]
fn a_sandboxed_client_lists_the_globals() {
    let (mut f, ordinary) = Fixture::connected(Kind::Xdg);
    let sandboxed = f.connect_sandboxed(ordinary);

    assert_snapshot!(format_globals(&f.client_state(sandboxed).globals));
}

#[test]
fn a_sandboxed_client_maps_a_window() {
    let (mut f, ordinary) = Fixture::connected(Kind::Xdg);
    let sandboxed = f.connect_sandboxed(ordinary);

    let window = f.map(sandboxed, super::fixture::WindowSpec::default());

    assert!(f.has_keyboard(window));
}
