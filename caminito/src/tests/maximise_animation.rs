//! The Maximise animation (CMN-174, ADR 0036): a window that's maximised or unmaximised both
//! ways goes from its frame as it was to its new one over 250 ms of Caminito's clock, eased
//! out, with a picture of it as it was fading out over it, read from the debug dump. Policy
//! has it at its new place from the start.
//!
//! An interrupted one hands over from what is on screen (CMN-176): each test reads the box
//! and the picture's opacity just before and just after the interruption, and at the end.

use std::time::Duration;

use caminito_test_client::format_input;
use caminito_test_client::layer_shell::{KeyboardInteractivity, Layer, LayerAnchor, LayerSpec};
use caminito_theme::ButtonFunction;
use smithay::utils::{Logical, Point, Rectangle};

use super::fixture::{
    BTN_LEFT, ClientId, ClientWindow, Decoration, Fixture, KEY_LEFTALT, Kind, WINDOW_COLOR,
    WindowSpec, on_both_kinds,
};
use super::open_close_animation::{Close, close, closing, opening};
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

/// Whether two opacities, read one just before an interruption and the other just after it,
/// are the same but for rounding.
fn near_alpha(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.01
}

/// How opaque each closing window's Maximise picture is drawn, from the debug dump.
fn faded_alphas(f: &Fixture) -> Vec<f64> {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    let Some(windows) = dump["closing_windows"].as_array() else {
        return Vec::new();
    };
    windows
        .iter()
        .filter_map(|window| window["faded_alpha"].as_f64())
        .collect()
}

/// A maximised window drawn at rest, and its frame before it was maximised.
fn maximised(f: &mut Fixture, id: ClientId) -> (ClientWindow, Rectangle<i32, Logical>) {
    let window = floating(f, id);
    let floating_frame = f.visible_frame(window);
    f.state.caminito.policy.set_maximized(f.pid(window), true);
    f.dispatch();
    let size = asked_size(f, window);
    f.draw_at_asked_size(window, size);
    f.advance_clock(LENGTH);
    (window, floating_frame)
}

on_both_kinds! {
    fn a_window_unmaximised_part_way_goes_back_from_where_it_is_drawn_for_a_fresh_250_ms(
        kind: Kind
    ) {
        let (mut f, id) = desk(kind, Vec::new());
        let window = floating(&mut f, id);
        let floating_frame = f.visible_frame(window);
        let pid = f.pid(window);
        f.state.caminito.policy.set_maximized(pid, true);
        f.dispatch();
        f.sync(window);
        f.advance_clock(PART_WAY);
        let (before, alpha) = drawn(&f, window);

        f.state.caminito.policy.set_maximized(pid, false);
        f.dispatch();
        f.sync(window);

        let (after, faded) = drawn(&f, window);
        assert!(near(before, after), "{before:?} then {after:?}");
        assert!(near_alpha(alpha, faded), "the first picture goes on: {alpha} then {faded}");
        f.advance_clock(PART_WAY);
        let (part_way, fainter) = drawn(&f, window);
        assert!(fainter < faded && fainter > 0.0, "it fades on: {fainter}");
        let expected = between(after, floating_frame, DONE_PART_WAY);
        assert!(near(part_way, expected), "{part_way:?}, not {expected:?}");
        // A fresh 250 ms from the interruption.
        f.advance_clock(LENGTH - PART_WAY - Duration::from_millis(1));
        drawn(&f, window);
        f.advance_clock(Duration::from_millis(1));
        assert!(maximizing(&f).is_empty(), "over: {:?}", maximizing(&f));
        assert_eq!(f.visible_frame(window), floating_frame);
    }
}

