//! The Overview on two outputs (246), under Per-output workspaces and under Spanning
//! workspaces (ADR 0032): each output's listed windows, layout and bar, read from the
//! debug dump, a click on a cell of either bar, the workspace bindings, a window dragged
//! from one output onto the other's layout or onto a cell of its bar, a window across both
//! outputs, an output that goes while it's open, and the transition on both at once.
//!
//! The outputs are `two_outputs_with`'s: two of 1920 × 1080 side by side, the left one the
//! primary. Each test's windows are 400 × 300, put where it says.
//!
//! The layouts and a drop onto the other output run on both kinds of window, and an
//! output that goes too. The rest map xdg toplevels only: the bars, the clicks on a cell,
//! the bindings and the transition are the Overview's own state and don't depend on the
//! kind, a window across both outputs is policy's Main output, which is the same for both,
//! and the shrink, the maximised and the tiled window read the size Caminito asked for
//! from an xdg client's configures, which an X client has no record of.

use caminito_policy::{Action, Tile};
use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle};

use super::fixture::{
    BTN_LEFT, ClientId, ClientWindow, Fixture, KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT, Kind,
    WindowSpec, on_both_kinds,
};
use super::overview::{OUTPUT, WINDOW, click, is_open, middle, open, overview, rect};
use super::per_output_workspaces::{active_workspaces, put_at, two_outputs_with, window_at};
use crate::output::{output_geometry, output_id};
use crate::settings::Value;
use crate::settings::translate::CAMINITO;
use crate::state::SEAT;

/// The two outputs with `workspaces-per-output` as `per_output` says, and a client of
/// `kind`: on the left output's first workspace a window at (100, 100) and one at
/// (900, 500), and on the right one's a window at (2300, 200), which is focused.
fn desk(kind: Kind, per_output: bool) -> (Fixture, ClientId, Output, Output, [ClientWindow; 3]) {
    desk_with(kind, per_output, vec![])
}

