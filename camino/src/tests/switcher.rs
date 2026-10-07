//! The Window switcher (108): the keys that open, step, commit and cancel it, the pointer
//! in it, and what's drawn.

use camino_policy::WindowId;
use camino_test_client::{Anchor, ConstraintAdjustment, Gravity, Positioner, format_input};
use insta::assert_snapshot;
use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle};

use super::fixture::{
    BTN_LEFT, ClientId, ClientWindow, Fixture, KEY_A, KEY_DOWN, KEY_ENTER, KEY_ESC, KEY_GRAVE,
    KEY_LEFT, KEY_LEFTALT, KEY_LEFTCTRL, KEY_LEFTSHIFT, KEY_RIGHT, KEY_TAB, KEY_UP, Kind,
    WINDOW_COLOR, WindowSpec, on_both_kinds,
};
use crate::output::output_geometry;
use crate::scene::SceneLayer;
use crate::settings::Value;
use crate::settings::translate::MARCO_GENERAL;
use crate::state::SEAT;

/// The debug dump's `switcher` member: `null` while the Window switcher is closed.
fn switcher(f: &Fixture) -> serde_json::Value {
    let dump: serde_json::Value = serde_json::from_str(&f.state.camino.state_dump()).unwrap();
    dump["switcher"].clone()
}

fn is_open(f: &Fixture) -> bool {
    !switcher(f).is_null()
}

/// The windows the open Window switcher lists, in order.
fn listed(f: &Fixture) -> Vec<WindowId> {
    let ids = switcher(f)["windows"].as_array().unwrap().clone();
    ids.iter()
        .map(|id| WindowId(id.as_u64().unwrap()))
        .collect()
}

/// The window the open Window switcher has selected.
fn selected(f: &Fixture) -> WindowId {
    WindowId(switcher(f)["selected"].as_u64().unwrap())
}

fn focus(f: &Fixture) -> Option<WindowId> {
    f.state.camino.policy.focus(SEAT)
}

/// Presses and releases `key`.
fn tap(f: &mut Fixture, key: u32) {
    f.key(key, true);
    f.key(key, false);
}

/// Maps `count` plain windows of `client`. The last has the focus, and each was used more
/// recently than the one before it.
fn map_windows(f: &mut Fixture, client: ClientId, count: usize) -> Vec<WindowId> {
    (0..count)
        .map(|_| {
            let window = f.map(client, WindowSpec::plain((400, 300)));
            f.pid(window)
        })
        .collect()
}

/// One 1920 × 1080 output and a client of `kind`, with `key` of `org.mate.Marco.general`
/// set to `value` from the start.
fn connected_with(kind: Kind, key: &str, value: Value) -> (Fixture, ClientId) {
    let mut f = Fixture::with_settings(vec![(MARCO_GENERAL, key.into(), value)]);
    f.add_output((1920, 1080), 1.0);
    let id = f.connect(kind);
    (f, id)
}

on_both_kinds! {
    fn a_quick_alt_tab_switches_to_the_window_used_before(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let a = f.map(id, WindowSpec::plain((400, 300)));
        let b = f.map(id, WindowSpec::plain((400, 300)));
        assert_eq!(focus(&f), f.policy_id(b));

        f.chord(&[KEY_LEFTALT, KEY_TAB]);

        assert_eq!(focus(&f), f.policy_id(a));
        assert!(f.has_keyboard(a));
        assert!(!is_open(&f));

        f.chord(&[KEY_LEFTALT, KEY_TAB]);

        assert_eq!(focus(&f), f.policy_id(b));
    }
}

on_both_kinds! {
    fn holding_alt_keeps_the_switcher_open_and_tab_steps_it(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let windows = map_windows(&mut f, id, 3);
        let (a, b, c) = (windows[0], windows[1], windows[2]);

        f.key(KEY_LEFTALT, true);
        tap(&mut f, KEY_TAB);

        assert_eq!(
            switcher(&f),
            serde_json::json!({
                "list": "windows",
                "windows": [c.0, b.0, a.0],
                "selected": b.0,
                "visible": false,
            })
        );
        assert_eq!(focus(&f), Some(c));

        tap(&mut f, KEY_TAB);

        assert_eq!(selected(&f), a);
        assert_eq!(focus(&f), Some(c));

        // It wraps from the last entry to the first.
        tap(&mut f, KEY_TAB);
        assert_eq!(selected(&f), c);
        tap(&mut f, KEY_TAB);
        tap(&mut f, KEY_TAB);

        f.key(KEY_LEFTALT, false);

        assert!(!is_open(&f));
        assert_eq!(focus(&f), Some(a));
    }
}

on_both_kinds! {
    fn shift_tab_steps_backward(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let windows = map_windows(&mut f, id, 3);
        let (a, b, c) = (windows[0], windows[1], windows[2]);

        f.key(KEY_LEFTALT, true);
        tap(&mut f, KEY_TAB);
        assert_eq!(selected(&f), b);

        f.key(KEY_LEFTSHIFT, true);
        tap(&mut f, KEY_TAB);
        assert_eq!(selected(&f), c);
        tap(&mut f, KEY_TAB);
        assert_eq!(selected(&f), a);

        // Shift's release leaves it open: Alt alone holds it.
        f.key(KEY_LEFTSHIFT, false);
        assert_eq!(selected(&f), a);
        tap(&mut f, KEY_TAB);
        assert_eq!(selected(&f), c);

        f.key(KEY_LEFTALT, false);
        assert_eq!(focus(&f), Some(c));
    }
}

on_both_kinds! {
    fn alt_shift_tab_opens_the_switcher_on_its_last_entry(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let windows = map_windows(&mut f, id, 3);
        let a = windows[0];

        f.key(KEY_LEFTALT, true);
        f.key(KEY_LEFTSHIFT, true);
        tap(&mut f, KEY_TAB);

        assert_eq!(selected(&f), a);

        // Alt holds it open, not Shift.
        f.key(KEY_LEFTSHIFT, false);
        assert!(is_open(&f));
        f.key(KEY_LEFTALT, false);
        assert_eq!(focus(&f), Some(a));
    }
}

