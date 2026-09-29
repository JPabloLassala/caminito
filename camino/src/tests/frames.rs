//! Using a frame as Marco's is used (07, 31): its buttons, titlebar and borders, the
//! cursors over them, and shade.

use camino_policy::ResizeEdges;
use std::sync::Arc;

use camino_test_client::x11::{WindowSpec, X11Client};
use camino_test_client::{DecorationMode, WindowId};
use camino_theme::{ButtonFunction, ButtonState};
use smithay::utils::{Logical, Point, Rectangle};
use x11rb::protocol::Event;

use super::fixture::{
    BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, ClientId, ClientWindow, Fixture, WINDOW_COLOR,
};
use crate::backend::headless::argb8888;
use crate::decoration::control::FrameControl;
use crate::scene::CLEAR_COLOR;
use crate::settings::Value;
use crate::settings::translate::MARCO_GENERAL;
use crate::window::Mapped;

type PolicyId = camino_policy::WindowId;

/// Every button Marco has, on one output and a connected client.
fn every_button() -> (Fixture, ClientId) {
    let mut f = Fixture::with_settings(vec![(
        MARCO_GENERAL,
        "button-layout".into(),
        Value::Str("menu,shade,above,stick:minimize,maximize,close".into()),
    )]);
    f.add_output((1920, 1080), 1.0);
    let id = f.add_client();
    f.roundtrip(id);
    f.client(id).bind_input();
    (f, id)
}

fn policy_id(f: &mut Fixture, id: ClientId, window: WindowId) -> camino_policy::WindowId {
    f.policy_id(id, window).unwrap()
}

/// A framed window under test, whichever protocol its client speaks.
#[derive(Clone)]
enum Framed {
    Wayland(ClientId, WindowId),
    X11(Arc<X11Client>, u32),
}

impl std::fmt::Debug for Framed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Framed::Wayland(..) => write!(f, "an xdg toplevel"),
            Framed::X11(..) => write!(f, "an XWayland window"),
        }
    }
}

impl Framed {
    /// One output and a Wayland client's SSD window, 400 × 300.
    fn wayland() -> (Fixture, Framed) {
        let (mut f, id) = Fixture::connected();
        f.client(id).bind_input();
        let window = map_ssd(&mut f, id, (400, 300));
        (f, Framed::Wayland(id, window))
    }

    /// One output and an X client's window, 400 × 300, which takes `WM_DELETE_WINDOW`.
    fn x11() -> (Fixture, Framed) {
        let (mut f, x) = Fixture::with_x11();
        let window = f.map_x11_window(
            &x,
            WindowSpec {
                size: (400, 300),
                delete_window: true,
                ..Default::default()
            },
        );
        f.dispatch();
        (f, Framed::X11(x, window))
    }

    fn client_window(&self) -> ClientWindow {
        match self {
            Framed::Wayland(id, window) => ClientWindow::Wayland(*id, *window),
            Framed::X11(_, window) => ClientWindow::X11(*window),
        }
    }

    fn pid(&self, f: &mut Fixture) -> PolicyId {
        f.window_policy_id(self.client_window()).expect("mapped")
    }

    /// Lets the client see what Camino sent it, and Camino what the client answered.
    fn sync(&self, f: &mut Fixture) {
        match self {
            Framed::Wayland(id, _) => f.roundtrip(*id),
            Framed::X11(..) => f.dispatch(),
        }
    }

    /// Whether Camino has asked the client to close the window since this was last asked.
    fn close_requested(&self, f: &mut Fixture) -> bool {
        match self {
            Framed::Wayland(id, window) => f.client(*id).window(*window).close_requested,
            Framed::X11(x, window) => {
                let window = *window;
                f.x11_call(x, move |x| {
                    let protocols = x.atom("WM_PROTOCOLS")?;
                    let delete = x.atom("WM_DELETE_WINDOW")?;
                    Ok::<_, x11rb::errors::ReplyError>(x.events()?.iter().any(|event| {
                        matches!(event, Event::ClientMessage(message)
                            if message.window == window
                                && message.type_ == protocols
                                && message.data.as_data32()[0] == delete)
                    }))
                })
                .unwrap()
            }
        }
    }