/// `desk`, under a Caminito with `settings` set from the start.
pub(super) fn desk_with(
    kind: Kind,
    per_output: bool,
    settings: Vec<(&'static str, String, Value)>,
) -> (Fixture, ClientId, Output, Output, [ClientWindow; 3]) {
    let (mut f, client, left, right) = two_outputs_with(kind, per_output, settings);
    let first = window_at(&mut f, client, (100, 100), 0);
    let second = window_at(&mut f, client, (900, 500), 0);
    let third = window_at(&mut f, client, (2300, 200), 0);
    let id = f.pid(third);
    f.state
        .caminito
        .policy
        .activate(SEAT, id, f.state.caminito.now());
    f.dispatch();
    (f, client, left, right, [first, second, third])
}

/// What the open Overview's dump has on `output`.
#[track_caller]
pub(super) fn on(f: &Fixture, output: &Output) -> serde_json::Value {
    let outputs = overview(f)["outputs"].as_array().unwrap().clone();
    let on = outputs
        .into_iter()
        .find(|on| on["output"] == output.name().as_str());
    on.expect("the Overview is on the output")
}

/// The windows the open Overview lists on `output`, by policy's ids.
pub(super) fn listed_on(f: &Fixture, output: &Output) -> Vec<u64> {
    let windows = on(f, output)["windows"].as_array().unwrap().clone();
    windows
        .iter()
        .map(|window| window["window"].as_u64().unwrap())
        .collect()
}

pub(super) fn ids(f: &Fixture, windows: &[ClientWindow]) -> Vec<u64> {
    windows.iter().map(|window| f.pid(*window).0).collect()
}

/// `local`, from `output`'s top left corner, in global coordinates.
pub(super) fn global(output: &Output, local: Point<i32, Logical>) -> Point<i32, Logical> {
    output_geometry(output).loc + local
}

/// Where the layout on `output` draws `window`, from the output's top left corner.
#[track_caller]
fn place_on(f: &Fixture, output: &Output, window: ClientWindow) -> Rectangle<i32, Logical> {
    let id = f.pid(window).0;
    let windows = on(f, output)["windows"].as_array().unwrap().clone();
    let listed = windows.iter().find(|listed| listed["window"] == id);
    rect(&listed.expect("the window is listed there")["rect"])
}

/// The cell of `workspace` of `output`'s bar, from the output's top left corner.
#[track_caller]
pub(super) fn cell_on(f: &Fixture, output: &Output, workspace: u64) -> Rectangle<i32, Logical> {
    let bar = on(f, output)["bar"].as_array().unwrap().clone();
    let cell = bar.iter().find(|cell| cell["workspace"] == workspace);
    rect(&cell.expect("the workspace has a cell")["rect"])
}

/// The workspace whose cell is marked active on `output`'s bar.
fn active_cell(f: &Fixture, output: &Output) -> u64 {
    let bar = on(f, output)["bar"].as_array().unwrap().clone();
    let active = bar.iter().filter(|cell| cell["active"] == true);
    let active: Vec<u64> = active
        .map(|cell| cell["workspace"].as_u64().unwrap())
        .collect();
    assert_eq!(active.len(), 1, "one cell is marked");
    active[0]
}

/// `output`'s pixel at `local`, from its top left corner, as `0xRRGGBB`.
fn pixel_on(f: &mut Fixture, output: &Output, local: Point<i32, Logical>) -> u32 {
    f.dispatch();
    let pixel = f
        .state
        .backend
        .headless_mut()
        .pixel(output, local.to_physical(1));
    pixel.expect("the output is drawn") & 0xff_ffff
}

/// Presses the left button on `window`'s place in `from`'s layout, and moves the pointer
/// past `drag-threshold` and on to the global point `to`, in steps as a mouse does.
fn drag_to(f: &mut Fixture, from: &Output, window: ClientWindow, to: Point<i32, Logical>) {
    let start = global(from, middle(place_on(f, from, window)));
    f.pointer_move(f64::from(start.x), f64::from(start.y));
    f.pointer_button(BTN_LEFT, true);
    for step in 1..=4 {
        let at = start
            + (to - start)
                .to_f64()
                .upscale(f64::from(step) / 4.0)
                .to_i32_round();
        f.pointer_move(f64::from(at.x), f64::from(at.y));
    }
}

/// The middle of `output`'s layout area, in global coordinates.
fn layout_spot(f: &Fixture, output: &Output) -> Point<i32, Logical> {
    let area = rect(&on(f, output)["area"]);
    global(output, middle(area))
}

/// Sorted, to compare lists whose stacking order a test doesn't set.
fn sorted(mut ids: Vec<u64>) -> Vec<u64> {
    ids.sort_unstable();
    ids
}

/// The output `window` shows by, and its workspace.
fn where_is(f: &Fixture, window: ClientWindow) -> (Option<String>, Option<usize>) {
    let policy = &f.state.caminito.policy;
    let id = f.pid(window);
    let output = policy.output_showing(id);
    let outputs = &f.state.caminito.outputs;
    let output = outputs.iter().find(|o| Some(output_id(o)) == output);
    (output.map(Output::name), policy.workspace_of(id))
}

/// The frame Caminito last asked of the xdg toplevel `window` of `client`: where policy
/// has it, at the size of the last configure with the frame's extents around it. The test
/// client doesn't take a configure's size by itself.
fn asked_frame(f: &mut Fixture, client: ClientId, window: ClientWindow) -> Rectangle<i32, Logical> {
    f.roundtrip(client);
    let (w, h) = f
        .client(client)
        .window(window.xdg())
        .configures
        .last()
        .unwrap()
        .size;
    let policy = &f.state.caminito.policy;
    let id = f.pid(window);
    let e = policy.frame_extents(id);
    let loc = policy.frame_rect(id).unwrap().loc;
    Rectangle::new(loc, (w + e.left + e.right, h + e.top + e.bottom).into())
}

// Each output's layout and bar.

on_both_kinds! {
    fn each_output_lays_out_its_own_windows_beneath_its_own_bar(kind: Kind) {
        for per_output in [true, false] {
            let (mut f, _, left, right, [first, second, third]) = desk(kind, per_output);

            open(&mut f);

            assert_eq!(overview(&f)["outputs"].as_array().unwrap().len(), 2);
            // The selection is on the focused window's output, which the dump's own
            // fields are of.
            assert_eq!(overview(&f)["output"], right.name().as_str());
            assert_eq!(overview(&f)["selected"], f.pid(third).0);
            assert_eq!(listed_on(&f, &left), ids(&f, &[first, second]));
            assert_eq!(listed_on(&f, &right), ids(&f, &[third]));
            for output in [&left, &right] {
                let bar = on(&f, output)["bar"].as_array().unwrap().len();
                assert_eq!(bar, 4, "{}", output.name());
                assert_eq!(active_cell(&f, output), 0);
                let area = rect(&on(&f, output)["area"]);
                assert!(area.loc.y >= cell_on(&f, output, 0).loc.y + cell_on(&f, output, 0).size.h);
                let hidden = &on(&f, output)["hidden_layer_surfaces"];
                assert!(hidden.is_array());
            }
            for (output, window) in [(&left, first), (&left, second), (&right, third)] {
                let place = place_on(&f, output, window);
                assert!(rect(&on(&f, output)["area"]).contains_rect(place));
                assert_eq!(pixel_on(&mut f, output, middle(place)), WINDOW);
            }
        }
    }
}

#[test]
fn under_per_output_workspaces_each_bar_marks_its_own_outputs_workspace() {
    let (mut f, client, left, right, _) = desk(Kind::Xdg, true);
    let elsewhere = window_at(&mut f, client, (2500, 600), 2);
    let policy = &mut f.state.caminito.policy;
    policy.switch_output_workspace(output_id(&right), 2);
    f.dispatch();

    open(&mut f);

    assert_eq!(active_cell(&f, &left), 0);
    assert_eq!(active_cell(&f, &right), 2);
    assert_eq!(on(&f, &right)["workspace"], 2);
    assert_eq!(listed_on(&f, &right), ids(&f, &[elsewhere]));
}

#[test]
fn a_window_across_both_outputs_is_in_its_main_outputs_layout_alone() {
    let (mut f, _, left, right, [first, second, third]) = desk(Kind::Xdg, true);
    // 220 pixels of it on the left output, 180 on the right one.
    put_at(&mut f, second, (1700, 500), 0);

    open(&mut f);

    assert_eq!(listed_on(&f, &left), ids(&f, &[first, second]));
    assert_eq!(listed_on(&f, &right), ids(&f, &[third]));
}

// Clicks on a cell, and the bindings.

#[test]
fn a_click_on_a_cell_switches_its_output_alone_under_per_output_workspaces() {
    let (mut f, _, left, right, [_, _, third]) = desk(Kind::Xdg, true);
    open(&mut f);

    let cell = cell_on(&f, &left, 1);
    click(&mut f, global(&left, middle(cell)));

    assert!(is_open(&f));
    assert_eq!(active_workspaces(&f, &left, &right), (1, 0));
    assert_eq!(active_cell(&f, &left), 1);
    assert_eq!(active_cell(&f, &right), 0);
    assert_eq!(listed_on(&f, &left), Vec::<u64>::new());
    assert_eq!(listed_on(&f, &right), ids(&f, &[third]));
}

#[test]
fn a_click_on_a_cell_switches_both_outputs_under_spanning_workspaces() {
    let (mut f, _, left, right, _) = desk(Kind::Xdg, false);
    open(&mut f);

    let cell = cell_on(&f, &right, 2);
    click(&mut f, global(&right, middle(cell)));

    assert!(is_open(&f));
    assert_eq!(active_workspaces(&f, &left, &right), (2, 2));
    for output in [&left, &right] {
        assert_eq!(active_cell(&f, output), 2);
        assert_eq!(listed_on(&f, output), Vec::<u64>::new());
    }
}

#[test]
fn a_workspace_binding_switches_the_active_outputs_workspace() {
    let (mut f, _, left, right, _) = desk(Kind::Xdg, true);
    open(&mut f);

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);

    assert!(is_open(&f));
    assert_eq!(active_workspaces(&f, &left, &right), (0, 1));
    assert_eq!(active_cell(&f, &right), 1);
}