on_both_kinds! {
    fn the_arrow_keys_move_in_the_grid(kind: Kind) {
        let (mut f, id) = connected_with(kind, "alt-tab-max-columns", Value::Int(3));
        map_windows(&mut f, id, 7);

        // Three rows: entries 0 to 2, 3 to 5, and 6.
        f.key(KEY_LEFTALT, true);
        tap(&mut f, KEY_TAB);
        let windows = listed(&f);
        assert_eq!(selected(&f), windows[1]);

        tap(&mut f, KEY_DOWN);
        assert_eq!(selected(&f), windows[4]);
        tap(&mut f, KEY_RIGHT);
        assert_eq!(selected(&f), windows[5]);
        tap(&mut f, KEY_UP);
        assert_eq!(selected(&f), windows[2]);
        tap(&mut f, KEY_LEFT);
        assert_eq!(selected(&f), windows[1]);

        f.key(KEY_LEFTALT, false);
        assert_eq!(focus(&f), Some(windows[1]));
    }
}

on_both_kinds! {
    fn enter_commits(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let windows = map_windows(&mut f, id, 3);
        let b = windows[1];

        f.key(KEY_LEFTALT, true);
        tap(&mut f, KEY_TAB);
        f.key(KEY_ENTER, true);

        assert!(!is_open(&f));
        assert_eq!(focus(&f), Some(b));

        // Nothing is left to commit when Alt goes up.
        f.key(KEY_ENTER, false);
        f.key(KEY_LEFTALT, false);
        assert_eq!(focus(&f), Some(b));
    }
}

on_both_kinds! {
    fn escape_cancels_and_leaves_focus_and_stacking_as_they_were(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let windows = map_windows(&mut f, id, 3);
        let c = windows[2];
        let stacking = f.state.camino.policy.stacking(0).to_vec();

        f.key(KEY_LEFTALT, true);
        tap(&mut f, KEY_TAB);
        tap(&mut f, KEY_TAB);
        tap(&mut f, KEY_ESC);

        assert!(!is_open(&f));
        f.key(KEY_LEFTALT, false);
        assert_eq!(focus(&f), Some(c));
        assert_eq!(f.state.camino.policy.stacking(0), stacking);
    }
}

on_both_kinds! {
    fn with_alt_tab_raise_windows_the_selected_window_is_raised_until_it_is_cancelled(kind: Kind) {
        let (mut f, id) = connected_with(kind, "alt-tab-raise-windows", Value::Bool(true));
        let windows = map_windows(&mut f, id, 3);
        let (a, b, c) = (windows[0], windows[1], windows[2]);
        assert_eq!(f.state.camino.policy.stacking(0), [a, b, c]);

        f.key(KEY_LEFTALT, true);
        tap(&mut f, KEY_TAB);
        assert_eq!(f.state.camino.policy.stacking(0), [a, c, b]);
        tap(&mut f, KEY_TAB);
        assert_eq!(f.state.camino.policy.stacking(0), [b, c, a]);

        tap(&mut f, KEY_ESC);
        f.key(KEY_LEFTALT, false);
        assert_eq!(f.state.camino.policy.stacking(0), [a, b, c]);
    }
}

on_both_kinds! {
    fn a_minimised_window_is_restored_on_commit(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let windows = map_windows(&mut f, id, 2);
        let (a, b) = (windows[0], windows[1]);
        f.state.camino.policy.minimize(a);
        f.dispatch();
        assert!(f.state.camino.policy.is_minimized(a));
        assert_eq!(focus(&f), Some(b));

        f.chord(&[KEY_LEFTALT, KEY_TAB]);

        assert!(!f.state.camino.policy.is_minimized(a));
        assert_eq!(focus(&f), Some(a));
    }
}

/// Maps a plain window of `client` with the app id `app_id`.
fn map_of_app(f: &mut Fixture, client: ClientId, app_id: &'static str) -> WindowId {
    let window = f.map(
        client,
        WindowSpec {
            app_id: Some(app_id),
            ..WindowSpec::plain((400, 300))
        },
    );
    f.pid(window)
}

on_both_kinds! {
    fn alt_grave_lists_the_focused_applications_windows(kind: Kind) {
        let (mut f, one) = Fixture::connected(kind);
        // Another application: a Wayland client of another process, or an X client with
        // another `WM_CLASS`, as the fixture's X clients set no `_NET_WM_PID`.
        let other = match kind {
            Kind::Xdg => f.connect_as_process(1),
            Kind::X11 => f.connect(kind),
        };
        let a = map_of_app(&mut f, one, "one");
        let between = map_of_app(&mut f, other, "other");
        let b = map_of_app(&mut f, one, "one");

        f.key(KEY_LEFTALT, true);
        tap(&mut f, KEY_GRAVE);

        assert_eq!(switcher(&f)["list"], "group");
        assert_eq!(listed(&f), [b, a]);
        assert_eq!(selected(&f), a);

        // The action's own key steps it, as Tab does.
        tap(&mut f, KEY_GRAVE);
        assert_eq!(selected(&f), b);
        tap(&mut f, KEY_TAB);
        assert_eq!(selected(&f), a);
        f.key(KEY_LEFTSHIFT, true);
        tap(&mut f, KEY_GRAVE);
        f.key(KEY_LEFTSHIFT, false);
        assert_eq!(selected(&f), b);
        tap(&mut f, KEY_GRAVE);

        f.key(KEY_LEFTALT, false);
        assert_eq!(focus(&f), Some(a));
        assert_ne!(focus(&f), Some(between));
    }
}

