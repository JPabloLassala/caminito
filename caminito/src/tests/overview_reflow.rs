//! The layout's windows easing to their new slots in the Overview (250), read from the
//! debug dump: when a window is dropped on another workspace's cell, each window left
//! eases from where it was drawn to its new slot, and a second drop part-way eases each
//! from where it's drawn then. With two outputs, a window dragged from one onto the
//! other's layout eases the windows of both. A window that closes eases the others too,
//! and one that opens doesn't (242). A click part-way chooses the window drawn under the
//! pointer. With `overview-animation` off, the new slots are taken in one frame, and the
//! Overview closing or a switch of the workspace cuts the easing to its end.
//!
//! Each test stops Caminito's clock, so the easing is where `advance_clock` takes it. What
//! it draws is the Overview's own and doesn't depend on the kind of window, so the tests
//! map xdg toplevels only.

use std::time::Duration;

use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle};

use super::fixture::{
    BTN_LEFT, ClientId, ClientWindow, Fixture, KEY_ESC, KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT, Kind,
    WindowSpec,
};
use super::overview::{
    OUTPUT, WINDOW, cell, click, destroy, drag_to, focus, is_open, map_three, middle, open,
    overview, pixel, place, rect,
};
use super::overview_outputs::{self, desk_with, layout_spot, listed_on, on, place_on};
use crate::settings::Value;
use crate::settings::translate::CAMINITO;
use crate::workspace_slide::LENGTH;

/// The middle of the easing, where the ease-out has the windows seven eighths of the way.
const MIDDLE: Duration = Duration::from_millis(125);

/// The last millisecond before the end.
const ALMOST: Duration = Duration::from_millis(249);

/// One output with `overview-animation` as `animated` says, and an xdg client, with
/// Caminito's clock stopped.
fn desk(animated: bool) -> (Fixture, ClientId) {
    let setting = (CAMINITO, "overview-animation".into(), Value::Bool(animated));
    let mut f = Fixture::with_settings(vec![setting]);
    f.add_output(OUTPUT, 1.0);
    let id = f.connect(Kind::Xdg);
    f.stop_clock();
    (f, id)
}

/// Opens the Overview and lets its transition finish.
fn open_at_rest(f: &mut Fixture) {
    open(f);
    f.advance_clock(LENGTH);
}

/// Each window of a layout's dump, `on`, with where it's drawn on its way to its new slot:
/// empty once they're there.
fn reflow_of(on: &serde_json::Value) -> Vec<(u64, Rectangle<i32, Logical>)> {
    let reflow = on["reflow"].as_array().unwrap().clone();
    reflow
        .iter()
        .map(|window| (window["window"].as_u64().unwrap(), rect(&window["rect"])))
        .collect()
}

/// The selection's output's layout's windows on their way to their new slots.
fn reflow(f: &Fixture) -> Vec<(u64, Rectangle<i32, Logical>)> {
    reflow_of(&overview(f))
}

/// Where `window` is drawn in the layout of `reflow` on its way to its new slot.
#[track_caller]
fn drawn_in(
    f: &Fixture,
    reflow: &[(u64, Rectangle<i32, Logical>)],
    window: ClientWindow,
) -> Rectangle<i32, Logical> {
    let id = f.pid(window).0;
    let found = reflow.iter().find(|(listed, _)| *listed == id);
    found.expect("the window is on its way to its new slot").1
}

