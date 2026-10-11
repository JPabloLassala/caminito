//! The Workspace popup (152): the bindings that show it, the keys and the presses that close
//! it, where it is, and what's drawn.

use caminito_test_client::format_input;
use caminito_test_client::session_lock::LockId;
use insta::assert_snapshot;
use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle};

use super::fixture::{
    BTN_LEFT, ClientId, Fixture, KEY_A, KEY_DOWN, KEY_F7, KEY_LEFT, KEY_LEFTALT, KEY_LEFTCTRL,
    KEY_LEFTSHIFT, KEY_RIGHT, KEY_TAB, KEY_UP, Kind, WindowSpec, on_both_kinds,
};
use crate::output::output_geometry;
use crate::scene::SceneLayer;
use crate::settings::Value;
use crate::settings::translate::{
    CAMINITO, MARCO_GENERAL, MARCO_GLOBAL_KEYBINDINGS, MARCO_WORKSPACE_NAMES,
};
use crate::state::SEAT;

/// The debug dump's `workspace_popup` member: `null` while the Workspace popup is closed.
fn popup(f: &Fixture) -> serde_json::Value {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    dump["workspace_popup"].clone()
}

fn is_open(f: &Fixture) -> bool {
    !popup(f).is_null()
}

/// The workspace the open popup has selected.
fn selected(f: &Fixture) -> u64 {
    popup(f)["selected"].as_u64().unwrap()
}

fn active(f: &Fixture) -> usize {
    f.state.caminito.policy.active_workspace()
}

/// Presses and releases `key`.
fn tap(f: &mut Fixture, key: u32) {
    f.key(key, true);
    f.key(key, false);
}

/// `count` workspaces in two rows on one 1920 × 1080 output, and a client of `kind`. With
/// four:
///
/// ```text
/// 0 1
/// 2 3
/// ```
fn two_rows_of(kind: Kind, count: i32) -> (Fixture, ClientId) {
    let mut f = Fixture::with_settings(vec![
        (CAMINITO, "workspace-layout-rows".into(), Value::Int(2)),
        (MARCO_GENERAL, "num-workspaces".into(), Value::Int(count)),
    ]);
    f.add_output((1920, 1080), 1.0);
    let id = f.connect(kind);
    (f, id)
}

fn two_rows(kind: Kind) -> (Fixture, ClientId) {
    two_rows_of(kind, 4)
}

fn rect(dump: &serde_json::Value) -> Rectangle<i32, Logical> {
    let field = |name: &str| dump[name].as_i64().unwrap() as i32;
    Rectangle::new(
        (field("x"), field("y")).into(),
        (field("width"), field("height")).into(),
    )
}

/// One of the rectangles the dump has for `workspace`: its `cells`, `previews` or `names`.
fn part(f: &Fixture, list: &str, workspace: usize) -> Rectangle<i32, Logical> {
    rect(&popup(f)[list][workspace])
}

fn cell(f: &Fixture, workspace: usize) -> Rectangle<i32, Logical> {
    part(f, "cells", workspace)
}

fn middle(rect: Rectangle<i32, Logical>) -> Point<i32, Logical> {
    rect.loc + Point::from((rect.size.w / 2, rect.size.h / 2))
}

/// `output`'s pixel at a logical point from its corner, as `0xRRGGBB`.
fn pixel_on(f: &mut Fixture, output: &Output, at: Point<i32, Logical>) -> u32 {
    let scale = output.current_scale().fractional_scale();
    let at = at.to_physical_precise_round(scale);
    let pixel = f.state.backend.headless_mut().pixel(output, at);
    pixel.expect("the output is drawn") & 0xff_ffff
}

/// The first output's pixel at a logical point, as `0xRRGGBB`.
fn pixel(f: &mut Fixture, at: Point<i32, Logical>) -> u32 {
    let output = f.state.caminito.outputs[0].clone();
    pixel_on(f, &output, at)
}

