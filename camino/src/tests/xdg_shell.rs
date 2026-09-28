//! xdg-shell: toplevels mapping into policy, their requests, and popups (22).

use camino_test_client::format_configures;
use insta::assert_snapshot;

use super::fixture::{Fixture, WINDOW_COLOR};

#[test]
fn a_toplevel_maps_into_policy_when_it_commits_a_buffer() {
    let (mut f, id) = Fixture::connected();

    let window = f.client(id).create_window();
    f.client(id).commit(window);
    f.roundtrip(id);
    assert_eq!(f.policy_id(id, window), None);

    f.client(id)
        .ack_and_commit_buffer(window, (200, 100), WINDOW_COLOR);
    f.roundtrip(id);

    let policy_id = f.policy_id(id, window).expect("mapped");
    let geometry = f.state.camino.policy.geometry(policy_id).unwrap();
    assert_eq!(geometry.size, (200, 100).into());
    assert_snapshot!(format_configures(&f.client(id).window(window).configures));
}

#[test]
fn a_mapped_toplevel_is_drawn_where_policy_placed_it() {
    let (mut f, id) = Fixture::connected();
    let output = f.state.camino.outputs[0].clone();

    let window = f.map_window(id, (200, 100));
    f.dispatch();

    let policy_id = f.policy_id(id, window).unwrap();
    let geometry = f.state.camino.policy.geometry(policy_id).unwrap();
    let pixels = f.state.backend.headless_mut().pixels(&output).unwrap();
    let at = |x: i32, y: i32| pixels[(y * 1920 + x) as usize];
    assert_eq!(at(geometry.loc.x, geometry.loc.y), WINDOW_COLOR);
    assert_eq!(at(geometry.loc.x + 199, geometry.loc.y + 99), WINDOW_COLOR);
}

#[test]
fn a_drawn_toplevel_gets_its_frame_callbacks() {
    let (mut f, id) = Fixture::connected();
    let window = f.map_window(id, (200, 100));

    f.client(id).request_frame(window);
    f.client(id).commit(window);
    f.roundtrip(id);
    f.dispatch();

    assert_eq!(f.client(id).window(window).frames_done, 1);
}

#[test]
fn committing_a_null_buffer_unmaps_and_a_new_buffer_maps_again() {
    let (mut f, id) = Fixture::connected();
    let window = f.map_window(id, (200, 100));

    f.client(id).commit_null_buffer(window);
    f.roundtrip(id);
    assert_eq!(f.policy_id(id, window), None);

    // After an unmap the client starts over with an initial commit, which gets a new
    // initial configure.
    let configures = f.client(id).window(window).configures.len();
    f.client(id).commit(window);
    f.roundtrip(id);
    assert_eq!(f.client(id).window(window).configures.len(), configures + 1);
    f.client(id)
        .ack_and_commit_buffer(window, (200, 100), WINDOW_COLOR);
    f.roundtrip(id);
    assert!(f.policy_id(id, window).is_some());
}

#[test]
fn destroying_a_toplevel_unmaps_it() {
    let (mut f, id) = Fixture::connected();
    let window = f.map_window(id, (200, 100));
    let policy_id = f.policy_id(id, window).unwrap();

    f.client(id).destroy_window(window);
    f.roundtrip(id);

    assert!(f.state.camino.policy.window(policy_id).is_none());
}

#[test]
fn maximising_configures_the_work_area_and_unmaximising_restores() {
    let (mut f, id) = Fixture::connected();
    let window = f.map_window(id, (200, 100));

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
    let (mut f, id) = Fixture::connected();
    f.add_output((1920, 1080), 1.5);
    f.roundtrip(id);
    f.client(id).bind_outputs();
    let window = f.map_window(id, (200, 100));

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
    let (mut f, id) = Fixture::connected();

    let window = f.client(id).create_window();
    f.client(id).set_maximized(window);
    f.client(id).commit(window);
    f.roundtrip(id);
    f.client(id)
        .ack_and_commit_buffer(window, (1920, 1080), WINDOW_COLOR);
    f.roundtrip(id);

    let policy_id = f.policy_id(id, window).unwrap();
    assert!(f.state.camino.policy.is_maximized(policy_id));
    assert_snapshot!(format_configures(&f.client(id).window(window).configures));
}

#[test]
fn fullscreen_before_mapping_starts_fullscreen() {
    let (mut f, id) = Fixture::connected();

    let window = f.client(id).create_window();
    f.client(id).set_fullscreen(window, None);
    f.client(id).commit(window);
    f.roundtrip(id);
    f.client(id)
        .ack_and_commit_buffer(window, (1920, 1080), WINDOW_COLOR);
    f.roundtrip(id);

    let policy_id = f.policy_id(id, window).unwrap();
    assert!(f.state.camino.policy.is_fullscreen(policy_id));
    assert_snapshot!(format_configures(&f.client(id).window(window).configures));
}

#[test]
fn minimising_hides_the_window() {
    let (mut f, id) = Fixture::connected();
    let window = f.map_window(id, (200, 100));
    let output = f.state.camino.outputs[0].clone();

    f.client(id).set_minimized(window);
    f.roundtrip(id);

    let policy_id = f.policy_id(id, window).unwrap();
    assert!(f.state.camino.policy.is_minimized(policy_id));
    // Only the cursor.
    assert_eq!(f.state.scene_element_count(&output), 1);
}

#[test]
fn a_toplevel_with_a_parent_is_its_transient() {
    let (mut f, id) = Fixture::connected();
    let parent = f.map_window(id, (400, 300));
    let child = f.client(id).create_window();
    f.client(id).set_parent(child, Some(parent));
    f.client(id).commit(child);
    f.roundtrip(id);
    f.client(id)
        .ack_and_commit_buffer(child, (200, 100), WINDOW_COLOR);
    f.roundtrip(id);

    let (parent, child) = (f.policy_id(id, parent), f.policy_id(id, child));
    assert_eq!(f.state.camino.policy.parent(child.unwrap()), parent);
}

#[test]
fn setting_a_parent_after_mapping_makes_a_transient() {
    let (mut f, id) = Fixture::connected();
    let parent = f.map_window(id, (400, 300));
    let child = f.map_window(id, (200, 100));

    f.client(id).set_parent(child, Some(parent));
    f.client(id).commit(child);
    f.roundtrip(id);

    let (parent, child) = (f.policy_id(id, parent), f.policy_id(id, child));
    assert_eq!(f.state.camino.policy.parent(child.unwrap()), parent);
}

#[test]
fn every_state_request_is_answered_with_a_configure() {
    let (mut f, id) = Fixture::connected();
    let window = f.map_window(id, (200, 100));
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
    let (mut f, id) = Fixture::connected();
    let output = f.state.camino.outputs[0].clone();
    let window = f.map_window(id, (200, 100));
    let subsurface = f
        .client(id)
        .add_subsurface(window, (10, 10), (20, 20), WINDOW_COLOR);
    f.client(id).commit(window);
    f.roundtrip(id);
    f.dispatch();

    // Only the subsurface commits.
    f.client(id)
        .commit_subsurface_buffer(window, &subsurface, (20, 20), 0xff0000ff);
    f.roundtrip(id);
    f.dispatch();

    let policy_id = f.policy_id(id, window).unwrap();
    let geometry = f.state.camino.policy.geometry(policy_id).unwrap();
    let pixels = f.state.backend.headless_mut().pixels(&output).unwrap();
    let (x, y) = (geometry.loc.x + 15, geometry.loc.y + 15);
    assert_eq!(pixels[(y * 1920 + x) as usize], 0xff0000ff);
}