// A drag onto the other output.

on_both_kinds! {
    fn a_window_dropped_on_the_other_outputs_layout_goes_to_its_active_workspace(kind: Kind) {
        let (mut f, client, left, right, [first, second, _]) = desk(kind, true);
        let elsewhere = window_at(&mut f, client, (2500, 600), 2);
        f.state.caminito.policy.switch_output_workspace(output_id(&right), 2);
        f.dispatch();
        let size = f.geometry(first).size;
        open(&mut f);
        let to = layout_spot(&f, &right);

        drag_to(&mut f, &left, first, to);

        let drag = &overview(&f)["drag"];
        assert_eq!(drag["onto"], right.name().as_str());
        assert!(drag["target"].is_null());
        // It's drawn under the pointer on the right output.
        let beside = to - Point::from((10, 10)) - output_geometry(&right).loc;
        assert_eq!(pixel_on(&mut f, &right, beside), WINDOW);

        f.pointer_button(BTN_LEFT, false);

        assert!(is_open(&f));
        assert_eq!(where_is(&f, first), (Some(right.name()), Some(2)));
        // At the same place relative to the work area, at its size.
        assert_eq!(f.geometry(first), Rectangle::new((1920 + 100, 100).into(), size));
        assert_eq!(listed_on(&f, &left), ids(&f, &[second]));
        assert_eq!(sorted(listed_on(&f, &right)), sorted(ids(&f, &[elsewhere, first])));
        assert_eq!(active_workspaces(&f, &left, &right), (0, 2));
    }
}

