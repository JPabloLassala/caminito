//! A window's life, whichever protocol it speaks: pending while it has no content
//! (CONTEXT.md), managed by policy from its first commit with content, and out of policy
//! again once withdrawn or destroyed. The protocol handlers only translate their events
//! into this module's: created, pending state request, committed content, withdrawn and
//! destroyed.

use caminito_policy::{FrameState, OutputId, SizeHints, WindowId};
use smithay::desktop::Window;
use smithay::desktop::space::SpaceElement;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{FrameExtents, Logical, Size};
use smithay::wayland::seat::WaylandFocus;

use super::{Mapped, PreMapConfigure, WindowKey, adapter, send_preferred_scale};
use crate::decoration::WindowFacts;
use crate::state::Caminito;

/// A window its client has asked to show, which Caminito doesn't manage until it has
/// content, and the states it has asked for meanwhile, which it maps with.
#[derive(Debug)]
struct Pending {
    window: Window,
    maximized: bool,
    /// Fullscreen was asked for, on this output if the client named one.
    fullscreen: Option<Option<OutputId>>,
    above: bool,
    below: bool,
    sticky: bool,
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

/// A state a window asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StateRequest {
    Maximized(bool),
    /// Fullscreen, on this output if the client named one.
    Fullscreen(Option<OutputId>),
    Unfullscreen,
    Minimized,
    /// Kept above others, or not. Recorded only until M8 (17), as are the two below.
    Above(bool),
    Below(bool),
    Sticky(bool),
}

impl Caminito {
    /// Created: the client has asked to show `window`, which is pending until it commits
    /// content.
    pub(crate) fn window_created(&mut self, window: Window) {
        self.pending.0.push(Pending {
            window,
            maximized: false,
            fullscreen: None,
            above: false,
            below: false,
            sticky: false,
        });
    }

    /// Pending state request: a window asks for a state. Policy decides for a managed
    /// window; a pending one keeps it and is configured again. Returns whether the window
    /// is managed.
    pub(crate) fn window_state_request(&mut self, key: &WindowKey, request: StateRequest) -> bool {
        if let Some(id) = self.mapped_for_key(key) {
            let policy = &mut self.policy;
            match request {
                StateRequest::Maximized(maximized) => policy.set_maximized(id, maximized),
                StateRequest::Fullscreen(output) => policy.set_fullscreen(id, true, output),
                StateRequest::Unfullscreen => policy.set_fullscreen(id, false, None),
                StateRequest::Minimized => policy.minimize(id),
                StateRequest::Above(above) => policy.set_above(id, above),
                StateRequest::Below(below) => policy.set_below(id, below),
                StateRequest::Sticky(sticky) => policy.set_sticky(id, sticky),
            }
            self.queue_redraw_all();
            return true;
        }
        let Some(index) = self.pending.position(key) else {
            return false;
        };
        let pending = &mut self.pending.0[index];
        match request {
            StateRequest::Maximized(maximized) => pending.maximized = maximized,
            StateRequest::Fullscreen(output) => pending.fullscreen = Some(output),
            StateRequest::Unfullscreen => pending.fullscreen = None,
            // A window that isn't managed has nothing to minimise.
            StateRequest::Minimized => return false,
            // Neither changes how the window is configured before it maps.
            StateRequest::Above(above) => {
                pending.above = above;
                return false;
            }
            StateRequest::Below(below) => {
                pending.below = below;
                return false;
            }
            StateRequest::Sticky(sticky) => {
                pending.sticky = sticky;
                return false;
            }
        }
        self.configure_pending(key);
        false
    }

    /// Committed content: the window's surface committed a buffer. A pending window maps
    /// into policy; a managed one tells policy what changed.
    pub(crate) fn window_committed(&mut self, key: &WindowKey) {
        if let Some(id) = self.mapped_for_key(key) {
            self.policy.window(id).unwrap().window().on_commit();
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

        let Pending {
            window,
            maximized,
            fullscreen,
            above,
            below,
            sticky,
        } = pending;
        let mut mapped = Mapped::new(id, window);
        mapped.read_facts(self.transient_parent(mapped.window()), &self.frame_context);

        let now = self.now();
        self.policy.map_window(mapped, now);
        if let Some(output) = fullscreen {
            self.policy.set_fullscreen(id, true, output);
        } else if maximized {
            self.policy.set_maximized(id, true);
        }
        self.policy.set_above(id, above);
        self.policy.set_below(id, below);
        self.policy.set_sticky(id, sticky);
        self.queue_redraw_all();
        id
    }

    /// Stops managing a window.
    fn unmap_window(&mut self, id: WindowId) {
        self.menu_claims.toplevel_gone(id);
        if let Some(mapped) = self.policy.unmap_window(id) {
            for output in &self.outputs {
                mapped.window().output_leave(output);
            }
            self.queue_redraw_all();
        }
    }

    /// Configures a pending window from the states it has asked for: the size it'll map at
    /// if it's maximised or fullscreen, and the bounds of the output it'll likely map on,
    /// both inside the frame it'll have.
    pub(crate) fn configure_pending(&self, key: &WindowKey) {
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
        let output = self.placement_bounds();
        let (extents, maximized_extents) = self.pending_extents(&pending.window);
        let bounds = output.map(|size| inside(size, extents));
        let fullscreen_size = pending.fullscreen.map(|output| {
            output
                .and_then(|id| self.policy.outputs().iter().find(|o| o.id == id))
                .map(|o| o.geometry.size)
                .or(bounds)
        });
        let size = if let Some(size) = fullscreen_size {
            size
        } else if pending.maximized {
            output.map(|size| inside(size, maximized_extents))
        } else {
            None
        };
        adapter(&pending.window).configure_pending(&PreMapConfigure {
            bounds,
            size,
            maximized: pending.maximized,
            fullscreen: pending.fullscreen.is_some(),
        });
    }

    /// The extents, unmaximised and maximised, of the frame a pending window will have, if
    /// it'll want a frame once configured.
    fn pending_extents(
        &self,
        window: &Window,
    ) -> (FrameExtents<i32, Logical>, FrameExtents<i32, Logical>) {
        let adapter = adapter(window);
        if !adapter.wants_frame_pending() {
            return Default::default();
        }
        let has_parent = self.transient_parent(window).is_some();
        let facts = WindowFacts {
            window_type: adapter.window_type(has_parent),
            has_parent,
            hints: SizeHints::default(),
            wants_frame: true,
        };
        let extents = |maximized| {
            let state = FrameState {
                maximized,
                // A new window is focused.
                focused: true,
                shaded: false,
            };
            self.frame_context.extents(&facts, state)
        };
        (extents(false), extents(true))
    }
}

/// The size inside a frame of `extents` that fills `size`.
fn inside(size: Size<i32, Logical>, extents: FrameExtents<i32, Logical>) -> Size<i32, Logical> {
    Size::new(
        (size.w - extents.left - extents.right).max(1),
        (size.h - extents.top - extents.bottom).max(1),
    )
}
