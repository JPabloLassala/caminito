//! Random operation sequences, with policy's invariants checked after every operation, in the
//! shape of niri's `check_ops` and `verify_invariants`.

use proptest::prelude::*;

use super::*;
use smithay::utils::Size;

use crate::{
    Crossing, FocusMode, GrabKey, GrabStep, MenuKey, Overview, PlacementMode, PointerTarget,
    PolicyWindow, PreMapWindow, ResizeEdges, SwitchEntry, SwitchList, WrapStyle,
};

const SEATS: u32 = 2;

#[derive(Debug, Clone)]
enum Op {
    AddOutput {
        id: u64,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    },
    RemoveOutput(u64),
    /// An output of `w` × `h` is plugged in, and the compositor lays every output out left
    /// to right again, as Caminito does (51).
    Plug {
        id: u64,
        w: i32,
        h: i32,
    },
    /// An output is unplugged, and the rest are laid out left to right again.
    Unplug(u64),
    /// A panel or a dock maps, changes or goes: an output's reserved edges change (64).
    Reserve {
        id: u64,
        reserved: ReservedEdges,
    },
    Map {
        window: TestWindow,
    },
    Unmap(u64),
    Commit {
        id: u64,
        w: i32,
        h: i32,
        parent: Option<u64>,
        window_type: WindowType,
    },
    Click {
        seat: u32,
        id: u64,
        stale: bool,
    },
    Action {
        seat: u32,
        action: Action,
    },
    SwitchWorkspace(usize),
    /// One output's switch, which switches it alone with `workspaces-per-output` on (230).
    SwitchOutputWorkspace {
        output: u64,
        workspace: usize,
    },
    /// `workspaces-per-output` is turned on or off (230).
    SetWorkspacesPerOutput(bool),
    MoveToWorkspace {
        id: u64,
        workspace: usize,
    },
    SetMaximized {
        id: u64,
        maximized: bool,
    },
    SetFullscreen {
        id: u64,
        fullscreen: bool,
        output: Option<u64>,
    },
    SetMaximizedHorizontally {
        id: u64,
        maximized: bool,
    },
    SetMaximizedVertically {
        id: u64,
        maximized: bool,
    },
    SetShaded {
        id: u64,
        shaded: bool,
    },
    /// The client changes decoration mode.
    SetSsd {
        id: u64,
        ssd: bool,
    },
    Lower {
        seat: u32,
        id: u64,
    },
    Minimize(u64),
    Activate {
        seat: u32,
        id: u64,
    },
    BeginMove {
        seat: u32,
        id: u64,
    },
    BeginResize {
        seat: u32,
        id: u64,
        top_left: bool,
    },
    GrabMotion {
        seat: u32,
        /// Shift held: snap-move instead of resisting (157).
        snap: bool,
        x: i16,
        y: i16,
    },
    EndGrab(u32),
    /// A key of a keyboard move or resize (156).
    GrabKey {
        seat: u32,
        key: GrabKey,
    },
    SetWorkspaceCount(usize),
    /// `dynamic-workspaces` (151), with Marco's four workspaces for when it's off.
    SetDynamicWorkspaces(bool),
    /// The grid's rows and `wrap-style` (150), keeping the count.
    SetWorkspaceGrid {
        rows: usize,
        wrap: WrapStyle,
    },
    RequestGeometry {
        id: u64,
        loc: Option<(i32, i32)>,
        size: Option<(i32, i32)>,
    },
    SetAbove {
        id: u64,
        above: bool,
    },
    SetBelow {
        id: u64,
        below: bool,
    },
    SetSticky {
        id: u64,
        sticky: bool,
    },
    SetShowingDesktop(bool),
    SwitcherStep(bool),
    SwitcherSelect(usize),
    SwitcherCommit,
    SwitcherCancel,
    /// The Window menu (161) opens some other way than by its binding.
    OpenWindowMenu {
        seat: u32,
        id: u64,
    },
    WindowMenuKey(MenuKey),
    CloseWindowMenu,
    /// The compositor asks for the Workspace popup (152), and says that it closed.
    OpenWorkspacePopup {
        moves: bool,
    },
    CloseWorkspacePopup,
    /// A click on a window of the Overview (241), and what closes it.
    OverviewChoose(u64),
    CloseOverview,
    /// An arrow key in the Overview, Enter, and a middle click on a window (242).
    OverviewSelect(crate::Direction),
    OverviewEnter,
    OverviewCloseWindow(u64),
    /// A click on a workspace of the Overview's bar (244).
    OverviewSwitch(usize),
    /// The left button down on a window of the Overview at (x, y), the pointer's move to
    /// (x, y) over a cell or none, the release, and Esc during a drag (245).
    OverviewPress(u64, (i16, i16)),
    OverviewPointer((i16, i16), Option<usize>),
    /// A click on a workspace of an output's bar, and the pointer's move onto an output and
    /// a cell of its bar or none, during a drag (246).
    OverviewSwitchOn(u64, usize),
    OverviewPointerOn((i16, i16), Option<u64>, Option<usize>),
    OverviewRelease,
    OverviewEndDrag,
    /// The settings change to a focus mode, `raise-on-click` and auto-raise (153).
    SetFocusMode {
        mode: FocusMode,
        raise_on_click: bool,
        auto_raise: bool,
    },
    PointerEntered {
        seat: u32,
        target: PointerTarget,
        crossing: Crossing,
    },
    TitlebarReleased(u64),
    /// Time passes, and policy runs what's due.
    Elapse(u64),
}

fn window_id() -> impl Strategy<Value = u64> {
    1..=8u64
}

fn seat() -> impl Strategy<Value = u32> {
    0..SEATS
}

fn window_type() -> impl Strategy<Value = WindowType> {
    prop_oneof![
        6 => Just(WindowType::Normal),
        2 => Just(WindowType::Dialog),
        1 => Just(WindowType::ModalDialog),
        1 => Just(WindowType::Utility),
        1 => Just(WindowType::Toolbar),
        1 => Just(WindowType::Menu),
        1 => Just(WindowType::Dock),
        1 => Just(WindowType::Desktop),
        1 => Just(WindowType::Splashscreen),
    ]
}

