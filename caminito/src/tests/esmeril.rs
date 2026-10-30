//! Esmeril behind frames (ADR 0048): where it goes and its damage on Pixman, where it draws
//! nothing, and its pixels on GLES, skipped where surfaceless EGL can't be created.

use caminito_test_client::layer_shell::{Layer, LayerSpec};
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::Element;
use smithay::output::Output;
use smithay::utils::{Physical, Point, Rectangle, Scale};

use super::fixture::{Fixture, Kind, WindowSpec};
use crate::backend::headless::{Drawing, drawing};
use crate::esmeril::Esmeril;
use crate::scene::{self, SceneElement};
use crate::settings::Value;
use crate::settings::translate::{CAMINITO, MARCO_GENERAL};

const OUTPUT: (i32, i32) = (400, 300);
const BENEATH: u32 = 0xff_80_60_40;
const OTHER: u32 = 0xff_10_20_c0;

/// Scale 1 throughout, so that physical pixels are the logical ones.
const SCALE: Scale<f64> = Scale { x: 1.0, y: 1.0 };

/// The settings of a centred window under `theme`, with Esmeril at `strength`.
fn settings(theme: &str, strength: u32) -> Vec<(&'static str, String, Value)> {
    vec![
        (MARCO_GENERAL, "theme".into(), Value::Str(theme.into())),
        (
            MARCO_GENERAL,
            "center-new-windows".into(),
            Value::Bool(true),
        ),
        (CAMINITO, "esmeril-strength".into(), Value::Uint(strength)),
    ]
}

/// A window with a frame on an output of `OUTPUT`, in the middle of it.
fn desk(f: &mut Fixture) -> Output {
    let output = f.add_output(OUTPUT, 1.0);
    window(f, &output);
    output
}

/// As `desk`, with the background covered in `color`.
fn desk_over(f: &mut Fixture, color: u32) -> Output {
    let output = f.add_output(OUTPUT, 1.0);
    let id = f.connect(Kind::Xdg);
    f.map_layer(id, LayerSpec::covering(Layer::Background, "test"), color);
    window(f, &output);
    output
}

fn window(f: &mut Fixture, output: &Output) {
    let id = f.connect(Kind::Xdg);
    f.map(
        id,
        WindowSpec {
            size: (200, 100),
            ..WindowSpec::default()
        },
    );
    f.draw(output);
}

/// Each Esmeril element the scene has for `output`, front to back.
fn esmerils(f: &mut Fixture, output: &Output) -> Vec<Esmeril> {
    let drawing = f.state.backend.headless_mut().drawing();
    drawing!(drawing, renderer, _memory, _T => {
        scene::output_elements(renderer, &f.state.caminito, output)
            .into_iter()
            .filter_map(|element| match element {
                SceneElement::Esmeril(esmeril) => Some(esmeril),
                _ => None,
            })
            .collect()
    })
}

/// The channels of an ARGB8888 pixel, red first.
fn channels(pixel: u32) -> [i32; 3] {
    [(pixel >> 16) & 0xff, (pixel >> 8) & 0xff, pixel & 0xff].map(|c| c as i32)
}

/// The pixel at `at` on `output`.
fn pixel(f: &mut Fixture, output: &Output, at: Point<i32, Physical>) -> u32 {
    f.state
        .backend
        .headless_mut()
        .pixel(output, at)
        .expect("the output is drawn")
}

/// `color` as Esmeril shows it over a uniform stretch of it.
fn saturated_and_contrasted(color: u32) -> [f64; 3] {
    let [r, g, b] = channels(color).map(|c| f64::from(c) / 255.0);
    let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    [r, g, b].map(|c| ((luma + (c - luma) * 1.7) * 1.2 - 0.1).clamp(0.0, 1.0) * 255.0)
}

#[test]
fn a_see_through_strip_gets_esmeril_behind_it_where_the_blur_reads() {
    let mut f = Fixture::with_settings(settings("EsmerilGlass", 15));
    let output = desk(&mut f);
    let elements = esmerils(&mut f, &output);
    let [esmeril] = elements.as_slice() else {
        panic!("one strip is see-through: {elements:?}");
    };
    let strip = esmeril.strip();
    let read = Rectangle::new(
        strip.loc - Point::from((150, 150)),
        (strip.size.w + 300, strip.size.h + 300).into(),
    );
    let on_output = Rectangle::from_size(OUTPUT.into());
    assert_eq!(
        esmeril.geometry(SCALE),
        read.intersection(on_output).unwrap()
    );
    assert!(esmeril.is_framebuffer_effect());

    // Under the strip, and over what's beneath.
    let drawing = f.state.backend.headless_mut().drawing();
    let Drawing::Pixman(renderer, _) = drawing else {
        panic!("it draws with pixman");
    };
    let elements = scene::output_elements(renderer, &f.state.caminito, &output);
    let at = elements
        .iter()
        .position(|element| matches!(element, SceneElement::Esmeril(_)))
        .unwrap();
    assert!(matches!(elements[at - 1], SceneElement::Frame(_)));
}