    /// Maps another window of the same client.
    fn map_another(&self, f: &mut Fixture) {
        match self {
            Framed::Wayland(id, _) => drop(f.map_window(*id, (100, 100))),
            Framed::X11(x, _) => {
                let spec = WindowSpec {
                    size: (100, 100),
                    ..Default::default()
                };
                f.map_x11_window(x, spec);
            }
        }
    }

    /// Has the client draw at the size Camino last asked of it, `size`, while a resize
    /// holds the pointer: an xdg client acks and commits, and Xwayland redraws the window
    /// once X has resized it.
    fn draw_at_asked_size(&self, f: &mut Fixture, size: (i32, i32)) {
        match self {
            Framed::Wayland(id, window) => {
                f.roundtrip(*id);
                let asked = f
                    .client(*id)
                    .window(*window)
                    .configures
                    .last()
                    .unwrap()
                    .size;
                assert_eq!(asked, size);
                f.client(*id)
                    .ack_and_commit_buffer(*window, size, WINDOW_COLOR);
            }
            Framed::X11(..) => {
                let pid = self.pid(f);
                f.dispatch_until(|f| geometry(f, pid).size == size.into());
            }
        }
    }
}

/// Clicks `button` of the frame with the pointer `button` code.
fn click_button(
    f: &mut Fixture,
    id: ClientId,
    window: WindowId,
    function: ButtonFunction,
    button: u32,
) {
    let pid = policy_id(f, id, window);
    let point = center(button_rect(f, pid, function));
    move_to(f, point);
    f.pointer_button(button, true);
    f.pointer_button(button, false);
    f.roundtrip(id);
}

/// Maps an SSD window at `size`, and draws its frame.
fn map_ssd(f: &mut Fixture, id: ClientId, size: (i32, i32)) -> WindowId {
    let window = f.client(id).create_window();
    f.client(id)
        .request_decoration(window, Some(DecorationMode::ServerSide));
    f.client(id).commit(window);
    f.roundtrip(id);
    f.client(id)
        .ack_and_commit_buffer(window, size, WINDOW_COLOR);
    f.roundtrip(id);
    f.dispatch();
    window
}

fn mapped(f: &Fixture, pid: PolicyId) -> &Mapped {
    f.state.camino.policy.window(pid).expect("mapped")
}

fn geometry(f: &Fixture, pid: PolicyId) -> Rectangle<i32, Logical> {
    f.state.camino.policy.geometry(pid).unwrap()
}

/// Where a button is drawn, globally.
fn button_rect(f: &Fixture, pid: PolicyId, button: ButtonFunction) -> Rectangle<i32, Logical> {
    let origin = geometry(f, pid).loc;
    let mut rect = mapped(f, pid)
        .frame()
        .button_rect(button)
        .expect("the frame shows the button");
    rect.loc += origin;
    rect
}

fn center(rect: Rectangle<i32, Logical>) -> Point<f64, Logical> {
    rect.to_f64().loc + rect.to_f64().size.downscale(2.).to_point()
}

fn button_state(f: &Fixture, pid: PolicyId, button: ButtonFunction) -> ButtonState {
    mapped(f, pid).frame().button_states().get(button)
}

fn move_to(f: &mut Fixture, point: Point<f64, Logical>) {
    f.pointer_move(point.x, point.y);
}

#[test]
fn close_prelights_under_the_pointer_presses_and_closes_on_release() {
    for framed in [Framed::wayland, Framed::x11] {
        let (mut f, window) = framed();
        let pid = window.pid(&mut f);
        let close = center(button_rect(&f, pid, ButtonFunction::Close));

        move_to(&mut f, close);
        assert_eq!(
            button_state(&f, pid, ButtonFunction::Close),
            ButtonState::Prelight,
            "{window:?}"
        );

        f.pointer_button(BTN_LEFT, true);
        window.sync(&mut f);
        assert_eq!(
            button_state(&f, pid, ButtonFunction::Close),
            ButtonState::Pressed,
            "{window:?}"
        );
        assert!(!window.close_requested(&mut f), "{window:?}");

        f.pointer_button(BTN_LEFT, false);
        window.sync(&mut f);
        assert!(window.close_requested(&mut f), "{window:?}");
        assert_eq!(
            button_state(&f, pid, ButtonFunction::Close),
            ButtonState::Prelight,
            "{window:?}"
        );
    }
}