/// Only X11 has docks and the desktop as managed windows: a Wayland panel and desktop are
/// layer surfaces, which policy doesn't list.
#[test]
fn ctrl_alt_tab_lists_docks_and_the_desktop() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let typed = |f: &mut Fixture, window_type| {
        let window = f.map(
            x,
            WindowSpec {
                types: vec![window_type],
                ..WindowSpec::default()
            },
        );
        f.pid(window)
    };
    let dock = typed(&mut f, "_NET_WM_WINDOW_TYPE_DOCK");
    let desktop = typed(&mut f, "_NET_WM_WINDOW_TYPE_DESKTOP");
    let normal = map_windows(&mut f, x, 2);

    f.key(KEY_LEFTCTRL, true);
    f.key(KEY_LEFTALT, true);
    tap(&mut f, KEY_TAB);

    assert_eq!(switcher(&f)["list"], "panels");
    let mut windows = listed(&f);
    // The focused window isn't listed, so the selection starts on the first entry.
    assert_eq!(selected(&f), windows[0]);
    windows.sort();
    assert_eq!(windows, [dock, desktop]);
    assert!(!windows.contains(&normal[0]) && !windows.contains(&normal[1]));

    // Ctrl and Alt hold it open together: either's release commits.
    tap(&mut f, KEY_TAB);
    let chosen = selected(&f);
    f.key(KEY_LEFTCTRL, false);
    assert!(!is_open(&f));
    assert_eq!(focus(&f), Some(chosen));
    f.key(KEY_LEFTALT, false);
}

on_both_kinds! {
    fn nothing_opens_during_a_move(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let _below = f.map(id, WindowSpec::plain((400, 300)));
        let window = f.map(id, WindowSpec::plain((400, 300)));
        let start = f.geometry(window);
        let point = (start.loc + Point::from((50, 10))).to_f64();
        f.pointer_move(point.x, point.y);
        f.key(KEY_LEFTALT, true);
        f.pointer_button(BTN_LEFT, true);
        f.pointer_move(point.x + 100., point.y + 50.);

        f.key(KEY_TAB, true);
        assert!(!is_open(&f));
        f.key(KEY_TAB, false);

        // The move goes on.
        f.pointer_move(point.x + 200., point.y + 50.);
        f.pointer_button(BTN_LEFT, false);
        f.key(KEY_LEFTALT, false);
        f.sync(window);
        assert_eq!(focus(&f), f.policy_id(window));
        assert_eq!(f.geometry(window).loc, start.loc + Point::from((200, 50)));
    }
}

/// Maps two windows of a client that listens for input, and returns them with the second,
/// which has the keyboard, cleared of the input it has seen.
fn two_windows_with_input(f: &mut Fixture, id: ClientId) -> (ClientWindow, ClientWindow) {
    f.client(id).bind_input();
    let a = f.map(id, WindowSpec::plain((400, 300)));
    let b = f.map(id, WindowSpec::plain((400, 300)));
    f.roundtrip(id);
    f.client(id).state.input.clear();
    (a, b)
}

/// A popup with a grab is xdg's: an X client's menu is an override-redirect window, which
/// takes the keyboard through Xwayland.
#[test]
fn a_clients_open_menu_is_dismissed_first() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let (a, b) = two_windows_with_input(&mut f, id);
    let geometry = f.geometry(b);
    f.click(geometry.loc.x as f64 + 10., geometry.loc.y as f64 + 10.);
    f.roundtrip(id);
    let press = f.client(id).state.last_press_serial.unwrap();
    let menu = f.client(id).create_popup(
        b.xdg(),
        Positioner {
            size: (300, 200),
            anchor_rect: (10, 20, 30, 40),
            anchor: Anchor::BottomRight,
            gravity: Gravity::BottomRight,
            constraint_adjustment: ConstraintAdjustment::None,
        },
    );
    f.client(id).grab_popup(menu, press);
    f.roundtrip(id);
    f.client(id).map_popup(menu, 0xffff0000);
    f.roundtrip(id);
    assert!(!f.client(id).popup(menu).done);
    assert!(f.state.camino.seat.get_keyboard().unwrap().is_grabbed());

    f.key(KEY_LEFTALT, true);
    f.key(KEY_TAB, true);
    f.roundtrip(id);

    assert!(f.client(id).popup(menu).done);
    assert!(is_open(&f));
    assert!(!f.state.camino.seat.get_keyboard().unwrap().is_grabbed());
    assert!(!f.state.camino.seat.get_pointer().unwrap().is_grabbed());

    f.key(KEY_TAB, false);
    f.key(KEY_LEFTALT, false);
    assert!(f.has_keyboard(a));
}

/// It checks the keys a Wayland client sees, so it maps xdg toplevels only (45).
#[test]
fn keys_pressed_while_it_is_open_reach_no_client_and_alt_s_release_does() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    two_windows_with_input(&mut f, id);

    f.key(KEY_LEFTALT, true);
    tap(&mut f, KEY_TAB);
    tap(&mut f, KEY_A);
    tap(&mut f, KEY_DOWN);
    tap(&mut f, KEY_TAB);
    tap(&mut f, KEY_TAB);
    f.key(KEY_LEFTALT, false);
    f.roundtrip(id);

    // The window that had the keyboard saw Alt go down, so it sees it go up, and then
    // loses the keyboard to the window that was selected.
    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    key 56 pressed
    key 56 released
    keyboard leaves window 1
    keyboard enters window 0
    ");
}

/// It checks the keys a Wayland client sees, so it maps xdg toplevels only (45).
#[test]
fn a_modifier_pressed_while_it_is_open_reaches_the_client() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    two_windows_with_input(&mut f, id);

    f.key(KEY_LEFTALT, true);
    tap(&mut f, KEY_TAB);
    f.key(KEY_LEFTSHIFT, true);
    tap(&mut f, KEY_TAB);
    tap(&mut f, KEY_ESC);
    f.key(KEY_LEFTSHIFT, false);
    f.key(KEY_LEFTALT, false);
    f.roundtrip(id);

    // A client is told of the modifiers with the keys it gets. One that got no Shift
    // would keep the modifiers it was last told of, whatever is held.
    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    key 56 pressed
    key 42 pressed
    key 42 released
    key 56 released
    ");
}

