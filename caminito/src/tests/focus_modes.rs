//! Marco's focus modes, `raise-on-click` and auto-raise (153): the pointer's crossings, as
//! Caminito reports them to policy, moving the focus and the keyboard with it, and raising.

use std::time::Duration;

use caminito_policy::WindowId;
use caminito_test_client::layer_shell::{KeyboardInteractivity, Layer, LayerAnchor, LayerSpec};
use caminito_theme::ButtonFunction;
use smithay::utils::{Logical, Point};

use super::fixture::{
    BTN_LEFT, ClientId, ClientWindow, Fixture, KEY_LEFTALT, KEY_TAB, Kind, WINDOW_COLOR,
    WindowSpec, on_both_kinds,
};
use crate::decoration::control::FrameControl;
use crate::settings::Value;
use crate::settings::translate::MARCO_GENERAL;
use crate::state::SEAT;

/// Where nothing is: no window, and no Layer surface.
const DESKTOP: (f64, f64) = (1900., 20.);

/// One 1920 × 1080 output and a client of `kind`, with these keys of
/// `org.mate.Marco.general` set from the start.
fn connected_with(kind: Kind, keys: &[(&str, Value)]) -> (Fixture, ClientId) {
    let settings = keys
        .iter()
        .map(|(key, value)| (MARCO_GENERAL, (*key).to_owned(), value.clone()))
        .collect();
    let mut f = Fixture::with_settings(settings);
    f.add_output((1920, 1080), 1.0);
    let id = f.connect(kind);
    (f, id)
}

fn focus_mode(mode: &str) -> (&'static str, Value) {
    ("focus-mode", Value::Str(mode.into()))
}

/// Two plain windows of `client`, the second cascaded over the first and focused.
fn two_windows(f: &mut Fixture, client: ClientId) -> (ClientWindow, ClientWindow) {
    let a = f.map(client, WindowSpec::plain((400, 300)));
    let b = f.map(client, WindowSpec::plain((400, 300)));
    (a, b)
}

/// A point inside `window` that the window cascaded over it doesn't cover: its top-left
/// corner.
fn top_left_of(f: &Fixture, window: ClientWindow) -> (f64, f64) {
    let at = f.visible_frame(window).loc.to_f64() + Point::<f64, Logical>::from((5., 5.));
    (at.x, at.y)
}

/// A point inside `window` that the window it's cascaded over doesn't take: its
/// bottom-right corner.
fn bottom_right_of(f: &Fixture, window: ClientWindow) -> (f64, f64) {
    let frame = f.visible_frame(window);
    let at = (frame.loc + frame.size).to_f64() - Point::<f64, Logical>::from((5., 5.));
    (at.x, at.y)
}

fn focus(f: &Fixture) -> Option<WindowId> {
    f.state.caminito.policy.focus(SEAT)
}

fn stacking(f: &Fixture) -> Vec<WindowId> {
    f.state.caminito.policy.stacking(0).to_vec()
}

fn move_to(f: &mut Fixture, (x, y): (f64, f64)) {
    f.pointer_move(x, y);
}

on_both_kinds! {
    fn under_sloppy_focus_the_focus_follows_the_pointer_and_stays_on_the_desktop(kind: Kind) {
        let (mut f, id) = connected_with(kind, &[focus_mode("sloppy")]);
        let (a, b) = two_windows(&mut f, id);
        let before = stacking(&f);

        let at = top_left_of(&f, a);

        move_to(&mut f, at);
        assert_eq!(focus(&f), f.policy_id(a));
        assert!(f.has_keyboard(a));
        // Sloppy focus doesn't raise.
        assert_eq!(stacking(&f), before);

        let at = bottom_right_of(&f, b);

        move_to(&mut f, at);
        assert_eq!(focus(&f), f.policy_id(b));
        assert!(f.has_keyboard(b));

        move_to(&mut f, DESKTOP);
        assert_eq!(focus(&f), f.policy_id(b));
        assert!(f.has_keyboard(b));
    }
}