fn test_window() -> impl Strategy<Value = TestWindow> {
    (
        window_id(),
        1..2500i32,
        1..1500i32,
        window_type(),
        proptest::option::of(window_id()),
        any::<bool>(),
        proptest::option::of((-500..3500i32, -500..2000i32)),
        any::<bool>(),
        // Past the most workspaces there are, as a client may ask.
        proptest::option::of(0..40usize),
    )
        .prop_map(
            |(id, w, h, window_type, parent, ssd, requested, input, workspace)| {
                let mut window = TestWindow::new(id, w, h).with_type(window_type);
                window.input = input;
                window.parent = parent.map(WindowId);
                if ssd {
                    // A theme whose extents depend on the state, as eOS's do.
                    window = window
                        .with_frame(5, 5, 25, 5)
                        .with_unfocused_border(1)
                        .with_shaded_top(20);
                    window.maximized_extents = smithay::utils::FrameExtents::new(0, 0, 22, 0);
                }
                if let Some((x, y)) = requested {
                    window = window.with_requested_position(x, y);
                }
                window.requested_workspace = workspace;
                window
            },
        )
}

fn action() -> impl Strategy<Value = Action> {
    prop_oneof![
        Just(Action::SwitchToWorkspaceLeft),
        Just(Action::SwitchToWorkspaceRight),
        Just(Action::SwitchToWorkspaceUp),
        Just(Action::SwitchToWorkspaceDown),
        Just(Action::SwitchToWorkspacePrev),
        Just(Action::MoveToWorkspaceUp),
        Just(Action::MoveToWorkspaceDown),
        Just(Action::Close),
        Just(Action::ToggleMaximized),
        (0..6usize).prop_map(Action::SwitchToWorkspace),
        (0..6usize).prop_map(Action::MoveToWorkspace),
        Just(Action::MoveToWorkspaceLeft),
        Just(Action::MoveToWorkspaceRight),
        Just(Action::Maximize),
        Just(Action::Unmaximize),
        Just(Action::ToggleMaximizedHorizontally),
        Just(Action::ToggleMaximizedVertically),
        Just(Action::ToggleFullscreen),
        Just(Action::ToggleShaded),
        Just(Action::Minimize),
        Just(Action::Raise),
        Just(Action::Lower),
        Just(Action::RaiseOrLower),
        (switch_list(), any::<bool>())
            .prop_map(|(list, backward)| Action::Switch { list, backward }),
        (switch_list(), any::<bool>())
            .prop_map(|(list, backward)| Action::Cycle { list, backward }),
        Just(Action::ShowDesktop),
        tile().prop_map(Action::Tile),
        anchor().prop_map(Action::MoveTo),
        direction().prop_map(Action::MoveToMonitor),
        Just(Action::BeginMove),
        Just(Action::BeginResize),
        Just(Action::ActivateWindowMenu),
        Just(Action::Overview),
    ]
}

fn tile() -> impl Strategy<Value = crate::Tile> {
    use crate::Tile;
    prop_oneof![
        Just(Tile::Left),
        Just(Tile::Right),
        Just(Tile::TopLeft),
        Just(Tile::TopRight),
        Just(Tile::BottomLeft),
        Just(Tile::BottomRight),
    ]
}

fn anchor() -> impl Strategy<Value = crate::Anchor> {
    use crate::Anchor;
    prop_oneof![
        Just(Anchor::NorthWest),
        Just(Anchor::North),
        Just(Anchor::NorthEast),
        Just(Anchor::West),
        Just(Anchor::Centre),
        Just(Anchor::East),
        Just(Anchor::SouthWest),
        Just(Anchor::South),
        Just(Anchor::SouthEast),
    ]
}

fn direction() -> impl Strategy<Value = crate::Direction> {
    use crate::Direction;
    prop_oneof![
        Just(Direction::Left),
        Just(Direction::Right),
        Just(Direction::Up),
        Just(Direction::Down),
    ]
}

fn grab_key() -> impl Strategy<Value = GrabKey> {
    let step = prop_oneof![
        Just(GrabStep::Normal),
        Just(GrabStep::Fine),
        Just(GrabStep::Snap)
    ];
    prop_oneof![
        6 => (-1..=1i32, -1..=1i32, step).prop_map(|(x, y, step)| GrabKey::Arrow { x, y, step }),
        1 => Just(GrabKey::Escape),
        1 => Just(GrabKey::Other),
    ]
}