/// Where `window` is drawn on the selection's output on its way to its new slot.
#[track_caller]
fn drawn(f: &Fixture, window: ClientWindow) -> Rectangle<i32, Logical> {
    drawn_in(f, &reflow(f), window)
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

/// Where `window`'s label is in the selection's output's layout.
#[track_caller]
fn label(f: &Fixture, window: ClientWindow) -> Rectangle<i32, Logical> {
    let id = f.pid(window).0;
    let windows = overview(f)["windows"].as_array().unwrap().clone();
    let listed = windows.iter().find(|listed| listed["window"] == id);
    rect(&listed.expect("the window is listed")["label"])
}

/// Drags `window` onto `workspace`'s cell and drops it there.
fn drop_on_cell(f: &mut Fixture, window: ClientWindow, workspace: u64) {
    let to = middle(cell(f, workspace));
    drag_to(f, window, to);
    f.pointer_button(BTN_LEFT, false);
    let policy = &f.state.caminito.policy;
    assert_eq!(
        policy.workspace_of(f.pid(window)),
        Some(workspace as usize),
        "the drop sends the window to the workspace"
    );
}

#[test]
fn after_a_drop_on_another_workspaces_cell_the_windows_left_ease_to_their_new_slots() {
    let (mut f, id) = desk(true);
    let [first, second, third] = map_three(&mut f, id);
    open_at_rest(&mut f);
    let old = [second, third].map(|window| place(&f, window));
    assert!(reflow(&f).is_empty(), "nothing eases before the drop");

    drop_on_cell(&mut f, first, 1);

    let new = [second, third].map(|window| place(&f, window));
    assert_ne!(old, new, "the layout changed");
    // At first each is drawn where it was.
    let left = [second, third].map(|window| f.pid(window).0);
    let eased: Vec<u64> = reflow(&f).iter().map(|(id, _)| *id).collect();
    assert_eq!(eased, left);
    for (window, old) in [second, third].into_iter().zip(old) {
        assert_eq!(drawn(&f, window), old);
    }
    // The scene draws them there: the middle of an old place that no new slot covers.
    let uncovered = old
        .iter()
        .find(|old| !new.iter().any(|new| new.contains(middle(**old))));
    let uncovered = uncovered.expect("an old place is clear of the new slots");
    assert_eq!(pixel(&mut f, middle(*uncovered)), WINDOW);

    f.advance_clock(MIDDLE);
    for (window, (old, new)) in [second, third].into_iter().zip(old.into_iter().zip(new)) {
        if old != new {
            assert!(is_on_its_way(drawn(&f, window), old, new));
        }
    }
    f.advance_clock(ALMOST - MIDDLE);
    for (window, new) in [second, third].into_iter().zip(new) {
        assert_eq!(drawn(&f, window), new);
    }
    f.advance_clock(LENGTH - ALMOST);
    assert!(reflow(&f).is_empty());
    assert_eq!([second, third].map(|window| place(&f, window)), new);
}

#[test]
fn a_second_drop_part_way_eases_from_where_they_are_drawn() {
    let (mut f, id) = desk(true);
    let [first, second, third] = map_three(&mut f, id);
    open_at_rest(&mut f);
    drop_on_cell(&mut f, first, 1);
    f.advance_clock(MIDDLE);
    let part_way = drawn(&f, third);
    assert_ne!(part_way, place(&f, third), "it's on its way");

    drop_on_cell(&mut f, second, 2);

    let newest = place(&f, third);
    assert_ne!(newest, part_way);
    assert_eq!(drawn(&f, third), part_way);
    f.advance_clock(MIDDLE);
    assert!(is_on_its_way(drawn(&f, third), part_way, newest));
    // The 250 ms count from the second drop.
    f.advance_clock(ALMOST - MIDDLE);
    assert_eq!(drawn(&f, third), newest);
    f.advance_clock(LENGTH - ALMOST);
    assert!(reflow(&f).is_empty());
}

/// The layout of `output`'s windows on their way to their new slots.
fn reflow_on(f: &Fixture, output: &Output) -> Vec<(u64, Rectangle<i32, Logical>)> {
    reflow_of(&on(f, output))
}

#[test]
fn with_two_outputs_the_layouts_of_both_ease_after_a_drag_from_one_to_the_other() {
    let animated = (CAMINITO, "overview-animation".into(), Value::Bool(true));
    let (mut f, _, left, right, [first, second, third]) =
        desk_with(Kind::Xdg, true, vec![animated]);
    f.stop_clock();
    open_at_rest(&mut f);
    let old_left = place_on(&f, &left, second);
    let old_right = place_on(&f, &right, third);
    let to = layout_spot(&f, &right);

    overview_outputs::drag_to(&mut f, &left, first, to);
    f.pointer_button(BTN_LEFT, false);

    assert!(listed_on(&f, &right).contains(&f.pid(first).0));
    let new_left = place_on(&f, &left, second);
    let new_right = place_on(&f, &right, third);
    assert_ne!(old_left, new_left, "the left layout changed");
    assert_ne!(old_right, new_right, "the right layout changed");
    assert_eq!(drawn_in(&f, &reflow_on(&f, &left), second), old_left);
    assert_eq!(drawn_in(&f, &reflow_on(&f, &right), third), old_right);

    f.advance_clock(MIDDLE);
    let left_way = drawn_in(&f, &reflow_on(&f, &left), second);
    let right_way = drawn_in(&f, &reflow_on(&f, &right), third);
    assert!(is_on_its_way(left_way, old_left, new_left));
    assert!(is_on_its_way(right_way, old_right, new_right));
    f.advance_clock(LENGTH - MIDDLE);
    assert!(reflow_on(&f, &left).is_empty());
    assert!(reflow_on(&f, &right).is_empty());
}

#[test]
fn without_the_animation_the_new_slots_are_taken_in_one_frame() {
    let (mut f, id) = desk(false);
    let [first, second, third] = map_three(&mut f, id);
    open(&mut f);
    let old = [second, third].map(|window| place(&f, window));

    drop_on_cell(&mut f, first, 1);

    assert!(reflow(&f).is_empty());
    let new = [second, third].map(|window| place(&f, window));
    assert_ne!(old, new, "the layout changed");
    for new in new {
        assert_eq!(pixel(&mut f, middle(new)), WINDOW, "{new:?}");
    }
}

#[test]
fn the_overview_closing_cuts_the_easing_to_its_end() {
    let (mut f, id) = desk(true);
    let [first, ..] = map_three(&mut f, id);
    open_at_rest(&mut f);
    drop_on_cell(&mut f, first, 1);
    assert!(!reflow(&f).is_empty());

    f.chord(&[KEY_ESC]);

    let shown = &f.state.caminito.overview;
    assert!(!shown.is_empty(), "it's drawn closing");
    assert!(shown.iter().all(|open| open.reflow.is_none()));
}

#[test]
fn a_switch_of_the_workspace_cuts_the_easing_to_its_end() {
    let (mut f, id) = desk(true);
    let [first, ..] = map_three(&mut f, id);
    open_at_rest(&mut f);
    drop_on_cell(&mut f, first, 1);
    assert!(!reflow(&f).is_empty());

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);

    assert_eq!(f.state.caminito.policy.active_workspace(), 1);
    assert!(reflow(&f).is_empty());
}

