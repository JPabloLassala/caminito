//! Using a frame as Marco's is used (07, 31): its buttons, titlebar and borders, the
//! cursors over them, and shade.

use camino_policy::{PolicyWindow, ResizeEdges};
use camino_test_client::{DecorationMode, WindowId};
use camino_theme::{ButtonFunction, ButtonState};
use smithay::utils::{Logical, Point, Rectangle};

use super::fixture::{BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, ClientId, Fixture, WINDOW_COLOR};
use crate::backend::headless::argb8888;
use crate::decoration::control::FrameControl;
use crate::scene::CLEAR_COLOR;
use crate::settings::Value;
use crate::settings::translate::MARCO_GENERAL;
use crate::window::Mapped;

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

/// Clicks `button` of the frame with the pointer `button` code.
fn click_button(
    f: &mut Fixture,
    id: ClientId,
    window: WindowId,
    function: ButtonFunction,
    button: u32,
) {
    let point = center(button_rect(f, id, window, function));
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

fn mapped(f: &mut Fixture, id: ClientId, window: WindowId) -> &Mapped {
    let policy_id = f.policy_id(id, window).expect("mapped");
    f.state.camino.policy.window(policy_id).unwrap()
}

fn geometry(f: &mut Fixture, id: ClientId, window: WindowId) -> Rectangle<i32, Logical> {
    let policy_id = f.policy_id(id, window).unwrap();
    f.state.camino.policy.geometry(policy_id).unwrap()
}

/// Where a button is drawn, globally.
fn button_rect(
    f: &mut Fixture,
    id: ClientId,
    window: WindowId,
    button: ButtonFunction,
) -> Rectangle<i32, Logical> {
    let origin = geometry(f, id, window).loc;
    let mut rect = mapped(f, id, window)
        .frame()
        .button_rect(button)
        .expect("the frame shows the button");
    rect.loc += origin;
    rect
}

fn center(rect: Rectangle<i32, Logical>) -> Point<f64, Logical> {
    rect.to_f64().loc + rect.to_f64().size.downscale(2.).to_point()
}

fn button_state(
    f: &mut Fixture,
    id: ClientId,
    window: WindowId,
    button: ButtonFunction,
) -> ButtonState {
    mapped(f, id, window).frame().button_states().get(button)
}

fn move_to(f: &mut Fixture, point: Point<f64, Logical>) {
    f.pointer_move(point.x, point.y);
}

#[test]
fn close_prelights_under_the_pointer_presses_and_closes_on_release() {
    let (mut f, id) = Fixture::connected();
    f.client(id).bind_input();
    let window = map_ssd(&mut f, id, (400, 300));
    let close = center(button_rect(&mut f, id, window, ButtonFunction::Close));

    move_to(&mut f, close);
    assert_eq!(
        button_state(&mut f, id, window, ButtonFunction::Close),
        ButtonState::Prelight
    );

    f.pointer_button(BTN_LEFT, true);
    f.roundtrip(id);
    assert_eq!(
        button_state(&mut f, id, window, ButtonFunction::Close),
        ButtonState::Pressed
    );
    assert!(!f.client(id).window(window).close_requested);

    f.pointer_button(BTN_LEFT, false);
    f.roundtrip(id);
    assert!(f.client(id).window(window).close_requested);
    assert_eq!(
        button_state(&mut f, id, window, ButtonFunction::Close),
        ButtonState::Prelight
    );
}

#[test]
fn releasing_off_the_button_cancels_the_click() {
    let (mut f, id) = Fixture::connected();
    f.client(id).bind_input();
    let window = map_ssd(&mut f, id, (400, 300));
    let close = button_rect(&mut f, id, window, ButtonFunction::Close);
    let minimize = center(button_rect(&mut f, id, window, ButtonFunction::Minimize));
    let title = center(close) - Point::from((150., 0.));

    move_to(&mut f, center(close));
    f.pointer_button(BTN_LEFT, true);
    // Off the button it's drawn normal, and no other button lights up.
    move_to(&mut f, minimize);
    assert_eq!(
        button_state(&mut f, id, window, ButtonFunction::Close),
        ButtonState::Normal
    );
    assert_eq!(
        button_state(&mut f, id, window, ButtonFunction::Minimize),
        ButtonState::Normal
    );
    // Back on it, it's pressed again.
    move_to(&mut f, center(close));
    assert_eq!(
        button_state(&mut f, id, window, ButtonFunction::Close),
        ButtonState::Pressed
    );
    move_to(&mut f, title);
    f.pointer_button(BTN_LEFT, false);
    f.roundtrip(id);

    assert!(!f.client(id).window(window).close_requested);
    assert_eq!(
        button_state(&mut f, id, window, ButtonFunction::Close),
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
    let before = geometry(&mut f, id, window);

    for function in [
        ButtonFunction::Menu,
        ButtonFunction::Above,
        ButtonFunction::Stick,
    ] {
        let point = center(button_rect(&mut f, id, window, function));
        move_to(&mut f, point);
        assert_eq!(
            button_state(&mut f, id, window, function),
            ButtonState::Prelight
        );
        f.pointer_button(BTN_LEFT, true);
        assert_eq!(
            button_state(&mut f, id, window, function),
            ButtonState::Pressed
        );
        f.pointer_button(BTN_LEFT, false);
    }
    f.roundtrip(id);

    let pid = policy_id(&mut f, id, window);
    let policy = &f.state.camino.policy;
    assert!(!policy.is_minimized(pid) && !policy.is_maximized(pid));
    assert_eq!(geometry(&mut f, id, window), before);
    assert!(!f.client(id).window(window).close_requested);
}

/// A point on the titlebar clear of the buttons, globally.
fn title_point(f: &mut Fixture, id: ClientId, window: WindowId) -> Point<f64, Logical> {
    let close = button_rect(f, id, window, ButtonFunction::Close);
    let geometry = geometry(f, id, window);
    Point::from((
        f64::from(geometry.loc.x + geometry.size.w / 2),
        center(close).y,
    ))
}

#[test]
fn dragging_the_titlebar_past_the_drag_threshold_moves_the_window() {
    let (mut f, id) = Fixture::connected();
    f.client(id).bind_input();
    let window = map_ssd(&mut f, id, (400, 300));
    let start = geometry(&mut f, id, window);
    let title = title_point(&mut f, id, window);

    // drag-threshold is 8 by default.
    move_to(&mut f, title);
    f.pointer_button(BTN_LEFT, true);
    move_to(&mut f, title + Point::from((8., 0.)));
    assert_eq!(geometry(&mut f, id, window), start);

    move_to(&mut f, title + Point::from((100., 50.)));
    assert_eq!(
        geometry(&mut f, id, window).loc,
        start.loc + Point::from((100, 50))
    );
    f.pointer_button(BTN_LEFT, false);
    move_to(&mut f, title);
    assert_eq!(
        geometry(&mut f, id, window).loc,
        start.loc + Point::from((100, 50))
    );
}

#[test]
fn pressing_the_titlebar_focuses_and_raises_the_window() {
    let (mut f, id) = Fixture::connected();
    f.client(id).bind_input();
    let window = map_ssd(&mut f, id, (400, 300));
    let title = title_point(&mut f, id, window);
    f.map_window(id, (100, 100));
    let pid = policy_id(&mut f, id, window);
    assert_ne!(f.state.camino.policy.focus(crate::state::SEAT), Some(pid));

    move_to(&mut f, title);
    f.pointer_button(BTN_LEFT, true);
    f.pointer_button(BTN_LEFT, false);

    let policy = &f.state.camino.policy;
    assert_eq!(policy.focus(crate::state::SEAT), Some(pid));
    assert_eq!(policy.stacking(0).last(), Some(&pid));
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
    let title = title_point(f, id, window);
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
    let before = geometry(&mut f, id, window);
    double_click_title(&mut f, id, window);
    let p = policy(&f);
    assert!(!p.is_maximized_horizontally(pid) && !p.is_maximized_vertically(pid));
    assert!(!p.is_shaded(pid) && !p.is_minimized(pid));
    assert_eq!(geometry(&mut f, id, window), before);
}

#[test]
fn two_clicks_further_apart_than_the_double_click_time_are_two_clicks() {
    let (mut f, id, window) = titlebar_with("double-click", "toggle_maximize");
    let pid = policy_id(&mut f, id, window);
    let title = title_point(&mut f, id, window);
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
    let lowered = policy_id(&mut f, id, window);
    let other = f.map_window(id, (100, 100));
    let other = policy_id(&mut f, id, other);
    f.state.camino.policy.raise(lowered);
    assert_eq!(f.state.camino.policy.stacking(0), [other, lowered]);
    let title = title_point(&mut f, id, window);
    move_to(&mut f, title);
    f.pointer_button(BTN_MIDDLE, true);
    f.pointer_button(BTN_MIDDLE, false);
    assert_eq!(f.state.camino.policy.stacking(0), [lowered, other]);

    let (mut f, id, window) = titlebar_with("right-click", "toggle_maximize");
    let pid = policy_id(&mut f, id, window);
    let title = title_point(&mut f, id, window);
    move_to(&mut f, title);
    f.pointer_button(BTN_RIGHT, true);
    f.pointer_button(BTN_RIGHT, false);
    assert!(f.state.camino.policy.is_maximized(pid));
}

/// The visible frame around a window, globally.
fn visible_frame(f: &mut Fixture, id: ClientId, window: WindowId) -> Rectangle<i32, Logical> {
    let geometry = geometry(f, id, window);
    let e = mapped(f, id, window).frame_extents(false);
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
    let (mut f, id) = Fixture::connected();
    f.client(id).bind_input();
    let window = map_ssd(&mut f, id, (400, 300));
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
        let frame = visible_frame(&mut f, id, window).to_f64();
        let before = geometry(&mut f, id, window);
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
        f.roundtrip(id);
        let size = (before.size.w + dx.abs() * 40, before.size.h + dy.abs() * 30);
        let configure = f
            .client(id)
            .window(window)
            .configures
            .last()
            .unwrap()
            .clone();
        assert_eq!(configure.size, size, "resizing from ({dx}, {dy})");
        f.client(id)
            .ack_and_commit_buffer(window, size, WINDOW_COLOR);
        f.pointer_button(BTN_LEFT, false);
        f.roundtrip(id);

        // The opposite edges stayed put.
        let after = geometry(&mut f, id, window);
        assert_eq!(after.size, size.into(), "resizing from ({dx}, {dy})");
        if dx < 0 {
            assert_eq!(after.loc.x + after.size.w, before.loc.x + before.size.w);
        } else {
            assert_eq!(after.loc.x, before.loc.x);
        }
        if dy < 0 {
            assert_eq!(after.loc.y + after.size.h, before.loc.y + before.size.h);
        } else {
            assert_eq!(after.loc.y, before.loc.y);
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
    let frame = visible_frame(&mut f, id, window).to_f64();
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
    let maximize = center(button_rect(&mut f, id, window, ButtonFunction::Maximize));
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
    let before = geometry(&mut f, id, window);

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
        assert_eq!(
            geometry(&mut f, id, window),
            before,
            "dragged from ({x}, {y})"
        );
    }
}

#[test]
fn a_shaded_window_shows_only_its_titlebar_and_keeps_the_focus_but_not_the_keyboard() {
    let (mut f, id, window) = titlebar_with("double-click", "toggle_shade");
    let pid = policy_id(&mut f, id, window);
    let output = f.state.camino.outputs[0].clone();
    let inside = center(geometry(&mut f, id, window));
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
    let strips: Vec<_> = mapped(&mut f, id, window)
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
    let frame = visible_frame(&mut f, id, window).to_f64();

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