#[test]
fn a_theme_with_no_see_through_pixels_gets_none() {
    let mut f = Fixture::with_settings(settings("EsmerilSolid", 15));
    let output = desk(&mut f);
    assert!(esmerils(&mut f, &output).is_empty());
}

#[test]
fn esmeril_off_removes_it_at_once_and_on_brings_it_back() {
    let mut f = Fixture::with_settings(settings("EsmerilGlass", 15));
    let output = desk(&mut f);
    f.set_setting(CAMINITO, "esmeril", Value::Bool(false));
    f.dispatch_until(|f| f.state.caminito.settings.esmeril.is_none());
    assert!(esmerils(&mut f, &output).is_empty());
    f.set_setting(CAMINITO, "esmeril", Value::Bool(true));
    f.dispatch_until(|f| f.state.caminito.settings.esmeril.is_some());
    assert_eq!(esmerils(&mut f, &output).len(), 1);
}

#[test]
fn reduced_resources_removes_it_at_once_and_animations_off_doesnt() {
    let mut f = Fixture::with_settings(settings("EsmerilGlass", 15));
    let output = desk(&mut f);
    f.set_setting(MARCO_GENERAL, "reduced-resources", Value::Bool(true));
    f.dispatch_until(|f| f.state.caminito.settings.esmeril.is_none());
    assert!(esmerils(&mut f, &output).is_empty());
    f.set_setting(MARCO_GENERAL, "reduced-resources", Value::Bool(false));
    f.dispatch_until(|f| f.state.caminito.settings.esmeril.is_some());
    f.set_setting(
        crate::settings::translate::INTERFACE,
        "enable-animations",
        Value::Bool(false),
    );
    f.dispatch_until(|f| !f.state.caminito.settings.minimize_animation);
    assert_eq!(esmerils(&mut f, &output).len(), 1);
}

#[test]
fn a_new_strength_changes_its_reach_and_damages_it_at_once() {
    let mut f = Fixture::with_settings(settings("EsmerilGlass", 15));
    let output = desk(&mut f);
    let before = esmerils(&mut f, &output).remove(0);
    f.set_setting(CAMINITO, "esmeril-strength", Value::Uint(1));
    f.dispatch_until(|f| f.state.caminito.settings.esmeril == Some(1));
    let after = esmerils(&mut f, &output).remove(0);
    let strip = after.strip();
    assert_eq!(after.geometry(SCALE).loc, strip.loc - Point::from((10, 10)));
    assert_ne!(before.current_commit(), after.current_commit());
}

#[test]
fn what_changes_where_the_blur_reads_damages_all_it_reads() {
    let mut f = Fixture::with_settings(settings("EsmerilGlass", 15));
    let output = desk(&mut f);
    let esmeril = esmerils(&mut f, &output).remove(0);
    let (strip, read) = (esmeril.strip(), esmeril.geometry(SCALE));

    let mut tracker = OutputDamageTracker::from_output(&output);
    let mut damage = |f: &mut Fixture| {
        let Drawing::Pixman(renderer, _) = f.state.backend.headless_mut().drawing() else {
            panic!("it draws with pixman");
        };
        let elements = scene::output_elements(renderer, &f.state.caminito, &output);
        let (damage, _) = tracker.damage_output(1, &elements).unwrap();
        damage.cloned().unwrap_or_default()
    };
    damage(&mut f);
    assert_eq!(damage(&mut f), vec![], "nothing changed");

    // A band across the middle of the output, under the window and sticking out of it:
    // under the strip it's not, but the blur reads it.
    let id = f.connect(Kind::Xdg);
    f.map_layer(id, LayerSpec::floating(Layer::Bottom, (380, 10)), OTHER);
    f.draw(&output);
    let band = Rectangle::<i32, Physical>::new((10, 145).into(), (380, 10).into());
    assert!(!band.overlaps(strip), "{band:?} and {strip:?}");
    assert!(band.overlaps(read), "{band:?} and {read:?}");

    let damage = damage(&mut f);
    let left = read.subtract_rects(damage.iter().copied());
    assert!(left.is_empty(), "not damaged: {left:?} of {damage:?}");
}

#[test]
fn a_see_through_pixel_shows_what_is_beneath_saturated_and_contrasted() {
    let Some(mut f) = Fixture::on_gles_with(settings("EsmerilGlass", 1)) else {
        return;
    };
    let output = desk_over(&mut f, BENEATH);
    let strip = esmerils(&mut f, &output).remove(0).strip();
    let middle = Point::from((
        strip.loc.x + strip.size.w / 2,
        strip.loc.y + strip.size.h - 2,
    ));
    f.draw(&output);

    let got = channels(pixel(&mut f, &output, middle));
    // The theme's half black tint over it.
    let want = saturated_and_contrasted(BENEATH).map(|c| c * 0.5);
    for (got, want) in got.into_iter().zip(want) {
        assert!(
            (f64::from(got) - want).abs() <= 3.0,
            "{got:?} and not {want:?}"
        );
    }
}