fn has_popup_layer(f: &mut Fixture, output: &Output) -> bool {
    let layers = f.state.scene_layers(output);
    layers.contains(&SceneLayer::WorkspacePopup)
}

on_both_kinds! {
    fn holding_ctrl_alt_with_right_and_down_shows_the_popup_with_the_new_workspace_selected(
        kind: Kind
    ) {
        let (mut f, id) = two_rows(kind);
        f.map(id, WindowSpec::plain((400, 300)));
        let output = f.state.caminito.outputs[0].clone();
        assert!(!is_open(&f));

        f.key(KEY_LEFTCTRL, true);
        f.key(KEY_LEFTALT, true);
        tap(&mut f, KEY_RIGHT);

        assert_eq!(active(&f), 1);
        assert_eq!(popup(&f)["output"], output.name());
        assert_eq!(popup(&f)["mode"], "switch");
        assert_eq!(popup(&f)["columns"], 2);
        assert_eq!(popup(&f)["rows"], 2);
        assert_eq!(selected(&f), 1);
        assert!(has_popup_layer(&mut f, &output));

        tap(&mut f, KEY_DOWN);

        assert_eq!(active(&f), 3);
        assert_eq!(selected(&f), 3);

        f.key(KEY_LEFTALT, false);

        assert!(!is_open(&f));
        assert!(!has_popup_layer(&mut f, &output));
        assert_eq!(active(&f), 3);
        f.key(KEY_LEFTCTRL, false);
    }
}

/// Holds Ctrl+Alt and taps `key`, one of the arrows.
fn hold_and_tap(f: &mut Fixture, key: u32) {
    f.key(KEY_LEFTCTRL, true);
    f.key(KEY_LEFTALT, true);
    tap(f, key);
}

fn release(f: &mut Fixture) {
    f.key(KEY_LEFTALT, false);
    f.key(KEY_LEFTCTRL, false);
}

// The tests from here to the drawing map no window, or one only to have a client, so they
// run on one kind: the popup is the same whatever the windows are.

#[test]
fn the_popup_is_in_the_middle_of_the_pointers_output_and_stays_there() {
    let (mut f, _) = two_rows(Kind::Xdg);
    let first = f.state.caminito.outputs[0].clone();
    let second = f.add_output((1280, 720), 1.0);
    let on_second = output_geometry(&second);
    f.pointer_move(
        f64::from(on_second.loc.x) + 100.,
        f64::from(on_second.loc.y) + 100.,
    );

    hold_and_tap(&mut f, KEY_RIGHT);

    assert_eq!(popup(&f)["output"], second.name());
    assert!(has_popup_layer(&mut f, &second));
    assert!(!has_popup_layer(&mut f, &first));
    let rect = rect(&popup(&f)["popup"]);
    assert_eq!(rect.loc.x, (1280 - rect.size.w) / 2);
    assert_eq!(rect.loc.y, (720 - rect.size.h) / 2);

    // It stays where it appeared, through a further press too.
    f.pointer_move(100., 100.);
    tap(&mut f, KEY_DOWN);
    assert_eq!(popup(&f)["output"], second.name());
    assert_eq!(selected(&f), 3);
}

#[test]
fn unplugging_the_popups_output_moves_the_popup() {
    let (mut f, _) = two_rows(Kind::Xdg);
    let first = f.state.caminito.outputs[0].clone();
    let second = f.add_output((1280, 720), 1.0);
    let on_second = output_geometry(&second);
    f.pointer_move(
        f64::from(on_second.loc.x) + 100.,
        f64::from(on_second.loc.y) + 100.,
    );
    hold_and_tap(&mut f, KEY_RIGHT);
    assert_eq!(popup(&f)["output"], second.name());

    f.unplug(&second);

    assert_eq!(popup(&f)["output"], first.name());
    assert!(has_popup_layer(&mut f, &first));
    let rect = rect(&popup(&f)["popup"]);
    assert_eq!(rect.loc.x, (1920 - rect.size.w) / 2);
}