on_both_kinds! {
    fn a_window_dropped_on_a_cell_of_the_other_outputs_bar_goes_to_that_workspace(kind: Kind) {
        for per_output in [true, false] {
            let (mut f, _, left, right, [first, second, third]) = desk(kind, per_output);
            let size = f.geometry(first).size;
            open(&mut f);
            let to = global(&right, middle(cell_on(&f, &right, 3)));

            drag_to(&mut f, &left, first, to);
            assert_eq!(overview(&f)["drag"]["target"], 3);
            f.pointer_button(BTN_LEFT, false);

            assert!(is_open(&f));
            assert_eq!(where_is(&f, first), (Some(right.name()), Some(3)));
            assert_eq!(f.geometry(first), Rectangle::new((1920 + 100, 100).into(), size));
            assert_eq!(listed_on(&f, &left), ids(&f, &[second]));
            assert_eq!(listed_on(&f, &right), ids(&f, &[third]));
            assert_eq!(active_workspaces(&f, &left, &right), (0, 0));
        }
    }
}

#[test]
fn under_spanning_workspaces_a_window_dropped_on_the_other_outputs_layout_keeps_its_workspace() {
    let (mut f, _, left, right, [first, second, third]) = desk(Kind::Xdg, false);
    open(&mut f);
    let to = layout_spot(&f, &right);

    drag_to(&mut f, &left, first, to);
    f.pointer_button(BTN_LEFT, false);

    assert_eq!(where_is(&f, first), (Some(right.name()), Some(0)));
    assert_eq!(listed_on(&f, &left), ids(&f, &[second]));
    assert_eq!(
        sorted(listed_on(&f, &right)),
        sorted(ids(&f, &[first, third]))
    );
}

#[test]
fn a_window_larger_than_the_other_outputs_work_area_is_shrunk_to_fit_it() {
    let (mut f, client, left, right) = two_outputs_with(Kind::Xdg, true, vec![]);
    // The right output is smaller: 1280 × 720.
    f.set_output_scale(&right, 1.5);
    let big = f.map(client, WindowSpec::plain((1800, 1000)));
    put_at(&mut f, big, (50, 50), 0);
    open(&mut f);
    let to = layout_spot(&f, &right);

    drag_to(&mut f, &left, big, to);
    f.pointer_button(BTN_LEFT, false);

    assert_eq!(where_is(&f, big).0, Some(right.name()));
    let frame = asked_frame(&mut f, client, big);
    let area = output_geometry(&right);
    assert!(area.contains_rect(frame), "{frame:?} is out of {area:?}");
    assert_eq!(frame.size.w, 1280);
}