#[test]
fn releasing_off_the_button_cancels_the_click() {
    let (mut f, id) = Fixture::connected();
    f.client(id).bind_input();
    let window = map_ssd(&mut f, id, (400, 300));
    let pid = policy_id(&mut f, id, window);
    let close = button_rect(&f, pid, ButtonFunction::Close);
    let minimize = center(button_rect(&f, pid, ButtonFunction::Minimize));
    let title = center(close) - Point::from((150., 0.));

    move_to(&mut f, center(close));
    f.pointer_button(BTN_LEFT, true);
    // Off the button it's drawn normal, and no other button lights up.
    move_to(&mut f, minimize);
    assert_eq!(
        button_state(&f, pid, ButtonFunction::Close),
        ButtonState::Normal
    );
    assert_eq!(
        button_state(&f, pid, ButtonFunction::Minimize),
        ButtonState::Normal
    );
    // Back on it, it's pressed again.
    move_to(&mut f, center(close));
    assert_eq!(
        button_state(&f, pid, ButtonFunction::Close),
        ButtonState::Pressed
    );
    move_to(&mut f, title);
    f.pointer_button(BTN_LEFT, false);
    f.roundtrip(id);

    assert!(!f.client(id).window(window).close_requested);
    assert_eq!(
        button_state(&f, pid, ButtonFunction::Close),
        ButtonState::Normal
    );
    let policy_id = f.policy_id(id, window).unwrap();
    assert!(!f.state.camino.policy.is_minimized(policy_id));
}

#[test]
fn minimise_minimises_and_maximise_toggles_both_ways_or_one_by_the_mouse_button() {
    let (mut f, id) = every_button();
    let window = map_ssd(&mut f, id, (600, 300));
    let pid = policy_id(&mut f, id, window);

    click_button(&mut f, id, window, ButtonFunction::Maximize, BTN_LEFT);
    assert!(f.state.camino.policy.is_maximized(pid));
    click_button(&mut f, id, window, ButtonFunction::Maximize, BTN_LEFT);
    assert!(!f.state.camino.policy.is_maximized_horizontally(pid));

    click_button(&mut f, id, window, ButtonFunction::Maximize, BTN_MIDDLE);
    assert!(f.state.camino.policy.is_maximized_vertically(pid));
    assert!(!f.state.camino.policy.is_maximized_horizontally(pid));
    click_button(&mut f, id, window, ButtonFunction::Maximize, BTN_MIDDLE);
    click_button(&mut f, id, window, ButtonFunction::Maximize, BTN_RIGHT);
    assert!(f.state.camino.policy.is_maximized_horizontally(pid));
    assert!(!f.state.camino.policy.is_maximized_vertically(pid));
    click_button(&mut f, id, window, ButtonFunction::Maximize, BTN_RIGHT);

    click_button(&mut f, id, window, ButtonFunction::Minimize, BTN_LEFT);
    assert!(f.state.camino.policy.is_minimized(pid));
}

#[test]
fn shade_shades_and_unshade_unshades() {
    let (mut f, id) = every_button();
    let window = map_ssd(&mut f, id, (600, 300));
    let pid = policy_id(&mut f, id, window);

    click_button(&mut f, id, window, ButtonFunction::Shade, BTN_LEFT);
    assert!(f.state.camino.policy.is_shaded(pid));
    click_button(&mut f, id, window, ButtonFunction::Unshade, BTN_LEFT);
    assert!(!f.state.camino.policy.is_shaded(pid));
}

#[test]
fn menu_above_and_stick_light_up_but_do_nothing_yet() {
    let (mut f, id) = every_button();
    let window = map_ssd(&mut f, id, (600, 300));
    let pid = policy_id(&mut f, id, window);
    let before = geometry(&f, pid);

    for function in [
        ButtonFunction::Menu,
        ButtonFunction::Above,
        ButtonFunction::Stick,
    ] {
        let point = center(button_rect(&f, pid, function));
        move_to(&mut f, point);
        assert_eq!(button_state(&f, pid, function), ButtonState::Prelight);
        f.pointer_button(BTN_LEFT, true);
        assert_eq!(button_state(&f, pid, function), ButtonState::Pressed);
        f.pointer_button(BTN_LEFT, false);
    }
    f.roundtrip(id);

    let pid = policy_id(&mut f, id, window);
    let policy = &f.state.camino.policy;
    assert!(!policy.is_minimized(pid) && !policy.is_maximized(pid));
    assert_eq!(geometry(&f, pid), before);
    assert!(!f.client(id).window(window).close_requested);
}

