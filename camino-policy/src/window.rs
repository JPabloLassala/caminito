//! The `PolicyWindow` seam: how policy sees a window and applies its decisions.

use smithay::utils::{FrameExtents, Logical, Point, Rectangle, Size};

use crate::OutputId;

/// A window's stable id, chosen by the compositor and unique among managed windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(pub u64);

/// Marco's window types (`MetaWindowType`), from the xdg role or `_NET_WM_WINDOW_TYPE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowType {
    #[default]
    Normal,
    Desktop,
    Dock,
    Dialog,
    ModalDialog,
    Toolbar,
    Menu,
    Utility,
    Splashscreen,
}

/// The client's size limits. A zero dimension means unbounded, as in xdg-shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SizeHints {
    pub min: Size<i32, Logical>,
    pub max: Size<i32, Logical>,
}

impl SizeHints {
    /// Clamps `size` into the hints, and to at least 1×1.
    pub fn clamp(&self, size: Size<i32, Logical>) -> Size<i32, Logical> {
        let dim = |v: i32, min: i32, max: i32| {
            let v = if max > 0 { v.min(max) } else { v };
            v.max(min).max(1)
        };
        Size::new(
            dim(size.w, self.min.w, self.max.w),
            dim(size.h, self.min.h, self.max.h),
        )
    }
}

/// The states policy asks a window to show, alongside its geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowStates {
    /// Maximised both ways. A window maximised one way is only given the size.
    pub maximized: bool,
    pub fullscreen: bool,
    /// Rolled up into its titlebar: only its frame's titlebar shows (05).
    pub shaded: bool,
    /// An interactive resize is in progress.
    pub resizing: bool,
}

/// A window as policy sees it: facts to read, and requests to apply decisions.
///
/// Policy owns its `W` values. It reads the facts when a window maps, when it commits, and when
/// the compositor says its frame changed, and calls each request only when what it asks for
/// changes.
pub trait PolicyWindow {
    // Facts.

    /// A stable id, unique among managed windows.
    fn id(&self) -> WindowId;
    fn window_type(&self) -> WindowType;
    /// The window this one is transient for, if any.
    fn transient_parent(&self) -> Option<WindowId>;
    /// The size of the window geometry (without the frame) the client last committed.
    fn size(&self) -> Size<i32, Logical>;
    fn size_hints(&self) -> SizeHints;
    /// A position the client asked for (X11's `USPosition` or `PPosition`), in global
    /// logical coordinates of the window geometry.
    fn requested_position(&self) -> Option<Point<i32, Logical>>;
    /// Whether the window takes keyboard input at all (ICCCM's input model). One that
    /// doesn't isn't focused when it maps.
    fn takes_input(&self) -> bool;
    fn pid(&self) -> Option<u32>;
    /// The xdg app id, or `WM_CLASS` for an XWayland window.
    fn app_id(&self) -> Option<String>;
    /// Whether Camino draws a frame for this window.
    fn is_ssd(&self) -> bool;
    /// The frame's size around the window geometry, maximised or not, since a theme can draw
    /// a maximised frame with other borders. Policy ignores it when the window isn't SSD, and
    /// while it's fullscreen.
    fn frame_extents(&self, maximized: bool) -> FrameExtents<i32, Logical>;

    // Requests.

    /// Places the window geometry at `geometry.loc` now, and asks the client for
    /// `geometry.size` with `states`.
    fn set_geometry(&mut self, geometry: Rectangle<i32, Logical>, states: WindowStates);
    fn set_activated(&mut self, activated: bool);
    /// Asks the client to close. Policy keeps managing the window until it unmaps.
    fn close(&mut self);
    /// The outputs the window is shown on; empty while it's hidden.
    fn set_outputs(&mut self, outputs: &[OutputId]);
}
