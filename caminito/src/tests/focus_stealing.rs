//! Focus-stealing prevention for native windows (165): xdg-activation's tokens, and
//! `focus-new-windows` judging them, on xdg windows only, since XWayland's are 166's. The
//! fixture's clients are all of the test's own process, so the clients here are connected
//! as processes of their own, which is how policy tells one client's windows from another's.

use caminito_policy::FocusNewWindows;
use caminito_test_client::xdg_activation::TokenId;

use super::fixture::{ClientId, ClientWindow, Fixture, WINDOW_COLOR, WindowSpec};
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
    f.roundtrip(window.client());
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
