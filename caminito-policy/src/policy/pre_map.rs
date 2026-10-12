//! What a Pending window is configured with before it maps (190). Policy doesn't hold
//! Pending windows: the compositor keeps them, and asks policy what their first configure
//! says, which policy answers as it'll decide it at the map.

use super::*;

/// What the compositor knows of a window before it maps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreMapWindow {
    /// Its type, as `PolicyWindow::window_type` will say once it maps.
    pub window_type: WindowType,
    /// Its size hints, which its functions depend on (177).
    pub hints: SizeHints,
    /// The window it says it's transient for, if policy manages it.
    pub parent: Option<WindowId>,
    /// Caminito will draw its frame.
    pub ssd: bool,
}

/// A Pending window's configure before it maps.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PreMapConfigure {
    /// The work area of the output it'll map on, inside the frame it'll have.
    pub bounds: Option<Size<i32, Logical>>,
    /// The size it'll map at, if policy sets it.
    pub size: Option<Size<i32, Logical>>,
    /// It'll map maximised: it asked to, has the function for it (177), and doesn't map
    /// fullscreen.
    pub maximized: bool,
    /// It'll map fullscreen: it asked to and has the function for it.
    pub fullscreen: bool,
}

impl<W: PolicyWindow> Policy<W> {
    /// What a window that maps with `states` is configured with before it maps: the bounds
    /// and size `map_window_with` will give it, with nothing changed between the two, as
    /// far as they're known before the map. The frame's extents in a state are
    /// `frame_extents`'s, as `PolicyWindow::frame_extents` gives a managed window's. Where
    /// the window would go by a position of its own, which isn't known before the map, it's
    /// placed by the automatic mode's choice of output. The pointer is the one
    /// `set_pointer` gave.
    pub fn pre_map_configure(
        &self,
        window: &PreMapWindow,
        states: &MapStates,
        frame_extents: impl Fn(FrameState) -> FrameExtents<i32, Logical>,
    ) -> PreMapConfigure {
        let functions = self.pre_map_functions(window);
        // As `map_window_with` has them: a state the window has no function for is one it
        // won't map with (177), and one that maps fullscreen isn't maximised.
        let fullscreen = states.fullscreen.filter(|_| functions.fullscreen);
        let maximized = states.maximized && functions.maximize && fullscreen.is_none();
        let parent = self.managed_parent(window);
        let (_, output) = self.new_window_placement(
            window.window_type,
            parent,
            !maximized && fullscreen.is_none(),
        );
        let output = output.or(self.primary_output());
        let extents = |maximized| {
            if window.ssd {
                frame_extents(self.pre_map_frame_state(window, maximized))
            } else {
                FrameExtents::default()
            }
        };
        let work_area = |output: &Output| Self::work_area(output).size;
        let bounds = output.map(|output| inside(work_area(output), extents(false)));
        let size = if let Some(named) = fullscreen {
            // `relayout`'s: the output named, if policy has it, else the one it's on.
            self.output_named(named)
                .or(output)
                .map(|output| output.geometry.size)
        } else if maximized {
            output.map(|output| inside(work_area(output), extents(true)))
        } else {
            None
        };
        PreMapConfigure {
            bounds,
            size,
            maximized,
            fullscreen: fullscreen.is_some(),
        }
    }

    /// The state the frame of a window that isn't managed yet is asked for in, once it
    /// maps, maximised or not: as policy will have it, with the functions it'll have
    /// (177), focused, as a new window is (ADR 0012).
    pub fn pre_map_frame_state(&self, window: &PreMapWindow, maximized: bool) -> FrameState {
        let states = WindowStates {
            maximized,
            ..WindowStates::default()
        };
        states.frame_state(true, self.pre_map_functions(window))
    }

    /// The functions a window that isn't managed yet will have once it maps, out of
    /// fullscreen.
    fn pre_map_functions(&self, window: &PreMapWindow) -> Functions {
        let facts = FunctionFacts {
            window_type: window.window_type,
            hints: window.hints,
            has_parent: self.managed_parent(window).is_some(),
            ssd: window.ssd,
            fullscreen: false,
        };
        functions::functions(&facts, self.config.attach_modal_dialogs)
    }

    /// The window's parent, if policy manages it, as `map` takes it.
    fn managed_parent(&self, window: &PreMapWindow) -> Option<WindowId> {
        window
            .parent
            .filter(|parent| self.windows.contains_key(parent))
    }
}

/// The size inside a frame of `extents` that fills `size`.
fn inside(size: Size<i32, Logical>, extents: FrameExtents<i32, Logical>) -> Size<i32, Logical> {
    Size::new(
        (size.w - extents.left - extents.right).max(1),
        (size.h - extents.top - extents.bottom).max(1),
    )
}