fn switch_list() -> impl Strategy<Value = SwitchList> {
    prop_oneof![
        Just(SwitchList::Windows),
        Just(SwitchList::WindowsAll),
        Just(SwitchList::Group),
        Just(SwitchList::Panels),
    ]
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        1 => (1..=3u64, -2000..4000i32, -2000..2000i32, 1..4000i32, 1..3000i32)
            .prop_map(|(id, x, y, w, h)| Op::AddOutput { id, x, y, w, h }),
        1 => (1..=3u64).prop_map(Op::RemoveOutput),
        1 => (1..=3u64, 1..4000i32, 1..3000i32).prop_map(|(id, w, h)| Op::Plug { id, w, h }),
        1 => (1..=3u64).prop_map(Op::Unplug),
        // Past the output's size too, as a client may ask.
        2 => (1..=3u64, 0..300i32, 0..300i32, 0..5000i32, 0..300i32).prop_map(
            |(id, left, right, top, bottom)| Op::Reserve {
                id,
                reserved: ReservedEdges { left, right, top, bottom },
            }
        ),
        3 => test_window().prop_map(|window| Op::Map { window }),
        2 => window_id().prop_map(Op::Unmap),
        1 => (window_id(), 1..2500i32, 1..1500i32, proptest::option::of(window_id()), window_type())
            .prop_map(|(id, w, h, parent, window_type)| Op::Commit { id, w, h, parent, window_type }),
        2 => (seat(), window_id(), any::<bool>())
            .prop_map(|(seat, id, stale)| Op::Click { seat, id, stale }),
        1 => (seat(), action()).prop_map(|(seat, action)| Op::Action { seat, action }),
        1 => (0..6usize).prop_map(Op::SwitchWorkspace),
        2 => (1..=3u64, 0..6usize)
            .prop_map(|(output, workspace)| Op::SwitchOutputWorkspace { output, workspace }),
        1 => any::<bool>().prop_map(Op::SetWorkspacesPerOutput),
        1 => (window_id(), 0..6usize).prop_map(|(id, workspace)| Op::MoveToWorkspace { id, workspace }),
        1 => (window_id(), any::<bool>())
            .prop_map(|(id, maximized)| Op::SetMaximized { id, maximized }),
        1 => (window_id(), any::<bool>(), proptest::option::of(1..=4u64))
            .prop_map(|(id, fullscreen, output)| Op::SetFullscreen { id, fullscreen, output }),
        1 => (window_id(), any::<bool>())
            .prop_map(|(id, maximized)| Op::SetMaximizedHorizontally { id, maximized }),
        1 => (window_id(), any::<bool>())
            .prop_map(|(id, maximized)| Op::SetMaximizedVertically { id, maximized }),
        2 => (window_id(), any::<bool>()).prop_map(|(id, shaded)| Op::SetShaded { id, shaded }),
        1 => (window_id(), any::<bool>()).prop_map(|(id, ssd)| Op::SetSsd { id, ssd }),
        1 => (seat(), window_id()).prop_map(|(seat, id)| Op::Lower { seat, id }),
        1 => window_id().prop_map(Op::Minimize),
        1 => (seat(), window_id()).prop_map(|(seat, id)| Op::Activate { seat, id }),
        1 => (seat(), window_id()).prop_map(|(seat, id)| Op::BeginMove { seat, id }),
        1 => (seat(), window_id(), any::<bool>())
            .prop_map(|(seat, id, top_left)| Op::BeginResize { seat, id, top_left }),
        1 => (seat(), any::<i16>(), any::<i16>(), any::<bool>())
            .prop_map(|(seat, x, y, snap)| Op::GrabMotion { seat, snap, x, y }),
        1 => seat().prop_map(Op::EndGrab),
        2 => (seat(), grab_key()).prop_map(|(seat, key)| Op::GrabKey { seat, key }),
        1 => (1..=6usize).prop_map(Op::SetWorkspaceCount),
        1 => any::<bool>().prop_map(Op::SetDynamicWorkspaces),
        1 => (
            0..=3usize,
            prop_oneof![
                Just(WrapStyle::NoWrap),
                Just(WrapStyle::Classic),
                Just(WrapStyle::Toroidal)
            ]
        )
            .prop_map(|(rows, wrap)| Op::SetWorkspaceGrid { rows, wrap }),
        1 => (
            window_id(),
            proptest::option::of((-500..3500i32, -500..2000i32)),
            proptest::option::of((1..2500i32, 1..1500i32)),
        )
            .prop_map(|(id, loc, size)| Op::RequestGeometry { id, loc, size }),
        1 => (window_id(), any::<bool>()).prop_map(|(id, above)| Op::SetAbove { id, above }),
        1 => (window_id(), any::<bool>()).prop_map(|(id, below)| Op::SetBelow { id, below }),
        1 => (window_id(), any::<bool>()).prop_map(|(id, sticky)| Op::SetSticky { id, sticky }),
        1 => any::<bool>().prop_map(Op::SetShowingDesktop),
        1 => any::<bool>().prop_map(Op::SwitcherStep),
        1 => (0..10usize).prop_map(Op::SwitcherSelect),
        1 => Just(Op::SwitcherCommit),
        1 => Just(Op::SwitcherCancel),
        2 => (seat(), window_id()).prop_map(|(seat, id)| Op::OpenWindowMenu { seat, id }),
        2 => menu_key().prop_map(Op::WindowMenuKey),
        1 => Just(Op::CloseWindowMenu),
        2 => any::<bool>().prop_map(|moves| Op::OpenWorkspacePopup { moves }),
        1 => Just(Op::CloseWorkspacePopup),
        2 => window_id().prop_map(Op::OverviewChoose),
        1 => Just(Op::CloseOverview),
        2 => direction().prop_map(Op::OverviewSelect),
        1 => Just(Op::OverviewEnter),
        1 => window_id().prop_map(Op::OverviewCloseWindow),
        1 => (0..5usize).prop_map(Op::OverviewSwitch),
        2 => (window_id(), any::<(i16, i16)>()).prop_map(|(id, at)| Op::OverviewPress(id, at)),
        2 => (any::<(i16, i16)>(), proptest::option::of(0..5usize))
            .prop_map(|(at, target)| Op::OverviewPointer(at, target)),
        1 => (1..=3u64, 0..5usize).prop_map(|(output, workspace)| Op::OverviewSwitchOn(output, workspace)),
        2 => (any::<(i16, i16)>(), proptest::option::of(1..=3u64), proptest::option::of(0..5usize))
            .prop_map(|(at, onto, target)| Op::OverviewPointerOn(at, onto, target)),
        1 => Just(Op::OverviewRelease),
        1 => Just(Op::OverviewEndDrag),
        1 => (focus_mode(), any::<bool>(), any::<bool>()).prop_map(|(mode, raise_on_click, auto_raise)| {
            Op::SetFocusMode { mode, raise_on_click, auto_raise }
        }),
        3 => (seat(), pointer_target(), crossing())
            .prop_map(|(seat, target, crossing)| Op::PointerEntered { seat, target, crossing }),
        1 => window_id().prop_map(Op::TitlebarReleased),
        1 => (0..1000u64).prop_map(Op::Elapse),
    ]
}

