//! The Opening animation (251): a window that's shown grows from 0.8 of its size to its own
//! about its centre and fades in, for 200 ms of Caminito's clock, read from the debug dump
//! and from the output's pixels. Policy has it mapped from the start.
//!
//! The Closing animation (252): a window that its client destroys or hides leaves a picture
//! that shrinks to 0.8 of its size about its centre and fades out, at the window's place in
//! the stack. Policy lets the window go at once.

use std::time::Duration;

use caminito_test_client::format_input;
use caminito_test_client::layer_shell::{KeyboardInteractivity, Layer, LayerAnchor, LayerSpec};
use caminito_test_client::x11::XWindowSpec;
use smithay::utils::{Logical, Point, Rectangle};

use super::fixture::{
    BTN_LEFT, ClientId, ClientWindow, Decoration, Fixture, KEY_A, Kind, WINDOW_COLOR, WindowSpec,
    on_both_kinds,
};
use crate::backend::headless::argb8888;
use crate::open_close_animation::LENGTH;
use crate::scene::CLEAR_COLOR;
use crate::settings::Value as Setting;
use crate::settings::translate::{CAMINITO, INTERFACE, MARCO_GENERAL};
use crate::state::SEAT;
use crate::window::WindowKey;

/// The window's colour, as `0xRRGGBB`.
const WINDOW: u32 = WINDOW_COLOR & 0xff_ffff;

const OUTPUT: (i32, i32) = (800, 600);

/// How far into the animation the tests read it.
const PART_WAY: Duration = Duration::from_millis(50);

