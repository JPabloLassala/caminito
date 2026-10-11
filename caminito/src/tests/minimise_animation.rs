//! The minimise animation (172): a window that's minimised shrinks into the rectangle a
//! window list set for it (65), drawn from its last content for 250 ms of Caminito's clock,
//! read from the output's pixels. Policy has it minimised from the start.

use std::time::Duration;

use caminito_test_client::layer_shell::{
    KeyboardInteractivity, Layer, LayerAnchor, LayerId, LayerSpec,
};
use caminito_theme::ButtonFunction;
use smithay::utils::{Logical, Point, Rectangle};

use super::fixture::{
    BTN_LEFT, ClientId, ClientWindow, Fixture, KEY_F9, KEY_LEFTALT, Kind, WINDOW_COLOR, WindowSpec,
    on_both_kinds,
};
use crate::backend::headless::argb8888;
use crate::minimize_animation::LENGTH;
use crate::scene::CLEAR_COLOR;
use crate::settings::Value as Setting;
use crate::settings::translate::{INTERFACE, MARCO_GENERAL};
use crate::state::SEAT;

/// The window's colour, as `0xRRGGBB`.
const WINDOW: u32 = WINDOW_COLOR & 0xff_ffff;

/// The panel's bar, in a colour that's neither the window's nor the clear colour's.
const BAR_COLOR: u32 = 0xff00_00ff;
const BAR_HEIGHT: i32 = 30;

/// The window's button on the bar, from the bar's top left corner.
const BUTTON: (i32, i32, i32, i32) = (300, 0, 100, BAR_HEIGHT);

/// The output, small enough that the window's way to its button is short beside its size:
/// the spot the tests read stays inside the shrinking window from 88 to 172 ms, so a slow
/// run between the minimise and the frame doesn't fail them.
const OUTPUT: (i32, i32) = (800, 600);

/// How far into the animation the tests read it.
const PART_WAY: Duration = Duration::from_millis(100);

/// One output with a panel's bar along its bottom, a framed window of a client of `kind`,
/// which has the focus, and the panel's index for it.
struct Desk {
    f: Fixture,
    window: ClientWindow,
    panel: ClientId,
    bar: LayerId,
    index: usize,
}