#[test]
fn a_maximised_and_a_tiled_window_keep_their_state_on_the_other_output() {
    let (mut f, client, left, right, [first, second, _]) = desk(Kind::Xdg, true);
    let (maximised, tiled) = (f.pid(first), f.pid(second));
    let now = f.state.caminito.now();
    let policy = &mut f.state.caminito.policy;
    policy.set_maximized(maximised, true);
    policy.activate(SEAT, tiled, now);
    policy.action(SEAT, Action::Tile(Tile::Left), now);
    f.dispatch();
    open(&mut f);

    for window in [first, second] {
        let to = layout_spot(&f, &right);
        drag_to(&mut f, &left, window, to);
        f.pointer_button(BTN_LEFT, false);
    }

    let policy = &f.state.caminito.policy;
    assert!(policy.is_maximized(maximised));
    assert_eq!(policy.tile(tiled), Some(Tile::Left));
    for window in [first, second] {
        assert_eq!(where_is(&f, window).0, Some(right.name()));
    }
    // The right output has no panel: its work area is all of it.
    assert_eq!(asked_frame(&mut f, client, first), output_geometry(&right));
    let half = Rectangle::new((1920, 0).into(), (960, 1080).into());
    assert_eq!(asked_frame(&mut f, client, second), half);
}

// Outputs that come and go.

on_both_kinds! {
    fn an_output_unplugged_while_it_is_open_leaves_it_open_on_the_other(kind: Kind) {
        let (mut f, _, left, right, [first, second, third]) = desk(kind, true);
        open(&mut f);

        f.unplug(&right);

        assert!(is_open(&f));
        assert_eq!(overview(&f)["outputs"].as_array().unwrap().len(), 1);
        assert_eq!(overview(&f)["output"], left.name().as_str());
        let mut listed = listed_on(&f, &left);
        listed.sort();
        let mut taken = ids(&f, &[first, second, third]);
        taken.sort();
        assert_eq!(listed, taken);
        let place = place_on(&f, &left, third);
        assert!(rect(&on(&f, &left)["area"]).contains_rect(place));
        assert_eq!(pixel_on(&mut f, &left, middle(place)), WINDOW);
    }
}

#[test]
fn an_output_plugged_in_while_it_is_open_gets_its_own_bar() {
    let (mut f, _, left, _, _) = desk(Kind::Xdg, true);
    open(&mut f);

    let third = f.add_output(OUTPUT, 1.0);
    f.dispatch();

    assert!(is_open(&f));
    assert_eq!(overview(&f)["outputs"].as_array().unwrap().len(), 3);
    assert_eq!(on(&f, &third)["bar"].as_array().unwrap().len(), 4);
    assert_eq!(listed_on(&f, &third), Vec::<u64>::new());
    assert_eq!(on(&f, &left)["bar"].as_array().unwrap().len(), 4);
}

// The transition (243).

#[test]
fn the_transition_runs_on_both_outputs_at_once() {
    let animated = vec![(CAMINITO, "overview-animation".into(), Value::Bool(true))];
    let (mut f, client, left, right) = two_outputs_with(Kind::Xdg, true, animated);
    window_at(&mut f, client, (100, 100), 0);
    window_at(&mut f, client, (2300, 200), 0);
    f.stop_clock();

    super::overview::press_shortcut(&mut f);

    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    let transition = &dump["overview_transition"];
    assert_eq!(transition["direction"], "opening");
    let outputs = transition["outputs"].as_array().unwrap();
    assert!(outputs.contains(&left.name().as_str().into()));
    assert!(outputs.contains(&right.name().as_str().into()));
    let now = f.state.caminito.now();
    for output in [&left, &right] {
        assert!(f.state.caminito.overview_transition_on(output, now));
    }
}
