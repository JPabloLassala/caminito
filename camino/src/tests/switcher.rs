//! The Window switcher (108): the keys that open, step, commit and cancel it, the pointer
//! in it, and what's drawn.

use camino_policy::WindowId;
use camino_test_client::{Anchor, ConstraintAdjustment, Gravity, Positioner, format_input};
use insta::assert_snapshot;
use smithay::utils::Point;

use super::fixture::{
    BTN_LEFT, ClientId, ClientWindow, Fixture, KEY_A, KEY_DOWN, KEY_ENTER, KEY_ESC, KEY_GRAVE,
    KEY_LEFT, KEY_LEFTALT, KEY_LEFTCTRL, KEY_LEFTSHIFT, KEY_RIGHT, KEY_TAB, KEY_UP, Kind,
    WindowSpec, on_both_kinds,
};
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
