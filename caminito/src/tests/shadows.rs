//! Shadows (07, 171): Marco's compositor shadow around the windows Caminito frames, on the
//! output and nowhere else. Its geometry and when it's drawn again have unit tests beside
//! `decoration::shadow` and the frame.

use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle};

use super::fixture::{
    BTN_LEFT, ClientWindow, Decoration, Fixture, Kind, WINDOW_COLOR, WindowSpec, on_both_kinds,
};
use crate::backend::headless::argb8888;
use crate::scene::CLEAR_COLOR;
use crate::settings::Value;
use crate::settings::translate::MARCO_GENERAL;
use crate::state::SEAT;

/// How far the large shadow reaches right of and below a frame, and the medium one below.
const LARGE_REACH: i32 = 21;

/// One output at `scale` and a 400 × 300 window from a client of `kind`, decorated as
/// `decoration` asks, under a Caminito with `settings` from the start. The pointer is
/// clear of the window.
fn window_with(
    kind: Kind,
    settings: Vec<(&'static str, String, Value)>,
    decoration: Decoration,
    scale: f64,
) -> (Fixture, Output, ClientWindow) {
    let mut f = Fixture::with_settings(settings);
    let output = f.add_output((1920, 1080), scale);
    let client = f.connect(kind);
    let window = f.map(
        client,
        WindowSpec {
            size: (400, 300),
            decoration,
            ..WindowSpec::default()
        },
    );
    f.pointer_move(1200., 700.);
    (f, output, window)
}

fn framed(kind: Kind) -> (Fixture, Output, ClientWindow) {
    window_with(kind, Vec::new(), Decoration::Server, 1.0)
}

fn compositing_manager(on: bool) -> (&'static str, String, Value) {
    (MARCO_GENERAL, "compositing-manager".into(), Value::Bool(on))
}

/// `output`'s pixel at a logical point, as `0xRRGGBB`, drawn afresh.
fn pixel(f: &mut Fixture, output: &Output, at: Point<i32, Logical>) -> u32 {
    f.draw(output);
    let scale = output.current_scale().fractional_scale();
    let at = at.to_physical_precise_round(scale);
    let pixel = f.state.backend.headless_mut().pixel(output, at);
    pixel.expect("the output is drawn") & 0xff_ffff
}

fn clear() -> u32 {
    argb8888(CLEAR_COLOR) & 0xff_ffff
}

/// Whether a pixel is the background with black over it.
fn is_shadowed(pixel: u32) -> bool {
    let clear = clear();
    let channels = |pixel: u32| [pixel >> 16 & 0xff, pixel >> 8 & 0xff, pixel & 0xff];
    channels(pixel)
        .into_iter()
        .zip(channels(clear))
        .all(|(got, clear)| got < clear)
}

/// The points beside a frame's right side and below its bottom, at each of `distances`
/// from it.
fn beside(
    visible: Rectangle<i32, Logical>,
    distances: std::ops::Range<i32>,
) -> Vec<Point<i32, Logical>> {
    let right = visible.loc.x + visible.size.w;
    let bottom = visible.loc.y + visible.size.h;
    let middle = visible.loc + visible.size.downscale(2).to_point();
    distances
        .flat_map(|distance| {
            [
                Point::from((right + distance, middle.y)),
                Point::from((middle.x, bottom + distance)),
            ]
        })
        .collect()
}

on_both_kinds! {
    fn a_framed_window_has_its_shadow_outside_its_frame(kind: Kind) {
        let (mut f, output, window) = framed(kind);
        let visible = f.visible_frame(window);
        let right = visible.loc.x + visible.size.w;
        let middle = visible.loc + visible.size.downscale(2).to_point();

        // The focused frame's shadow reaches 21 right of it and below, and fades.
        let near = pixel(&mut f, &output, (right + 2, middle.y).into());
        let far = pixel(&mut f, &output, (right + 14, middle.y).into());
        assert!(is_shadowed(near), "{near:06x} beside the frame");
        assert!(is_shadowed(far), "{far:06x} further from it");
        assert!(near < far, "{near:06x} is darker than {far:06x}");
        let below = Point::from((middle.x, visible.loc.y + visible.size.h + 2));
        assert!(is_shadowed(pixel(&mut f, &output, below)));
        let past = Point::from((right + LARGE_REACH, middle.y));
        assert_eq!(pixel(&mut f, &output, past), clear(), "past the shadow");

        // The window and its frame are as they'd be with no shadow.
        let geometry = f.geometry(window);
        let content = geometry.loc + geometry.size.downscale(2).to_point();
        assert_eq!(pixel(&mut f, &output, content), WINDOW_COLOR & 0xff_ffff);
        assert_eq!(
            f.mapped(window).frame().shadow_rect().map(|rect| rect.size),
            Some((visible.size.w + 36, visible.size.h + 36).into())
        );
    }
}

on_both_kinds! {
    fn an_unfocused_frame_has_the_smaller_shadow(kind: Kind) {
        let (mut f, output, window) = framed(kind);
        let other = f.connect(Kind::Xdg);
        f.map(other, WindowSpec { size: (100, 100), ..WindowSpec::default() });
        assert_ne!(f.state.caminito.policy.focus(SEAT), Some(f.pid(window)));
        let visible = f.visible_frame(window);
        let right = visible.loc.x + visible.size.w;
        let middle = visible.loc.y + visible.size.h / 2;

        // The medium shadow reaches 9 right of the frame.
        assert!(is_shadowed(pixel(&mut f, &output, (right + 2, middle).into())));
        assert_eq!(pixel(&mut f, &output, (right + 9, middle).into()), clear());
    }
}

on_both_kinds! {
    fn a_csd_window_has_no_shadow(kind: Kind) {
        let (mut f, output, window) = window_with(kind, Vec::new(), Decoration::Client, 1.0);
        let geometry = f.geometry(window);

        assert_eq!(f.mapped(window).frame().shadow_rect(), None);
        for at in beside(geometry, 0..LARGE_REACH) {
            assert_eq!(pixel(&mut f, &output, at), clear(), "at {at:?}");
        }
    }
}

on_both_kinds! {
    fn there_are_no_shadows_with_the_compositing_manager_off(kind: Kind) {
        let settings = vec![compositing_manager(false)];
        let (mut f, output, window) = window_with(kind, settings, Decoration::Server, 1.0);
        let visible = f.visible_frame(window);

        for at in beside(visible, 0..LARGE_REACH) {
            assert_eq!(pixel(&mut f, &output, at), clear(), "at {at:?}");
        }

        // The key is live, both ways.
        let (schema, key, _) = compositing_manager(true);
        f.set_setting(schema, &key, Value::Bool(true));
        f.dispatch_until(|f| f.mapped(window).frame().shadow_rect().is_some());
        let beside = beside(visible, 0..3);
        for &at in &beside {
            assert!(is_shadowed(pixel(&mut f, &output, at)), "at {at:?}");
        }

        f.set_setting(schema, &key, Value::Bool(false));
        f.dispatch_until(|f| f.mapped(window).frame().shadow_rect().is_none());
        for at in beside {
            assert_eq!(pixel(&mut f, &output, at), clear(), "at {at:?}");
        }
    }
}

on_both_kinds! {
    fn a_shadow_is_drawn_at_a_fractional_scale(kind: Kind) {
        let (mut f, output, window) = window_with(kind, Vec::new(), Decoration::Server, 1.5);
        let visible = f.visible_frame(window);

        // The frame's strips are drawn at the scale rounded up (13), and its shadow with
        // them. A strip a logical pixel high is two physical ones here, so the frame can
        // cover the first row of the shadow beside it.
        assert_eq!(f.mapped(window).frame().scale(), 2);
        for at in beside(visible, 1..4) {
            let pixel = pixel(&mut f, &output, at);
            assert!(is_shadowed(pixel), "{pixel:06x} at {at:?}, beside {visible:?}");
        }
        let right = visible.loc.x + visible.size.w;
        let past = Point::from((right + LARGE_REACH, visible.loc.y + visible.size.h / 2));
        assert_eq!(pixel(&mut f, &output, past), clear());
    }
}

on_both_kinds! {
    fn a_click_on_a_shadow_goes_to_the_window_under_it(kind: Kind) {
        let mut f = Fixture::new();
        f.add_output((1920, 1080), 1.0);
        let client = f.connect(kind);
        let under = f.map(client, WindowSpec { size: (800, 600), ..WindowSpec::default() });
        let over = f.map(client, WindowSpec { size: (200, 150), ..WindowSpec::default() });

        // Drag the small window by its titlebar onto the large one.
        let geometry = f.geometry(over);
        let top = f.frame_extents(over).top;
        let title: Point<i32, Logical> =
            (geometry.loc.x + geometry.size.w / 2, geometry.loc.y - top / 2).into();
        let target = f.geometry(under);
        let target = target.loc + target.size.downscale(2).to_point();
        f.pointer_move(f64::from(title.x), f64::from(title.y));
        f.pointer_button(BTN_LEFT, true);
        f.pointer_move(f64::from(target.x), f64::from(target.y));
        f.pointer_button(BTN_LEFT, false);
        let (under_id, over_id) = (f.pid(under), f.pid(over));
        assert_eq!(f.state.caminito.policy.focus(SEAT), Some(over_id));

        // A point on its shadow, past its invisible resize border, over the large window.
        let visible = f.visible_frame(over);
        let at = Point::from((
            visible.loc.x + visible.size.w + 15,
            visible.loc.y + visible.size.h / 2,
        ));
        let shadow = f.mapped(over).frame().shadow_rect().unwrap();
        let shadow = Rectangle::new(shadow.loc + f.geometry(over).loc, shadow.size);
        assert!(shadow.contains(at), "{at:?} is on the shadow {shadow:?}");
        assert!(f.geometry(under).contains(at), "and over the window under it");

        f.pointer_move(f64::from(at.x), f64::from(at.y));
        assert_eq!(f.control_under_pointer(), None, "the shadow is no part of the frame");
        f.pointer_button(BTN_LEFT, true);
        f.pointer_button(BTN_LEFT, false);

        assert_eq!(f.state.caminito.policy.focus(SEAT), Some(under_id));
        assert!(f.has_keyboard(under));
    }
}
