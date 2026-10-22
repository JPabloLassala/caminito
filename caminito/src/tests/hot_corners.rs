//! Hot corners (CMN-183): a corner whose key is set fires its action once the pointer has
//! rested in it the Activation delay, 150 ms by default (CMN-191), fires again only once the pointer has left and 350 ms have passed,
//! is a corner only where the Output layout stops the pointer both ways, and is off with a
//! button held, during another Interaction, while locked, over a focused fullscreen window
//! on its output and with the pointer locked or confined. Each test stops Caminito's clock,
//! so only `advance_clock` moves it, and a corner that mustn't fire is given 500 ms of it,
//! past both the rest and the 350 ms.
//!
//! The windows are xdg toplevels only: a corner fires whatever is under the pointer, and
//! the fullscreen window and the pointer constraints are policy's state and the client's
//! constraint, the same for both kinds.

use std::time::Duration;

use smithay::output::Output;

use super::fixture::{
    BTN_LEFT, ClientId, ClientWindow, Decoration, Fixture, KEY_LEFTMETA, KEY_W, Kind, WindowSpec,
};
use super::overview::{is_open, overview};
use super::per_output_workspaces::{put_at, two_outputs_with};
use crate::output::output_geometry;
use crate::settings::Value;
use crate::settings::translate::CAMINITO;
use crate::state::SEAT;

/// The key of `corner`, such as `top-left`, set to `action`.
fn corner(corner: &str, action: &str) -> (&'static str, String, Value) {
    let key = format!("hot-corner-{corner}");
    (CAMINITO, key, Value::Str(action.into()))
}