fn menu_key() -> impl Strategy<Value = MenuKey> {
    prop_oneof![
        3 => Just(MenuKey::Down),
        1 => Just(MenuKey::Up),
        1 => Just(MenuKey::Home),
        1 => Just(MenuKey::End),
        1 => Just(MenuKey::Left),
        1 => Just(MenuKey::Right),
        3 => Just(MenuKey::Activate),
        1 => Just(MenuKey::Escape),
    ]
}

fn focus_mode() -> impl Strategy<Value = FocusMode> {
    prop_oneof![
        Just(FocusMode::Click),
        Just(FocusMode::Sloppy),
        Just(FocusMode::Mouse),
    ]
}

fn pointer_target() -> impl Strategy<Value = PointerTarget> {
    prop_oneof![
        4 => window_id().prop_map(|id| PointerTarget::Window(WindowId(id))),
        1 => Just(PointerTarget::Desktop),
        1 => Just(PointerTarget::Other),
    ]
}

fn crossing() -> impl Strategy<Value = Crossing> {
    prop_oneof![Just(Crossing::Motion), Just(Crossing::Scene)]
}

impl Op {
    fn apply(self, policy: &mut Policy<TestWindow>, step: u64) {
        let now = ms(1000 + step);
        let pointer = |x: i16, y: i16| Point::from((f64::from(x), f64::from(y)));
        match self {
            Op::AddOutput { id, x, y, w, h } => policy.add_output(output(id, x, y, w, h)),
            Op::RemoveOutput(id) => policy.remove_output(OutputId(id)),
            Op::Plug { id, w, h } => {
                let mut sizes = sizes_without(policy, id);
                sizes.push((id, w, h));
                policy.set_outputs(left_to_right(sizes));
            }
            Op::Unplug(id) => policy.set_outputs(left_to_right(sizes_without(policy, id))),
            Op::Reserve { id, reserved } => {
                let mut outputs = policy.outputs().to_vec();
                if let Some(output) = outputs.iter_mut().find(|o| o.id == OutputId(id)) {
                    output.reserved = reserved;
                }
                policy.set_outputs(outputs);
            }
            Op::Map { window } => {
                // The compositor never maps an id twice.
                if policy.window(window.id).is_none() {
                    policy.map_window(window, now);
                }
            }
            Op::Unmap(id) => drop(policy.unmap_window(WindowId(id))),
            Op::Commit {
                id,
                w,
                h,
                parent,
                window_type,
            } => {
                if let Some(window) = policy.window_mut(WindowId(id)) {
                    window.size = (w, h).into();
                    window.parent = parent.map(WindowId);
                    window.window_type = window_type;
                    policy.commit(WindowId(id));
                }
            }
            Op::Click { seat, id, stale } => {
                let now = if stale { ms(step) } else { now };
                policy.click(SeatId(seat), WindowId(id), now);
            }
            Op::Action { seat, action } => policy.action(SeatId(seat), action, now),
            Op::SwitchWorkspace(workspace) => policy.switch_workspace(workspace),
            Op::SwitchOutputWorkspace { output, workspace } => {
                policy.switch_output_workspace(OutputId(output), workspace)
            }
            Op::SetWorkspacesPerOutput(workspaces_per_output) => policy.settings_changed(Config {
                workspace_count: policy.workspace_count(),
                workspaces_per_output,
                ..Config::default()
            }),
            Op::MoveToWorkspace { id, workspace } => {
                policy.move_to_workspace(WindowId(id), workspace)
            }
            Op::SetMaximized { id, maximized } => policy.set_maximized(WindowId(id), maximized),
            Op::SetFullscreen {
                id,
                fullscreen,
                output,
            } => policy.set_fullscreen(WindowId(id), fullscreen, output.map(OutputId)),
            Op::SetMaximizedHorizontally { id, maximized } => {
                policy.set_maximized_horizontally(WindowId(id), maximized)
            }
            Op::SetMaximizedVertically { id, maximized } => {
                policy.set_maximized_vertically(WindowId(id), maximized)
            }
            Op::SetShaded { id, shaded } => policy.set_shaded(WindowId(id), shaded),
            Op::SetSsd { id, ssd } => {
                if let Some(window) = policy.window_mut(WindowId(id)) {
                    window.ssd = ssd;
                    policy.frame_changed(WindowId(id));
                }
            }
            Op::Lower { seat, id } => policy.lower(SeatId(seat), WindowId(id), now),
            Op::Minimize(id) => policy.minimize(WindowId(id)),
            Op::Activate { seat, id } => policy.activate(SeatId(seat), WindowId(id), now),
            Op::BeginMove { seat, id } => {
                policy.begin_move(SeatId(seat), WindowId(id), pointer(0, 0));
            }
            Op::BeginResize { seat, id, top_left } => {
                let edges = if top_left {
                    ResizeEdges::TOP_LEFT
                } else {
                    ResizeEdges::BOTTOM_RIGHT
                };
                policy.begin_resize(SeatId(seat), WindowId(id), edges, pointer(0, 0));
            }
            Op::GrabMotion { seat, snap, x, y } => {
                policy.grab_motion(SeatId(seat), pointer(x, y), snap)
            }
            Op::EndGrab(seat) => policy.end_grab(SeatId(seat)),
            Op::GrabKey { seat, key } => policy.grab_key(SeatId(seat), key),
            Op::SetWorkspaceCount(workspace_count) => policy.settings_changed(Config {
                workspace_count,
                workspace_names: Vec::new(),
                ..keeping_the_key(policy)
            }),
            Op::SetDynamicWorkspaces(dynamic_workspaces) => policy.settings_changed(Config {
                dynamic_workspaces,
                ..keeping_the_key(policy)
            }),
            Op::SetWorkspaceGrid { rows, wrap } => policy.settings_changed(Config {
                workspace_count: policy.workspace_count(),
                workspace_layout: WorkspaceLayout {
                    rows,
                    ..WorkspaceLayout::default()
                },
                wrap_style: wrap,
                ..keeping_the_key(policy)
            }),
            Op::RequestGeometry { id, loc, size } => {
                policy.request_geometry(WindowId(id), loc.map(Point::from), size.map(Size::from))
            }
            Op::SetAbove { id, above } => policy.set_above(WindowId(id), above),
            Op::SetBelow { id, below } => policy.set_below(WindowId(id), below),
            Op::SetSticky { id, sticky } => policy.set_sticky(WindowId(id), sticky),
            Op::SetShowingDesktop(showing) => policy.set_showing_desktop(showing),
            Op::SwitcherStep(backward) => policy.switcher_step(backward),
            Op::SwitcherSelect(index) => policy.switcher_select(index),
            Op::SwitcherCommit => policy.switcher_commit(now),
            Op::SwitcherCancel => policy.switcher_cancel(now),
            Op::OpenWindowMenu { seat, id } => {
                policy.open_window_menu(SeatId(seat), WindowId(id));
            }
            Op::WindowMenuKey(key) => policy.window_menu_key(key, now),
            Op::CloseWindowMenu => policy.close_window_menu(),
            Op::OpenWorkspacePopup { moves } => {
                policy.open_workspace_popup(moves);
            }
            Op::CloseWorkspacePopup => policy.close_workspace_popup(),
            Op::OverviewChoose(id) => policy.overview_choose(WindowId(id), now),
            Op::CloseOverview => policy.close_overview(),
            Op::OverviewSelect(direction) => {
                // The listed windows on rows of three, as a layout might have them.
                let count = policy.overview().map_or(0, |open| open.windows.len());
                let places: Vec<_> = (0..count as i32)
                    .map(|n| Rectangle::new((n % 3 * 200, n / 3 * 200).into(), (100, 100).into()))
                    .collect();
                policy.overview_select(direction, &places);
            }
            Op::OverviewEnter => policy.overview_enter(now),
            Op::OverviewCloseWindow(id) => policy.overview_close_window(WindowId(id)),
            Op::OverviewSwitch(workspace) => policy.overview_switch(workspace),
            Op::OverviewPress(id, (x, y)) => {
                let at = Point::from((f64::from(x), f64::from(y)));
                policy.overview_press(WindowId(id), at);
            }
            Op::OverviewPointer((x, y), target) => {
                policy.overview_pointer(Point::from((f64::from(x), f64::from(y))), target);
            }
            Op::OverviewSwitchOn(output, workspace) => {
                policy.overview_switch_on(OutputId(output), workspace);
            }
            Op::OverviewPointerOn((x, y), onto, target) => {
                let at = Point::from((f64::from(x), f64::from(y)));
                policy.overview_pointer_on(at, onto.map(OutputId), target);
            }
            Op::OverviewRelease => policy.overview_release(now),
            Op::OverviewEndDrag => {
                policy.overview_end_drag();
            }
            Op::SetFocusMode {
                mode,
                raise_on_click,
                auto_raise,
            } => policy.settings_changed(Config {
                workspace_count: policy.workspace_count(),
                focus_mode: mode,
                raise_on_click,
                auto_raise,
                auto_raise_delay: ms(200),
                ..keeping_the_key(policy)
            }),
            Op::PointerEntered {
                seat,
                target,
                crossing,
            } => policy.pointer_entered(SeatId(seat), target, crossing, now),
            Op::TitlebarReleased(id) => policy.titlebar_released(WindowId(id)),
            Op::Elapse(by) => policy.elapse(now + ms(by)),
        }
    }
}

