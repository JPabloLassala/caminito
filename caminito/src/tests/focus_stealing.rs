//! Focus-stealing prevention: xdg-activation's tokens, and `focus-new-windows` judging
//! them, on xdg windows (165); XWayland's windows, which carry no token, with
//! `_NET_ACTIVE_WINDOW`'s source indications and `_NET_WM_STATE_DEMANDS_ATTENTION`; and the
//! urgency of either kind as a window list is told of it (166).
//!
//! The fixture's clients are all of the test's own process, so the xdg clients here are
//! connected as processes of their own, and the XWayland windows say theirs in `_NET_WM_PID`,
//! which is how policy tells one client's windows from another's.

use caminito_policy::FocusNewWindows;
use caminito_test_client::layer_shell::{Layer, LayerSpec};
use caminito_test_client::xdg_activation::TokenId;

use super::fixture::{ClientId, ClientWindow, Fixture, Kind, WINDOW_COLOR, WindowSpec};
use crate::settings::Value;
use crate::settings::translate::MARCO_GENERAL;
use crate::state::SEAT;

/// One 1920 × 1080 output, with `focus-new-windows` set to `mode` from the start.
fn fixture(mode: &str) -> Fixture {
    let mut f = Fixture::with_settings(vec![(
        MARCO_GENERAL,
        "focus-new-windows".into(),
        Value::Str(mode.into()),
    )]);
    f.add_output((1920, 1080), 1.0);
    f
}

/// A client of the process `pid` with the pointer and keyboard, as a toolkit has them.
fn client(f: &mut Fixture, pid: u32) -> ClientId {
    let id = f.connect_as_process(pid);
    f.client(id).bind_input();
    f.roundtrip(id);
    id
}

fn has_focus(f: &Fixture, window: ClientWindow) -> bool {
    f.state.caminito.policy.focus(SEAT) == Some(f.pid(window))
}

