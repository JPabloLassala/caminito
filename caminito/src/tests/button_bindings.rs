//! Caminito's bindings on a mouse button (184, ADR 0028): a key of
//! `org.mate.Marco.global-keybindings` or `window-keybindings` that names a button runs its
//! action when the button is pressed, wherever the pointer is. Each runs with an xdg toplevel
//! and an XWayland window (45), but what a client sees of a press, which the xdg client
//! records.

use caminito_policy::WindowId;
use caminito_test_client::InputEvent;
use caminito_theme::ButtonFunction;
use smithay::utils::Point;
use x11rb::protocol::xproto::GrabStatus;

use super::fixture::{
    BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, ClientId, ClientWindow, Fixture, KEY_ESC, KEY_LEFTALT,
    KEY_LEFTMETA, KEY_LEFTSHIFT, KEY_SPACE, Kind, WindowSpec, on_both_kinds,
};
use crate::decoration::control::FrameControl;
use crate::settings::Value;
use crate::settings::translate::{
    MARCO_GENERAL, MARCO_GLOBAL_KEYBINDINGS, MARCO_WINDOW_KEYBINDINGS,
};
use crate::state::SEAT;

// The kernel's codes for X's buttons 8 and 9, the back and forward buttons, written out
// here so the tests don't take the mapping from the code they check.
const BTN_SIDE: u32 = 0x113;
const BTN_EXTRA: u32 = 0x114;

const GLOBAL: &str = MARCO_GLOBAL_KEYBINDINGS;
const WINDOW: &str = MARCO_WINDOW_KEYBINDINGS;

fn binding(schema: &'static str, key: &str, value: &str) -> (&'static str, String, Value) {
    (schema, key.into(), Value::Str(value.into()))
}

