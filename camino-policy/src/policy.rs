//! `Policy`: Camino's window-management model, driven by events and applied through `W`.

use std::time::Duration;

use std::collections::{BTreeMap, BTreeSet};

use smithay::utils::{FrameExtents, Logical, Point, Rectangle, Size};

use crate::placement::{self, Obstacle};
use crate::stack::{self, Layer};
use crate::{
    Action, FrameState, Output, OutputId, PolicyWindow, SizeHints, WindowId, WindowStates,
    WindowType,
};

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
    /// The window geometry when the grab began, at its committed size.
    start: Rectangle<i32, Logical>,
}

/// Marco's `MAX_REASONABLE_WORKSPACES`, the schema's upper bound on `num-workspaces`.
pub const MAX_WORKSPACES: usize = 36;

/// Policy's settings, from GSettings (09).
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// `num-workspaces`, which policy keeps within 1 to `MAX_WORKSPACES`.
    pub workspace_count: usize,
    /// Each workspace's name, from the first. Policy only holds them, for ext-workspace
    /// and the debug dump; a missing name is Marco's "Workspace N".
    pub workspace_names: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        // Marco's default `num-workspaces`, with its default names.
        let workspace_count = 4;
        Self {
            workspace_count,
            workspace_names: (1..=workspace_count).map(default_workspace_name).collect(),
        }
    }
}

/// Marco's name for a workspace with none set, numbered from 1.
pub fn default_workspace_name(number: usize) -> String {
    format!("Workspace {number}")
}

/// What policy has last asked of a window, so each request is sent only when it changes.
#[derive(Default)]
struct Sent {
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
    /// Kept above, kept below and sticky, as the client asked; they take effect in M8 (17).
    above: bool,
    below: bool,
    sticky: bool,
    /// The geometry to go back to, while maximised or fullscreen.
    floating: Option<Rectangle<i32, Logical>>,
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
        let e = self.visible_extents();
        let mut frame = frame_around(Rectangle::new(self.geometry.loc, self.size), e);
        if self.shaded {
            frame.size.h = e.top;
        }
        frame
    }
}

impl<W> Managed<W> {
    /// Maximised both ways, as Marco's `META_WINDOW_MAXIMIZED` has it.
    fn maximized(&self) -> bool {
        self.maximized_horizontally && self.maximized_vertically
    }

    /// Policy sets the size: maximised either way, or fullscreen.
    fn constrained(&self) -> bool {
        self.maximized_horizontally || self.maximized_vertically || self.fullscreen
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
        }
    }

    fn frame_state(&self) -> FrameState {
        FrameState {
            maximized: self.maximized(),
            focused: self.activated,
            shaded: self.shaded,
        }
    }
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

pub struct Policy<W: PolicyWindow> {
    windows: BTreeMap<WindowId, Managed<W>>,
    /// Each workspace's windows, bottom to top.
    stacks: Vec<Vec<WindowId>>,
    active: usize,
    /// Each workspace's name, one per stack.
    workspace_names: Vec<String>,
    outputs: Vec<Output>,
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
}

impl<W: PolicyWindow> Policy<W> {
    pub fn new(config: Config) -> Self {
        let mut policy = Self {
            windows: BTreeMap::new(),
            stacks: vec![Vec::new()],
            active: 0,
            workspace_names: Vec::new(),
            outputs: Vec::new(),
            seats: BTreeSet::new(),
            focus: BTreeMap::new(),
            focus_time: BTreeMap::new(),
            last_seat: SeatId(0),
            grabs: BTreeMap::new(),
            mru: Vec::new(),
        };
        policy.settings_changed(config);
        policy
    }

    // Events.

    /// Adds an output, or updates the one with the same id.
    pub fn add_output(&mut self, output: Output) {
        match self.outputs.iter_mut().find(|o| o.id == output.id) {
            Some(existing) => *existing = output,
            None => self.outputs.push(output),
        }
        self.rehome_windows();
        self.apply();
    }

    /// Removes an output. Windows left on no output move onto the first remaining one.
    pub fn remove_output(&mut self, id: OutputId) {
        self.outputs.retain(|o| o.id != id);
        self.rehome_windows();
        self.apply();
    }

