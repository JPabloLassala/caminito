//! A dragged window's shrink and its drop in the Overview (249), read from the debug dump:
//! over a cell of the bar the window eases to the cell's scale about the point it was
//! grabbed by, and off the bar back to its layout size. Dropped, it eases from under the
//! pointer to its place in a cell's picture or to its slot in a layout, and the picture
//! leaves it out until it's there. The Overview closing, a switch to the drop's workspace
//! and a press on the window cut a drop to its end. With two outputs of different sizes,
//! the other output's bar has its own scale.
//!
//! Each test stops Caminito's clock, so the shrink and the drop are where `advance_clock`
//! takes them. What the shrink and the drop draw is the Overview's own, so all but the
//! first test map xdg toplevels only.

use std::time::Duration;

use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle, Size};

use super::fixture::{
    BTN_LEFT, ClientId, ClientWindow, Fixture, KEY_ESC, KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT, Kind,
    on_both_kinds,
};
use super::overview::{
    OUTPUT, WINDOW, cell, click, destroy, drag, drag_to, empty_spot, map_three, middle, open,
    overview, pixel, place, rect,
};
use super::overview_outputs::{cell_on, global, on, place_on};
use super::per_output_workspaces::window_at;
use crate::output::output_geometry;
use crate::overview::drag::scaled_into;
use crate::settings::Value;
use crate::settings::translate::CAMINITO;
use crate::state::SEAT;
use crate::workspace_slide::LENGTH;

/// The middle of the shrink and of the drop, where the ease-out has them seven eighths of
/// the way.
const MIDDLE: Duration = Duration::from_millis(125);

/// The last millisecond before the end.
const ALMOST: Duration = Duration::from_millis(249);

/// One output with `overview-animation` as `animated` says, and a client of `kind`, with
/// Caminito's clock stopped.
fn desk(kind: Kind, animated: bool) -> (Fixture, ClientId) {
    let setting = (CAMINITO, "overview-animation".into(), Value::Bool(animated));
    let mut f = Fixture::with_settings(vec![setting]);
    f.add_output(OUTPUT, 1.0);
    let id = f.connect(kind);
    f.stop_clock();
    (f, id)
}

/// Opens the Overview and lets its transition finish.
fn open_at_rest(f: &mut Fixture) {
    open(f);
    f.advance_clock(LENGTH);
}

/// Where the dragged window is drawn, from the top left corner of the selection's output.
#[track_caller]
fn dragged(f: &Fixture) -> Rectangle<i32, Logical> {
    rect(&drag(f)["rect"])
}

/// The drop on its way: `null` with none.
fn dropping(f: &Fixture) -> serde_json::Value {
    overview(f)["drop"].clone()
}

/// Where the drop has the window now, and where it goes.
#[track_caller]
fn drop_rects(f: &Fixture) -> (Rectangle<i32, Logical>, Rectangle<i32, Logical>) {
    let drop = dropping(f);
    assert!(!drop.is_null(), "a drop runs");
    (rect(&drop["rect"]), rect(&drop["target"]))
}

/// The windows the picture of `workspace`'s cell on the selection's output draws.
fn in_picture(f: &Fixture, workspace: u64) -> Vec<u64> {
    let bar = overview(f)["bar"].as_array().unwrap().clone();
    let cell = bar
        .iter()
        .find(|cell| cell["workspace"] == workspace)
        .unwrap();
    let windows = cell["windows"].as_array().unwrap();
    windows.iter().map(|id| id.as_u64().unwrap()).collect()
}

/// `size` times `scale`, rounded as the drag draws it.
fn scaled(size: Size<i32, Logical>, scale: f64) -> Size<i32, Logical> {
    let at = |n: i32| (f64::from(n) * scale).round() as i32;
    Size::from((at(size.w), at(size.h)))
}

/// The scale of the cells of a bar on an output `width` wide.
fn scale_of(cell: Rectangle<i32, Logical>, width: i32) -> f64 {
    f64::from(cell.size.w) / f64::from(width)
}

/// Whether `size` is strictly between `from` and `to` on both axes.
fn is_between(size: Size<i32, Logical>, from: Size<i32, Logical>, to: Size<i32, Logical>) -> bool {
    let within = |n: i32, a: i32, b: i32| a.min(b) < n && n < a.max(b);
    within(size.w, from.w, to.w) && within(size.h, from.h, to.h)
}