/// The default settings, but for `workspaces-per-output`, which stays as it is.
fn keeping_the_key(policy: &Policy<TestWindow>) -> Config {
    Config {
        workspaces_per_output: policy.workspaces_per_output(),
        ..Config::default()
    }
}

/// Each output's id and size, but for `id`'s.
fn sizes_without(policy: &Policy<TestWindow>, id: u64) -> Vec<(u64, i32, i32)> {
    policy
        .outputs()
        .iter()
        .map(|o| (o.id.0, o.geometry.size.w, o.geometry.size.h))
        .filter(|(existing, _, _)| *existing != id)
        .collect()
}

/// Outputs of these ids and sizes side by side from the origin, in order.
fn left_to_right(sizes: Vec<(u64, i32, i32)>) -> Vec<Output> {
    let mut x = 0;
    sizes
        .into_iter()
        .map(|(id, w, h)| {
            let placed = output(id, x, 0, w, h);
            x += w;
            placed
        })
        .collect()
}

fn verify_invariants(policy: &Policy<TestWindow>) {
    let windows: Vec<WindowId> = policy.windows().map(|w| w.id).collect();
    let stacks: Vec<Vec<WindowId>> = (0..policy.workspace_count())
        .map(|ws| policy.stacking(ws))
        .collect();

    // The stack holds every managed window once. A sticky window is on every workspace,
    // and any other on exactly one: the one policy says it's on. A transient family is on
    // one workspace, or on all. The active workspace is one of them.
    assert!(
        policy.active_workspace() < stacks.len(),
        "the active workspace exists"
    );
    for output in policy.outputs() {
        assert!(
            policy.active_workspace_on(output.id) < stacks.len(),
            "{:?}'s active workspace exists",
            output.id
        );
    }
    // With `workspaces-per-output` off, every output has the same active workspace, through
    // switches, outputs coming and going, dynamic workspaces, a change of the count and the
    // key turned off: every switch sets them all (221, 230), and an output added takes the
    // one the others have. `active_workspace` is the primary output's.
    if !policy.workspaces_per_output() {
        for output in policy.outputs() {
            assert_eq!(
                policy.active_workspace_on(output.id),
                policy.active_workspace(),
                "{:?}'s active workspace is every output's",
                output.id
            );
        }
    }
    let mut all = policy.stacking_all().to_vec();
    all.sort();
    let mut managed = windows.clone();
    managed.sort();
    assert_eq!(all, managed, "the stack holds every window once");
    for &id in &windows {
        let on: Vec<usize> = (0..stacks.len())
            .filter(|ws| stacks[*ws].contains(&id))
            .collect();
        let workspace = policy.workspace_of(id).unwrap();
        assert!(workspace < stacks.len(), "{id:?}'s workspace exists");
        if policy.is_sticky(id) {
            assert_eq!(
                on.len(),
                stacks.len(),
                "sticky {id:?} is on every workspace"
            );
        } else {
            assert_eq!(on, [workspace], "{id:?} should be on exactly one workspace");
        }
        if let Some(parent) = policy.parent(id) {
            assert_eq!(
                policy.is_sticky(id),
                policy.is_sticky(parent),
                "{id:?}'s family"
            );
            assert_eq!(
                policy.workspace_of(parent),
                Some(workspace),
                "{id:?}'s family"
            );
        }
    }

    for (ws, stack) in stacks.iter().enumerate() {
        let position = |id: WindowId| stack.iter().position(|w| *w == id);
        for &id in stack {
            // Transients are above their parents.
            if let Some(parent) = policy.parent(id) {
                assert!(
                    position(parent) < position(id) && position(parent).is_some(),
                    "{id:?} should be above its parent {parent:?} on workspace {ws}"
                );
            }
        }
        // Layers never decrease going up.
        let layers: Vec<Layer> = stack.iter().map(|id| policy.layer(*id).unwrap()).collect();
        assert!(
            layers.is_sorted(),
            "layers are in order on workspace {ws}: {layers:?}"
        );
    }

    for &id in &windows {
        // Only an SSD window can be shaded, and never while it's fullscreen.
        if policy.is_shaded(id) {
            assert!(policy.window(id).unwrap().ssd, "{id:?} is shaded but CSD");
            assert!(!policy.is_fullscreen(id), "{id:?} is shaded and fullscreen");
        }
        // Maximised is maximised both ways, and that's what the window is told.
        assert_eq!(
            policy.is_maximized(id),
            policy.is_maximized_horizontally(id) && policy.is_maximized_vertically(id)
        );
        let window = policy.window(id).unwrap();
        if let Some((_, states)) = window.last_geometry() {
            assert_eq!(states.maximized, policy.is_maximized(id), "{id:?}'s states");
            assert_eq!(states.shaded, policy.is_shaded(id), "{id:?}'s states");
            // Policy lays the window out with the frame drawn for what it was last sent.
            let drawn = if window.ssd && !states.fullscreen {
                window.frame_extents(states.frame_state(window.is_activated(), window.functions))
            } else {
                smithay::utils::FrameExtents::default()
            };
            assert_eq!(policy.frame_extents(id), drawn, "{id:?}'s frame extents");
        }
    }

    // Each output's work area is in it, and isn't empty, whatever is reserved.
    for (output, area) in policy.outputs().iter().zip(policy.work_areas()) {
        assert!(
            output.geometry.contains_rect(area) && area.size.w >= 1 && area.size.h >= 1,
            "{:?}'s work area {area:?} should be in {:?}",
            output.id,
            output.geometry
        );
    }

    for &id in &windows {
        let window = policy.window(id).unwrap();
        let Some((geometry, states)) = window.last_geometry() else {
            continue;
        };
        // A fullscreen window covers an output, reserved edges and all.
        if states.fullscreen && !policy.outputs().is_empty() {
            assert!(
                policy.outputs().iter().any(|o| o.geometry == geometry),
                "fullscreen {id:?} at {geometry:?} should cover an output"
            );
        }
    }

    // A window is told only of outputs there are.
    for &id in &windows {
        if let Some(outputs) = policy.window(id).unwrap().last_outputs() {
            for output in outputs {
                assert!(
                    policy.outputs().iter().any(|o| o.id == *output),
                    "{id:?} is on {output:?}, which policy doesn't have"
                );
            }
        }
    }

    // The Window switcher lists managed windows, once each, and selects one of them.
    if let Some(switcher) = policy.switcher() {
        assert!(
            !switcher.entries.is_empty(),
            "an open switcher lists windows"
        );
        assert!(
            switcher.selected < switcher.entries.len(),
            "the selection {} is in the {} listed windows",
            switcher.selected,
            switcher.entries.len()
        );
        for (place, entry) in switcher.entries.iter().enumerate() {
            let id = entry
                .window()
                .expect("the compositor named no Layer surface");
            assert!(policy.window(id).is_some(), "{id:?} is listed but gone");
            assert!(
                !switcher.entries[..place].contains(entry),
                "{id:?} is listed twice"
            );
        }
    }

    // A seat has one Interaction at most (187). The Window switcher, the Window menu, the
    // Workspace popup and the Overview are every seat's, so one of them is open at most,
    // and with none of the seats moving or resizing a window.
    let shared = [
        policy.switcher().is_some(),
        policy.window_menu().is_some(),
        policy.workspace_popup().is_some(),
        policy.overview().is_some(),
    ];
    let shared = shared.into_iter().filter(|open| *open).count();
    assert!(shared <= 1, "{shared} Interactions are every seat's");

    // The open Overview (241) is on every output policy has (246), and lists managed
    // windows, each once, of each output's active workspace (244), shown by that output.
    // Its selection (242) is one of the selection's output's.
    if let Some(overview) = policy.overview() {
        let on: Vec<OutputId> = overview.outputs().map(|on| on.output).collect();
        let outputs: Vec<OutputId> = policy.outputs().iter().map(|o| o.id).collect();
        assert_eq!(
            on.len(),
            outputs.len(),
            "the Overview isn't on every output"
        );
        assert!(outputs.iter().all(|output| on.contains(output)));
        let listed: Vec<WindowId> = overview.outputs().flat_map(|on| on.windows).collect();
        for (place, id) in listed.iter().enumerate() {
            assert!(!listed[..place].contains(id), "{id:?} is listed twice");
        }
        for on in overview.outputs() {
            assert_eq!(
                on.workspace,
                policy.active_workspace_on(on.output),
                "the Overview lists another workspace than its output's"
            );
            for id in &on.windows {
                assert_eq!(policy.output_showing(*id), Some(on.output));
            }
        }
        if let Some(selected) = overview.selected {
            assert!(
                overview.windows.contains(&selected),
                "{selected:?} is selected and not listed"
            );
        }
        // A press or a drag (245) is of a listed window, and has a target only as a drag.
        if let Some(drag) = overview.drag {
            assert!(
                listed.contains(&drag.window),
                "{:?} is dragged and not listed",
                drag.window
            );
            assert!(drag.dragging || drag.onto.is_none());
            assert!(drag.onto.is_some() || drag.target.is_none());
        }
        let on = overview.output;
        assert!(policy.outputs().iter().any(|output| output.id == on));
        for (place, id) in overview.windows.iter().enumerate() {
            assert!(policy.window(*id).is_some(), "{id:?} is listed and gone");
            assert!(
                !overview.windows[..place].contains(id),
                "{id:?} is listed twice"
            );
        }
    }
    for seat in 0..SEATS {
        let own = usize::from(policy.grab(SeatId(seat)).is_some());
        assert!(own + shared <= 1, "seat {seat} has two Interactions");
        assert_eq!(
            policy.interaction(SeatId(seat)).is_some(),
            own + shared == 1,
            "seat {seat}'s Interaction is what it has open"
        );
    }

    // At most one focused window per seat, and it is on screen.
    for seat in 0..SEATS {
        if let Some(focus) = policy.focus(SeatId(seat)) {
            assert!(
                policy.is_showing(focus),
                "seat {seat} focuses a showing window"
            );
        }
    }
}