#[test]
fn releasing_either_modifier_closes_it() {
    let (mut f, _) = two_rows(Kind::Xdg);

    hold_and_tap(&mut f, KEY_RIGHT);
    assert!(is_open(&f));
    f.key(KEY_LEFTCTRL, false);
    assert!(!is_open(&f));
    f.key(KEY_LEFTALT, false);

    // And it shows again at the next switch.
    hold_and_tap(&mut f, KEY_LEFT);
    assert_eq!(selected(&f), 0);
    release(&mut f);
    assert!(!is_open(&f));
}

#[test]
fn a_quick_chord_shows_it_only_until_the_release() {
    let (mut f, _) = two_rows(Kind::Xdg);
    let output = f.state.caminito.outputs[0].clone();

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);

    assert_eq!(active(&f), 1);
    assert!(!is_open(&f));
    assert!(!has_popup_layer(&mut f, &output));
}

#[test]
fn it_shows_at_an_edge_where_the_switch_goes_nowhere() {
    let (mut f, _) = two_rows(Kind::Xdg);

    hold_and_tap(&mut f, KEY_LEFT);

    assert_eq!(active(&f), 0);
    assert_eq!(selected(&f), 0);
    release(&mut f);
}

#[test]
fn it_follows_a_wrap() {
    let mut f = Fixture::with_settings(vec![
        (CAMINITO, "workspace-layout-rows".into(), Value::Int(2)),
        (
            MARCO_GENERAL,
            "wrap-style".into(),
            Value::Str("toroidal".into()),
        ),
    ]);
    f.add_output((1920, 1080), 1.0);

    hold_and_tap(&mut f, KEY_LEFT);
    assert_eq!(selected(&f), 1);
    tap(&mut f, KEY_UP);
    assert_eq!(selected(&f), 3);
    release(&mut f);
}

#[test]
fn nothing_shows_with_one_workspace() {
    let (mut f, _) = two_rows_of(Kind::Xdg, 1);

    hold_and_tap(&mut f, KEY_RIGHT);

    assert!(!is_open(&f));
    release(&mut f);
}

#[test]
fn a_switch_by_number_or_to_the_previous_workspace_shows_nothing() {
    let mut f = Fixture::with_settings(vec![
        (CAMINITO, "workspace-layout-rows".into(), Value::Int(2)),
        (
            MARCO_GLOBAL_KEYBINDINGS,
            "switch-to-workspace-prev".into(),
            Value::Str("<Control><Alt>a".into()),
        ),
        (
            MARCO_GLOBAL_KEYBINDINGS,
            "switch-to-workspace-4".into(),
            Value::Str("<Control><Alt>Tab".into()),
        ),
    ]);
    f.add_output((1920, 1080), 1.0);

    f.key(KEY_LEFTCTRL, true);
    f.key(KEY_LEFTALT, true);
    tap(&mut f, KEY_TAB);
    assert_eq!(active(&f), 3);
    assert!(!is_open(&f));
    tap(&mut f, KEY_A);
    assert_eq!(active(&f), 0);
    assert!(!is_open(&f));
    release(&mut f);
}

#[test]
fn any_other_key_closes_it_and_does_nothing_else() {
    let mut f = Fixture::with_settings(vec![
        (CAMINITO, "workspace-layout-rows".into(), Value::Int(2)),
        (
            MARCO_GLOBAL_KEYBINDINGS,
            "switch-to-workspace-4".into(),
            Value::Str("<Control><Alt>a".into()),
        ),
    ]);
    f.add_output((1920, 1080), 1.0);

    hold_and_tap(&mut f, KEY_RIGHT);
    assert!(is_open(&f));
    tap(&mut f, KEY_A);

    // The key closed the popup, as it ends Marco's grab, and its own binding didn't run.
    assert!(!is_open(&f));
    assert_eq!(active(&f), 1);
    // With the popup closed the binding runs again.
    tap(&mut f, KEY_A);
    assert_eq!(active(&f), 3);
    release(&mut f);
}

