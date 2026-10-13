//! `Policy`: Caminito's window-management model, driven by events and applied through `W`.

use std::cmp::Ordering;
use std::time::Duration;

use std::collections::{BTreeMap, BTreeSet};

use smithay::utils::{FrameExtents, Logical, Point, Rectangle, Size};

use crate::edges::{By, EdgeWindow, Edges};
use crate::functions;
use crate::grid::Direction;
use crate::placement::{self, Obstacle};
use crate::stack::{self, Layer};
use crate::{
    Action, FrameState, FunctionFacts, Functions, Grid, Output, OutputId, PlacementMode,
    PolicyWindow, ShowDesktopSkipList, SizeHints, Tile, WindowId, WindowStates, WindowType,
    WorkspaceLayout, WrapStyle,
};

mod active_workspaces;
mod dynamic_workspaces;
mod focus_stealing;
mod interaction;
mod keyboard_grab;
mod move_to;
mod pointer;
mod pre_map;
mod state_requests;
mod switcher;
mod window_actions;
mod window_menu;

pub use active_workspaces::OutputSwitch;
pub use focus_stealing::{
    ActivationSource, ActivationToken, FocusClaim, FocusNewWindows, TOKEN_LIFETIME, UrgentPlacement,
};
pub use interaction::{Interaction, WorkspacePopup};
pub use keyboard_grab::{GrabKey, GrabStep};
pub use pointer::{Crossing, FocusMode, PointerTarget};
pub use pre_map::{PreMapConfigure, PreMapWindow};
pub use state_requests::{MapStates, StateRequest};
pub use switcher::{LayerSurfaceId, MinimizedPlacement, SwitchEntry, SwitchList, Switcher};
pub use window_menu::{MenuEntry, MenuItem, MenuKey, MenuMark, MenuOp, MenuSelection, WindowMenu};

/// A seat, as the compositor numbers them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SeatId(pub u32);

/// The edges an interactive resize moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ResizeEdges {
    pub top: bool,
    pub bottom: bool,
    pub left: bool,
    pub right: bool,
}

impl ResizeEdges {
    pub const TOP_LEFT: Self = Self {
        top: true,
        bottom: false,
        left: true,
        right: false,
    };
    pub const BOTTOM_RIGHT: Self = Self {
        top: false,
        bottom: true,
        left: false,
        right: true,
    };
}

/// An interactive move or resize a seat is driving.
struct Grab {
    window: WindowId,
    /// The edges being resized, or `None` for a move.
    resize: Option<ResizeEdges>,
    start_pointer: Point<f64, Logical>,
    /// The window geometry when the grab began, at its committed size, or when a key last
    /// stepped it.
    start: Rectangle<i32, Logical>,
    /// The window geometry when the grab began, which Escape puts it back to.
    initial: Rectangle<i32, Logical>,
    /// A keyboard move or resize (156), which the keys step, put down and put back.
    keyboard: bool,
    /// A manual placement of a new window (154): a keyboard move that doesn't warp the
    /// pointer.
    placing: bool,
    /// Where the pointer is to be warped to, for the compositor to take (156).
    warp: Option<Point<i32, Logical>>,
    /// Whether the pointer has gone Marco's 8 pixels from where the grab began
    /// (display.c:3995-3997), past which it isn't a click.
    moved: bool,
    /// Where letting go of a move tiles or maximises the window, while the pointer is in a
    /// side's, a corner's or the top's zone (158).
    tile_target: Option<TileTarget>,
    /// A move its client asked for of a maximised window, which stays maximised until the
    /// pointer shakes it loose (144).
    held: bool,
}

/// What letting go of a move does with the window, from where the pointer is (158).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TileTarget {
    /// Tiles it on the output.
    Tile(Tile, OutputId),
    /// Maximises it, as `allow-top-tiling` has the top edge do.
    Maximize(OutputId),
}

/// How far, on either axis, the pointer goes from where a grab began before it's no longer
/// a click, as Marco has it (display.c:3995-3997).
const CLICK_DISTANCE: f64 = 8.;

/// Marco's `MAX_REASONABLE_WORKSPACES`, the schema's upper bound on `num-workspaces`.
pub const MAX_WORKSPACES: usize = 36;

/// Policy's settings, from GSettings (09).
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// `num-workspaces`, which policy keeps within 1 to `MAX_WORKSPACES`. Policy doesn't
    /// read it while `dynamic_workspaces` is on.
    pub workspace_count: usize,
    /// Each workspace's name, from the first. Policy only holds them, for ext-workspace
    /// and the debug dump; a missing name is Marco's "Workspace N".
    pub workspace_names: Vec<String>,
    /// Marco master's `dynamic-workspaces` (151): policy adds and removes workspaces as
    /// windows come and go, and the count is its own.
    pub dynamic_workspaces: bool,
    /// The `workspace-layout-*` keys: the grid the workspaces are in.
    pub workspace_layout: WorkspaceLayout,
    /// `wrap-style`: where a move past an edge of the grid goes (150).
    pub wrap_style: WrapStyle,
    /// `alt-tab-raise-windows`: the Window switcher raises the selected window while it's
    /// open (108).
    pub alt_tab_raise_windows: bool,
    /// Marco master's `alt-tab-minimized-placement`: where the Window switcher's lists have
    /// the minimised windows (163).
    pub alt_tab_minimized_placement: MinimizedPlacement,
    /// `placement-mode` (154).
    pub placement_mode: PlacementMode,
    /// `center-new-windows`: the automatic mode puts a new window in the middle of the work
    /// area (154).
    pub center_new_windows: bool,
    /// `show-desktop-skip-list` (164).
    pub show_desktop_skip_list: ShowDesktopSkipList,
    /// `focus-mode` (153).
    pub focus_mode: FocusMode,
    /// `raise-on-click`: off, a click on a window's body focuses it without raising it. A
    /// click on its Frame always raises it (153).
    pub raise_on_click: bool,
    /// `auto-raise`: with a pointer focus mode, a window the pointer focuses is raised
    /// after `auto_raise_delay`, if the pointer is still in it (153).
    pub auto_raise: bool,
    /// `auto-raise-delay`.
    pub auto_raise_delay: Duration,
    /// `allow-tiling`: a window dragged to an output's side or corner is tiled there (158).
    pub allow_tiling: bool,
    /// `allow-top-tiling`: with `allow-tiling`, a window dragged to an output's top is
    /// maximised (158).
    pub allow_top_tiling: bool,
    /// `allow-tile-cycling`: repeated presses of a tiling keybinding cycle the tile's width
    /// through Marco's sizes before untiling it (159).
    pub allow_tile_cycling: bool,
    /// `org.mate.peripherals-mouse drag-threshold`, in logical pixels: twice it is how far
    /// into the work area a side's and a corner's tiling zones reach, as in Marco (158).
    pub drag_threshold: i32,
    /// `focus-new-windows` (165).
    pub focus_new_windows: FocusNewWindows,
    /// `alt-tab-urgent-placement`, where the installed schema has it (165).
    pub urgent_placement: UrgentPlacement,
    /// `attach-modal-dialogs`: a modal dialog with a parent has a border only (177).
    pub attach_modal_dialogs: bool,
    /// `org.mate.Caminito`'s `workspaces-per-output` (ADR 0032): each output switches its
    /// own active workspace. Off, every switch switches them all: Spanning workspaces.
    pub workspaces_per_output: bool,
}

impl Default for Config {
    /// Marco's defaults, except `center-new-windows`, `allow-tiling` and
    /// `workspaces-per-output`, which are off: the tests were written for Caminito before
    /// 154, 158 and 230.
    fn default() -> Self {
        // Marco's default `num-workspaces`, with its default names.
        let workspace_count = 4;
        Self {
            workspace_count,
            workspace_names: (1..=workspace_count).map(default_workspace_name).collect(),
            workspace_layout: WorkspaceLayout::default(),
            wrap_style: WrapStyle::NoWrap,
            dynamic_workspaces: false,
            alt_tab_raise_windows: false,
            alt_tab_minimized_placement: MinimizedPlacement::End,
            placement_mode: PlacementMode::Automatic,
            center_new_windows: false,
            show_desktop_skip_list: ShowDesktopSkipList::default(),
            // Marco's defaults.
            focus_mode: FocusMode::Click,
            raise_on_click: true,
            auto_raise: false,
            auto_raise_delay: Duration::from_millis(500),
            allow_tiling: false,
            allow_top_tiling: true,
            allow_tile_cycling: true,
            // msd's default.
            drag_threshold: 8,
            focus_new_windows: FocusNewWindows::Smart,
            // Marco 1.29.0's, which has no key (165).
            urgent_placement: UrgentPlacement::Mru,
            attach_modal_dialogs: false,
            // Spanning workspaces, as Marco has them: the tests were written for them,
            // before 230. The schema's default is on.
            workspaces_per_output: false,
        }
    }
}

/// Marco's name for a workspace with none set, numbered from 1.
pub fn default_workspace_name(number: usize) -> String {
    format!("Workspace {number}")
}

/// A workspace showing its desktop (94).
#[derive(Clone)]
struct ShownDesktop {
    /// The window every seat focused when it was shown.
    focus: BTreeMap<SeatId, WindowId>,
    /// `show-desktop-skip-list` when it was shown: a change waits for the next time.
    skip_list: ShowDesktopSkipList,
}

/// What policy has last asked of a window, so each request is sent only when it changes.
#[derive(Default)]
struct Sent {
    functions: Option<Functions>,
    geometry: Option<(Rectangle<i32, Logical>, WindowStates)>,
    activated: bool,
    outputs: Option<Vec<OutputId>>,
}

/// A window policy manages, with policy's state for it.
struct Managed<W> {
    window: W,
    window_type: WindowType,
    /// The transient parent, when it's managed and linking to it makes no cycle.
    parent: Option<WindowId>,
    workspace: usize,
    /// `loc` is where the window geometry is placed, `size` the size the client is asked for.
    geometry: Rectangle<i32, Logical>,
    /// The size the client last committed.
    size: Size<i32, Logical>,
    hints: SizeHints,
    ssd: bool,
    /// Focused by some seat, as policy lays it out: the frame extents can depend on it.
    activated: bool,
    /// The edges and starting geometry of an interactive resize in progress.
    resize: Option<(ResizeEdges, Rectangle<i32, Logical>)>,
    maximized_horizontally: bool,
    maximized_vertically: bool,
    /// Shaded, which only an SSD window can be.
    shaded: bool,
    fullscreen: bool,
    /// The output a client asked to be fullscreen on.
    fullscreen_output: Option<OutputId>,
    minimized: bool,
    /// Kept above the windows of the normal layer (155).
    above: bool,
    /// Kept below them.
    below: bool,
    /// On every workspace, as Marco's `on_all_workspaces` (155). `workspace` is kept, as
    /// Marco keeps the window's own, though unsticking puts the window on the active one.
    sticky: bool,
    /// The half or quarter it's tiled to, on which output's work area (158), and how wide
    /// (159).
    tile: Option<Tiled>,
    /// The geometry to go back to, while maximised, tiled or fullscreen.
    floating: Option<Rectangle<i32, Logical>>,
    /// Refused the focus, or its client asked for attention, and not focused since (165,
    /// 166): Marco's `wm_state_demands_attention`.
    demands_attention: bool,
    /// `attach-modal-dialogs`, which the window's functions depend on (177).
    attach_modal_dialogs: bool,
    /// The Main output `apply` last saw the window on, to notice a drag across (230). It
    /// isn't the window's output, which is always its frame's: only a memo of it.
    main_output: Option<OutputId>,
    sent: Sent,
}

impl<W: PolicyWindow> Managed<W> {
    /// Re-reads the frame. A window that no longer has one can't stay shaded.
    fn read_frame(&mut self) {
        self.ssd = self.window.is_ssd();
        self.shaded &= self.ssd;
    }

    /// The frame's extents, or none when the window has no frame showing.
    fn visible_extents(&self) -> FrameExtents<i32, Logical> {
        if self.ssd && !self.fullscreen {
            self.window.frame_extents(self.frame_state())
        } else {
            FrameExtents::default()
        }
    }

    /// The frame's extents once unmaximised: the frame around the floating geometry.
    fn floating_extents(&self) -> FrameExtents<i32, Logical> {
        if self.ssd {
            self.window.frame_extents(FrameState {
                maximized: false,
                tiled_left: false,
                tiled_right: false,
                ..self.frame_state()
            })
        } else {
            FrameExtents::default()
        }
    }

    /// The window's outer rectangle: its committed geometry plus the frame, if it has one.
    /// A shaded window is only its frame's titlebar, which Marco's shaded styles keep the
    /// height of.
    fn frame(&self) -> Rectangle<i32, Logical> {
        self.frame_of(Rectangle::new(self.geometry.loc, self.size))
    }

    /// The outer rectangle the window would have at window geometry `geometry`.
    fn frame_of(&self, geometry: Rectangle<i32, Logical>) -> Rectangle<i32, Logical> {
        let e = self.visible_extents();
        let mut frame = frame_around(geometry, e);
        if self.shaded {
            frame.size.h = e.top;
        }
        frame
    }

    /// Moves the window geometry towards `loc`, its frame held back by `edges` from where
    /// policy last asked the window to be, as `unmaximize_for_drag` measures it (157).
    fn move_within(&mut self, loc: Point<i32, Logical>, edges: &Edges, snap: bool, by: By) {
        let current = self.geometry;
        let old_frame = self.frame_of(current);
        let proposed = self.frame_of(Rectangle::new(loc, current.size));
        let frame_loc = edges.for_move(old_frame, proposed, snap, by);
        self.geometry.loc = loc + (frame_loc - proposed.loc);
    }

    /// Resizes the window geometry from `start` towards `size` within its hints, its
    /// `moving` edges moving and the others staying, the moving ones held back by `edges`.
    fn resize_within(
        &mut self,
        start: Rectangle<i32, Logical>,
        moving: ResizeEdges,
        size: Size<i32, Logical>,
        edges: &Edges,
        snap: bool,
        by: By,
    ) {
        let before = self.geometry;
        let old_frame = self.frame_of(before);
        let hints = self.hints;
        let proposed = |size| {
            let size = hints.clamp(size);
            let mut loc = start.loc;
            if moving.left {
                loc.x = start.loc.x + start.size.w - size.w;
            }
            if moving.top {
                loc.y = start.loc.y + start.size.h - size.h;
            }
            Rectangle::new(loc, size)
        };
        let unresisted = proposed(size);
        let proposed_frame = self.frame_of(unresisted);
        let (w, h) = edges.for_resize(old_frame, proposed_frame, snap, by);
        let resisted = Size::new(
            (unresisted.size.w + w - proposed_frame.size.w).max(0),
            (unresisted.size.h + h - proposed_frame.size.h).max(0),
        );
        self.geometry = proposed(resisted);
        // A tile resizes by its inner sides alone, and keeps that size (159).
        if let Some(tiled) = &mut self.tile {
            if tiled.resizes_by(moving) {
                tiled.resized = true;
            } else {
                self.geometry = before;
            }
        }
    }
}