/// The queries `caminito/src/scene.rs` makes of policy, in the alphabet's order. `SceneView`
/// holds the answer to each, and `the_scene_view_has_every_query_the_scene_makes` holds
/// this list to the Scene's source (179).
const SCENE_QUERIES: [&str; 13] = [
    "active_stacking",
    "frame_rect",
    "geometry",
    "grab",
    "is_fullscreen",
    "is_minimized",
    "is_showing",
    "layer",
    "overview",
    "stacking_all",
    "switcher",
    "tile_preview",
    "window",
];

/// What the Scene reads from policy, by the queries it reads it with (179).
#[derive(Debug, PartialEq)]
struct SceneView {
    /// `active_stacking`.
    stacking: Vec<WindowId>,
    /// `stacking_all`.
    stack: Vec<WindowId>,
    /// Each managed window, as `window` has them, with its answers.
    windows: Vec<WindowView>,
    /// Each seat's answers.
    grabs: Vec<GrabView>,
    /// The entries of `switcher`.
    switcher: Option<Vec<SwitchEntry>>,
    /// `overview`.
    overview: Option<Overview>,
}

#[derive(Debug, PartialEq)]
struct GrabView {
    grab: Option<WindowId>,
    tile_preview: Option<Rectangle<i32, Logical>>,
}