on_both_kinds! {
    fn releasing_alt_while_another_key_is_down_commits(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let windows = map_windows(&mut f, id, 2);
        let a = windows[0];

        f.key(KEY_LEFTALT, true);
        f.key(KEY_TAB, true);
        assert!(is_open(&f));
        f.key(KEY_LEFTALT, false);

        assert!(!is_open(&f));
        assert_eq!(focus(&f), Some(a));
        f.key(KEY_TAB, false);
        assert_eq!(focus(&f), Some(a));
    }
}

on_both_kinds! {
    fn the_host_taking_the_keyboard_commits(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let windows = map_windows(&mut f, id, 2);
        let a = windows[0];
        f.key(KEY_LEFTALT, true);
        f.key(KEY_TAB, true);
        assert!(is_open(&f));

        // The host takes the keyboard while Alt and Tab are down: their releases never
        // come.
        f.state.release_pressed_keys();
        f.dispatch();

        assert!(!is_open(&f));
        assert_eq!(focus(&f), Some(a));
    }
}

/// It checks the keys a Wayland client sees, so it maps xdg toplevels only (45).
#[test]
fn an_inhibitor_does_not_trap_the_keyboard_in_an_open_switcher() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let (_, b) = two_windows_with_input(&mut f, id);
    f.client(id).inhibit_shortcuts(b.xdg());
    f.roundtrip(id);

    f.key(KEY_LEFTALT, true);
    f.key(KEY_TAB, true);
    // Bindings pass to the client under an active inhibitor, so the Window switcher
    // never opens to hold the keyboard.
    assert!(!is_open(&f));
    f.key(KEY_TAB, false);
    f.key(KEY_LEFTALT, false);
    f.roundtrip(id);

    assert_eq!(focus(&f), f.policy_id(b));
    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    key 56 pressed
    key 15 pressed
    key 15 released
    key 56 released
    ");
}

on_both_kinds! {
    fn a_binding_works_again_once_the_workspace_changed_under_the_switcher(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        map_windows(&mut f, id, 2);
        f.key(KEY_LEFTALT, true);
        tap(&mut f, KEY_TAB);
        assert!(is_open(&f));

        // As a pager does. Policy closes its switcher, and Camino's side follows.
        f.state.camino.policy.switch_workspace(1);
        f.dispatch();
        assert!(!is_open(&f));

        // An open switcher would take Left for its grid.
        f.key(KEY_LEFTCTRL, true);
        tap(&mut f, KEY_LEFT);
        assert_eq!(f.state.camino.policy.active_workspace(), 0);
        f.key(KEY_LEFTCTRL, false);
        f.key(KEY_LEFTALT, false);
        assert!(!is_open(&f));
    }
}

// The popup: when and where it shows, the pointer in it, and what's drawn.

/// Holds Alt, taps Tab, and dispatches until the popup has appeared and is drawn.
fn show(f: &mut Fixture) {
    f.key(KEY_LEFTALT, true);
    tap(f, KEY_TAB);
    f.dispatch_until(|f| switcher(f)["visible"] == true);
    f.dispatch();
}

fn rect(dump: &serde_json::Value) -> Rectangle<i32, Logical> {
    let field = |name: &str| dump[name].as_i64().unwrap() as i32;
    Rectangle::new(
        (field("x"), field("y")).into(),
        (field("width"), field("height")).into(),
    )
}

/// The cell of the popup's entry `index`, from its output's corner.
fn cell(f: &Fixture, index: usize) -> Rectangle<i32, Logical> {
    rect(&switcher(f)["cells"][index])
}

fn popup(f: &Fixture) -> Rectangle<i32, Logical> {
    rect(&switcher(f)["popup"])
}

fn middle(rect: Rectangle<i32, Logical>) -> Point<i32, Logical> {
    rect.loc + Point::from((rect.size.w / 2, rect.size.h / 2))
}

/// The first output's pixel at a logical point, as `0xRRGGBB`.
fn pixel(f: &mut Fixture, at: Point<i32, Logical>) -> u32 {
    let output = f.state.camino.outputs[0].clone();
    pixel_on(f, &output, at)
}

/// `output`'s pixel at a logical point from its corner, as `0xRRGGBB`.
fn pixel_on(f: &mut Fixture, output: &Output, at: Point<i32, Logical>) -> u32 {
    let scale = output.current_scale().fractional_scale();
    let at = at.to_physical_precise_round(scale);
    let pixel = f.state.backend.headless_mut().pixel(output, at);
    pixel.expect("the output is drawn") & 0xff_ffff
}

fn has_switcher_layer(f: &mut Fixture, output: &Output) -> bool {
    f.state.scene_layers(output).contains(&SceneLayer::Switcher)
}

/// The window's colour, as `pixel` reads it.
const WINDOW: u32 = WINDOW_COLOR & 0xff_ffff;

on_both_kinds! {
    fn a_quick_alt_tab_draws_no_popup(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        map_windows(&mut f, id, 2);
        let output = f.state.camino.outputs[0].clone();

        f.chord(&[KEY_LEFTALT, KEY_TAB]);

        assert!(!is_open(&f));
        assert!(!has_switcher_layer(&mut f, &output));
        // The popup's timer went with the switcher: nothing appears later. 150 ms is
        // well past the 90 ms the popup waits.
        f.dispatch_for(std::time::Duration::from_millis(150));
        assert!(!is_open(&f));
        assert!(!has_switcher_layer(&mut f, &output));
    }
}

