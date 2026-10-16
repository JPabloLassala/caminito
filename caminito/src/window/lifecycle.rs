//! A window's life, whichever protocol it speaks: pending while it has no content
//! (CONTEXT.md), managed by policy from its first commit with content, and out of policy
//! again once withdrawn or destroyed. The protocol handlers only translate their events
//! into this module's: created, pending state request, committed content, withdrawn and
//! destroyed.

use caminito_policy::{
    ActivationToken, FocusClaim, MapStates, PreMapWindow, StateRequest, WindowId,
};
use smithay::desktop::Window;
use smithay::desktop::space::SpaceElement;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::seat::WaylandFocus;

use super::{Mapped, WindowKey, adapter, send_preferred_scale};
use crate::decoration::WindowFacts;
use crate::state::{Caminito, SEAT};

/// A window its client has asked to show, which Caminito doesn't manage until it has
/// content, and the states it has asked for meanwhile, which it maps with.
#[derive(Debug)]
struct Pending {
    window: Window,
    states: MapStates,
}

/// The pending windows.
#[derive(Debug, Default)]
pub(crate) struct PendingWindows(Vec<Pending>);

impl PendingWindows {
    fn position(&self, key: &WindowKey) -> Option<usize> {
        self.0
            .iter()
            .position(|pending| WindowKey::of(&pending.window) == *key)
    }

    fn get(&self, key: &WindowKey) -> Option<&Pending> {
        self.position(key).map(|index| &self.0[index])
    }