/// A point on the titlebar clear of the buttons, globally.
fn title_point(f: &Fixture, pid: PolicyId) -> Point<f64, Logical> {
    let close = button_rect(f, pid, ButtonFunction::Close);
    let geometry = geometry(f, pid);
    Point::from((
        f64::from(geometry.loc.x + geometry.size.w / 2),
        center(close).y,
    ))
}

#[test]
fn dragging_the_titlebar_past_the_drag_threshold_moves_the_window() {
    for framed in [Framed::wayland, Framed::x11] {
        let (mut f, window) = framed();
        let pid = window.pid(&mut f);
        let start = geometry(&f, pid);
        let title = title_point(&f, pid);

        // drag-threshold is 8 by default.
        move_to(&mut f, title);
        f.pointer_button(BTN_LEFT, true);
        move_to(&mut f, title + Point::from((8., 0.)));
        assert_eq!(geometry(&f, pid), start, "{window:?}");

        move_to(&mut f, title + Point::from((100., 50.)));
        let moved = start.loc + Point::from((100, 50));
        assert_eq!(geometry(&f, pid).loc, moved, "{window:?}");
        f.pointer_button(BTN_LEFT, false);
        move_to(&mut f, title);
        assert_eq!(geometry(&f, pid).loc, moved, "{window:?}");
    }
}

#[test]
fn pressing_the_titlebar_focuses_and_raises_the_window() {
    for framed in [Framed::wayland, Framed::x11] {
        let (mut f, window) = framed();
        let pid = window.pid(&mut f);
        let title = title_point(&f, pid);
        window.map_another(&mut f);
        assert_ne!(f.state.camino.policy.focus(crate::state::SEAT), Some(pid));

        move_to(&mut f, title);
        f.pointer_button(BTN_LEFT, true);
        f.pointer_button(BTN_LEFT, false);

        let policy = &f.state.camino.policy;
        assert_eq!(policy.focus(crate::state::SEAT), Some(pid), "{window:?}");
        assert_eq!(policy.stacking(0).last(), Some(&pid), "{window:?}");
    }
}

/// One SSD window under a fresh Camino whose titlebar `click` action is `action`.
fn titlebar_with(click: &str, action: &str) -> (Fixture, ClientId, WindowId) {
    let mut f = Fixture::with_settings(vec![(
        MARCO_GENERAL,
        format!("action-{click}-titlebar"),
        Value::Str(action.into()),
    )]);
    f.add_output((1920, 1080), 1.0);
    let id = f.add_client();
    f.roundtrip(id);
    f.client(id).bind_input();
    let window = map_ssd(&mut f, id, (400, 300));
    (f, id, window)
}

fn double_click_title(f: &mut Fixture, id: ClientId, window: WindowId) {
    let pid = policy_id(f, id, window);
    let title = title_point(f, pid);
    move_to(f, title);
    for _ in 0..2 {
        f.pointer_button(BTN_LEFT, true);
        f.pointer_button(BTN_LEFT, false);
    }
    f.roundtrip(id);
}

fn policy(f: &Fixture) -> &camino_policy::Policy<Mapped> {
    &f.state.camino.policy
}