type Settings = Vec<(&'static str, String, Setting)>;

/// One output with `open-close-animation` on, unless `settings` has it, and a client of
/// `kind`. The fixture has the key off, for the tests written when a window was there at
/// once. The clock is stopped, so the animation is read where `advance_clock` has it.
fn desk(kind: Kind, mut settings: Settings) -> (Fixture, ClientId) {
    if !settings
        .iter()
        .any(|(_, key, _)| key == "open-close-animation")
    {
        settings.push((CAMINITO, "open-close-animation".into(), Setting::Bool(true)));
    }
    let mut f = Fixture::with_settings(settings);
    f.add_output(OUTPUT, 1.0);
    let id = f.connect(kind);
    f.stop_clock();
    (f, id)
}

/// Each window on its way open, by policy's id, with where its frame is drawn and its
/// opacity, from the debug dump's `opening_windows`, which isn't there while none is.
fn opening(f: &Fixture) -> Vec<(u64, Rectangle<i32, Logical>, f64)> {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    let int = |value: &serde_json::Value| value.as_i64().unwrap() as i32;
    let Some(windows) = dump["opening_windows"].as_array() else {
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

/// Where `window`'s frame is drawn and its opacity, while it's the one window on its way
/// open.
#[track_caller]
fn drawn(f: &Fixture, window: ClientWindow) -> (Rectangle<i32, Logical>, f64) {
    let opening = opening(f);
    match opening[..] {
        [(id, rect, alpha)] if id == f.pid(window).0 => (rect, alpha),
        _ => panic!("{window:?} alone opens: {opening:?}"),
    }
}

/// Whether `window` is on its way open.
fn opens(f: &Fixture, window: ClientWindow) -> bool {
    let pid = f.pid(window).0;
    opening(f).iter().any(|(id, _, _)| *id == pid)
}

/// Twice the middle of `rect`, which is then whole.
fn twice_middle(rect: Rectangle<i32, Logical>) -> Point<i32, Logical> {
    rect.loc + rect.loc + rect.size.to_point()
}

/// The middle of `rect`.
fn middle(rect: Rectangle<i32, Logical>) -> Point<i32, Logical> {
    rect.loc + Point::from((rect.size.w / 2, rect.size.h / 2))
}

/// Where nothing is drawn, as `0xRRGGBB`.
fn clear() -> u32 {
    argb8888(CLEAR_COLOR) & 0xff_ffff
}

/// The output's pixel at the global point `at`, as `0xRRGGBB`.
fn pixel(f: &mut Fixture, at: Point<i32, Logical>) -> u32 {
    let output = f.state.caminito.outputs[0].clone();
    let pixel = f
        .state
        .backend
        .headless_mut()
        .pixel(&output, at.to_physical(1));
    pixel.expect("the output is drawn") & 0xff_ffff
}

on_both_kinds! {
    fn a_window_grows_and_fades_in_about_its_centre_into_its_place(kind: Kind) {
        let (mut f, id) = desk(kind, Vec::new());
        let window = f.map(id, WindowSpec::default());
        let frame = f.visible_frame(window);
        let (start, alpha) = drawn(&f, window);
        assert_eq!(alpha, 0.0, "it starts with no opacity");
        let four_fifths = |n: i32| (f64::from(n) * 0.8).round() as i32;
        assert_eq!(start.size, (four_fifths(frame.size.w), four_fifths(frame.size.h)).into());

        f.advance_clock(PART_WAY);
        let (part_way, alpha) = drawn(&f, window);
        assert!(alpha > 0.0 && alpha < 1.0, "fainter part-way: {alpha}");
        assert!(
            part_way.size.w < frame.size.w && part_way.size.h < frame.size.h,
            "smaller part-way: {part_way:?} in {frame:?}"
        );
        assert!(start.size.w < part_way.size.w, "it grows: {start:?} to {part_way:?}");
        let (centre, its_own) = (twice_middle(part_way), twice_middle(frame));
        assert!(
            (centre.x - its_own.x).abs() <= 1 && (centre.y - its_own.y).abs() <= 1,
            "about its centre: {part_way:?} in {frame:?}"
        );
        assert!(frame.contains_rect(part_way), "{part_way:?} in {frame:?}");
        let faint = pixel(&mut f, middle(part_way));
        assert!(faint != WINDOW && faint != clear(), "it's drawn faint: {faint:#08x}");

        f.advance_clock(LENGTH - PART_WAY);
        assert!(opening(&f).is_empty(), "it's over at 200 ms");
        assert_eq!(f.visible_frame(window), frame, "at its own place");
        assert_eq!(pixel(&mut f, middle(frame)), WINDOW, "with full opacity");
    }
}

/// An X11 window of `types`, with its decoration as `decoration` asks.
fn typed(types: Vec<&'static str>, decoration: Decoration) -> WindowSpec {
    WindowSpec {
        types,
        decoration,
        ..WindowSpec::default()
    }
}

// A window's type and the windows outside policy are X11's alone, as is an unmap that a
// client can follow with a map of the same window.

#[test]
fn a_utility_window_with_a_frame_opens_and_a_menu_or_splash_screen_without_one_doesnt() {
    let (mut f, x) = desk(Kind::X11, Vec::new());
    let utility = f.map(
        x,
        typed(vec!["_NET_WM_WINDOW_TYPE_UTILITY"], Decoration::Server),
    );
    assert!(f.mapped(utility).shows_frame());
    assert!(opens(&f, utility), "a framed utility window opens");

    let menu = f.map(
        x,
        typed(vec!["_NET_WM_WINDOW_TYPE_MENU"], Decoration::Client),
    );
    let splash = f.map(
        x,
        typed(vec!["_NET_WM_WINDOW_TYPE_SPLASH"], Decoration::Server),
    );
    for window in [menu, splash] {
        assert!(!f.mapped(window).shows_frame(), "{window:?} has no frame");
        assert!(!opens(&f, window), "{window:?} is there at once");
    }
}

#[test]
fn an_override_redirect_window_and_a_layer_surface_are_there_at_once() {
    let (mut f, x) = desk(Kind::X11, Vec::new());
    f.map_override_redirect(x, (100, 100), (200, 150), 0xff00_ff00);
    let panel = f.connect(Kind::Xdg);
    f.client(panel).bind_outputs();
    let bar = LayerSpec {
        layer: Layer::Top,
        namespace: "panel",
        output: None,
        anchor: LayerAnchor::Bottom | LayerAnchor::Left | LayerAnchor::Right,
        size: (0, 30),
        exclusive_zone: 30,
        keyboard: KeyboardInteractivity::None,
    };
    f.map_layer(panel, bar, 0xff00_00ff);

    assert!(opening(&f).is_empty(), "{:?}", opening(&f));
    assert_eq!(pixel(&mut f, Point::from((150, 150))), 0x00_ff00);
    assert_eq!(pixel(&mut f, Point::from((400, OUTPUT.1 - 15))), 0x00_00ff);
}

#[test]
fn an_x11_window_unmapped_and_mapped_again_opens_again() {
    let (mut f, x) = desk(Kind::X11, Vec::new());
    let window = f.map(x, WindowSpec::default());
    f.advance_clock(LENGTH);
    assert!(opening(&f).is_empty());

    let id = window.x11();
    f.x11_call(x, move |x| x.unmap(id)).unwrap();
    f.dispatch_until(|f| f.policy_id(window).is_none());
    f.x11_call(x, move |x| x.map(id)).unwrap();
    f.dispatch_until(|f| f.policy_id(window).is_some());

    let (rect, alpha) = drawn(&f, window);
    assert_eq!(alpha, 0.0);
    assert!(rect.size.w < f.visible_frame(window).size.w, "{rect:?}");
}

#[test]
fn a_window_that_maps_on_a_workspace_no_output_shows_doesnt_open() {
    let (mut f, x) = desk(Kind::X11, Vec::new());
    let window = f.map(
        x,
        WindowSpec {
            cardinals: vec![("_NET_WM_DESKTOP", vec![2])],
            ..WindowSpec::default()
        },
    );
    assert!(!opens(&f, window), "nothing shows it");

    // Nor when a switch shows it after, which isn't its opening.
    f.state.caminito.policy.switch_workspace(2);
    f.dispatch();
    assert!(f.state.caminito.policy.is_showing(f.pid(window)));
    assert!(opening(&f).is_empty(), "{:?}", opening(&f));
}

// What a client is sent is read from a Wayland client, as `input.rs` reads it.
#[test]
fn keys_and_clicks_part_way_reach_the_new_window_at_its_real_place() {
    let (mut f, id) = desk(Kind::Xdg, Vec::new());
    f.client(id).bind_input();
    // With no frame, its corner is the frame's, which the animation draws well inside.
    let window = f.map(id, WindowSpec::plain((400, 300)));
    f.roundtrip(id);

    f.advance_clock(PART_WAY);
    assert!(f.has_keyboard(window), "it has the focus at once");
    f.key(KEY_A, true);
    f.key(KEY_A, false);
    f.roundtrip(id);
    let input = format_input(&f.client(id).state.input);
    assert!(input.contains("keyboard enters window 0"), "{input}");
    assert!(input.contains(&format!("key {KEY_A} pressed")), "{input}");

    let spot = f.geometry(window).loc + Point::from((5, 5));
    let (rect, _) = drawn(&f, window);
    assert!(!rect.contains(spot), "{spot:?} is outside {rect:?}");
    f.client(id).state.input.clear();
    f.pointer_move(f64::from(spot.x), f64::from(spot.y));
    f.pointer_button(BTN_LEFT, true);
    f.pointer_button(BTN_LEFT, false);
    f.roundtrip(id);
    let input = format_input(&f.client(id).state.input);
    assert!(input.contains("pointer enters window 0 at 5, 5"), "{input}");
    assert!(
        input.contains(&format!("button {BTN_LEFT:#x} pressed")),
        "{input}"
    );
}

// The settings are read the same for both kinds, which reach the one `map_pending`.

/// Maps a window and checks that the frame after it is the window at its place, whole.
fn assert_there_at_once(mut f: Fixture, id: ClientId) {
    let window = f.map(id, WindowSpec::default());

    assert!(opening(&f).is_empty(), "nothing opens: {:?}", opening(&f));
    let frame = f.visible_frame(window);
    assert_eq!(pixel(&mut f, middle(frame)), WINDOW);
}

#[test]
fn with_its_key_or_one_of_the_three_others_against_it_a_window_is_there_at_once() {
    for (schema, key, value) in [
        (CAMINITO, "open-close-animation", false),
        (MARCO_GENERAL, "compositing-manager", false),
        (MARCO_GENERAL, "reduced-resources", true),
        (INTERFACE, "enable-animations", false),
    ] {
        let settings = vec![(schema, key.into(), Setting::Bool(value))];
        let (f, id) = desk(Kind::Xdg, settings);

        assert_there_at_once(f, id);
    }
}

#[test]
fn the_key_turned_off_leaves_the_next_window_there_at_once() {
    let (mut f, id) = desk(Kind::Xdg, Vec::new());
    f.set_setting(CAMINITO, "open-close-animation", Setting::Bool(false));
    f.dispatch_until(|f| !f.state.caminito.settings.open_close_animation);

    assert_there_at_once(f, id);
}

/// Each closing window's picture, by the id its window had in policy, with where its frame
/// is drawn, its opacity, and the window it's drawn just over, from the debug dump's
/// `closing_windows`, which isn't there while none is.
type Closing = (u64, Rectangle<i32, Logical>, f64, Option<u64>);

fn closing(f: &Fixture) -> Vec<Closing> {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    let int = |value: &serde_json::Value| value.as_i64().unwrap() as i32;
    let Some(windows) = dump["closing_windows"].as_array() else {
        return Vec::new();
    };
    let windows = windows.iter().map(|window| {
        let rect = &window["rect"];
        let rect = Rectangle::new(
            (int(&rect["x"]), int(&rect["y"])).into(),
            (int(&rect["width"]), int(&rect["height"])).into(),
        );
        let alpha = window["alpha"].as_f64().unwrap();
        let over = window["over"].as_u64();
        (window["id"].as_u64().unwrap(), rect, alpha, over)
    });
    windows.collect()
}

/// Where the one closing window's picture is drawn, its opacity and what it's drawn over,
/// checking it's `id`'s.
#[track_caller]
fn drawn_closing(f: &Fixture, id: u64) -> (Rectangle<i32, Logical>, f64, Option<u64>) {
    let closing = closing(f);
    match closing[..] {
        [(closed, rect, alpha, over)] if closed == id => (rect, alpha, over),
        _ => panic!("window {id} alone closes: {closing:?}"),
    }
}

/// Maps a window and lets its opening finish.
fn map_opened(f: &mut Fixture, id: ClientId, spec: WindowSpec) -> ClientWindow {
    let window = f.map(id, spec);
    f.advance_clock(LENGTH);
    window
}

/// How a client closes a window.
#[derive(Debug, Clone, Copy)]
enum Close {
    /// It destroys it: the xdg toplevel and its surface, or the X window.
    Destroy,
    /// X11 only: it unmaps it, and may map it again.
    Unmap,
}

/// Has `window`'s client close it as `close` says, and waits until Caminito no longer
/// manages it.
fn close(f: &mut Fixture, window: ClientWindow, close: Close) {
    match (window, close) {
        (ClientWindow::Xdg(id, xdg), Close::Destroy) => {
            f.client(id).destroy_window(xdg);
            f.roundtrip(id);
        }
        (ClientWindow::X11(id, x11), Close::Destroy) => {
            f.x11_call(id, move |x| x.destroy(x11)).unwrap();
            f.dispatch_until(|f| f.policy_id(window).is_none());
        }
        (ClientWindow::X11(id, x11), Close::Unmap) => {
            f.x11_call(id, move |x| x.unmap(x11)).unwrap();
            f.dispatch_until(|f| f.policy_id(window).is_none());
        }
        (ClientWindow::Xdg(..), Close::Unmap) => panic!("an X11 unmap"),
    }
}

/// Checks that `window`, which was at `frame` with id `id`, shrinks about its centre and
/// fades out from the picture of it, and is gone at 200 ms, with nothing of it left where
/// it was.
fn assert_shrinks_and_fades_out(f: &mut Fixture, id: u64, frame: Rectangle<i32, Logical>) {
    let (start, alpha, _) = drawn_closing(f, id);
    assert_eq!(start, frame, "it starts at its own place");
    assert_eq!(alpha, 1.0, "with full opacity");
    assert_eq!(pixel(f, middle(frame)), WINDOW, "its last frame is drawn");

    f.advance_clock(PART_WAY);
    let (part_way, alpha, _) = drawn_closing(f, id);
    assert!(alpha > 0.0 && alpha < 1.0, "fainter part-way: {alpha}");
    assert!(
        part_way.size.w < frame.size.w && part_way.size.h < frame.size.h,
        "smaller part-way: {part_way:?} in {frame:?}"
    );
    let four_fifths = |n: i32| (f64::from(n) * 0.8).round() as i32;
    assert!(part_way.size.w > four_fifths(frame.size.w), "{part_way:?}");
    let (centre, its_own) = (twice_middle(part_way), twice_middle(frame));
    assert!(
        (centre.x - its_own.x).abs() <= 1 && (centre.y - its_own.y).abs() <= 1,
        "about its centre: {part_way:?} in {frame:?}"
    );
    let faint = pixel(f, middle(part_way));
    assert!(
        faint != WINDOW && faint != clear(),
        "it's drawn faint: {faint:#08x}"
    );

    f.advance_clock(LENGTH - PART_WAY - Duration::from_millis(1));
    let (end, alpha, _) = drawn_closing(f, id);
    assert!(alpha < 0.05, "all but gone at the end: {alpha}");
    assert!(
        (end.size.w - four_fifths(frame.size.w)).abs() <= 1,
        "0.8 of its size: {end:?}"
    );

    f.advance_clock(Duration::from_millis(1));
    assert!(
        closing(f).is_empty(),
        "it's over at 200 ms: {:?}",
        closing(f)
    );
    assert_eq!(pixel(f, middle(frame)), clear(), "nothing is left of it");
}

on_both_kinds! {
    fn a_destroyed_window_shrinks_and_fades_out_about_its_centre(kind: Kind) {
        let (mut f, id) = desk(kind, Vec::new());
        let window = map_opened(&mut f, id, WindowSpec::default());
        let frame = f.visible_frame(window);
        let pid = f.pid(window).0;

        close(&mut f, window, Close::Destroy);

        assert_shrinks_and_fades_out(&mut f, pid, frame);
    }
}

#[test]
fn an_xdg_window_its_client_hides_with_a_null_buffer_shrinks_and_fades_out() {
    let (mut f, id) = desk(Kind::Xdg, Vec::new());
    let window = map_opened(&mut f, id, WindowSpec::default());
    let frame = f.visible_frame(window);
    let pid = f.pid(window).0;

    f.client(id).commit_null_buffer(window.xdg());
    f.roundtrip(id);
    assert!(f.policy_id(window).is_none(), "it's unmapped");

    assert_shrinks_and_fades_out(&mut f, pid, frame);
}

#[test]
fn an_unmapped_x11_window_shrinks_and_fades_out_about_its_centre() {
    let (mut f, x) = desk(Kind::X11, Vec::new());
    let window = map_opened(&mut f, x, WindowSpec::default());
    let frame = f.visible_frame(window);
    let pid = f.pid(window).0;

    close(&mut f, window, Close::Unmap);

    assert_shrinks_and_fades_out(&mut f, pid, frame);
}

on_both_kinds! {
    fn the_focus_moves_on_in_the_first_frame_after_the_close(kind: Kind) {
        let (mut f, id) = desk(kind, Vec::new());
        let beneath = map_opened(&mut f, id, WindowSpec::default());
        let window = map_opened(&mut f, id, WindowSpec::default());
        assert!(f.has_keyboard(window));
        let pid = f.pid(window).0;

        close(&mut f, window, Close::Destroy);

        drawn_closing(&f, pid);
        let focus = f.state.caminito.policy.focus(SEAT);
        assert_eq!(focus, Some(f.pid(beneath)), "policy let it go at once");
        assert!(f.has_keyboard(beneath), "the window beneath has the keyboard");
    }
}

/// An X window of `client` at `position`, of `size`, filled with `color` (`0xRRGGBB`),
/// mapped and with its opening over.
fn map_colored(
    f: &mut Fixture,
    client: ClientId,
    position: (i16, i16),
    size: (u16, u16),
    color: u32,
) -> ClientWindow {
    let spec = XWindowSpec {
        size,
        position: Some(position),
        background: Some(0xff00_0000 | color),
        delete_window: true,
        ..XWindowSpec::default()
    };
    let window = f
        .x11_call(client, move |x| {
            let window = x.create_window(&spec)?;
            x.map(window)?;
            Ok::<_, x11rb::errors::ReplyOrIdError>(window)
        })
        .expect("the X window maps");
    let window = ClientWindow::X11(client, window);
    f.dispatch_until(|f| f.policy_id(window).is_some());
    f.sync(window);
    f.advance_clock(LENGTH);
    window
}

const BENEATH: u32 = 0xff_0000;
const ABOVE: u32 = 0x00_00ff;

// Placed with `USPosition` and filled with a colour of their own, which only an X window
// can be.
#[test]
fn part_way_a_window_above_stays_above_the_picture_and_a_click_on_it_reaches_the_window_beneath() {
    let (mut f, x) = desk(Kind::X11, Vec::new());
    let beneath = map_colored(&mut f, x, (50, 50), (500, 400), BENEATH);
    let window = map_opened(
        &mut f,
        x,
        WindowSpec {
            position: Some((150, 100)),
            ..WindowSpec::default()
        },
    );
    let above = map_colored(&mut f, x, (420, 300), (300, 200), ABOVE);
    let (frame, above_geometry) = (f.visible_frame(window), f.geometry(above));
    let pid = f.pid(window).0;

    close(&mut f, window, Close::Destroy);
    f.advance_clock(PART_WAY);

    let (drawn, _, over) = drawn_closing(&f, pid);
    assert_eq!(
        over,
        Some(f.pid(beneath).0),
        "it's drawn just over the window beneath"
    );
    let covered = drawn.intersection(above_geometry).unwrap();
    assert!(
        covered.size.w > 20 && covered.size.h > 20,
        "{drawn:?} {above_geometry:?}"
    );
    assert_eq!(
        pixel(&mut f, middle(covered)),
        ABOVE,
        "the window above is drawn over it"
    );
    // Where it's over the window beneath alone, that window shows through it, and a click
    // reaches it.
    let spot = Point::from((drawn.loc.x + 40, drawn.loc.y + drawn.size.h / 2));
    assert!(!above_geometry.contains(spot) && f.geometry(beneath).contains(spot));
    assert!(frame.contains(spot));
    let blend = pixel(&mut f, spot);
    assert!(
        blend != BENEATH && blend != WINDOW,
        "both show there: {blend:#08x}"
    );
    f.pointer_move(f64::from(spot.x), f64::from(spot.y));
    f.pointer_button(BTN_LEFT, true);
    f.pointer_button(BTN_LEFT, false);
    f.dispatch();
    let focus = f.state.caminito.policy.focus(SEAT);
    assert_eq!(
        focus,
        Some(f.pid(beneath)),
        "the click reached the window beneath"
    );
}

// A window's picture is kept by the close handlers and the surface's hooks, which run while
// its buffer is there. Caminito's own close, with none kept, is a close whose buffer went.
#[test]
fn a_window_with_no_picture_kept_closes_at_once() {
    let (mut f, id) = desk(Kind::Xdg, Vec::new());
    let window = map_opened(&mut f, id, WindowSpec::default());
    let frame = f.visible_frame(window);
    let surface = f.state.caminito.policy.window(f.pid(window)).unwrap();
    let key = WindowKey::of(surface.window());

    f.state.caminito.window_destroyed(&key);
    f.dispatch();

    assert!(f.policy_id(window).is_none());
    assert!(closing(&f).is_empty(), "nothing closes: {:?}", closing(&f));
    assert_eq!(
        pixel(&mut f, middle(frame)),
        clear(),
        "it's gone in one frame"
    );
}

#[test]
fn with_its_key_or_one_of_the_three_others_against_it_a_window_is_gone_in_one_frame() {
    for (schema, key, value) in [
        (CAMINITO, "open-close-animation", false),
        (MARCO_GENERAL, "compositing-manager", false),
        (MARCO_GENERAL, "reduced-resources", true),
        (INTERFACE, "enable-animations", false),
    ] {
        let settings = vec![(schema, key.into(), Setting::Bool(value))];
        let (mut f, id) = desk(Kind::Xdg, settings);
        let window = map_opened(&mut f, id, WindowSpec::default());
        let frame = f.visible_frame(window);

        close(&mut f, window, Close::Destroy);

        assert!(closing(&f).is_empty(), "{key}: {:?}", closing(&f));
        assert_eq!(
            pixel(&mut f, middle(frame)),
            clear(),
            "{key}: gone in one frame"
        );
    }
}