on_both_kinds! {
    fn the_popup_is_not_drawn_before_its_delay(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        map_windows(&mut f, id, 2);
        let output = f.state.camino.outputs[0].clone();

        f.key(KEY_LEFTALT, true);
        tap(&mut f, KEY_TAB);

        assert_eq!(switcher(&f)["visible"], false);
        assert!(!has_switcher_layer(&mut f, &output));

        f.dispatch_until(|f| switcher(f)["visible"] == true);
        assert!(has_switcher_layer(&mut f, &output));
        f.key(KEY_LEFTALT, false);
        assert!(!has_switcher_layer(&mut f, &output));
    }
}

on_both_kinds! {
    fn holding_alt_draws_the_popup_on_the_pointers_output(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let first = f.state.camino.outputs[0].clone();
        let second = f.add_output((1280, 720), 1.0);
        map_windows(&mut f, id, 3);
        let on_second = output_geometry(&second);
        f.pointer_move(
            f64::from(on_second.loc.x) + 100.,
            f64::from(on_second.loc.y) + 100.,
        );

        show(&mut f);

        assert_eq!(switcher(&f)["output"], second.name());
        assert!(has_switcher_layer(&mut f, &second));
        assert!(!has_switcher_layer(&mut f, &first));
        // Centred on it.
        let popup = popup(&f);
        assert_eq!(popup.loc.x, (1280 - popup.size.w) / 2);
        assert_eq!(popup.loc.y, (720 - popup.size.h) / 2);

        // It stays where it appeared.
        f.pointer_move(100., 100.);
        assert_eq!(switcher(&f)["output"], second.name());
    }
}

on_both_kinds! {
    fn a_click_on_a_cell_commits_that_window(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let windows = map_windows(&mut f, id, 3);
        show(&mut f);
        assert_eq!(listed(&f), [windows[2], windows[1], windows[0]]);

        let at = middle(cell(&f, 2));
        f.click(f64::from(at.x), f64::from(at.y));

        assert!(!is_open(&f));
        assert_eq!(focus(&f), Some(windows[0]));
        // Alt's release has nothing left to commit.
        f.key(KEY_LEFTALT, false);
        assert_eq!(focus(&f), Some(windows[0]));
    }
}

on_both_kinds! {
    fn a_click_outside_the_cells_cancels(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let windows = map_windows(&mut f, id, 3);
        let stacking = f.state.camino.policy.stacking(0).to_vec();
        show(&mut f);

        // Inside the popup, in its padding.
        let at = popup(&f).loc + Point::from((2, 2));
        f.click(f64::from(at.x), f64::from(at.y));

        assert!(!is_open(&f));
        f.key(KEY_LEFTALT, false);
        assert_eq!(focus(&f), Some(windows[2]));
        assert_eq!(f.state.camino.policy.stacking(0), stacking);
    }
}

on_both_kinds! {
    fn hovering_a_cell_selects_nothing(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let windows = map_windows(&mut f, id, 3);
        show(&mut f);

        let at = middle(cell(&f, 2));
        f.pointer_move(f64::from(at.x), f64::from(at.y));

        assert_eq!(selected(&f), windows[1]);
    }
}

/// It checks the buttons a Wayland client sees, so it maps xdg toplevels only (45).
#[test]
fn no_click_reaches_a_client_while_it_is_open() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let (_, b) = two_windows_with_input(&mut f, id);
    let over_b = f.geometry(b).loc + Point::from((20, 20));
    f.pointer_move(f64::from(over_b.x), f64::from(over_b.y));
    f.roundtrip(id);
    f.client(id).state.input.clear();

    f.key(KEY_LEFTALT, true);
    tap(&mut f, KEY_TAB);
    f.pointer_button(BTN_LEFT, true);
    assert!(!is_open(&f));
    f.pointer_button(BTN_LEFT, false);
    f.key(KEY_LEFTALT, false);
    f.roundtrip(id);

    // The press cancelled the switcher: the window under it saw neither it nor its
    // release, and kept the keyboard.
    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    key 56 pressed
    key 56 released
    ");
}

on_both_kinds! {
    fn the_grid_wraps_at_alt_tab_max_columns(kind: Kind) {
        let (mut f, id) = connected_with(kind, "alt-tab-max-columns", Value::Int(3));
        map_windows(&mut f, id, 7);

        show(&mut f);

        assert_eq!(switcher(&f)["columns"], 3);
        assert_eq!(switcher(&f)["style"], "thumbnails");
        let cells: Vec<_> = (0..7).map(|index| cell(&f, index)).collect();
        // Rows of three: the fourth cell is under the first, and the seventh under both.
        assert_eq!(cells[1].loc.y, cells[0].loc.y);
        assert_eq!(cells[2].loc.y, cells[0].loc.y);
        assert!(cells[1].loc.x > cells[0].loc.x && cells[2].loc.x > cells[1].loc.x);
        assert_eq!(cells[3].loc.x, cells[0].loc.x);
        assert!(cells[3].loc.y >= cells[0].loc.y + cells[0].size.h);
        assert_eq!(cells[6].loc.x, cells[0].loc.x);
        assert!(cells[6].loc.y >= cells[3].loc.y + cells[3].size.h);
    }
}

/// One 1024 × 768 output and `count` windows of a client of `kind`, with the popup shown:
/// the size of its first cell.
fn first_cell_with(kind: Kind, count: usize) -> smithay::utils::Size<i32, Logical> {
    let mut f = Fixture::new();
    f.add_output((1024, 768), 1.0);
    let id = f.connect(kind);
    map_windows(&mut f, id, count);
    show(&mut f);
    let output = Rectangle::from_size((1024, 768).into());
    for index in 0..count {
        assert!(output.contains_rect(cell(&f, index)), "cell {index}");
    }
    cell(&f, 0).size
}