#[test]
fn double_clicking_the_titlebar_runs_its_action() {
    let (mut f, id, window) = titlebar_with("double-click", "toggle_maximize");
    let pid = policy_id(&mut f, id, window);
    double_click_title(&mut f, id, window);
    assert!(policy(&f).is_maximized(pid));
    double_click_title(&mut f, id, window);
    assert!(!policy(&f).is_maximized(pid));

    let (mut f, id, window) = titlebar_with("double-click", "toggle_maximize_horizontally");
    let pid = policy_id(&mut f, id, window);
    double_click_title(&mut f, id, window);
    assert!(policy(&f).is_maximized_horizontally(pid));
    assert!(!policy(&f).is_maximized_vertically(pid));

    let (mut f, id, window) = titlebar_with("double-click", "toggle_maximize_vertically");
    let pid = policy_id(&mut f, id, window);
    double_click_title(&mut f, id, window);
    assert!(policy(&f).is_maximized_vertically(pid));
    assert!(!policy(&f).is_maximized_horizontally(pid));

    let (mut f, id, window) = titlebar_with("double-click", "toggle_shade");
    let pid = policy_id(&mut f, id, window);
    double_click_title(&mut f, id, window);
    assert!(policy(&f).is_shaded(pid));
    double_click_title(&mut f, id, window);
    assert!(!policy(&f).is_shaded(pid));

    let (mut f, id, window) = titlebar_with("double-click", "minimize");
    let pid = policy_id(&mut f, id, window);
    double_click_title(&mut f, id, window);
    assert!(policy(&f).is_minimized(pid));

    let (mut f, id, window) = titlebar_with("double-click", "none");
    let pid = policy_id(&mut f, id, window);
    let before = geometry(&f, pid);
    double_click_title(&mut f, id, window);
    let p = policy(&f);
    assert!(!p.is_maximized_horizontally(pid) && !p.is_maximized_vertically(pid));
    assert!(!p.is_shaded(pid) && !p.is_minimized(pid));
    assert_eq!(geometry(&f, pid), before);
}

#[test]
fn two_clicks_further_apart_than_the_double_click_time_are_two_clicks() {
    let (mut f, id, window) = titlebar_with("double-click", "toggle_maximize");
    let pid = policy_id(&mut f, id, window);
    let title = title_point(&f, pid);
    move_to(&mut f, title);

    f.pointer_button(BTN_LEFT, true);
    f.pointer_button(BTN_LEFT, false);
    // double-click is 400 ms by default; each input event is 10 ms after the last.
    for _ in 0..40 {
        move_to(&mut f, title);
    }
    f.pointer_button(BTN_LEFT, true);
    f.pointer_button(BTN_LEFT, false);

    assert!(!f.state.camino.policy.is_maximized(pid));
}

#[test]
fn middle_and_right_clicks_on_the_titlebar_run_their_actions() {
    let (mut f, id, window) = titlebar_with("middle-click", "lower");
    let pid = policy_id(&mut f, id, window);
    let lowered = policy_id(&mut f, id, window);
    let other = f.map_window(id, (100, 100));
    let other = policy_id(&mut f, id, other);
    f.state.camino.policy.raise(lowered);
    assert_eq!(f.state.camino.policy.stacking(0), [other, lowered]);
    let title = title_point(&f, pid);
    move_to(&mut f, title);
    f.pointer_button(BTN_MIDDLE, true);
    f.pointer_button(BTN_MIDDLE, false);
    assert_eq!(f.state.camino.policy.stacking(0), [lowered, other]);

    let (mut f, id, window) = titlebar_with("right-click", "toggle_maximize");
    let pid = policy_id(&mut f, id, window);
    let title = title_point(&f, pid);
    move_to(&mut f, title);
    f.pointer_button(BTN_RIGHT, true);
    f.pointer_button(BTN_RIGHT, false);
    assert!(f.state.camino.policy.is_maximized(pid));
}

/// The visible frame around a window, globally.
fn visible_frame(f: &Fixture, pid: PolicyId) -> Rectangle<i32, Logical> {
    let geometry = geometry(f, pid);
    let e = f.state.camino.policy.frame_extents(pid);
    Rectangle::new(
        geometry.loc - Point::from((e.left, e.top)),
        (
            geometry.size.w + e.left + e.right,
            geometry.size.h + e.top + e.bottom,
        )
            .into(),
    )
}