// Policy's own move and resize, which a client's request or a keyboard move ends in, the
// same for both kinds.
#[test]
fn a_window_moved_and_resized_part_way_goes_from_where_it_is_drawn_for_whats_left() {
    let (mut f, id) = desk(Kind::Xdg, Vec::new());
    let (window, floating_frame) = maximised(&mut f, id);
    let pid = f.pid(window);
    f.state.caminito.policy.set_maximized(pid, false);
    f.dispatch();
    f.sync(window);
    f.advance_clock(PART_WAY);
    let (before, alpha) = drawn(&f, window);

    let geometry = f.geometry(window);
    let loc = geometry.loc + Point::from((60, 40));
    let size = (geometry.size.w - 100, geometry.size.h - 80);
    f.state
        .caminito
        .policy
        .request_geometry(pid, Some(loc), Some(size.into()));
    f.dispatch();
    f.draw_at_asked_size(window, size);

    let (after, faded) = drawn(&f, window);
    assert!(near(before, after), "{before:?} then {after:?}");
    assert!(near_alpha(alpha, faded), "{alpha} then {faded}");
    let to = f.visible_frame(window);
    assert_ne!(to, floating_frame, "it went somewhere new");
    f.advance_clock(Duration::from_millis(50));
    let (part_way, _) = drawn(&f, window);
    let on_its_way = |a: i32, b: i32, c: i32| a.min(c) <= b && b <= a.max(c);
    assert!(
        on_its_way(after.loc.x, part_way.loc.x, to.loc.x)
            && on_its_way(after.loc.y, part_way.loc.y, to.loc.y)
            && on_its_way(after.size.w, part_way.size.w, to.size.w)
            && on_its_way(after.size.h, part_way.size.h, to.size.h),
        "towards its new frame: {after:?}, {part_way:?}, {to:?}"
    );
    // What's left of the 250 ms from the unmaximise.
    f.advance_clock(LENGTH - PART_WAY - Duration::from_millis(51));
    drawn(&f, window);
    f.advance_clock(Duration::from_millis(1));
    assert!(maximizing(&f).is_empty(), "over: {:?}", maximizing(&f));
    assert_eq!(f.visible_frame(window), to);
}

on_both_kinds! {
    fn a_window_closed_part_way_closes_from_its_box(kind: Kind) {
        let settings = vec![(CAMINITO, "open-close-animation".into(), Setting::Bool(true))];
        let (mut f, id) = desk(kind, settings);
        let window = floating(&mut f, id);
        f.advance_clock(crate::open_close_animation::LENGTH);
        let pid = f.pid(window);
        f.state.caminito.policy.set_maximized(pid, true);
        f.dispatch();
        f.advance_clock(PART_WAY);
        let (before, alpha) = drawn(&f, window);

        close(&mut f, window, Close::Destroy);

        assert!(maximizing(&f).is_empty(), "it ends: {:?}", maximizing(&f));
        let pictures = closing(&f);
        let [picture] = &pictures[..] else {
            panic!("it alone closes: {pictures:?}");
        };
        assert_eq!(picture.id, pid.0);
        assert!(near(before, picture.rect), "{before:?} then {:?}", picture.rect);
        // The live window was drawn whole under the fading picture: the closing's picture is
        // of it, and the fading one goes on over it.
        assert_eq!(picture.alpha, 1.0);
        let faded = faded_alphas(&f);
        assert!(
            matches!(faded[..], [faded] if near_alpha(alpha, faded)),
            "{alpha} then {faded:?}"
        );
        f.advance_clock(crate::open_close_animation::LENGTH / 2);
        let [fainter] = faded_alphas(&f)[..] else {
            panic!("{:?}", faded_alphas(&f));
        };
        assert!(fainter < alpha, "it fades on with the closing: {fainter}");
        f.advance_clock(crate::open_close_animation::LENGTH / 2);
        assert!(closing(&f).is_empty(), "gone: {:?}", closing(&f));
    }
}

