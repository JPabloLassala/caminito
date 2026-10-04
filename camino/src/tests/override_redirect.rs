//! Override-redirect windows: X's menus and tooltips, in the scene's own layer above every
//! toplevel (11, 36). Only X clients make them, so the toplevels here are X windows too, as
//! in an X app whose menus open over its own window (45).

use smithay::utils::{Logical, Point, Rectangle};
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{GrabStatus, StackMode};

use super::fixture::{Fixture, KEY_A, KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT, Kind, WindowSpec};
use crate::scene::{LAYERS, SceneLayer};
use crate::state::SEAT;

const RED: u32 = 0xff0000;
const BLUE: u32 = 0x0000ff;

fn center(rect: Rectangle<i32, Logical>) -> Point<i32, Logical> {
    rect.loc + rect.size.downscale(2).to_point()
}

/// `size` at `center`, as an X window's position and size.
fn around(center: Point<i32, Logical>, size: u16) -> ((i16, i16), (u16, u16)) {
    let half = i32::from(size) / 2;
    let at = (
        i16::try_from(center.x - half).unwrap(),
        i16::try_from(center.y - half).unwrap(),
    );
    (at, (size, size))
}

#[test]
fn an_override_redirect_window_shows_above_a_toplevel() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let toplevel = f.map(x, WindowSpec::default());
    let middle = center(f.geometry(toplevel));

    let (at, size) = around(middle, 100);
    f.map_override_redirect(x, at, size, RED);

    f.wait_for_pixel(middle.x, middle.y, RED);
}

#[test]
fn an_override_redirect_window_shows_above_a_fullscreen_toplevel() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let toplevel = f.map(
        x,
        WindowSpec {
            states: vec!["_NET_WM_STATE_FULLSCREEN"],
            ..WindowSpec::default()
        },
    );
    assert!(f.state.camino.policy.is_fullscreen(f.pid(toplevel)));

    let (at, size) = around(Point::from((960, 540)), 100);
    f.map_override_redirect(x, at, size, RED);

    f.wait_for_pixel(960, 540, RED);
}

#[test]
fn an_override_redirect_window_stays_visible_after_switching_workspace() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let (at, size) = around(Point::from((960, 540)), 100);
    f.map_override_redirect(x, at, size, RED);
    f.wait_for_pixel(960, 540, RED);

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    assert_eq!(f.state.camino.policy.active_workspace(), 1);
    f.map(x, WindowSpec::default());

    f.wait_for_pixel(960, 540, RED);
}

#[test]
fn two_override_redirect_windows_keep_their_above_order_after_a_restack() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let toplevel = f.map(x, WindowSpec::default());
    let middle = center(f.geometry(toplevel));
    let (at, size) = around(middle, 100);
    let red = f.map_override_redirect(x, at, size, RED);
    let blue = f.map_override_redirect(x, at, size, BLUE);
    f.wait_for_pixel(middle.x, middle.y, BLUE);

    // Raised to the top, then put back just above the toplevel's frame, below blue.
    f.x11_call(x, move |x| x.restack(red, StackMode::ABOVE, None))
        .unwrap();
    f.wait_for_pixel(middle.x, middle.y, RED);
    f.x11_call(x, move |x| x.restack(red, StackMode::BELOW, Some(blue)))
        .unwrap();
    f.wait_for_pixel(middle.x, middle.y, BLUE);

    // Raising the toplevel over another, newer one restacks X's windows, but not these two.
    f.x11_call(x, move |x| x.restack(red, StackMode::ABOVE, None))
        .unwrap();
    f.wait_for_pixel(middle.x, middle.y, RED);
    f.map(x, WindowSpec::default());
    let now = f.state.camino.now();
    f.state.camino.policy.activate(SEAT, f.pid(toplevel), now);
    f.dispatch();
    // Camino has seen X's answer to the restack once the client is in sync.
    f.sync(toplevel);
    f.wait_for_pixel(middle.x, middle.y, RED);
}

#[test]
fn clicking_an_override_redirect_window_reaches_it_and_leaves_keyboard_focus_alone() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let below = f.map(x, WindowSpec::default());
    let focused = f.map(x, WindowSpec::default());
    let middle = center(f.geometry(below));
    let (at, size) = around(middle, 100);
    let menu = f.map_override_redirect(x, at, size, RED);

    f.click(f64::from(middle.x), f64::from(middle.y));

    assert_eq!(f.override_redirect_under_pointer(), Some(menu));
    assert_eq!(f.state.camino.policy.focus(SEAT), Some(f.pid(focused)));
    assert_eq!(f.x11_call(x, |x| x.input_focus()).unwrap(), focused.x11());
}

#[test]
fn an_override_redirect_window_that_grabs_the_keyboard_gets_the_keys_until_it_lets_go() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let toplevel = f.map(x, WindowSpec::default());
    let middle = center(f.geometry(toplevel));
    let (at, size) = around(middle, 100);
    let menu = f.map_override_redirect(x, at, size, RED);

    let status = f.x11_call(x, move |x| x.grab_keyboard(menu)).unwrap();
    assert_eq!(status, GrabStatus::SUCCESS);
    f.dispatch_until(|f| f.override_redirect_with_keyboard() == Some(menu));

    f.key(KEY_A, true);
    f.key(KEY_A, false);
    let events = f.x11_call(x, |x| x.events()).unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::KeyPress(key) if key.event == menu)),
        "{events:?}"
    );

    f.x11_call(x, |x| x.ungrab_keyboard()).unwrap();
    f.dispatch_until(|f| f.has_keyboard(toplevel));
}

#[test]
fn the_override_redirect_layer_is_drawn_between_the_top_and_overlay_layers() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let toplevel = f.map(x, WindowSpec::default());
    let middle = center(f.geometry(toplevel));
    let (at, size) = around(middle, 100);
    f.map_override_redirect(x, at, size, RED);
    f.pointer_move(f64::from(middle.x), f64::from(middle.y));

    let output = f.state.camino.outputs[0].clone();
    let mut layers = f.state.scene_layers(&output);
    layers.dedup();
    assert_eq!(
        layers,
        [
            SceneLayer::Cursor,
            SceneLayer::OverrideRedirect,
            SceneLayer::Windows
        ]
    );
    // An X client makes no layer surfaces, so where their layers go is the layer order's.
    let at = |layer| LAYERS.iter().position(|l| *l == layer).unwrap();
    assert!(at(SceneLayer::Overlay) < at(SceneLayer::OverrideRedirect));
    assert!(at(SceneLayer::OverrideRedirect) < at(SceneLayer::Top));
}
