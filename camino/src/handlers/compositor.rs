//! `wl_compositor` and `wl_subcompositor`, which the seat's surface focus needs, and
//! `wl_shm` for clients' buffers. A commit of a dmabuf waits until the GPU is done
//! writing it (57).

use smithay::backend::renderer::utils::on_commit_buffer_handler;
use smithay::reexports::calloop::{EventSource, Interest};
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, Resource};
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{
    Blocker, BufferAssignment, CompositorClientState, CompositorHandler, CompositorState,
    SurfaceAttributes, add_blocker, add_pre_commit_hook, get_parent, with_states,
};
use smithay::wayland::dmabuf::get_dmabuf;
use smithay::wayland::drm_syncobj::DrmSyncobjCachedState;
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

    fn new_surface(&mut self, surface: &WlSurface) {
        add_pre_commit_hook::<Self, _>(surface, |state, _, surface| {
            state.block_on_dmabuf(surface);
        });
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);
        if self.commit_if_drag_icon(surface) {
            return;
        }

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

impl State {
    /// Holds back a commit of a dmabuf until the client's GPU is done writing it, so the
    /// scene never draws, or waits on, an unfinished buffer. With explicit sync, done is
    /// the commit's acquire point; without it, the dmabuf's implicit fence. Adapted from
    /// anvil's `new_surface`, at Smithay commit 928d4a9, `anvil/src/shell/mod.rs:117-170`.
    fn block_on_dmabuf(&mut self, surface: &WlSurface) {
        let (dmabuf, acquire_point) = with_states(surface, |states| {
            let acquire_point = states
                .cached_state
                .get::<DrmSyncobjCachedState>()
                .pending()
                .acquire_point
                .clone();
            let dmabuf = match &states
                .cached_state
                .get::<SurfaceAttributes>()
                .pending()
                .buffer
            {
                Some(BufferAssignment::NewBuffer(buffer)) => get_dmabuf(buffer).ok().cloned(),
                _ => None,
            };
            (dmabuf, acquire_point)
        });
        let Some(dmabuf) = dmabuf else {
            return;
        };
        // drm-syncobj holds a client to an acquire point with every dmabuf, but one whose
        // eventfd can't be made falls back to the implicit fence.
        if let Some(acquire_point) = acquire_point {
            match acquire_point.generate_blocker() {
                Ok((blocker, source)) => {
                    self.block_until(surface, blocker, source);
                    return;
                }
                Err(err) => tracing::debug!("can't wait on an acquire point: {err}"),
            }
        }
        // `Err` means the dmabuf is ready already.
        if let Ok((blocker, source)) = dmabuf.generate_blocker(Interest::READ) {
            self.block_until(surface, blocker, source);
        }
    }

    /// Holds back `surface`'s commit until `source` fires, then lets its client's commits
    /// through.
    fn block_until<S>(
        &mut self,
        surface: &WlSurface,
        blocker: impl Blocker + Send + 'static,
        source: S,
    ) where
        S: EventSource<Event = (), Ret = Result<(), std::io::Error>> + 'static,
    {
        let Some(client) = surface.client() else {
            return;
        };
        let inserted = self
            .camino
            .loop_handle
            .insert_source(source, move |_, _, state| {
                let dh = state.camino.display_handle.clone();
                state
                    .client_compositor_state(&client)
                    .blocker_cleared(state, &dh);
                Ok(())
            });
        match inserted {
            Ok(_) => add_blocker(surface, blocker),
            Err(err) => tracing::debug!("can't wait for a client's buffer: {err}"),
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