on_both_kinds! {
    fn under_mouse_focus_leaving_for_the_desktop_takes_the_focus_away(kind: Kind) {
        let (mut f, id) = connected_with(kind, &[focus_mode("mouse")]);
        let (a, b) = two_windows(&mut f, id);

        let at = top_left_of(&f, a);

        move_to(&mut f, at);
        assert_eq!(focus(&f), f.policy_id(a));
        assert!(f.has_keyboard(a));

        move_to(&mut f, DESKTOP);
        assert_eq!(focus(&f), None);
        assert!(!f.has_keyboard(a));
        assert!(!f.has_keyboard(b));

        let at = bottom_right_of(&f, b);

        move_to(&mut f, at);
        assert_eq!(focus(&f), f.policy_id(b));
        assert!(f.has_keyboard(b));
    }
}

on_both_kinds! {
    fn auto_raise_raises_the_window_the_pointer_focused_after_its_delay(kind: Kind) {
        let (mut f, id) = connected_with(
            kind,
            &[
                focus_mode("sloppy"),
                ("auto-raise", Value::Bool(true)),
                ("auto-raise-delay", Value::Int(2000)),
            ],
        );
        let (a, b) = two_windows(&mut f, id);
        let (pa, pb) = (f.pid(a), f.pid(b));

        let at = top_left_of(&f, a);

        move_to(&mut f, at);
        assert_eq!(focus(&f), Some(pa));
        // Half the delay, with the real time the test takes to spare.
        f.advance_clock(Duration::from_millis(1000));
        assert_eq!(stacking(&f), [pa, pb]);

        f.advance_clock(Duration::from_millis(1000));
        assert_eq!(stacking(&f), [pb, pa]);
    }
}

on_both_kinds! {
    fn auto_raise_draws_the_window_it_raised_under_a_pointer_that_stays_still(kind: Kind) {
        let (mut f, id) = connected_with(
            kind,
            &[
                focus_mode("sloppy"),
                ("auto-raise", Value::Bool(true)),
                ("auto-raise-delay", Value::Int(2000)),
            ],
        );
        // Too big to go side by side, so `b` covers a part of `a`.
        let big = || WindowSpec {
            size: (1200, 700),
            ..WindowSpec::default()
        };
        let a = f.map(id, big());
        let b = f.map(id, big());
        // On the titlebar of `a`, under `b`'s body.
        let frame = f.visible_frame(a);
        let at = frame.loc + Point::from((300, 10));
        assert!(f.geometry(b).contains(at) && !f.geometry(a).contains(at));
        let body = WINDOW_COLOR & 0xff_ffff;

        let still = bottom_right_of(&f, a);
        move_to(&mut f, still);
        assert_eq!(focus(&f), f.policy_id(a));
        assert_eq!(pixel(&mut f, at), body);

        // Nothing but the time passes: the raise is the one cause of a frame (179).
        f.advance_clock(Duration::from_millis(2500));

        assert_ne!(pixel(&mut f, at), body);
    }
}

/// The first output's pixel at `at`, as `0xRRGGBB`, as it was last drawn: nothing asks for
/// a frame here.
fn pixel(f: &mut Fixture, at: Point<i32, Logical>) -> u32 {
    let output = f.state.caminito.outputs[0].clone();
    let pixel = f
        .state
        .backend
        .headless_mut()
        .pixel(&output, at.to_physical(1));
    pixel.expect("the output is drawn") & 0xff_ffff
}

#[test]
fn auto_raise_is_cancelled_when_the_pointer_leaves_the_window_first() {
    let (mut f, id) = connected_with(
        Kind::Xdg,
        &[focus_mode("sloppy"), ("auto-raise", Value::Bool(true))],
    );
    let (a, b) = two_windows(&mut f, id);
    let (pa, pb) = (f.pid(a), f.pid(b));

    let at = top_left_of(&f, a);

    move_to(&mut f, at);
    move_to(&mut f, DESKTOP);
    f.advance_clock(Duration::from_millis(500));

    assert_eq!(stacking(&f), [pa, pb]);
    assert_eq!(focus(&f), Some(pa));
}