/// Whether `rect` is between `from` and `to`, at neither.
fn is_on_its_way(
    rect: Rectangle<i32, Logical>,
    from: Rectangle<i32, Logical>,
    to: Rectangle<i32, Logical>,
) -> bool {
    let within = |n: i32, a: i32, b: i32| a.min(b) <= n && n <= a.max(b);
    within(rect.loc.x, from.loc.x, to.loc.x)
        && within(rect.loc.y, from.loc.y, to.loc.y)
        && within(rect.size.w, from.size.w, to.size.w)
        && within(rect.size.h, from.size.h, to.size.h)
        && rect != from
        && rect != to
}

/// The point of `rect` at `grab`, parts of its width and height from its top left corner.
fn at_grab(rect: Rectangle<i32, Logical>, grab: (f64, f64)) -> Point<f64, Logical> {
    let rect = rect.to_f64();
    Point::from((
        rect.loc.x + grab.0 * rect.size.w,
        rect.loc.y + grab.1 * rect.size.h,
    ))
}

/// Whether `a` and `b` are within a pixel of each other on each axis.
fn near(a: Point<f64, Logical>, b: Point<i32, Logical>) -> bool {
    (a.x - f64::from(b.x)).abs() <= 1.0 && (a.y - f64::from(b.y)).abs() <= 1.0
}

on_both_kinds! {
    fn over_a_cell_a_dragged_window_eases_to_its_scale_about_its_grab_point(kind: Kind) {
        let (mut f, id) = desk(kind, true);
        let [first, ..] = map_three(&mut f, id);
        open_at_rest(&mut f);
        let layout = place(&f, first);
        let real = f.visible_frame(first);
        let to = middle(cell(&f, 1));

        let pressed = drag_to(&mut f, first, to);

        // It just got there, at its layout size.
        assert_eq!(dragged(&f).size, layout.size);
        let grab = (
            f64::from(pressed.x - layout.loc.x) / f64::from(layout.size.w),
            f64::from(pressed.y - layout.loc.y) / f64::from(layout.size.h),
        );
        let small = scaled(real.size, scale_of(cell(&f, 1), OUTPUT.0));
        f.advance_clock(MIDDLE);
        assert!(is_between(dragged(&f).size, small, layout.size), "{:?}", dragged(&f));
        assert!(near(at_grab(dragged(&f), grab), to));
        f.advance_clock(LENGTH - MIDDLE);
        assert_eq!(dragged(&f).size, small);
        assert!(near(at_grab(dragged(&f), grab), to));
        // The same point of it is under the pointer, which is in the cell.
        assert!(cell(&f, 1).contains(to));

        // From one cell to another of the same bar, its size stays.
        let next = middle(cell(&f, 2));
        f.pointer_move(f64::from(next.x), f64::from(next.y));
        assert_eq!(dragged(&f).size, small);
        assert!(near(at_grab(dragged(&f), grab), next));
    }
}

#[test]
fn the_grabbed_point_stays_under_the_pointer_when_the_layout_changes() {
    let (mut f, id) = desk(Kind::Xdg, true);
    let [first, second, third] = map_three(&mut f, id);
    open_at_rest(&mut f);
    let layout = place(&f, first);
    let to = middle(cell(&f, 1));
    let pressed = drag_to(&mut f, first, to);
    f.advance_clock(LENGTH);
    let grab = (
        f64::from(pressed.x - layout.loc.x) / f64::from(layout.size.w),
        f64::from(pressed.y - layout.loc.y) / f64::from(layout.size.h),
    );

    // The others leave it: its slot is laid out anew, larger.
    destroy(&mut f, second);
    destroy(&mut f, third);
    assert_ne!(place(&f, first), layout);

    assert!(near(at_grab(dragged(&f), grab), to), "{:?}", dragged(&f));
}

