//! A test client talking to `State` through the fixture (14).

use caminito_test_client::{ResizeEdge, format_configures, format_globals};
use insta::assert_snapshot;
use smithay::backend::renderer::utils::with_renderer_surface_state;
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::xdg::XdgToplevelSurfaceData;

use super::fixture::{Fixture, Kind};

#[test]
fn a_client_lists_the_globals() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);

    assert_snapshot!(format_globals(&f.client(id).state.globals));
}

#[test]
fn a_new_toplevel_gets_a_first_configure() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);

    let window = f.client(id).create_window();
    f.client(id).commit(window);
    f.roundtrip(id);

    assert_snapshot!(format_configures(&f.client(id).window(window).configures));
}

#[test]
fn a_toplevel_commits_a_buffer_and_its_properties() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);

    let parent = f.client(id).create_window();
    f.client(id).commit(parent);
    let window = f.client(id).create_window();
    f.client(id).set_title(window, "Caja");
    f.client(id).set_app_id(window, "caja");
    f.client(id).set_parent(window, Some(parent));
    f.client(id).commit(window);
    f.roundtrip(id);

    f.client(id)
        .ack_and_commit_buffer(window, (200, 100), 0xff00ff00);
    f.client(id).start_move(window, 0);
    f.client(id)
        .start_resize(window, 0, ResizeEdge::BottomRight);
    f.client(id).set_minimized(window);
    f.roundtrip(id);

    let toplevels = f.state.caminito.xdg_shell_state.toplevel_surfaces();
    let [parent, window] = toplevels else {
        panic!("two toplevels, got {}", toplevels.len());
    };
    let (title, app_id) = with_states(window.wl_surface(), |states| {
        let data = states
            .data_map
            .get::<XdgToplevelSurfaceData>()
            .unwrap()
            .lock()
            .unwrap();
        (data.title.clone(), data.app_id.clone())
    });
    assert_eq!(title.as_deref(), Some("Caja"));
    assert_eq!(app_id.as_deref(), Some("caja"));
    assert_eq!(window.parent().as_ref(), Some(parent.wl_surface()));
    let buffer_size = with_renderer_surface_state(window.wl_surface(), |s| s.buffer_size());
    assert_eq!(buffer_size, Some(Some((200, 100).into())));
}
