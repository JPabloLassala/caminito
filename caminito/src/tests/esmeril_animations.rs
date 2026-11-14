//! Esmeril where a window is drawn somewhere other than its place (ADR 0048, CMN-236): it goes
//! with the window, scaled and faded as it is, through the Opening, Maximise and Closing
//! animations and a Workspace slide, and a Popup slide leaves the window's as it is. None of a
//! window's goes into the Overview's thumbnails. They're read on Pixman, where Esmeril draws
//! nothing, from the elements of the scene; image_capture.rs has the captures' pixels, and
//! minimise_animation.rs the minimise animation's.
//!
//! The window is xdg's: a frame's strips are drawn from its theme alone, whichever kind of
//! window there is, and the animations draw both kinds alike.

use std::time::Duration;

use smithay::backend::renderer::element::Element;
use smithay::output::Output;
use smithay::utils::{Logical, Physical, Rectangle};

use super::esmeril::esmerils;
use super::fixture::{
    ClientId, ClientWindow, Fixture, KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT, Kind, WindowSpec,
};
use super::maximise_animation::{self, asked_size};
use super::open_close_animation::{self, Close, PART_WAY, close, closing, map_opened, opening};
use super::overview;
use super::popup_slide::the_slide;
use super::window_menu::alt_space;
use super::workspace_slide::slides;
use crate::backend::headless::Drawing;
use crate::esmeril::Esmeril;
use crate::open_close_animation::LENGTH;
use crate::scene::{self, SceneElement};
use crate::settings::Value;
use crate::settings::translate::{CAMINITO, MARCO_GENERAL};

const OUTPUT: (i32, i32) = (800, 600);

/// The settings of a centred window whose frame has one see-through strip, with `key` of
/// `org.mate.Caminito` on.
fn settings(key: &str) -> Vec<(&'static str, String, Value)> {
    themed("EsmerilSeeThrough", key)
}

/// As `settings`, with the frame drawn from `theme`.
fn themed(theme: &str, key: &str) -> Vec<(&'static str, String, Value)> {
    vec![
        (MARCO_GENERAL, "theme".into(), Value::Str(theme.into())),
        (
            MARCO_GENERAL,
            "center-new-windows".into(),
            Value::Bool(true),
        ),
        (CAMINITO, key.into(), Value::Bool(true)),
    ]
}

/// One output and a client, with `key` on and the clock stopped.
fn desk(key: &str) -> (Fixture, ClientId, Output) {
    desk_at(key, 1.0)
}

/// As `desk`, with the output at `scale`.
fn desk_at(key: &str, scale: f64) -> (Fixture, ClientId, Output) {
    let mut f = Fixture::with_settings(settings(key));
    let output = f.add_output(OUTPUT, scale);
    let id = f.connect(Kind::Xdg);
    f.stop_clock();
    (f, id, output)
}

fn spec() -> WindowSpec {
    WindowSpec {
        size: (300, 200),
        ..WindowSpec::default()
    }
}

/// The one Esmeril element on `output`.
#[track_caller]
fn the_esmeril(f: &mut Fixture, output: &Output) -> Esmeril {
    let elements = esmerils(f, output);
    let [esmeril] = elements.as_slice() else {
        panic!("one strip's: {elements:?}");
    };
    esmeril.clone()
}

/// Where a strip at `strip` on a window whose frame is at `frame` is when the frame is drawn
/// at `drawn`: scaled with it, from its corner.
fn scaled(
    strip: Rectangle<i32, Physical>,
    frame: Rectangle<i32, Logical>,
    drawn: Rectangle<i32, Logical>,
) -> Rectangle<f64, Physical> {
    scaled_at(strip, frame, drawn, 1.0)
}

/// As `scaled`, on an output at `scale`.
fn scaled_at(
    strip: Rectangle<i32, Physical>,
    frame: Rectangle<i32, Logical>,
    drawn: Rectangle<i32, Logical>,
    scale: f64,
) -> Rectangle<f64, Physical> {
    let (x, y) = (
        f64::from(drawn.size.w) / f64::from(frame.size.w),
        f64::from(drawn.size.h) / f64::from(frame.size.h),
    );
    let frame = frame.to_f64().to_physical(scale);
    let drawn = drawn.to_f64().to_physical(scale);
    Rectangle::new(
        (
            drawn.loc.x + (f64::from(strip.loc.x) - frame.loc.x) * x,
            drawn.loc.y + (f64::from(strip.loc.y) - frame.loc.y) * y,
        )
            .into(),
        (f64::from(strip.size.w) * x, f64::from(strip.size.h) * y).into(),
    )
}