#[test]
fn shift_pressed_while_it_shows_keeps_it_a_switch() {
    let (mut f, id) = two_rows(Kind::Xdg);
    let window = f.map(id, WindowSpec::plain((400, 300)));
    let window = f.pid(window);

    hold_and_tap(&mut f, KEY_RIGHT);
    f.key(KEY_LEFTSHIFT, true);
    tap(&mut f, KEY_DOWN);

    // Ctrl+Shift+Alt+Down is the move's, and a switch opened the popup: it switches.
    assert_eq!(popup(&f)["mode"], "switch");
    assert_eq!(selected(&f), 3);
    assert_eq!(f.state.caminito.policy.workspace_of(window), Some(0));
    f.key(KEY_LEFTSHIFT, false);
    release(&mut f);
}

on_both_kinds! {
    fn a_move_shows_it_and_each_direction_moves_the_window_on(kind: Kind) {
        let (mut f, id) = two_rows(kind);
        let window = f.map(id, WindowSpec::plain((400, 300)));
        let window = f.pid(window);

        f.key(KEY_LEFTCTRL, true);
        f.key(KEY_LEFTSHIFT, true);
        f.key(KEY_LEFTALT, true);
        tap(&mut f, KEY_RIGHT);

        assert_eq!(popup(&f)["mode"], "move");
        assert_eq!(selected(&f), 1);
        assert_eq!(f.state.caminito.policy.workspace_of(window), Some(1));

        tap(&mut f, KEY_DOWN);

        assert_eq!(selected(&f), 3);
        let policy = &f.state.caminito.policy;
        assert_eq!(policy.workspace_of(window), Some(3));
        assert_eq!(policy.focus(SEAT), Some(window));

        // Shift is one of the move's modifiers: letting go of it closes the popup.
        f.key(KEY_LEFTSHIFT, false);
        assert!(!is_open(&f));
        release(&mut f);
    }
}

#[test]
fn a_move_with_no_window_to_move_shows_nothing() {
    let (mut f, _) = two_rows(Kind::Xdg);

    f.key(KEY_LEFTCTRL, true);
    f.key(KEY_LEFTSHIFT, true);
    f.key(KEY_LEFTALT, true);
    tap(&mut f, KEY_RIGHT);

    assert_eq!(active(&f), 0);
    assert!(!is_open(&f));
    f.key(KEY_LEFTSHIFT, false);
    release(&mut f);
}

#[test]
fn a_press_of_a_pointer_button_closes_it_and_reaches_no_client() {
    let (mut f, id) = two_rows(Kind::Xdg);
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    let over = f.geometry(window).loc + Point::from((20, 20));
    f.pointer_move(f64::from(over.x), f64::from(over.y));
    f.roundtrip(id);
    f.client(id).state.input.clear();

    // At the grid's edge the switch goes nowhere, so the window stays under the pointer.
    hold_and_tap(&mut f, KEY_LEFT);
    assert!(is_open(&f));
    f.pointer_button(BTN_LEFT, true);
    assert!(!is_open(&f));
    f.pointer_button(BTN_LEFT, false);
    release(&mut f);
    f.roundtrip(id);

    // The window saw the modifiers, and neither the arrow, the press nor its release.
    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    key 29 pressed
    key 56 pressed
    key 56 released
    key 29 released
    ");
}

/// Locks the session as a new client's ext-session-lock. Returns the locker and its lock.
fn lock_the_session(f: &mut Fixture) -> (ClientId, LockId) {
    let locker = f.connect(Kind::Xdg);
    let lock = f.client(locker).lock_session();
    f.roundtrip(locker);
    assert_eq!(f.lock_state(), "locked");
    (locker, lock)
}