    /// The settings changed. A smaller workspace count moves the windows on the removed
    /// workspaces to the new last one, and the active workspace with them, as Marco's
    /// `update_num_workspaces` does; a larger one adds empty workspaces at the end.
    pub fn settings_changed(&mut self, config: Config) {
        let count = config.workspace_count.clamp(1, MAX_WORKSPACES);
        let mut names = config.workspace_names;
        names.truncate(count);
        names.extend((names.len() + 1..=count).map(default_workspace_name));
        self.workspace_names = names;

        if count < self.stacks.len() {
            let last = count - 1;
            let removed: Vec<WindowId> = self.stacks.drain(count..).flatten().collect();
            for id in &removed {
                self.windows.get_mut(id).unwrap().workspace = last;
            }
            // Marco keeps one stacking order across workspaces, which moving leaves alone.
            // With a stack per workspace, the moved windows go above the kept ones, in the
            // order of the workspaces they came from, which is only close to that.
            self.stacks[last].extend(removed);
            self.active = self.active.min(last);
        } else {
            self.stacks.resize(count, Vec::new());
        }
        // The active workspace may have been removed, so every seat's focus is checked.
        self.apply();
    }

    /// Starts managing a window the client has mapped.
    pub fn map_window(&mut self, window: W, now: Duration) {
        let id = window.id();
        let size = window.size();
        let geometry = Rectangle::new(Point::default(), size);
        let window_type = window.window_type();
        let takes_input = window.takes_input();
        let hints = window.size_hints();
        let ssd = window.is_ssd();
        let parent = window
            .transient_parent()
            .filter(|p| self.windows.contains_key(p));
        let workspace = parent.map_or(self.active, |p| self.windows[&p].workspace);
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
                sticky: false,
                floating: None,
                sent: Sent::default(),
            },
        );
        self.stacks[workspace].push(id);
        self.place(id);
        self.maximize_if_too_big(id);
        self.adopt_transients(id);
        // Marco's `window_state_on_map` (window.c:1997-2044).
        if takes_input
            && !matches!(
                window_type,
                WindowType::Desktop
                    | WindowType::Dock
                    | WindowType::Splashscreen
                    | WindowType::Menu
                    | WindowType::Utility
                    | WindowType::Toolbar
            )
        {
            self.focus_window(self.last_seat, id, now);
        }
        self.apply();
    }

    /// Stops managing a window the client has unmapped or destroyed, and hands it back.
    pub fn unmap_window(&mut self, id: WindowId) -> Option<W> {
        let managed = self.windows.remove(&id)?;
        self.stacks[managed.workspace].retain(|w| *w != id);
        self.grabs.retain(|_, grab| grab.window != id);
        for other in self.windows.values_mut() {
            if other.parent == Some(id) {
                other.parent = None;
            }
        }
        // Marco hands focus to a lost transient's parent before the rest of the stack.
        if let Some(parent) = managed.parent.filter(|p| self.is_showing(*p)) {
            for focus in self.focus.values_mut() {
                if *focus == id {
                    *focus = parent;
                }
            }
        }
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
                self.move_tree(id, self.windows[&parent].workspace);
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

    /// A button press on a window: click focus, and Marco's raise on click. Marco raises a
    /// dock but doesn't focus it, since panels ask for focus themselves (display.c:1937-1946).
    pub fn click(&mut self, seat: SeatId, id: WindowId, now: Duration) {
        self.set_last_seat(seat);
        if self.is_showing(id) {
            let dock = self.windows[&id].window_type == WindowType::Dock;
            if dock || self.focus_window(seat, id, now) {
                self.raise_family(id);
            }
        }
        self.apply();
    }

    /// A keybinding action on behalf of `seat`.
    pub fn action(&mut self, seat: SeatId, action: Action, now: Duration) {
        self.set_last_seat(seat);
        match action {
            Action::SwitchToWorkspaceLeft => self.switch_workspace(self.active.saturating_sub(1)),
            Action::SwitchToWorkspaceRight => {
                self.switch_workspace((self.active + 1).min(self.stacks.len() - 1))
            }
            Action::ToggleMaximized => {
                // Marco gives the desktop and docks no maximise function.
                if let Some(id) = self.focus(seat).filter(|id| !self.is_desktop_or_dock(*id)) {
                    self.set_maximized(id, !self.is_maximized(id));
                }
            }
            Action::Close => {
                if let Some(id) = self.focus(seat) {
                    self.close(id);
                }
            }
            Action::SwitchWindows => {
                let current = self.focus(seat);
                let next = self.mru.iter().copied().find(|id| {
                    let managed = &self.windows[id];
                    Some(*id) != current
                        && managed.workspace == self.active
                        && !matches!(managed.window_type, WindowType::Dock | WindowType::Desktop)
                });
                if let Some(id) = next {
                    self.activate(seat, id, now);
                }
            }
        }
        self.apply();
    }

    /// Makes `workspace` the active one, and gives every seat its default focus.
    pub fn switch_workspace(&mut self, workspace: usize) {
        let workspace = workspace.min(self.stacks.len() - 1);
        if workspace != self.active {
            self.active = workspace;
            self.focus.clear();
            if let Some(id) = self.default_focus() {
                for seat in &self.seats {
                    self.focus.insert(*seat, id);
                }
            }
        }
        self.apply();
    }

    /// Moves a window to `workspace`, with every window in its transient tree.
    pub fn move_to_workspace(&mut self, id: WindowId, workspace: usize) {
        if !self.windows.contains_key(&id) {
            return;
        }
        let workspace = workspace.min(self.stacks.len() - 1);
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
        self.switch_workspace(workspace);
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
    fn set_maximized_axes(
        &mut self,
        id: WindowId,
        horizontally: Option<bool>,
        vertically: Option<bool>,
    ) {
        let Some(managed) = self.windows.get_mut(&id) else {
            return;
        };
        let horizontally = horizontally.unwrap_or(managed.maximized_horizontally);
        let vertically = vertically.unwrap_or(managed.maximized_vertically);
        if (managed.maximized_horizontally, managed.maximized_vertically)
            != (horizontally, vertically)
        {
            managed.save_floating();
            if vertically && !managed.maximized_vertically {
                managed.shaded = false;
            }
            managed.maximized_horizontally = horizontally;
            managed.maximized_vertically = vertically;
            self.relayout(id);
        }
        self.apply();
    }

    /// Shades or unshades an SSD window. A CSD window can't be shaded: Camino doesn't
    /// know where its titlebar is (07).
    pub fn set_shaded(&mut self, id: WindowId, shaded: bool) {
        if let Some(managed) = self.windows.get_mut(&id) {
            managed.shaded = shaded && managed.ssd && !managed.fullscreen;
        }
        self.apply();
    }

    /// Enters fullscreen, on `output` if the client named one, or leaves it.
    pub fn set_fullscreen(&mut self, id: WindowId, fullscreen: bool, output: Option<OutputId>) {
        let Some(managed) = self.windows.get_mut(&id) else {
            return;
        };
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
    /// fullscreen, or being moved or resized, as Marco ignores one during a grab
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
        if let Some(output) = self.output_of(frame).or(self.outputs.first()) {
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

    /// Keeps the window above others, or stops. Recorded only until M8 (17).
    pub fn set_above(&mut self, id: WindowId, above: bool) {
        if let Some(managed) = self.windows.get_mut(&id) {
            managed.above = above;
        }
    }

    /// Keeps the window below others, or stops. Recorded only until M8 (17).
    pub fn set_below(&mut self, id: WindowId, below: bool) {
        if let Some(managed) = self.windows.get_mut(&id) {
            managed.below = below;
        }
    }

    /// Shows the window on every workspace, or stops. Recorded only until M8 (17).
    pub fn set_sticky(&mut self, id: WindowId, sticky: bool) {
        if let Some(managed) = self.windows.get_mut(&id) {
            managed.sticky = sticky;
        }
    }

    pub fn minimize(&mut self, id: WindowId) {
        if let Some(managed) = self.windows.get_mut(&id) {
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
        let Some(workspace) = self.workspace_of(id) else {
            return;
        };
        self.set_last_seat(seat);
        let stack = &mut self.stacks[workspace];
        stack.retain(|w| *w != id);
        stack.insert(0, id);
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
        self.begin_grab(seat, id, None, pointer)
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
        self.begin_grab(seat, id, Some(edges), pointer)
    }

    /// The seat's pointer moved during a grab.
    pub fn grab_motion(&mut self, seat: SeatId, pointer: Point<f64, Logical>) {
        let Some(grab) = self.grabs.get(&seat) else {
            return;
        };
        let delta = pointer - grab.start_pointer;
        let start = grab.start;
        let managed = self.windows.get_mut(&grab.window).unwrap();
        let current = managed.geometry;
        match grab.resize {
            None => managed.geometry.loc = (start.loc.to_f64() + delta).to_i32_round(),
            Some(edges) => {
                let (dx, dy) = (delta.x.round() as i32, delta.y.round() as i32);
                let mut size = start.size;
                if edges.left {
                    size.w -= dx;
                } else if edges.right {
                    size.w += dx;
                }
                if edges.top {
                    size.h -= dy;
                } else if edges.bottom {
                    size.h += dy;
                }
                let size = managed.hints.clamp(size);
                let mut loc = start.loc;
                if edges.left {
                    loc.x = start.loc.x + start.size.w - size.w;
                }
                if edges.top {
                    loc.y = start.loc.y + start.size.h - size.h;
                }
                managed.geometry = Rectangle::new(loc, size);
            }
        }
        managed.pin_maximized_axes(current);
        self.apply();
    }

    /// Ends the seat's grab, if it has one.
    pub fn end_grab(&mut self, seat: SeatId) {
        if let Some(grab) = self.grabs.remove(&seat)
            && let Some(managed) = self.windows.get_mut(&grab.window)
        {
            managed.resize = None;
        }
        self.apply();
    }

    /// Ends grabs on windows that can no longer be moved or resized: hidden, maximised
    /// both ways or fullscreen, or shaded during a resize.
    fn end_stale_grabs(&mut self) {
        let stale: Vec<SeatId> = self
            .grabs
            .iter()
            .filter(|(_, g)| {
                let managed = &self.windows[&g.window];
                !self.is_showing(g.window)
                    || managed.maximized()
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

    fn begin_grab(
        &mut self,
        seat: SeatId,
        id: WindowId,
        resize: Option<ResizeEdges>,
        pointer: Point<f64, Logical>,
    ) -> bool {
        let movable = self.is_showing(id)
            && !self.grabs.iter().any(|(s, g)| g.window == id && *s != seat)
            && self.windows.get(&id).is_some_and(|m| {
                !m.maximized()
                    && !m.fullscreen
                    && !(m.shaded && resize.is_some())
                    && !matches!(m.window_type, WindowType::Desktop | WindowType::Dock)
            });
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
            },
        );
        self.apply();
        true
    }

    // Queries.

    pub fn window(&self, id: WindowId) -> Option<&W> {
        self.windows.get(&id).map(|m| &m.window)
    }

    pub fn active_workspace(&self) -> usize {
        self.active
    }

    pub fn outputs(&self) -> &[Output] {
        &self.outputs
    }

    /// A workspace's windows, bottom to top.
    pub fn stacking(&self, workspace: usize) -> &[WindowId] {
        &self.stacks[workspace]
    }

    pub fn focus(&self, seat: SeatId) -> Option<WindowId> {
        self.focus.get(&seat).copied()
    }

    pub fn windows(&self) -> impl Iterator<Item = &W> {
        self.windows.values().map(|m| &m.window)
    }

    pub fn workspace_count(&self) -> usize {
        self.stacks.len()
    }

    /// Each workspace's name, from the first.
    pub fn workspace_names(&self) -> &[String] {
        &self.workspace_names
    }

    pub fn window_mut(&mut self, id: WindowId) -> Option<&mut W> {
        self.windows.get_mut(&id).map(|m| &mut m.window)
    }

    /// The window the seat is moving or resizing.
    pub fn grab(&self, seat: SeatId) -> Option<WindowId> {
        self.grabs.get(&seat).map(|g| g.window)
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

    pub fn is_shaded(&self, id: WindowId) -> bool {
        self.windows.get(&id).is_some_and(|m| m.shaded)
    }

    pub fn is_fullscreen(&self, id: WindowId) -> bool {
        self.windows.get(&id).is_some_and(|m| m.fullscreen)
    }

    pub fn is_minimized(&self, id: WindowId) -> bool {
        self.windows.get(&id).is_some_and(|m| m.minimized)
    }

    /// Whether the client asked to keep the window above others.
    pub fn is_above(&self, id: WindowId) -> bool {
        self.windows.get(&id).is_some_and(|m| m.above)
    }

    /// Whether the client asked to keep the window below others.
    pub fn is_below(&self, id: WindowId) -> bool {
        self.windows.get(&id).is_some_and(|m| m.below)
    }

    /// Whether the client asked to show the window on every workspace.
    pub fn is_sticky(&self, id: WindowId) -> bool {
        self.windows.get(&id).is_some_and(|m| m.sticky)
    }

    pub fn workspace_of(&self, id: WindowId) -> Option<usize> {
        Some(self.windows.get(&id)?.workspace)
    }

    /// Where a window's geometry is, at the size the client last committed.
    pub fn geometry(&self, id: WindowId) -> Option<Rectangle<i32, Logical>> {
        let managed = self.windows.get(&id)?;
        Some(Rectangle::new(managed.geometry.loc, managed.size))
    }

    /// A window's transient parent, as policy resolved it.
    pub fn parent(&self, id: WindowId) -> Option<WindowId> {
        self.windows.get(&id)?.parent
    }

    pub fn layer(&self, id: WindowId) -> Option<Layer> {
        let managed = self.windows.get(&id)?;
        let own = match managed.window_type {
            WindowType::Desktop => Layer::Desktop,
            WindowType::Dock => Layer::Top,
            _ if managed.fullscreen && self.focus.values().any(|f| self.is_ancestor(id, *f)) => {
                Layer::Fullscreen
            }
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
        let workspace = self.windows[&parent].workspace;
        for id in orphans {
            if !self.is_ancestor(id, parent) {
                self.windows.get_mut(&id).unwrap().parent = Some(parent);
                self.move_tree(id, workspace);
            }
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

    /// Moves a window and its transient descendants to `workspace`, keeping their order.
    fn move_tree(&mut self, id: WindowId, workspace: usize) {
        let tree = self.tree(id);
        for stack in &mut self.stacks {
            stack.retain(|w| !tree.contains(w));
        }
        for w in &tree {
            self.windows.get_mut(w).unwrap().workspace = workspace;
        }
        self.stacks[workspace].extend(&tree);
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

    /// The part of an output windows are placed and maximised in. Struts come later.
    fn work_area(output: &Output) -> Rectangle<i32, Logical> {
        output.geometry
    }

    /// Sets the geometry of a maximised or fullscreen window from its output, or restores a
    /// floating window's. A window maximised one way keeps its floating geometry the
    /// other way.
    fn relayout(&mut self, id: WindowId) {
        let managed = &self.windows[&id];
        if !managed.constrained() {
            let managed = self.windows.get_mut(&id).unwrap();
            if let Some(floating) = managed.floating.take() {
                managed.geometry = floating;
            }
            return;
        }
        let output = managed
            .fullscreen_output
            .and_then(|o| self.outputs.iter().find(|output| output.id == o))
            .or_else(|| self.output_of(managed.frame()))
            .or(self.outputs.first());
        let Some(output) = output else {
            return;
        };
        let geometry = if managed.fullscreen {
            output.geometry
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
        self.windows.get_mut(&id).unwrap().geometry = geometry;
    }

    /// After outputs change: moves every window that's on no output onto the first one, and
    /// lays out maximised and fullscreen windows again.
    fn rehome_windows(&mut self) {
        let Some(first) = self.outputs.first() else {
            return;
        };
        let area = Self::work_area(first);
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

    /// Marco maximises a framed window that's placed at least as big as its work area.
    fn maximize_if_too_big(&mut self, id: WindowId) {
        let managed = &self.windows[&id];
        let frame = managed.frame();
        let Some(output) = self.output_of(frame).or(self.outputs.first()) else {
            return;
        };
        let area = Self::work_area(output);
        if managed.ssd && frame.size.w >= area.size.w && frame.size.h >= area.size.h {
            let managed = self.windows.get_mut(&id).unwrap();
            managed.save_floating();
            managed.maximized_horizontally = true;
            managed.maximized_vertically = true;
            self.relayout(id);
        }
    }

    /// Places a newly mapped window with Marco's automatic placement.
    fn place(&mut self, id: WindowId) {
        let managed = &self.windows[&id];
        let e = managed.visible_extents();
        let (left, top) = (e.left, e.top);
        let frame_size = managed.frame().size;
        let size = managed.size;

        let loc = if let Some(requested) = managed.window.requested_position(e) {
            // Marco honours USPosition and PPosition with its workarounds on (the default).
            requested
        } else if let (WindowType::Dialog | WindowType::ModalDialog, Some(parent)) =
            (managed.window_type, managed.parent)
        {
            // Centred over the parent, leaving twice as much space below as above.
            let parent_frame = self.windows[&parent].frame();
            let parent = self.geometry(parent).unwrap();
            let loc = Point::new(
                parent.loc.x + parent.size.w / 2 - size.w / 2,
                parent.loc.y + (parent.size.h - size.h) / 3 + top,
            );
            match self.output_of(parent_frame) {
                Some(output) => {
                    let frame = Rectangle::new((loc.x - left, loc.y - top).into(), frame_size);
                    let frame_loc = placement::constrain(frame, Self::work_area(output));
                    Point::new(frame_loc.x + left, frame_loc.y + top)
                }
                None => loc,
            }
        } else {
            let output = self
                .focus(self.last_seat)
                .and_then(|f| self.output_of(self.windows[&f].frame()))
                .or(self.outputs.first());
            match output {
                None => Point::new(left, top),
                Some(output) => self.place_on(id, output, frame_size, (left, top)),
            }
        };
        self.windows.get_mut(&id).unwrap().geometry.loc = loc;
    }

    /// Places a window without a parent to centre on, on `output`.
    fn place_on(
        &self,
        id: WindowId,
        output: &Output,
        frame_size: Size<i32, Logical>,
        (left, top): (i32, i32),
    ) -> Point<i32, Logical> {
        let managed = &self.windows[&id];
        if matches!(
            managed.window_type,
            WindowType::Dialog | WindowType::ModalDialog | WindowType::Splashscreen
        ) {
            // Marco centres the window geometry, not the frame, on the output.
            let area = output.geometry;
            return Point::new(
                area.loc.x + (area.size.w - managed.size.w) / 2,
                area.loc.y + (area.size.h - managed.size.h) / 2,
            );
        }

        let area = Self::work_area(output);
        let others: Vec<Obstacle> = self.stacks[managed.workspace]
            .iter()
            .filter(|other| **other != id && self.is_visible_on_workspace(**other))
            .map(|other| &self.windows[other])
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
        let frame_loc = placement::first_fit(frame_size, area, &others)
            .unwrap_or_else(|| placement::cascade(frame_size, (left, top), area, &others));
        let frame_loc = placement::constrain(Rectangle::new(frame_loc, frame_size), area);
        Point::new(frame_loc.x + left, frame_loc.y + top)
    }

    // Focus and stacking.

    fn set_last_seat(&mut self, seat: SeatId) {
        self.seats.insert(seat);
        self.last_seat = seat;
    }

    /// Focuses `id` for `seat` on user input at `now`. Returns false, changing nothing, if the
    /// request is older than the seat's last focus change.
    fn focus_window(&mut self, seat: SeatId, id: WindowId, now: Duration) -> bool {
        if self.focus_time.get(&seat).is_some_and(|last| now < *last) {
            return false;
        }
        self.seats.insert(seat);
        self.focus_time.insert(seat, now);
        self.focus.insert(seat, id);
        true
    }

    fn is_desktop_or_dock(&self, id: WindowId) -> bool {
        matches!(
            self.windows[&id].window_type,
            WindowType::Desktop | WindowType::Dock
        )
    }

    /// Whether a window is on screen: managed, and shown on the active workspace.
    pub fn is_showing(&self, id: WindowId) -> bool {
        self.windows
            .get(&id)
            .is_some_and(|m| m.workspace == self.active)
            && self.is_visible_on_workspace(id)
    }

    /// Whether a window would show if its workspace were active: neither it nor a transient
    /// ancestor is minimised.
    fn is_visible_on_workspace(&self, id: WindowId) -> bool {
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
    /// the active workspace's stack that isn't a dock, or failing that the desktop.
    fn default_focus(&self) -> Option<WindowId> {
        let candidates = || {
            self.stacks[self.active]
                .iter()
                .rev()
                .copied()
                .filter(|id| self.is_showing(*id))
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

    /// Moves a window to the top of its workspace's stack.
    fn move_to_top(&mut self, id: WindowId) {
        let stack = &mut self.stacks[self.windows[&id].workspace];
        stack.retain(|w| *w != id);
        stack.push(id);
    }

    // Applying decisions.

    /// Sends each window whatever policy now wants of it that it hasn't been sent.
    fn apply(&mut self) {
        self.end_stale_grabs();
        self.fix_focus();
        self.mru.retain(|id| self.windows.contains_key(id));
        if let Some(focused) = self.focus(self.last_seat) {
            self.mru.retain(|id| *id != focused);
            self.mru.insert(0, focused);
        }
        let mut stacks = std::mem::take(&mut self.stacks);
        for stack in &mut stacks {
            stack::restack(stack, |id| self.layer(id).unwrap(), |id| self.parent(id));
        }
        self.stacks = stacks;

        let focused: Vec<WindowId> = self.focus.values().copied().collect();
        // A window's frame extents can change with its focus (ADR 0012).
        let refocused: Vec<WindowId> = self
            .windows
            .iter_mut()
            .filter_map(|(id, managed)| {
                let activated = focused.contains(id);
                let changed = managed.activated != activated;
                managed.activated = activated;
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
