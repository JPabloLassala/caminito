//! The scale a window is asked to draw at across outputs of different scales (13, 52):
//! enter and leave for every output it overlaps, and one preferred scale, its primary
//! output's, as `preferred_buffer_scale` and `wp_fractional_scale_v1`. The outputs are a
//! 1.0 and a 2.0 one, left to right. Output events and preferred scales are Wayland surface
//! events, and Xwayland owns the surfaces of XWayland windows, so these map xdg toplevels
//! only.

use insta::assert_snapshot;
use smithay::output::Output;
use smithay::utils::Point;

use super::fixture::{BTN_LEFT, ClientId, ClientWindow, Fixture, KEY_LEFTALT, Kind, WindowSpec};

/// Camino with a 1920 × 1080 output at 1.0, then a 3840 × 2160 one at 2.0 to its right,
/// both 1920 × 1080 logical, and an xdg client that has bound them.
fn two_scales() -> (Fixture, ClientId, Output, Output) {
    let mut f = Fixture::new();
    let low = f.add_output((1920, 1080), 1.0);
    let high = f.add_output((3840, 2160), 2.0);
    let client = f.connect(Kind::Xdg);
    f.client(client).bind_outputs();
    f.roundtrip(client);
    (f, client, low, high)
}

/// A 400 × 300 window without a frame that asks for fractional scales.
fn spec() -> WindowSpec {
    WindowSpec {
        fractional_scale: true,
        ..WindowSpec::plain((400, 300))
    }
}

/// The window's output and scale events since `seen` of them were read.
fn events_since(f: &mut Fixture, window: ClientWindow, seen: &mut usize) -> String {
    let events = &f.client(window.client()).window(window.xdg()).output_events;
    let new = events[*seen..].join(", ");
    *seen = events.len();
    new
}

/// Alt+drags the window so its left edge goes through each of `xs` in turn, and says
/// what it was told at each.
fn drag_through(f: &mut Fixture, window: ClientWindow, xs: &[i32], seen: &mut usize) -> String {
    let start = f.geometry(window).loc;
    let grip = start + Point::from((10, 10));
    f.pointer_move(grip.x.into(), grip.y.into());
    f.key(KEY_LEFTALT, true);
    f.pointer_button(BTN_LEFT, true);
    let mut log = Vec::new();
    for &x in xs {
        f.pointer_move((grip.x + x - start.x).into(), grip.y.into());
        f.roundtrip(window.client());
        assert_eq!(f.geometry(window).loc.x, x);
        let events = events_since(f, window, seen);
        log.push(format!(
            "at {x}:{}{events}",
            if events.is_empty() { "" } else { " " }
        ));
    }
    f.pointer_button(BTN_LEFT, false);
    f.key(KEY_LEFTALT, false);
    log.join("\n")
}

/// The 400-pixel-wide window's left edge at 1640 leaves 120 × 300 of it on the 2.0 output,
/// 144 000 pixels there against 84 000 on the 1.0 one: less than twice. At 1660 it's
/// 168 000 against 78 000. Back the other way, the 1.0 output needs twice the 2.0 one's
/// pixels, which it never has while the window spans both.
#[test]
fn a_window_dragged_onto_a_denser_output_prefers_its_scale_past_twice_the_visible_area() {
    let (mut f, client, _, _) = two_scales();
    let window = f.map(client, spec());
    let mut seen = 0;
    let mapped = events_since(&mut f, window, &mut seen);

    let across = drag_through(&mut f, window, &[1400, 1640, 1660, 2200], &mut seen);
    let back = drag_through(&mut f, window, &[1660, 1400], &mut seen);

    assert_snapshot!(format!("mapped: {mapped}\n{across}\n{back}"), @"
    mapped: fractional scale 1, enter headless-1
    at 1400:
    at 1640: enter headless-2
    at 1660: buffer scale 2, fractional scale 2
    at 2200: leave headless-1
    at 1660: enter headless-1
    at 1400: leave headless-2, buffer scale 1, fractional scale 1
    ");
}

/// Policy places a new window on the focused window's output, or the first, so this one
/// maps on the 1.0 output and takes its scale once drawn there.
#[test]
fn a_new_window_first_prefers_the_scale_of_the_output_under_the_pointer() {
    let (mut f, client, _, _) = two_scales();
    f.pointer_move(2500., 500.);

    let window = f.map(client, spec());

    assert_snapshot!(
        f.client(client).window(window.xdg()).output_events.join("\n"),
        @"
    buffer scale 2
    fractional scale 2
    enter headless-1
    buffer scale 1
    fractional scale 1
    "
    );
}

#[test]
fn a_new_dialog_first_prefers_the_scale_of_its_parents_output() {
    let (mut f, client, _, _) = two_scales();
    let parent = f.map(client, spec());
    let mut seen = 0;
    drag_through(&mut f, parent, &[2200], &mut seen);
    f.pointer_move(100., 100.);

    let dialog = f.map(
        client,
        WindowSpec {
            parent: Some(parent),
            ..spec()
        },
    );

    assert_snapshot!(
        f.client(client).window(dialog.xdg()).output_events.join("\n"),
        @"
    buffer scale 2
    fractional scale 2
    enter headless-2
    "
    );
}

#[test]
fn changing_an_outputs_scale_sends_the_windows_on_it_their_new_preferred_scale() {
    let (mut f, client, low, _) = two_scales();
    let on_low = f.map(client, spec());
    let on_high = f.map(client, spec());
    let mut seen_high = 0;
    drag_through(&mut f, on_high, &[2200], &mut seen_high);
    let mut seen_low = f.client(client).window(on_low.xdg()).output_events.len();
    events_since(&mut f, on_high, &mut seen_high);

    f.set_output_scale(&low, 1.5);
    f.roundtrip(client);

    assert_snapshot!(
        format!(
            "on the rescaled output: {}\non the other: {}",
            events_since(&mut f, on_low, &mut seen_low),
            events_since(&mut f, on_high, &mut seen_high)
        ),
        @"
    on the rescaled output: buffer scale 2, fractional scale 1.5
    on the other:
    "
    );
}
