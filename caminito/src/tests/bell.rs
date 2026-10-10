//! The one bell event (105): `gtk_shell1`, xdg-system-bell and XKB each ring it, for the
//! window the bell names or, when it names none, for the focused window, and the debug dump
//! shows how many have rung and the last one. `gtk_shell1` and xdg-system-bell are Wayland's,
//! and XKB's bell is X's, so each test maps only the kind of window its source is for.
//!
//! The visual bell (169) acts on it: a bell flashes every output, or the bell's window's
//! frame, for one flash on Caminito's clock, read from the outputs' pixels.

use serde_json::{Value, json};
use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle};

use super::fixture::{
    ClientId, ClientWindow, Fixture, Kind, WINDOW_COLOR, WindowSpec, on_both_kinds,
};
use crate::backend::headless::argb8888;
use crate::bell::FLASH;
use crate::output::output_geometry;
use crate::scene::CLEAR_COLOR;
use crate::settings::Value as Setting;
use crate::settings::translate::{MARCO_GENERAL, VisualBellType};

/// The dump's bell.
fn bell(f: &Fixture) -> Value {
    let dump: Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    dump["bell"].clone()
}

/// The window the dump has focused.
fn focus(f: &Fixture) -> Value {
    let dump: Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    dump["focus"].clone()
}

/// `count` bells rung, the last from `source` for `window`.
fn rung(count: u64, source: &str, window: Value) -> Value {
    json!({ "count": count, "last": { "source": source, "window": window } })
}

/// Two windows of one client of `kind`, the second focused, so a bell that took the focused
/// window where it should take another, or none, shows.
fn two_windows(kind: Kind) -> (Fixture, ClientId, ClientWindow, ClientWindow) {
    let (mut f, client) = Fixture::connected(kind);
    let first = f.map(client, WindowSpec::default());
    let second = f.map(client, WindowSpec::default());
    assert_eq!(focus(&f), json!(f.pid(second).0));
    (f, client, first, second)
}

#[test]
fn no_bell_has_rung_at_first() {
    let f = Fixture::new();

    assert_eq!(bell(&f), json!({ "count": 0, "last": null }));
}

#[test]
fn a_gtk_shell_bell_for_a_surface_is_for_its_window() {
    let (mut f, client, first, _) = two_windows(Kind::Xdg);

    f.client(client).system_bell(Some(first.xdg()));
    f.roundtrip(client);

    assert_eq!(bell(&f), rung(1, "gtk_shell1", json!(f.pid(first).0)));
}

#[test]
fn a_gtk_shell_bell_for_no_surface_is_for_the_focused_window() {
    let (mut f, client, _, second) = two_windows(Kind::Xdg);

    f.client(client).system_bell(None);
    f.roundtrip(client);

    assert_eq!(bell(&f), rung(1, "gtk_shell1", json!(f.pid(second).0)));
}

#[test]
fn an_xdg_system_bell_for_a_surface_is_for_its_window() {
    let (mut f, client, first, _) = two_windows(Kind::Xdg);

    f.client(client).ring_system_bell(Some(first.xdg()));
    f.roundtrip(client);

    assert_eq!(bell(&f), rung(1, "xdg_system_bell", json!(f.pid(first).0)));
}

#[test]
fn an_xdg_system_bell_for_no_surface_is_for_the_focused_window() {
    let (mut f, client, _, second) = two_windows(Kind::Xdg);

    f.client(client).ring_system_bell(None);
    f.roundtrip(client);

    assert_eq!(bell(&f), rung(1, "xdg_system_bell", json!(f.pid(second).0)));
}

#[test]
fn a_bell_for_a_destroyed_surface_is_for_no_window() {
    let (mut f, client, first, _) = two_windows(Kind::Xdg);
    // The first bell makes the window's `gtk_surface1`, which outlives its surface: GTK3
    // has no request to destroy one at version 1.
    f.client(client).system_bell(Some(first.xdg()));
    f.client(client).destroy_window(first.xdg());
    f.roundtrip(client);

    f.client(client).system_bell(Some(first.xdg()));
    f.roundtrip(client);

    assert_eq!(bell(&f), rung(2, "gtk_shell1", Value::Null));
}

#[test]
fn a_bell_for_a_surface_that_isnt_a_windows_is_for_no_window() {
    let (mut f, client, _, _) = two_windows(Kind::Xdg);
    let surface = f.client(client).create_bare_surface();

    f.client(client).ring_system_bell_for_surface(&surface);
    f.roundtrip(client);

    assert_eq!(bell(&f), rung(1, "xdg_system_bell", Value::Null));
}