#[test]
fn off_the_cell_it_eases_back_to_its_layout_size_from_where_it_is() {
    let (mut f, id) = desk(Kind::Xdg, true);
    let windows = map_three(&mut f, id);
    open_at_rest(&mut f);
    let layout = place(&f, windows[0]).size;
    let small = scaled(
        f.visible_frame(windows[0]).size,
        scale_of(cell(&f, 1), OUTPUT.0),
    );
    let spot = empty_spot(&f, &windows);
    let to = middle(cell(&f, 1));
    drag_to(&mut f, windows[0], to);
    f.advance_clock(MIDDLE);
    let part_way = dragged(&f).size;
    assert!(is_between(part_way, small, layout));

    f.pointer_move(f64::from(spot.x), f64::from(spot.y));

    // It reverses from the size it's at.
    assert_eq!(dragged(&f).size, part_way);
    f.advance_clock(MIDDLE);
    assert!(
        is_between(dragged(&f).size, part_way, layout),
        "{:?}",
        dragged(&f)
    );
    f.advance_clock(LENGTH - MIDDLE);
    assert_eq!(dragged(&f).size, layout);
}

#[test]
fn a_drop_on_another_workspaces_cell_ends_at_its_place_in_the_cells_picture() {
    let (mut f, id) = desk(Kind::Xdg, true);
    let [first, ..] = map_three(&mut f, id);
    let window = f.pid(first).0;
    open_at_rest(&mut f);
    let to = middle(cell(&f, 1));
    drag_to(&mut f, first, to);
    f.advance_clock(LENGTH);
    let under_the_pointer = dragged(&f);

    f.pointer_button(BTN_LEFT, false);

    // Policy moved it at the release.
    assert_eq!(f.state.caminito.policy.workspace_of(f.pid(first)), Some(1));
    assert!(drag(&f).is_null());
    assert_eq!(dropping(&f)["window"], window);
    assert_eq!(dropping(&f)["to"], "cell");
    let (at, target) = drop_rects(&f);
    assert_eq!(at, under_the_pointer);
    // Its real rectangle on that workspace, scaled into the cell.
    let real = f.visible_frame(first);
    assert_eq!(target, scaled_into(real, cell(&f, 1), OUTPUT.into()));
    assert!(!in_picture(&f, 1).contains(&window));

    f.advance_clock(MIDDLE);
    assert!(is_on_its_way(drop_rects(&f).0, under_the_pointer, target));
    assert!(!in_picture(&f, 1).contains(&window));
    f.advance_clock(ALMOST - MIDDLE);
    assert_eq!(drop_rects(&f).0, target);
    assert!(!in_picture(&f, 1).contains(&window));
    f.advance_clock(LENGTH - ALMOST);
    assert!(dropping(&f).is_null());
    assert!(in_picture(&f, 1).contains(&window));
}

#[test]
fn a_drop_on_nothing_ends_at_its_slot() {
    let (mut f, id) = desk(Kind::Xdg, true);
    let windows = map_three(&mut f, id);
    open_at_rest(&mut f);
    let slot = place(&f, windows[0]);
    let spot = empty_spot(&f, &windows);
    drag_to(&mut f, windows[0], spot);
    let under_the_pointer = dragged(&f);

    f.pointer_button(BTN_LEFT, false);

    assert_eq!(dropping(&f)["to"], "slot");
    let (at, target) = drop_rects(&f);
    assert_eq!((at, target), (under_the_pointer, slot));
    // The slot leaves it out until it's there.
    let in_slot = middle(slot);
    assert!(!at.contains(in_slot));
    if !windows[1..].iter().any(|w| place(&f, *w).contains(in_slot)) {
        assert_ne!(pixel(&mut f, in_slot), WINDOW);
    }
    f.advance_clock(MIDDLE);
    assert!(is_on_its_way(drop_rects(&f).0, under_the_pointer, slot));
    f.advance_clock(ALMOST - MIDDLE);
    assert_eq!(drop_rects(&f).0, slot);
    f.advance_clock(LENGTH - ALMOST);
    assert!(dropping(&f).is_null());
    assert_eq!(place(&f, windows[0]), slot);
}

#[test]
fn after_esc_the_window_eases_back_to_its_slot() {
    let (mut f, id) = desk(Kind::Xdg, true);
    let [first, ..] = map_three(&mut f, id);
    open_at_rest(&mut f);
    let slot = place(&f, first);
    let to = middle(cell(&f, 1));
    drag_to(&mut f, first, to);
    f.advance_clock(MIDDLE);
    let under_the_pointer = dragged(&f);

    f.chord(&[KEY_ESC]);

    assert!(drag(&f).is_null());
    assert_eq!(dropping(&f)["to"], "slot");
    assert_eq!(drop_rects(&f), (under_the_pointer, slot));
    f.advance_clock(LENGTH);
    assert!(dropping(&f).is_null());
}