on_both_kinds! {
    fn the_pointer_entering_a_window_under_click_focus_changes_nothing(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let (a, b) = two_windows(&mut f, id);

        let at = top_left_of(&f, a);

        move_to(&mut f, at);

        assert_eq!(focus(&f), f.policy_id(b));
        assert!(f.has_keyboard(b));
    }
}

on_both_kinds! {
    fn without_raise_on_click_a_click_focuses_the_window_without_raising_it(kind: Kind) {
        let (mut f, id) = connected_with(kind, &[("raise-on-click", Value::Bool(false))]);
        let (a, b) = two_windows(&mut f, id);
        let (pa, pb) = (f.pid(a), f.pid(b));

        let (x, y) = top_left_of(&f, a);
        // Below the frame's top, if there is one, so on the window's body.
        let y = y + f64::from(f.frame_extents(a).top);
        f.click(x, y);

        assert_eq!(focus(&f), Some(pa));
        assert_eq!(stacking(&f), [pa, pb]);
    }
}

on_both_kinds! {
    fn without_raise_on_click_a_click_on_the_titlebar_raises_the_window(kind: Kind) {
        let (mut f, id) = connected_with(kind, &[("raise-on-click", Value::Bool(false))]);
        let a = f.map(id, WindowSpec::default());
        let b = f.map(id, WindowSpec::default());
        let (pa, pb) = (f.pid(a), f.pid(b));
        let close = f.button_rect(a, ButtonFunction::Close).to_f64();
        let geometry = f.geometry(a);
        let x = f64::from(geometry.loc.x + geometry.size.w / 2);
        f.pointer_move(x, close.loc.y + close.size.h / 2.);
        assert_eq!(f.control_under_pointer(), Some(FrameControl::Title));

        f.pointer_button(BTN_LEFT, true);
        assert_eq!(focus(&f), Some(pa));
        assert_eq!(stacking(&f), [pa, pb]);
        f.pointer_button(BTN_LEFT, false);

        assert_eq!(stacking(&f), [pb, pa]);
    }
}

#[test]
fn a_layer_surface_that_takes_the_keyboard_on_demand_is_not_focused_by_the_pointer() {
    let (mut f, id) = connected_with(Kind::Xdg, &[focus_mode("mouse")]);
    let window = f.map(id, WindowSpec::plain((400, 300)));
    let spec = LayerSpec {
        anchor: LayerAnchor::Bottom | LayerAnchor::Right,
        keyboard: KeyboardInteractivity::OnDemand,
        ..LayerSpec::floating(Layer::Top, (200, 100))
    };
    let shell = f.connect(Kind::Xdg);
    f.map_layer(shell, spec, 0xff_00_80_ff);
    let at = top_left_of(&f, window);
    move_to(&mut f, at);

    move_to(&mut f, (1820., 1030.));

    assert_eq!(focus(&f), f.policy_id(window));
    assert!(f.has_keyboard(window));
    assert!(f.state.caminito.layer_with_keyboard().is_none());
}

on_both_kinds! {
    fn the_focus_stays_put_under_the_pointer_while_the_window_switcher_is_open(kind: Kind) {
        let (mut f, id) = connected_with(kind, &[focus_mode("sloppy")]);
        let (a, b) = two_windows(&mut f, id);
        let at = bottom_right_of(&f, b);
        move_to(&mut f, at);

        f.key(KEY_LEFTALT, true);
        f.key(KEY_TAB, true);
        f.key(KEY_TAB, false);
        assert!(f.state.caminito.policy.switcher().is_some());
        let at = top_left_of(&f, a);
        move_to(&mut f, at);
        let at = bottom_right_of(&f, b);
        move_to(&mut f, at);

        assert_eq!(focus(&f), f.policy_id(b));
        assert!(f.has_keyboard(b));
        f.key(KEY_LEFTALT, false);
        assert_eq!(focus(&f), f.policy_id(a));
    }
}
