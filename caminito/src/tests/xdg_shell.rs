//! xdg-shell: toplevels mapping into policy, their requests, and popups (22). The protocol
//! is what's under test, so windows are xdg toplevels, and some are mapped by hand (45).

use caminito_policy::PolicyWindow;
use caminito_test_client::format_configures;
use insta::assert_snapshot;

use super::fixture::{ClientWindow, Fixture, Kind, WINDOW_COLOR, WindowSpec};

#[test]
fn a_toplevel_maps_into_policy_when_it_commits_a_buffer() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);

    let window = f.client(id).create_window();
    f.client(id).commit(window);
    f.roundtrip(id);
    assert_eq!(f.policy_id(ClientWindow::Xdg(id, window)), None);

    f.client(id)
        .ack_and_commit_buffer(window, (200, 100), WINDOW_COLOR);
    f.roundtrip(id);

    let geometry = f.geometry(ClientWindow::Xdg(id, window));
    assert_eq!(geometry.size, (200, 100).into());
    assert_snapshot!(format_configures(&f.client(id).window(window).configures));
}

#[test]
fn a_mapped_toplevel_is_drawn_where_policy_placed_it() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let output = f.state.caminito.outputs[0].clone();

    let window = f.map(id, WindowSpec::plain((200, 100)));

    let geometry = f.geometry(window);
    let pixels = f.state.backend.headless_mut().pixels(&output).unwrap();
    let at = |x: i32, y: i32| pixels[(y * 1920 + x) as usize];
    assert_eq!(at(geometry.loc.x, geometry.loc.y), WINDOW_COLOR);
    assert_eq!(at(geometry.loc.x + 199, geometry.loc.y + 99), WINDOW_COLOR);
}

#[test]
fn a_drawn_toplevel_gets_its_frame_callbacks() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let window = f.map(id, WindowSpec::plain((200, 100))).xdg();

    f.client(id).request_frame(window);
    f.client(id).commit(window);
    f.roundtrip(id);

    assert_eq!(f.client(id).window(window).frames_done, 1);
}

#[test]
fn committing_a_null_buffer_unmaps_and_a_new_buffer_maps_again() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let window = f.map(id, WindowSpec::plain((200, 100))).xdg();

    f.client(id).commit_null_buffer(window);
    f.roundtrip(id);
    assert_eq!(f.policy_id(ClientWindow::Xdg(id, window)), None);

    // After an unmap the client starts over with an initial commit, which gets a new
    // initial configure.
    let configures = f.client(id).window(window).configures.len();
    f.client(id).commit(window);
    f.roundtrip(id);
    assert_eq!(f.client(id).window(window).configures.len(), configures + 1);
    f.client(id)
        .ack_and_commit_buffer(window, (200, 100), WINDOW_COLOR);
    f.roundtrip(id);
    assert!(f.policy_id(ClientWindow::Xdg(id, window)).is_some());
}

#[test]
fn destroying_a_toplevel_unmaps_it() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let window = f.map(id, WindowSpec::plain((200, 100))).xdg();
    let policy_id = f.policy_id(ClientWindow::Xdg(id, window)).unwrap();

    f.client(id).destroy_window(window);
    f.roundtrip(id);

    assert!(
        f.state
            .caminito
            .policy
            .windows()
            .all(|mapped| mapped.id() != policy_id)
    );
}

#[test]
fn maximising_configures_the_work_area_and_unmaximising_restores() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let window = f.map(id, WindowSpec::plain((200, 100))).xdg();

    f.client(id).set_maximized(window);
    f.roundtrip(id);
    f.client(id)
        .ack_and_commit_buffer(window, (1920, 1080), WINDOW_COLOR);
    f.client(id).unset_maximized(window);
    f.roundtrip(id);

    assert_snapshot!(format_configures(&f.client(id).window(window).configures));
}

#[test]
fn fullscreen_configures_the_output_it_names() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.add_output((1920, 1080), 1.5);
    f.roundtrip(id);
    f.client(id).bind_outputs();
    let window = f.map(id, WindowSpec::plain((200, 100))).xdg();

    f.client(id).set_fullscreen(window, Some(1));
    f.roundtrip(id);
    f.client(id)
        .ack_and_commit_buffer(window, (1280, 720), WINDOW_COLOR);
    f.client(id).unset_fullscreen(window);
    f.roundtrip(id);

    assert_snapshot!(format_configures(&f.client(id).window(window).configures));
}

