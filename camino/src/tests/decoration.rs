//! xdg-decoration and KDE server-decoration, which answer client side until M2 (17, 22).

use camino_test_client::{KdeMode, format_configures};
use insta::assert_snapshot;

use super::fixture::{Fixture, WINDOW_COLOR};

#[test]
fn xdg_decoration_answers_client_side_in_the_initial_configure() {
    let (mut f, id) = Fixture::connected();

    let window = f.client(id).create_window();
    f.client(id).request_server_side(window);
    f.client(id).commit(window);
    f.roundtrip(id);

    assert_snapshot!(format_configures(&f.client(id).window(window).configures));
}

#[test]
fn xdg_decoration_answers_client_side_to_a_mapped_window() {
    let (mut f, id) = Fixture::connected();
    let window = f.map_window(id, (200, 100));

    f.client(id).request_server_side(window);
    f.roundtrip(id);

    assert_snapshot!(format_configures(&f.client(id).window(window).configures));
}

#[test]
fn kde_server_decoration_defaults_to_and_answers_client_side() {
    let (mut f, id) = Fixture::connected();
    let window = f.client(id).create_window();
    f.client(id).kde_request_server_side(window);
    f.client(id).commit(window);
    f.roundtrip(id);
    f.client(id)
        .ack_and_commit_buffer(window, (200, 100), WINDOW_COLOR);
    f.roundtrip(id);

    let client = f.client(id);
    assert_eq!(client.state.kde_default_mode, Some(KdeMode::Client));
    assert_eq!(
        client.window(window).kde_modes.last(),
        Some(&KdeMode::Client)
    );
}
