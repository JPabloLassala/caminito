//! Esmeril behind frames, listed windows and layer surfaces (ADR 0048): where it goes and its damage on
//! Pixman, where it draws nothing, and its pixels on GLES, skipped where surfaceless EGL can't
//! be created. Those of frames run with xdg toplevels only: a frame's strips are drawn from
//! its theme alone, whichever kind of window has them, and frames.rs runs what a frame does on
//! both kinds. Those of listed windows run on both, as their app ids come from each its own
//! way. A layer surface is a Wayland client's, and the window that hides an Intellihide panel
//! only has to overlap it.

use std::time::Duration;

use caminito_test_client::layer_shell::{KeyboardInteractivity, Layer, LayerAnchor, LayerSpec};
use caminito_test_client::{Anchor, ConstraintAdjustment, Gravity, Positioner};
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::Element;
use smithay::output::Output;
use smithay::utils::{Physical, Point, Rectangle, Scale};

use super::fixture::{
    ClientWindow, Fixture, KEY_F10, KEY_LEFTALT, Kind, WindowSpec, on_both_kinds,
};
use super::maximise_animation::asked_size;
use super::overview;
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
    let mut f = Fixture::with_settings(settings("EsmerilSeeThrough", 15));
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
    let mut f = Fixture::with_settings(settings("EsmerilSeeThrough", 15));
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
    let mut f = Fixture::with_settings(settings("EsmerilSeeThrough", 15));
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
    let mut f = Fixture::with_settings(settings("EsmerilSeeThrough", 15));
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
    let mut f = Fixture::with_settings(settings("EsmerilSeeThrough", 15));
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
    let Some(mut f) = Fixture::on_gles_with(settings("EsmerilSeeThrough", 1)) else {
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
    let on = Fixture::on_gles_with(settings("EsmerilSeeThrough", 1));
    let Some(mut on) = on else {
        return;
    };
    let mut off_settings = settings("EsmerilSeeThrough", 1);
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
    let Some(mut f) = Fixture::on_gles_with(settings("EsmerilSeeThrough", 1)) else {
        return;
    };
    let output = desk_over(&mut f, BENEATH);
    let strip = esmerils(&mut f, &output).remove(0).strip();
    let y = strip.loc.y + strip.size.h - 2;
    let at = |x: i32| Point::from((x, y));
    let middle = OUTPUT.0 / 2;
    let before = pixel(&mut f, &output, at(middle - 30));

    // A band mapped beneath the middle of the strip: a sharp edge at its side, 30 pixels from
    // the middle, would be at full strength a pixel on either side of it.
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

    // Stronger, the edge spreads further: 15 pixels out of it isn't what it was.
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
    let Some(mut f) = Fixture::on_gles_with(settings("EsmerilSeeThrough", 1)) else {
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

/// A theme with no see-through pixels and a centred window with `app_id` on an output of
/// `OUTPUT`, so that any Esmeril there is the window's own.
fn desk_with(f: &mut Fixture, kind: Kind, app_id: &'static str) -> (Output, ClientWindow) {
    let output = f.add_output(OUTPUT, 1.0);
    let id = f.connect(kind);
    let window = f.map(
        id,
        WindowSpec {
            size: (200, 100),
            app_id: Some(app_id),
            ..WindowSpec::default()
        },
    );
    f.draw(&output);
    (output, window)
}

on_both_kinds! {
    fn a_listed_window_gets_esmeril_behind_its_surface_and_an_unlisted_one_doesnt(kind: Kind) {
        let mut f = Fixture::with_settings(settings("EsmerilSolid", 15));
        let (output, window) = desk_with(&mut f, kind, "mate-terminal");
        let elements = esmerils(&mut f, &output);
        let [esmeril] = elements.as_slice() else {
            panic!("the window's surface has one: {elements:?}");
        };
        let geometry = f.geometry(window);
        let surface = Rectangle::new(geometry.loc, geometry.size);
        assert_eq!(esmeril.strip(), surface.to_physical(1));

        let mut f = Fixture::with_settings(settings("EsmerilSolid", 15));
        let (output, _) = desk_with(&mut f, kind, "pluma");
        assert!(esmerils(&mut f, &output).is_empty());
    }
}

on_both_kinds! {
    fn a_window_is_matched_whatever_the_case_of_its_app_id(kind: Kind) {
        // What an X window of mate-terminal has as its `WM_CLASS` class.
        let mut f = Fixture::with_settings(settings("EsmerilSolid", 15));
        let (output, _) = desk_with(&mut f, kind, "Mate-terminal");
        assert_eq!(esmerils(&mut f, &output).len(), 1);
    }
}

on_both_kinds! {
    fn a_change_of_the_list_applies_to_windows_already_open(kind: Kind) {
        let mut f = Fixture::with_settings(settings("EsmerilSolid", 15));
        let (output, _) = desk_with(&mut f, kind, "pluma");
        assert!(esmerils(&mut f, &output).is_empty());
        let listed = |list: &[&str]| Value::StrList(list.iter().map(|s| (*s).into()).collect());
        f.set_setting(CAMINITO, "esmeril-applications", listed(&["pluma"]));
        f.dispatch_until(|f| !f.state.caminito.settings.esmeril_applications.is_empty()
            && f.state.caminito.settings.esmeril_applications[0] == "pluma");
        assert_eq!(esmerils(&mut f, &output).len(), 1);
        f.set_setting(CAMINITO, "esmeril-applications", listed(&[]));
        f.dispatch_until(|f| f.state.caminito.settings.esmeril_applications.is_empty());
        assert!(esmerils(&mut f, &output).is_empty());
    }
}

on_both_kinds! {
    fn esmeril_off_removes_it_from_a_listed_window(kind: Kind) {
        let mut f = Fixture::with_settings(settings("EsmerilSolid", 15));
        let (output, _) = desk_with(&mut f, kind, "mate-terminal");
        f.set_setting(CAMINITO, "esmeril", Value::Bool(false));
        f.dispatch_until(|f| f.state.caminito.settings.esmeril.is_none());
        assert!(esmerils(&mut f, &output).is_empty());
    }
}

#[test]
fn a_listed_window_s_subsurface_gets_it_and_its_popup_doesnt() {
    let mut f = Fixture::with_settings(settings("EsmerilSolid", 15));
    let output = f.add_output(OUTPUT, 1.0);
    let id = f.connect(Kind::Xdg);
    let window = f.map(
        id,
        WindowSpec {
            size: (200, 100),
            app_id: Some("mate-terminal"),
            ..WindowSpec::default()
        },
    );
    let corner = f.geometry(window).loc;
    f.client(id)
        .add_subsurface(window.xdg(), (10, 20), (50, 40), OTHER);
    let popup = f.client(id).create_popup(
        window.xdg(),
        Positioner {
            size: (60, 60),
            anchor_rect: (0, 0, 10, 10),
            anchor: Anchor::BottomRight,
            gravity: Gravity::BottomRight,
            constraint_adjustment: ConstraintAdjustment::None,
        },
    );
    f.roundtrip(id);
    f.client(id).map_popup(popup, OTHER);
    f.client(id).commit(window.xdg());
    f.roundtrip(id);
    f.draw(&output);

    let mut strips: Vec<_> = esmerils(&mut f, &output)
        .iter()
        .map(Esmeril::strip)
        .collect();
    strips.sort_by_key(|strip| strip.size.w);
    let at = |x, y, w, h| Rectangle::new((corner.x + x, corner.y + y).into(), (w, h).into());
    assert_eq!(strips, vec![at(10, 20, 50, 40), at(0, 0, 200, 100)]);
}

#[test]
fn a_listed_window_s_esmeril_is_under_its_surface() {
    let mut f = Fixture::with_settings(settings("EsmerilSolid", 15));
    let (output, _) = desk_with(&mut f, Kind::Xdg, "mate-terminal");
    let Drawing::Pixman(renderer, _) = f.state.backend.headless_mut().drawing() else {
        panic!("it draws with pixman");
    };
    let elements = scene::output_elements(renderer, &f.state.caminito, &output);
    let at = |wanted: fn(&SceneElement<_>) -> bool| elements.iter().position(wanted).unwrap();
    let surface = at(|element| matches!(element, SceneElement::Window(_)));
    let esmeril = at(|element| matches!(element, SceneElement::Esmeril(_)));
    assert!(surface < esmeril, "front to back: {surface} and {esmeril}");
}

/// A listed window, whose client draws it `argb`, in the middle of an output covered in
/// `BENEATH`, with the middle of its surface returned.
fn glass_over_beneath(
    f: &mut Fixture,
    app_id: &'static str,
    argb: u32,
) -> (Output, Point<i32, Physical>) {
    let output = f.add_output(OUTPUT, 1.0);
    let id = f.connect(Kind::Xdg);
    f.map_layer(id, LayerSpec::covering(Layer::Background, "test"), BENEATH);
    let id = f.connect(Kind::Xdg);
    let window = f.map(
        id,
        WindowSpec {
            size: (200, 100),
            app_id: Some(app_id),
            ..WindowSpec::default()
        },
    );
    f.client(id)
        .ack_and_commit_buffer(window.xdg(), (200, 100), argb);
    f.roundtrip(id);
    f.draw(&output);
    let geometry = f.geometry(window);
    let middle = Point::from((
        geometry.loc.x + geometry.size.w / 2,
        geometry.loc.y + geometry.size.h / 2,
    ));
    (output, middle)
}

#[test]
fn a_see_through_surface_of_a_listed_window_shows_what_is_beneath_saturated_and_contrasted() {
    let Some(mut f) = Fixture::on_gles_with(settings("EsmerilSolid", 1)) else {
        return;
    };
    // Half black, premultiplied, as a translucent terminal's background.
    let (output, middle) = glass_over_beneath(&mut f, "mate-terminal", 0x80_00_00_00);
    f.draw(&output);
    let got = channels(pixel(&mut f, &output, middle));
    let want = saturated_and_contrasted(BENEATH).map(|c| c * (1.0 - 128.0 / 255.0));
    for (got, want) in got.into_iter().zip(want) {
        assert!(
            (f64::from(got) - want).abs() <= 3.0,
            "{got:?} and not {want:?}"
        );
    }
}

#[test]
fn an_unlisted_see_through_surface_shows_what_is_beneath_as_it_is() {
    let Some(mut f) = Fixture::on_gles_with(settings("EsmerilSolid", 1)) else {
        return;
    };
    let (output, middle) = glass_over_beneath(&mut f, "pluma", 0x80_00_00_00);
    f.draw(&output);
    let got = channels(pixel(&mut f, &output, middle));
    let want = channels(BENEATH).map(|c| f64::from(c) * (1.0 - 128.0 / 255.0));
    for (got, want) in got.into_iter().zip(want) {
        assert!(
            (f64::from(got) - want).abs() <= 3.0,
            "{got:?} and not {want:?}"
        );
    }
}

#[test]
fn a_wholly_clear_or_opaque_surface_of_a_listed_window_looks_as_an_unlisted_one_does() {
    for argb in [0x00_00_00_00, 0xff_20_40_60] {
        let Some(mut listed) = Fixture::on_gles_with(settings("EsmerilSolid", 1)) else {
            return;
        };
        let Some(mut unlisted) = Fixture::on_gles_with(settings("EsmerilSolid", 1)) else {
            return;
        };
        let (output, middle) = glass_over_beneath(&mut listed, "mate-terminal", argb);
        listed.draw(&output);
        let a = pixel(&mut listed, &output, middle);
        let (output, middle) = glass_over_beneath(&mut unlisted, "pluma", argb);
        unlisted.draw(&output);
        let b = pixel(&mut unlisted, &output, middle);
        assert_eq!(a, b, "{argb:#x}");
    }
}

#[test]
fn a_commit_that_changes_a_listed_surface_s_alpha_shows_at_once() {
    let Some(mut f) = Fixture::on_gles_with(settings("EsmerilSolid", 1)) else {
        return;
    };
    let output = f.add_output(OUTPUT, 1.0);
    let id = f.connect(Kind::Xdg);
    f.map_layer(id, LayerSpec::covering(Layer::Background, "test"), BENEATH);
    let id = f.connect(Kind::Xdg);
    let window = f.map(
        id,
        WindowSpec {
            size: (200, 100),
            app_id: Some("mate-terminal"),
            ..WindowSpec::default()
        },
    );
    f.draw(&output);
    let geometry = f.geometry(window);
    let middle = Point::from((geometry.loc.x + 100, geometry.loc.y + 50));
    let opaque = pixel(&mut f, &output, middle);

    f.client(id)
        .ack_and_commit_buffer(window.xdg(), (200, 100), 0x00_00_00_00);
    f.roundtrip(id);
    f.draw(&output);
    let clear = channels(pixel(&mut f, &output, middle));
    assert_ne!(channels(opaque), clear);
    assert_eq!(clear, channels(BENEATH), "a clear pixel gets none");

    f.client(id)
        .ack_and_commit_buffer(window.xdg(), (200, 100), 0x80_00_00_00);
    f.roundtrip(id);
    f.draw(&output);
    let got = channels(pixel(&mut f, &output, middle));
    let want = saturated_and_contrasted(BENEATH).map(|c| c * (1.0 - 128.0 / 255.0));
    for (got, want) in got.into_iter().zip(want) {
        assert!(
            (f64::from(got) - want).abs() <= 3.0,
            "{got:?} and not {want:?}"
        );
    }
}

/// A see-through colour of a surface: half black, premultiplied.
const TINT: u32 = 0x80_00_00_00;

/// How many Esmeril elements a surface of `namespace` in `color` gets on the top layer.
fn esmerils_of(namespace: &'static str, color: u32) -> usize {
    let mut f = Fixture::with_settings(vec![]);
    let output = f.add_output(OUTPUT, 1.0);
    let id = f.connect(Kind::Xdg);
    f.map_layer(id, LayerSpec::covering(Layer::Top, namespace), color);
    f.draw(&output);
    esmerils(&mut f, &output).len()
}

#[test]
fn a_see_through_layer_surface_gets_esmeril_by_its_namespace() {
    for (namespace, count) in [
        ("panel", 1),
        ("mate-osd", 1),
        ("desktop-menu", 1),
        ("caminito-notification", 1),
        ("desktop", 0),
        ("mate-background", 0),
        ("something-else", 0),
    ] {
        assert_eq!(esmerils_of(namespace, TINT), count, "{namespace}");
    }
}

#[test]
fn a_layer_surface_with_no_see_through_pixels_gets_none() {
    assert_eq!(esmerils_of("panel", 0xff_20_40_60), 0, "wholly opaque");
    assert_eq!(esmerils_of("panel", 0x00_00_00_00), 0, "wholly clear");
}

#[test]
fn a_layer_surfaces_esmeril_is_under_it_at_its_place() {
    let mut f = Fixture::with_settings(vec![]);
    let output = f.add_output(OUTPUT, 1.0);
    let id = f.connect(Kind::Xdg);
    f.map_layer(id, LayerSpec::top_bar(28), TINT);
    f.draw(&output);
    let elements = esmerils(&mut f, &output);
    let [esmeril] = elements.as_slice() else {
        panic!("the panel is see-through: {elements:?}");
    };
    let strip = Rectangle::new((0, 0).into(), (OUTPUT.0, 28).into());
    assert_eq!(esmeril.strip(), strip);
    assert_eq!(
        esmeril.geometry(SCALE),
        Rectangle::new((0, 0).into(), (OUTPUT.0, 178).into())
    );

    let Drawing::Pixman(renderer, _) = f.state.backend.headless_mut().drawing() else {
        panic!("it draws with pixman");
    };
    let elements = scene::output_elements(renderer, &f.state.caminito, &output);
    let at = elements
        .iter()
        .position(|element| matches!(element, SceneElement::Esmeril(_)))
        .unwrap();
    assert!(matches!(elements[at - 1], SceneElement::Window(_)));
}

#[test]
fn esmeril_off_removes_a_layer_surfaces_too() {
    let mut f = Fixture::with_settings(vec![]);
    let output = f.add_output(OUTPUT, 1.0);
    let id = f.connect(Kind::Xdg);
    f.map_layer(id, LayerSpec::top_bar(28), TINT);
    f.draw(&output);
    assert_eq!(esmerils(&mut f, &output).len(), 1);
    f.set_setting(CAMINITO, "esmeril", Value::Bool(false));
    f.dispatch_until(|f| f.state.caminito.settings.esmeril.is_none());
    assert!(esmerils(&mut f, &output).is_empty());
}

#[test]
fn a_commit_of_the_surface_keeps_its_element_and_can_end_it() {
    let mut f = Fixture::with_settings(vec![]);
    let output = f.add_output(OUTPUT, 1.0);
    let id = f.connect(Kind::Xdg);
    let panel = f.map_layer(id, LayerSpec::top_bar(28), TINT);
    f.draw(&output);
    let before = esmerils(&mut f, &output).remove(0);

    f.client(id).redraw_layer_surface(panel, 0x40_00_00_00);
    f.roundtrip(id);
    f.draw(&output);
    let after = esmerils(&mut f, &output).remove(0);
    assert_eq!(before.id(), after.id());

    f.client(id).redraw_layer_surface(panel, 0xff_20_40_60);
    f.roundtrip(id);
    f.draw(&output);
    assert!(esmerils(&mut f, &output).is_empty(), "it's opaque now");
}

/// A menu hanging below a bar at the top of the output, as mate-panel's are.
fn menu() -> Positioner {
    Positioner {
        size: (200, 100),
        anchor_rect: (50, 0, 20, 28),
        anchor: Anchor::BottomLeft,
        gravity: Gravity::BottomRight,
        constraint_adjustment: ConstraintAdjustment::None,
    }
}

/// The Esmeril elements of a bar of `namespace` in `bar` with a popup in `popup`.
fn esmerils_with_a_popup(namespace: &'static str, bar: u32, popup: u32) -> Vec<Esmeril> {
    let mut f = Fixture::with_settings(vec![]);
    let output = f.add_output(OUTPUT, 1.0);
    let id = f.connect(Kind::Xdg);
    let spec = LayerSpec {
        namespace,
        ..LayerSpec::top_bar(28)
    };
    let parent = f.map_layer(id, spec, bar);
    let menu = f.client(id).create_layer_popup(parent, menu(), None);
    f.roundtrip(id);
    f.client(id).map_popup(menu, popup);
    f.roundtrip(id);
    f.draw(&output);
    esmerils(&mut f, &output)
}

#[test]
fn a_panels_see_through_popup_gets_esmeril_at_its_place() {
    let elements = esmerils_with_a_popup("panel", 0xff_20_40_60, TINT);
    let [esmeril] = elements.as_slice() else {
        panic!("only the popup is see-through: {elements:?}");
    };
    let place = Rectangle::new((50, 28).into(), (200, 100).into());
    assert_eq!(esmeril.strip(), place);
}

#[test]
fn an_opaque_popup_and_the_desktops_get_none() {
    let opaque = esmerils_with_a_popup("panel", 0xff_20_40_60, 0xff_20_40_60);
    assert!(opaque.is_empty(), "{opaque:?}");
    let desktop = esmerils_with_a_popup("desktop", TINT, TINT);
    assert!(desktop.is_empty(), "{desktop:?}");
}

#[test]
fn an_intellihide_panels_esmeril_slides_with_it_and_goes_when_it_has() {
    let mut f = Fixture::with_settings(vec![(CAMINITO, "popup-slide".into(), Value::Bool(true))]);
    let output = f.add_output(OUTPUT, 1.0);
    let id = f.connect(Kind::Xdg);
    f.stop_clock();
    f.roundtrip(id);
    let window = f.map(id, WindowSpec::plain((100, 50)));
    let spec = LayerSpec {
        layer: Layer::Top,
        namespace: "panel",
        output: None,
        anchor: LayerAnchor::Bottom | LayerAnchor::Left | LayerAnchor::Right,
        size: (0, 28),
        exclusive_zone: 0,
        keyboard: KeyboardInteractivity::None,
    };
    let panel = f.map_layer(id, spec, TINT);
    f.client(id).ask_for_intellihide(panel);
    f.roundtrip(id);
    let rest = OUTPUT.1 - 28;
    let at_rest = esmerils(&mut f, &output).remove(0);
    assert_eq!(at_rest.strip().loc.y, rest);

    f.chord(&[KEY_LEFTALT, KEY_F10]);
    let size = asked_size(&f, window);
    f.draw_at_asked_size(window, size);
    // A slide into the edge starts slowly.
    f.advance_clock(Duration::from_millis(150));
    let moving = esmerils(&mut f, &output).remove(0);
    let y = moving.strip().loc.y;
    assert!(rest < y && y < OUTPUT.1, "on its way out, at {y}");
    assert_eq!(moving.id(), at_rest.id());

    f.advance_clock(Duration::from_millis(50));
    let gone = esmerils(&mut f, &output);
    assert!(gone.is_empty(), "the panel is off the output: {gone:?}");
}

#[test]
fn a_panels_esmeril_fades_with_it_as_the_overview_opens() {
    let mut f = Fixture::with_settings(vec![(
        CAMINITO,
        "overview-animation".into(),
        Value::Bool(true),
    )]);
    let output = f.add_output(OUTPUT, 1.0);
    let id = f.connect(Kind::Xdg);
    f.stop_clock();
    f.map_layer(id, LayerSpec::top_bar(28), TINT);
    f.draw(&output);
    assert_eq!(esmerils(&mut f, &output).remove(0).alpha(), 1.0);

    overview::open(&mut f);
    f.advance_clock(Duration::from_millis(50));
    let alpha = esmerils(&mut f, &output).remove(0).alpha();
    assert!(0.0 < alpha && alpha < 1.0, "part-way: {alpha}");

    f.advance_clock(Duration::from_millis(250));
    assert!(esmerils(&mut f, &output).is_empty());
}

#[test]
fn a_see_through_panel_shows_what_is_beneath_saturated_and_contrasted() {
    let settings = vec![(CAMINITO, "esmeril-strength".into(), Value::Uint(1))];
    let Some(mut f) = Fixture::on_gles_with(settings) else {
        return;
    };
    let output = f.add_output(OUTPUT, 1.0);
    let id = f.connect(Kind::Xdg);
    f.map_layer(id, LayerSpec::covering(Layer::Background, "test"), BENEATH);
    // Covering it, as a bar's exclusive zone would push what's beneath out from under it.
    f.map_layer(id, LayerSpec::covering(Layer::Top, "panel"), TINT);
    f.draw(&output);

    let got = channels(pixel(&mut f, &output, (200, 50).into()));
    let want = saturated_and_contrasted(BENEATH).map(|c| c * (1.0 - 128.0 / 255.0));
    for (got, want) in got.into_iter().zip(want) {
        assert!(
            (f64::from(got) - want).abs() <= 3.0,
            "{got:?} and not {want:?}"
        );
    }
}

#[test]
fn a_popups_esmeril_slides_with_it_and_rests_at_its_place() {
    let mut f = Fixture::with_settings(vec![(CAMINITO, "popup-slide".into(), Value::Bool(true))]);
    let output = f.add_output(OUTPUT, 1.0);
    let id = f.connect(Kind::Xdg);
    f.stop_clock();
    let parent = f.map_layer(id, LayerSpec::top_bar(28), 0xff_20_40_60);
    let popup = f.client(id).create_layer_popup(parent, menu(), None);
    f.roundtrip(id);
    f.client(id).map_popup(popup, TINT);
    f.roundtrip(id);
    f.draw(&output);
    let place = Rectangle::new((50, 28).into(), (200, 100).into());

    f.advance_clock(Duration::from_millis(20));
    let sliding = esmerils(&mut f, &output).remove(0);
    assert!(sliding.strip().loc.y < 28, "{:?}", sliding.strip());

    f.advance_clock(Duration::from_millis(400));
    let resting = esmerils(&mut f, &output).remove(0);
    assert_eq!(resting.strip(), place);
}
