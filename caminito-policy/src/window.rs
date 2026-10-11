//! The `PolicyWindow` seam: how policy sees a window and applies its decisions.

use smithay::utils::{FrameExtents, Logical, Point, Rectangle, Size};

use crate::{Functions, OutputId};

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
    /// The half or quarter of a work area the window is tiled to (158).
    pub tile: Option<Tile>,
}

impl WindowStates {
    /// The state its frame is drawn and measured in, focused or not, with `functions`.
    /// Only a side tile has a theme style of its own, as in Marco's
    /// `META_WINDOW_TILED_LEFT` and `_RIGHT` (window-private.h:427-433) (158).
    pub fn frame_state(&self, focused: bool, functions: Functions) -> FrameState {
        FrameState {
            border_only: functions.border_only,
            resizes_horizontally: functions.resize_horizontally,
            resizes_vertically: functions.resize_vertically,
            maximized: self.maximized,
            focused,
            shaded: self.shaded,
            tiled_left: self.tile == Some(Tile::Left),
            tiled_right: self.tile == Some(Tile::Right),
        }
    }
}

/// A half or quarter of a work area that a window dragged to an output's side or corner
/// fills, as Marco's `MetaTileMode` has them (158). The top edge's tiling is a maximise,
/// so it isn't one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tile {
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Tile {
    /// The work area's edges the tile lies along, which xdg-shell's `tiled_*` states name.
    pub fn edges(self) -> TiledEdges {
        let (left, right) = match self {
            Tile::Left | Tile::TopLeft | Tile::BottomLeft => (true, false),
            Tile::Right | Tile::TopRight | Tile::BottomRight => (false, true),
        };
        let (top, bottom) = match self {
            Tile::Left | Tile::Right => (true, true),
            Tile::TopLeft | Tile::TopRight => (true, false),
            Tile::BottomLeft | Tile::BottomRight => (false, true),
        };
        TiledEdges {
            left,
            right,
            top,
            bottom,
        }
    }
}

/// Which of a window's edges lie along its work area's, for xdg-shell's `tiled_*` states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TiledEdges {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
}

/// The state of a window that its frame extents can depend on (ADR 0012), which policy lays
/// the window out in, and those of its functions that they can depend on (177).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FrameState {
    /// Maximised both ways.
    pub maximized: bool,
    /// Focused by some seat, so shown activated.
    pub focused: bool,
    pub shaded: bool,
    /// Tiled to the work area's left or right half, which a Marco theme can draw in its
    /// `tiled_left` and `tiled_right` styles. A quarter has no style of its own (158).
    pub tiled_left: bool,
    pub tiled_right: bool,
    /// The frame has no titlebar ([`Functions::border_only`]).
    pub border_only: bool,
    /// The ways the window has the resize function, which a Marco theme picks a frame's
    /// style by (`resize="both"`, `"horizontal"`, `"vertical"` or `"none"`).
    pub resizes_horizontally: bool,
    pub resizes_vertically: bool,
}

/// A window as policy sees it: facts to read, and requests to apply decisions.
///
/// Policy owns its `W` values. It reads the facts when a window maps, when it commits, and when
/// the compositor says its frame changed, except the frame extents, which it asks for whenever
/// it lays the window out. It calls each request only when what it asks for changes.
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
    /// logical coordinates of the window geometry, for a frame of `extents`.
    fn requested_position(
        &self,
        extents: FrameExtents<i32, Logical>,
    ) -> Option<Point<i32, Logical>>;
    /// The workspace the client asked to map on (X11's `_NET_WM_DESKTOP` before map),
    /// counted from 0.
    fn requested_workspace(&self) -> Option<usize>;
    /// Whether the window takes keyboard input at all (ICCCM's input model). One that
    /// doesn't isn't focused when it maps.
    fn takes_input(&self) -> bool;
    fn pid(&self) -> Option<u32>;
    /// The xdg app id, or `WM_CLASS` for an XWayland window.
    fn app_id(&self) -> Option<String>;
    /// Whether the window is left out of the taskbar, by its own asking or by Marco's rules
    /// for its type. The Window switcher leaves it out too (108).
    fn skips_taskbar(&self) -> bool;
    /// Whether the client asks for the user's attention itself, as an X client can.
    fn urgent(&self) -> bool;
    /// Whether Caminito draws a frame for this window.
    fn is_ssd(&self) -> bool;
    /// The frame's size around the window geometry in `state`, since a theme can draw a
    /// maximised, unfocused or shaded frame with other borders. Policy ignores it when the
    /// window isn't SSD, and while it's fullscreen.
    fn frame_extents(&self, state: FrameState) -> FrameExtents<i32, Logical>;

    // Requests.

    /// What the window can do, which its frame shows (177). Policy says before the first
    /// geometry, and before a geometry laid out with new ones.
    fn set_functions(&mut self, functions: Functions);
    /// Places the window geometry at `geometry.loc` now, and asks the client for
    /// `geometry.size` with `states`.
    fn set_geometry(&mut self, geometry: Rectangle<i32, Logical>, states: WindowStates);
    fn set_activated(&mut self, activated: bool);
    /// Asks the client to close. Policy keeps managing the window until it unmaps.
    fn close(&mut self);
    /// The outputs the window is shown on; empty while it's hidden.
    fn set_outputs(&mut self, outputs: &[OutputId]);
}
