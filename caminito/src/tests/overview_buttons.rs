//! The workspace bindings on mouse buttons in the Overview, and its selection on the
//! Active output (248): a button bound to a workspace switch does what its key does
//! wherever the pointer is, a button bound to the Overview closes it, and another bound
//! button is swallowed over a window and closes it over the empty area. On two outputs, a
//! click on the other output's cell makes that output the Active output, which a workspace
//! key and a workspace button then switch.
//!
//! `switch-to-workspace-left` and `-right` are on `Button8` and `Button9`, the back and
//! forward buttons, as the developer has them, and the Workspace slide is on. What a press
//! over a window does runs on both kinds of window. The rest map xdg toplevels only: it's
//! the Overview's own state, and doesn't depend on the kind.

use smithay::output::Output;
use smithay::utils::{Logical, Point};

use super::button_bindings::{BTN_EXTRA, BTN_SIDE};
use super::fixture::{
    BTN_LEFT, ClientId, Fixture, KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT, Kind, on_both_kinds,
};
use super::overview::{
    OUTPUT, cell, drag, drag_to, empty_spot, is_open, listed, listed_workspace, middle,
    one_on_each_of_two, open, overview, place, selected, slides,
};
use super::overview_outputs::{cell_on, desk_with, global, listed_on};
use super::per_output_workspaces::active_workspaces;
use crate::output::output_id;
use crate::settings::Value;
use crate::settings::translate::{CAMINITO, MARCO_GLOBAL_KEYBINDINGS, MARCO_WINDOW_KEYBINDINGS};
use crate::state::SEAT;

fn setting(schema: &'static str, key: &str, value: &str) -> (&'static str, String, Value) {
    (schema, key.into(), Value::Str(value.into()))
}

/// The developer's workspace buttons: `switch-to-workspace-left` on `Button8` and `-right`
/// on `Button9`.
fn workspace_buttons() -> Vec<(&'static str, String, Value)> {
    vec![
        setting(
            MARCO_GLOBAL_KEYBINDINGS,
            "switch-to-workspace-left",
            "Button8",
        ),
        setting(
            MARCO_GLOBAL_KEYBINDINGS,
            "switch-to-workspace-right",
            "Button9",
        ),
    ]
}