/// `got` is `want` to within a pixel each way, as each is rounded.
#[track_caller]
fn assert_near(got: Rectangle<i32, Physical>, want: Rectangle<f64, Physical>) {
    let near = |got: i32, want: f64| (f64::from(got) - want).abs() <= 1.0;
    assert!(
        near(got.loc.x, want.loc.x)
            && near(got.loc.y, want.loc.y)
            && near(got.size.w, want.size.w)
            && near(got.size.h, want.size.h),
        "{got:?} and not {want:?}"
    );
}

#[track_caller]
fn assert_alpha(esmeril: &Esmeril, want: f64) {
    let got = f64::from(esmeril.alpha());
    assert!((got - want).abs() < 1e-3, "alpha {got} and not {want}");
}

/// The see-through strip's Esmeril at rest, and the window's frame.
fn at_rest(
    f: &mut Fixture,
    output: &Output,
    window: ClientWindow,
) -> (Esmeril, Rectangle<i32, Logical>) {
    f.draw(output);
    (the_esmeril(f, output), f.visible_frame(window))
}

#[test]
fn a_strips_esmeril_grows_and_fades_in_with_its_opening_window() {
    let (mut f, id, output) = desk("open-close-animation");
    let window = f.map(id, spec());
    f.advance_clock(PART_WAY);
    let [(_, drawn, alpha)] = opening(&f)[..] else {
        panic!("it opens: {:?}", opening(&f));
    };
    assert!(alpha > 0.0 && alpha < 1.0, "part way: {alpha}");
    let part_way = the_esmeril(&mut f, &output);

    f.advance_clock(LENGTH);
    let (rest, frame) = at_rest(&mut f, &output, window);
    assert_ne!(drawn, frame, "smaller part way");
    assert_near(part_way.strip(), scaled(rest.strip(), frame, drawn));
    assert_alpha(&part_way, alpha);
    assert_alpha(&rest, 1.0);
}

#[test]
fn at_an_output_scale_of_1_5_a_strips_esmeril_opens_where_its_strip_is_drawn() {
    let (mut f, id, output) = desk_at("open-close-animation", 1.5);
    let window = f.map(id, spec());
    f.advance_clock(PART_WAY);
    let [(_, drawn, _)] = opening(&f)[..] else {
        panic!("it opens: {:?}", opening(&f));
    };
    let part_way = the_esmeril(&mut f, &output);

    f.advance_clock(LENGTH);
    let (rest, frame) = at_rest(&mut f, &output, window);
    // The opening rounds its rectangle to a logical pixel, which is 1.5 physical ones.
    let want = scaled_at(rest.strip(), frame, drawn, 1.5);
    let near = |got: i32, want: f64| (f64::from(got) - want).abs() <= 1.5;
    let got = part_way.strip();
    assert!(
        near(got.loc.x, want.loc.x)
            && near(got.loc.y, want.loc.y)
            && near(got.size.w, want.size.w)
            && near(got.size.h, want.size.h),
        "{got:?} and not {want:?}"
    );
}

#[test]
fn a_strips_esmeril_scales_with_its_window_into_the_maximise_animations_box() {
    let (mut f, id, output) = desk("maximize-animation");
    let window = maximise_animation::floating(&mut f, id);
    let (rest, frame) = at_rest(&mut f, &output, window);

    f.state.caminito.policy.set_maximized(f.pid(window), true);
    f.dispatch();
    f.advance_clock(maximise_animation::PART_WAY);
    let (drawn, _) = maximise_animation::drawn(&f, window);
    assert_ne!(drawn, frame, "on its way");
    // The live window, under its picture as it was, which fades: its Esmeril is whole.
    let part_way = the_esmeril(&mut f, &output);
    assert_near(part_way.strip(), scaled(rest.strip(), frame, drawn));
    assert_alpha(&part_way, 1.0);

    let size = asked_size(&f, window);
    f.draw_at_asked_size(window, size);
    f.advance_clock(crate::maximize_animation::LENGTH);
    let (maximised, frame) = at_rest(&mut f, &output, window);
    assert_eq!(maximised.strip().loc, frame.loc.to_physical(1));
}

#[test]
fn a_strips_esmeril_shrinks_and_fades_out_behind_its_closing_windows_picture() {
    let (mut f, id, output) = desk("open-close-animation");
    let window = map_opened(&mut f, id, spec());
    let (rest, frame) = at_rest(&mut f, &output, window);

    close(&mut f, window, Close::Destroy);
    f.advance_clock(PART_WAY);
    let [ref picture] = closing(&f)[..] else {
        panic!("it closes: {:?}", closing(&f));
    };
    assert!(picture.alpha > 0.0 && picture.alpha < 1.0, "part way");
    let part_way = the_esmeril(&mut f, &output);
    assert_near(part_way.strip(), scaled(rest.strip(), frame, picture.rect));
    assert_alpha(&part_way, picture.alpha);
    assert_eq!(part_way.id(), rest.id(), "it keeps its blur");

    // Right behind the picture.
    let Drawing::Pixman(renderer, _) = f.state.backend.headless_mut().drawing() else {
        panic!("it draws with pixman");
    };
    let elements = scene::output_elements(renderer, &f.state.caminito, &output);
    let at = elements
        .iter()
        .position(|element| matches!(element, SceneElement::Esmeril(_)))
        .unwrap();
    assert!(matches!(elements[at - 1], SceneElement::Picture(_)));

    f.advance_clock(LENGTH);
    assert!(esmerils(&mut f, &output).is_empty(), "gone with it");
}