/// Two outputs of different sizes, `workspaces-per-output` on and `overview-animation` on,
/// with Caminito's clock stopped: on the left one a window at (100, 100), focused, and one
/// at (900, 500), and on the right one a window at (2200, 200).
fn two_sizes() -> (Fixture, Output, Output, [ClientWindow; 3]) {
    let mut f = Fixture::with_settings(vec![
        (CAMINITO, "workspaces-per-output".into(), Value::Bool(true)),
        (CAMINITO, "overview-animation".into(), Value::Bool(true)),
    ]);
    let left = f.add_output((1920, 1080), 1.0);
    let right = f.add_output((1280, 720), 1.0);
    f.choose_primary_output(&left);
    assert_eq!(output_geometry(&right).loc, (1920, 0).into());
    let client = f.connect(Kind::Xdg);
    let first = window_at(&mut f, client, (100, 100), 0);
    let second = window_at(&mut f, client, (900, 500), 0);
    let third = window_at(&mut f, client, (2200, 200), 0);
    let id = f.pid(first);
    let now = f.state.caminito.now();
    f.state.caminito.policy.activate(SEAT, id, now);
    f.dispatch();
    f.stop_clock();
    (f, left, right, [first, second, third])
}

#[test]
fn a_drag_onto_the_other_outputs_bar_takes_its_scale_and_its_drop_ends_at_its_new_geometry_there() {
    let (mut f, left, right, [first, ..]) = two_sizes();
    open_at_rest(&mut f);
    let real = f.visible_frame(first);
    let left_scale = scale_of(cell_on(&f, &left, 1), 1920);
    let right_cell = cell_on(&f, &right, 2);
    let right_scale = scale_of(right_cell, 1280);
    assert!(
        (left_scale - right_scale).abs() > 0.01,
        "{left_scale} {right_scale}"
    );
    let start = global(&left, middle(place_on(&f, &left, first)));
    f.pointer_move(f64::from(start.x), f64::from(start.y));
    f.pointer_button(BTN_LEFT, true);

    // Over the left output's own bar first, then onto the right one's.
    let own = global(&left, middle(cell_on(&f, &left, 1)));
    for at in [start + Point::from((0, -100)), own] {
        f.pointer_move(f64::from(at.x), f64::from(at.y));
    }
    f.advance_clock(LENGTH);
    assert_eq!(dragged(&f).size, scaled(real.size, left_scale));
    let to = global(&right, middle(right_cell));
    f.pointer_move(f64::from(to.x), f64::from(to.y));
    // Early, as the two sizes are a few pixels apart.
    let early = Duration::from_millis(40);
    f.advance_clock(early);
    let part_way = dragged(&f).size;
    let (small, smaller) = (
        scaled(real.size, left_scale),
        scaled(real.size, right_scale),
    );
    assert!(
        is_between(part_way, smaller, small),
        "{part_way:?} {smaller:?} {small:?}"
    );
    f.advance_clock(LENGTH - early);
    assert_eq!(dragged(&f).size, scaled(real.size, right_scale));
    let under_the_pointer = dragged(&f);

    f.pointer_button(BTN_LEFT, false);

    let policy = &f.state.caminito.policy;
    assert_eq!(policy.workspace_of(f.pid(first)), Some(2));
    // Its new geometry on the right output, scaled into that cell, from the left output's
    // corner, which the selection is on.
    let origin = output_geometry(&right).loc;
    let moved = f.visible_frame(first);
    let local = Rectangle::new(moved.loc - origin, moved.size);
    let in_cell = scaled_into(local, right_cell, (1280, 720).into());
    let target = Rectangle::new(in_cell.loc + origin, in_cell.size);
    assert_eq!(drop_rects(&f), (under_the_pointer, target));
    f.advance_clock(ALMOST);
    assert_eq!(drop_rects(&f).0, target);
    f.advance_clock(LENGTH - ALMOST);
    assert!(dropping(&f).is_null());
    let bar = on(&f, &right)["bar"].as_array().unwrap().clone();
    let windows = bar[2]["windows"].as_array().unwrap().clone();
    assert!(windows.contains(&f.pid(first).0.into()));
}

