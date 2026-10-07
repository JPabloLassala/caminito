//! Using a frame as Marco's is used (07, 31): its buttons, titlebar and borders, the
//! cursors over them, and shade. Each test runs with an xdg toplevel and an XWayland
//! window (45).

use caminito_policy::ResizeEdges;
use caminito_theme::{ButtonFunction, ButtonState};
use smithay::utils::{Logical, Point, Rectangle};

use super::fixture::{
    BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, ClientWindow, Decoration, Fixture, Kind, WINDOW_COLOR,
    WindowSpec, on_both_kinds,
};
use crate::backend::headless::argb8888;
use crate::decoration::control::FrameControl;
use crate::scene::CLEAR_COLOR;
use crate::settings::Value;
use crate::settings::translate::MARCO_GENERAL;
use crate::window::Mapped;

/// One output and a framed window of `size` from a client of `kind`, under a Caminito with
/// `settings` set from the start.
fn framed_with(
    kind: Kind,
    settings: Vec<(&'static str, String, Value)>,
    size: (i32, i32),
) -> (Fixture, ClientWindow) {
    let mut f = Fixture::with_settings(settings);
    f.add_output((1920, 1080), 1.0);
    let client = f.connect(kind);
    let window = f.map(
        client,
        WindowSpec {
            size,
            ..WindowSpec::default()
        },
    );
    (f, window)
}

/// One output and a 400 × 300 framed window from a client of `kind`.
fn framed(kind: Kind) -> (Fixture, ClientWindow) {
    framed_with(kind, Vec::new(), (400, 300))
}

/// A 600 × 300 framed window with every button Marco has.
fn every_button(kind: Kind) -> (Fixture, ClientWindow) {
    let layout = "menu,shade,above,stick:minimize,maximize,close";
    let settings = vec![(
        MARCO_GENERAL,
        "button-layout".into(),
        Value::Str(layout.into()),
    )];
    framed_with(kind, settings, (600, 300))
}

/// A 400 × 300 framed window under a fresh Caminito whose titlebar `click` action is `action`.
fn titlebar_with(kind: Kind, click: &str, action: &str) -> (Fixture, ClientWindow) {
    let settings = vec![(
        MARCO_GENERAL,
        format!("action-{click}-titlebar"),
        Value::Str(action.into()),
    )];
    framed_with(kind, settings, (400, 300))
}

fn policy(f: &Fixture) -> &caminito_policy::Policy<Mapped> {
    &f.state.caminito.policy
}

fn center(rect: Rectangle<i32, Logical>) -> Point<f64, Logical> {
    rect.to_f64().loc + rect.to_f64().size.downscale(2.).to_point()
}

fn move_to(f: &mut Fixture, point: Point<f64, Logical>) {
    f.pointer_move(point.x, point.y);
}

/// Clicks `function`'s button of the frame with the pointer `button` code.
fn click_button(f: &mut Fixture, window: ClientWindow, function: ButtonFunction, button: u32) {
    let point = center(f.button_rect(window, function));
    move_to(f, point);
    f.pointer_button(button, true);
    f.pointer_button(button, false);
    f.sync(window);
}

/// A point on the titlebar clear of the buttons, globally.
fn title_point(f: &Fixture, window: ClientWindow) -> Point<f64, Logical> {
    let close = f.button_rect(window, ButtonFunction::Close);
    let geometry = f.geometry(window);
    Point::from((
        f64::from(geometry.loc.x + geometry.size.w / 2),
        center(close).y,
    ))
}

fn double_click_title(f: &mut Fixture, window: ClientWindow) {
    let title = title_point(f, window);
    move_to(f, title);
    for _ in 0..2 {
        f.pointer_button(BTN_LEFT, true);
        f.pointer_button(BTN_LEFT, false);
    }
    f.sync(window);
}

/// What the frame's control under the pointer resizes, if anything.
fn resize_under_pointer(f: &Fixture) -> Option<ResizeEdges> {
    match f.control_under_pointer() {
        Some(FrameControl::Resize(edges)) => Some(edges),
        _ => None,
    }
}

on_both_kinds! {
    fn close_prelights_under_the_pointer_presses_and_closes_on_release(kind: Kind) {
        let (mut f, window) = framed(kind);
        let close = center(f.button_rect(window, ButtonFunction::Close));

        move_to(&mut f, close);
        assert_eq!(
            f.button_state(window, ButtonFunction::Close),
            ButtonState::Prelight
        );

        f.pointer_button(BTN_LEFT, true);
        f.sync(window);
        assert_eq!(
            f.button_state(window, ButtonFunction::Close),
            ButtonState::Pressed
        );
        assert!(!f.close_requested(window));

        f.pointer_button(BTN_LEFT, false);
        f.sync(window);
        assert!(f.close_requested(window));
        assert_eq!(
            f.button_state(window, ButtonFunction::Close),
            ButtonState::Prelight
        );
    }
}

on_both_kinds! {
    fn releasing_off_the_button_cancels_the_click(kind: Kind) {
        let (mut f, window) = framed(kind);
        let close = f.button_rect(window, ButtonFunction::Close);
        let minimize = center(f.button_rect(window, ButtonFunction::Minimize));
        let title = center(close) - Point::from((150., 0.));

        move_to(&mut f, center(close));
        f.pointer_button(BTN_LEFT, true);
        // Off the button it's drawn normal, and no other button lights up.
        move_to(&mut f, minimize);
        assert_eq!(
            f.button_state(window, ButtonFunction::Close),
            ButtonState::Normal
        );
        assert_eq!(
            f.button_state(window, ButtonFunction::Minimize),
            ButtonState::Normal
        );
        // Back on it, it's pressed again.
        move_to(&mut f, center(close));
        assert_eq!(
            f.button_state(window, ButtonFunction::Close),
            ButtonState::Pressed
        );
        move_to(&mut f, title);
        f.pointer_button(BTN_LEFT, false);
        f.sync(window);

        assert!(!f.close_requested(window));
        assert_eq!(
            f.button_state(window, ButtonFunction::Close),
            ButtonState::Normal
        );
        assert!(!policy(&f).is_minimized(f.pid(window)));
    }
}

on_both_kinds! {
    fn minimise_minimises_and_maximise_toggles_both_ways_or_one_by_the_mouse_button(kind: Kind) {
        let (mut f, window) = every_button(kind);
        let pid = f.pid(window);

        click_button(&mut f, window, ButtonFunction::Maximize, BTN_LEFT);
        assert!(policy(&f).is_maximized(pid));
        click_button(&mut f, window, ButtonFunction::Maximize, BTN_LEFT);
        assert!(!policy(&f).is_maximized_horizontally(pid));

        click_button(&mut f, window, ButtonFunction::Maximize, BTN_MIDDLE);
        assert!(policy(&f).is_maximized_vertically(pid));
        assert!(!policy(&f).is_maximized_horizontally(pid));
        click_button(&mut f, window, ButtonFunction::Maximize, BTN_MIDDLE);
        click_button(&mut f, window, ButtonFunction::Maximize, BTN_RIGHT);
        assert!(policy(&f).is_maximized_horizontally(pid));
        assert!(!policy(&f).is_maximized_vertically(pid));
        click_button(&mut f, window, ButtonFunction::Maximize, BTN_RIGHT);

        click_button(&mut f, window, ButtonFunction::Minimize, BTN_LEFT);
        assert!(policy(&f).is_minimized(pid));
    }
}

on_both_kinds! {
    fn shade_shades_and_unshade_unshades(kind: Kind) {
        let (mut f, window) = every_button(kind);
        let pid = f.pid(window);

        click_button(&mut f, window, ButtonFunction::Shade, BTN_LEFT);
        assert!(policy(&f).is_shaded(pid));
        click_button(&mut f, window, ButtonFunction::Unshade, BTN_LEFT);
        assert!(!policy(&f).is_shaded(pid));
    }
}

on_both_kinds! {
    fn menu_above_and_stick_light_up_but_do_nothing_yet(kind: Kind) {
        let (mut f, window) = every_button(kind);
        let before = f.geometry(window);

        for function in [
            ButtonFunction::Menu,
            ButtonFunction::Above,
            ButtonFunction::Stick,
        ] {
            let point = center(f.button_rect(window, function));
            move_to(&mut f, point);
            assert_eq!(f.button_state(window, function), ButtonState::Prelight);
            f.pointer_button(BTN_LEFT, true);
            assert_eq!(f.button_state(window, function), ButtonState::Pressed);
            f.pointer_button(BTN_LEFT, false);
        }
        f.sync(window);

        let pid = f.pid(window);
        assert!(!policy(&f).is_minimized(pid) && !policy(&f).is_maximized(pid));
        assert_eq!(f.geometry(window), before);
        assert!(!f.close_requested(window));
    }
}

on_both_kinds! {
    fn dragging_the_titlebar_past_the_drag_threshold_moves_the_window(kind: Kind) {
        let (mut f, window) = framed(kind);
        let start = f.geometry(window);
        let title = title_point(&f, window);

        // drag-threshold is 8 by default.
        move_to(&mut f, title);
        f.pointer_button(BTN_LEFT, true);
        move_to(&mut f, title + Point::from((8., 0.)));
        assert_eq!(f.geometry(window), start);

        move_to(&mut f, title + Point::from((100., 50.)));
        let moved = start.loc + Point::from((100, 50));
        assert_eq!(f.geometry(window).loc, moved);
        f.pointer_button(BTN_LEFT, false);
        move_to(&mut f, title);
        assert_eq!(f.geometry(window).loc, moved);
    }
}

on_both_kinds! {
    fn pressing_the_titlebar_focuses_and_raises_the_window(kind: Kind) {
        let (mut f, window) = framed(kind);
        let pid = f.pid(window);
        let title = title_point(&f, window);
        f.map(
            window.client(),
            WindowSpec {
                size: (100, 100),
                ..WindowSpec::default()
            },
        );
        assert_ne!(policy(&f).focus(crate::state::SEAT), Some(pid));

        move_to(&mut f, title);
        f.pointer_button(BTN_LEFT, true);
        f.pointer_button(BTN_LEFT, false);

        assert_eq!(policy(&f).focus(crate::state::SEAT), Some(pid));
        assert_eq!(policy(&f).stacking(0).last(), Some(&pid));
    }
}

on_both_kinds! {
    fn double_clicking_the_titlebar_runs_its_action(kind: Kind) {
        let (mut f, window) = titlebar_with(kind, "double-click", "toggle_maximize");
        let pid = f.pid(window);
        double_click_title(&mut f, window);
        assert!(policy(&f).is_maximized(pid));
        double_click_title(&mut f, window);
        assert!(!policy(&f).is_maximized(pid));

        let (mut f, window) = titlebar_with(kind, "double-click", "toggle_maximize_horizontally");
        let pid = f.pid(window);
        double_click_title(&mut f, window);
        assert!(policy(&f).is_maximized_horizontally(pid));
        assert!(!policy(&f).is_maximized_vertically(pid));

        let (mut f, window) = titlebar_with(kind, "double-click", "toggle_maximize_vertically");
        let pid = f.pid(window);
        double_click_title(&mut f, window);
        assert!(policy(&f).is_maximized_vertically(pid));
        assert!(!policy(&f).is_maximized_horizontally(pid));

        let (mut f, window) = titlebar_with(kind, "double-click", "toggle_shade");
        let pid = f.pid(window);
        double_click_title(&mut f, window);
        assert!(policy(&f).is_shaded(pid));
        double_click_title(&mut f, window);
        assert!(!policy(&f).is_shaded(pid));

        let (mut f, window) = titlebar_with(kind, "double-click", "minimize");
        let pid = f.pid(window);
        double_click_title(&mut f, window);
        assert!(policy(&f).is_minimized(pid));

        let (mut f, window) = titlebar_with(kind, "double-click", "none");
        let pid = f.pid(window);
        let before = f.geometry(window);
        double_click_title(&mut f, window);
        let p = policy(&f);
        assert!(!p.is_maximized_horizontally(pid) && !p.is_maximized_vertically(pid));
        assert!(!p.is_shaded(pid) && !p.is_minimized(pid));
        assert_eq!(f.geometry(window), before);
    }
}

on_both_kinds! {
    fn two_clicks_further_apart_than_the_double_click_time_are_two_clicks(kind: Kind) {
        let (mut f, window) = titlebar_with(kind, "double-click", "toggle_maximize");
        let title = title_point(&f, window);
        move_to(&mut f, title);

        f.pointer_button(BTN_LEFT, true);
        f.pointer_button(BTN_LEFT, false);
        // double-click is 400 ms by default; each input event is 10 ms after the last.
        for _ in 0..40 {
            move_to(&mut f, title);
        }
        f.pointer_button(BTN_LEFT, true);
        f.pointer_button(BTN_LEFT, false);

        assert!(!policy(&f).is_maximized(f.pid(window)));
    }
}

on_both_kinds! {
    fn middle_and_right_clicks_on_the_titlebar_run_their_actions(kind: Kind) {
        let (mut f, window) = titlebar_with(kind, "middle-click", "lower");
        let lowered = f.pid(window);
        let other = f.map(
            window.client(),
            WindowSpec {
                size: (100, 100),
                ..WindowSpec::default()
            },
        );
        let other = f.pid(other);
        f.state.caminito.policy.raise(lowered);
        assert_eq!(policy(&f).stacking(0), [other, lowered]);
        let title = title_point(&f, window);
        move_to(&mut f, title);
        f.pointer_button(BTN_MIDDLE, true);
        f.pointer_button(BTN_MIDDLE, false);
        assert_eq!(policy(&f).stacking(0), [lowered, other]);

        let (mut f, window) = titlebar_with(kind, "right-click", "toggle_maximize");
        let title = title_point(&f, window);
        move_to(&mut f, title);
        f.pointer_button(BTN_RIGHT, true);
        f.pointer_button(BTN_RIGHT, false);
        assert!(policy(&f).is_maximized(f.pid(window)));
    }
}

on_both_kinds! {
    fn dragging_each_border_and_corner_resizes_from_it(kind: Kind) {
        let (mut f, window) = framed(kind);
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
            let frame = f.visible_frame(window).to_f64();
            let before = f.geometry(window);
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
            f.draw_at_asked_size(window, size);
            f.pointer_button(BTN_LEFT, false);
            f.sync(window);

            // The opposite edges stayed put.
            let after = f.geometry(window);
            let edge = format!("resizing from ({dx}, {dy})");
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

on_both_kinds! {
    fn the_corners_resize_zones_reach_along_the_sides_as_marcos_do(kind: Kind) {
        let (mut f, window) = framed(kind);
        let frame = f.visible_frame(window).to_f64();
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
}

on_both_kinds! {
    fn a_maximised_frame_has_no_resize_border_where_it_meets_the_output(kind: Kind) {
        let (mut f, window) = framed(kind);
        click_button(&mut f, window, ButtonFunction::Maximize, BTN_LEFT);
        let e = f.frame_extents(window);
        f.draw_at_asked_size(window, (1920 - e.left - e.right, 1080 - e.top - e.bottom));
        assert!(policy(&f).is_maximized(f.pid(window)));
        let before = f.geometry(window);

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
            assert_eq!(f.geometry(window), before, "dragged from ({x}, {y})");
        }
    }
}

on_both_kinds! {
    fn a_shaded_window_shows_only_its_titlebar_and_keeps_the_focus_but_not_the_keyboard(
        kind: Kind
    ) {
        let (mut f, window) = titlebar_with(kind, "double-click", "toggle_shade");
        let pid = f.pid(window);
        let output = f.state.caminito.outputs[0].clone();
        let inside = center(f.geometry(window));
        let pixel_inside = |f: &mut Fixture| {
            let pixels = f.state.backend.headless_mut().pixels(&output).unwrap();
            pixels[(inside.y as usize) * 1920 + inside.x as usize]
        };
        assert_eq!(pixel_inside(&mut f), WINDOW_COLOR);

        double_click_title(&mut f, window);

        assert!(policy(&f).is_shaded(pid));
        assert_eq!(policy(&f).focus(crate::state::SEAT), Some(pid));
        let keyboard = f.state.caminito.seat.get_keyboard().unwrap();
        assert_eq!(keyboard.current_focus(), None);
        assert_eq!(pixel_inside(&mut f), argb8888(CLEAR_COLOR));
        let strips: Vec<_> = f
            .mapped(window)
            .frame()
            .strips()
            .map(|(rect, _)| rect)
            .collect();
        assert_eq!(strips.len(), 1, "only the titlebar: {strips:?}");
        // The pointer finds nothing where the client was.
        move_to(&mut f, inside);
        assert_eq!(f.state.caminito.window_under(inside), None);

        double_click_title(&mut f, window);
        assert!(!policy(&f).is_shaded(pid));
        assert_eq!(pixel_inside(&mut f), WINDOW_COLOR);
        let keyboard = f.state.caminito.seat.get_keyboard().unwrap();
        assert!(keyboard.current_focus().is_some());
    }
}

on_both_kinds! {
    fn a_csd_window_cant_be_shaded(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let window = f.map(
            client,
            WindowSpec {
                decoration: Decoration::Client,
                ..WindowSpec::default()
            },
        );
        let pid = f.pid(window);

        f.state.caminito.policy.set_shaded(pid, true);

        assert!(!policy(&f).is_shaded(pid));
    }
}

on_both_kinds! {
    fn the_pointer_goes_through_the_rounded_corners(kind: Kind) {
        let (f, window) = framed(kind);
        let pid = f.pid(window);
        let frame = f.visible_frame(window).to_f64();

        // Menta rounds its top corners with a radius of 4, which cuts the corner pixel out.
        let corner = frame.loc + Point::from((0.5, 0.5));
        assert_eq!(f.state.caminito.window_under(corner), None);
        let beside = corner + Point::from((6., 0.));
        assert_eq!(f.state.caminito.window_under(beside), Some(pid));
        let right_corner = Point::from((frame.loc.x + frame.size.w - 0.5, corner.y));
        assert_eq!(f.state.caminito.window_under(right_corner), None);
    }
}

on_both_kinds! {
    fn any_mouse_button_on_a_maximised_windows_button_unmaximises_it(kind: Kind) {
        let (mut f, window) = every_button(kind);
        let pid = f.pid(window);

        for button in [BTN_MIDDLE, BTN_RIGHT] {
            click_button(&mut f, window, ButtonFunction::Maximize, BTN_LEFT);
            assert!(policy(&f).is_maximized(pid));
            click_button(&mut f, window, ButtonFunction::Maximize, button);
            let p = policy(&f);
            assert!(!p.is_maximized_horizontally(pid) && !p.is_maximized_vertically(pid));
        }
    }
}