#[test]
fn an_opaque_pixel_and_a_clear_one_look_as_they_do_with_esmeril_off() {
    let on = Fixture::on_gles_with(settings("EsmerilGlass", 1));
    let Some(mut on) = on else {
        return;
    };
    let mut off_settings = settings("EsmerilGlass", 1);
    off_settings.push((CAMINITO, "esmeril".into(), Value::Bool(false)));
    let Some(mut off) = Fixture::on_gles_with(off_settings) else {
        return;
    };
    let mut rows = Vec::new();
    for f in [&mut on, &mut off] {
        let output = desk_over(f, BENEATH);
        f.draw(&output);
        rows.push((
            f.state.backend.headless_mut().pixels(&output).unwrap(),
            output,
        ));
    }
    let strip = esmerils(&mut on, &rows[0].1).remove(0).strip();
    let row = |pixels: &[u32], y: i32| -> Vec<u32> {
        let at = (y * OUTPUT.0 + strip.loc.x + 20) as usize;
        pixels[at..at + strip.size.w as usize - 40].to_vec()
    };
    for y in 0..4 {
        let y = strip.loc.y + y;
        assert_eq!(row(&rows[0].0, y), row(&rows[1].0, y), "row {y}");
    }
    // And it does something in the see-through rows.
    let y = strip.loc.y + 6;
    assert_ne!(row(&rows[0].0, y), row(&rows[1].0, y));
}

#[test]
fn what_is_beneath_shows_blurred_live_and_by_its_strength() {
    let Some(mut f) = Fixture::on_gles_with(settings("EsmerilGlass", 1)) else {
        return;
    };
    let output = desk_over(&mut f, BENEATH);
    let strip = esmerils(&mut f, &output).remove(0).strip();
    let y = strip.loc.y + strip.size.h - 2;
    let at = |x: i32| Point::from((x, y));
    let middle = OUTPUT.0 / 2;
    let before = pixel(&mut f, &output, at(middle - 30));

    // A band mapped beneath the middle of the strip: a sharp edge at 100 and 300 would be at
    // full strength a pixel on either side of it.
    let id = f.connect(Kind::Xdg);
    f.map_layer(id, LayerSpec::floating(Layer::Bottom, (60, 280)), OTHER);
    f.draw(&output);
    let (inside, outside) = (at(middle - 28), at(middle - 32));
    let (a, b) = (
        pixel(&mut f, &output, inside),
        pixel(&mut f, &output, outside),
    );
    assert_ne!(a, before, "it follows what's beneath");
    let far = pixel(&mut f, &output, at(middle - 90));
    let centre = pixel(&mut f, &output, at(middle));
    let spread = |p: u32, q: u32| {
        channels(p)
            .into_iter()
            .zip(channels(q))
            .map(|(p, q)| (p - q).abs())
            .max()
            .unwrap()
    };
    let whole = spread(far, centre);
    assert!(
        spread(a, b) < whole,
        "4 pixels across the edge {} of the {whole} across all of it",
        spread(a, b)
    );

    // Stronger, the edge spreads further: 30 pixels out of it isn't what's far from it.
    let edge_out = at(middle - 45);
    let weak = pixel(&mut f, &output, edge_out);
    f.set_setting(CAMINITO, "esmeril-strength", Value::Uint(15));
    f.dispatch_until(|f| f.state.caminito.settings.esmeril == Some(15));
    f.draw(&output);
    let strong = pixel(&mut f, &output, edge_out);
    assert_ne!(weak, strong, "a new strength shows at once");
}

#[test]
fn what_is_beneath_shows_the_right_way_up() {
    let Some(mut f) = Fixture::on_gles_with(settings("EsmerilGlass", 1)) else {
        return;
    };
    let output = desk_over(&mut f, BENEATH);
    let strip = esmerils(&mut f, &output).remove(0).strip();
    // A band whose top edge is across the strip's see-through rows, so what's beneath its
    // lower rows is the band, and what's read upside down would have it over its upper ones.
    let (upper, lower) = (strip.loc.y + 5, strip.loc.y + strip.size.h - 1);
    let edge = (upper + lower) / 2;
    let height = 2 * (OUTPUT.1 / 2 - edge);
    let id = f.connect(Kind::Xdg);
    f.map_layer(
        id,
        LayerSpec::floating(Layer::Bottom, (OUTPUT.0 as u32, height as u32)),
        OTHER,
    );
    f.draw(&output);

    let x = OUTPUT.0 / 2;
    let blue = |f: &mut Fixture, y: i32| channels(pixel(f, &output, (x, y).into()))[2];
    let (upper, lower) = (blue(&mut f, upper), blue(&mut f, lower));
    assert!(lower > upper, "{lower} under the band, {upper} above it");
}