/// Caminito with one output of 1920 × 1080, `settings` set, and its clock stopped.
fn one_output(settings: Vec<(&'static str, String, Value)>) -> Fixture {
    let mut f = Fixture::with_settings(settings);
    f.add_output((1920, 1080), 1.0);
    f.stop_clock();
    f.dispatch();
    f
}

/// `locker-command` set to a locker that runs until its stdin closes.
fn locker() -> (&'static str, String, Value) {
    let command = ["sh", "-c", "exec cat >/dev/null"]
        .map(String::from)
        .to_vec();
    (CAMINITO, "locker-command".into(), Value::StrList(command))
}

fn dump(f: &Fixture) -> serde_json::Value {
    serde_json::from_str(&f.state.caminito.state_dump()).unwrap()
}

fn showing_desktop(f: &Fixture) -> bool {
    dump(f)["showing_desktop"].as_bool().unwrap()
}

fn ms(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

#[test]
fn a_corner_fires_once_the_pointer_has_rested_in_it_150_ms() {
    let mut f = one_output(vec![corner("top-left", "show-desktop")]);
    f.pointer_move(0., 0.);

    f.advance_clock(ms(149));
    assert!(!showing_desktop(&f), "it hasn't rested long enough");

    f.advance_clock(ms(1));
    assert!(showing_desktop(&f));
}

/// `hot-corner-activation-delay` set to `ms`.
fn activation_delay(ms: u32) -> (&'static str, String, Value) {
    let key = "hot-corner-activation-delay".into();
    (CAMINITO, key, Value::Uint(ms))
}

/// Sets `hot-corner-activation-delay` to `ms`, as `gsettings set` would, and waits until
/// Caminito has it.
fn set_activation_delay(f: &mut Fixture, ms: u32) {
    let key = "hot-corner-activation-delay";
    f.set_setting(CAMINITO, key, Value::Uint(ms));
    let delay = Duration::from_millis(ms.into());
    f.dispatch_until(|f| f.state.caminito.settings.hot_corners.activation_delay == delay);
}

#[test]
fn a_corner_fires_once_the_pointer_has_rested_in_it_the_activation_delay() {
    let settings = vec![corner("top-left", "show-desktop"), activation_delay(400)];
    let mut f = one_output(settings);
    f.pointer_move(0., 0.);

    f.advance_clock(ms(399));
    assert!(!showing_desktop(&f), "it hasn't rested long enough");

    f.advance_clock(ms(1));
    assert!(showing_desktop(&f));
}

#[test]
fn with_no_activation_delay_a_corner_fires_as_the_pointer_reaches_it() {
    let settings = vec![corner("top-left", "show-desktop"), activation_delay(0)];
    let mut f = one_output(settings);

    f.pointer_move(0., 0.);
    f.dispatch_until(showing_desktop);
}

#[test]
fn a_longer_activation_delay_counts_for_the_rest_under_way() {
    let mut f = one_output(vec![corner("top-left", "show-desktop")]);
    f.pointer_move(0., 0.);
    f.advance_clock(ms(100));

    set_activation_delay(&mut f, 500);
    f.advance_clock(ms(399));
    assert!(!showing_desktop(&f), "it has rested 499 ms of the 500");

    f.advance_clock(ms(1));
    assert!(showing_desktop(&f));
}

#[test]
fn a_shorter_activation_delay_counts_for_the_rest_under_way() {
    let settings = vec![corner("top-left", "show-desktop"), activation_delay(500)];
    let mut f = one_output(settings);
    f.pointer_move(0., 0.);
    f.advance_clock(ms(100));

    set_activation_delay(&mut f, 300);
    f.advance_clock(ms(199));
    assert!(!showing_desktop(&f), "it has rested 299 ms of the 300");

    f.advance_clock(ms(1));
    assert!(showing_desktop(&f));
}

#[test]
fn an_activation_delay_shorter_than_the_rest_so_far_fires_the_corner_at_once() {
    let settings = vec![corner("top-left", "show-desktop"), activation_delay(500)];
    let mut f = one_output(settings);
    f.pointer_move(0., 0.);
    f.advance_clock(ms(200));
    assert!(!showing_desktop(&f));

    // With the clock stopped: the loop's timer is what fires it.
    set_activation_delay(&mut f, 50);
    f.dispatch_until(showing_desktop);
}

#[test]
fn the_350_ms_hold_with_no_activation_delay() {
    let settings = vec![corner("top-right", "show-desktop"), activation_delay(0)];
    let mut f = one_output(settings);
    f.pointer_move(1919., 0.);
    f.dispatch_until(showing_desktop);

    f.pointer_move(1800., 100.);
    f.pointer_move(1919., 0.);
    f.advance_clock(ms(349));
    assert!(showing_desktop(&f), "350 ms haven't passed");
    f.advance_clock(ms(1));
    assert!(!showing_desktop(&f));
}

#[test]
fn a_corner_the_pointer_leaves_sooner_doesnt_fire() {
    let mut f = one_output(vec![corner("bottom-right", "show-desktop")]);
    f.pointer_move(1919., 1079.);
    f.advance_clock(ms(100));

    f.pointer_move(1900., 1079.);
    f.advance_clock(ms(500));

    assert!(!showing_desktop(&f));
}

#[test]
fn it_fires_again_only_once_the_pointer_has_left_and_350_ms_have_passed() {
    let mut f = one_output(vec![corner("top-right", "show-desktop")]);
    f.pointer_move(1919., 0.);
    f.advance_clock(ms(150));
    assert!(showing_desktop(&f));

    // Back at once: the rest is over 150 ms on, but 350 ms have passed only 200 ms later.
    f.pointer_move(1800., 100.);
    f.pointer_move(1919., 0.);
    f.advance_clock(ms(349));
    assert!(showing_desktop(&f), "350 ms haven't passed");
    f.advance_clock(ms(1));
    assert!(!showing_desktop(&f), "the second toggle stops showing it");

    f.advance_clock(ms(1000));
    assert!(!showing_desktop(&f), "the pointer hasn't left");

    f.pointer_move(1800., 100.);
    f.pointer_move(1919., 0.);
    f.advance_clock(ms(149));
    assert!(
        !showing_desktop(&f),
        "long after, the rest is what it waits for"
    );
    f.advance_clock(ms(1));
    assert!(showing_desktop(&f));
}

#[test]
fn a_corner_shared_with_another_output_doesnt_fire() {
    let mut f = Fixture::with_settings(vec![corner("top-right", "show-desktop")]);
    f.add_output((1920, 1080), 1.0);
    let right = f.add_output((1920, 1080), 1.0);
    assert_eq!(output_geometry(&right).loc, (1920, 0).into());
    f.stop_clock();

    f.pointer_move(1919., 0.);
    f.advance_clock(ms(500));

    assert!(!showing_desktop(&f));
}

#[test]
fn the_inner_corner_of_two_outputs_of_different_heights_fires() {
    let mut f = Fixture::with_settings(vec![corner("bottom-left", "show-desktop")]);
    f.add_output((1920, 1080), 1.0);
    let taller = f.add_output((1920, 1200), 1.0);
    assert_eq!(output_geometry(&taller).loc, (1920, 0).into());
    f.stop_clock();

    f.pointer_move(1920., 1199.);
    f.advance_clock(ms(150));

    assert!(showing_desktop(&f));
}

/// Two outputs side by side with `top-right` set to the Overview, and a focused window on
/// the left one, which is the Active output.
fn overview_on_the_right() -> (Fixture, ClientId, Output, Output) {
    let settings = vec![corner("top-right", "overview")];
    let (mut f, client, left, right) = two_outputs_with(Kind::Xdg, true, settings);
    f.map(client, WindowSpec::plain((400, 300)));
    f.stop_clock();
    assert_eq!(dump(&f)["active_output"], left.name().as_str());
    (f, client, left, right)
}

#[test]
fn the_overview_opens_on_the_corners_output_when_the_active_output_is_another() {
    let (mut f, _, _, right) = overview_on_the_right();

    f.pointer_move(3839., 0.);
    f.advance_clock(ms(150));

    assert!(is_open(&f));
    assert_eq!(overview(&f)["output"], right.name().as_str());
    assert_eq!(dump(&f)["active_output"], right.name().as_str());
}

#[test]
fn the_overview_corner_closes_the_open_overview() {
    let (mut f, _, _, _) = overview_on_the_right();
    f.chord(&[KEY_LEFTMETA, KEY_W]);
    assert!(is_open(&f));

    f.pointer_move(3839., 0.);
    f.advance_clock(ms(150));

    assert!(!is_open(&f));
}

#[test]
fn the_lock_corner_locks_the_session() {
    let mut f = one_output(vec![corner("bottom-left", "lock"), locker()]);

    f.pointer_move(0., 1079.);
    f.advance_clock(ms(150));

    assert_ne!(f.lock_state(), "unlocked");
}

#[test]
fn a_corner_doesnt_fire_with_a_button_held() {
    let mut f = one_output(vec![corner("top-left", "show-desktop")]);
    f.pointer_move(100., 100.);
    f.pointer_button(BTN_LEFT, true);

    f.pointer_move(0., 0.);
    f.advance_clock(ms(500));

    assert!(!showing_desktop(&f));
}

#[test]
fn a_corner_doesnt_fire_during_another_interaction() {
    let mut f = one_output(vec![corner("top-left", "show-desktop")]);
    f.chord(&[KEY_LEFTMETA, KEY_W]);
    assert!(is_open(&f));

    f.pointer_move(0., 0.);
    f.advance_clock(ms(500));

    assert!(!showing_desktop(&f));
}

#[test]
fn a_corner_doesnt_fire_while_the_session_is_locked() {
    let mut f = one_output(vec![corner("top-left", "show-desktop"), locker()]);
    assert!(f.state.request_lock());

    f.pointer_move(0., 0.);
    f.advance_clock(ms(500));

    assert!(!showing_desktop(&f));
}

#[test]
fn a_corner_doesnt_fire_over_a_focused_fullscreen_window_on_its_output() {
    let settings = vec![
        corner("top-left", "show-desktop"),
        corner("top-right", "show-desktop"),
    ];
    let (mut f, client, left, _) = two_outputs_with(Kind::Xdg, true, settings);
    let window = f.map(client, WindowSpec::plain((400, 300)));
    let id = f.pid(window);
    f.client(client).set_fullscreen(window.xdg(), None);
    f.dispatch_until(|f| f.state.caminito.policy.is_fullscreen(id));
    assert_eq!(f.state.caminito.policy.focus(SEAT), Some(id));
    assert_eq!(output_geometry(&left).loc, (0, 0).into());
    f.stop_clock();

    f.pointer_move(0., 0.);
    f.advance_clock(ms(500));
    assert!(!showing_desktop(&f));

    // The other output's corner isn't the fullscreen window's.
    f.pointer_move(3839., 0.);
    f.advance_clock(ms(150));
    assert!(showing_desktop(&f));
}

/// One output with `top-left` set to Show desktop, and a window of its own decoration
/// whose top left is the output's, under the pointer, whose client has `constrain`ed the
/// pointer and seen it take effect.
fn constrained(
    constrain: impl FnOnce(&mut Fixture, ClientId, ClientWindow),
    took_effect: impl Fn(&caminito_test_client::State) -> bool,
) -> Fixture {
    let mut f = one_output(vec![corner("top-left", "show-desktop")]);
    let client = f.connect(Kind::Xdg);
    f.client(client).bind_input();
    let spec = WindowSpec {
        decoration: Decoration::Client,
        ..WindowSpec::plain((400, 300))
    };
    let window = f.map(client, spec);
    put_at(&mut f, window, (0, 0), 0);
    f.pointer_move(10., 10.);
    constrain(&mut f, client, window);
    f.roundtrip(client);
    assert!(took_effect(f.client_state(client)));
    f
}

#[test]
fn a_corner_doesnt_fire_with_the_pointer_locked() {
    let lock = |f: &mut Fixture, client, window: ClientWindow| {
        f.client(client).lock_pointer(window.xdg());
    };
    let mut f = constrained(lock, |client| client.pointer_locked);

    // An absolute device moves a locked pointer all the same.
    f.pointer_move(0., 0.);
    f.advance_clock(ms(500));

    assert!(!showing_desktop(&f));
}

#[test]
fn a_corner_doesnt_fire_with_the_pointer_confined() {
    let confine = |f: &mut Fixture, client, window: ClientWindow| {
        f.client(client).confine_pointer(window.xdg());
    };
    let mut f = constrained(confine, |client| client.pointer_confined);

    f.pointer_move_by(-10., -10.);
    assert_eq!(
        f.state
            .caminito
            .seat
            .get_pointer()
            .unwrap()
            .current_location(),
        (0., 0.).into()
    );
    f.advance_clock(ms(500));

    assert!(!showing_desktop(&f));
}

#[test]
fn every_corner_is_off_by_default() {
    let mut f = one_output(vec![]);

    for (x, y) in [(0., 0.), (1919., 0.), (0., 1079.), (1919., 1079.)] {
        f.pointer_move(x, y);
        f.advance_clock(ms(500));
    }

    assert!(!showing_desktop(&f));
    assert!(!is_open(&f));
    assert_eq!(f.lock_state(), "unlocked");
}