#[test]
fn a_window_that_closes_eases_the_others_and_one_that_opens_doesnt() {
    let (mut f, id) = desk(true);
    let [first, second, third] = map_three(&mut f, id);
    open_at_rest(&mut f);
    let old = [first, second].map(|window| place(&f, window));

    destroy(&mut f, third);

    assert_ne!([first, second].map(|window| place(&f, window)), old);
    for (window, old) in [first, second].into_iter().zip(old) {
        assert_eq!(drawn(&f, window), old);
    }
    f.advance_clock(LENGTH);
    assert!(reflow(&f).is_empty());
    let before = [first, second].map(|window| place(&f, window));

    // As before (242): the others take their new slots in that frame.
    f.map(id, WindowSpec::plain((300, 500)));

    assert_ne!([first, second].map(|window| place(&f, window)), before);
    assert!(reflow(&f).is_empty());
}

#[test]
fn a_click_part_way_chooses_the_window_drawn_under_the_pointer() {
    let (mut f, id) = desk(true);
    let [first, second, _] = map_three(&mut f, id);
    open_at_rest(&mut f);
    drop_on_cell(&mut f, first, 1);
    // A point of `second`, which isn't focused, where it's drawn and not where its new slot
    // or its label is: there the layout at rest has another window, or none.
    assert_ne!(focus(&f), Some(f.pid(second).0));
    let drawn_at = drawn(&f, second);
    let slot = place(&f, second);
    let label = label(&f, second);
    let inset = 4;
    let corners = [
        (inset, inset),
        (drawn_at.size.w - inset, inset),
        (inset, drawn_at.size.h - inset),
        (drawn_at.size.w - inset, drawn_at.size.h - inset),
    ];
    let at = corners
        .into_iter()
        .map(|corner| drawn_at.loc + Point::from(corner))
        .find(|at| !slot.contains(*at) && !label.contains(*at));
    let at = at.expect("a corner of the window is drawn away from its new slot");
    let away = second;

    click(&mut f, at);

    assert!(!is_open(&f));
    assert_eq!(focus(&f), Some(f.pid(away).0));
}