/// Whether the debug dump lists the window as urgent.
fn dumped_urgent(f: &Fixture, window: ClientWindow) -> bool {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    let id = f.pid(window).0;
    let entry = dump["windows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == id)
        .expect("the window is dumped")
        .clone();
    entry["urgent"].as_bool().unwrap()
}

/// Clicks the middle of a window.
fn click(f: &mut Fixture, window: ClientWindow) {
    let geometry = f.geometry(window);
    let x = geometry.loc.x + geometry.size.w / 2;
    let y = geometry.loc.y + geometry.size.h / 2;
    f.click(x.into(), y.into());
    f.sync(window);
}

/// Asks for a token from `from` with its client's last input serial, and returns it once
/// issued.
fn token_from(f: &mut Fixture, from: ClientWindow) -> String {
    let id = from.client();
    let serial = f.client_state(id).last_input_serial;
    assert!(serial.is_some(), "{from:?}'s client has had input");
    let token: TokenId = f
        .client(id)
        .request_activation_token(serial, Some(from.xdg()));
    f.roundtrip(id);
    f.client(id).activation_token(token).unwrap().to_owned()
}

fn activate(f: &mut Fixture, token: &str, window: ClientWindow) {
    f.client(window.client()).activate(token, window.xdg());
    f.roundtrip(window.client());
}

#[test]
fn a_token_from_the_focused_client_activates_its_second_window() {
    for mode in ["smart", "strict"] {
        let mut f = fixture(mode);
        let id = client(&mut f, 100);
        let first = f.map(id, WindowSpec::plain((400, 300)));
        let second = f.map(id, WindowSpec::plain((400, 300)));
        click(&mut f, first);
        assert!(has_focus(&f, first), "{mode}");

        let token = token_from(&mut f, first);
        activate(&mut f, &token, second);

        assert!(has_focus(&f, second), "{mode}");
        assert!(f.has_keyboard(second), "{mode}");
        assert!(!dumped_urgent(&f, second), "{mode}");
    }
}

/// A notification bubble of caminito-notifications's process, on the overlay layer, which
/// never takes the keyboard, and the middle of it.
fn bubble(f: &mut Fixture, pid: u32) -> (ClientId, (f64, f64)) {
    let id = client(f, pid);
    f.map_layer(
        id,
        LayerSpec {
            namespace: "caminito-notification",
            ..LayerSpec::floating(Layer::Overlay, (360, 100))
        },
        WINDOW_COLOR,
    );
    (id, (960.0, 540.0))
}

/// Asks for a token from the client `id` with its last input serial, which no window of its
/// is named with, and returns it once issued.
fn token_from_client(f: &mut Fixture, id: ClientId) -> String {
    let serial = f.client_state(id).last_input_serial;
    assert!(serial.is_some(), "the client has had input");
    let token: TokenId = f.client(id).request_activation_token(serial, None);
    f.roundtrip(id);
    f.client(id).activation_token(token).unwrap().to_owned()
}

/// A click on a notification bubble gives its app's window the focus (CMN-266), though the
/// bubble never had the keyboard.
#[test]
fn a_token_from_a_click_on_a_layer_surface_without_the_keyboard_activates_a_window() {
    for mode in ["smart", "strict"] {
        let mut f = fixture("smart");
        let app = client(&mut f, 100);
        let app_window = f.map(app, WindowSpec::plain((400, 300)));
        let other = client(&mut f, 200);
        let focused = f.map(other, WindowSpec::plain((400, 300)));
        assert!(has_focus(&f, focused));
        f.set_setting(MARCO_GENERAL, "focus-new-windows", Value::Str(mode.into()));
        let (daemon, middle) = bubble(&mut f, 300);

        f.click(middle.0, middle.1);
        f.roundtrip(daemon);
        let token = token_from_client(&mut f, daemon);
        activate(&mut f, &token, app_window);

        assert!(has_focus(&f, app_window), "{mode}");
        assert!(f.has_keyboard(app_window), "{mode}");
        assert!(!dumped_urgent(&f, app_window), "{mode}");
    }
}

#[test]
fn a_token_from_a_layer_surface_the_pointer_has_left_is_refused_under_strict() {
    let mut f = fixture("smart");
    let app = client(&mut f, 100);
    let app_window = f.map(app, WindowSpec::plain((400, 300)));
    let other = client(&mut f, 200);
    let focused = f.map(other, WindowSpec::plain((400, 300)));
    f.set_setting(
        MARCO_GENERAL,
        "focus-new-windows",
        Value::Str("strict".into()),
    );
    let (daemon, middle) = bubble(&mut f, 300);
    f.click(middle.0, middle.1);
    f.roundtrip(daemon);

    // Off the bubble, onto the focused window, before the token is asked for.
    f.pointer_move(5.0, 5.0);
    f.roundtrip(daemon);
    let token = token_from_client(&mut f, daemon);
    activate(&mut f, &token, app_window);

    assert!(has_focus(&f, focused));
    assert!(dumped_urgent(&f, app_window));
}

#[test]
fn a_token_from_a_window_that_is_not_focused_is_refused_under_strict() {
    let mut f = fixture("smart");
    let other = client(&mut f, 100);
    let unfocused = f.map(other, WindowSpec::plain((400, 300)));
    let id = client(&mut f, 200);
    let focused = f.map(id, WindowSpec::plain((400, 300)));
    assert!(has_focus(&f, focused));
    f.set_setting(
        MARCO_GENERAL,
        "focus-new-windows",
        Value::Str("strict".into()),
    );
    f.dispatch_until(|f| {
        f.state.caminito.settings.policy.focus_new_windows == FocusNewWindows::Strict
    });

    let token = token_from(&mut f, unfocused);
    activate(&mut f, &token, unfocused);

    assert!(has_focus(&f, focused));
    assert!(f.has_keyboard(focused));
    assert!(dumped_urgent(&f, unfocused));
    let stacking = f.state.caminito.policy.stacking(0);
    assert_eq!(stacking, [f.pid(unfocused), f.pid(focused)]);

    // Focusing it ends its urgency.
    click(&mut f, unfocused);
    assert!(has_focus(&f, unfocused));
    assert!(!dumped_urgent(&f, unfocused));
}

#[test]
fn a_first_window_with_no_token_is_focused_under_smart() {
    let mut f = fixture("smart");
    let other = client(&mut f, 100);
    f.map(other, WindowSpec::plain((400, 300)));
    let id = client(&mut f, 200);

    let first = f.map(id, WindowSpec::plain((400, 300)));

    assert!(has_focus(&f, first));
    assert!(f.has_keyboard(first));
    assert!(!dumped_urgent(&f, first));
}

#[test]
fn a_window_that_opens_by_itself_is_refused_and_put_under_the_focused_one() {
    let mut f = fixture("smart");
    let id = client(&mut f, 200);
    let earlier = f.map(id, WindowSpec::plain((400, 300)));
    let other = client(&mut f, 100);
    let focused = f.map(other, WindowSpec::plain((400, 300)));
    assert!(has_focus(&f, focused));

    // The process 200's second window, with no token, while the other client has the focus.
    let later = f.map(id, WindowSpec::plain((400, 300)));

    assert!(has_focus(&f, focused));
    assert!(f.has_keyboard(focused));
    assert!(dumped_urgent(&f, later));
    let stacking = f.state.caminito.policy.stacking(0);
    assert_eq!(stacking, [f.pid(earlier), f.pid(later), f.pid(focused)]);
}

#[test]
fn a_token_asked_for_before_the_window_maps_gives_it_the_focus() {
    let mut f = fixture("strict");
    let launcher = client(&mut f, 100);
    let terminal = f.map(launcher, WindowSpec::plain((400, 300)));
    click(&mut f, terminal);
    let token = token_from(&mut f, terminal);

    // The launched app activates its window with the token before it has content, as GTK
    // does with the token it was launched with.
    let id = client(&mut f, 200);
    let window = f.client(id).create_window();
    f.client(id).commit(window);
    f.roundtrip(id);
    f.client(id).activate(&token, window);
    f.client(id)
        .ack_and_commit_buffer(window, (400, 300), WINDOW_COLOR);
    f.roundtrip(id);

    let launched = ClientWindow::Xdg(id, window);
    assert!(has_focus(&f, launched));
    assert!(!dumped_urgent(&f, launched));
}

// XWayland windows (166).

/// `fixture` with an X client.
fn x_fixture(mode: &str) -> (Fixture, ClientId) {
    let mut f = fixture(mode);
    let x = f.connect(Kind::X11);
    (f, x)
}

/// An XWayland window of the process `pid`, as `_NET_WM_PID` says.
fn of_process(pid: u32) -> WindowSpec {
    WindowSpec {
        cardinals: vec![("_NET_WM_PID", vec![pid])],
        ..WindowSpec::default()
    }
}

/// Whether the XWayland window's `_NET_WM_STATE` has `_NET_WM_STATE_DEMANDS_ATTENTION`.
fn x_demands_attention(f: &mut Fixture, window: ClientWindow) -> bool {
    let id = window.x11();
    let states = f
        .x11_call(window.client(), move |x| {
            let mut names = caminito_test_client::x11::Names::new(x);
            names.property(x, id, "_NET_WM_STATE")
        })
        .unwrap();
    states.contains("_NET_WM_STATE_DEMANDS_ATTENTION")
}

/// Sends `_NET_ACTIVE_WINDOW` for the window with the source indication `source`: 1 for an
/// application, 2 for a pager, and 0 as a client older than the indication does. It gives
/// no timestamp.
fn ask_to_be_active(f: &mut Fixture, window: ClientWindow, source: u32) {
    ask_to_be_active_at(f, window, source, 0);
}

/// Sends `_NET_ACTIVE_WINDOW` for the window with the source indication `source` and X's
/// timestamp `time`.
fn ask_to_be_active_at(f: &mut Fixture, window: ClientWindow, source: u32, time: u32) {
    let id = window.x11();
    f.x11_call(window.client(), move |x| {
        x.send_client_message(id, "_NET_ACTIVE_WINDOW", [source, time, 0, 0, 0])
    })
    .unwrap();
}

/// X's timestamp `ago` milliseconds before now: Xwayland's time is Caminito's clock in 32
/// bits.
fn x_time_ago(f: &Fixture, ago: u32) -> u32 {
    (f.state.caminito.now().as_millis() as u32).wrapping_sub(ago)
}

/// XWayland windows of the processes 200 and 100, the last focused, under `mode`, which
/// is set once both are mapped, as *strict* would refuse the second.
fn x_window_beside_the_focused_one(mode: &str) -> (Fixture, ClientId, ClientWindow, ClientWindow) {
    let (mut f, x) = x_fixture("smart");
    let other = f.map(x, of_process(200));
    let focused = f.map(x, of_process(100));
    assert!(has_focus(&f, focused));
    let wanted = match mode {
        "smart" => FocusNewWindows::Smart,
        "strict" => FocusNewWindows::Strict,
        other => panic!("{other} isn't a mode"),
    };
    f.set_setting(MARCO_GENERAL, "focus-new-windows", Value::Str(mode.into()));
    f.dispatch_until(|f| f.state.caminito.settings.policy.focus_new_windows == wanted);
    (f, x, other, focused)
}

#[test]
fn an_x_clients_first_window_is_focused_under_smart_and_refused_under_strict() {
    for (mode, focuses) in [("smart", true), ("strict", false)] {
        let (mut f, x) = x_fixture(mode);
        let focused = f.map(x, of_process(100));
        assert!(has_focus(&f, focused), "{mode}");

        let first = f.map(x, of_process(200));

        assert_eq!(has_focus(&f, first), focuses, "{mode}");
        assert_eq!(f.has_keyboard(first), focuses, "{mode}");
        assert_eq!(dumped_urgent(&f, first), !focuses, "{mode}");
    }
}

#[test]
fn an_x_window_that_opens_by_itself_is_refused_and_demands_attention_until_focused() {
    let (mut f, x, earlier, focused) = x_window_beside_the_focused_one("smart");

    // The process 200's second window, while another client has the focus.
    let later = f.map(x, of_process(200));

    assert!(has_focus(&f, focused));
    assert!(f.has_keyboard(focused));
    assert!(dumped_urgent(&f, later));
    let stacking = f.state.caminito.policy.stacking(0);
    assert_eq!(stacking, [f.pid(earlier), f.pid(later), f.pid(focused)]);
    // Caminito says so in `_NET_WM_STATE`.
    f.dispatch_until_mut(|f| x_demands_attention(f, later));

    click(&mut f, later);

    assert!(has_focus(&f, later));
    assert!(!dumped_urgent(&f, later));
    f.dispatch_until_mut(|f| !x_demands_attention(f, later));
}

#[test]
fn a_dialog_transient_for_the_focused_x_window_is_focused_under_both_modes() {
    for mode in ["smart", "strict"] {
        let (mut f, x, _other, focused) = x_window_beside_the_focused_one(mode);

        // Not the process 200's first window, and not of the focused window's process.
        let dialog = f.map(
            x,
            WindowSpec {
                types: vec!["_NET_WM_WINDOW_TYPE_DIALOG"],
                parent: Some(focused),
                size: (300, 200),
                ..of_process(200)
            },
        );

        assert!(has_focus(&f, dialog), "{mode}");
        assert!(f.has_keyboard(dialog), "{mode}");
        assert!(!dumped_urgent(&f, dialog), "{mode}");
    }
}

#[test]
fn a_dialog_transient_for_an_x_window_that_is_not_focused_is_refused() {
    let (mut f, x, other, focused) = x_window_beside_the_focused_one("smart");

    let dialog = f.map(
        x,
        WindowSpec {
            types: vec!["_NET_WM_WINDOW_TYPE_DIALOG"],
            parent: Some(other),
            size: (300, 200),
            ..of_process(200)
        },
    );

    assert!(has_focus(&f, focused));
    assert!(dumped_urgent(&f, dialog));
}

#[test]
fn net_active_window_from_a_pager_or_an_old_client_is_honoured() {
    for (mode, source) in [("smart", 2), ("strict", 2), ("smart", 0), ("strict", 0)] {
        let (mut f, _x, other, _focused) = x_window_beside_the_focused_one(mode);

        ask_to_be_active(&mut f, other, source);

        f.dispatch_until(|f| has_focus(f, other));
        assert!(f.has_keyboard(other), "{mode} {source}");
        assert!(!dumped_urgent(&f, other), "{mode} {source}");
        assert_eq!(
            f.state.caminito.policy.stacking(0).last(),
            Some(&f.pid(other))
        );
    }
}

#[test]
fn net_active_window_from_an_application_that_has_not_the_focus_makes_the_window_urgent() {
    let (mut f, _x, other, focused) = x_window_beside_the_focused_one("smart");
    let stacking = f.state.caminito.policy.stacking(0);

    ask_to_be_active(&mut f, other, 1);

    f.dispatch_until(|f| dumped_urgent(f, other));
    assert!(has_focus(&f, focused));
    assert!(f.has_keyboard(focused));
    assert_eq!(f.state.caminito.policy.stacking(0), stacking);
    f.dispatch_until_mut(|f| x_demands_attention(f, other));
}

/// A launcher such as Albert, shown by a key binding's command while another client has
/// the focus: Qt's `requestActivate` gives the time its window was shown at.
#[test]
fn net_active_window_from_an_application_since_the_focus_last_changed_is_honoured() {
    for mode in ["smart", "strict"] {
        let (mut f, _x, other, _focused) = x_window_beside_the_focused_one(mode);

        let now = x_time_ago(&f, 0);
        ask_to_be_active_at(&mut f, other, 1, now);

        f.dispatch_until(|f| has_focus(f, other));
        assert!(f.has_keyboard(other), "{mode}");
        assert!(!dumped_urgent(&f, other), "{mode}");
        assert_eq!(
            f.state.caminito.policy.stacking(0).last(),
            Some(&f.pid(other))
        );
    }
}

#[test]
fn net_active_window_from_an_application_older_than_the_focus_makes_the_window_urgent() {
    let (mut f, _x, other, focused) = x_window_beside_the_focused_one("smart");

    let a_minute_ago = x_time_ago(&f, 60_000);
    ask_to_be_active_at(&mut f, other, 1, a_minute_ago);

    f.dispatch_until(|f| dumped_urgent(f, other));
    assert!(has_focus(&f, focused));
    assert!(f.has_keyboard(focused));
}

#[test]
fn net_active_window_from_the_focused_windows_application_is_honoured() {
    let (mut f, x, _other, focused) = x_window_beside_the_focused_one("strict");
    let second = f.map(x, of_process(100));
    click(&mut f, focused);
    assert!(has_focus(&f, focused));

    ask_to_be_active(&mut f, second, 1);

    f.dispatch_until(|f| has_focus(f, second));
    assert!(f.has_keyboard(second));
    assert!(!dumped_urgent(&f, second));
}

#[test]
fn an_x_client_takes_back_the_demand_for_attention_caminito_made() {
    let (mut f, _x, other, _focused) = x_window_beside_the_focused_one("smart");
    ask_to_be_active(&mut f, other, 1);
    f.dispatch_until_mut(|f| x_demands_attention(f, other));

    let id = other.x11();
    f.x11_call(other.client(), move |x| {
        let attention = x.atom("_NET_WM_STATE_DEMANDS_ATTENTION")?;
        x.send_client_message(id, "_NET_WM_STATE", [0, attention, 0, 1, 0])
    })
    .unwrap();

    f.dispatch_until(|f| !dumped_urgent(f, other));
    f.dispatch_until_mut(|f| !x_demands_attention(f, other));
}

/// An XWayland window that asks for attention before it maps has the focus as it maps, so
/// it demands none.
#[test]
fn an_x_window_that_maps_demanding_attention_and_takes_the_focus_demands_none() {
    let (mut f, x) = x_fixture("smart");

    let window = f.map(
        x,
        WindowSpec {
            states: vec!["_NET_WM_STATE_DEMANDS_ATTENTION"],
            ..of_process(100)
        },
    );

    assert!(has_focus(&f, window));
    assert!(!dumped_urgent(&f, window));
    f.dispatch_until_mut(|f| !x_demands_attention(f, window));
}

// The window list (166): libxfce4windowing reads Xfce's `demands_attention` as urgent.

/// A client that has bound what libxfce4windowing binds, and has seen the toplevels there
/// are.
fn panel(f: &mut Fixture) -> ClientId {
    let client = f.connect(Kind::Xdg);
    f.client(client).bind_outputs();
    f.client(client).bind_workspaces();
    f.client(client).bind_toplevel_workspaces();
    f.client(client).bind_foreign_toplevels();
    f.roundtrip(client);
    f.roundtrip(client);
    f.client(client).take_toplevel_events();
    client
}

/// What the panel was told of its toplevels' Xfce states since this was last asked.
fn xfce_states(f: &mut Fixture, panel: ClientId) -> Vec<String> {
    f.roundtrip(panel);
    let events = f.client(panel).take_toplevel_events();
    events
        .lines()
        .filter(|event| event.contains("xfce state"))
        .map(str::to_owned)
        .collect()
}

#[test]
fn a_panel_is_told_when_an_xdg_window_demands_attention_and_when_it_stops() {
    let mut f = fixture("strict");
    let focused = client(&mut f, 100);
    f.map(focused, WindowSpec::plain((400, 300)));
    let panel = panel(&mut f);

    let other = client(&mut f, 200);
    let refused = f.map(other, WindowSpec::plain((400, 300)));
    assert!(dumped_urgent(&f, refused));

    assert_eq!(
        xfce_states(&mut f, panel),
        ["toplevel 1: xfce state [DemandsAttention]"]
    );

    click(&mut f, refused);
    assert!(has_focus(&f, refused));

    assert_eq!(xfce_states(&mut f, panel), ["toplevel 1: xfce state []"]);
}

#[test]
fn a_panel_is_told_when_an_x_window_demands_attention_and_when_it_stops() {
    let (mut f, _x, other, _focused) = x_window_beside_the_focused_one("smart");
    let panel = panel(&mut f);

    ask_to_be_active(&mut f, other, 1);
    f.dispatch_until(|f| dumped_urgent(f, other));

    assert_eq!(
        xfce_states(&mut f, panel),
        ["toplevel 0: xfce state [DemandsAttention]"]
    );

    click(&mut f, other);
    assert!(has_focus(&f, other));

    assert_eq!(xfce_states(&mut f, panel), ["toplevel 0: xfce state []"]);
}

/// The urgency hint of `WM_HINTS` is the client's to end, so it outlasts the focus.
#[test]
fn a_panel_is_told_of_an_x_windows_urgency_hint() {
    let (mut f, _x, other, _focused) = x_window_beside_the_focused_one("smart");
    let panel = panel(&mut f);

    let id = other.x11();
    f.x11_call(other.client(), move |x| x.set_urgency_hint(id, true))
        .unwrap();
    f.dispatch_until(|f| dumped_urgent(f, other));

    assert_eq!(
        xfce_states(&mut f, panel),
        ["toplevel 0: xfce state [DemandsAttention]"]
    );

    f.x11_call(other.client(), move |x| x.set_urgency_hint(id, false))
        .unwrap();
    f.dispatch_until(|f| !dumped_urgent(f, other));

    assert_eq!(xfce_states(&mut f, panel), ["toplevel 0: xfce state []"]);
}