on_both_kinds! {
    fn a_window_minimised_part_way_goes_to_its_button_from_its_real_place(kind: Kind) {
        let (mut f, id) = desk(kind, Vec::new());
        // A panel with the window's button, which the minimise animation goes to.
        let panel = f.connect(Kind::Xdg);
        f.client(panel).bind_outputs();
        f.client(panel).bind_foreign_toplevels();
        let bar = LayerSpec {
            layer: Layer::Top,
            namespace: "panel",
            output: None,
            anchor: LayerAnchor::Bottom | LayerAnchor::Left | LayerAnchor::Right,
            size: (0, 30),
            exclusive_zone: 30,
            keyboard: KeyboardInteractivity::None,
        };
        let bar = f.map_layer(panel, bar, 0xff00_00ff);
        let spec = WindowSpec { title: Some("Caja"), ..WindowSpec::default() };
        let window = f.map(id, spec);
        f.roundtrip(panel);
        let index = f.client(panel).foreign_toplevels().titled("Caja");
        f.client(panel).set_toplevel_rectangle_on_layer(index, bar, (300, 0, 100, 30));
        f.roundtrip(panel);
        let pid = f.pid(window);
        f.state.caminito.policy.set_maximized(pid, true);
        f.dispatch();
        f.sync(window);
        f.advance_clock(PART_WAY);
        drawn(&f, window);
        let real = f.visible_frame(window);

        f.state.caminito.policy.minimize(pid);
        f.dispatch();

        // The one-frame jump the spec accepts, as for the Opening animation (CMN-112).
        assert!(maximizing(&f).is_empty(), "it ends: {:?}", maximizing(&f));
        let boxes = |f: &Fixture| -> Vec<_> {
            let now = f.state.caminito.now();
            f.state.caminito.minimize_animations.boxes(now).collect()
        };
        assert_eq!(boxes(&f), vec![(pid, real)], "from its real place");
        f.advance_clock(crate::minimize_animation::LENGTH);
        assert!(boxes(&f).is_empty());
        assert!(maximizing(&f).is_empty(), "{:?}", maximizing(&f));
    }
}

on_both_kinds! {
    fn a_window_maximised_while_it_opens_is_grown_by_its_opening_alone(kind: Kind) {
        let settings = vec![(CAMINITO, "open-close-animation".into(), Setting::Bool(true))];
        let (mut f, id) = desk(kind, settings);
        let window = f.map(id, WindowSpec::default());
        let pid = f.pid(window);
        let opening_of = |f: &Fixture| {
            let opening = opening(f);
            match opening[..] {
                [(id, rect, alpha)] if id == pid.0 => (rect, alpha),
                _ => panic!("{window:?} alone opens: {opening:?}"),
            }
        };
        f.advance_clock(Duration::from_millis(50));
        let (before, alpha) = opening_of(&f);

        f.state.caminito.policy.set_maximized(pid, true);
        f.dispatch();
        f.sync(window);

        assert!(maximizing(&f).is_empty(), "none starts: {:?}", maximizing(&f));
        let (after, same) = opening_of(&f);
        assert!(near(before, after), "{before:?} then {after:?}");
        assert!(near_alpha(alpha, same), "{alpha} then {same}");
        f.advance_clock(crate::open_close_animation::LENGTH);
        assert!(opening(&f).is_empty(), "over: {:?}", opening(&f));
        assert!(maximizing(&f).is_empty(), "{:?}", maximizing(&f));
        assert_eq!(f.visible_frame(window).loc, Point::from((0, 0)), "maximised");
    }
}

on_both_kinds! {
    fn a_drag_part_way_ends_the_animation_and_the_window_follows_the_pointer(kind: Kind) {
        let (mut f, id) = desk(kind, Vec::new());
        let (window, floating_frame) = maximised(&mut f, id);
        let pid = f.pid(window);
        f.state.caminito.policy.set_maximized(pid, false);
        f.dispatch();
        f.sync(window);
        f.advance_clock(PART_WAY);
        drawn(&f, window);

        // An Alt+drag from inside its real place, which feeds policy's grab as a titlebar
        // drag does.
        let grip = (floating_frame.loc + Point::from((50, 50))).to_f64();
        f.pointer_move(grip.x, grip.y);
        f.key(KEY_LEFTALT, true);
        f.pointer_button(BTN_LEFT, true);
        f.pointer_move(grip.x + 100.0, grip.y + 50.0);
        f.sync(window);

        assert!(maximizing(&f).is_empty(), "it ends: {:?}", maximizing(&f));
        let moved = floating_frame.loc + Point::from((100, 50));
        assert_eq!(f.visible_frame(window).loc, moved, "under the pointer");
        f.pointer_button(BTN_LEFT, false);
        f.key(KEY_LEFTALT, false);
        f.advance_clock(LENGTH);
        assert_eq!(f.visible_frame(window).loc, moved);
    }
}