    fn remove(&mut self, key: &WindowKey) -> Option<Pending> {
        self.position(key).map(|index| self.0.remove(index))
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Caminito {
    /// Created: the client has asked to show `window`, which is pending until it commits
    /// content.
    pub(crate) fn window_created(&mut self, window: Window) {
        self.pending.0.push(Pending {
            window,
            states: MapStates {
                // An XWayland window carries no token either (166).
                focus: FocusClaim::NoToken,
                ..MapStates::default()
            },
        });
    }

    /// Pending state request: a window asks for a state. Policy decides for a managed
    /// window; a pending one keeps it and is configured again. Returns whether the window
    /// is managed.
    pub(crate) fn window_state_request(&mut self, key: &WindowKey, request: StateRequest) -> bool {
        if let Some(id) = self.mapped_for_key(key) {
            let now = self.now();
            self.policy.request_state(SEAT, id, request, now);
            return true;
        }
        let Some(index) = self.pending.position(key) else {
            return false;
        };
        if self.pending.0[index].states.request(request) {
            self.configure_pending(key);
        }
        false
    }

    /// Activation requested: a client asks for the window to be activated with an
    /// xdg-activation token Caminito issued, or with one it doesn't know. Policy decides for
    /// a managed window; a pending one maps with it (165).
    pub(crate) fn window_activation_requested(
        &mut self,
        key: &WindowKey,
        token: Option<ActivationToken>,
    ) {
        if let Some(id) = self.mapped_for_key(key) {
            let now = self.now();
            self.policy.request_activation(SEAT, id, token, now);
        } else if let Some(index) = self.pending.position(key) {
            self.pending.0[index].states.focus =
                token.map_or(FocusClaim::NoToken, FocusClaim::Token);
        }
    }

    /// Committed content: the window's surface committed a buffer. A pending window maps
    /// into policy; a managed one tells policy what changed.
    pub(crate) fn window_committed(&mut self, key: &WindowKey) {
        if let Some(id) = self.mapped_for_key(key) {
            self.policy.window(id).unwrap().window().on_commit();
            // A picture kept for its closing is of a frame it has drawn over (252).
            self.open_close_animations.forget_kept(id);
            self.window_changed(id);
        } else if let Some(pending) = self.pending.remove(key) {
            pending.window.on_commit();
            self.map_pending(pending);
        }
    }

    /// Withdrawn: the client took the window off the screen. It leaves policy, and a
    /// remap is a new window with a new id.
    pub(crate) fn window_withdrawn(&mut self, key: &WindowKey) {
        self.pending.remove(key);
        if let Some(id) = self.mapped_for_key(key) {
            self.unmap_window(id);
        }
    }

    /// Destroyed: the window is gone.
    pub(crate) fn window_destroyed(&mut self, key: &WindowKey) {
        self.window_withdrawn(key);
    }

    /// The key of the window, pending or managed, whose surface is `surface`.
    pub(crate) fn window_key_for_surface(&self, surface: &WlSurface) -> Option<WindowKey> {
        let pending = self.pending.0.iter().map(|pending| &pending.window);
        let managed = self.policy.windows().map(|mapped| mapped.window());
        pending
            .chain(managed)
            .find(|window| window.wl_surface().as_deref() == Some(surface))
            .map(WindowKey::of)
    }

    /// The window, pending or managed, that `key` names.
    pub(crate) fn window_for_key(&self, key: &WindowKey) -> Option<&Window> {
        self.known_windows()
            .find(|window| WindowKey::of(window) == *key)
    }

    /// Every pending and managed window.
    pub(crate) fn known_windows(&self) -> impl Iterator<Item = &Window> {
        let pending = self.pending.0.iter().map(|pending| &pending.window);
        let managed = self.policy.windows().map(|mapped| mapped.window());
        pending.chain(managed)
    }

    /// Whether `key` names a pending window.
    pub(crate) fn is_pending(&self, key: &WindowKey) -> bool {
        self.pending.get(key).is_some()
    }

    /// A managed window's facts changed: reads them again, then has policy read them.
    pub(crate) fn window_changed(&mut self, id: WindowId) {
        if self.read_window_facts(id) {
            self.policy.commit(id);
            // The client's commit, with what it drew.
            self.queue_redraw_all();
        }
    }

    /// Reads a managed window's facts again, which policy must be told of next: the one
    /// place they're read after it maps. Returns whether the window is managed.
    pub(crate) fn read_window_facts(&mut self, id: WindowId) -> bool {
        let Some(mapped) = self.policy.window(id) else {
            return false;
        };
        let parent = self.transient_parent(mapped.window());
        let mapped = self.policy.window_mut(id).unwrap();
        mapped.read_facts(parent, &self.frame_context);
        true
    }

    /// Starts managing a pending window, with the states it asked for.
    fn map_pending(&mut self, pending: Pending) -> WindowId {
        let id = WindowId(self.next_window_id);
        self.next_window_id += 1;

        let Pending { window, states } = pending;
        let mut mapped = Mapped::new(id, window);
        mapped.read_facts(self.transient_parent(mapped.window()), &self.frame_context);

        let now = self.now();
        // For the placement modes that put a new window at the pointer (154).
        let pointer = self.seat.get_pointer().unwrap().current_location();
        self.policy.set_pointer(pointer);
        self.policy.map_window_with(mapped, states, now);
        // Shown, for the first time or again after its client hid it (251).
        self.open_close_animations.window_mapped(id);
        id
    }

    /// Stops managing a window, which closes from the picture kept of it, if any (252).
    fn unmap_window(&mut self, id: WindowId) {
        self.start_closing(id);
        self.menu_claims.toplevel_gone(id);
        // Its "not responding" dialog goes with it (168).
        self.forget_close_ping(id);
        if let Some(mapped) = self.policy.unmap_window(id) {
            for output in &self.outputs {
                mapped.window().output_leave(output);
            }
        }
    }

    /// Configures a pending window as policy says it'll map (190): the bounds of the output
    /// it'll go on, and the size it'll map at if it's maximised or fullscreen, with the
    /// states it has asked for and has the function for (177).
    pub(crate) fn configure_pending(&mut self, key: &WindowKey) {
        let Some(pending) = self.pending.get(key) else {
            return;
        };
        // So the client draws its first buffer at the scale it'll likely be shown at.
        let parent = self.transient_parent(&pending.window);
        if let Some(output) = self.main_output_until_drawn(parent) {
            pending.window.with_surfaces(|surface, data| {
                send_preferred_scale(surface, data, &output);
            });
        }
        let adapter = adapter(&pending.window);
        let ssd = adapter.wants_frame_pending();
        let window = PreMapWindow {
            window_type: adapter.window_type(parent.is_some()),
            hints: adapter.size_hints(),
            parent,
            ssd,
        };
        let facts = WindowFacts {
            window_type: window.window_type,
            wants_frame: ssd,
        };
        let states = pending.states;
        // For the placement modes that put a new window at the pointer (154), as at the map.
        let pointer = self.seat.get_pointer().unwrap().current_location();
        self.policy.set_pointer(pointer);
        let configure = self.policy.pre_map_configure(&window, &states, |state| {
            self.frame_context.extents(&facts, state)
        });
        adapter.configure_pending(&configure);
    }
}