#[test]
fn without_the_animation_every_size_and_place_is_reached_in_one_frame() {
    let (mut f, id) = desk(Kind::Xdg, false);
    let [first, ..] = map_three(&mut f, id);
    open(&mut f);
    let layout = place(&f, first);
    let real = f.visible_frame(first);

    let to = middle(cell(&f, 1));
    drag_to(&mut f, first, to);
    assert_eq!(
        dragged(&f).size,
        scaled(real.size, scale_of(cell(&f, 1), OUTPUT.0))
    );
    f.pointer_move(f64::from(layout.loc.x), f64::from(layout.loc.y + 50));
    assert_eq!(dragged(&f).size, layout.size);
    let to = middle(cell(&f, 1));
    f.pointer_move(f64::from(to.x), f64::from(to.y));
    f.pointer_button(BTN_LEFT, false);

    assert!(dropping(&f).is_null());
    assert!(in_picture(&f, 1).contains(&f.pid(first).0));
}

/// Drops `window` on `workspace`'s cell, or on nothing, and checks a drop runs.
fn drop_on(f: &mut Fixture, windows: &[ClientWindow], workspace: Option<u64>) {
    let to = match workspace {
        Some(workspace) => middle(cell(f, workspace)),
        None => empty_spot(f, windows),
    };
    drag_to(f, windows[0], to);
    f.pointer_button(BTN_LEFT, false);
    assert!(!dropping(f).is_null(), "a drop runs");
}

#[test]
fn the_overview_closing_cuts_the_drop_to_its_end() {
    let (mut f, id) = desk(Kind::Xdg, true);
    let windows = map_three(&mut f, id);
    open_at_rest(&mut f);
    drop_on(&mut f, &windows, Some(1));

    f.chord(&[KEY_ESC]);

    assert!(f.state.caminito.overview_drop.is_none());
}

#[test]
fn a_switch_to_the_drops_workspace_cuts_it_to_its_end() {
    let (mut f, id) = desk(Kind::Xdg, true);
    let windows = map_three(&mut f, id);
    open_at_rest(&mut f);
    drop_on(&mut f, &windows, Some(1));

    // With its binding: a click on the cell would be a press on the window drawn there,
    // which cuts the drop too.
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);

    assert_eq!(f.state.caminito.policy.active_workspace(), 1);
    assert!(dropping(&f).is_null());
}

#[test]
fn a_switch_away_from_a_drop_to_its_slot_cuts_it_to_its_end() {
    let (mut f, id) = desk(Kind::Xdg, true);
    let windows = map_three(&mut f, id);
    open_at_rest(&mut f);
    drop_on(&mut f, &windows, None);

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);

    // Its slot is the old workspace's, which the layout leaves: it's in the picture of
    // that workspace's cell again.
    assert_eq!(f.state.caminito.policy.active_workspace(), 1);
    assert!(dropping(&f).is_null());
    assert!(in_picture(&f, 0).contains(&f.pid(windows[0]).0));
}

#[test]
fn a_switch_to_another_workspace_leaves_the_drop_on_its_way() {
    let (mut f, id) = desk(Kind::Xdg, true);
    let windows = map_three(&mut f, id);
    open_at_rest(&mut f);
    drop_on(&mut f, &windows, Some(1));

    let at = middle(cell(&f, 2));
    click(&mut f, at);

    assert_eq!(f.state.caminito.policy.active_workspace(), 2);
    assert!(!dropping(&f).is_null());
}

#[test]
fn a_press_on_the_window_cuts_the_drop_to_its_end() {
    let (mut f, id) = desk(Kind::Xdg, true);
    let windows = map_three(&mut f, id);
    open_at_rest(&mut f);
    let slot = place(&f, windows[0]);
    drop_on(&mut f, &windows, None);

    let at = middle(slot);
    f.pointer_move(f64::from(at.x), f64::from(at.y));
    f.pointer_button(BTN_LEFT, true);

    assert!(dropping(&f).is_null());
}