impl<W> Managed<W> {
    /// Maximised both ways, as Marco's `META_WINDOW_MAXIMIZED` has it.
    fn maximized(&self) -> bool {
        self.maximized_horizontally && self.maximized_vertically
    }

    /// Policy sets the size: maximised either way, tiled, or fullscreen.
    fn constrained(&self) -> bool {
        self.maximized_horizontally
            || self.maximized_vertically
            || self.tile.is_some()
            || self.fullscreen
    }

    fn tiled(&self) -> Option<Tile> {
        self.tile.map(|tiled| tiled.tile)
    }

    /// Undoes a grab's change to the ways the window is maximised, from its geometry
    /// `before`, and keeps the change the other way for when it's unmaximised: a window
    /// maximised one way moves and resizes only the other way, as in Marco.
    fn pin_maximized_axes(&mut self, before: Rectangle<i32, Logical>) {
        let geometry = &mut self.geometry;
        if self.maximized_horizontally {
            (geometry.loc.x, geometry.size.w) = (before.loc.x, before.size.w);
        }
        if self.maximized_vertically {
            (geometry.loc.y, geometry.size.h) = (before.loc.y, before.size.h);
        }
        if let Some(floating) = &mut self.floating {
            if !self.maximized_horizontally {
                (floating.loc.x, floating.size.w) = (geometry.loc.x, geometry.size.w);
            }
            if !self.maximized_vertically {
                (floating.loc.y, floating.size.h) = (geometry.loc.y, geometry.size.h);
            }
        }
    }

    /// Remembers the floating geometry before the window first leaves it.
    fn save_floating(&mut self) {
        if !self.constrained() && self.floating.is_none() {
            self.floating = Some(Rectangle::new(self.geometry.loc, self.size));
        }
    }

    fn states(&self) -> WindowStates {
        WindowStates {
            maximized: self.maximized(),
            fullscreen: self.fullscreen,
            shaded: self.shaded,
            resizing: self.resize.is_some(),
            tile: self.tiled(),
        }
    }

    /// What the window's functions are worked out from.
    fn function_facts(&self) -> FunctionFacts {
        FunctionFacts {
            window_type: self.window_type,
            hints: self.hints,
            has_parent: self.parent.is_some(),
            ssd: self.ssd,
            fullscreen: self.fullscreen,
        }
    }

    /// The window's functions (177).
    fn functions(&self) -> Functions {
        functions::functions(&self.function_facts(), self.attach_modal_dialogs)
    }

    /// The state the frame is asked for in. A fullscreen window shows no frame, so its
    /// frame is asked for only as it'll be out of fullscreen, with the functions it'll have
    /// then.
    fn frame_state(&self) -> FrameState {
        let facts = FunctionFacts {
            fullscreen: false,
            ..self.function_facts()
        };
        let functions = functions::functions(&facts, self.attach_modal_dialogs);
        self.states().frame_state(self.activated, functions)
    }
}

/// Where `place` put a new window.
struct Placed {
    /// At the pointer, by `pointer` or `manual` placement.
    by_pointer: bool,
    /// On this output, unless it went by a position of its own.
    on: Option<OutputId>,
}

/// How a new window with no position of its own is placed (154).
#[derive(Debug, Clone, Copy, PartialEq)]
enum NewPlacement {
    /// A dialog, centred over its parent.
    OverParent(WindowId),
    /// A dialog with no parent, or a splash screen, centred on the output.
    Centred,
    /// Centred on the pointer, by `pointer` or `manual` placement.
    AtPointer(Point<i32, Logical>),
    /// The first fit or a cascade, or the middle of the work area.
    Automatic,
}

/// A window's tile: which half or quarter of which output's work area (158), and how wide
/// (159).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Tiled {
    tile: Tile,
    output: OutputId,
    size: TileSize,
    /// Resized by its inner sides, so it keeps the geometry it was resized to rather than
    /// its share of the work area, as Marco's `tile_resized` has it (constraints.c:947-965).
    resized: bool,
}

impl Tiled {
    fn new(tile: Tile, output: OutputId, size: TileSize) -> Self {
        Self {
            tile,
            output,
            size,
            resized: false,
        }
    }

    /// Whether an interactive resize moving `edges` resizes the tile: only its inner sides
    /// move, those not along its work area's edges, alone or together, as Marco's
    /// `can_resize_tiled` has it (constraints.c:986-1060).
    fn resizes_by(self, edges: ResizeEdges) -> bool {
        let along = self.tile.edges();
        (edges.left || edges.right || edges.top || edges.bottom)
            && !(edges.left && along.left)
            && !(edges.right && along.right)
            && !(edges.top && along.top)
            && !(edges.bottom && along.bottom)
    }
}

/// A tile's width, as a share of its work area's: Marco's `MetaTileCycle`
/// (window-private.h:87-96), which the tiling keybindings step through with
/// `allow-tile-cycling` (159). A drag tiles a window `Uncycled`, Marco's
/// `META_TILE_CYCLE_NONE`, which is as wide as `Half`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TileSize {
    Uncycled,
    Half,
    Third,
    Quarter,
    Full,
    ThreeQuarters,
    TwoThirds,
}

impl TileSize {
    /// The share of the work area's width, as `meta_window_get_current_tile_area` has it
    /// (window.c:8238-8261).
    fn ratio(self) -> f64 {
        match self {
            TileSize::Uncycled | TileSize::Half => 1. / 2.,
            TileSize::Third => 1. / 3.,
            TileSize::Quarter => 1. / 4.,
            TileSize::Full => 1.,
            TileSize::ThreeQuarters => 3. / 4.,
            TileSize::TwoThirds => 2. / 3.,
        }
    }

    /// The size the next press of the same binding gives, or `None` for none, which
    /// untiles: 1/2, 1/3, 1/4, 1/1, 3/4, 2/3, then untiled, as `handle_toggle_tiled` has it
    /// (keybindings.c:3196-3225).
    fn next(self) -> Option<TileSize> {
        match self {
            TileSize::Uncycled => Some(TileSize::Half),
            TileSize::Half => Some(TileSize::Third),
            TileSize::Third => Some(TileSize::Quarter),
            TileSize::Quarter => Some(TileSize::Full),
            TileSize::Full => Some(TileSize::ThreeQuarters),
            TileSize::ThreeQuarters => Some(TileSize::TwoThirds),
            TileSize::TwoThirds => None,
        }
    }
}

/// The part of work area `area` that a window tiled to `tile` at `size` fills, frame and
/// all, as Marco's `meta_window_get_current_tile_area` has it (window.c:8205-8282): that
/// share of its width, rounded down, and for a corner half its height too.
fn tile_area(area: Rectangle<i32, Logical>, tile: Tile, size: TileSize) -> Rectangle<i32, Logical> {
    let edges = tile.edges();
    let mut frame = area;
    // In floating point as Marco's, so a third of an odd width rounds as there.
    frame.size.w = (f64::from(area.size.w) * size.ratio()) as i32;
    if !(edges.top && edges.bottom) {
        frame.size.h = area.size.h / 2;
    }
    if edges.right {
        frame.loc.x += area.size.w - frame.size.w;
    }
    if !edges.top {
        frame.loc.y += frame.size.h;
    }
    frame
}

/// The frame of `extents` around window geometry `geometry`.
fn frame_around(
    geometry: Rectangle<i32, Logical>,
    e: FrameExtents<i32, Logical>,
) -> Rectangle<i32, Logical> {
    Rectangle::new(
        (geometry.loc.x - e.left, geometry.loc.y - e.top).into(),
        (
            geometry.size.w + e.left + e.right,
            geometry.size.h + e.top + e.bottom,
        )
            .into(),
    )
}

/// What the Scene reads from policy (179): the answers to the queries
/// `caminito/src/scene.rs` makes, which `tests/invariants.rs` lists as `SCENE_QUERIES` and
/// holds to the Scene's source. A query the Scene adds gets its answer here too.
#[derive(Default, PartialEq)]
struct SceneReads {
    /// `active_stacking`.
    stacking: Vec<WindowId>,
    /// `stacking_all`.
    stack: Vec<WindowId>,
    /// Each window `window` has, with what's asked of it.
    windows: Vec<WindowReads>,
    /// Each seat's grab.
    grabs: Vec<GrabReads>,
    /// The entries of `switcher`, which the popup's thumbnails are of.
    switcher: Option<Vec<SwitchEntry>>,
}

/// What the Scene reads of one seat's grab.
#[derive(PartialEq)]
struct GrabReads {
    seat: SeatId,
    /// `grab`.
    window: WindowId,
    /// `tile_preview`.
    tile_preview: Option<Rectangle<i32, Logical>>,
}

/// What the Scene reads of one window.
#[derive(PartialEq)]
struct WindowReads {
    id: WindowId,
    /// `geometry`.
    geometry: Rectangle<i32, Logical>,
    /// `frame_rect`.
    frame: Rectangle<i32, Logical>,
    /// `is_showing`.
    showing: bool,
    /// `is_fullscreen`.
    fullscreen: bool,
    /// `layer`.
    layer: Option<Layer>,
}

pub struct Policy<W: PolicyWindow> {
    /// The settings, as `settings_changed` last gave them.
    config: Config,
    windows: BTreeMap<WindowId, Managed<W>>,
    /// Every window, bottom to top: one stack, as Marco keeps it, which each workspace's
    /// stacking is the windows on it of (ADR 0025).
    stack: Vec<WindowId>,
    /// How many workspaces there are, never 0.
    workspace_count: usize,
    /// Each output's active and previous workspace (ADR 0032), which every switch sets
    /// alike.
    active_workspaces: active_workspaces::ActiveWorkspaces,
    /// The way each switch went since `take_workspace_switches` last took them, for the
    /// compositor's switch sounds (170).
    switches: Vec<Direction>,
    /// Each output's switch since `take_output_switches` last took them, for the
    /// compositor's Workspace slide (233).
    output_switches: Vec<OutputSwitch>,
    /// The workspaces that show their desktop (94), as Marco's `showing_desktop` marks them
    /// (workspace.h:79).
    showing_desktop: BTreeMap<usize, ShownDesktop>,
    /// Each workspace's name, one per stack.
    workspace_names: Vec<String>,
    outputs: Vec<Output>,
    /// The user's primary output, or its stand-in (81), if the compositor has told of one.
    primary_output: Option<OutputId>,
    /// Every seat seen so far.
    seats: BTreeSet<SeatId>,
    focus: BTreeMap<SeatId, WindowId>,
    /// When each seat's focus last changed on user input, to drop stale focus requests as
    /// Marco does.
    focus_time: BTreeMap<SeatId, Duration>,
    /// The seat that last acted, which a newly mapped window is focused for.
    last_seat: SeatId,
    grabs: BTreeMap<SeatId, Grab>,
    /// Windows by when they last had focus, most recent first, for Alt+Tab.
    mru: Vec<WindowId>,
    /// The Window switcher (108), while it's open.
    switcher: Option<switcher::Open>,
    /// The window menu (161), while it's open.
    window_menu: Option<WindowMenu>,
    /// The Workspace popup (152), while it's open.
    workspace_popup: Option<WorkspacePopup>,
    /// The Layer surfaces that accept the keyboard, most recently used first (163).
    layers: Vec<LayerSurfaceId>,
    /// The one that has the keyboard in place of the focused window.
    keyboard_layer: Option<LayerSurfaceId>,
    /// What the Window switcher last chose, until the compositor takes it.
    chosen: Option<SwitchEntry>,
    /// Where the pointer was when the compositor last said, which `pointer` and `manual`
    /// placement put a new window at.
    pointer: Option<Point<f64, Logical>>,
    /// The pointer focus modes' and auto-raise's state (153).
    pointer_focus: pointer::PointerFocus,
    /// Whether `apply` waits for the end of a change made of several, which applies once.
    holding: bool,
    /// What the Scene read when `take_scene_changed` was last asked (179).
    last_scene_reads: SceneReads,
}

impl<W: PolicyWindow> Policy<W> {
    pub fn new(config: Config) -> Self {
        let mut policy = Self {
            config,
            windows: BTreeMap::new(),
            stack: Vec::new(),
            workspace_count: 1,
            active_workspaces: Default::default(),
            switches: Vec::new(),
            output_switches: Vec::new(),
            showing_desktop: BTreeMap::new(),
            workspace_names: Vec::new(),
            outputs: Vec::new(),
            primary_output: None,
            seats: BTreeSet::new(),
            focus: BTreeMap::new(),
            focus_time: BTreeMap::new(),
            last_seat: SeatId(0),
            grabs: BTreeMap::new(),
            mru: Vec::new(),
            switcher: None,
            window_menu: None,
            workspace_popup: None,
            layers: Vec::new(),
            keyboard_layer: None,
            chosen: None,
            pointer: None,
            pointer_focus: pointer::PointerFocus::default(),
            holding: false,
            last_scene_reads: SceneReads::default(),
        };
        policy.apply_config();
        policy
    }

    // Events.

    /// Adds an output, or updates the one with the same id.
    pub fn add_output(&mut self, output: Output) {
        let mut outputs = self.outputs.clone();
        match outputs.iter_mut().find(|o| o.id == output.id) {
            Some(existing) => *existing = output,
            None => outputs.push(output),
        }
        self.set_outputs(outputs);
    }

    /// Removes an output. Windows left on no output move onto the primary output.
    pub fn remove_output(&mut self, id: OutputId) {
        let mut outputs = self.outputs.clone();
        outputs.retain(|o| o.id != id);
        self.set_outputs(outputs);
    }