#[test]
fn dragging_each_border_and_corner_resizes_from_it() {
    for framed in [Framed::wayland, Framed::x11] {
        let (mut f, window) = framed();
        let pid = window.pid(&mut f);
        // Each edge as (dx, dy): where on the frame, and which way it grows.
        let edges = [
            (0, -1),
            (0, 1),
            (-1, 0),
            (1, 0),
            (-1, -1),
            (1, -1),
            (-1, 1),
            (1, 1),
        ];
        for (dx, dy) in edges {
            let frame = visible_frame(&f, pid).to_f64();
            let before = geometry(&f, pid);
            // 5 pixels out into the invisible border, halfway along the side.
            let along = |d: i32, start: f64, size: f64| match d {
                -1 => start - 5.,
                1 => start + size + 5.,
                _ => start + size / 2.,
            };
            let point = Point::from((
                along(dx, frame.loc.x, frame.size.w),
                along(dy, frame.loc.y, frame.size.h),
            ));
            move_to(&mut f, point);
            f.pointer_button(BTN_LEFT, true);
            move_to(
                &mut f,
                point + Point::from((f64::from(dx * 40), f64::from(dy * 30))),
            );
            let size = (before.size.w + dx.abs() * 40, before.size.h + dy.abs() * 30);
            window.draw_at_asked_size(&mut f, size);
            f.pointer_button(BTN_LEFT, false);
            window.sync(&mut f);

            // The opposite edges stayed put.
            let after = geometry(&f, pid);
            let edge = format!("{window:?} resizing from ({dx}, {dy})");
            assert_eq!(after.size, size.into(), "{edge}");
            if dx < 0 {
                assert_eq!(
                    after.loc.x + after.size.w,
                    before.loc.x + before.size.w,
                    "{edge}"
                );
            } else {
                assert_eq!(after.loc.x, before.loc.x, "{edge}");
            }
            if dy < 0 {
                assert_eq!(
                    after.loc.y + after.size.h,
                    before.loc.y + before.size.h,
                    "{edge}"
                );
            } else {
                assert_eq!(after.loc.y, before.loc.y, "{edge}");
            }
        }
    }
}

/// What the frame's control under the pointer resizes, if anything.
fn resize_under_pointer(f: &Fixture) -> Option<ResizeEdges> {
    match f.state.camino.frame_pointer.hover {
        Some((_, FrameControl::Resize(edges))) => Some(edges),
        _ => None,
    }
}

#[test]
fn the_corners_resize_zones_reach_along_the_sides_as_marcos_do() {
    let (mut f, id) = Fixture::connected();
    f.client(id).bind_input();
    let window = map_ssd(&mut f, id, (400, 300));
    let pid = policy_id(&mut f, id, window);
    let frame = visible_frame(&f, pid).to_f64();
    let bottom = frame.loc.y + frame.size.h;
    let right = frame.loc.x + frame.size.w;
    let both = |top, left| ResizeEdges {
        top,
        bottom: !top,
        left,
        right: !left,
    };

    // On the left border, 12 pixels below the frame's top: the top-left corner.
    move_to(&mut f, Point::from((frame.loc.x - 5., frame.loc.y + 12.)));
    assert_eq!(resize_under_pointer(&f), Some(both(true, true)));
    // On the bottom border, 12 pixels in from the right: the bottom-right corner.
    move_to(&mut f, Point::from((right - 12., bottom + 5.)));
    assert_eq!(resize_under_pointer(&f), Some(both(false, false)));
    // Halfway along the right border: just the right edge.
    move_to(
        &mut f,
        Point::from((right + 5., frame.loc.y + frame.size.h / 2.)),
    );
    assert_eq!(
        resize_under_pointer(&f),
        Some(ResizeEdges {
            right: true,
            ..ResizeEdges::default()
        })
    );
}

#[test]
fn a_maximised_frame_has_no_resize_border_where_it_meets_the_output() {
    let (mut f, id) = Fixture::connected();
    f.client(id).bind_input();
    let window = map_ssd(&mut f, id, (400, 300));
    let pid = policy_id(&mut f, id, window);
    let maximize = center(button_rect(&f, pid, ButtonFunction::Maximize));
    move_to(&mut f, maximize);
    f.pointer_button(BTN_LEFT, true);
    f.pointer_button(BTN_LEFT, false);
    f.roundtrip(id);
    let size = f.client(id).window(window).configures.last().unwrap().size;
    f.client(id)
        .ack_and_commit_buffer(window, size, WINDOW_COLOR);
    f.roundtrip(id);
    f.dispatch();
    assert!(f.state.camino.policy.is_maximized(pid));
    let before = geometry(&f, pid);

    for (x, y) in [
        (0., 540.),
        (1919., 540.),
        (960., 1079.),
        (0., 0.),
        (1919., 1079.),
    ] {
        move_to(&mut f, Point::from((x, y)));
        assert_eq!(resize_under_pointer(&f), None, "at ({x}, {y})");
        f.pointer_button(BTN_LEFT, true);
        move_to(&mut f, Point::from((960., 540.)));
        f.pointer_button(BTN_LEFT, false);
        assert_eq!(geometry(&f, pid), before, "dragged from ({x}, {y})");
    }
}