#[test]
fn a_bell_for_a_windows_subsurface_is_for_the_window() {
    let (mut f, client, first, _) = two_windows(Kind::Xdg);
    let subsurface = f
        .client(client)
        .add_subsurface(first.xdg(), (10, 10), (20, 20), 0xff0000ff);
    f.roundtrip(client);

    f.client(client).ring_system_bell_for_surface(&subsurface);
    f.roundtrip(client);

    assert_eq!(bell(&f), rung(1, "xdg_system_bell", json!(f.pid(first).0)));
}

#[test]
fn an_x_bell_for_a_window_is_for_that_window() {
    let (mut f, client, first, _) = two_windows(Kind::X11);
    let window = first.x11();

    f.x11_call(client, move |c| c.bell(Some(window)).unwrap());
    f.dispatch_until(|f| bell(f)["count"] == 1);

    assert_eq!(bell(&f), rung(1, "xkb", json!(f.pid(first).0)));
}

#[test]
fn an_x_bell_for_no_window_is_for_the_focused_window() {
    let (mut f, client, _, second) = two_windows(Kind::X11);

    f.x11_call(client, |c| c.bell(None).unwrap());
    f.dispatch_until(|f| bell(f)["count"] == 1);

    assert_eq!(bell(&f), rung(1, "xkb", json!(f.pid(second).0)));
}

/// What a full-screen flash shows, as `0xRRGGBB`: white, as Marco fills its flash window.
const FLASH_WHITE: u32 = 0xff_ffff;

/// Where nothing is drawn, as `0xRRGGBB`.
fn clear() -> u32 {
    argb8888(CLEAR_COLOR) & 0xff_ffff
}

/// The window's colour, as `0xRRGGBB`.
const WINDOW: u32 = WINDOW_COLOR & 0xff_ffff;

/// The visual bell on, of `kind` (`fullscreen` or `frame_flash`).
fn visual_bell(kind: &str) -> Vec<(&'static str, String, Setting)> {
    vec![
        (MARCO_GENERAL, "visual-bell".into(), Setting::Bool(true)),
        (
            MARCO_GENERAL,
            "visual-bell-type".into(),
            Setting::Str(kind.into()),
        ),
    ]
}