    /// Replaces every output at once, as the compositor lays them out again (51) or as what
    /// their edges reserve changes (64). Windows on an output that moved move with it,
    /// keeping their place on it; windows left on no output move onto the primary output.
    /// Maximised windows fill their output's new work area, and a floating window stays
    /// where it is, even under a new panel, as in Marco.
    pub fn set_outputs(&mut self, outputs: Vec<Output>) {
        let moves: Vec<(WindowId, Point<i32, Logical>)> = self
            .windows
            .iter()
            .filter_map(|(id, managed)| {
                let old = self.output_of(managed.frame())?;
                let new = outputs.iter().find(|o| o.id == old.id)?;
                let delta = new.geometry.loc - old.geometry.loc;
                (delta != Point::default()).then_some((*id, delta))
            })
            .collect();
        for (id, delta) in moves {
            let managed = self.windows.get_mut(&id).unwrap();
            managed.geometry.loc += delta;
            if let Some(floating) = &mut managed.floating {
                floating.loc += delta;
            }
        }
        self.active_workspaces.set_outputs(&outputs);
        self.outputs = outputs;
        self.rehome_windows();
        // A window an output change put on another output hasn't been dragged there.
        self.note_main_outputs();
        self.apply();
    }

    /// Names the primary output (81): where a new window opens with nothing better to go
    /// by, and where windows left on no output go. No window moves when it changes.
    pub fn set_primary_output(&mut self, primary: Option<OutputId>) {
        self.primary_output = primary;
    }

    /// The settings changed. A smaller workspace count moves the windows on the removed
    /// workspaces to the new last one, and the active workspace with them, as Marco's
    /// `update_num_workspaces` does; a larger one adds empty workspaces at the end. With
    /// `dynamic-workspaces` on, the count is policy's own and `num-workspaces` is left
    /// unread (151).
    pub fn settings_changed(&mut self, config: Config) {
        self.pointer_focus.settings_changed(&config);
        // A dialog that's attached or let go has another frame to be laid out with (177).
        let attach_modal_dialogs = config.attach_modal_dialogs;
        let attaches_changed = self.config.attach_modal_dialogs != attach_modal_dialogs;
        let spans = self.config.workspaces_per_output && !config.workspaces_per_output;
        self.config = config;
        if spans {
            self.span_workspaces();
        }
        if attaches_changed {
            let ids: Vec<WindowId> = self.windows.keys().copied().collect();
            for id in ids {
                self.windows.get_mut(&id).unwrap().attach_modal_dialogs = attach_modal_dialogs;
                self.relayout(id);
            }
        }
        self.apply_config();
    }

    /// Does what a new `config` asks for at once: the workspaces' count and names, and
    /// `apply()`.
    fn apply_config(&mut self) {
        if !self.config.dynamic_workspaces {
            self.set_workspace_count(self.config.workspace_count);
        }
        self.name_workspaces();
        // The active workspace may have been removed, so every seat's focus is checked.
        self.apply();
    }

    /// Makes `count` workspaces, within 1 to `MAX_WORKSPACES`, as `num-workspaces` asks.
    fn set_workspace_count(&mut self, count: usize) {
        let count = count.clamp(1, MAX_WORKSPACES);
        if count < self.workspace_count {
            let last = count - 1;
            // The stacking order is one across workspaces, which moving leaves alone, as in
            // Marco.
            for managed in self.windows.values_mut() {
                managed.workspace = managed.workspace.min(last);
            }
            // In the grid as it was, which had the workspace. The sound is one for every
            // output's switch, from the primary output's, which stands in for the Active
            // output's (231).
            let from = self.primary_shown().active;
            self.note_switch(self.workspace_grid(), from, from.min(last));
            self.workspace_count = count;
            for shown in self.active_workspaces.each_mut() {
                shown.active = shown.active.min(last);
            }
            self.forget_removed_previous();
            self.showing_desktop
                .retain(|workspace, _| *workspace < count);
        } else {
            self.workspace_count = count;
        }
    }

    /// Forgets the previous workspace once it's no longer there, or it's the active one
    /// since the count changed. `switch-to-workspace-prev` then does nothing until the
    /// next switch. Marco keeps a pointer to the removed workspace.
    fn forget_removed_previous(&mut self) {
        let count = self.workspace_count;
        for shown in self.active_workspaces.each_mut() {
            let active = shown.active;
            shown.previous = shown
                .previous
                .filter(|previous| *previous < count && *previous != active);
        }
    }