#[test]
fn maximising_before_mapping_starts_maximised() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);

    let window = f.client(id).create_window();
    f.client(id).set_maximized(window);
    f.client(id).commit(window);
    f.roundtrip(id);
    f.client(id)
        .ack_and_commit_buffer(window, (1920, 1080), WINDOW_COLOR);
    f.roundtrip(id);

    let policy_id = f.policy_id(ClientWindow::Xdg(id, window)).unwrap();
    assert!(f.state.caminito.policy.is_maximized(policy_id));
    assert_snapshot!(format_configures(&f.client(id).window(window).configures));
}

#[test]
fn fullscreen_before_mapping_starts_fullscreen() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);

    let window = f.client(id).create_window();
    f.client(id).set_fullscreen(window, None);
    f.client(id).commit(window);
    f.roundtrip(id);
    f.client(id)
        .ack_and_commit_buffer(window, (1920, 1080), WINDOW_COLOR);
    f.roundtrip(id);

    let policy_id = f.policy_id(ClientWindow::Xdg(id, window)).unwrap();
    assert!(f.state.caminito.policy.is_fullscreen(policy_id));
    assert_snapshot!(format_configures(&f.client(id).window(window).configures));
}

#[test]
fn minimising_hides_the_window() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let window = f.map(id, WindowSpec::plain((200, 100))).xdg();
    let output = f.state.caminito.outputs[0].clone();

    f.client(id).set_minimized(window);
    f.roundtrip(id);

    let policy_id = f.policy_id(ClientWindow::Xdg(id, window)).unwrap();
    assert!(f.state.caminito.policy.is_minimized(policy_id));
    // Only the cursor.
    assert_eq!(f.state.scene_element_count(&output), 1);
}

#[test]
fn a_toplevel_with_a_parent_is_its_transient() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let parent = f.map(id, WindowSpec::plain((400, 300)));
    let child = f.map(
        id,
        WindowSpec {
            parent: Some(parent),
            ..WindowSpec::plain((200, 100))
        },
    );

    assert_eq!(
        f.state.caminito.policy.parent(f.pid(child)),
        Some(f.pid(parent))
    );
}

#[test]
fn setting_a_parent_after_mapping_makes_a_transient() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let parent = f.map(id, WindowSpec::plain((400, 300)));
    let child = f.map(id, WindowSpec::plain((200, 100)));

    f.client(id).set_parent(child.xdg(), Some(parent.xdg()));
    f.client(id).commit(child.xdg());
    f.roundtrip(id);

    assert_eq!(
        f.state.caminito.policy.parent(f.pid(child)),
        Some(f.pid(parent))
    );
}

#[test]
fn every_state_request_is_answered_with_a_configure() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let window = f.map(id, WindowSpec::plain((200, 100))).xdg();
    f.client(id).set_maximized(window);
    f.roundtrip(id);
    let configures = f.client(id).window(window).configures.len();

    // Already maximised, so nothing changes, but the client still hears back.
    f.client(id).set_maximized(window);
    f.roundtrip(id);

    assert_eq!(f.client(id).window(window).configures.len(), configures + 1);
}

#[test]
fn a_desynchronised_subsurface_commit_is_drawn() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let output = f.state.caminito.outputs[0].clone();
    let window = f.map(id, WindowSpec::plain((200, 100))).xdg();
    let subsurface = f
        .client(id)
        .add_subsurface(window, (10, 10), (20, 20), WINDOW_COLOR);
    f.client(id).commit(window);
    f.roundtrip(id);

    // Only the subsurface commits.
    f.client(id)
        .commit_subsurface_buffer(window, &subsurface, (20, 20), 0xff0000ff);
    f.roundtrip(id);

    let geometry = f.geometry(ClientWindow::Xdg(id, window));
    let pixels = f.state.backend.headless_mut().pixels(&output).unwrap();
    let (x, y) = (geometry.loc.x + 15, geometry.loc.y + 15);
    assert_eq!(pixels[(y * 1920 + x) as usize], 0xff0000ff);
}
