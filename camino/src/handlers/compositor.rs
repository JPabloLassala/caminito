//! `wl_compositor` and `wl_subcompositor`, which the seat's surface focus needs, and
//! `wl_shm` for clients' buffers.

use smithay::backend::renderer::utils::on_commit_buffer_handler;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, Resource};
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{
    CompositorClientState, CompositorHandler, CompositorState, get_parent,
};
use smithay::wayland::shm::{ShmHandler, ShmState};
use smithay::xwayland::XWaylandClientData;

use crate::state::{ClientState, State};

impl CompositorHandler for State {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.camino.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        if let Some(xwayland) = client.get_data::<XWaylandClientData>() {
            return &xwayland.compositor_state;
        }
        &client
            .get_data::<ClientState>()
            .expect("every client but Xwayland is inserted with ClientState")
            .compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);

        let mut root = surface.clone();
        while let Some(parent) = get_parent(&root) {
            root = parent;
        }
        if root == *surface {
            let xwayland = surface
                .client()
                .is_some_and(|client| client.get_data::<XWaylandClientData>().is_some());
            if xwayland {
                self.xwayland_commit(surface);
            } else {
                self.xdg_shell_commit(surface);
            }
        } else {
            // A subsurface: its window's cached geometry may have changed, and a
            // desynchronised one draws without its parent committing.
            let camino = &mut self.camino;
            if let Some(id) = camino.mapped_for_surface(&root) {
                camino.policy.window(id).unwrap().window().on_commit();
            }
            camino.queue_redraw_all();
        }
    }
}

impl BufferHandler for State {
    fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {}
}

impl ShmHandler for State {
    fn shm_state(&self) -> &ShmState {
        &self.camino.shm_state
    }
}