fn desk(kind: Kind, settings: Vec<(&'static str, String, Setting)>) -> Desk {
    let mut f = Fixture::with_settings(settings);
    f.add_output(OUTPUT, 1.0);
    let panel = f.connect(Kind::Xdg);
    f.client(panel).bind_outputs();
    f.client(panel).bind_foreign_toplevels();
    let bar = LayerSpec {
        layer: Layer::Top,
        namespace: "panel",
        output: None,
        anchor: LayerAnchor::Bottom | LayerAnchor::Left | LayerAnchor::Right,
        size: (0, BAR_HEIGHT as u32),
        exclusive_zone: BAR_HEIGHT,
        keyboard: KeyboardInteractivity::None,
    };
    let bar = f.map_layer(panel, bar, BAR_COLOR);
    let client = f.connect(kind);
    let window = f.map(
        client,
        WindowSpec {
            title: Some("Caja"),
            ..WindowSpec::default()
        },
    );
    f.roundtrip(panel);
    let index = f.client(panel).foreign_toplevels().titled("Caja");
    Desk {
        f,
        window,
        panel,
        bar,
        index,
    }
}

/// A desk whose panel has said where the window's button is.
fn desk_with_button(kind: Kind, settings: Vec<(&'static str, String, Setting)>) -> Desk {
    let mut desk = desk(kind, settings);
    desk.f
        .client(desk.panel)
        .set_toplevel_rectangle_on_layer(desk.index, desk.bar, BUTTON);
    desk.f.roundtrip(desk.panel);
    desk
}

/// Where the window's button is, globally.
fn button() -> Rectangle<i32, Logical> {
    let (x, y, width, height) = BUTTON;
    Rectangle::new(
        (x, OUTPUT.1 - BAR_HEIGHT + y).into(),
        (width, height).into(),
    )
}

/// The middle of `rect`.
fn middle(rect: Rectangle<i32, Logical>) -> Point<i32, Logical> {
    rect.loc + Point::from((rect.size.w / 2, rect.size.h / 2))
}

/// A spot the shrinking window covers part-way from its frame to `to`: the middle of where
/// it is a little past half way.
fn spot_on_the_way(desk: &Desk, to: Rectangle<i32, Logical>) -> Point<i32, Logical> {
    let frame = desk.f.visible_frame(desk.window);
    let at = |from: i32, to: i32| from + (to - from) * 55 / 100;
    middle(Rectangle::new(
        (at(frame.loc.x, to.loc.x), at(frame.loc.y, to.loc.y)).into(),
        (at(frame.size.w, to.size.w), at(frame.size.h, to.size.h)).into(),
    ))
}

/// A spot the shrinking window covers part-way from its frame to `to`, and that neither
/// the window in its place nor `to` does.
fn spot_between(desk: &Desk, to: Rectangle<i32, Logical>) -> Point<i32, Logical> {
    let frame = desk.f.visible_frame(desk.window);
    let spot = spot_on_the_way(desk, to);
    assert!(!frame.contains(spot), "{spot:?} is clear of {frame:?}");
    assert!(!to.contains(spot), "{spot:?} is clear of {to:?}");
    spot
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

/// Where nothing is drawn, as `0xRRGGBB`.
fn clear() -> u32 {
    argb8888(CLEAR_COLOR) & 0xff_ffff
}

fn is_minimized(f: &Fixture, window: ClientWindow) -> bool {
    f.state.caminito.policy.is_minimized(f.pid(window))
}

/// The ways a window is minimised.
#[derive(Debug, Clone, Copy)]
enum Way {
    /// Alt+F9, Marco's default binding.
    Binding,
    /// The Frame's minimise button.
    FrameButton,
    /// The panel's window list.
    WindowList,
    /// The window's own client.
    Client,
}

/// Minimises the desk's window, and returns once policy has it minimised.
fn minimise(desk: &mut Desk, way: Way) {
    let Desk {
        f,
        window,
        panel,
        index,
        ..
    } = desk;
    let window = *window;
    match way {
        Way::Binding => f.chord(&[KEY_LEFTALT, KEY_F9]),
        Way::FrameButton => {
            let button = f.button_rect(window, ButtonFunction::Minimize).to_f64();
            let center = button.loc + button.size.downscale(2.).to_point();
            f.pointer_move(center.x, center.y);
            f.pointer_button(BTN_LEFT, true);
            f.pointer_button(BTN_LEFT, false);
        }
        Way::WindowList => {
            f.client(*panel).set_toplevel_minimized(*index, true);
            f.roundtrip(*panel);
        }
        Way::Client => match window {
            ClientWindow::Xdg(client, xdg) => {
                f.client(client).set_minimized(xdg);
                f.roundtrip(client);
            }
            ClientWindow::X11(client, x) => {
                // ICCCM's `IconicState`.
                f.x11_call(client, move |c| {
                    c.send_client_message(x, "WM_CHANGE_STATE", [3, 0, 0, 0, 0])
                })
                .unwrap();
            }
        },
    }
    f.dispatch_until(|f| is_minimized(f, window));
}

on_both_kinds! {
    fn a_minimised_window_shrinks_into_its_buttons_rectangle(kind: Kind) {
        let mut desk = desk_with_button(kind, Vec::new());
        let inside = middle(desk.f.geometry(desk.window));
        let between = spot_between(&desk, button());
        let pid = desk.f.pid(desk.window);
        assert_eq!(pixel(&mut desk.f, inside), WINDOW);
        assert_eq!(pixel(&mut desk.f, between), clear());

        minimise(&mut desk, Way::Binding);
        // Policy has it minimised at once, so the focus has moved on: only the drawing
        // takes its time.
        assert_ne!(desk.f.state.caminito.policy.focus(SEAT), Some(pid));

        desk.f.advance_clock(PART_WAY);
        assert_eq!(pixel(&mut desk.f, between), WINDOW, "it's on its way");
        assert_eq!(pixel(&mut desk.f, inside), clear(), "it has left its place");

        desk.f.advance_clock(LENGTH - PART_WAY);
        assert_eq!(pixel(&mut desk.f, between), clear(), "it's gone");
        assert_eq!(pixel(&mut desk.f, inside), clear());
        assert_eq!(pixel(&mut desk.f, middle(button())), BAR_COLOR & 0xff_ffff);
    }
}

on_both_kinds! {
    fn every_way_of_minimising_a_window_animates_it(kind: Kind) {
        for way in [Way::Binding, Way::FrameButton, Way::WindowList, Way::Client] {
            let mut desk = desk_with_button(kind, Vec::new());
            let between = spot_between(&desk, button());

            minimise(&mut desk, way);
            desk.f.advance_clock(PART_WAY);

            assert_eq!(pixel(&mut desk.f, between), WINDOW, "{way:?}");
        }
    }
}

/// Minimises the desk's window and checks that nothing of it is drawn from then on.
fn assert_no_animation(mut desk: Desk) {
    let inside = middle(desk.f.geometry(desk.window));
    let between = spot_between(&desk, button());

    minimise(&mut desk, Way::Binding);
    desk.f.dispatch();
    assert_eq!(pixel(&mut desk.f, inside), clear(), "it's gone at once");
    assert_eq!(pixel(&mut desk.f, between), clear());

    desk.f.advance_clock(PART_WAY);
    assert_eq!(
        pixel(&mut desk.f, between),
        clear(),
        "nothing is on its way"
    );
}

/// A desk with a button for its window, under a Caminito with `key` of `schema` at `value`.
fn desk_with_key(kind: Kind, schema: &'static str, key: &str, value: bool) -> Desk {
    desk_with_button(kind, vec![(schema, key.into(), Setting::Bool(value))])
}

on_both_kinds! {
    fn compositing_manager_off_turns_the_animation_off(kind: Kind) {
        assert_no_animation(desk_with_key(kind, MARCO_GENERAL, "compositing-manager", false));
    }
}

on_both_kinds! {
    fn reduced_resources_turns_the_animation_off(kind: Kind) {
        assert_no_animation(desk_with_key(kind, MARCO_GENERAL, "reduced-resources", true));
    }
}

on_both_kinds! {
    fn enable_animations_off_turns_the_animation_off(kind: Kind) {
        assert_no_animation(desk_with_key(kind, INTERFACE, "enable-animations", false));
    }
}

on_both_kinds! {
    fn a_window_with_no_rectangle_isnt_animated(kind: Kind) {
        assert_no_animation(desk(kind, Vec::new()));
    }
}

on_both_kinds! {
    fn a_window_whose_rectangle_was_taken_back_isnt_animated(kind: Kind) {
        let mut desk = desk_with_button(kind, Vec::new());
        desk.f
            .client(desk.panel)
            .set_toplevel_rectangle_on_layer(desk.index, desk.bar, (0, 0, 0, 0));
        desk.f.roundtrip(desk.panel);

        assert_no_animation(desk);
    }
}

on_both_kinds! {
    fn a_window_restored_during_the_animation_is_back_in_its_place_alone(kind: Kind) {
        let mut desk = desk_with_button(kind, Vec::new());
        let inside = middle(desk.f.geometry(desk.window));
        let between = spot_between(&desk, button());
        minimise(&mut desk, Way::Binding);
        desk.f.advance_clock(PART_WAY);
        assert_eq!(pixel(&mut desk.f, between), WINDOW);

        desk.f.client(desk.panel).set_toplevel_minimized(desk.index, false);
        desk.f.roundtrip(desk.panel);
        desk.f.wait_for_pixel(inside.x, inside.y, WINDOW);

        assert!(!is_minimized(&desk.f, desk.window));
        assert_eq!(pixel(&mut desk.f, between), clear(), "the animation is over");
    }
}

on_both_kinds! {
    fn a_setting_turned_off_stops_the_next_animation(kind: Kind) {
        let mut desk = desk_with_button(kind, Vec::new());
        desk.f
            .set_setting(MARCO_GENERAL, "reduced-resources", Setting::Bool(true));
        desk.f
            .dispatch_until(|f| !f.state.caminito.settings.minimize_animation);

        assert_no_animation(desk);
    }
}

// The rectangle is on any surface of the window list's client (65). The tests above have
// it on a layer surface, as mate-panel's is; this one has it on a window of the panel's,
// which only a Wayland client has, so the animated window is the only one of both kinds.
on_both_kinds! {
    fn a_rectangle_on_a_windows_surface_is_animated_to(kind: Kind) {
        let mut desk = desk(kind, Vec::new());
        let list = desk.f.map(desk.panel, WindowSpec::plain((200, 40)));
        let (x, y, width, height) = (150, 0, 50, 40);
        desk.f
            .client(desk.panel)
            .set_toplevel_rectangle(desk.index, list.xdg(), (x, y, width, height));
        desk.f.roundtrip(desk.panel);
        let origin = desk.f.geometry(list).loc;
        let to = Rectangle::new(origin + Point::from((x, y)), (width, height).into());
        // The list is near the window, so the spot is one the window covers in its place
        // too: once it's minimised, only the animation draws it there.
        let on_the_way = spot_on_the_way(&desk, to);
        assert!(!desk.f.geometry(list).contains(on_the_way));

        minimise(&mut desk, Way::WindowList);
        desk.f.advance_clock(PART_WAY);
        assert_eq!(pixel(&mut desk.f, on_the_way), WINDOW);

        desk.f.advance_clock(LENGTH - PART_WAY);
        assert_eq!(pixel(&mut desk.f, on_the_way), clear());
    }
}