// The closing's picture is drawn scaled into the box it took over from, with a CSD
// window's shadow, which then reaches an output the window's own frame and shadow don't:
// that output is drawn at each frame too, or it flickers.
#[test]
fn an_output_the_picture_of_a_window_closed_part_way_reaches_is_drawn_at_every_frame() {
    let settings = vec![(CAMINITO, "open-close-animation".into(), Setting::Bool(true))];
    let (mut f, id) = desk(Kind::Xdg, settings);
    let left = f.state.caminito.outputs[0].clone();
    let right = f.add_output(OUTPUT, 1.0);
    f.set_output_position(&left, (0, 0));
    f.set_output_position(&right, (OUTPUT.0, 0));
    let spec = WindowSpec {
        decoration: Decoration::Client,
        ..WindowSpec::default()
    };
    let window = f.map(id, spec);
    f.advance_clock(crate::open_close_animation::LENGTH);
    let pid = f.pid(window);
    f.state
        .caminito
        .policy
        .request_geometry(pid, Some(Point::from((150, 120))), None);
    f.dispatch();
    draw_with_shadow(&mut f, window, SHADOW);
    f.state.caminito.policy.set_maximized(pid, true);
    f.dispatch();
    draw_with_shadow(&mut f, window, 0);
    f.advance_clock(LENGTH);
    f.state.caminito.policy.set_maximized(pid, false);
    f.dispatch();
    draw_with_shadow(&mut f, window, SHADOW);
    f.advance_clock(Duration::from_millis(10));
    let frame = f.visible_frame(window);
    assert!(frame.loc.x + frame.size.w + SHADOW < OUTPUT.0, "{frame:?}");

    close(&mut f, window, Close::Destroy);

    let now = f.state.caminito.now();
    assert!(f.state.caminito.open_close_animation_on(&right, now));
}

on_both_kinds! {
    fn a_titlebar_drag_that_pulls_it_off_part_way_ends_it_and_starts_none(kind: Kind) {
        let (mut f, id) = desk(kind, Vec::new());
        let window = floating(&mut f, id);
        let pid = f.pid(window);
        f.state.caminito.policy.set_maximized(pid, true);
        f.dispatch();
        f.sync(window);
        f.advance_clock(PART_WAY);
        drawn(&f, window);

        // On the titlebar at its real, maximised place, clear of the buttons.
        let close = f.button_rect(window, ButtonFunction::Close);
        let geometry = f.geometry(window);
        let title: Point<f64, Logical> = Point::from((
            f64::from(geometry.loc.x + geometry.size.w / 2),
            f64::from(close.loc.y + close.size.h / 2),
        ));
        f.pointer_move(title.x, title.y);
        f.pointer_button(BTN_LEFT, true);
        // Past the drag threshold, which pulls it off. Read in the pass that does, which
        // draws a frame.
        f.pointer_move(title.x + 50.0, title.y + 100.0);

        assert!(!f.state.caminito.policy.is_maximized(pid), "pulled off");
        assert!(maximizing(&f).is_empty(), "{:?}", maximizing(&f));
        f.pointer_move(title.x + 60.0, title.y + 110.0);
        f.sync(window);
        assert!(maximizing(&f).is_empty(), "{:?}", maximizing(&f));
        f.pointer_button(BTN_LEFT, false);
    }
}
