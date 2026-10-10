//! Edge and corner tiling (158) through a titlebar drag, as a user tiles a window: the tile
//! preview in the debug dump while the pointer is at an edge or a corner, the window tiled
//! there on release, what each kind of client is told of it, and a drag back out. Each
//! test that both kinds of window share runs on both (45); the xdg `tiled_*` states are
//! xdg-shell's, X's refused configure request X's, and the preview's pixels are the
//! Scene's, whatever the window under it.

use serde_json::{Value, json};
use smithay::output::Output;
use smithay::reexports::wayland_protocols::xdg::shell::client::xdg_toplevel;
use smithay::utils::{Logical, Point, Rectangle};

use super::fixture::WindowSpec;
use super::fixture::{BTN_LEFT, ClientWindow, Fixture, Kind, WINDOW_COLOR, on_both_kinds};
use crate::backend::headless::argb8888;
use crate::scene::CLEAR_COLOR;

/// The dump, parsed.
fn dump(f: &Fixture) -> Value {
    serde_json::from_str(&f.state.caminito.state_dump()).unwrap()
}

/// The dump's tile preview, or `null` while there's none.
fn preview(f: &Fixture) -> Value {
    dump(f).get("tile_preview").cloned().unwrap_or(Value::Null)
}

/// The dump's tile of `window`, or `null` while it isn't tiled.
fn tile(f: &Fixture, window: ClientWindow) -> Value {
    let id = f.pid(window).0;
    let dump = dump(f);
    let window = dump["windows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["id"] == json!(id))
        .expect("the dump has the window")
        .clone();
    window.get("tile").cloned().unwrap_or(Value::Null)
}

fn rect(x: i32, y: i32, width: i32, height: i32) -> Value {
    json!({ "x": x, "y": y, "width": width, "height": height })
}

fn as_rect(r: Rectangle<i32, Logical>) -> Value {
    rect(r.loc.x, r.loc.y, r.size.w, r.size.h)
}

/// A point on the window's titlebar, half way across.
fn title_point(f: &Fixture, window: ClientWindow) -> Point<f64, Logical> {
    let frame = f.visible_frame(window);
    let top = f.frame_extents(window).top;
    Point::from((
        f64::from(frame.loc.x + frame.size.w / 2),
        f64::from(frame.loc.y + top / 2),
    ))
}

/// Presses the window's titlebar and drags it past the drag threshold, so the window is
/// being moved.
fn grab_title(f: &mut Fixture, window: ClientWindow) {
    let title = title_point(f, window);
    f.pointer_move(title.x, title.y);
    f.pointer_button(BTN_LEFT, true);
    f.pointer_move(title.x + 20., title.y + 20.);
    f.sync(window);
}

/// Moves the dragging pointer to `x`,`y`.
fn drag_to(f: &mut Fixture, window: ClientWindow, x: f64, y: f64) {
    f.pointer_move(x, y);
    f.sync(window);
}

/// Lets go, and has the client draw at the size it was asked for, as a client does.
fn release(f: &mut Fixture, window: ClientWindow) {
    f.pointer_button(BTN_LEFT, false);
    f.sync(window);
    draw_as_asked(f, window);
}

/// Has the window's client draw at the size Caminito last asked of it.
fn draw_as_asked(f: &mut Fixture, window: ClientWindow) {
    if let ClientWindow::Xdg(client, xdg) = window {
        let size = f.client(client).window(xdg).configures.last().unwrap().size;
        f.draw_at_asked_size(window, size);
    }
}

on_both_kinds! {
    fn a_drag_to_each_edge_and_corner_shows_the_preview_and_tiles_on_release(kind: Kind) {
        let cases = [
            ((0., 500.), "left", rect(0, 0, 960, 1080)),
            ((1919., 500.), "right", rect(960, 0, 960, 1080)),
            ((0., 0.), "top-left", rect(0, 0, 960, 540)),
            ((1919., 0.), "top-right", rect(960, 0, 960, 540)),
            ((0., 1079.), "bottom-left", rect(0, 540, 960, 540)),
            ((1919., 1079.), "bottom-right", rect(960, 540, 960, 540)),
        ];
        for ((x, y), name, area) in cases {
            let (mut f, client) = Fixture::connected(kind);
            let window = f.map(client, WindowSpec::default());
            grab_title(&mut f, window);
            assert_eq!(preview(&f), Value::Null);

            drag_to(&mut f, window, x, y);
            assert_eq!(preview(&f), area, "the preview at {x},{y}");
            assert_eq!(tile(&f, window), Value::Null, "nothing tiles before the release");

            release(&mut f, window);
            assert_eq!(preview(&f), Value::Null);
            assert_eq!(tile(&f, window), json!(name));
            assert_eq!(as_rect(f.visible_frame(window)), area, "{name}");
        }
    }
}

on_both_kinds! {
    fn a_drag_to_the_top_maximises_and_one_back_away_from_an_edge_does_nothing(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let window = f.map(client, WindowSpec::default());
        grab_title(&mut f, window);

        drag_to(&mut f, window, 960., 0.);
        assert_eq!(preview(&f), rect(0, 0, 1920, 1080));
        drag_to(&mut f, window, 960., 400.);
        assert_eq!(preview(&f), Value::Null);
        drag_to(&mut f, window, 960., 0.);
        release(&mut f, window);

        assert!(f.state.caminito.policy.is_maximized(f.pid(window)));
        assert_eq!(tile(&f, window), Value::Null);
    }
}

on_both_kinds! {
    fn dragging_a_tiled_windows_titlebar_out_untiles_it_at_its_floating_size(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let window = f.map(client, WindowSpec::default());
        let floating = f.geometry(window).size;
        grab_title(&mut f, window);
        drag_to(&mut f, window, 0., 500.);
        release(&mut f, window);
        assert_eq!(tile(&f, window), json!("left"));

        grab_title(&mut f, window);
        draw_as_asked(&mut f, window);

        assert_eq!(tile(&f, window), Value::Null);
        assert_eq!(f.geometry(window).size, floating);
        let pointer = f.state.caminito.seat.get_pointer().unwrap().current_location();
        assert!(
            f.visible_frame(window).to_f64().contains(pointer),
            "the untiled window is under the pointer"
        );
        release(&mut f, window);
    }
}

/// xdg-shell's alone.
#[test]
fn a_tiled_xdg_toplevel_is_told_the_edges_it_lies_along() {
    let (mut f, client) = Fixture::connected(Kind::Xdg);
    let window = f.map(client, WindowSpec::default());
    let states = |f: &mut Fixture| {
        let configures = &f.client(client).window(window.xdg()).configures;
        let mut states = configures.last().unwrap().states.clone();
        states.retain(|state| {
            matches!(
                state,
                xdg_toplevel::State::TiledLeft
                    | xdg_toplevel::State::TiledRight
                    | xdg_toplevel::State::TiledTop
                    | xdg_toplevel::State::TiledBottom
            )
        });
        states.sort_by_key(|state| *state as u32);
        states
    };

    grab_title(&mut f, window);
    drag_to(&mut f, window, 0., 1079.);
    release(&mut f, window);
    assert_eq!(
        states(&mut f),
        [
            xdg_toplevel::State::TiledLeft,
            xdg_toplevel::State::TiledBottom
        ]
    );

    grab_title(&mut f, window);
    release(&mut f, window);
    assert_eq!(states(&mut f), []);
}

/// X's alone.
#[test]
fn an_x11_configure_request_is_refused_while_the_window_is_tiled() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let window = f.map(x, WindowSpec::default());
    grab_title(&mut f, window);
    drag_to(&mut f, window, 1919., 500.);
    release(&mut f, window);
    let tiled = f.geometry(window);

    let id = window.x11();
    f.x11_call(x, move |x| x.configure(id, (10, 10), (100, 100)))
        .unwrap();
    // Caminito has refused the request once the client is in sync, as Xwayland redirects it
    // to Caminito before it answers the client.
    f.sync(window);

    assert_eq!(f.geometry(window), tiled);
    assert_eq!(tile(&f, window), json!("right"));
}

