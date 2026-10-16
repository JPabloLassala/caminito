//! The Maximise animation (CMN-174, ADR 0036): a window that's maximised or unmaximised both
//! ways goes from its frame as it was to its new one over 250 ms of Caminito's clock, eased
//! out, with a picture of it as it was fading out over it, read from the debug dump. Policy
//! has it at its new place from the start.

use std::time::Duration;

use caminito_test_client::format_input;
use smithay::utils::{Logical, Point, Rectangle};

use super::fixture::{
    BTN_LEFT, ClientId, ClientWindow, Decoration, Fixture, Kind, WINDOW_COLOR, WindowSpec,
    on_both_kinds,
};
use crate::maximize_animation::LENGTH;
use crate::settings::Value as Setting;
use crate::settings::translate::{CAMINITO, INTERFACE, MARCO_GENERAL};

const OUTPUT: (i32, i32) = (800, 600);

/// How far into the animation the tests read it.
const PART_WAY: Duration = Duration::from_millis(100);

/// How far the box has gone at `PART_WAY`: a cubic ease-out, `1 - (1 - 100/250)³`.
const DONE_PART_WAY: f64 = 0.784;

type Settings = Vec<(&'static str, String, Setting)>;

/// One output with `maximize-animation` on, unless `settings` has it, and a client of
/// `kind`. The fixture has the key off, for the tests written when a window was maximised
/// at once. The clock is stopped, so the animation is read where `advance_clock` has it.
fn desk(kind: Kind, mut settings: Settings) -> (Fixture, ClientId) {
    if !settings
        .iter()
        .any(|(_, key, _)| key == "maximize-animation")
    {
        settings.push((CAMINITO, "maximize-animation".into(), Setting::Bool(true)));
    }
    let mut f = Fixture::with_settings(settings);
    f.add_output(OUTPUT, 1.0);
    let id = f.connect(kind);
    f.stop_clock();
    (f, id)
}

/// A window of 400×300 away from the output's corner, drawn at rest.
fn floating(f: &mut Fixture, id: ClientId) -> ClientWindow {
    let window = f.map(id, WindowSpec::default());
    let pid = f.pid(window);
    let loc = Point::from((150, 120));
    f.state
        .caminito
        .policy
        .request_geometry(pid, Some(loc), None);
    f.dispatch();
    f.sync(window);
    window
}

/// Each window on its way, by policy's id, with its box and its picture's opacity, from the
/// debug dump's `maximizing_windows`, which isn't there while none is.
fn maximizing(f: &Fixture) -> Vec<(u64, Rectangle<i32, Logical>, f64)> {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    let int = |value: &serde_json::Value| value.as_i64().unwrap() as i32;
    let Some(windows) = dump["maximizing_windows"].as_array() else {
        return Vec::new();
    };
    let windows = windows.iter().map(|window| {
        let rect = &window["rect"];
        let rect = Rectangle::new(
            (int(&rect["x"]), int(&rect["y"])).into(),
            (int(&rect["width"]), int(&rect["height"])).into(),
        );
        let alpha = window["alpha"].as_f64().unwrap();
        (window["id"].as_u64().unwrap(), rect, alpha)
    });
    windows.collect()
}

/// `window`'s box and its picture's opacity, while it's the one window on its way.
#[track_caller]
fn drawn(f: &Fixture, window: ClientWindow) -> (Rectangle<i32, Logical>, f64) {
    let maximizing = maximizing(f);
    match maximizing[..] {
        [(id, rect, alpha)] if id == f.pid(window).0 => (rect, alpha),
        _ => panic!("{window:?} alone is on its way: {maximizing:?}"),
    }
}

/// The rectangle `done` of the way from `from` to `to`.
fn between(
    from: Rectangle<i32, Logical>,
    to: Rectangle<i32, Logical>,
    done: f64,
) -> Rectangle<i32, Logical> {
    let at = |from: i32, to: i32| from + (done * f64::from(to - from)).round() as i32;
    Rectangle::new(
        (at(from.loc.x, to.loc.x), at(from.loc.y, to.loc.y)).into(),
        (at(from.size.w, to.size.w), at(from.size.h, to.size.h)).into(),
    )
}

/// Whether two rectangles are the same to within two pixels each way.
fn near(a: Rectangle<i32, Logical>, b: Rectangle<i32, Logical>) -> bool {
    let close = |a: i32, b: i32| (a - b).abs() <= 2;
    close(a.loc.x, b.loc.x)
        && close(a.loc.y, b.loc.y)
        && close(a.size.w, b.size.w)
        && close(a.size.h, b.size.h)
}

/// The size policy last asked `window`'s client for.
fn asked_size(f: &Fixture, window: ClientWindow) -> (i32, i32) {
    let asked = f
        .state
        .caminito
        .policy
        .asked_geometry(f.pid(window))
        .unwrap();
    (asked.size.w, asked.size.h)
}

/// Checks that `window`, whose frame was at `from` and is asked to be at its new frame,
/// goes there over 250 ms, eased out, with its picture fading, and doesn't jump when its
/// client draws at its new size part-way.
#[track_caller]
fn assert_goes_from(f: &mut Fixture, window: ClientWindow, from: Rectangle<i32, Logical>) {
    let (start, alpha) = drawn(f, window);
    assert_eq!(start, from, "from its frame as it was drawn");
    assert_eq!(alpha, 1.0, "its picture over it, whole");

    f.advance_clock(PART_WAY);
    let (part_way, alpha) = drawn(f, window);
    let size = asked_size(f, window);
    f.draw_at_asked_size(window, size);
    let to = f.visible_frame(window);
    assert_ne!(from, to, "it went somewhere");
    assert_eq!(
        drawn(f, window).0,
        part_way,
        "the client's commit moves nothing"
    );
    let expected = between(from, to, DONE_PART_WAY);
    assert!(
        near(part_way, expected),
        "eased out: {part_way:?}, not {expected:?}"
    );
    assert!(
        (alpha - (1.0 - DONE_PART_WAY)).abs() < 0.01,
        "the picture fades as the box moves: {alpha}"
    );

    f.advance_clock(LENGTH);
    assert!(maximizing(f).is_empty(), "over: {:?}", maximizing(f));
    assert_eq!(f.visible_frame(window), to);
}

on_both_kinds! {
    fn a_maximised_window_goes_from_its_frame_into_the_work_area(kind: Kind) {
        let (mut f, id) = desk(kind, Vec::new());
        let window = floating(&mut f, id);
        let from = f.visible_frame(window);

        // Policy's own maximise, which a client's request, a binding or a button ends in.
        f.state.caminito.policy.set_maximized(f.pid(window), true);
        f.dispatch();

        assert_goes_from(&mut f, window, from);
    }
}

on_both_kinds! {
    fn an_unmaximised_window_goes_back_to_where_it_was(kind: Kind) {
        let (mut f, id) = desk(kind, Vec::new());
        let window = floating(&mut f, id);
        let floating_frame = f.visible_frame(window);
        f.state.caminito.policy.set_maximized(f.pid(window), true);
        f.dispatch();
        let size = asked_size(&f, window);
        f.draw_at_asked_size(window, size);
        f.advance_clock(LENGTH);
        let from = f.visible_frame(window);

        f.state.caminito.policy.set_maximized(f.pid(window), false);
        f.dispatch();

        assert_goes_from(&mut f, window, from);
        assert_eq!(f.visible_frame(window), floating_frame);
    }
}

on_both_kinds! {
    fn a_window_whose_client_doesnt_draw_is_at_rest_after_250_ms(kind: Kind) {
        let (mut f, id) = desk(kind, Vec::new());
        let window = floating(&mut f, id);

        f.state.caminito.policy.set_maximized(f.pid(window), true);
        f.dispatch();
        assert!(!maximizing(&f).is_empty());

        f.advance_clock(LENGTH);
        assert!(maximizing(&f).is_empty(), "over: {:?}", maximizing(&f));
    }
}

// What a client is sent is read from a Wayland client, as `input.rs` reads it.
#[test]
fn a_click_part_way_reaches_the_window_at_its_real_place() {
    let (mut f, id) = desk(Kind::Xdg, Vec::new());
    f.client(id).bind_input();
    // With no frame, the window's corner is the frame's.
    let window = f.map(id, WindowSpec::plain((400, 300)));
    let pid = f.pid(window);
    f.state
        .caminito
        .policy
        .request_geometry(pid, Some(Point::from((300, 200))), None);
    f.dispatch();
    f.roundtrip(id);
    f.state.caminito.policy.set_maximized(pid, true);
    f.dispatch();
    f.draw_at_asked_size(window, OUTPUT);
    f.advance_clock(PART_WAY);

    let spot = Point::from((5, 5));
    let (rect, _) = drawn(&f, window);
    assert!(!rect.contains(spot), "{spot:?} is outside {rect:?}");
    f.client(id).state.input.clear();
    f.pointer_move(f64::from(spot.x), f64::from(spot.y));
    f.pointer_button(BTN_LEFT, true);
    f.pointer_button(BTN_LEFT, false);
    f.roundtrip(id);
    // The pointer was already over the maximised window, which the client alone has.
    let input = format_input(&f.client(id).state.input);
    assert!(
        input.contains(&format!("button {BTN_LEFT:#x} pressed")),
        "{input}"
    );
}

// The settings are read the same for both kinds.

/// Maximises a window and checks that it isn't animated.
#[track_caller]
fn assert_maximised_at_once(mut f: Fixture, id: ClientId) {
    let window = floating(&mut f, id);
    f.state.caminito.policy.set_maximized(f.pid(window), true);
    f.dispatch();
    assert!(maximizing(&f).is_empty(), "{:?}", maximizing(&f));
}

#[test]
fn nothing_animates_with_its_key_or_any_of_the_three_off() {
    for (schema, key, value) in [
        (CAMINITO, "maximize-animation", false),
        (MARCO_GENERAL, "compositing-manager", false),
        (MARCO_GENERAL, "reduced-resources", true),
        (INTERFACE, "enable-animations", false),
    ] {
        let (f, id) = desk(Kind::Xdg, vec![(schema, key.into(), Setting::Bool(value))]);
        assert_maximised_at_once(f, id);
    }
}

#[test]
fn maximising_one_way_and_fullscreen_dont_animate() {
    let (mut f, id) = desk(Kind::Xdg, Vec::new());
    let window = floating(&mut f, id);
    let pid = f.pid(window);

    f.state.caminito.policy.set_maximized_vertically(pid, true);
    f.dispatch();
    assert!(
        maximizing(&f).is_empty(),
        "vertically: {:?}",
        maximizing(&f)
    );
    f.state.caminito.policy.set_maximized_vertically(pid, false);
    f.dispatch();

    f.state.caminito.policy.set_fullscreen(pid, true, None);
    f.dispatch();
    assert!(
        maximizing(&f).is_empty(),
        "fullscreen: {:?}",
        maximizing(&f)
    );
}

/// How far a CSD window's surface reaches past its window geometry each way: its shadow.
const SHADOW: i32 = 40;

/// Acks the client's last configure and draws a CSD window at the size it asks for, with a
/// shadow of `shadow` around it outside its window geometry, as GTK and Firefox draw one.
fn draw_with_shadow(f: &mut Fixture, window: ClientWindow, shadow: i32) {
    let ClientWindow::Xdg(id, xdg) = window else {
        unreachable!("an xdg window")
    };
    f.roundtrip(id);
    let client = f.client(id);
    let (w, h) = client.window(xdg).configures.last().unwrap().size;
    client.ack_last_configure(xdg);
    client.set_window_geometry(xdg, (shadow, shadow, w, h));
    client.attach_buffer(xdg, (w + 2 * shadow, h + 2 * shadow), WINDOW_COLOR);
    client.commit(xdg);
    f.roundtrip(id);
}

// A CSD window's shadow is drawn scaled with it, past its box: an output it reaches is drawn
// at each frame too, as the box's is, or it shows the shadow only at the frames something
// else has it drawn, and flickers.
#[test]
fn an_output_the_shadow_reaches_part_way_is_drawn_at_every_frame() {
    let (mut f, id) = desk(Kind::Xdg, Vec::new());
    let left = f.state.caminito.outputs[0].clone();
    let right = f.add_output(OUTPUT, 1.0);
    f.set_output_position(&left, (0, 0));
    f.set_output_position(&right, (OUTPUT.0, 0));
    let spec = WindowSpec {
        decoration: Decoration::Client,
        ..WindowSpec::default()
    };
    let window = f.map(id, spec);
    let pid = f.pid(window);
    f.state
        .caminito
        .policy
        .request_geometry(pid, Some(Point::from((150, 120))), None);
    f.dispatch();
    draw_with_shadow(&mut f, window, SHADOW);
    // Maximised it has no shadow, as GTK and Firefox draw it.
    f.state.caminito.policy.set_maximized(pid, true);
    f.dispatch();
    draw_with_shadow(&mut f, window, 0);
    f.advance_clock(LENGTH);
    assert_eq!(f.visible_frame(window), Rectangle::from_size(OUTPUT.into()));

    f.state.caminito.policy.set_maximized(pid, false);
    f.dispatch();
    draw_with_shadow(&mut f, window, SHADOW);
    f.draw(&right);

    // Just inside the right output, which the box never reaches.
    let spot = Point::from((5, OUTPUT.1 / 2));
    let mut reached = false;
    for _ in 0..25 {
        f.advance_clock(Duration::from_millis(10));
        let shown = f.state.backend.headless_mut().pixel(&right, spot);
        f.draw(&right);
        let drawn = f.state.backend.headless_mut().pixel(&right, spot);
        assert_eq!(shown, drawn, "the right output was drawn at that frame");
        reached |= drawn == Some(WINDOW_COLOR);
    }
    assert!(reached, "the shadow reaches the right output part-way");
}
