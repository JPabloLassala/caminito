//! `Mapped`: a window Camino manages, as policy sees it.

use camino_policy::{OutputId, PolicyWindow, SizeHints, WindowId, WindowStates, WindowType};
use smithay::desktop::Window;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::utils::{FrameExtents, Logical, Point, Rectangle, Size};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::xdg::{SurfaceCachedState, ToplevelSurface, XdgToplevelSurfaceData};

/// Smithay's `Window`, which covers xdg toplevels and XWayland windows alike, plus Camino's
/// per-window data.
#[derive(Debug)]
pub struct Mapped {
    id: WindowId,
    window: Window,
    parent: Option<WindowId>,
    pid: Option<u32>,
    outputs: Vec<OutputId>,
}

impl Mapped {
    /// `pid` comes from the client's credentials.
    pub fn new(id: WindowId, window: Window, pid: Option<u32>) -> Self {
        Self {
            id,
            window,
            parent: None,
            pid,
            outputs: Vec::new(),
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    /// Records the managed window this one is transient for.
    pub fn set_parent(&mut self, parent: Option<WindowId>) {
        self.parent = parent;
    }

    /// The outputs policy last put the window on.
    pub fn outputs(&self) -> &[OutputId] {
        &self.outputs
    }

    fn toplevel(&self) -> Option<&ToplevelSurface> {
        self.window.toplevel()
    }
}

impl PolicyWindow for Mapped {
    fn id(&self) -> WindowId {
        self.id
    }

    fn window_type(&self) -> WindowType {
        // Marco makes a normal window with a transient parent a dialog.
        if self.parent.is_some() {
            WindowType::Dialog
        } else {
            WindowType::Normal
        }
    }

    fn transient_parent(&self) -> Option<WindowId> {
        self.parent
    }

    fn size(&self) -> Size<i32, Logical> {
        self.window.geometry().size
    }

    fn size_hints(&self) -> SizeHints {
        let Some(toplevel) = self.toplevel() else {
            return SizeHints::default();
        };
        with_states(toplevel.wl_surface(), |states| {
            let mut cached = states.cached_state.get::<SurfaceCachedState>();
            let current = cached.current();
            SizeHints {
                min: current.min_size,
                max: current.max_size,
            }
        })
    }

    fn requested_position(&self) -> Option<Point<i32, Logical>> {
        // xdg toplevels can't ask for a position; XWayland windows come in M3.
        None
    }

    fn pid(&self) -> Option<u32> {
        self.pid
    }

    fn app_id(&self) -> Option<String> {
        let toplevel = self.toplevel()?;
        with_states(toplevel.wl_surface(), |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()?
                .lock()
                .unwrap()
                .app_id
                .clone()
        })
    }

    fn is_ssd(&self) -> bool {
        // Every window is CSD until frames come in M2 (17).
        false
    }

    fn frame_extents(&self) -> FrameExtents<i32, Logical> {
        FrameExtents::default()
    }

    fn set_geometry(&mut self, geometry: Rectangle<i32, Logical>, states: WindowStates) {
        let Some(toplevel) = self.toplevel() else {
            return;
        };
        toplevel.with_pending_state(|pending| {
            pending.size = Some(geometry.size);
            for (state, on) in [
                (xdg_toplevel::State::Maximized, states.maximized),
                (xdg_toplevel::State::Fullscreen, states.fullscreen),
                (xdg_toplevel::State::Resizing, states.resizing),
            ] {
                if on {
                    pending.states.set(state);
                } else {
                    pending.states.unset(state);
                }
            }
        });
        toplevel.send_pending_configure();
    }

    fn set_activated(&mut self, activated: bool) {
        self.window.set_activated(activated);
        if let Some(toplevel) = self.toplevel() {
            toplevel.send_pending_configure();
        }
    }

    fn close(&mut self) {
        if let Some(toplevel) = self.toplevel() {
            toplevel.send_close();
        }
    }

    fn set_outputs(&mut self, outputs: &[OutputId]) {
        self.outputs = outputs.to_vec();
    }
}
