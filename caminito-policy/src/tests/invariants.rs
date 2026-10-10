//! Random operation sequences, with policy's invariants checked after every operation, in the
//! shape of niri's `check_ops` and `verify_invariants`.

use proptest::prelude::*;

use super::*;
use smithay::utils::Size;

use crate::{
    Crossing, FocusMode, FrameState, PointerTarget, PolicyWindow, ResizeEdges, SwitchList,
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
        x: i16,
        y: i16,
    },
    EndGrab(u32),
    SetWorkspaceCount(usize),
    RequestGeometry {
        id: u64,
        loc: Option<(i32, i32)>,
        size: Option<(i32, i32)>,
    },
    SetAbove {
        id: u64,
        above: bool,
    },
    SetShowingDesktop(bool),
    SwitcherStep(bool),
    SwitcherSelect(usize),
    SwitcherCommit,
    SwitcherCancel,
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
    FrameClick {
        seat: u32,
        id: u64,
    },
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
        Just(Action::Close),
        Just(Action::ToggleMaximized),
        (switch_list(), any::<bool>())
            .prop_map(|(list, backward)| Action::Switch { list, backward }),
        Just(Action::ShowDesktop),
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
        1 => (seat(), any::<i16>(), any::<i16>()).prop_map(|(seat, x, y)| Op::GrabMotion { seat, x, y }),
        1 => seat().prop_map(Op::EndGrab),
        1 => (1..=6usize).prop_map(Op::SetWorkspaceCount),
        1 => (
            window_id(),
            proptest::option::of((-500..3500i32, -500..2000i32)),
            proptest::option::of((1..2500i32, 1..1500i32)),
        )
            .prop_map(|(id, loc, size)| Op::RequestGeometry { id, loc, size }),
        1 => (window_id(), any::<bool>()).prop_map(|(id, above)| Op::SetAbove { id, above }),
        1 => any::<bool>().prop_map(Op::SetShowingDesktop),
        1 => any::<bool>().prop_map(Op::SwitcherStep),
        1 => (0..10usize).prop_map(Op::SwitcherSelect),
        1 => Just(Op::SwitcherCommit),
        1 => Just(Op::SwitcherCancel),
        1 => (focus_mode(), any::<bool>(), any::<bool>()).prop_map(|(mode, raise_on_click, auto_raise)| {
            Op::SetFocusMode { mode, raise_on_click, auto_raise }
        }),
        3 => (seat(), pointer_target(), crossing())
            .prop_map(|(seat, target, crossing)| Op::PointerEntered { seat, target, crossing }),
        1 => (seat(), window_id()).prop_map(|(seat, id)| Op::FrameClick { seat, id }),
        1 => (0..1000u64).prop_map(Op::Elapse),
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
            Op::GrabMotion { seat, x, y } => policy.grab_motion(SeatId(seat), pointer(x, y)),
            Op::EndGrab(seat) => policy.end_grab(SeatId(seat)),
            Op::SetWorkspaceCount(workspace_count) => policy.settings_changed(Config {
                workspace_count,
                workspace_names: Vec::new(),
                ..Config::default()
            }),
            Op::RequestGeometry { id, loc, size } => {
                policy.request_geometry(WindowId(id), loc.map(Point::from), size.map(Size::from))
            }
            Op::SetAbove { id, above } => policy.set_above(WindowId(id), above),
            Op::SetShowingDesktop(showing) => policy.set_showing_desktop(showing),
            Op::SwitcherStep(backward) => policy.switcher_step(backward),
            Op::SwitcherSelect(index) => policy.switcher_select(index),
            Op::SwitcherCommit => policy.switcher_commit(now),
            Op::SwitcherCancel => policy.switcher_cancel(now),
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
                ..Config::default()
            }),
            Op::PointerEntered {
                seat,
                target,
                crossing,
            } => policy.pointer_entered(SeatId(seat), target, crossing, now),
            Op::FrameClick { seat, id } => policy.frame_click(SeatId(seat), WindowId(id), now),
            Op::Elapse(by) => policy.elapse(now + ms(by)),
        }
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
    let stacks: Vec<&[WindowId]> = (0..policy.workspace_count())
        .map(|ws| policy.stacking(ws))
        .collect();

    // Stacking is a permutation of the managed windows, and every window is on exactly one
    // workspace: the one policy says it's on. The active workspace is one of them.
    assert!(
        policy.active_workspace() < stacks.len(),
        "the active workspace exists"
    );
    let stacked: usize = stacks.iter().map(|s| s.len()).sum();
    assert_eq!(stacked, windows.len(), "stacks hold every window once");
    for &id in &windows {
        let on: Vec<usize> = (0..stacks.len())
            .filter(|ws| stacks[*ws].contains(&id))
            .collect();
        assert_eq!(
            on,
            [policy.workspace_of(id).unwrap()],
            "{id:?} should be on exactly one workspace"
        );
    }

    for (ws, stack) in stacks.iter().enumerate() {
        let position = |id: WindowId| stack.iter().position(|w| *w == id);
        for &id in *stack {
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
                window.frame_extents(FrameState {
                    maximized: states.maximized,
                    focused: window.is_activated(),
                    shaded: states.shaded,
                })
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
            !switcher.windows.is_empty(),
            "an open switcher lists windows"
        );
        assert!(
            switcher.selected < switcher.windows.len(),
            "the selection {} is in the {} listed windows",
            switcher.selected,
            switcher.windows.len()
        );
        for (place, id) in switcher.windows.iter().enumerate() {
            assert!(policy.window(*id).is_some(), "{id:?} is listed but gone");
            assert!(
                !switcher.windows[..place].contains(id),
                "{id:?} is listed twice"
            );
        }
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

fn check_ops(ops: Vec<Op>) {
    let mut policy = Policy::new(Config::default());
    for (step, op) in ops.into_iter().enumerate() {
        op.apply(&mut policy, step as u64);
        verify_invariants(&policy);
    }
}

proptest! {
    #[test]
    fn random_operations_keep_the_invariants(ops in proptest::collection::vec(op(), 1..60)) {
        check_ops(ops);
    }
}