/// One 1920 × 1080 output and a 400 × 300 window from a client of `kind`, under a Caminito
/// with `settings` set from the start.
fn with_window(
    kind: Kind,
    settings: Vec<(&'static str, String, Value)>,
) -> (Fixture, ClientId, ClientWindow) {
    let mut f = Fixture::with_settings(settings);
    f.add_output((1920, 1080), 1.0);
    let client = f.connect(kind);
    if kind == Kind::Xdg {
        f.client(client).bind_input();
    }
    let window = f.map(client, WindowSpec::plain((400, 300)));
    (f, client, window)
}

/// Holds `keys`, presses and releases `button`, and lets go of the keys.
fn press(f: &mut Fixture, keys: &[u32], button: u32) {
    for key in keys {
        f.key(*key, true);
    }
    f.pointer_button(button, true);
    f.pointer_button(button, false);
    for key in keys.iter().rev() {
        f.key(*key, false);
    }
}

/// Moves the pointer `(dx, dy)` inside `window`.
fn point_inside(f: &mut Fixture, window: ClientWindow, dx: i32, dy: i32) {
    let point = (f.geometry(window).loc + Point::from((dx, dy))).to_f64();
    f.pointer_move(point.x, point.y);
}

fn is_maximized(f: &Fixture, window: ClientWindow) -> bool {
    f.state.caminito.policy.is_maximized(f.pid(window))
}

fn focus(f: &Fixture) -> Option<WindowId> {
    f.state.caminito.policy.focus(SEAT)
}

fn stacking(f: &Fixture) -> Vec<WindowId> {
    f.state.caminito.policy.stacking(0).to_vec()
}

/// The presses and releases the xdg client saw since it last forgot its input, as the
/// button and whether it went down.
fn buttons_seen(f: &mut Fixture, client: ClientId) -> Vec<(u32, bool)> {
    f.roundtrip(client);
    let input = std::mem::take(&mut f.client(client).state.input);
    input
        .into_iter()
        .filter_map(|event| match event {
            InputEvent::Button { button, pressed } => Some((button, pressed)),
            _ => None,
        })
        .collect()
}

on_both_kinds! {
    fn a_bound_button_runs_its_action_over_a_window_and_over_the_desktop(kind: Kind) {
        let settings = vec![binding(WINDOW, "toggle-maximized", "<Super>Button8")];
        let (mut f, _, window) = with_window(kind, settings);

        point_inside(&mut f, window, 200, 150);
        let at = (f.geometry(window).loc + Point::from((200, 150))).to_f64();
        let under = f.state.caminito.window_under(at);
        assert_eq!(under, Some(f.pid(window)));
        press(&mut f, &[KEY_LEFTMETA], BTN_SIDE);
        assert!(is_maximized(&f, window));

        let settings = vec![binding(WINDOW, "toggle-maximized", "<Super>Button8")];
        let (mut f, _, window) = with_window(kind, settings);
        f.pointer_move(1900., 1000.);
        assert_eq!(f.state.caminito.window_under((1900., 1000.).into()), None);
        press(&mut f, &[KEY_LEFTMETA], BTN_SIDE);
        assert!(is_maximized(&f, window));
    }
}

on_both_kinds! {
    fn a_bound_button_runs_its_action_over_a_frame_and_not_the_frames_own(kind: Kind) {
        // The middle button on a titlebar lowers the window by default.
        let mut f = Fixture::with_settings(vec![binding(WINDOW, "toggle-maximized", "Button2")]);
        f.add_output((1920, 1080), 1.0);
        let client = f.connect(kind);
        let below = f.map(client, WindowSpec::default());
        let window = f.map(client, WindowSpec::default());
        let before = stacking(&f);

        let close = f.button_rect(window, ButtonFunction::Close);
        let x = f.geometry(window).loc.x + 200;
        f.pointer_move(f64::from(x), f64::from(close.loc.y + close.size.h / 2));
        assert_eq!(f.control_under_pointer(), Some(FrameControl::Title));
        press(&mut f, &[], BTN_MIDDLE);
        assert!(is_maximized(&f, window));
        assert!(!is_maximized(&f, below));
        assert_eq!(stacking(&f), before);
    }
}

on_both_kinds! {
    fn a_button_runs_its_binding_with_exactly_its_modifiers_and_no_other_button_does(kind: Kind) {
        let settings = vec![binding(WINDOW, "toggle-maximized", "<Super>Button8")];
        let (mut f, _, window) = with_window(kind, settings);
        point_inside(&mut f, window, 200, 150);

        press(&mut f, &[], BTN_SIDE);
        press(&mut f, &[KEY_LEFTMETA], BTN_EXTRA);
        press(&mut f, &[KEY_LEFTMETA], BTN_MIDDLE);
        assert!(!is_maximized(&f, window));
    }
}

#[test]
fn the_client_under_the_pointer_gets_neither_the_press_nor_the_release_of_a_bound_button() {
    let settings = vec![binding(WINDOW, "toggle-maximized", "<Super>Button8")];
    let (mut f, client, window) = with_window(Kind::Xdg, settings);
    point_inside(&mut f, window, 200, 150);
    buttons_seen(&mut f, client);

    press(&mut f, &[KEY_LEFTMETA], BTN_SIDE);
    assert!(is_maximized(&f, window));
    assert_eq!(buttons_seen(&mut f, client), []);
}

#[test]
fn an_unbound_button_reaches_the_client_as_before() {
    let settings = vec![binding(WINDOW, "toggle-maximized", "<Super>Button8")];
    let (mut f, client, window) = with_window(Kind::Xdg, settings);
    point_inside(&mut f, window, 200, 150);
    buttons_seen(&mut f, client);

    // The bound button without its modifier, and another with it.
    press(&mut f, &[], BTN_SIDE);
    press(&mut f, &[KEY_LEFTMETA], BTN_EXTRA);
    press(&mut f, &[], BTN_LEFT);
    assert_eq!(
        buttons_seen(&mut f, client),
        [
            (BTN_SIDE, true),
            (BTN_SIDE, false),
            (BTN_EXTRA, true),
            (BTN_EXTRA, false),
            (BTN_LEFT, true),
            (BTN_LEFT, false),
        ]
    );
    assert!(!is_maximized(&f, window));
}

on_both_kinds! {
    fn the_action_applies_to_the_focused_window_and_focus_and_stacking_stay(kind: Kind) {
        let settings = vec![binding(WINDOW, "toggle-maximized", "<Super>Button8")];
        let (mut f, client, a) = with_window(kind, settings);
        let b = f.map(client, WindowSpec::plain((400, 300)));
        let (pa, pb) = (f.pid(a), f.pid(b));
        assert_eq!(focus(&f), Some(pb));
        let before = stacking(&f);

        // Over the other window, where a plain click would focus and raise it.
        let at = f.geometry(a).loc + Point::from((2, 2));
        f.pointer_move(f64::from(at.x), f64::from(at.y));
        assert_eq!(f.state.caminito.window_under(at.to_f64()), Some(pa));
        // The pointer's move is no click: the focus mode is `click`.
        assert_eq!(focus(&f), Some(pb));

        press(&mut f, &[KEY_LEFTMETA], BTN_SIDE);
        assert!(is_maximized(&f, b));
        assert!(!is_maximized(&f, a));
        assert_eq!(focus(&f), Some(pb));
        assert_eq!(stacking(&f), before);
    }
}

// What keeps a binding from running.

fn dump(f: &Fixture) -> serde_json::Value {
    serde_json::from_str(&f.state.caminito.state_dump()).unwrap()
}

fn window_menu_is_open(f: &Fixture) -> bool {
    !dump(f)["window_menu"].is_null()
}

on_both_kinds! {
    fn a_bound_button_runs_nothing_while_the_session_is_locked(kind: Kind) {
        let settings = vec![binding(WINDOW, "toggle-maximized", "<Super>Button8")];
        let (mut f, _, window) = with_window(kind, settings);
        point_inside(&mut f, window, 200, 150);

        // An xdg client's lock whatever kind the window is: Xwayland has no session lock.
        let locker = f.connect(Kind::Xdg);
        f.client(locker).lock_session();
        f.roundtrip(locker);
        assert_eq!(f.lock_state(), "locked");

        press(&mut f, &[KEY_LEFTMETA], BTN_SIDE);
        assert!(!is_maximized(&f, window));
    }
}

on_both_kinds! {
    fn a_bound_button_runs_nothing_during_a_move_or_a_resize(kind: Kind) {
        // Alt and the left button move the window, and Alt and the right one resize it.
        for drag in [BTN_LEFT, BTN_RIGHT] {
            let settings = vec![binding(WINDOW, "toggle-maximized", "<Alt>Button8")];
            let (mut f, _, window) = with_window(kind, settings);
            point_inside(&mut f, window, 380, 270);

            f.key(KEY_LEFTALT, true);
            f.pointer_button(drag, true);
            assert!(f.state.caminito.policy.grab(SEAT).is_some(), "button {drag:#x}");
            f.pointer_button(BTN_SIDE, true);
            f.pointer_button(BTN_SIDE, false);
            assert!(!is_maximized(&f, window), "button {drag:#x}");
            assert!(f.state.caminito.policy.grab(SEAT).is_some(), "button {drag:#x}");

            // And once it's over the binding runs.
            f.pointer_button(drag, false);
            f.pointer_button(BTN_SIDE, true);
            f.pointer_button(BTN_SIDE, false);
            f.key(KEY_LEFTALT, false);
            assert!(is_maximized(&f, window), "button {drag:#x}");
        }
    }
}

on_both_kinds! {
    fn with_the_window_menu_open_a_bound_buttons_press_is_the_menus(kind: Kind) {
        let settings = vec![binding(WINDOW, "toggle-maximized", "<Alt>Button8")];
        let (mut f, _, window) = with_window(kind, settings);
        f.pointer_move(1900., 1000.);

        // Alt+Space, Marco's default for `activate-window-menu`.
        f.key(KEY_LEFTALT, true);
        f.key(KEY_SPACE, true);
        f.key(KEY_SPACE, false);
        assert!(window_menu_is_open(&f));

        // A press outside the menu closes it, as any button's does.
        f.pointer_button(BTN_SIDE, true);
        f.pointer_button(BTN_SIDE, false);
        f.key(KEY_LEFTALT, false);
        assert!(!window_menu_is_open(&f));
        assert!(!is_maximized(&f, window));
    }
}

/// An X11 app's menu is an override-redirect window that grabs the keyboard through Xwayland,
/// and the pointer inside it, where Caminito can't see the grab.
#[test]
fn with_an_x11_clients_menu_open_a_bound_button_runs_nothing() {
    let settings = vec![binding(WINDOW, "toggle-maximized", "<Super>Button8")];
    let (mut f, x, window) = with_window(Kind::X11, settings);
    let at = f.geometry(window).loc + Point::from((150, 100));
    let corner = (i16::try_from(at.x).unwrap(), i16::try_from(at.y).unwrap());
    let menu = f.map_override_redirect(x, corner, (100, 100), 0xff0000);
    let status = f.x11_call(x, move |x| x.grab_keyboard(menu)).unwrap();
    assert_eq!(status, GrabStatus::SUCCESS);
    f.dispatch_until(|f| f.override_redirect_with_keyboard() == Some(menu));

    f.pointer_move(f64::from(at.x + 50), f64::from(at.y + 50));
    press(&mut f, &[KEY_LEFTMETA], BTN_SIDE);
    assert!(!is_maximized(&f, window));
}

// The popups.

on_both_kinds! {
    fn a_button_on_switch_windows_steps_the_window_switcher_and_supers_release_picks(kind: Kind) {
        let settings = vec![binding(GLOBAL, "switch-windows", "<Super>Button9")];
        let (mut f, client, a) = with_window(kind, settings);
        let b = f.map(client, WindowSpec::plain((400, 300)));
        let c = f.map(client, WindowSpec::plain((400, 300)));
        let (pa, pb, pc) = (f.pid(a), f.pid(b), f.pid(c));
        assert_eq!(focus(&f), Some(pc));
        let selected = |f: &Fixture| dump(f)["switcher"]["selected"].as_u64().map(WindowId);
        f.pointer_move(1900., 1000.);

        f.key(KEY_LEFTMETA, true);
        f.pointer_button(BTN_EXTRA, true);
        f.pointer_button(BTN_EXTRA, false);
        assert_eq!(selected(&f), Some(pb));
        f.pointer_button(BTN_EXTRA, true);
        f.pointer_button(BTN_EXTRA, false);
        assert_eq!(selected(&f), Some(pa));
        // Shift turns it round, as it does a key.
        f.key(KEY_LEFTSHIFT, true);
        f.pointer_button(BTN_EXTRA, true);
        f.pointer_button(BTN_EXTRA, false);
        f.key(KEY_LEFTSHIFT, false);
        assert_eq!(selected(&f), Some(pb));
        f.pointer_button(BTN_EXTRA, true);
        f.pointer_button(BTN_EXTRA, false);
        assert_eq!(selected(&f), Some(pa));
        assert_eq!(focus(&f), Some(pc));

        f.key(KEY_LEFTMETA, false);
        assert!(dump(&f)["switcher"].is_null());
        assert_eq!(focus(&f), Some(pa));
    }
}

#[test]
fn the_presses_that_step_the_window_switcher_reach_no_client() {
    let settings = vec![binding(GLOBAL, "switch-windows", "<Super>Button9")];
    let (mut f, client, a) = with_window(Kind::Xdg, settings);
    f.map(client, WindowSpec::plain((400, 300)));
    point_inside(&mut f, a, 2, 2);
    buttons_seen(&mut f, client);

    f.key(KEY_LEFTMETA, true);
    for _ in 0..2 {
        f.pointer_button(BTN_EXTRA, true);
        f.pointer_button(BTN_EXTRA, false);
    }
    f.key(KEY_LEFTMETA, false);
    assert_eq!(buttons_seen(&mut f, client), []);
}

/// No window is in it, so it runs once: a button on a switch to the workspace beside, with
/// Marco's four workspaces in a row.
#[test]
fn a_button_on_a_workspace_switch_shows_the_workspace_popup_and_further_presses_step_it() {
    let mut f = Fixture::with_settings(vec![
        binding(GLOBAL, "switch-to-workspace-right", "<Super>Button8"),
        (MARCO_GENERAL, "num-workspaces".into(), Value::Int(4)),
    ]);
    f.add_output((1920, 1080), 1.0);
    let active = |f: &Fixture| f.state.caminito.policy.active_workspace();
    let popup_is_open = |f: &Fixture| !dump(f)["workspace_popup"].is_null();

    f.key(KEY_LEFTMETA, true);
    f.pointer_button(BTN_SIDE, true);
    f.pointer_button(BTN_SIDE, false);
    assert_eq!(active(&f), 1);
    assert!(popup_is_open(&f));
    f.pointer_button(BTN_SIDE, true);
    f.pointer_button(BTN_SIDE, false);
    assert_eq!(active(&f), 2);
    assert!(popup_is_open(&f));

    // Any other button closes it, as before (152).
    f.pointer_button(BTN_LEFT, true);
    f.pointer_button(BTN_LEFT, false);
    assert!(!popup_is_open(&f));
    assert_eq!(active(&f), 2);

    // And letting go of Super does.
    f.pointer_button(BTN_SIDE, true);
    f.pointer_button(BTN_SIDE, false);
    assert_eq!(active(&f), 3);
    assert!(popup_is_open(&f));
    f.key(KEY_LEFTMETA, false);
    assert!(!popup_is_open(&f));
}

#[test]
fn a_button_with_no_modifier_on_a_workspace_switch_switches_and_shows_no_popup() {
    let mut f = Fixture::with_settings(vec![
        binding(GLOBAL, "switch-to-workspace-right", "Button8"),
        (MARCO_GENERAL, "num-workspaces".into(), Value::Int(4)),
    ]);
    f.add_output((1920, 1080), 1.0);

    press(&mut f, &[], BTN_SIDE);
    assert_eq!(f.state.caminito.policy.active_workspace(), 1);
    assert!(dump(&f)["workspace_popup"].is_null());
}

// Precedence.

on_both_kinds! {
    fn a_modifier_click_wins_over_a_window_and_the_binding_runs_over_the_desktop(kind: Kind) {
        let settings = vec![binding(WINDOW, "toggle-maximized", "<Alt>Button2")];
        let (mut f, _, window) = with_window(kind, settings);

        // Alt and the middle button open the window's menu by default (162).
        point_inside(&mut f, window, 200, 150);
        press(&mut f, &[KEY_LEFTALT], BTN_MIDDLE);
        assert!(window_menu_is_open(&f));
        assert!(!is_maximized(&f, window));
        f.key(KEY_ESC, true);
        f.key(KEY_ESC, false);
        assert!(!window_menu_is_open(&f));

        f.pointer_move(1900., 1000.);
        press(&mut f, &[KEY_LEFTALT], BTN_MIDDLE);
        assert!(!window_menu_is_open(&f));
        assert!(is_maximized(&f, window));
    }
}

/// A shortcuts inhibitor is xdg's.
#[test]
fn an_inhibitor_on_the_focused_surface_gets_a_bound_buttons_press_and_an_alt_click() {
    let settings = vec![binding(WINDOW, "toggle-maximized", "<Super>Button8")];
    let (mut f, client, window) = with_window(Kind::Xdg, settings);
    f.client(client).inhibit_shortcuts(window.xdg());
    f.roundtrip(client);
    point_inside(&mut f, window, 200, 150);
    buttons_seen(&mut f, client);

    press(&mut f, &[KEY_LEFTMETA], BTN_SIDE);
    assert!(!is_maximized(&f, window));
    assert_eq!(
        buttons_seen(&mut f, client),
        [(BTN_SIDE, true), (BTN_SIDE, false)]
    );

    // Alt and the left button would move the window (162).
    let start = f.geometry(window);
    f.key(KEY_LEFTALT, true);
    f.pointer_button(BTN_LEFT, true);
    f.pointer_move_by(100., 50.);
    f.pointer_button(BTN_LEFT, false);
    f.key(KEY_LEFTALT, false);
    assert_eq!(f.geometry(window), start);
    assert_eq!(
        buttons_seen(&mut f, client),
        [(BTN_LEFT, true), (BTN_LEFT, false)]
    );
}

#[test]
fn without_an_inhibitor_the_same_presses_are_caminitos() {
    let settings = vec![binding(WINDOW, "toggle-above", "<Super>Button8")];
    let (mut f, client, window) = with_window(Kind::Xdg, settings);
    point_inside(&mut f, window, 200, 150);
    buttons_seen(&mut f, client);

    press(&mut f, &[KEY_LEFTMETA], BTN_SIDE);
    let start = f.geometry(window);
    f.key(KEY_LEFTALT, true);
    f.pointer_button(BTN_LEFT, true);
    f.pointer_move_by(100., 50.);
    f.pointer_button(BTN_LEFT, false);
    f.key(KEY_LEFTALT, false);
    assert_eq!(f.geometry(window).loc, start.loc + Point::from((100, 50)));
    assert_eq!(buttons_seen(&mut f, client), []);
}

// The debug dump and the settings.

on_both_kinds! {
    fn a_key_changed_to_a_button_binds_it_at_once_and_the_dump_lists_it(kind: Kind) {
        let (mut f, _, window) = with_window(kind, Vec::new());
        point_inside(&mut f, window, 200, 150);
        press(&mut f, &[KEY_LEFTMETA], BTN_SIDE);
        assert!(!is_maximized(&f, window));

        let in_force = |f: &Fixture, combination: &str| {
            let rows = dump(f)["bindings"].as_array().unwrap().clone();
            rows.iter()
                .any(|row| row["key"] == "toggle-maximized" && row["combination"] == combination)
        };
        let value = Value::Str("<Mod4>button8".into());
        f.set_setting(WINDOW, "toggle-maximized", value);
        f.dispatch_until(|f| in_force(f, "<Super>Button8"));
        press(&mut f, &[KEY_LEFTMETA], BTN_SIDE);
        assert!(is_maximized(&f, window));

        // And `disabled` gives the button back.
        f.set_setting(WINDOW, "toggle-maximized", Value::Str("disabled".into()));
        f.dispatch_until(|f| !in_force(f, "<Super>Button8"));
        press(&mut f, &[KEY_LEFTMETA], BTN_SIDE);
        assert!(is_maximized(&f, window));
    }
}