#[test]
fn nothing_shows_while_the_session_is_locked() {
    let (mut f, _) = two_rows(Kind::Xdg);
    let (locker, lock) = lock_the_session(&mut f);

    hold_and_tap(&mut f, KEY_RIGHT);

    assert!(!is_open(&f));
    assert_eq!(active(&f), 0);
    release(&mut f);

    // And it shows again once it's unlocked.
    f.client(locker).unlock_and_destroy(lock);
    f.roundtrip(locker);
    hold_and_tap(&mut f, KEY_RIGHT);
    assert_eq!(selected(&f), 1);
    release(&mut f);
}

#[test]
fn an_open_popup_closes_as_the_session_locks() {
    let (mut f, _) = two_rows(Kind::Xdg);
    let output = f.state.caminito.outputs[0].clone();
    hold_and_tap(&mut f, KEY_RIGHT);
    assert!(is_open(&f));

    let (locker, lock) = lock_the_session(&mut f);

    assert!(!is_open(&f));
    release(&mut f);
    f.client(locker).unlock_and_destroy(lock);
    f.roundtrip(locker);
    assert!(!is_open(&f));
    assert!(!has_popup_layer(&mut f, &output));
}

on_both_kinds! {
    fn nothing_shows_with_the_window_switcher_open(kind: Kind) {
        let (mut f, id) = two_rows(kind);
        f.map(id, WindowSpec::plain((400, 300)));
        f.map(id, WindowSpec::plain((400, 300)));
        let switcher_is_open = |f: &Fixture| {
            let dump: serde_json::Value =
                serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
            !dump["switcher"].is_null()
        };

        f.key(KEY_LEFTALT, true);
        tap(&mut f, KEY_TAB);
        assert!(switcher_is_open(&f));
        f.key(KEY_LEFTCTRL, true);
        tap(&mut f, KEY_RIGHT);

        // The arrow is the switcher's, which has the keyboard.
        assert!(switcher_is_open(&f));
        assert!(!is_open(&f));
        assert_eq!(active(&f), 0);
        release(&mut f);
        assert!(!is_open(&f));
    }
}

on_both_kinds! {
    fn nothing_shows_during_a_move(kind: Kind) {
        let (mut f, id) = two_rows(kind);
        let window = f.map(id, WindowSpec::plain((400, 300)));
        let start = f.geometry(window);
        let point = (start.loc + Point::from((50, 10))).to_f64();
        f.pointer_move(point.x, point.y);
        f.key(KEY_LEFTALT, true);
        f.pointer_button(BTN_LEFT, true);
        f.pointer_move(point.x + 100., point.y + 50.);

        f.key(KEY_LEFTCTRL, true);
        tap(&mut f, KEY_RIGHT);

        // The binding switches, as it did before the popup was there (150).
        assert!(!is_open(&f));
        assert_eq!(active(&f), 1);

        f.pointer_button(BTN_LEFT, false);
        release(&mut f);
        assert!(!is_open(&f));
    }
}

on_both_kinds! {
    fn nothing_shows_during_a_keyboard_move(kind: Kind) {
        let (mut f, id) = two_rows(kind);
        f.map(id, WindowSpec::plain((400, 300)));
        f.chord(&[KEY_LEFTALT, KEY_F7]);
        assert!(f.state.caminito.policy.keyboard_grab(SEAT).is_some());

        hold_and_tap(&mut f, KEY_RIGHT);

        // The arrow is the move's, which has the keyboard (156).
        assert!(!is_open(&f));
        assert_eq!(active(&f), 0);
        release(&mut f);
    }
}

// The drawing.

/// The popup of four workspaces in two rows, shown with workspace 1 selected.
fn shown(kind: Kind) -> (Fixture, ClientId) {
    let (mut f, id) = two_rows(kind);
    hold_and_tap(&mut f, KEY_RIGHT);
    f.dispatch();
    (f, id)
}

/// A point in a cell's padding, where the cell's fill shows.
fn in_padding(cell: Rectangle<i32, Logical>) -> Point<i32, Logical> {
    cell.loc + Point::from((2, 2))
}