#[derive(Debug, PartialEq)]
struct WindowView {
    id: WindowId,
    geometry: Option<Rectangle<i32, Logical>>,
    frame_rect: Option<Rectangle<i32, Logical>>,
    is_showing: bool,
    is_fullscreen: bool,
    is_minimized: bool,
    layer: Option<Layer>,
}

fn scene_view(policy: &Policy<TestWindow>) -> SceneView {
    SceneView {
        stacking: policy.active_stacking(),
        stack: policy.stacking_all().to_vec(),
        windows: policy
            .windows()
            .map(|window| {
                let id = window.id;
                WindowView {
                    id,
                    geometry: policy.geometry(id),
                    frame_rect: policy.frame_rect(id),
                    is_showing: policy.is_showing(id),
                    is_fullscreen: policy.is_fullscreen(id),
                    is_minimized: policy.is_minimized(id),
                    layer: policy.layer(id),
                }
            })
            .collect(),
        grabs: (0..SEATS)
            .map(|seat| GrabView {
                grab: policy.grab(SeatId(seat)),
                tile_preview: policy.tile_preview(SeatId(seat)),
            })
            .collect(),
        switcher: policy.switcher().map(|switcher| switcher.entries.clone()),
        overview: policy.overview().cloned(),
    }
}

/// The names of the methods `source` calls on something named `policy`.
fn policy_queries(source: &str) -> std::collections::BTreeSet<&str> {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let mut queries = std::collections::BTreeSet::new();
    for (at, _) in source.match_indices("policy") {
        if source[..at].ends_with(word) {
            continue;
        }
        let rest = source[at + "policy".len()..].trim_start();
        let Some(rest) = rest.strip_prefix('.') else {
            continue;
        };
        let rest = rest.trim_start();
        let name = &rest[..rest.find(|c| !word(c)).unwrap_or(rest.len())];
        if rest[name.len()..].starts_with('(') {
            queries.insert(name);
        }
    }
    queries
}

#[test]
fn the_scene_view_has_every_query_the_scene_makes() {
    let scene = include_str!("../../../caminito/src/scene.rs");
    assert_eq!(
        policy_queries(scene).into_iter().collect::<Vec<_>>(),
        SCENE_QUERIES,
        "`SceneView`, and `SceneReads` in policy.rs, hold what `scene.rs` reads from policy"
    );
}

/// What outputs coming and going keep (225), as it was before an operation.
struct OutputsBefore {
    outputs: Vec<OutputId>,
    per_output: bool,
    workspaces: Vec<(WindowId, Option<usize>)>,
}

impl OutputsBefore {
    fn of(policy: &Policy<TestWindow>) -> Self {
        Self {
            outputs: policy.outputs().iter().map(|o| o.id).collect(),
            per_output: policy.workspaces_per_output(),
            workspaces: workspaces(policy),
        }
    }