    /// Gives each workspace its name from the settings, or Marco's default. A name is the
    /// number's, not the workspace's, as Marco keeps them (`meta_prefs_get_workspace_name`,
    /// prefs.c:2257-2268).
    fn name_workspaces(&mut self) {
        self.workspace_names = (0..self.workspace_count)
            .map(|index| {
                self.config
                    .workspace_names
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| default_workspace_name(index + 1))
            })
            .collect();
    }

    /// Where the pointer is, which `pointer` and `manual` placement put a new window at. The
    /// compositor says before it maps a window.
    pub fn set_pointer(&mut self, pointer: Point<f64, Logical>) {
        self.pointer = Some(pointer);
    }

    /// Starts managing a window the client has mapped.
    pub fn map_window(&mut self, window: W, now: Duration) {
        self.map_window_with(window, MapStates::default(), now);
    }

    /// Starts managing a window the client has mapped with `states`, applied as one. A
    /// window that maps maximised or fullscreen keeps the automatic placement, for the
    /// geometry it goes back to, whatever the placement keys say (154). The others are set
    /// once it's placed and focused, and only those it asked for: a transient of a sticky
    /// window is sticky with it unasked. Asking to be active is judged for the seat it's
    /// focused for, at `now`.
    pub fn map_window_with(&mut self, window: W, states: MapStates, now: Duration) {
        let id = window.id();
        self.holding = true;
        let placed_on = self.map(window, states, now);
        // The setters refuse a state the window has no function for (177).
        if let Some(output) = states.fullscreen {
            self.set_fullscreen(id, true, output);
        } else if states.maximized {
            self.set_maximized(id, true);
        }
        // On the output it was placed on, as it was configured before it mapped (190),
        // though the frame it drew may be more on another. Fullscreen on an output policy
        // has goes there.
        if let Some(output) = placed_on {
            let named = self.windows[&id].fullscreen_output;
            let named = self.output_named(named).map(|named| named.id);
            self.relayout_on(id, named.or(Some(output)));
        }
        if states.above {
            self.set_above(id, true);
        }
        if states.below {
            self.set_below(id, true);
        }
        if states.sticky {
            self.set_sticky(id, true);
        }
        // Not unset: policy has made the demand itself for a window it refused the focus.
        if states.demands_attention {
            self.set_demands_attention(id, true);
        }
        if let Some(source) = states.active {
            self.request_activation_from(self.last_seat, id, source, now);
        }
        self.holding = false;
        self.apply();
    }

    /// Returns the output a window that maps maximised or fullscreen was placed on, unless
    /// it went by a position of its own.
    fn map(&mut self, window: W, states: MapStates, now: Duration) -> Option<OutputId> {
        let id = window.id();
        let size = window.size();
        let geometry = Rectangle::new(Point::default(), size);
        let window_type = window.window_type();
        let hints = window.size_hints();
        let ssd = window.is_ssd();
        let parent = window
            .transient_parent()
            .filter(|p| self.windows.contains_key(p));
        // Marco honours `_NET_WM_DESKTOP` for a desktop it has. A transient goes with its
        // parent, sticky if it is, since policy keeps a transient family on one workspace,
        // or on all of them (window.c:747-768).
        let (workspace, sticky) = match parent {
            Some(parent) => {
                let parent = &self.windows[&parent];
                (parent.workspace, parent.sticky)
            }
            None => (
                window
                    .requested_workspace()
                    .filter(|workspace| *workspace < self.workspace_count)
                    .unwrap_or(self.new_window_workspace()),
                false,
            ),
        };
        self.windows.insert(
            id,
            Managed {
                window,
                window_type,
                parent,
                workspace,
                geometry,
                size,
                hints,
                ssd,
                activated: false,
                resize: None,
                maximized_horizontally: false,
                maximized_vertically: false,
                shaded: false,
                fullscreen: false,
                fullscreen_output: None,
                minimized: false,
                above: false,
                below: false,
                sticky,
                tile: None,
                floating: None,
                demands_attention: false,
                attach_modal_dialogs: self.config.attach_modal_dialogs,
                main_output: None,
                sent: Sent::default(),
            },
        );
        self.stack.push(id);
        // Before it's placed, which is among the windows that show.
        self.leave_show_desktop(id);
        // A state the window has no function for is one it won't map with (177).
        let functions = self.windows[&id].functions();
        let constrained = (states.maximized && functions.maximize)
            || (states.fullscreen.is_some() && functions.fullscreen);
        let Placed { by_pointer, on } = self.place(id, !constrained);
        // Marco's pointer placement skips its maximising of a window too big for its work
        // area (place.c:898-905).
        if !by_pointer {
            self.maximize_if_too_big(id);
        }
        self.adopt_transients(id);
        // While the Window switcher is open the focus stays where it is, and the window gets
        // it when the switcher closes, unless `focus-new-windows` refuses it now.
        if self.focuses_on_map(id) && !self.claim_gives_focus(id, states.focus, now) {
            self.refuse_focus(id);
        } else if self.switcher.is_some() {
            self.switcher_defer_focus(id, now);
        } else {
            self.focus_mapped(id, now);
        }
        if by_pointer && self.config.placement_mode == PlacementMode::Manual {
            self.begin_manual_placement(id);
        }
        self.apply();
        on.filter(|_| constrained)
    }

    /// Has the last seat move a new window with its pointer until it's put down, as Marco's
    /// `move_after_placement` begins a keyboard move (window.c:2329-2334). Like Marco's
    /// `meta_window_begin_grab_op`, it doesn't start while the seat has an Interaction.
    fn begin_manual_placement(&mut self, id: WindowId) {
        let (seat, Some(pointer)) = (self.last_seat, self.pointer) else {
            return;
        };
        if self.interaction(seat).is_some() || !self.begin_grab(seat, id, None, pointer, false) {
            return;
        }
        let grab = self.grabs.get_mut(&seat).unwrap();
        grab.keyboard = true;
        grab.placing = true;
    }

    /// Focuses a window that has just mapped, as Marco's `window_state_on_map`
    /// (window.c:1997-2044) does. A window mapped on another workspace isn't shown, so it
    /// isn't focused either.
    fn focus_mapped(&mut self, id: WindowId, now: Duration) {
        if self.focuses_on_map(id) {
            self.focus_window(self.last_seat, id, now);
        }
    }

    /// Whether a window that has just mapped is one Marco's `window_state_on_map` focuses.
    fn focuses_on_map(&self, id: WindowId) -> bool {
        self.windows.get(&id).is_some_and(|managed| {
            managed.window.takes_input()
                && self.is_on_shown_workspace(id)
                && !matches!(
                    managed.window_type,
                    WindowType::Desktop
                        | WindowType::Dock
                        | WindowType::Splashscreen
                        | WindowType::Menu
                        | WindowType::Utility
                        | WindowType::Toolbar
                )
        })
    }

    /// Stops managing a window the client has unmapped or destroyed, and hands it back.
    pub fn unmap_window(&mut self, id: WindowId) -> Option<W> {
        let managed = self.windows.remove(&id)?;
        self.stack.retain(|w| *w != id);
        self.grabs.retain(|_, grab| grab.window != id);
        for other in self.windows.values_mut() {
            if other.parent == Some(id) {
                other.parent = None;
            }
        }
        // Marco hands focus to a lost transient's parent before the rest of the stack.
        if let Some(parent) = managed.parent.filter(|p| self.is_showing(*p)) {
            let parent = self.modal_transient(parent);
            for focus in self.focus.values_mut() {
                if *focus == id {
                    *focus = parent;
                }
            }
        }
        self.switcher_window_unmapped(id);
        self.apply();
        Some(managed.window)
    }

    /// The client committed new state: re-reads its facts.
    pub fn commit(&mut self, id: WindowId) {
        let Some(managed) = self.windows.get_mut(&id) else {
            return;
        };
        managed.window_type = managed.window.window_type();
        managed.hints = managed.window.size_hints();
        managed.read_frame();
        let size = managed.window.size();
        managed.size = size;
        if let Some((edges, start)) = managed.resize {
            // The client may settle on another size than asked; the far edges stay put.
            let before = managed.geometry;
            if edges.left {
                managed.geometry.loc.x = start.loc.x + start.size.w - size.w;
            }
            if edges.top {
                managed.geometry.loc.y = start.loc.y + start.size.h - size.h;
            }
            managed.pin_maximized_axes(before);
        } else if !managed.constrained() {
            // A floating client picks its own size, and already knows it.
            managed.geometry.size = size;
            if let Some((sent, _)) = &mut managed.sent.geometry {
                sent.size = size;
            }
        }

        let requested = managed.window.transient_parent();
        if requested != managed.parent {
            let parent =
                requested.filter(|p| self.windows.contains_key(p) && !self.is_ancestor(id, *p));
            self.windows.get_mut(&id).unwrap().parent = parent;
            if let Some(parent) = parent {
                self.join_parent(id, parent);
            }
        }
        self.relayout(id);
        self.apply();
    }

    /// The window's frame changed without a commit, such as when the theme or its decoration
    /// mode did: re-reads it, and lays the window out again.
    pub fn frame_changed(&mut self, id: WindowId) {
        let Some(managed) = self.windows.get_mut(&id) else {
            return;
        };
        managed.read_frame();
        self.relayout(id);
        self.apply();
    }

    /// A button press on a window, its body or its Frame: click focus, and Marco's raise on
    /// click, which `raise-on-click` turns off (153). Marco raises a dock but doesn't
    /// focus it, since panels ask for focus themselves (display.c:1923-1946). With the key
    /// off, a press that starts a move or resize, or the titlebar's, raises the window if it
    /// ends as a click: see `end_grab` and `titlebar_released`.
    pub fn click(&mut self, seat: SeatId, id: WindowId, now: Duration) {
        self.set_last_seat(seat);
        if self.is_showing(id) {
            let dock = self.windows[&id].window_type == WindowType::Dock;
            if (dock || self.focus_window(seat, id, now)) && self.config.raise_on_click {
                self.raise_family(id);
            }
        }
        self.apply();
    }

    /// A press on a window's titlebar ended without dragging it. With `raise-on-click` off
    /// that raises the window, as Marco raises one it was about to move when the pointer
    /// hardly moved (display.c:3857-3870); with it on, the press raised it already (153).
    pub fn titlebar_released(&mut self, id: WindowId) {
        if !self.config.raise_on_click && self.is_showing(id) {
            self.raise_family(id);
        }
        self.apply();
    }

    /// A keybinding action on behalf of `seat`.
    pub fn action(&mut self, seat: SeatId, action: Action, now: Duration) {
        self.set_last_seat(seat);
        match action {
            Action::SwitchToWorkspaceLeft => self.switch_workspace_towards(Direction::Left),
            Action::SwitchToWorkspaceRight => self.switch_workspace_towards(Direction::Right),
            Action::SwitchToWorkspaceUp => self.switch_workspace_towards(Direction::Up),
            Action::SwitchToWorkspaceDown => self.switch_workspace_towards(Direction::Down),
            Action::SwitchToWorkspacePrev => {
                // The primary output's previous workspace, where the Active output's is
                // taken (231).
                if let Some(previous) = self.primary_shown().previous {
                    self.switch_workspace(previous);
                }
            }
            Action::MoveToWorkspaceLeft => {
                self.move_to_workspace_towards(seat, Direction::Left, now);
            }
            Action::MoveToWorkspaceRight => {
                self.move_to_workspace_towards(seat, Direction::Right, now);
            }
            Action::MoveToWorkspaceUp => self.move_to_workspace_towards(seat, Direction::Up, now),
            Action::MoveToWorkspaceDown => {
                self.move_to_workspace_towards(seat, Direction::Down, now);
            }
            // A number past the count does nothing (`handle_switch_to_workspace`).
            Action::SwitchToWorkspace(workspace) if workspace < self.workspace_count => {
                self.switch_workspace(workspace);
            }
            Action::SwitchToWorkspace(_) => {}
            Action::Switch { list, backward } => self.switch(seat, list, backward, true, now),
            Action::Cycle { list, backward } => self.switch(seat, list, backward, false, now),
            Action::ShowDesktop => self.set_showing_desktop(!self.is_showing_desktop()),
            Action::ActivateWindowMenu => {
                if let Some(id) = self.focus(seat) {
                    self.open_window_menu(seat, id);
                }
            }
            Action::BeginMove | Action::BeginResize => {
                if let Some(id) = self.focus(seat) {
                    self.begin_keyboard_grab(seat, id, action == Action::BeginResize);
                }
            }
            action => {
                if let Some(id) = self.focus(seat) {
                    self.window_action(id, action);
                }
            }
        }
        self.apply();
    }

    /// Shows the desktop on every output's active workspace, or stops, as Marco's
    /// `show-desktop` and `_NET_SHOWING_DESKTOP` do. Showing it hides the workspace's windows, but for the
    /// desktop, docks and their transients, without minimising them, and focuses the
    /// desktop if it's a window (`meta_screen_show_desktop`, screen.c:2850-2882). Stopping
    /// brings them back as they were stacked, each seat with the focus it had. Marco gives
    /// the default focus there (`handle_show_desktop`, keybindings.c:2902-2917), which is
    /// the topmost window and so another one if the focused window wasn't on top; 94 asks
    /// for the focus they had. It also ends, another way, when a window is mapped or
    /// activated on the workspace: see `leave_show_desktop`.
    pub fn set_showing_desktop(&mut self, showing: bool) {
        if showing == self.is_showing_desktop() {
            return;
        }
        if showing {
            let shown = ShownDesktop {
                focus: self.focus.clone(),
                skip_list: self.config.show_desktop_skip_list.clone(),
            };
            for workspace in self.shown_workspaces() {
                self.showing_desktop.insert(workspace, shown.clone());
            }
            // Marco focuses the most recently used desktop window. One that never had the
            // focus isn't in `mru`, so the stack is looked through after it.
            let top_down = self.active_stacking().into_iter().rev();
            let desktop = self.mru.iter().copied().chain(top_down).find(|id| {
                self.windows[id].window_type == WindowType::Desktop && self.is_showing(*id)
            });
            if let Some(desktop) = desktop {
                for seat in &self.seats {
                    // A seat on a listed window keeps it, which Marco's focusing of the
                    // desktop takes away.
                    if !self.focus.get(seat).is_some_and(|id| self.is_skipped(*id)) {
                        self.focus.insert(*seat, desktop);
                    }
                }
            }
        } else {
            // Each seat's focus as the first of the workspaces had it.
            let mut before = BTreeMap::new();
            for workspace in self.shown_workspaces() {
                if let Some(shown) = self.showing_desktop.remove(&workspace) {
                    for (seat, id) in shown.focus {
                        before.entry(seat).or_insert(id);
                    }
                }
            }
            let default = self.default_focus();
            for seat in &self.seats {
                let had = before.get(seat).copied().filter(|id| self.is_showing(*id));
                if let Some(id) = had.or(default) {
                    self.focus.insert(*seat, id);
                }
            }
        }
        // The seats that focused a window that's hidden now lose it there.
        self.apply();
    }

    /// The workspace beside the active one in the grid, where `wrap-style` takes a move past
    /// its edge, if the move goes anywhere. It's beside the primary output's, where the
    /// Active output's is taken (231).
    fn workspace_towards(&self, direction: Direction) -> Option<usize> {
        let active = self.primary_shown().active;
        self.workspace_grid()
            .neighbour(active, direction, self.config.wrap_style)
    }

    /// Switches to the workspace beside the active one in the grid, if there is one.
    fn switch_workspace_towards(&mut self, direction: Direction) {
        if let Some(workspace) = self.workspace_towards(direction) {
            self.switch_shown(None, workspace, Some(direction));
        }
    }

    /// Moves `seat`'s focused window to the workspace beside the active one, with its
    /// transient family, and switches there with the window focused and raised, as Marco's
    /// `handle_workspace_switch_or_move` does (keybindings.c:3560-3576). Marco gives the
    /// desktop and docks no such move: they're on every workspace.
    fn move_to_workspace_towards(&mut self, seat: SeatId, direction: Direction, now: Duration) {
        let Some(id) = self.focus(seat).filter(|id| !self.is_desktop_or_dock(*id)) else {
            return;
        };
        let Some(workspace) = self.workspace_towards(direction) else {
            return;
        };
        self.move_tree(self.root(id), workspace);
        self.switch_shown(None, workspace, Some(direction));
        if self.focus_window(seat, id, now) {
            self.raise_family(id);
        }
    }

    /// Makes `workspace` the active one of every output, and gives every seat its default
    /// focus. Every switch there is switches every output, until the bindings switch the
    /// Active output's (231).
    pub fn switch_workspace(&mut self, workspace: usize) {
        self.switch_shown(None, workspace, None);
    }

    /// Makes `workspace` `output`'s active one, and leaves the other outputs as they are,
    /// with `workspaces-per-output` on (230). Off, it switches every output, as
    /// `switch_workspace` does. An output policy doesn't have is left alone.
    pub fn switch_output_workspace(&mut self, output: OutputId, workspace: usize) {
        if !self.outputs.iter().any(|o| o.id == output) {
            return;
        }
        let only = self.config.workspaces_per_output.then_some(output);
        self.switch_shown(only, workspace, None);
    }

    /// Switches `only`'s active workspace to `workspace`, or every output's for none.
    ///
    /// Switching every output gives every seat the default focus, as Marco's
    /// `meta_workspace_activate_with_focus` does. Switching one gives it to the seats whose
    /// focused window is on that output, from that output's windows, and leaves the other
    /// seats theirs.
    ///
    /// `towards` is the way a binding asked to go, if one did: the Workspace slide then
    /// goes that way, also where the switch wraps past the grid's edge (233).
    fn switch_shown(
        &mut self,
        only: Option<OutputId>,
        workspace: usize,
        towards: Option<Direction>,
    ) {
        let workspace = workspace.min(self.workspace_count - 1);
        // The sound is one for the switch, from the output's workspace, or the primary
        // output's for every output, which stands in for the Active output's (231).
        let from = match only {
            Some(output) => self.active_workspaces.of(Some(output)).active,
            None => self.primary_shown().active,
        };
        let switches = match only {
            Some(_) => from != workspace,
            None => self
                .active_workspaces
                .each()
                .any(|shown| shown.active != workspace),
        };
        if switches {
            // The workspace changing under an open switcher closes it (ruling 9 of 108).
            self.close_switcher(switcher::Close::Dropped);
            // And closes the window menu, whose moves are from the active workspace (161).
            self.window_menu = None;
            self.note_switch(self.workspace_grid(), from, workspace);
            self.note_output_switches(only, workspace, towards);
            // A window being moved goes with the switch, with its transient family, as
            // Marco's `meta_workspace_activate_with_focus` takes the grab's window along
            // unless it's on every workspace (workspace.c:416-456): it stays under the
            // pointer, and the Workspace slide leaves it still (234).
            let moved: Vec<WindowId> = self
                .grabs
                .values()
                .filter(|grab| grab.resize.is_none())
                .map(|grab| grab.window)
                .filter(|id| only.is_none_or(|only| self.output_showing(*id) == Some(only)))
                .filter(|id| !self.is_on_workspace(*id, workspace))
                .collect();
            for id in moved {
                self.move_tree(self.root(id), workspace);
            }
            let switch = |shown: &mut active_workspaces::Shown| {
                if shown.active != workspace {
                    shown.previous = Some(shown.active);
                    shown.active = workspace;
                }
            };
            match only {
                None => {
                    self.active_workspaces.each_mut().for_each(switch);
                    self.focus.clear();
                    if let Some(id) = self.default_focus() {
                        for seat in &self.seats {
                            self.focus.insert(*seat, id);
                        }
                    }
                }
                Some(output) => {
                    self.active_workspaces.of_mut(output).map(switch);
                    let on_output = |policy: &Self, id: WindowId| {
                        policy
                            .output_of(policy.windows[&id].frame())
                            .is_some_and(|o| o.id == output)
                    };
                    let seats: Vec<SeatId> = self
                        .focus
                        .iter()
                        .filter(|(_, id)| on_output(self, **id))
                        .map(|(seat, _)| *seat)
                        .collect();
                    let default = self.default_focus_where(|id| on_output(self, id));
                    for seat in seats {
                        match default {
                            Some(id) => self.focus.insert(seat, id),
                            None => self.focus.remove(&seat),
                        };
                    }
                }
            }
        }
        self.apply();
    }

    /// Notes the way a switch from `from` to `to` goes in `grid`, by their cells, as
    /// Marco's `workspace_switch_sound` has it (workspace.c:310-362): left or right where
    /// the columns differ, and up or down only within a column. A switch that wraps
    /// past the grid's edge so goes the other way.
    pub(super) fn note_switch(&mut self, grid: Grid, from: usize, to: usize) {
        let (from_column, from_row) = grid.cell(from);
        let (column, row) = grid.cell(to);
        let direction = match (column.cmp(&from_column), row.cmp(&from_row)) {
            (Ordering::Less, _) => Direction::Left,
            (Ordering::Greater, _) => Direction::Right,
            (Ordering::Equal, Ordering::Less) => Direction::Up,
            (Ordering::Equal, Ordering::Greater) => Direction::Down,
            (Ordering::Equal, Ordering::Equal) => return,
        };
        self.switches.push(direction);
    }

    /// The way each switch of the active workspace went since this was last called, the
    /// oldest first, however the switch came (170).
    pub fn take_workspace_switches(&mut self) -> Vec<Direction> {
        std::mem::take(&mut self.switches)
    }

    /// Each output's change of active workspace since this was last called, the oldest
    /// first, however the switch came (233).
    pub fn take_output_switches(&mut self) -> Vec<OutputSwitch> {
        std::mem::take(&mut self.output_switches)
    }

    /// Moves a window to `workspace`, with every window in its transient tree. A sticky
    /// window stops being sticky, as in Marco's `meta_window_change_workspace`
    /// (window.c:4515-4554).
    pub fn move_to_workspace(&mut self, id: WindowId, workspace: usize) {
        if !self.windows.contains_key(&id) {
            return;
        }
        let workspace = workspace.min(self.workspace_count - 1);
        self.move_tree(self.root(id), workspace);
        self.apply();
    }

    /// Brings a window to the user, as the window list or Alt+Tab does: switches to its
    /// workspace, then focuses and raises it.
    pub fn activate(&mut self, seat: SeatId, id: WindowId, now: Duration) {
        let Some(workspace) = self.workspace_of(id) else {
            return;
        };
        self.set_last_seat(seat);
        // A sticky window is on the active workspace already, which Marco's
        // `meta_window_activate` then stays on (window.c:3246-3258).
        if !self.is_on_shown_workspace(id) {
            self.switch_workspace(workspace);
        }
        // Marco drops an activation older than the user's last input before it looks at
        // the desktop (window.c:3179-3201), so a stale one hides nothing.
        if !self.focus_time.get(&seat).is_some_and(|last| now < *last) {
            self.leave_show_desktop(id);
        }
        let mut current = Some(id);
        while let Some(w) = current {
            let managed = self.windows.get_mut(&w).unwrap();
            managed.minimized = false;
            current = managed.parent;
        }
        if self.focus_window(seat, id, now) {
            self.raise_family(id);
        }
        self.apply();
    }

    // Client requests.

    /// Maximises the window both ways, or unmaximises it both ways.
    pub fn set_maximized(&mut self, id: WindowId, maximized: bool) {
        self.set_maximized_axes(id, Some(maximized), Some(maximized));
    }

    pub fn set_maximized_horizontally(&mut self, id: WindowId, maximized: bool) {
        self.set_maximized_axes(id, Some(maximized), None);
    }

    pub fn set_maximized_vertically(&mut self, id: WindowId, maximized: bool) {
        self.set_maximized_axes(id, None, Some(maximized));
    }

    /// Sets the ways the window is maximised, leaving `None`s as they are. Maximising
    /// vertically unshades, as in Marco's `meta_window_maximize`.
    ///
    /// A window is maximised both ways only with the maximise function, as Marco's
    /// `handle_maximize` and its `_NET_WM_STATE` handler ask (keybindings.c:3275-3301,
    /// window.c:5205-5254), and one way with the resize function, as
    /// `handle_maximize_vertically` and `_horizontally` ask (keybindings.c:2529-2559).
    /// Unmaximising is never refused (177).
    fn set_maximized_axes(
        &mut self,
        id: WindowId,
        horizontally: Option<bool>,
        vertically: Option<bool>,
    ) {
        let Some(managed) = self.windows.get_mut(&id) else {
            return;
        };
        let functions = managed.functions();
        let allowed = if horizontally.is_some() && vertically.is_some() {
            functions.maximize
        } else {
            functions.resize()
        };
        let maximizes = (horizontally == Some(true) && !managed.maximized_horizontally)
            || (vertically == Some(true) && !managed.maximized_vertically);
        if maximizes && !allowed {
            return;
        }
        let horizontally = horizontally.unwrap_or(managed.maximized_horizontally);
        let vertically = vertically.unwrap_or(managed.maximized_vertically);
        // Maximising or unmaximising either way untiles, keeping the floating geometry, as
        // Marco's `meta_window_maximize` and `meta_window_unmaximize` do (158).
        if managed.tile.is_some()
            || (managed.maximized_horizontally, managed.maximized_vertically)
                != (horizontally, vertically)
        {
            managed.save_floating();
            managed.tile = None;
            if vertically && !managed.maximized_vertically {
                managed.shaded = false;
            }
            managed.maximized_horizontally = horizontally;
            managed.maximized_vertically = vertically;
            self.relayout(id);
        }
        self.apply();
    }

    /// Shades a window that has the shade function, or unshades one. A CSD window has
    /// none: Caminito doesn't know where its titlebar is (07). Nor has one whose frame is a
    /// border only, or a fullscreen one (177).
    pub fn set_shaded(&mut self, id: WindowId, shaded: bool) {
        if let Some(managed) = self.windows.get_mut(&id)
            && (!shaded || managed.functions().shade)
        {
            managed.shaded = shaded;
        }
        self.apply();
    }

    /// Enters fullscreen, on `output` if the client named one, or leaves it. A window
    /// enters it only with the fullscreen function (177).
    pub fn set_fullscreen(&mut self, id: WindowId, fullscreen: bool, output: Option<OutputId>) {
        let Some(managed) = self.windows.get_mut(&id) else {
            return;
        };
        if fullscreen && !managed.fullscreen && !managed.functions().fullscreen {
            return;
        }
        managed.save_floating();
        managed.fullscreen = fullscreen;
        // Marco unshades a window it makes fullscreen.
        managed.shaded &= !fullscreen;
        managed.fullscreen_output = output.filter(|_| fullscreen);
        self.relayout(id);
        self.apply();
    }

    /// The client asks to move its window to `loc` or resize it to `size`, as an X11
    /// `ConfigureRequest` does. It's refused while the window is maximised either way,
    /// tiled (158), fullscreen, or being moved or resized, as Marco ignores one during a grab
    /// (window.c:4866-4885). Otherwise the size is kept within the hints, and the frame
    /// shrunk and moved inside the work area, as Marco's constraints do for a configure
    /// request (constraints.c:1456-1479).
    pub fn request_geometry(
        &mut self,
        id: WindowId,
        loc: Option<Point<i32, Logical>>,
        size: Option<Size<i32, Logical>>,
    ) {
        let grabbed = self.grabs.values().any(|grab| grab.window == id);
        let Some(managed) = self.windows.get_mut(&id) else {
            return;
        };
        if grabbed || managed.constrained() || (loc.is_none() && size.is_none()) {
            return;
        }
        if let Some(size) = size {
            managed.geometry.size = managed.hints.clamp(size);
        }
        if let Some(loc) = loc {
            managed.geometry.loc = loc;
        }
        let e = managed.visible_extents();
        let frame = frame_around(managed.geometry, e);
        if let Some(output) = self.output_of(frame).or(self.primary_output()) {
            let area = Self::work_area(output);
            let managed = self.windows.get_mut(&id).unwrap();
            let size = &mut managed.geometry.size;
            size.w = size.w.min(area.size.w - e.left - e.right).max(1);
            size.h = size.h.min(area.size.h - e.top - e.bottom).max(1);
            let frame_loc = placement::constrain(frame_around(managed.geometry, e), area);
            managed.geometry.loc = Point::new(frame_loc.x + e.left, frame_loc.y + e.top);
        }
        self.apply();
    }

    /// Keeps the window above the normal layer, or stops, and raises it either way, as
    /// Marco's `meta_window_make_above` and `meta_window_unmake_above` do
    /// (window.c:2951-2971).
    pub fn set_above(&mut self, id: WindowId, above: bool) {
        let Some(managed) = self.windows.get_mut(&id) else {
            return;
        };
        if managed.above != above {
            managed.above = above;
            self.raise_family(id);
        }
        self.apply();
    }

    /// Keeps the window below the normal layer, or stops. Marco only changes its layer.
    pub fn set_below(&mut self, id: WindowId, below: bool) {
        if let Some(managed) = self.windows.get_mut(&id) {
            managed.below = below;
        }
        self.apply();
    }

    /// Shows the window on every workspace, or only on one, with its transient family, as
    /// Marco's `meta_window_stick` and `meta_window_unstick` take its transients along
    /// (window.c:4646-4666). Policy keeps a family on one workspace or on all, so the
    /// whole family goes, from its root. Unsticking leaves it on its root's output's active
    /// workspace, where it's showing, as Marco's `window_unstick_impl` does (window.c:4615-4624).
    pub fn set_sticky(&mut self, id: WindowId, sticky: bool) {
        if !self.windows.contains_key(&id) {
            return;
        }
        let active = self.active_workspace_of(self.root(id));
        for w in self.tree(self.root(id)) {
            let managed = self.windows.get_mut(&w).unwrap();
            if managed.sticky && !sticky {
                managed.workspace = active;
            }
            managed.sticky = sticky;
        }
        self.apply();
    }

    /// Minimises a window that has the minimise function (177).
    pub fn minimize(&mut self, id: WindowId) {
        if let Some(managed) = self.windows.get_mut(&id)
            && managed.functions().minimize
        {
            managed.minimized = true;
        }
        self.apply();
    }

    /// Raises a window, as Marco's titlebar `raise` action does, without focusing it.
    pub fn raise(&mut self, id: WindowId) {
        if self.windows.contains_key(&id) {
            self.raise_family(id);
        }
        self.apply();
    }

    /// Asks a window to close. Marco gives the desktop and docks no close function.
    pub fn close(&mut self, id: WindowId) {
        if self.windows.contains_key(&id) && !self.is_desktop_or_dock(id) {
            self.windows.get_mut(&id).unwrap().window.close();
        }
    }

    /// Lowers a window to the bottom of its layer, as Marco's titlebar middle-click
    /// (`meta_core_user_lower_and_unfocus`) does: it goes last in the Alt+Tab order, and
    /// if the seat focused it, the seat focuses the default window instead.
    pub fn lower(&mut self, seat: SeatId, id: WindowId, now: Duration) {
        if !self.windows.contains_key(&id) {
            return;
        }
        self.set_last_seat(seat);
        self.stack.retain(|w| *w != id);
        self.stack.insert(0, id);
        self.mru.retain(|w| *w != id);
        self.mru.push(id);
        if self.focus(seat) == Some(id) {
            // Restacked first, so the default window is the one now on top.
            self.apply();
            if let Some(next) = self.default_focus().filter(|next| *next != id) {
                self.focus_window(seat, next, now);
            }
        }
        self.apply();
    }

    // Interactive move and resize, driven by the caller.

    /// Starts moving a window with `seat`'s pointer. Returns false if the window can't be moved.
    pub fn begin_move(&mut self, seat: SeatId, id: WindowId, pointer: Point<f64, Logical>) -> bool {
        self.begin_grab(seat, id, None, pointer, false)
    }

    /// Starts resizing a window's `edges` with `seat`'s pointer. Returns false if the window
    /// can't be resized.
    pub fn begin_resize(
        &mut self,
        seat: SeatId,
        id: WindowId,
        edges: ResizeEdges,
        pointer: Point<f64, Logical>,
    ) -> bool {
        self.begin_grab(seat, id, Some(edges), pointer, false)
    }

    /// Starts a move the window's client asked for, as one that draws its own titlebar does
    /// for a press on it. Such a press is the client's titlebar drag, so a maximised window
    /// is moved too: `grab_motion` holds it until the pointer shakes it loose, since the
    /// client may ask at the press, before anything says it's a drag (144). Returns false
    /// if the window can't be moved.
    pub fn begin_requested_move(
        &mut self,
        seat: SeatId,
        id: WindowId,
        pointer: Point<f64, Logical>,
    ) -> bool {
        self.begin_grab(seat, id, None, pointer, true)
    }

    /// Starts moving a window from a titlebar drag that's crossed the drag threshold, with
    /// `seat`'s pointer at `at_press` (where the titlebar was first pressed) and now at
    /// `pointer`. If the window is maximised either way, unmaximises it first, keeping
    /// `pointer`'s position across the titlebar's width the same fraction it was across the
    /// maximised one, as Marco and KDE Plasma's drag-to-unmaximise both do (144). Returns
    /// false if the window can't be moved.
    pub fn begin_titlebar_move(
        &mut self,
        seat: SeatId,
        id: WindowId,
        at_press: Point<f64, Logical>,
        pointer: Point<f64, Logical>,
    ) -> bool {
        if self
            .windows
            .get(&id)
            .is_some_and(|m| m.maximized_horizontally || m.maximized_vertically)
            || self.tile(id).is_some()
        {
            self.unmaximize_for_drag(id, at_press, pointer);
        }
        self.begin_move(seat, id, at_press)
    }

    /// Unmaximises or untiles `id` for `begin_titlebar_move`, keeping `pointer`'s fraction
    /// across the still-maximised titlebar the same across the one it's restored to, and
    /// `at_press`'s fraction down its frame. It then takes the catch up `begin_move`'s
    /// caller applies for the distance dragged since `at_press`.
    ///
    /// Built from `geometry`, policy's own requested rectangle, rather than `Managed::frame`
    /// (which lags behind it until the client commits a matching size): a drag must track
    /// where the frame is asked to be right now, the same as `grab_motion` already does.
    fn unmaximize_for_drag(
        &mut self,
        id: WindowId,
        at_press: Point<f64, Logical>,
        pointer: Point<f64, Logical>,
    ) {
        let Some(managed) = self.windows.get(&id) else {
            return;
        };
        let before = frame_around(managed.geometry, managed.visible_extents());
        let fraction = if before.size.w > 0 {
            ((pointer.x - f64::from(before.loc.x)) / f64::from(before.size.w)).clamp(0., 1.)
        } else {
            0.5
        };
        // A tile or a maximised window may be far taller than the window it goes back to,
        // and its floating geometry may be anywhere, so the press keeps its place down the
        // frame too, as Marco puts a shaken loose window under the pointer
        // (window.c:7470-7485) (158).
        let down = ((at_press.y - f64::from(before.loc.y)) / f64::from(before.size.h.max(1)))
            .clamp(0., 1.);
        self.set_maximized(id, false);
        let Some(managed) = self.windows.get_mut(&id) else {
            return;
        };
        let e = managed.visible_extents();
        let after_width = f64::from(managed.geometry.size.w + e.left + e.right);
        let frame_x = (at_press.x - fraction * after_width).round() as i32;
        managed.geometry.loc.x = frame_x + e.left;
        let after_height = f64::from(managed.geometry.size.h + e.top + e.bottom);
        let frame_y = (at_press.y - down * after_height).round() as i32;
        managed.geometry.loc.y = frame_y + e.top;
        self.apply();
    }

    /// The seat's pointer moved during a grab. With `snap` (Shift held), the window snaps to
    /// the nearest edges; otherwise it resists at the edges it meets (157). A keyboard
    /// resize that has no edge yet takes the one the pointer goes towards (156).
    ///
    /// As in Marco's `update_move` (window.c:7380-7445), a move holds a tiled window where
    /// it is until the pointer is twice the drag threshold from where the move began, then
    /// untiles it under the pointer, as a titlebar drag does at once (144). A move its
    /// client asked for holds a maximised window the same way. The pointer's
    /// place then picks where letting go tiles the window, before edge resistance moves
    /// it, and a snapping move tiles nowhere (158), nor does a keyboard move (156).
    pub fn grab_motion(&mut self, seat: SeatId, pointer: Point<f64, Logical>, snap: bool) {
        let Some(grab) = self.grabs.get_mut(&seat) else {
            return;
        };
        let delta = pointer - grab.start_pointer;
        grab.moved |= delta.x.abs() >= CLICK_DISTANCE || delta.y.abs() >= CLICK_DISTANCE;
        if grab.keyboard && grab.resize == Some(ResizeEdges::default()) {
            self.pick_resize_edges(seat, delta.to_i32_round());
            return;
        }
        if grab.resize.is_none() {
            let (window, start_pointer, held) = (grab.window, grab.start_pointer, grab.held);
            if held || self.tile(window).is_some() {
                let shake = f64::from(2 * self.config.drag_threshold);
                if delta.x.abs().max(delta.y.abs()) < shake {
                    return;
                }
                // Let go of first, so that nothing ends the grab as its window is laid out.
                self.grabs.get_mut(&seat).unwrap().held = false;
                self.unmaximize_for_drag(window, start_pointer, pointer);
                let loc = self.windows[&window].geometry.loc;
                let grab = self.grabs.get_mut(&seat).unwrap();
                grab.start.loc = loc;
                grab.initial.loc = loc;
            }
            let grab = &self.grabs[&seat];
            let target = if snap || grab.keyboard {
                None
            } else {
                self.tile_target(grab.window, pointer)
            };
            self.grabs.get_mut(&seat).unwrap().tile_target = target;
        }
        let grab = &self.grabs[&seat];
        let start = grab.start;
        let resize = grab.resize;
        let window = grab.window;
        let edges = self.edges_for(window);
        let managed = self.windows.get_mut(&window).unwrap();
        let current = managed.geometry;
        match resize {
            None => {
                let loc = (start.loc.to_f64() + delta).to_i32_round();
                managed.move_within(loc, &edges, snap, By::Pointer);
            }
            Some(moving) => {
                let (dx, dy) = (delta.x.round() as i32, delta.y.round() as i32);
                let mut size = start.size;
                if moving.left {
                    size.w -= dx;
                } else if moving.right {
                    size.w += dx;
                }
                if moving.top {
                    size.h -= dy;
                } else if moving.bottom {
                    size.h += dy;
                }
                managed.resize_within(start, moving, size, &edges, snap, By::Pointer);
            }
        }
        managed.pin_maximized_axes(current);
        self.apply();
    }

    /// Ends the seat's grab with its window back as it began, as Escape does in Marco's
    /// keyboard move and resize (keybindings.c:1523-1545, 1786-1806). It tiles nothing.
    pub fn cancel_grab(&mut self, seat: SeatId) {
        if let Some(grab) = self.grabs.get_mut(&seat) {
            grab.tile_target = None;
            let managed = self.windows.get_mut(&grab.window).unwrap();
            match grab.resize {
                None => managed.geometry.loc = grab.initial.loc,
                Some(_) => managed.geometry = grab.initial,
            }
        }
        self.end_grab(seat);
    }

    /// Ends the seat's grab, if it has one. With `raise-on-click` off, one that ends as a
    /// click raises its window, as Marco's does (display.c:3857-3870) (153). A move let go
    /// of in a tiling zone tiles or maximises its window there, as Marco's button release
    /// does (window.c:8022-8038), and the window goes back to where the move began when
    /// it's untiled (158).
    pub fn end_grab(&mut self, seat: SeatId) {
        if let Some(grab) = self.grabs.remove(&seat)
            && let Some(managed) = self.windows.get_mut(&grab.window)
        {
            managed.resize = None;
            if !self.config.raise_on_click && !grab.moved {
                self.raise_family(grab.window);
            }
            let managed = self.windows.get_mut(&grab.window).unwrap();
            if let Some(target) = grab.tile_target {
                // A window maximised one way keeps the floating geometry it had before.
                if managed.floating.is_none() {
                    managed.floating =
                        Some(Rectangle::new(grab.initial.loc, managed.geometry.size));
                }
                match target {
                    TileTarget::Tile(tile, output) => {
                        managed.tile = Some(Tiled::new(tile, output, TileSize::Uncycled));
                        managed.maximized_horizontally = false;
                        managed.maximized_vertically = false;
                        self.relayout(grab.window);
                    }
                    TileTarget::Maximize(_) => self.set_maximized(grab.window, true),
                }
            }
        }
        self.apply();
    }

    /// Where letting go of a move of `id` with the pointer at `pointer` tiles it, as
    /// Marco's `calculate_tiling_mode` finds it (window.c:7590-7629): on the output the
    /// pointer is on, a side's zone reaches from the output's edge to twice the drag
    /// threshold inside the work area's, a corner is where it meets the same strip along
    /// the top or bottom, and the top's zone is the output's edge down to the work area's.
    fn tile_target(&self, id: WindowId, pointer: Point<f64, Logical>) -> Option<TileTarget> {
        if !self.config.allow_tiling {
            return None;
        }
        let p = Point::<i32, Logical>::new(pointer.x.floor() as i32, pointer.y.floor() as i32);
        let output = self.outputs.iter().find(|o| o.geometry.contains(p))?;
        let (o, area) = (output.geometry, Self::work_area(output));
        let zone = 2 * self.config.drag_threshold;
        let can_maximize = self.windows[&id].functions().maximize;
        let can_tile = self.can_tile(id, area);
        let corner = |top, bottom, side| {
            if p.y >= o.loc.y && p.y < area.loc.y + zone {
                top
            } else if p.y < o.loc.y + o.size.h && p.y > area.loc.y + area.size.h - zone {
                bottom
            } else {
                side
            }
        };
        let tile = if can_tile && p.x >= o.loc.x && p.x < area.loc.x + zone {
            Some(corner(Tile::TopLeft, Tile::BottomLeft, Tile::Left))
        } else if can_tile && p.x >= area.loc.x + area.size.w - zone && p.x < o.loc.x + o.size.w {
            Some(corner(Tile::TopRight, Tile::BottomRight, Tile::Right))
        } else {
            None
        };
        match tile {
            Some(tile) => Some(TileTarget::Tile(tile, output.id)),
            None => (can_maximize
                && self.config.allow_top_tiling
                && p.y >= o.loc.y
                && p.y <= area.loc.y)
                .then_some(TileTarget::Maximize(output.id)),
        }
    }

    /// Ends grabs on windows that can no longer be moved or resized: hidden, maximised
    /// both ways or fullscreen, or shaded during a resize. A held move's window is still
    /// maximised.
    fn end_stale_grabs(&mut self) {
        let stale: Vec<SeatId> = self
            .grabs
            .iter()
            .filter(|(_, g)| {
                let managed = &self.windows[&g.window];
                !self.is_showing(g.window)
                    || (managed.maximized() && !g.held)
                    || managed.fullscreen
                    || (managed.shaded && g.resize.is_some())
            })
            .map(|(seat, _)| *seat)
            .collect();
        for seat in stale {
            let grab = self.grabs.remove(&seat).unwrap();
            if let Some(managed) = self.windows.get_mut(&grab.window) {
                managed.resize = None;
            }
        }
    }

    /// The edges a grab's window meets (157): the work areas', and the frames of the other
    /// windows showing whose edges Marco counts, all but desktops, menus and splash
    /// screens (edge-resistance.c:33-39).
    fn edges_for(&self, grabbed: WindowId) -> Edges {
        let windows: Vec<EdgeWindow> = self
            .active_stacking()
            .into_iter()
            .filter(|id| *id != grabbed && self.is_showing(*id))
            .filter_map(|id| {
                let managed = &self.windows[&id];
                let has_edges = match managed.window_type {
                    WindowType::Desktop | WindowType::Menu | WindowType::Splashscreen => {
                        return None;
                    }
                    WindowType::Dock => false,
                    _ => true,
                };
                Some(EdgeWindow {
                    frame: managed.frame(),
                    has_edges,
                })
            })
            .collect();
        Edges::new(
            self.outputs.iter().map(Self::work_area),
            &windows,
            self.screen(),
        )
    }

    /// The outputs' bounding box, Marco's screen.
    fn screen(&self) -> Rectangle<i32, Logical> {
        self.outputs
            .iter()
            .map(|o| o.geometry)
            .reduce(|a, b| a.merge(b))
            .unwrap_or_default()
    }

    fn begin_grab(
        &mut self,
        seat: SeatId,
        id: WindowId,
        resize: Option<ResizeEdges>,
        pointer: Point<f64, Logical>,
        requested: bool,
    ) -> bool {
        let held = requested
            && resize.is_none()
            && self.is_showing(id)
            && self
                .windows
                .get(&id)
                .is_some_and(|m| m.maximized() && !m.fullscreen);
        // In place of the seat's own move or resize, and of no other Interaction.
        let movable = (held || self.can_grab(id, resize.is_some()))
            && self.shared_interaction().is_none()
            && !self.grabs.iter().any(|(s, g)| g.window == id && *s != seat);
        if !movable {
            return false;
        }
        self.set_last_seat(seat);
        self.end_grab(seat);
        let managed = self.windows.get_mut(&id).unwrap();
        let start = Rectangle::new(managed.geometry.loc, managed.size);
        managed.resize = resize.map(|edges| (edges, start));
        self.grabs.insert(
            seat,
            Grab {
                window: id,
                resize,
                start_pointer: pointer,
                start,
                initial: start,
                keyboard: false,
                placing: false,
                warp: None,
                moved: false,
                tile_target: None,
                held,
            },
        );
        self.apply();
        true
    }

    // Queries.

    /// Whether what the Scene reads from policy changed since this was last asked (179):
    /// the compositor then draws its outputs again. It's worked out here, from the answers
    /// themselves, so no event has to say that it changed them.
    pub fn take_scene_changed(&mut self) -> bool {
        let reads = self.scene_reads();
        let changed = reads != self.last_scene_reads;
        self.last_scene_reads = reads;
        changed
    }

    /// What the Scene reads from policy, now.
    fn scene_reads(&self) -> SceneReads {
        SceneReads {
            stacking: self.active_stacking(),
            stack: self.stack.clone(),
            windows: self
                .windows
                .iter()
                .map(|(id, managed)| WindowReads {
                    id: *id,
                    geometry: Rectangle::new(managed.geometry.loc, managed.size),
                    frame: managed.frame(),
                    showing: self.is_showing(*id),
                    fullscreen: managed.fullscreen,
                    layer: self.layer(*id),
                })
                .collect(),
            grabs: self
                .grabs
                .iter()
                .map(|(seat, grab)| GrabReads {
                    seat: *seat,
                    window: grab.window,
                    tile_preview: self.tile_preview(*seat),
                })
                .collect(),
            switcher: self.switcher().map(|switcher| switcher.entries.clone()),
        }
    }

    pub fn window(&self, id: WindowId) -> Option<&W> {
        self.windows.get(&id).map(|m| &m.window)
    }

    /// The primary output's active workspace, which every output has while every switch
    /// sets them all (221) and with `workspaces-per-output` off (230). It
    /// takes one for all, so the compositor asks by output instead, and the tests, which
    /// mostly have one output or none, ask this.
    pub fn active_workspace(&self) -> usize {
        self.primary_shown().active
    }

    /// Whether each output switches its own active workspace: `workspaces-per-output` (230).
    pub fn workspaces_per_output(&self) -> bool {
        self.config.workspaces_per_output
    }

    /// `output`'s active workspace, or the one an output added takes if policy has no such
    /// output.
    pub fn active_workspace_on(&self, output: OutputId) -> usize {
        self.active_workspaces.of(Some(output)).active
    }

    /// The primary output's active workspace, or the one an output added takes while policy
    /// has no output. It stands in for the Active output's (222, 228).
    pub fn primary_active_workspace(&self) -> usize {
        let primary = self.primary_output().map(|output| output.id);
        self.active_workspaces.of(primary).active
    }

    /// Whether every output's active workspace shows its desktop (94).
    pub fn is_showing_desktop(&self) -> bool {
        self.shown_workspaces()
            .iter()
            .all(|workspace| self.showing_desktop.contains_key(workspace))
    }

    pub fn outputs(&self) -> &[Output] {
        &self.outputs
    }

    /// Each output's work area, in the order of [`outputs`](Self::outputs): the output less
    /// its reserved edges (64). It's the same on every workspace.
    pub fn work_areas(&self) -> impl Iterator<Item = Rectangle<i32, Logical>> + '_ {
        self.outputs.iter().map(Self::work_area)
    }

    /// The windows on a workspace, sticky ones too, bottom to top.
    pub fn stacking(&self, workspace: usize) -> Vec<WindowId> {
        self.stack
            .iter()
            .copied()
            .filter(|id| self.is_on_workspace(*id, workspace))
            .collect()
    }

    /// The windows on the active workspace of their Main output, sticky ones too, bottom to
    /// top: those that show, and the minimised among them.
    pub fn active_stacking(&self) -> Vec<WindowId> {
        self.stack
            .iter()
            .copied()
            .filter(|id| self.is_on_shown_workspace(*id))
            .collect()
    }

    /// Every window, bottom to top, whatever workspace it's on.
    pub fn stacking_all(&self) -> &[WindowId] {
        &self.stack
    }

    pub fn focus(&self, seat: SeatId) -> Option<WindowId> {
        self.focus.get(&seat).copied()
    }

    pub fn windows(&self) -> impl Iterator<Item = &W> {
        self.windows.values().map(|m| &m.window)
    }

    /// The windows on screen by when they last had focus, most recent first, so the
    /// focused one leads. A window that never had the focus isn't among them.
    pub fn recently_focused(&self) -> impl Iterator<Item = WindowId> + '_ {
        let on_screen = |id: &WindowId| self.is_showing(*id);
        self.mru.iter().copied().filter(on_screen)
    }

    pub fn workspace_count(&self) -> usize {
        self.workspace_count
    }

    /// Where each workspace is: the grid the layout keys give this many workspaces (96).
    pub fn workspace_grid(&self) -> Grid {
        Grid::new(self.workspace_count, self.config.workspace_layout)
    }

    /// Each workspace's name, from the first.
    pub fn workspace_names(&self) -> &[String] {
        &self.workspace_names
    }

    pub fn window_mut(&mut self, id: WindowId) -> Option<&mut W> {
        self.windows.get_mut(&id).map(|m| &mut m.window)
    }

    /// The new window the seat is placing by hand (154), if its grab is one.
    pub fn placing(&self, seat: SeatId) -> Option<WindowId> {
        self.grabs
            .get(&seat)
            .filter(|grab| grab.placing)
            .map(|grab| grab.window)
    }

    /// The window the seat is moving or resizing.
    pub fn grab(&self, seat: SeatId) -> Option<WindowId> {
        self.grabs.get(&seat).map(|g| g.window)
    }

    /// The half or quarter of a work area the window is tiled to (158).
    pub fn tile(&self, id: WindowId) -> Option<Tile> {
        self.windows.get(&id).and_then(Managed::tiled)
    }

    /// Where letting go of the seat's move would put the window's frame, while the pointer
    /// is in a tiling zone: the tile preview's rectangle (158).
    pub fn tile_preview(&self, seat: SeatId) -> Option<Rectangle<i32, Logical>> {
        let (area, tile) = match self.grabs.get(&seat)?.tile_target? {
            TileTarget::Tile(tile, output) => (self.work_area_of(output)?, Some(tile)),
            TileTarget::Maximize(output) => (self.work_area_of(output)?, None),
        };
        Some(tile.map_or(area, |tile| tile_area(area, tile, TileSize::Uncycled)))
    }

    fn work_area_of(&self, output: OutputId) -> Option<Rectangle<i32, Logical>> {
        self.outputs
            .iter()
            .find(|o| o.id == output)
            .map(Self::work_area)
    }

    /// Whether the window is maximised both ways.
    pub fn is_maximized(&self, id: WindowId) -> bool {
        self.windows.get(&id).is_some_and(|m| m.maximized())
    }

    pub fn is_maximized_horizontally(&self, id: WindowId) -> bool {
        self.windows
            .get(&id)
            .is_some_and(|m| m.maximized_horizontally)
    }

    pub fn is_maximized_vertically(&self, id: WindowId) -> bool {
        self.windows
            .get(&id)
            .is_some_and(|m| m.maximized_vertically)
    }

    /// The frame extents policy lays the window out with: none for a CSD or fullscreen
    /// window. What the debug dump shows and `_NET_FRAME_EXTENTS` publishes (39).
    pub fn frame_extents(&self, id: WindowId) -> FrameExtents<i32, Logical> {
        self.windows
            .get(&id)
            .map(Managed::visible_extents)
            .unwrap_or_default()
    }

    /// What Marco lets the window do (177). A window policy doesn't manage has no function.
    pub fn functions(&self, id: WindowId) -> Functions {
        self.windows
            .get(&id)
            .map(Managed::functions)
            .unwrap_or_default()
    }

    pub fn is_shaded(&self, id: WindowId) -> bool {
        self.windows.get(&id).is_some_and(|m| m.shaded)
    }

    pub fn is_fullscreen(&self, id: WindowId) -> bool {
        self.windows.get(&id).is_some_and(|m| m.fullscreen)
    }

    pub fn is_minimized(&self, id: WindowId) -> bool {
        self.windows.get(&id).is_some_and(|m| m.minimized)
    }

    /// Whether the window is kept above the normal layer.
    pub fn is_above(&self, id: WindowId) -> bool {
        self.windows.get(&id).is_some_and(|m| m.above)
    }

    /// Whether the window is kept below the normal layer.
    pub fn is_below(&self, id: WindowId) -> bool {
        self.windows.get(&id).is_some_and(|m| m.below)
    }

    /// Whether the window is on every workspace.
    pub fn is_sticky(&self, id: WindowId) -> bool {
        self.windows.get(&id).is_some_and(|m| m.sticky)
    }

    /// The workspace a window is on, or for a sticky one the workspace it was on before.
    pub fn workspace_of(&self, id: WindowId) -> Option<usize> {
        Some(self.windows.get(&id)?.workspace)
    }

    /// Where a window's geometry is, at the size the client last committed.
    pub fn geometry(&self, id: WindowId) -> Option<Rectangle<i32, Logical>> {
        let managed = self.windows.get(&id)?;
        Some(Rectangle::new(managed.geometry.loc, managed.size))
    }

    /// A window's outer rectangle: its geometry with its frame around it, or its frame's
    /// titlebar alone while it's shaded. What a capture of the window shows (103).
    pub fn frame_rect(&self, id: WindowId) -> Option<Rectangle<i32, Logical>> {
        Some(self.windows.get(&id)?.frame())
    }

    /// A window's transient parent, as policy resolved it.
    pub fn parent(&self, id: WindowId) -> Option<WindowId> {
        self.windows.get(&id)?.parent
    }

    pub fn layer(&self, id: WindowId) -> Option<Layer> {
        let managed = self.windows.get(&id)?;
        // Marco's `get_standalone_layer` (stack.c:272-315): kept below wins over
        // fullscreen, which wins over kept above, and a dock kept below goes down too.
        let own = match managed.window_type {
            WindowType::Desktop => Layer::Desktop,
            _ if managed.below => Layer::Bottom,
            WindowType::Dock => Layer::Top,
            _ if managed.fullscreen && self.focus.values().any(|f| self.is_ancestor(id, *f)) => {
                Layer::Fullscreen
            }
            _ if managed.above => Layer::Top,
            _ => Layer::Normal,
        };
        // Marco promotes a transient into its parent's layer.
        Some(
            managed
                .parent
                .and_then(|p| self.layer(p))
                .map_or(own, |p| p.max(own)),
        )
    }

    // Transients.

    /// Whether `ancestor` is `id` or one of its transient ancestors.
    fn is_ancestor(&self, ancestor: WindowId, id: WindowId) -> bool {
        let mut current = Some(id);
        while let Some(w) = current {
            if w == ancestor {
                return true;
            }
            current = self.windows[&w].parent;
        }
        false
    }

    /// Links the windows that are transient for a newly mapped `parent` to it, unless that
    /// makes a cycle, and moves them to its workspace.
    fn adopt_transients(&mut self, parent: WindowId) {
        let orphans: Vec<WindowId> = self
            .windows
            .iter()
            .filter(|(_, m)| m.parent.is_none() && m.window.transient_parent() == Some(parent))
            .map(|(id, _)| *id)
            .collect();
        for id in orphans {
            if !self.is_ancestor(id, parent) {
                self.windows.get_mut(&id).unwrap().parent = Some(parent);
                self.join_parent(id, parent);
            }
        }
    }

    /// Puts a window that's become transient for `parent` where the parent is, with its
    /// own transients: on its workspace, and sticky if it is.
    fn join_parent(&mut self, id: WindowId, parent: WindowId) {
        let (workspace, sticky) = {
            let parent = &self.windows[&parent];
            (parent.workspace, parent.sticky)
        };
        self.move_tree(id, workspace);
        for w in self.tree(id) {
            self.windows.get_mut(&w).unwrap().sticky = sticky;
        }
    }

    /// A window and all its transient descendants.
    fn tree(&self, id: WindowId) -> Vec<WindowId> {
        let mut tree = vec![id];
        let mut i = 0;
        while i < tree.len() {
            let parent = tree[i];
            tree.extend(
                self.windows
                    .iter()
                    .filter(|(_, m)| m.parent == Some(parent))
                    .map(|(id, _)| *id),
            );
            i += 1;
        }
        tree
    }

    /// Moves a window and its transient descendants to `workspace`, on top of its windows,
    /// keeping their order. None of them is sticky after.
    fn move_tree(&mut self, id: WindowId, workspace: usize) {
        let tree = self.tree(id);
        self.stack.retain(|w| !tree.contains(w));
        for w in &tree {
            let managed = self.windows.get_mut(w).unwrap();
            managed.workspace = workspace;
            managed.sticky = false;
        }
        self.stack.extend(&tree);
    }

    /// The root of a window's transient tree.
    fn root(&self, id: WindowId) -> WindowId {
        let mut id = id;
        while let Some(parent) = self.windows[&id].parent {
            id = parent;
        }
        id
    }

    // Outputs and placement.

    /// The primary output (81), or the first while policy is told of none that it has.
    fn primary_output(&self) -> Option<&Output> {
        self.outputs
            .iter()
            .find(|output| Some(output.id) == self.primary_output)
            .or(self.outputs.first())
    }

    /// The output policy has of those named `named`, as a client names one for fullscreen.
    fn output_named(&self, named: Option<OutputId>) -> Option<&Output> {
        named.and_then(|named| self.outputs.iter().find(|output| output.id == named))
    }

    /// The output a frame is most on, if it's on any.
    fn output_of(&self, frame: Rectangle<i32, Logical>) -> Option<&Output> {
        self.outputs
            .iter()
            .filter_map(|o| {
                let overlap = o.geometry.intersection(frame)?;
                Some((o, i64::from(overlap.size.w) * i64::from(overlap.size.h)))
            })
            .max_by_key(|(_, area)| *area)
            .map(|(o, _)| o)
    }

    /// The part of an output windows are placed and maximised in: the output less its
    /// reserved edges (64). Edges that reserve the whole output leave one pixel of it, so
    /// the area is never empty. Marco gives such a monitor a work area with no size
    /// (workspace.c:717-727).
    fn work_area(output: &Output) -> Rectangle<i32, Logical> {
        let Output {
            geometry, reserved, ..
        } = *output;
        let left = reserved.left.clamp(0, (geometry.size.w - 1).max(0));
        let right = reserved.right.clamp(0, (geometry.size.w - 1 - left).max(0));
        let top = reserved.top.clamp(0, (geometry.size.h - 1).max(0));
        let bottom = reserved.bottom.clamp(0, (geometry.size.h - 1 - top).max(0));
        Rectangle::new(
            (geometry.loc.x + left, geometry.loc.y + top).into(),
            (
                geometry.size.w - left - right,
                geometry.size.h - top - bottom,
            )
                .into(),
        )
    }

    /// Sets the geometry of a maximised or fullscreen window from its output, or restores a
    /// floating window's. A window maximised one way keeps its floating geometry the
    /// other way.
    fn relayout(&mut self, id: WindowId) {
        self.relayout_on(id, None);
    }

    /// `relayout`, on output `on` if it names one policy has: a window a binding moved to
    /// another output (160) is laid out there, though its frame is still on the one it left.
    fn relayout_on(&mut self, id: WindowId, on: Option<OutputId>) {
        let managed = &self.windows[&id];
        if !managed.constrained() {
            let managed = self.windows.get_mut(&id).unwrap();
            if let Some(floating) = managed.floating.take() {
                managed.geometry = floating;
            }
            return;
        }
        let output = on
            .or(managed.fullscreen_output.filter(|_| managed.fullscreen))
            .or(managed.tile.map(|tiled| tiled.output))
            .and_then(|o| self.outputs.iter().find(|output| output.id == o))
            // The frame it was laid out in: one the client hasn't drawn yet may be on another.
            .or_else(|| self.output_of(managed.frame_of(managed.geometry)))
            .or(self.primary_output());
        let Some(output) = output else {
            return;
        };
        let geometry = if managed.fullscreen {
            output.geometry
        } else if managed.tile.is_some_and(|tiled| tiled.resized) {
            managed.geometry
        } else if let Some(tiled) = managed.tile {
            // Marco ignores the size hints but the least size, as it does for a maximised
            // window (constraints.c:928-934).
            let e = managed.visible_extents();
            let frame = tile_area(Self::work_area(output), tiled.tile, tiled.size);
            let size: Size<i32, Logical> = Size::new(
                frame.size.w - e.left - e.right,
                frame.size.h - e.top - e.bottom,
            );
            let size = Size::new(
                size.w.max(managed.hints.min.w).max(1),
                size.h.max(managed.hints.min.h).max(1),
            );
            Rectangle::new((frame.loc.x + e.left, frame.loc.y + e.top).into(), size)
        } else {
            let area = Self::work_area(output);
            let e = managed.visible_extents();
            let mut geometry = managed.floating.unwrap_or(managed.geometry);
            if managed.maximized_horizontally {
                geometry.loc.x = area.loc.x + e.left;
                geometry.size.w = (area.size.w - e.left - e.right).max(1);
            }
            if managed.maximized_vertically {
                geometry.loc.y = area.loc.y + e.top;
                geometry.size.h = (area.size.h - e.top - e.bottom).max(1);
            }
            geometry
        };
        // A tiled window whose output went away is tiled on the one it's on now.
        let tile = managed.tile.map(|tiled| Tiled {
            output: output.id,
            ..tiled
        });
        let managed = self.windows.get_mut(&id).unwrap();
        managed.geometry = geometry;
        managed.tile = tile;
    }

    /// After outputs change: moves every window that's on no output onto the primary
    /// output, and lays out maximised and fullscreen windows again.
    fn rehome_windows(&mut self) {
        let Some(primary) = self.primary_output() else {
            return;
        };
        let area = Self::work_area(primary);
        let ids: Vec<WindowId> = self.windows.keys().copied().collect();
        for id in ids {
            let managed = &self.windows[&id];
            let frame = managed.frame();
            if self.output_of(frame).is_none() {
                let e = managed.visible_extents();
                let frame_loc = placement::constrain(frame, area);
                let managed = self.windows.get_mut(&id).unwrap();
                managed.geometry.loc = Point::new(frame_loc.x + e.left, frame_loc.y + e.top);
            }
            // The geometry to restore must land on an output too.
            let managed = &self.windows[&id];
            if let Some(floating) = managed.floating {
                let e = managed.floating_extents();
                let frame = frame_around(floating, e);
                if self.output_of(frame).is_none() {
                    let frame_loc = placement::constrain(frame, area);
                    let floating = self
                        .windows
                        .get_mut(&id)
                        .unwrap()
                        .floating
                        .as_mut()
                        .unwrap();
                    floating.loc = Point::new(frame_loc.x + e.left, frame_loc.y + e.top);
                }
            }
            let managed = self.windows.get_mut(&id).unwrap();
            if managed
                .fullscreen_output
                .is_some_and(|o| !self.outputs.iter().any(|output| output.id == o))
            {
                managed.fullscreen_output = None;
            }
            self.relayout(id);
        }
    }

    /// Marco maximises a framed window that's placed at least as big as its work area, if
    /// it has the maximise function (place.c:913-918).
    fn maximize_if_too_big(&mut self, id: WindowId) {
        let managed = &self.windows[&id];
        let frame = managed.frame();
        let Some(output) = self.output_of(frame).or(self.primary_output()) else {
            return;
        };
        let area = Self::work_area(output);
        if managed.ssd
            && managed.functions().maximize
            && frame.size.w >= area.size.w
            && frame.size.h >= area.size.h
        {
            let managed = self.windows.get_mut(&id).unwrap();
            managed.save_floating();
            managed.maximized_horizontally = true;
            managed.maximized_vertically = true;
            self.relayout(id);
        }
    }

    /// Places a newly mapped window as Marco's `meta_window_place` does (place.c:700-905),
    /// with the placement keys if `by_mode` (154), else with the automatic mode as if
    /// `center-new-windows` were off.
    fn place(&mut self, id: WindowId, by_mode: bool) -> Placed {
        let managed = &self.windows[&id];
        let e = managed.visible_extents();
        let (left, top) = (e.left, e.top);
        let frame_size = managed.frame().size;
        let size = managed.size;

        let mut by_pointer = false;
        let mut placed_on = None;
        let loc = if let Some(requested) = managed.window.requested_position(e) {
            // Marco honours USPosition and PPosition with its workarounds on (the default).
            requested
        } else {
            let (placement, output) =
                self.new_window_placement(managed.window_type, managed.parent, by_mode);
            placed_on = output.map(|output| output.id);
            match (placement, output) {
                (NewPlacement::OverParent(parent), output) => {
                    // Centred over the parent, leaving twice as much space below as above.
                    let parent = self.geometry(parent).unwrap();
                    let loc = Point::new(
                        parent.loc.x + parent.size.w / 2 - size.w / 2,
                        parent.loc.y + (parent.size.h - size.h) / 3 + top,
                    );
                    match output {
                        Some(output) => {
                            let frame =
                                Rectangle::new((loc.x - left, loc.y - top).into(), frame_size);
                            let frame_loc = placement::constrain(frame, Self::work_area(output));
                            Point::new(frame_loc.x + left, frame_loc.y + top)
                        }
                        None => loc,
                    }
                }
                // Marco centres the window geometry, not the frame, on the output.
                (NewPlacement::Centred, Some(output)) => {
                    let area = output.geometry;
                    Point::new(
                        area.loc.x + (area.size.w - size.w) / 2,
                        area.loc.y + (area.size.h - size.h) / 2,
                    )
                }
                (NewPlacement::AtPointer(pointer), output) => {
                    // Kept within the work area of the output the pointer is on (154), as
                    // Marco's constraints keep a new window on its monitor.
                    by_pointer = true;
                    let frame_loc = placement::at_pointer(frame_size, pointer);
                    let frame_loc = match output {
                        Some(output) => placement::constrain(
                            Rectangle::new(frame_loc, frame_size),
                            Self::work_area(output),
                        ),
                        None => frame_loc,
                    };
                    Point::new(frame_loc.x + left, frame_loc.y + top)
                }
                (NewPlacement::Automatic, Some(output)) => {
                    let center = by_mode && self.config.center_new_windows;
                    self.place_on(id, output, frame_size, (left, top), center)
                }
                (NewPlacement::Centred | NewPlacement::Automatic, None) => Point::new(left, top),
            }
        };
        self.windows.get_mut(&id).unwrap().geometry.loc = loc;
        Placed {
            by_pointer,
            on: placed_on,
        }
    }

    /// How a new window of `window_type`, transient for the managed window `parent`, is
    /// placed when it has no position of its own, and on which output: the one choice of
    /// output that both `place` and `pre_map_configure` make (190). The placement keys
    /// count if `by_mode` (154): a window that maps maximised or fullscreen is placed as if
    /// the mode were automatic.
    fn new_window_placement(
        &self,
        window_type: WindowType,
        parent: Option<WindowId>,
        by_mode: bool,
    ) -> (NewPlacement, Option<&Output>) {
        let dialog = matches!(window_type, WindowType::Dialog | WindowType::ModalDialog);
        if let (true, Some(parent)) = (dialog, parent) {
            let output = self.output_of(self.windows[&parent].frame());
            (NewPlacement::OverParent(parent), output)
        } else if dialog || window_type == WindowType::Splashscreen {
            (NewPlacement::Centred, self.placement_output())
        } else if let (true, PlacementMode::Pointer | PlacementMode::Manual, Some(pointer)) =
            (by_mode, self.config.placement_mode, self.pointer)
        {
            let pointer = pointer.to_i32_round();
            let output = self
                .outputs
                .iter()
                .find(|o| o.geometry.contains(pointer))
                .or(self.primary_output());
            (NewPlacement::AtPointer(pointer), output)
        } else {
            (NewPlacement::Automatic, self.placement_output())
        }
    }

    /// The output a new window goes on with nothing else to go by: the focused window's,
    /// else the primary output.
    fn placement_output(&self) -> Option<&Output> {
        self.focus(self.last_seat)
            .and_then(|f| self.output_of(self.windows[&f].frame()))
            .or(self.primary_output())
    }

    /// Places a window with the automatic mode on `output`, in the middle of its work area if
    /// `center`.
    fn place_on(
        &self,
        id: WindowId,
        output: &Output,
        frame_size: Size<i32, Logical>,
        (left, top): (i32, i32),
        center: bool,
    ) -> Point<i32, Logical> {
        let area = Self::work_area(output);
        let frame_loc = if center {
            // A frame that doesn't fit the middle doesn't fit beside another window either,
            // and Marco leaves it at the monitor's origin with no cascade
            // (place.c:606-628, 941), which the work area moves it into.
            placement::center(frame_size, area).unwrap_or(output.geometry.loc)
        } else {
            let workspace = self.workspace_shown_on(id);
            let others: Vec<Obstacle> = self
                .stacking(workspace)
                .into_iter()
                .filter(|other| *other != id && self.is_visible_on_workspace(*other))
                .map(|other| &self.windows[&other])
                .filter(|m| self.output_of(m.frame()).map(|o| o.id) == Some(output.id))
                .map(|m| Obstacle {
                    frame: m.frame(),
                    client: m.geometry.loc,
                    blocks: matches!(
                        m.window_type,
                        WindowType::Normal
                            | WindowType::Utility
                            | WindowType::Toolbar
                            | WindowType::Menu
                    ),
                })
                .collect();
            placement::first_fit(frame_size, area, &others)
                .unwrap_or_else(|| placement::cascade(frame_size, (left, top), area, &others))
        };
        let frame_loc = placement::constrain(Rectangle::new(frame_loc, frame_size), area);
        Point::new(frame_loc.x + left, frame_loc.y + top)
    }

    // Focus and stacking.

    fn set_last_seat(&mut self, seat: SeatId) {
        self.seats.insert(seat);
        self.last_seat = seat;
    }

    /// Focuses `id` for `seat` on user input at `now`, or the modal dialog over it. Returns
    /// false, changing nothing, if the request is older than the seat's last focus change.
    fn focus_window(&mut self, seat: SeatId, id: WindowId, now: Duration) -> bool {
        if self.focus_time.get(&seat).is_some_and(|last| now < *last) {
            return false;
        }
        self.seats.insert(seat);
        self.focus_time.insert(seat, now);
        self.focus.insert(seat, self.modal_transient(id));
        true
    }

    /// The window that takes the focus `id` is given: the last of the modal dialogs over
    /// it, or `id` itself with none, as Marco's `get_modal_transient` finds it
    /// (window.c:4381-4417). Marco then focuses nothing if that dialog is minimised; policy
    /// skips such a dialog instead, so that a seat's focus is always on screen. A transient
    /// is on its parent's workspace already, so nothing is moved as Marco moves it.
    fn modal_transient(&self, id: WindowId) -> WindowId {
        let mut current = id;
        // Parents make no cycle, so each step goes one dialog further and the walk ends.
        while let Some((&next, _)) = self.windows.iter().find(|(_, managed)| {
            managed.parent == Some(current)
                && managed.window_type == WindowType::ModalDialog
                && !managed.minimized
        }) {
            current = next;
        }
        current
    }

    /// Whether a window is a desktop or a dock, which Marco gives no window functions.
    pub fn is_desktop_or_dock(&self, id: WindowId) -> bool {
        self.windows.get(&id).is_some_and(|managed| {
            matches!(managed.window_type, WindowType::Desktop | WindowType::Dock)
        })
    }

    /// Whether `id` or one of its transient ancestors is the desktop or a dock: the
    /// windows that stay while the desktop is shown, so that the user can work on a file
    /// of the desktop's through a dialog (window.c:190-200).
    fn is_desktop_component(&self, id: WindowId) -> bool {
        let mut current = Some(id);
        while let Some(w) = current {
            if self.is_desktop_or_dock(w) {
                return true;
            }
            current = self.windows[&w].parent;
        }
        false
    }

    /// Whether a window shows while its workspace shows the desktop: a desktop component, or
    /// a window `show-desktop-skip-list` lists.
    fn stays_on_desktop(&self, id: WindowId) -> bool {
        self.is_desktop_component(id) || self.is_skipped(id)
    }

    /// Whether the skip list its workspace took when it showed the desktop lists a window,
    /// or a transient ancestor, so that a listed app's dialogs stay with it. Marco matches
    /// each window by its own class, which a toolkit gives its dialogs too.
    fn is_skipped(&self, id: WindowId) -> bool {
        let Some(shown) = self.showing_desktop.get(&self.workspace_shown_on(id)) else {
            return false;
        };
        let mut current = Some(id);
        while let Some(w) = current {
            let managed = &self.windows[&w];
            if shown.skip_list.matches(managed.window.app_id().as_deref()) {
                return true;
            }
            current = managed.parent;
        }
        false
    }

    /// Stops showing the desktop on the workspace of `id`, a window newly mapped or
    /// activated there, unless it's a desktop component. The windows that were hidden
    /// are minimised for good, so that `id` comes up alone: Marco's
    /// `maybe_leave_show_desktop_mode` (window.c:183-208), which minimises those with a
    /// minimise function, the normal windows and dialogs (window.c:6821-6827).
    ///
    /// Two things aren't Marco's, on purpose. Marco looks at the active workspace
    /// whichever one the window is on, where this looks at the window's own. And Marco
    /// minimises a new dialog's parents with the rest, which hides the dialog with them;
    /// here they're kept, as Marco itself keeps an activated window's.
    fn leave_show_desktop(&mut self, id: WindowId) {
        let workspace = self.workspace_shown_on(id);
        if !self.showing_desktop.contains_key(&workspace) || self.stays_on_desktop(id) {
            return;
        }
        for other in self.stacking(workspace) {
            let managed = &self.windows[&other];
            if matches!(managed.window_type, WindowType::Normal | WindowType::Dialog)
                && !self.is_ancestor(other, id)
                && !self.is_skipped(other)
            {
                self.windows.get_mut(&other).unwrap().minimized = true;
            }
        }
        self.showing_desktop.remove(&workspace);
    }

    /// Whether a window is on screen: managed, and shown on its Main output's active
    /// workspace.
    pub fn is_showing(&self, id: WindowId) -> bool {
        self.is_on_shown_workspace(id) && self.is_visible_on_workspace(id)
    }

    /// Whether a managed window is on `workspace`: it's sticky, or that's its workspace, as
    /// Marco's `meta_window_located_on_workspace` has it (window.c:1346-1351).
    fn is_on_workspace(&self, id: WindowId, workspace: usize) -> bool {
        self.windows
            .get(&id)
            .is_some_and(|m| m.sticky || m.workspace == workspace)
    }

    /// The workspace whose desktop showing hides a window: its own, or its Main output's
    /// active one for a sticky window, as in Marco's `meta_window_showing_on_its_workspace`
    /// (window.c:1395-1401).
    fn workspace_shown_on(&self, id: WindowId) -> usize {
        let managed = &self.windows[&id];
        if managed.sticky {
            self.active_workspace_of(id)
        } else {
            managed.workspace
        }
    }

    /// Whether a window would show if its workspace were active: neither it nor a transient
    /// ancestor is minimised, and its workspace doesn't show its desktop instead, as in
    /// Marco's `meta_window_showing_on_its_workspace` (window.c:1385-1425).
    fn is_visible_on_workspace(&self, id: WindowId) -> bool {
        if self.windows.contains_key(&id)
            && self
                .showing_desktop
                .contains_key(&self.workspace_shown_on(id))
            && !self.stays_on_desktop(id)
        {
            return false;
        }
        let mut current = Some(id);
        while let Some(w) = current {
            let Some(managed) = self.windows.get(&w) else {
                return false;
            };
            if managed.minimized {
                return false;
            }
            current = managed.parent;
        }
        true
    }

    /// The window Marco focuses when the focused one goes away: the topmost showing window in
    /// the stack that isn't a dock, or failing that the desktop.
    fn default_focus(&self) -> Option<WindowId> {
        self.default_focus_where(|_| true)
    }

    /// `default_focus`, of the windows `among` takes.
    fn default_focus_where(&self, among: impl Fn(WindowId) -> bool) -> Option<WindowId> {
        let candidates = || {
            self.stack
                .iter()
                .rev()
                .copied()
                .filter(|id| self.is_showing(*id) && among(*id))
        };
        let window_type = |id: &WindowId| self.windows[id].window_type;
        candidates()
            .find(|id| !matches!(window_type(id), WindowType::Dock | WindowType::Desktop))
            .or_else(|| candidates().find(|id| window_type(id) == WindowType::Desktop))
    }

    /// Gives every seat whose focused window is no longer showing the default focus.
    fn fix_focus(&mut self) {
        let lost: Vec<SeatId> = self
            .focus
            .iter()
            .filter(|(_, id)| !self.is_showing(**id))
            .map(|(seat, _)| *seat)
            .collect();
        for seat in lost {
            match self.default_focus() {
                Some(id) => self.focus.insert(seat, id),
                None => self.focus.remove(&seat),
            };
        }
    }

    /// Raises a window as Marco does: its transient root first, then the window itself.
    fn raise_family(&mut self, id: WindowId) {
        self.move_to_top(self.root(id));
        self.move_to_top(id);
    }

    /// Moves a window to the top of the stack.
    fn move_to_top(&mut self, id: WindowId) {
        self.stack.retain(|w| *w != id);
        self.stack.push(id);
    }

    // Applying decisions.

    /// Sends each window whatever policy now wants of it that it hasn't been sent.
    fn apply(&mut self) {
        if self.holding {
            return;
        }
        // A window dragged across joins what the other output shows, before anything goes
        // by what's showing.
        self.follow_main_outputs();
        // First, as Marco runs it once the windows have moved (screen.c:1360-1371).
        self.update_dynamic_workspaces();
        self.end_stale_grabs();
        self.close_stale_window_menu();
        self.fix_focus();
        self.mru.retain(|id| self.windows.contains_key(id));
        if let Some(focused) = self.focus(self.last_seat) {
            self.mru.retain(|id| *id != focused);
            self.mru.insert(0, focused);
        }
        let mut stack = std::mem::take(&mut self.stack);
        stack::restack(
            &mut stack,
            |id| self.layer(id).unwrap(),
            |id| self.parent(id),
        );
        self.stack = stack;

        let focused: Vec<WindowId> = self.focus.values().copied().collect();
        // A window's frame extents can change with its focus (ADR 0012).
        let refocused: Vec<WindowId> = self
            .windows
            .iter_mut()
            .filter_map(|(id, managed)| {
                let activated = focused.contains(id);
                let changed = managed.activated != activated;
                managed.activated = activated;
                // Marco's `meta_window_focus` unsets it (window.c:4468-4469).
                managed.demands_attention &= !activated;
                (changed && managed.constrained()).then_some(*id)
            })
            .collect();
        for id in refocused {
            self.relayout(id);
        }
        let showing: BTreeSet<WindowId> = self
            .windows
            .keys()
            .copied()
            .filter(|id| self.is_showing(*id))
            .collect();
        for (id, managed) in &mut self.windows {
            let outputs: Vec<OutputId> = if showing.contains(id) {
                let frame = managed.frame();
                self.outputs
                    .iter()
                    .filter(|o| o.geometry.overlaps(frame))
                    .map(|o| o.id)
                    .collect()
            } else {
                Vec::new()
            };
            if managed.sent.outputs.as_ref() != Some(&outputs) {
                managed.window.set_outputs(&outputs);
                managed.sent.outputs = Some(outputs);
            }

            let functions = managed.functions();
            if managed.sent.functions != Some(functions) {
                managed.window.set_functions(functions);
                managed.sent.functions = Some(functions);
            }

            let request = (managed.geometry, managed.states());
            if managed.sent.geometry != Some(request) {
                managed.window.set_geometry(request.0, request.1);
                managed.sent.geometry = Some(request);
            }

            let activated = focused.contains(id);
            if managed.sent.activated != activated {
                managed.sent.activated = activated;
                managed.window.set_activated(activated);
            }
        }
    }
}