/// The popup's background, from just inside its border.
fn background(f: &mut Fixture) -> u32 {
    let popup = rect(&popup(f)["popup"]);
    pixel(f, popup.loc + Point::from((3, 3)))
}

#[test]
fn the_cells_of_a_two_by_two_grid_are_in_its_rows_and_columns() {
    let (mut f, _) = shown(Kind::Xdg);

    assert_eq!(popup(&f)["cells"].as_array().unwrap().len(), 4);
    let cells: Vec<_> = (0..4).map(|workspace| cell(&f, workspace)).collect();
    let size = cells[0].size;
    assert!(cells.iter().all(|cell| cell.size == size));
    assert_eq!(cells[1].loc.y, cells[0].loc.y);
    assert!(cells[1].loc.x >= cells[0].loc.x + size.w);
    assert_eq!(cells[2].loc.x, cells[0].loc.x);
    assert!(cells[2].loc.y >= cells[0].loc.y + size.h);
    assert_eq!(cells[3].loc, Point::from((cells[1].loc.x, cells[2].loc.y)));
    let popup = rect(&popup(&f)["popup"]);
    for (workspace, cell) in cells.iter().enumerate() {
        assert!(popup.contains_rect(*cell));
        // A preview has the output's shape, and it and the name are in the cell.
        let preview = part(&f, "previews", workspace);
        assert_eq!(preview.size.w * 1080 / 1920, preview.size.h);
        assert!(cell.contains_rect(preview));
        let name = part(&f, "names", workspace);
        assert!(cell.contains_rect(name));
        assert!(name.loc.y >= preview.loc.y + preview.size.h);
    }

    // The selected cell alone is filled, and every preview is set off from the popup.
    let background = background(&mut f);
    for (workspace, cell) in cells.iter().enumerate() {
        let fill = pixel(&mut f, in_padding(*cell));
        assert_eq!(fill != background, workspace == 1, "workspace {workspace}");
        let preview = middle(part(&f, "previews", workspace));
        assert_ne!(pixel(&mut f, preview), fill, "workspace {workspace}");
    }
}

#[test]
fn the_selection_follows_each_press() {
    let (mut f, _) = shown(Kind::Xdg);
    let background = background(&mut f);
    let (one, three) = (in_padding(cell(&f, 1)), in_padding(cell(&f, 3)));
    assert_ne!(pixel(&mut f, one), background);

    tap(&mut f, KEY_DOWN);
    f.dispatch();

    assert_eq!(pixel(&mut f, one), background);
    assert_ne!(pixel(&mut f, three), background);
}

#[test]
fn a_part_filled_grid_has_no_cell_where_it_has_no_workspace() {
    let (mut f, _) = two_rows_of(Kind::Xdg, 3);
    hold_and_tap(&mut f, KEY_RIGHT);
    f.dispatch();

    // 0 1
    // 2
    assert_eq!(popup(&f)["columns"], 2);
    assert_eq!(popup(&f)["rows"], 2);
    assert_eq!(popup(&f)["cells"].as_array().unwrap().len(), 3);
    let (one, two) = (cell(&f, 1), cell(&f, 2));
    // The popup has room for the fourth cell, and nothing is drawn there.
    let empty = Rectangle::new((one.loc.x, two.loc.y).into(), one.size);
    assert!(rect(&popup(&f)["popup"]).contains_rect(empty));
    let background = background(&mut f);
    for y in (empty.loc.y..empty.loc.y + empty.size.h).step_by(3) {
        for x in (empty.loc.x..empty.loc.x + empty.size.w).step_by(3) {
            assert_eq!(
                pixel(&mut f, Point::from((x, y))),
                background,
                "at {x}, {y}"
            );
        }
    }
    // Down from the second workspace goes nowhere, and the popup stays on it.
    tap(&mut f, KEY_DOWN);
    assert_eq!(selected(&f), 1);
}