    /// With `workspaces-per-output` on, an output that joins others starts on the first
    /// workspace. With it on or off, no window changes its workspace's number as outputs
    /// come, go, move or change: the windows of one that went are on the same numbers of
    /// the output they're on now.
    fn verify(self, policy: &Policy<TestWindow>, done: &Op) {
        let stays = |id: &OutputId| policy.outputs().iter().any(|o| o.id == *id);
        if self.per_output && self.outputs.iter().any(stays) {
            for output in policy.outputs() {
                if !self.outputs.contains(&output.id) {
                    assert_eq!(
                        policy.active_workspace_on(output.id),
                        0,
                        "{:?} starts on the first workspace",
                        output.id
                    );
                }
            }
        }
        let of_outputs = matches!(
            done,
            Op::AddOutput { .. }
                | Op::RemoveOutput(_)
                | Op::Plug { .. }
                | Op::Unplug(_)
                | Op::Reserve { .. }
        );
        if of_outputs {
            assert_eq!(
                workspaces(policy),
                self.workspaces,
                "{done:?} left every window on its workspace"
            );
        }
    }
}

/// Each window's workspace.
fn workspaces(policy: &Policy<TestWindow>) -> Vec<(WindowId, Option<usize>)> {
    let workspace = |w: &TestWindow| (w.id, policy.workspace_of(w.id));
    policy.windows().map(workspace).collect()
}

fn check_ops(workspaces_per_output: bool, ops: Vec<Op>) {
    // Tiling on, as Marco's default has it, so the drags can tile (158).
    let mut policy = Policy::new(Config {
        allow_tiling: true,
        workspaces_per_output,
        ..Config::default()
    });
    policy.take_scene_changed();
    let mut seen = scene_view(&policy);
    for (step, op) in ops.into_iter().enumerate() {
        let done = op.clone();
        let before = OutputsBefore::of(&policy);
        op.apply(&mut policy, step as u64);
        before.verify(&policy, &done);
        // A change in what the Scene reads comes with policy saying so (179). Saying so
        // with no change is allowed.
        let view = scene_view(&policy);
        let said = policy.take_scene_changed();
        assert!(
            said || view == seen,
            "{done:?} changed what the Scene reads, and policy didn't say so"
        );
        seen = view;
        verify_invariants(&policy);
    }
}

proptest! {
    #[test]
    fn random_operations_keep_the_invariants(ops in proptest::collection::vec(op(), 1..60)) {
        check_ops(false, ops);
    }

    /// The same with `workspaces-per-output` on to start with, as Caminito's default has it
    /// (230).
    #[test]
    fn random_operations_keep_the_invariants_with_workspaces_per_output(
        ops in proptest::collection::vec(op(), 1..60)
    ) {
        check_ops(true, ops);
    }
}

/// A window that asks to map maximised or fullscreen is first configured with the size
/// policy sends it at the map (190), with nothing changed between the two: no operation
/// between them, so the same focus, pointer, outputs and config.
///
/// But, on purpose:
///
/// - **A maximised window that policy doesn't focus as it maps.** The answer takes the frame
///   as a new window's, focused (ADR 0012), and the window's frame can be another size
///   unfocused. Whether policy will focus it isn't known before the map: it can take no
///   input, or map on another workspace, or `focus-new-windows` can refuse it.
/// - **A window with a position of its own.** It's laid out on the output its frame is most
///   on there, and the position isn't known before the map, so the answer takes the
///   automatic choice of output.
/// - **Outputs that overlap**, as mirrored ones do. A frame as much on one as on the other is
///   on either, as `output_of` breaks the tie, and the output a window is laid out on is
///   found again from its frame each time it's laid out.
fn check_pre_map_size(
    ops: Vec<Op>,
    placement_mode: PlacementMode,
    pointer: (i16, i16),
    window: TestWindow,
    states: MapStates,
) {
    let mut policy = Policy::new(Config {
        allow_tiling: true,
        placement_mode,
        ..Config::default()
    });
    for (step, op) in ops.into_iter().enumerate() {
        op.apply(&mut policy, step as u64);
    }
    // The compositor never maps an id twice.
    if policy.window(window.id).is_some() || window.requested_position.is_some() {
        return;
    }
    let outputs = policy.outputs();
    let overlap = outputs.iter().enumerate().any(|(i, a)| {
        outputs[i + 1..]
            .iter()
            .any(|b| a.geometry.overlaps(b.geometry))
    });
    if overlap {
        return;
    }
    policy.set_pointer(Point::from((f64::from(pointer.0), f64::from(pointer.1))));
    let pending = PreMapWindow {
        window_type: window.window_type,
        hints: window.hints,
        parent: window.parent,
        ssd: window.ssd,
    };
    let answer = policy.pre_map_configure(&pending, &states, |state| window.frame_extents(state));

    let id = window.id;
    policy.map_window_with(window, states, ms(100_000));

    assert!(
        !answer.maximized || policy.is_maximized(id),
        "it maps maximised"
    );
    assert_eq!(
        answer.fullscreen,
        policy.is_fullscreen(id),
        "it maps fullscreen or not"
    );
    let Some(size) = answer.size else {
        return;
    };
    let mapped = policy.window(id).unwrap();
    if answer.maximized && !mapped.is_activated() {
        return;
    }
    let (sent, _) = mapped.last_geometry().expect("a geometry is sent");
    assert_eq!(
        size, sent.size,
        "the size it was configured with is the one it maps at"
    );
}

proptest! {
    #[test]
    fn a_window_maps_at_the_size_it_was_configured_with_before_it_mapped(
        ops in proptest::collection::vec(op(), 0..40),
        placement_mode in prop_oneof![
            Just(PlacementMode::Automatic),
            Just(PlacementMode::Pointer),
            Just(PlacementMode::Manual),
        ],
        pointer in (any::<i16>(), any::<i16>()),
        window in test_window(),
        maximized in any::<bool>(),
        fullscreen in proptest::option::of(proptest::option::of(1..=4u64)),
    ) {
        let states = MapStates {
            maximized,
            fullscreen: fullscreen.map(|output| output.map(OutputId)),
            ..MapStates::default()
        };
        check_pre_map_size(ops, placement_mode, pointer, window, states);
    }
}