/// `output`'s pixel at the global point `at`, as `0xRRGGBB`.
fn pixel(f: &mut Fixture, output: &Output, at: Point<i32, Logical>) -> u32 {
    let pixel = f
        .state
        .backend
        .headless_mut()
        .pixel(output, at.to_physical(1));
    pixel.expect("the output is drawn") & 0xff_ffff
}

/// The colour `over` shows through a fill of `color` at the preview's opacity, 64 of 255,
/// to within 1 a channel for the renderer's rounding.
#[track_caller]
fn assert_blend(actual: u32, color: u32, over: u32) {
    let channel = |c: u32, shift: u32| f64::from((c >> shift) & 0xff);
    for shift in [16, 8, 0] {
        let expected = channel(color, shift) * 64. / 255. + channel(over, shift) * 191. / 255.;
        let got = channel(actual, shift);
        assert!(
            (got - expected).abs() <= 1.,
            "{actual:06x} isn't {color:06x} over {over:06x} at 64/255"
        );
    }
}

/// The Scene's alone: the preview is drawn the same over any window.
#[test]
fn the_preview_is_drawn_translucent_inside_a_line_beneath_the_moved_window() {
    let (mut f, client) = Fixture::connected(Kind::Xdg);
    let output = f.state.caminito.outputs[0].clone();
    // The first window sits in the work area's top left corner, where the left tile's
    // preview covers it.
    let under = f.map(client, WindowSpec::default());
    let moved = f.map(client, WindowSpec::default());
    let under_spot = f.geometry(under).loc + Point::from((10, 10));
    let window = WINDOW_COLOR & 0xff_ffff;
    let clear = argb8888(CLEAR_COLOR) & 0xff_ffff;
    assert_eq!(pixel(&mut f, &output, under_spot), window);
    grab_title(&mut f, moved);

    drag_to(&mut f, moved, 0., 500.);
    f.dispatch();
    // The neutral palette selects in its background colour, so the text colour draws it.
    let line = 0x2e_3436;
    let spot = |f: &Fixture, at: Point<i32, Logical>| {
        assert!(
            !f.visible_frame(moved).contains(at),
            "{at:?} is clear of the moved window"
        );
        at
    };
    let edge = spot(&f, Point::from((959, 1000)));
    assert_eq!(pixel(&mut f, &output, edge), line, "its right edge");
    let inside = spot(&f, Point::from((900, 1000)));
    assert_blend(pixel(&mut f, &output, inside), line, clear);
    assert_blend(pixel(&mut f, &output, under_spot), line, window);
    let g = f.geometry(moved);
    let moved_spot = Point::from((g.loc.x + g.size.w - 10, g.loc.y + 10));
    assert!(
        moved_spot.x > 0 && moved_spot.x < 959,
        "{moved_spot:?} is over the preview"
    );
    assert_eq!(
        pixel(&mut f, &output, moved_spot),
        window,
        "the moved window is over it"
    );

    drag_to(&mut f, moved, 700., 500.);
    f.dispatch();
    assert_eq!(pixel(&mut f, &output, edge), clear, "it's gone");
    assert_eq!(pixel(&mut f, &output, under_spot), window);
    release(&mut f, moved);
}