/// How many of a band's pixels aren't `fill`, the cell's own colour.
fn ink(f: &mut Fixture, band: Rectangle<i32, Logical>, fill: u32) -> usize {
    let mut count = 0;
    for y in band.loc.y..band.loc.y + band.size.h {
        for x in band.loc.x..band.loc.x + band.size.w {
            count += usize::from(pixel(f, Point::from((x, y))) != fill);
        }
    }
    count
}

#[test]
fn a_workspaces_name_is_in_its_cell() {
    let mut f = Fixture::with_settings(vec![
        (CAMINITO, "workspace-layout-rows".into(), Value::Int(2)),
        // A blank name is a name: Marco's default stands in for an empty one only.
        (
            MARCO_WORKSPACE_NAMES,
            "name-3".into(),
            Value::Str(" ".into()),
        ),
        (
            MARCO_WORKSPACE_NAMES,
            "name-4".into(),
            Value::Str("Mail".into()),
        ),
    ]);
    f.add_output((1920, 1080), 1.0);
    assert_eq!(f.state.caminito.policy.workspace_names()[3], "Mail");
    hold_and_tap(&mut f, KEY_RIGHT);
    f.dispatch();

    let background = background(&mut f);
    let named = part(&f, "names", 3);
    let blank = part(&f, "names", 2);
    let default = part(&f, "names", 0);
    assert!(ink(&mut f, named, background) > 0, "Mail is drawn");
    assert!(ink(&mut f, default, background) > 0, "Workspace 1 is drawn");
    assert_eq!(ink(&mut f, blank, background), 0);
    // The selected cell's name is drawn on the selection.
    let selected = part(&f, "names", 1);
    let in_cell = in_padding(cell(&f, 1));
    let fill = pixel(&mut f, in_cell);
    assert!(ink(&mut f, selected, fill) > 0, "Workspace 2 is drawn");

    // A name follows its key while the popup shows.
    f.set_setting(MARCO_WORKSPACE_NAMES, "name-3", Value::Str("Music".into()));
    f.dispatch_until(|f| f.state.caminito.policy.workspace_names()[2] == "Music");
    f.dispatch();
    assert!(ink(&mut f, blank, background) > 0, "Music is drawn");
}

#[test]
fn a_very_long_name_stays_in_its_band() {
    let name = "A workspace with a name much longer than its cell is wide".to_string();
    let mut f = Fixture::with_settings(vec![
        (CAMINITO, "workspace-layout-rows".into(), Value::Int(2)),
        (MARCO_WORKSPACE_NAMES, "name-1".into(), Value::Str(name)),
    ]);
    f.add_output((1920, 1080), 1.0);
    hold_and_tap(&mut f, KEY_RIGHT);
    f.dispatch();

    let background = background(&mut f);
    let (cell, band) = (cell(&f, 0), part(&f, "names", 0));
    assert!(ink(&mut f, band, background) > 0);
    // Nothing of it beside its band, in the cell's padding or the gap to the next cell.
    let right = Rectangle::new(
        (band.loc.x + band.size.w, band.loc.y).into(),
        (
            cell.loc.x + cell.size.w - band.loc.x - band.size.w + 4,
            band.size.h,
        )
            .into(),
    );
    assert_eq!(ink(&mut f, right, background), 0);
}