#[test]
fn a_shaded_window_shows_only_its_titlebar_and_keeps_the_focus_but_not_the_keyboard() {
    let (mut f, id, window) = titlebar_with("double-click", "toggle_shade");
    let pid = policy_id(&mut f, id, window);
    let output = f.state.camino.outputs[0].clone();
    let inside = center(geometry(&f, pid));
    let pixel_inside = |f: &mut Fixture| {
        let pixels = f.state.backend.headless_mut().pixels(&output).unwrap();
        pixels[(inside.y as usize) * 1920 + inside.x as usize]
    };
    assert_eq!(pixel_inside(&mut f), WINDOW_COLOR);

    double_click_title(&mut f, id, window);
    f.dispatch();

    assert!(policy(&f).is_shaded(pid));
    assert_eq!(policy(&f).focus(crate::state::SEAT), Some(pid));
    let keyboard = f.state.camino.seat.get_keyboard().unwrap();
    assert_eq!(keyboard.current_focus(), None);
    assert_eq!(pixel_inside(&mut f), argb8888(CLEAR_COLOR));
    let strips: Vec<_> = mapped(&f, pid)
        .frame()
        .strips()
        .map(|(rect, _)| rect)
        .collect();
    assert_eq!(strips.len(), 1, "only the titlebar: {strips:?}");
    // The pointer finds nothing where the client was.
    move_to(&mut f, inside);
    assert_eq!(f.state.camino.window_under(inside), None);

    double_click_title(&mut f, id, window);
    f.dispatch();
    assert!(!policy(&f).is_shaded(pid));
    assert_eq!(pixel_inside(&mut f), WINDOW_COLOR);
    let keyboard = f.state.camino.seat.get_keyboard().unwrap();
    assert!(keyboard.current_focus().is_some());
}

#[test]
fn a_csd_window_cant_be_shaded() {
    let (mut f, id) = Fixture::connected();
    let window = f.map_window(id, (400, 300));
    let pid = policy_id(&mut f, id, window);

    f.state.camino.policy.set_shaded(pid, true);

    assert!(!policy(&f).is_shaded(pid));
}

#[test]
fn the_pointer_goes_through_the_rounded_corners() {
    let (mut f, id) = Fixture::connected();
    let window = map_ssd(&mut f, id, (400, 300));
    let pid = policy_id(&mut f, id, window);
    let frame = visible_frame(&f, pid).to_f64();

    // Menta rounds its top corners with a radius of 4, which cuts the corner pixel out.
    let corner = frame.loc + Point::from((0.5, 0.5));
    assert_eq!(f.state.camino.window_under(corner), None);
    let beside = corner + Point::from((6., 0.));
    assert_eq!(f.state.camino.window_under(beside), Some(pid));
    let right_corner = Point::from((frame.loc.x + frame.size.w - 0.5, corner.y));
    assert_eq!(f.state.camino.window_under(right_corner), None);
}

#[test]
fn any_mouse_button_on_a_maximised_windows_button_unmaximises_it() {
    let (mut f, id) = every_button();
    let window = map_ssd(&mut f, id, (600, 300));
    let pid = policy_id(&mut f, id, window);

    for button in [BTN_MIDDLE, BTN_RIGHT] {
        click_button(&mut f, id, window, ButtonFunction::Maximize, BTN_LEFT);
        assert!(policy(&f).is_maximized(pid));
        click_button(&mut f, id, window, ButtonFunction::Maximize, button);
        let p = policy(&f);
        assert!(!p.is_maximized_horizontally(pid) && !p.is_maximized_vertically(pid));
    }
}