on_both_kinds! {
    fn thumbnails_shrink_with_many_windows(kind: Kind) {
        // Three thumbnails fit a 1024 pixel output at their full size; a row of five
        // doesn't, and two rows of five are smaller still.
        let few = first_cell_with(kind, 3);
        let row = first_cell_with(kind, 5);
        let rows = first_cell_with(kind, 10);

        assert_eq!((few.w, few.h), (272, 176));
        assert!(row.w < few.w && row.h < few.h, "{row:?}");
        assert!(rows.w <= row.w && rows.h <= row.h, "{rows:?}");
    }
}

on_both_kinds! {
    fn the_selected_cell_is_drawn_apart_from_the_others(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        map_windows(&mut f, id, 3);
        show(&mut f);
        // Just inside each cell's corner, in its padding.
        let corner = |f: &Fixture, index| cell(f, index).loc + Point::from((2, 2));
        let corners = [corner(&f, 0), corner(&f, 1), corner(&f, 2)];

        let colours = corners.map(|at| pixel(&mut f, at));
        assert_eq!(colours[0], colours[2]);
        assert_ne!(colours[1], colours[0]);

        tap(&mut f, KEY_TAB);
        let colours = corners.map(|at| pixel(&mut f, at));
        assert_eq!(colours[0], colours[1]);
        assert_ne!(colours[2], colours[0]);
    }
}

/// A 16 × 16 `_NET_WM_ICON` of one ARGB colour.
fn icon(argb: u32) -> Vec<u32> {
    let mut icon = vec![16, 16];
    icon.extend(std::iter::repeat_n(argb, 16 * 16));
    icon
}

/// Two X windows with icons of their own, red and blue, under a Camino with `key` of
/// `org.mate.Marco.general` set to `value`, and the popup shown. Only an X window brings
/// an icon of a colour the test knows (39).
fn icons_grid(key: &str, value: Value) -> Fixture {
    let (mut f, x) = connected_with(Kind::X11, key, value);
    for argb in [0xffff_0000, 0xff00_00ff] {
        f.map(
            x,
            WindowSpec {
                cardinals: vec![("_NET_WM_ICON", icon(argb))],
                ..WindowSpec::default()
            },
        );
    }
    show(&mut f);
    f
}

fn assert_icons_only(f: &mut Fixture) {
    assert_eq!(switcher(f)["style"], "icons");
    // Most recently used first: the blue one, then the red. Each icon fills the middle of
    // its cell, and nothing of the window is drawn.
    let middles = [middle(cell(f, 0)), middle(cell(f, 1))];
    assert_eq!(pixel(f, middles[0]), 0x0000ff);
    assert_eq!(pixel(f, middles[1]), 0xff0000);
    // `icon-size`'s default is 48: the cell is that and its padding.
    assert_eq!(cell(f, 0).size, (64, 64).into());
}

#[test]
fn with_compositing_off_the_grid_shows_icons() {
    let mut f = icons_grid("compositing-manager", Value::Bool(false));
    assert_icons_only(&mut f);
}

#[test]
fn with_fast_alt_tab_the_grid_shows_icons() {
    let mut f = icons_grid("compositing-fast-alt-tab", Value::Bool(true));
    assert_icons_only(&mut f);
}

#[test]
fn icon_size_sets_the_icons_of_the_icons_only_grid() {
    let mut f = Fixture::with_settings(vec![
        (
            MARCO_GENERAL,
            "compositing-manager".into(),
            Value::Bool(false),
        ),
        (MARCO_GENERAL, "icon-size".into(), Value::Int(96)),
    ]);
    f.add_output((1920, 1080), 1.0);
    let x = f.connect(Kind::X11);
    map_windows(&mut f, x, 2);
    show(&mut f);

    assert_eq!(cell(&f, 0).size, (112, 112).into());
}

/// A thumbnail has its window's icon in its bottom right corner. Only an X window brings
/// an icon of a colour the test knows (39).
#[test]
fn a_thumbnail_has_its_windows_icon_in_a_corner() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    for argb in [0xffff_0000, 0xff00_00ff] {
        f.map(
            x,
            WindowSpec {
                cardinals: vec![("_NET_WM_ICON", icon(argb))],
                ..WindowSpec::default()
            },
        );
    }
    show(&mut f);

    // The content box is the cell less its padding of 8, and the icon is a quarter of
    // its height: 40 pixels of 160.
    let cell = cell(&f, 1);
    let corner = cell.loc + Point::from((cell.size.w - 8 - 20, cell.size.h - 8 - 20));
    assert_eq!(pixel(&mut f, corner), 0xff0000);
}

/// Only an X window can ask for attention: wlr-foreign-toplevel has no state for it (65).
#[test]
fn an_urgent_window_is_marked() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let urgent = f.map(x, WindowSpec::default());
    let urgent_id = f.pid(urgent);
    map_windows(&mut f, x, 2);
    // Where the mark's middle is in a cell: 8 pixels from its top right corner.
    let mark = |f: &Fixture, index| {
        let cell = cell(f, index);
        cell.loc + Point::from((cell.size.w - 8, 8))
    };
    show(&mut f);
    assert_eq!(listed(&f)[2], urgent_id);
    let (plain, marked) = (mark(&f, 0), mark(&f, 2));
    assert_eq!(pixel(&mut f, marked), pixel(&mut f, plain));

    let window = urgent.x11();
    f.x11_call(x, move |x| {
        let attention = x.atom("_NET_WM_STATE_DEMANDS_ATTENTION")?;
        x.send_client_message(window, "_NET_WM_STATE", [1, attention, attention, 1, 0])
    })
    .unwrap();
    f.dispatch_until(|f| f.mapped(urgent).is_urgent());
    f.dispatch();

    // It's marked while the popup shows, and keeps its place.
    assert_eq!(listed(&f)[2], urgent_id);
    assert_ne!(pixel(&mut f, marked), pixel(&mut f, plain));
}

