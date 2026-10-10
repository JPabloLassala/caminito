//! Example tests for each behaviour of `Policy`, through `TestWindow`.

mod actions;
mod commit;
mod dynamic_workspaces;
mod edge_resistance;
mod focus;
mod grid;
mod interactive;
mod invariants;
mod outputs;
mod placement;
mod placement_modes;
mod pointer_focus;
mod requests;
mod shade;
mod show_desktop;
mod stacking;
mod states;
mod switcher;
mod transients;
mod window_actions;
mod work_area;
mod workspace_moves;
mod workspaces;

use std::time::Duration;

use smithay::utils::{Logical, Point, Rectangle};

use crate::test_window::TestWindow;
use crate::{
    Action, Config, Layer, Output, OutputId, Policy, ReservedEdges, SeatId, WindowId, WindowStates,
    WindowType, WorkspaceLayout,
};

const SEAT: SeatId = SeatId(0);

/// A policy with one 1920×1080 output at scale 1.
fn policy() -> Policy<TestWindow> {
    let mut policy = Policy::new(Config::default());
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy
}

fn output(id: u64, x: i32, y: i32, w: i32, h: i32) -> Output {
    Output {
        id: OutputId(id),
        geometry: Rectangle::new((x, y).into(), (w, h).into()),
        scale: 1.,
        reserved: ReservedEdges::default(),
    }
}

/// `output` with these edges reserved.
fn reserving(output: Output, left: i32, right: i32, top: i32, bottom: i32) -> Output {
    Output {
        reserved: ReservedEdges {
            left,
            right,
            top,
            bottom,
        },
        ..output
    }
}

fn ms(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

fn id(id: u64) -> WindowId {
    WindowId(id)
}

fn window(policy: &Policy<TestWindow>, window: u64) -> &TestWindow {
    policy.window(WindowId(window)).expect("window is managed")
}