on_both_kinds! {
    fn a_workspaces_windows_are_outlined_in_its_preview(kind: Kind) {
        let (mut f, id) = two_rows(kind);
        let window = f.map(id, WindowSpec::plain((800, 600)));
        let frame = f.visible_frame(window);
        hold_and_tap(&mut f, KEY_RIGHT);
        f.dispatch();

        // Where the middle of the window is in a preview: the output in small.
        let in_preview = |preview: Rectangle<i32, Logical>| {
            let at = middle(frame);
            preview.loc
                + Point::from((
                    at.x * preview.size.w / 1920,
                    at.y * preview.size.h / 1080,
                ))
        };
        let (first, third) = (part(&f, "previews", 0), part(&f, "previews", 2));
        let empty = pixel(&mut f, in_preview(third));
        assert_ne!(pixel(&mut f, in_preview(first)), empty);
        // Its outline is its frame's, in small: nothing of it past the frame's corner.
        let corner = frame.loc + frame.size;
        let past = first.loc
            + Point::from((
                corner.x * first.size.w / 1920 + 3,
                corner.y * first.size.h / 1080 + 3,
            ));
        assert!(first.contains(past));
        assert_eq!(pixel(&mut f, past), empty);

        // The window goes with a move, and so does its outline.
        release(&mut f);
        f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFT]);
        f.key(KEY_LEFTCTRL, true);
        f.key(KEY_LEFTSHIFT, true);
        f.key(KEY_LEFTALT, true);
        tap(&mut f, KEY_DOWN);
        f.dispatch();
        assert_eq!(selected(&f), 2);
        let fourth = part(&f, "previews", 3);
        let empty = pixel(&mut f, in_preview(fourth));
        assert_eq!(pixel(&mut f, in_preview(first)), empty);
        assert_ne!(pixel(&mut f, in_preview(third)), empty);
    }
}

/// On one kind: which windows a preview leaves out is policy's facts, the same for both.
#[test]
fn a_minimised_window_has_no_outline() {
    let (mut f, id) = two_rows(Kind::Xdg);
    let window = f.map(id, WindowSpec::plain((800, 600)));
    let at = middle(f.visible_frame(window));
    let policy_id = f.pid(window);
    f.state.caminito.policy.minimize(policy_id);
    f.dispatch();
    hold_and_tap(&mut f, KEY_RIGHT);
    f.dispatch();

    let spot = |preview: Rectangle<i32, Logical>| {
        preview.loc + Point::from((at.x * preview.size.w / 1920, at.y * preview.size.h / 1080))
    };
    let (first, third) = (part(&f, "previews", 0), part(&f, "previews", 2));
    assert_eq!(pixel(&mut f, spot(first)), pixel(&mut f, spot(third)));
}

#[test]
fn the_popup_of_thirty_six_workspaces_in_one_row_fits_its_output() {
    let mut f = Fixture::with_settings(vec![(
        MARCO_GENERAL,
        "num-workspaces".into(),
        Value::Int(36),
    )]);
    f.add_output((1280, 720), 1.0);

    hold_and_tap(&mut f, KEY_RIGHT);

    assert_eq!(popup(&f)["columns"], 36);
    let rect = rect(&popup(&f)["popup"]);
    assert!(Rectangle::from_size((1280, 720).into()).contains_rect(rect));
    assert!(rect.size.w <= 1280 * 9 / 10);
    let (first, second) = (cell(&f, 0), cell(&f, 1));
    assert!(second.loc.x >= first.loc.x + first.size.w);
    assert!(part(&f, "previews", 0).size.w >= 1);
}

#[test]
fn the_popup_is_right_on_a_scaled_output() {
    let mut f = Fixture::with_settings(vec![(
        CAMINITO,
        "workspace-layout-rows".into(),
        Value::Int(2),
    )]);
    let output = f.add_output((3840, 2160), 2.0);
    hold_and_tap(&mut f, KEY_RIGHT);
    f.dispatch();

    // The rectangles are logical, in the middle of a 1920 × 1080 output.
    let rect = rect(&popup(&f)["popup"]);
    assert_eq!(rect.loc.x, (1920 - rect.size.w) / 2);
    let background = pixel_on(&mut f, &output, rect.loc + Point::from((3, 3)));
    let (selected, other) = (in_padding(cell(&f, 1)), in_padding(cell(&f, 0)));
    assert_ne!(pixel_on(&mut f, &output, selected), background);
    assert_eq!(pixel_on(&mut f, &output, other), background);
}