/// Every pixel of the first output, with the popup's own left out.
fn pixels_outside(f: &mut Fixture, popup: Rectangle<i32, Logical>) -> Vec<u32> {
    let output = f.state.camino.outputs[0].clone();
    let width = output_geometry(&output).size.w;
    let pixels = f.state.backend.headless_mut().pixels(&output).unwrap();
    let inside = |index: usize| {
        let at = Point::from((index as i32 % width, index as i32 / width));
        popup.contains(at)
    };
    let outside = pixels
        .iter()
        .enumerate()
        .filter(|(index, _)| !inside(*index));
    outside.map(|(_, pixel)| *pixel).collect()
}

/// Two windows, the one used before titled `title`, and the popup shown over them: the
/// title under the grid is that window's. Returns whether anything of the title is drawn.
fn titled(kind: Kind, title: &'static str) -> bool {
    let (mut f, id) = Fixture::connected(kind);
    f.map(
        id,
        WindowSpec {
            title: Some(title),
            ..WindowSpec::plain((400, 300))
        },
    );
    f.map(id, WindowSpec::plain((400, 300)));
    f.dispatch();
    let before = f
        .state
        .backend
        .headless_mut()
        .pixels(&f.state.camino.outputs[0].clone());

    show(&mut f);

    // Nothing is drawn outside the popup, however long the title is.
    let popup = popup(&f);
    let width = 1920;
    let before: Vec<u32> = before
        .unwrap()
        .iter()
        .enumerate()
        .filter(|(index, _)| {
            !popup.contains(Point::from((*index as i32 % width, *index as i32 / width)))
        })
        .map(|(_, pixel)| *pixel)
        .collect();
    assert!(
        before == pixels_outside(&mut f, popup),
        "the popup drew outside itself"
    );

    // The title band is the popup's last 32 pixels above its padding of 12.
    let band = Rectangle::new(
        popup.loc + Point::from((12, popup.size.h - 12 - 32)),
        (popup.size.w - 24, 32).into(),
    );
    let background = pixel(&mut f, popup.loc + Point::from((3, 3)));
    let mut drawn = false;
    for y in band.loc.y..band.loc.y + band.size.h {
        for x in (band.loc.x..band.loc.x + band.size.w).step_by(2) {
            drawn |= pixel(&mut f, Point::from((x, y))) != background;
        }
    }
    drawn
}

on_both_kinds! {
    fn the_selected_windows_title_is_under_the_grid(kind: Kind) {
        assert!(titled(kind, "A title"));
    }
}

on_both_kinds! {
    fn a_very_long_title_stays_inside_the_popup(kind: Kind) {
        assert!(titled(
            kind,
            "A title that goes on and on, far past the popup's width, as a browser's does \
             when the page's own title is a sentence or two, and then some more of it, and \
             more, and more, and more, and more, and more, and more, and more, until the end"
        ));
    }
}

on_both_kinds! {
    fn a_non_latin_title_draws(kind: Kind) {
        // Whether it has glyphs to draw depends on the fonts installed; that it's laid
        // out and stays inside the popup doesn't.
        titled(kind, "日本語のタイトル مرحبا بالعالم");
    }
}

on_both_kinds! {
    fn an_empty_title_draws_nothing_under_the_grid(kind: Kind) {
        assert!(!titled(kind, ""));
    }
}

on_both_kinds! {
    fn unplugging_the_popups_output_moves_the_popup(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let first = f.state.camino.outputs[0].clone();
        let second = f.add_output((1280, 720), 1.0);
        map_windows(&mut f, id, 3);
        let on_second = output_geometry(&second);
        f.pointer_move(
            f64::from(on_second.loc.x) + 100.,
            f64::from(on_second.loc.y) + 100.,
        );
        show(&mut f);
        assert_eq!(switcher(&f)["output"], second.name());

        f.unplug(&second);

        assert_eq!(switcher(&f)["output"], first.name());
        assert!(has_switcher_layer(&mut f, &first));
        let popup = popup(&f);
        assert_eq!(popup.loc.x, (1920 - popup.size.w) / 2);
    }
}

on_both_kinds! {
    fn unplugging_the_last_output_cancels_the_switcher(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let output = f.state.camino.outputs[0].clone();
        let windows = map_windows(&mut f, id, 3);
        show(&mut f);

        f.unplug(&output);

        assert!(!is_open(&f));
        f.key(KEY_LEFTALT, false);
        assert_eq!(focus(&f), Some(windows[2]));
    }
}

// Thumbnails.

on_both_kinds! {
    fn a_cell_shows_its_windows_content(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        map_windows(&mut f, id, 3);
        show(&mut f);

        for index in 0..3 {
            let at = middle(cell(&f, index));
            assert_eq!(pixel(&mut f, at), WINDOW, "cell {index}");
        }
    }
}

/// The test's Wayland client draws its window in another colour while the popup shows;
/// the fixture's X clients draw once (45).
#[test]
fn a_thumbnail_follows_its_window_live() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let a = f.map(id, WindowSpec::plain((400, 300)));
    f.map(id, WindowSpec::plain((400, 300)));
    show(&mut f);
    let at = middle(cell(&f, 1));
    assert_eq!(pixel(&mut f, at), WINDOW);

    f.client(id).attach_buffer(a.xdg(), (400, 300), 0xff00_00ff);
    f.client(id).commit(a.xdg());
    f.roundtrip(id);

    f.wait_for_pixel(at.x, at.y, 0x0000ff);
    assert!(is_open(&f));
}

on_both_kinds! {
    fn a_minimised_window_shows_its_last_content(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let windows = map_windows(&mut f, id, 3);
        f.state.camino.policy.minimize(windows[0]);
        f.dispatch();
        show(&mut f);
        assert_eq!(listed(&f)[2], windows[0]);

        let at = middle(cell(&f, 2));
        assert_eq!(pixel(&mut f, at), WINDOW);
    }
}