/// One output, `settings` and the Workspace slide on, and a client of `kind`.
fn with(kind: Kind, mut settings: Vec<(&'static str, String, Value)>) -> (Fixture, ClientId) {
    settings.push((CAMINITO, "workspace-slide".into(), Value::Bool(true)));
    let mut f = Fixture::with_settings(settings);
    f.add_output(OUTPUT, 1.0);
    let id = f.connect(kind);
    (f, id)
}

/// Presses and releases `button` with the pointer at `at`.
fn press_at(f: &mut Fixture, at: Point<i32, Logical>, button: u32) {
    f.pointer_move(f64::from(at.x), f64::from(at.y));
    f.pointer_button(button, true);
    f.pointer_button(button, false);
}

/// Whether the Workspace popup shows, from the debug dump's `workspace_popup`.
fn popup_shown(f: &Fixture) -> bool {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    !dump["workspace_popup"].is_null()
}

/// The Overview is open on `workspace`, which the output has switched to with the
/// Workspace slide, and no Workspace popup shows.
#[track_caller]
fn switched_to(f: &Fixture, workspace: usize) {
    assert!(is_open(f), "the Overview stays open");
    assert_eq!(f.state.caminito.policy.active_workspace(), workspace);
    assert_eq!(listed_workspace(f), workspace as u64);
    assert!(slides(f), "it slides");
    assert!(!popup_shown(f), "no Workspace popup shows");
}

// One output.

on_both_kinds! {
    fn a_workspace_button_over_a_window_switches_and_neither_chooses_nor_closes_it(kind: Kind) {
        let (mut f, id) = with(kind, workspace_buttons());
        let [first, second] = one_on_each_of_two(&mut f, id);
        open(&mut f);
        let focus = f.state.caminito.policy.focus(SEAT);

        let at = middle(place(&f, first));
        press_at(&mut f, at, BTN_EXTRA);

        switched_to(&f, 1);
        f.dispatch();
        assert!(f.state.caminito.policy.window(f.pid(first)).is_some(), "it isn't closed");
        assert_eq!(f.state.caminito.policy.workspace_of(f.pid(first)), Some(0));

        // And back, over the other window.
        let at = middle(place(&f, second));
        press_at(&mut f, at, BTN_SIDE);
        switched_to(&f, 0);
        f.dispatch();
        assert!(f.state.caminito.policy.window(f.pid(second)).is_some());
        assert_eq!(f.state.caminito.policy.focus(SEAT), focus);
    }
}

#[test]
fn a_workspace_button_over_a_cell_switches_to_the_bindings_workspace_not_the_cells() {
    let (mut f, id) = with(Kind::Xdg, workspace_buttons());
    one_on_each_of_two(&mut f, id);
    open(&mut f);

    let at = middle(cell(&f, 3));
    press_at(&mut f, at, BTN_EXTRA);
    switched_to(&f, 1);

    let at = middle(cell(&f, 3));
    press_at(&mut f, at, BTN_SIDE);
    switched_to(&f, 0);
}

#[test]
fn a_workspace_button_over_the_empty_area_switches_and_leaves_it_open() {
    let (mut f, id) = with(Kind::Xdg, workspace_buttons());
    let [first, _] = one_on_each_of_two(&mut f, id);
    open(&mut f);

    let at = empty_spot(&f, &[first]);
    press_at(&mut f, at, BTN_EXTRA);

    switched_to(&f, 1);
    press_at(&mut f, at, BTN_SIDE);
    switched_to(&f, 0);
}

#[test]
fn a_workspace_button_during_a_drag_ends_it_as_a_workspace_key_does() {
    let (mut f, id) = with(Kind::Xdg, workspace_buttons());
    let [first, _] = one_on_each_of_two(&mut f, id);
    open(&mut f);
    let target = middle(cell(&f, 2));
    drag_to(&mut f, first, target);
    assert!(!drag(&f).is_null());

    f.pointer_button(BTN_EXTRA, true);
    f.pointer_button(BTN_EXTRA, false);

    switched_to(&f, 1);
    assert!(drag(&f).is_null(), "the drag ends");
    // The release after is no drop, and no click.
    f.pointer_button(BTN_LEFT, false);
    assert!(is_open(&f));
    assert_eq!(f.state.caminito.policy.workspace_of(f.pid(first)), Some(0));
}

#[test]
fn a_button_bound_to_the_overview_still_closes_it() {
    let settings = vec![setting(CAMINITO, "overview", "Button8")];
    let (mut f, id) = with(Kind::Xdg, settings);
    let [first, _] = one_on_each_of_two(&mut f, id);
    open_with_button(&mut f, first_spot());

    let at = middle(place(&f, first));
    press_at(&mut f, at, BTN_SIDE);

    assert!(!is_open(&f));
    assert_eq!(f.state.caminito.policy.active_workspace(), 0);
}

/// A spot on the wallpaper, away from the windows `one_on_each_of_two` maps, which are
/// centred.
fn first_spot() -> Point<i32, Logical> {
    Point::from((20, OUTPUT.1 - 20))
}

/// Opens the Overview with its shortcut on `Button8`, pressed at `at`.
#[track_caller]
fn open_with_button(f: &mut Fixture, at: Point<i32, Logical>) {
    press_at(f, at, BTN_SIDE);
    assert!(is_open(f), "the button opens the Overview");
}

#[test]
fn another_bound_button_does_nothing_over_a_window_and_closes_it_over_the_empty_area() {
    let settings = vec![setting(
        MARCO_WINDOW_KEYBINDINGS,
        "toggle-maximized",
        "Button8",
    )];
    let (mut f, id) = with(Kind::Xdg, settings);
    let [first, _] = one_on_each_of_two(&mut f, id);
    open(&mut f);
    let before = (listed(&f), selected(&f));

    let at = middle(place(&f, first));
    press_at(&mut f, at, BTN_SIDE);

    assert!(is_open(&f));
    assert_eq!((listed(&f), selected(&f)), before);
    let maximized = |f: &Fixture| f.state.caminito.policy.is_maximized(f.pid(first));
    assert!(!maximized(&f));
    assert_eq!(f.state.caminito.policy.active_workspace(), 0);

    let at = empty_spot(&f, &[first]);
    press_at(&mut f, at, BTN_SIDE);

    assert!(!is_open(&f));
    assert!(!maximized(&f));
    assert_eq!(f.state.caminito.policy.active_workspace(), 0);
}

// Two outputs.

/// The name of the output the open Overview's selection is on.
fn selection_output(f: &Fixture) -> String {
    overview(f)["output"].as_str().unwrap().to_owned()
}

/// `overview_outputs`' desk, under `per_output`, with `switch-to-workspace-left` on
/// `Button8` and `-right` on its key, open with the selection on the right output, and
/// then a click on the left output's cell of workspace 1.
fn clicked_on_the_left(per_output: bool) -> (Fixture, Output, Output) {
    let settings = vec![setting(
        MARCO_GLOBAL_KEYBINDINGS,
        "switch-to-workspace-left",
        "Button8",
    )];
    let (mut f, _, left, right, _) = desk_with(Kind::Xdg, per_output, settings);
    open(&mut f);
    assert_eq!(selection_output(&f), right.name());
    let focus = f.state.caminito.policy.focus(SEAT);

    let at = global(&left, middle(cell_on(&f, &left, 1)));
    press_at(&mut f, at, BTN_LEFT);

    assert!(is_open(&f));
    assert_eq!(selection_output(&f), left.name());
    assert_eq!(selected(&f), None);
    // Under Spanning workspaces the switch of every output gives the seat its default
    // focus, as it did before; the click itself leaves it alone.
    if per_output {
        assert_eq!(f.state.caminito.policy.focus(SEAT), focus);
    }
    let active = f.state.caminito.policy.active_output(SEAT);
    assert_eq!(active, Some(output_id(&left)));
    (f, left, right)
}

#[test]
fn under_per_output_workspaces_after_a_click_on_the_other_outputs_cell_the_bindings_switch_it() {
    let (mut f, left, right) = clicked_on_the_left(true);
    assert_eq!(active_workspaces(&f, &left, &right), (1, 0));

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    assert!(is_open(&f));
    assert_eq!(active_workspaces(&f, &left, &right), (2, 0));
    assert_eq!(listed_on(&f, &left), Vec::<u64>::new());

    let at = global(&right, Point::from((960, 900)));
    press_at(&mut f, at, BTN_SIDE);
    assert!(is_open(&f));
    assert_eq!(active_workspaces(&f, &left, &right), (1, 0));
    assert!(!popup_shown(&f));
}

#[test]
fn under_spanning_workspaces_after_a_click_on_the_other_outputs_cell_the_bindings_switch_it() {
    let (mut f, left, right) = clicked_on_the_left(false);
    assert_eq!(active_workspaces(&f, &left, &right), (1, 1));

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    assert!(is_open(&f));
    assert_eq!(active_workspaces(&f, &left, &right), (2, 2));
    assert_eq!(selection_output(&f), left.name());

    let at = global(&right, Point::from((960, 900)));
    press_at(&mut f, at, BTN_SIDE);
    assert!(is_open(&f));
    assert_eq!(active_workspaces(&f, &left, &right), (1, 1));
    assert_eq!(selection_output(&f), left.name());
    assert!(!popup_shown(&f));
}