/// Caminito on `settings`, with two 1920 × 1080 outputs side by side and a client of `kind`.
fn two_outputs(
    kind: Kind,
    settings: Vec<(&'static str, String, Setting)>,
) -> (Fixture, ClientId, Output, Output) {
    let mut f = Fixture::with_settings(settings);
    let first = f.add_output((1920, 1080), 1.0);
    let second = f.add_output((1920, 1080), 1.0);
    assert!(
        !output_geometry(&first).overlaps(output_geometry(&second)),
        "the outputs are side by side"
    );
    let client = f.connect(kind);
    (f, client, first, second)
}

/// Has `client` ring the bell for `window`, as its kind does.
fn ring(f: &mut Fixture, client: ClientId, window: ClientWindow) {
    match window {
        ClientWindow::Xdg(..) => {
            f.client(client).ring_system_bell(Some(window.xdg()));
            f.roundtrip(client);
        }
        ClientWindow::X11(..) => {
            let x = window.x11();
            f.x11_call(client, move |c| c.bell(Some(x)).unwrap());
        }
    }
}

/// `output`'s pixel at the global point `at`, as `0xRRGGBB`.
fn pixel(f: &mut Fixture, output: &Output, at: Point<i32, Logical>) -> u32 {
    let local = at - output_geometry(output).loc;
    let pixel = f
        .state
        .backend
        .headless_mut()
        .pixel(output, local.to_physical(1));
    pixel.expect("the output is drawn") & 0xff_ffff
}

/// Dispatches until `output` shows `color` at the global point `at`.
#[track_caller]
fn wait_for(f: &mut Fixture, output: &Output, at: Point<i32, Logical>, color: u32) {
    let local = at - output_geometry(output).loc;
    f.wait_for_pixel_on(output, local.x, local.y, color);
}

/// A spot of `output` that no window covers: its bottom right corner.
fn empty_spot(f: &Fixture, output: &Output, windows: &[ClientWindow]) -> Point<i32, Logical> {
    let geometry = output_geometry(output);
    let spot = geometry.loc + Point::from((geometry.size.w - 5, geometry.size.h - 5));
    for &window in windows {
        assert!(!f.visible_frame(window).contains(spot), "{spot:?} is empty");
    }
    spot
}

/// The middle of `rect`.
fn middle(rect: Rectangle<i32, Logical>) -> Point<i32, Logical> {
    rect.loc + Point::from((rect.size.w / 2, rect.size.h / 2))
}

/// The output that holds `at`.
fn output_at(outputs: [&Output; 2], at: Point<i32, Logical>) -> Output {
    outputs
        .into_iter()
        .find(|output| output_geometry(output).contains(at))
        .expect("the point is on an output")
        .clone()
}

on_both_kinds! {
    fn a_fullscreen_bell_flashes_every_output_for_one_flash(kind: Kind) {
        let (mut f, client, first, second) = two_outputs(kind, visual_bell("fullscreen"));
        let window = f.map(client, WindowSpec::default());
        let inside = middle(f.geometry(window));
        let on = output_at([&first, &second], inside);
        let spots = [
            (first.clone(), empty_spot(&f, &first, &[window])),
            (second.clone(), empty_spot(&f, &second, &[window])),
        ];
        assert_eq!(pixel(&mut f, &on, inside), WINDOW);

        ring(&mut f, client, window);

        wait_for(&mut f, &on, inside, FLASH_WHITE);
        for (output, spot) in &spots {
            assert_eq!(pixel(&mut f, output, *spot), FLASH_WHITE, "{spot:?} flashes");
        }
        f.advance_clock(FLASH);
        assert_eq!(pixel(&mut f, &on, inside), WINDOW);
        for (output, spot) in &spots {
            assert_eq!(pixel(&mut f, output, *spot), clear(), "{spot:?} is over");
        }
    }
}

/// The middle of the top edge of `window`'s frame, globally, where the test theme draws
/// the frame's outline darker while it's focused.
fn frame_edge(f: &Fixture, window: ClientWindow) -> Point<i32, Logical> {
    let frame = f.visible_frame(window);
    frame.loc + Point::from((frame.size.w / 2, 0))
}

on_both_kinds! {
    fn a_frame_flash_bell_flashes_its_windows_frame_alone_for_one_flash(kind: Kind) {
        let (mut f, client, first, second) = two_outputs(kind, visual_bell("frame_flash"));
        let unfocused = f.map(client, WindowSpec::default());
        let focused = f.map(client, WindowSpec::default());
        let (bell_spot, focused_spot) = (frame_edge(&f, unfocused), frame_edge(&f, focused));
        assert!(!f.visible_frame(focused).contains(bell_spot), "the frame's edge shows");
        let on = output_at([&first, &second], bell_spot);
        let focused_on = output_at([&first, &second], focused_spot);
        let empty = empty_spot(&f, &on, &[unfocused, focused]);
        let before = pixel(&mut f, &on, bell_spot);
        let focused_look = pixel(&mut f, &focused_on, focused_spot);
        assert_ne!(before, focused_look, "a focused frame looks unlike an unfocused one");

        ring(&mut f, client, unfocused);

        // Marco flashes a frame by drawing it in the other focus state.
        wait_for(&mut f, &on, bell_spot, focused_look);
        assert_eq!(pixel(&mut f, &on, empty), clear(), "the output doesn't flash");
        assert_eq!(pixel(&mut f, &focused_on, focused_spot), focused_look);
        f.advance_clock(FLASH);
        assert_eq!(pixel(&mut f, &on, bell_spot), before);
    }
}

// Where `frame_flash` finds no frame, the output of the window, or of the pointer, flashes,
// as Marco flashes the screen then (`bell.c:bell_visual_notify`). What the bell is for
// doesn't depend on its source, so these ring xdg-system-bell alone.

#[test]
fn a_frame_flash_bell_for_a_window_with_no_frame_flashes_its_output() {
    let (mut f, client, first, second) = two_outputs(Kind::Xdg, visual_bell("frame_flash"));
    let window = f.map(client, WindowSpec::plain((400, 300)));
    let inside = middle(f.geometry(window));
    let (on, off) = if output_geometry(&first).contains(inside) {
        (first, second)
    } else {
        (second, first)
    };
    let off_spot = empty_spot(&f, &off, &[window]);

    ring(&mut f, client, window);

    wait_for(&mut f, &on, inside, FLASH_WHITE);
    assert_eq!(
        pixel(&mut f, &off, off_spot),
        clear(),
        "the other output doesn't flash"
    );
    f.advance_clock(FLASH);
    assert_eq!(pixel(&mut f, &on, inside), WINDOW);
}

#[test]
fn a_frame_flash_bell_for_no_window_flashes_the_output_the_pointer_is_on() {
    let (mut f, client, first, second) = two_outputs(Kind::Xdg, visual_bell("frame_flash"));
    let surface = f.client(client).create_bare_surface();
    let target = middle(output_geometry(&second));
    f.pointer_move(target.x.into(), target.y.into());
    let first_spot = empty_spot(&f, &first, &[]);
    let second_spot = empty_spot(&f, &second, &[]);

    f.client(client).ring_system_bell_for_surface(&surface);
    f.roundtrip(client);

    wait_for(&mut f, &second, second_spot, FLASH_WHITE);
    assert_eq!(
        pixel(&mut f, &first, first_spot),
        clear(),
        "the other output doesn't flash"
    );
    f.advance_clock(FLASH);
    assert_eq!(pixel(&mut f, &second, second_spot), clear());
}

on_both_kinds! {
    fn no_bell_flashes_with_the_visual_bell_off(kind: Kind) {
        let settings = vec![
            (MARCO_GENERAL, "visual-bell".into(), Setting::Bool(false)),
            (MARCO_GENERAL, "visual-bell-type".into(), Setting::Str("frame_flash".into())),
        ];
        let (mut f, client, first, second) = two_outputs(kind, settings);
        let unfocused = f.map(client, WindowSpec::default());
        let _focused = f.map(client, WindowSpec::default());
        let edge = frame_edge(&f, unfocused);
        let on = output_at([&first, &second], edge);
        let empty = empty_spot(&f, &on, &[unfocused, _focused]);
        let before = pixel(&mut f, &on, edge);

        ring(&mut f, client, unfocused);
        f.dispatch_until(|f| bell(f)["count"] == 1);

        assert_eq!(pixel(&mut f, &on, edge), before, "the frame doesn't flash");
        assert_eq!(pixel(&mut f, &on, empty), clear(), "the output doesn't flash");
    }
}

/// The keys are read when a bell rings, whatever its source, so this rings
/// xdg-system-bell alone.
#[test]
fn a_change_of_the_visual_bell_keys_applies_to_the_next_bell() {
    let (mut f, client, first, _) = two_outputs(Kind::Xdg, visual_bell("fullscreen"));
    let window = f.map(client, WindowSpec::default());
    let edge = frame_edge(&f, window);
    let empty = empty_spot(&f, &first, &[window]);
    let unfocused_look = unfocused_look(&mut f, client, &first, window);
    let focused_look = pixel(&mut f, &first, edge);
    assert_ne!(focused_look, unfocused_look);
    ring(&mut f, client, window);
    wait_for(&mut f, &first, empty, FLASH_WHITE);
    f.advance_clock(FLASH);

    f.set_setting(
        MARCO_GENERAL,
        "visual-bell-type",
        Setting::Str("frame_flash".into()),
    );
    f.dispatch_until(|f| f.state.caminito.settings.bell.visual_type == VisualBellType::FrameFlash);
    assert_eq!(
        pixel(&mut f, &first, empty),
        clear(),
        "a change flashes nothing itself"
    );
    ring(&mut f, client, window);

    wait_for(&mut f, &first, edge, unfocused_look);
    assert_eq!(
        pixel(&mut f, &first, empty),
        clear(),
        "the output doesn't flash"
    );
    f.advance_clock(FLASH);
    assert_eq!(pixel(&mut f, &first, edge), focused_look);

    f.set_setting(MARCO_GENERAL, "visual-bell", Setting::Bool(false));
    f.dispatch_until(|f| !f.state.caminito.settings.bell.visual);
    ring(&mut f, client, window);
    f.dispatch_until(|f| bell(f)["count"] == 3);

    assert_eq!(
        pixel(&mut f, &first, edge),
        focused_look,
        "the frame doesn't flash"
    );
}

/// How the edge of `window`'s frame looks unfocused: as a second window's, mapped over
/// `window` and gone again, left it while it had the focus.
fn unfocused_look(f: &mut Fixture, client: ClientId, output: &Output, window: ClientWindow) -> u32 {
    let edge = frame_edge(f, window);
    let other = f.map(client, WindowSpec::plain((10, 10)));
    let look = pixel(f, output, edge);
    f.client(client).destroy_window(other.xdg());
    f.roundtrip(client);
    look
}

#[test]
fn a_second_bell_during_a_flash_starts_the_flash_again() {
    let (mut f, client, first, _) = two_outputs(Kind::Xdg, visual_bell("fullscreen"));
    let window = f.map(client, WindowSpec::default());
    let empty = empty_spot(&f, &first, &[window]);
    ring(&mut f, client, window);
    wait_for(&mut f, &first, empty, FLASH_WHITE);
    f.advance_clock(FLASH / 2);

    ring(&mut f, client, window);
    f.dispatch_until(|f| bell(f)["count"] == 2);
    // Past the first bell's flash, and inside the second's: the clock also moves as the
    // test runs, which the 40 ms left of the second's flash allows for.
    f.advance_clock(FLASH * 3 / 5);

    assert_eq!(pixel(&mut f, &first, empty), FLASH_WHITE);
    f.advance_clock(FLASH);
    assert_eq!(pixel(&mut f, &first, empty), clear());
}