on_both_kinds! {
    fn a_framed_windows_thumbnail_has_its_frame(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let framed = f.map(id, WindowSpec::default());
        f.map(id, WindowSpec::default());
        show(&mut f);
        let frame = f.state.camino.policy.frame_rect(f.pid(framed)).unwrap();
        let geometry = f.geometry(framed);
        assert!(frame.size.h > geometry.size.h, "the window has a frame");
        let grid = &f.state.camino.visible_switcher().unwrap().grid;
        let thumbnail = grid.thumbnail(1, frame.size);

        // The thumbnail is the frame's shape, with the titlebar along its top and the
        // window under it.
        let titlebar = thumbnail.loc + Point::from((thumbnail.size.w / 2, 1));
        let background = cell(&f, 1).loc + Point::from((2, 2));
        let titlebar = pixel(&mut f, titlebar);
        assert_ne!(titlebar, WINDOW);
        assert_ne!(titlebar, pixel(&mut f, background));
        assert_eq!(pixel(&mut f, middle(thumbnail)), WINDOW);
    }
}

on_both_kinds! {
    fn nothing_of_a_thumbnail_is_drawn_outside_its_cell(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        // Much wider than high, so its thumbnail leaves the content box bare above and
        // below it.
        f.map(id, WindowSpec::plain((800, 100)));
        f.map(id, WindowSpec::plain((400, 300)));
        f.map(id, WindowSpec::plain((400, 300)));
        show(&mut f);
        tap(&mut f, KEY_TAB);
        assert_eq!(switcher(&f)["selected"], listed(&f)[2].0);

        let wide = cell(&f, 2);
        let middle_of = middle(wide);
        let bare = wide.loc + Point::from((wide.size.w / 2, 10));
        let padding = wide.loc + Point::from((2, 2));
        assert_eq!(pixel(&mut f, middle_of), WINDOW);
        assert_eq!(pixel(&mut f, bare), pixel(&mut f, padding));
    }
}

/// One output at `scale`, three windows of a client of `kind` and the popup shown: each
/// cell's middle is its window's.
fn thumbnails_at(kind: Kind, scale: f64) {
    let mut f = Fixture::new();
    f.add_output((1920, 1080), scale);
    let id = f.connect(kind);
    map_windows(&mut f, id, 3);
    show(&mut f);

    for index in 0..3 {
        let at = middle(cell(&f, index));
        assert_eq!(pixel(&mut f, at), WINDOW, "cell {index} at {scale}");
        // And the popup's own colour between two cells' content.
        let padding = cell(&f, index).loc + Point::from((2, 2));
        assert_ne!(pixel(&mut f, padding), WINDOW, "cell {index} at {scale}");
    }
}

on_both_kinds! {
    fn thumbnails_are_right_on_a_scaled_output(kind: Kind) {
        thumbnails_at(kind, 2.0);
        thumbnails_at(kind, 1.5);
    }
}

on_both_kinds! {
    fn the_icons_only_grid_draws_no_thumbnail(kind: Kind) {
        let (mut f, id) = connected_with(kind, "compositing-fast-alt-tab", Value::Bool(true));
        map_windows(&mut f, id, 3);
        show(&mut f);

        // The windows themselves are behind the popup; nothing in it is of their colour.
        let popup = popup(&f);
        for y in (popup.loc.y..popup.loc.y + popup.size.h).step_by(3) {
            for x in (popup.loc.x..popup.loc.x + popup.size.w).step_by(3) {
                assert_ne!(pixel(&mut f, Point::from((x, y))), WINDOW, "({x}, {y})");
            }
        }
    }
}

// What the popup must leave alone.

on_both_kinds! {
    fn the_pointer_reaches_nothing_under_the_popup(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        map_windows(&mut f, id, 3);
        show(&mut f);
        // A window fills the output's middle under the popup, or the output is bare there:
        // either way the popup has the pointer for no client.
        let under_popup = middle(popup(&f));
        let over_a_window = f.state.camino.policy.switcher().unwrap().windows[0];
        let window = f.state.camino.policy.geometry(over_a_window).unwrap();
        f.state
            .camino
            .policy
            .request_geometry(over_a_window, Some(under_popup - Point::from((50, 50))), None);
        f.dispatch();
        assert!(window.size.w >= 100 && window.size.h >= 100);

        f.pointer_move(f64::from(under_popup.x), f64::from(under_popup.y));
        assert_eq!(f.state.camino.pointer_focus, None);
        assert_eq!(f.state.camino.window_under(under_popup.to_f64()), None);

        // With the popup gone the pointer is the window's as ever.
        f.key(KEY_LEFTALT, false);
        f.pointer_move(f64::from(under_popup.x) + 1., f64::from(under_popup.y));
        assert!(f.state.camino.pointer_focus.is_some());
    }
}

on_both_kinds! {
    fn a_thumbnail_on_another_output_leaves_its_windows_main_output(kind: Kind) {
        let mut f = Fixture::new();
        let first = f.add_output((1920, 1080), 1.0);
        let second = f.add_output((2560, 1440), 2.0);
        let id = f.connect(kind);
        // Small, so that its thumbnail on the output at scale 2 has more than twice the
        // pixels the window itself has on the first.
        let small = f.map(id, WindowSpec::plain((200, 150)));
        f.map(id, WindowSpec::plain((200, 150)));
        assert_eq!(f.mapped(small).main_output(), Some(&first));
        let on_second = output_geometry(&second);
        f.pointer_move(
            f64::from(on_second.loc.x) + 100.,
            f64::from(on_second.loc.y) + 100.,
        );

        show(&mut f);
        assert_eq!(switcher(&f)["output"], second.name());
        // A few frames of both outputs, with the thumbnails on the second.
        for _ in 0..3 {
            f.draw(&first);
            f.draw(&second);
        }

        assert_eq!(f.mapped(small).main_output(), Some(&first));
    }
}