#[test]
fn a_listed_windows_esmeril_grows_with_it_and_goes_as_it_starts_to_close() {
    let mut f = Fixture::with_settings(themed("EsmerilSolid", "open-close-animation"));
    let output = f.add_output(OUTPUT, 1.0);
    let id = f.connect(Kind::Xdg);
    f.stop_clock();
    let listed = WindowSpec {
        app_id: Some("mate-terminal"),
        ..spec()
    };
    let window = f.map(id, listed);
    f.advance_clock(PART_WAY);
    let [(_, drawn, alpha)] = opening(&f)[..] else {
        panic!("it opens: {:?}", opening(&f));
    };
    let part_way = the_esmeril(&mut f, &output);

    f.advance_clock(LENGTH);
    let (rest, frame) = at_rest(&mut f, &output, window);
    assert_near(part_way.strip(), scaled(rest.strip(), frame, drawn));
    assert_alpha(&part_way, alpha);

    // Its surface has no buffer left to read its alpha from.
    close(&mut f, window, Close::Destroy);
    assert_eq!(closing(&f).len(), 1);
    assert!(esmerils(&mut f, &output).is_empty());
}

#[test]
fn with_esmeril_off_a_closing_window_has_none() {
    let (mut f, id, output) = desk("open-close-animation");
    let window = map_opened(&mut f, id, spec());
    f.set_setting(CAMINITO, "esmeril", Value::Bool(false));
    f.dispatch_until(|f| f.state.caminito.settings.esmeril.is_none());
    close(&mut f, window, Close::Destroy);
    f.advance_clock(PART_WAY);
    assert_eq!(closing(&f).len(), 1);
    assert!(esmerils(&mut f, &output).is_empty());
}

#[test]
fn a_strips_esmeril_slides_with_its_window_and_workspace() {
    let (mut f, id, output) = desk("workspace-slide");
    let window = f.map(id, spec());
    let (rest, frame) = at_rest(&mut f, &output, window);

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    f.advance_clock(Duration::from_millis(50));
    let slide = &slides(&f)[0];
    let pid = f.pid(window).0;
    let [(_, drawn)] = slide.windows[..] else {
        panic!("it slides: {slide:?}");
    };
    assert_eq!(slide.windows[0].0, pid);
    assert_ne!(drawn, frame, "on its way");
    let part_way = the_esmeril(&mut f, &output);
    let mut moved = rest.strip();
    moved.loc += (drawn.loc - frame.loc).to_physical(1);
    assert_eq!(part_way.strip(), moved);
    assert_alpha(&part_way, 1.0);
}

#[test]
fn a_popup_slide_leaves_its_windows_esmeril_as_it_is() {
    let mut settings = settings("open-close-animation");
    settings.push((
        CAMINITO,
        "popup-animation".into(),
        Value::Str("slide".into()),
    ));
    let mut f = Fixture::with_settings(settings);
    let output = f.add_output(OUTPUT, 1.0);
    let id = f.connect(Kind::Xdg);
    f.stop_clock();
    let window = map_opened(&mut f, id, spec());
    let (rest, _) = at_rest(&mut f, &output, window);

    alt_space(&mut f);
    f.advance_clock(open_close_animation::PART_WAY);
    assert!(the_slide(&f).alpha < 1.0, "the menu slides");
    let elements = esmerils(&mut f, &output);
    let window_s = elements
        .iter()
        .find(|esmeril| esmeril.id() == rest.id())
        .expect("the window keeps its Esmeril");
    assert_eq!(window_s.strip(), rest.strip());
    assert_alpha(window_s, 1.0);
}

#[test]
fn the_overviews_thumbnail_of_a_window_has_none_of_its_esmeril() {
    let (mut f, id, output) = desk("open-close-animation");
    let window = map_opened(&mut f, id, spec());
    let (rest, _) = at_rest(&mut f, &output, window);

    overview::open(&mut f);
    for step in [Duration::ZERO, PART_WAY, Duration::from_secs(1)] {
        f.advance_clock(step);
        let elements = esmerils(&mut f, &output);
        assert!(
            elements.iter().all(|esmeril| esmeril.id() != rest.id()),
            "{step:?} after it opens: {elements:?}"
        );
    }
}
