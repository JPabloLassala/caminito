//! Camino's theme engine against Marco's goldens (14, ADR 0009): every case in
//! `tests/goldens/cases.json`, for every golden theme at every scale, computed with the
//! pinned font and compared with what `marco-frame-dump` recorded.
//!
//! Geometry has to match exactly (26). Pixels have to match within the tolerance below
//! (27), and a case that doesn't writes Camino's image and a diff beside the golden, as
//! `<case>@<scale>x.camino.png` and `.diff.png`, for CI to keep. The four strips of each
//! case, joined, have to give the whole frame's pixels, to within one step in a channel
//! (see `STRIP_CHANNEL`).
//!
//! This test has its own `main`, so it can point fontconfig at the vendored font before
//! any thread starts.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use cairo::ImageSurface;
use camino_theme::{
    Border, ButtonFunction, ButtonLayout, ButtonState, ButtonStates, Frame, FrameFlags,
    FrameGeometry, FrameType, GtkState, PaletteColors, Rect, Rgba, StateColors, Strip, Theme,
    ThemeDirs, Title, gdk_pixbuf::Pixbuf, pango, surface_from_pixbuf, title_context,
};
use serde_json::{Map, Value, json};

fn repo_tests() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests")
}

fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn strings(value: &Value) -> Vec<&str> {
    value
        .as_array()
        .map(|array| {
            array
                .iter()
                .map(|v| v.as_str().expect("a string"))
                .collect()
        })
        .unwrap_or_default()
}

/// A case with the base's inputs applied, as `marco-frame-dump`'s `dump_case` resolves
/// it.
struct Case<'a> {
    name: &'a str,
    frame_type: FrameType,
    flags: FrameFlags,
    flag_names: Vec<&'a str>,
    layout: ButtonLayout,
    button_states: ButtonStates,
}

/// `MetaFrameFlags`' names, in the order `marco-frame-dump` writes them.
const FLAG_NAMES: [&str; 18] = [
    "allows_delete",
    "allows_menu",
    "allows_appmenu",
    "allows_minimize",
    "allows_maximize",
    "allows_vertical_resize",
    "allows_horizontal_resize",
    "has_focus",
    "shaded",
    "stuck",
    "maximized",
    "allows_shade",
    "allows_move",
    "fullscreen",
    "is_flashing",
    "above",
    "tiled_left",
    "tiled_right",
];

fn flag<'f>(flags: &'f mut FrameFlags, name: &str) -> &'f mut bool {
    match name {
        "allows_delete" => &mut flags.allows_delete,
        "allows_menu" => &mut flags.allows_menu,
        "allows_appmenu" => &mut flags.allows_appmenu,
        "allows_minimize" => &mut flags.allows_minimize,
        "allows_maximize" => &mut flags.allows_maximize,
        "allows_vertical_resize" => &mut flags.allows_vertical_resize,
        "allows_horizontal_resize" => &mut flags.allows_horizontal_resize,
        "has_focus" => &mut flags.has_focus,
        "shaded" => &mut flags.shaded,
        "stuck" => &mut flags.stuck,
        "maximized" => &mut flags.maximized,
        "allows_shade" => &mut flags.allows_shade,
        "allows_move" => &mut flags.allows_move,
        "fullscreen" => &mut flags.fullscreen,
        "is_flashing" => &mut flags.is_flashing,
        "above" => &mut flags.above,
        "tiled_left" => &mut flags.tiled_left,
        "tiled_right" => &mut flags.tiled_right,
        _ => panic!("cases: unknown frame flag {name:?}"),
    }
}

fn resolve<'a>(base: &'a Value, case: &'a Value) -> Case<'a> {
    let type_name = case["type"].as_str().or(base["type"].as_str()).unwrap();
    let mut flags = FrameFlags::default();
    for name in strings(&base["flags"])
        .into_iter()
        .chain(strings(&case["add_flags"]))
    {
        *flag(&mut flags, name) = true;
    }
    for name in strings(&case["remove_flags"]) {
        *flag(&mut flags, name) = false;
    }
    let flag_names = FLAG_NAMES
        .into_iter()
        .filter(|&name| *flag(&mut flags.clone(), name))
        .collect();
    let layout = case["button_layout"]
        .as_str()
        .or(base["button_layout"].as_str())
        .unwrap();
    let mut button_states = ButtonStates::default();
    let states = case
        .get("button_states")
        .unwrap_or(&base["button_states"])
        .as_object()
        .unwrap();
    for (name, state) in states {
        let function = ButtonFunction::ALL
            .into_iter()
            .find(|function| function.button_type().name() == name)
            .unwrap_or_else(|| panic!("cases: unknown button {name:?}"));
        let state = ButtonState::from_name(state.as_str().unwrap())
            .unwrap_or_else(|| panic!("cases: unknown button state {state}"));
        button_states.set(function, state);
    }
    Case {
        name: case["name"].as_str().unwrap(),
        frame_type: FrameType::from_name(type_name)
            .unwrap_or_else(|| panic!("cases: unknown frame type {type_name:?}")),
        flags,
        flag_names,
        layout: ButtonLayout::parse(layout),
        button_states,
    }
}

/// The title's pango context, with the rendering options `cases.json` pins.
fn context(cases: &Value) -> pango::Context {
    assert_eq!(cases["antialias"], "gray");
    assert_eq!(cases["hint_style"], "slight");
    let mut options = cairo::FontOptions::new().unwrap();
    options.set_antialias(cairo::Antialias::Gray);
    options.set_hint_style(cairo::HintStyle::Slight);
    options.set_hint_metrics(match cases["hint_metrics"].as_str().unwrap() {
        "on" => cairo::HintMetrics::On,
        "off" => cairo::HintMetrics::Off,
        other => panic!("cases: unknown hint_metrics {other:?}"),
    });
    title_context(
        &options,
        cases["dpi"].as_f64().unwrap(),
        &pango::Language::from_string(cases["language"].as_str().unwrap()),
    )
}

/// The default variant of the palette `marco-frame-dump` recorded.
fn palette(goldens: &Path) -> PaletteColors {
    let palette = read_json(&goldens.join("palette.json"));
    let colors = &palette["variants"]["default"];
    let rgba = |value: &Value| {
        let c: Vec<f64> = value
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_f64().unwrap())
            .collect();
        Rgba {
            red: c[0],
            green: c[1],
            blue: c[2],
            alpha: c[3],
        }
    };
    let states = GtkState::ALL.map(|state| {
        let state = &colors["states"][state.name().to_ascii_lowercase()];
        StateColors {
            background: rgba(&state["background"]),
            text: rgba(&state["text"]),
        }
    });
    let named: BTreeMap<String, Rgba> = colors["named"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(_, colour)| !colour.is_null())
        .map(|(name, colour)| (name.clone(), rgba(colour)))
        .collect();
    PaletteColors::new(states, named)
}

/// An icon as `marco-frame-dump` loads it, at its own size.
fn icon(goldens: &Path, cases: &Value, key: &str) -> ImageSurface {
    let path = goldens.join(cases[key].as_str().unwrap());
    let pixbuf = Pixbuf::from_file(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    surface_from_pixbuf(&pixbuf).unwrap()
}

/// How far Camino's pixels may be from Marco's.
#[derive(Debug, Clone, Copy)]
struct Tolerance {
    /// The most a channel of a pixel may differ by and the pixel still match.
    channel: u8,
    /// How many pixels of an image may fail to match.
    pixels: usize,
}

/// Camino makes the cairo calls Marco makes, on the same cairo, pango and gdk-pixbuf,
/// with the pinned font, so its pixels are Marco's exactly.
const EXACT: Tolerance = Tolerance {
    channel: 0,
    pixels: 0,
};

/// The themes that draw with `gtk_arrow`, `gtk_box` or `gtk_vline`, which Marco hands to
/// GTK and Camino approximates (07). GTK draws Adwaita's arrow as a chevron icon where
/// Camino draws a triangle, and Adwaita's box with no border where Camino draws one.
const APPROXIMATED_THEMES: [&str; 2] = ["Atlanta", "HighContrast"];

/// Those themes may differ by as many pixels as the approximated ops account for:
/// Atlanta's pressed buttons, which have all three, differ in 154 pixels at scale 1 and
/// 594 at scale 2. The count isn't confined to those ops' rects, so the bounds are kept
/// close to what they need.
fn tolerance(theme: &str, scale: i32) -> Tolerance {
    if APPROXIMATED_THEMES.contains(&theme) {
        Tolerance {
            channel: 0,
            pixels: if scale == 1 { 160 } else { 600 },
        }
    } else {
        EXACT
    }
}

/// How far a strip's pixels may be from the whole frame's. Cairo works a gradient's
/// transform out from where it's drawn on the surface, and a strip's surface starts
/// elsewhere than the whole frame's, so a colour that falls on a rounding boundary can
/// round the other way: ClearlooksRe's utility buttons at scale 2 have 20 such pixels.
/// Drawing each strip at the whole frame's device position would take a surface the
/// size of the frame.
const STRIP_CHANNEL: u8 = 1;

/// An image's ARGB32 pixels as 0xAARRGGBB words.
fn pixels(surface: &ImageSurface) -> Vec<u32> {
    let (width, height) = (surface.width() as usize, surface.height() as usize);
    let stride = surface.stride() as usize;
    let mut words = Vec::with_capacity(width * height);
    // Cairo has no data for an empty image.
    if words.capacity() == 0 {
        return words;
    }
    surface
        .with_data(|data| {
            for y in 0..height {
                let row = &data[y * stride..][..width * 4];
                words.extend(
                    row.as_chunks::<4>()
                        .0
                        .iter()
                        .map(|&p| u32::from_ne_bytes(p)),
                );
            }
        })
        .unwrap();
    words
}

/// The surface as it comes back from a PNG, which is how Marco's pixels come: cairo
/// unpremultiplies to write one and premultiplies to read it.
fn through_png(surface: &ImageSurface) -> ImageSurface {
    let mut png = Vec::new();
    surface.write_to_png(&mut png).unwrap();
    ImageSurface::create_from_png(&mut png.as_slice()).unwrap()
}

fn channels(pixel: u32) -> [u8; 4] {
    pixel.to_be_bytes()
}

fn channel_difference(a: u32, b: u32) -> u8 {
    channels(a)
        .into_iter()
        .zip(channels(b))
        .map(|(a, b)| a.abs_diff(b))
        .max()
        .unwrap()
}

/// A picture of where two images differ: the golden faded to grey, with the pixels
/// that differ by more than the tolerance in red.
fn diff_image(
    width: i32,
    height: i32,
    marco: &[u32],
    camino: &[u32],
    tolerance: Tolerance,
) -> ImageSurface {
    let mut diff = ImageSurface::create(cairo::Format::ARgb32, width, height).unwrap();
    let stride = diff.stride() as usize;
    {
        let mut data = diff.data().unwrap();
        for (i, (&m, &c)) in marco.iter().zip(camino).enumerate() {
            let [a, r, g, b] = channels(m);
            let grey = ((u32::from(r) + u32::from(g) + u32::from(b)) / 3) as u8;
            // Transparent pixels are black.
            let level = if a > 0 { grey / 4 + 64 } else { 0 };
            let faded = u32::from_be_bytes([255, level, level, level]);
            let pixel = if channel_difference(m, c) > tolerance.channel {
                0xffff0000
            } else {
                faded
            };
            let (x, y) = (i % width as usize, i / width as usize);
            data[y * stride + x * 4..][..4].copy_from_slice(&pixel.to_ne_bytes());
        }
    }
    diff
}

/// Compares Camino's image with Marco's, writing Camino's and a diff beside the golden
/// if they differ by more than the tolerance.
fn compare_pixels(
    golden: &Path,
    camino: &ImageSurface,
    tolerance: Tolerance,
) -> Result<(), String> {
    let mut file = fs::File::open(golden).map_err(|e| e.to_string())?;
    let marco = ImageSurface::create_from_png(&mut file).map_err(|e| e.to_string())?;
    let ours = through_png(camino);
    let stem = golden.with_extension("");
    let failure_image = |suffix: &str| stem.with_extension(format!("{suffix}.png"));
    let write = |suffix: &str, surface: &ImageSurface| {
        let path = failure_image(suffix);
        let mut file = fs::File::create(&path).unwrap();
        surface.write_to_png(&mut file).unwrap();
    };
    if (marco.width(), marco.height()) != (ours.width(), ours.height()) {
        write("camino", camino);
        return Err(format!(
            "Marco's is {}x{}, Camino's {}x{}",
            marco.width(),
            marco.height(),
            ours.width(),
            ours.height()
        ));
    }
    let (marco_pixels, our_pixels) = (pixels(&marco), pixels(&ours));
    let differences: Vec<u8> = marco_pixels
        .iter()
        .zip(&our_pixels)
        .map(|(&m, &c)| channel_difference(m, c))
        .filter(|&d| d > tolerance.channel)
        .collect();
    if differences.len() <= tolerance.pixels {
        // Those of an earlier failure would be misleading now.
        for suffix in ["camino", "diff"] {
            let _ = fs::remove_file(failure_image(suffix));
        }
        return Ok(());
    }
    write("camino", camino);
    write(
        "diff",
        &diff_image(
            marco.width(),
            marco.height(),
            &marco_pixels,
            &our_pixels,
            tolerance,
        ),
    );
    Err(format!(
        "{} pixels differ, by up to {}",
        differences.len(),
        differences.iter().max().unwrap()
    ))
}

/// Whether the four strips, joined, are the whole frame with its corners cut out.
fn compare_strips(
    theme: &Theme,
    frame: &Frame,
    geometry: &FrameGeometry,
    scale: i32,
    whole: &ImageSurface,
) -> Result<(), String> {
    let whole_width = whole.width() as usize;
    let mut expected = pixels(whole);
    for cutout in geometry.corner_cutouts(scale) {
        let row = cutout.y as usize * whole_width;
        expected[row + cutout.x as usize..][..cutout.width as usize].fill(0);
    }
    for strip in Strip::ALL {
        let rect = geometry.strip_rect(strip);
        let surface = theme.render_strip(frame, strip, scale).unwrap();
        let (width, height) = (surface.width(), surface.height());
        if (width, height) != (rect.width * scale, rect.height * scale) {
            return Err(format!(
                "{strip:?} is {width}x{height}, not {rect:?} at {scale}x"
            ));
        }
        let got = pixels(&surface);
        let differing = (0..height as usize)
            .flat_map(|y| (0..width as usize).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let whole_x = (rect.x * scale) as usize + x;
                let whole_y = (rect.y * scale) as usize + y;
                let (got, expected) = (
                    got[y * width as usize + x],
                    expected[whole_y * whole_width + whole_x],
                );
                channel_difference(got, expected) > STRIP_CHANNEL
            })
            .count();
        if differing > 0 {
            return Err(format!("{differing} pixels of the {strip:?} strip differ"));
        }
    }
    Ok(())
}

// The geometry as `marco-frame-dump`'s `geometry_to_json` writes it.

fn rect_json(rect: Rect) -> Value {
    json!({ "x": rect.x, "y": rect.y, "width": rect.width, "height": rect.height })
}

fn border_json(border: Border) -> Value {
    json!({
        "left": border.left,
        "right": border.right,
        "top": border.top,
        "bottom": border.bottom,
    })
}

fn geometry_json(
    theme: &str,
    case: &Case,
    scale: u64,
    text_height: i32,
    geometry: &FrameGeometry,
) -> Value {
    let mut buttons = Map::new();
    for function in ButtonFunction::ALL {
        if let Some(space) = geometry.button(function) {
            buttons.insert(
                function.button_type().name().to_owned(),
                json!({
                    "visible": rect_json(space.visible),
                    "clickable": rect_json(space.clickable),
                }),
            );
        }
    }

    let backgrounds = &geometry.button_backgrounds;
    let mut background_json = Map::new();
    let mut single = |name: &str, rect: Option<Rect>| {
        if let Some(rect) = rect {
            background_json.insert(name.to_owned(), rect_json(rect));
        }
    };
    single("left_left", backgrounds.left_left);
    single("left_right", backgrounds.left_right);
    single("left_single", backgrounds.left_single);
    single("right_left", backgrounds.right_left);
    single("right_right", backgrounds.right_right);
    single("right_single", backgrounds.right_single);
    for (name, middle) in [
        ("left_middle", &backgrounds.left_middle),
        ("right_middle", &backgrounds.right_middle),
    ] {
        let used = middle
            .iter()
            .rposition(Option::is_some)
            .map_or(0, |i| i + 1);
        if used > 0 {
            let rects = middle[..used]
                .iter()
                .map(|rect| rect_json(rect.unwrap_or_default()))
                .collect();
            background_json.insert(name.to_owned(), Value::Array(rects));
        }
    }

    let borders = &geometry.borders;
    let radii = &geometry.corner_radii;
    json!({
        "theme": theme,
        "case": case.name,
        "scale": scale,
        "type": case.frame_type.name(),
        "flags": case.flag_names,
        "text_height": text_height,
        "width": geometry.width,
        "height": geometry.height,
        "borders": {
            "visible": border_json(borders.visible),
            "invisible": border_json(borders.invisible),
            "total": border_json(borders.total),
        },
        "title_rect": rect_json(geometry.title_rect),
        "titlebar_edges": {
            "left": geometry.left_titlebar_edge,
            "right": geometry.right_titlebar_edge,
            "top": geometry.top_titlebar_edge,
            "bottom": geometry.bottom_titlebar_edge,
        },
        "buttons": buttons,
        "button_backgrounds": background_json,
        "corner_radii": {
            "top_left": radii.top_left,
            "top_right": radii.top_right,
            "bottom_left": radii.bottom_left,
            "bottom_right": radii.bottom_right,
        },
    })
}

/// Where two JSON values differ, as `path: expected ≠ actual` lines.
fn differences(path: &str, expected: &Value, actual: &Value, out: &mut Vec<String>) {
    match (expected, actual) {
        (Value::Object(e), Value::Object(a)) => {
            let mut keys: Vec<&String> = e.keys().chain(a.keys()).collect();
            keys.sort();
            keys.dedup();
            for key in keys {
                let null = Value::Null;
                differences(
                    &format!("{path}.{key}"),
                    e.get(key).unwrap_or(&null),
                    a.get(key).unwrap_or(&null),
                    out,
                );
            }
        }
        (Value::Array(e), Value::Array(a)) if e.len() == a.len() => {
            for (i, (e, a)) in e.iter().zip(a).enumerate() {
                differences(&format!("{path}[{i}]"), e, a, out);
            }
        }
        _ if expected != actual => out.push(format!("{path}: Marco {expected}, Camino {actual}")),
        _ => {}
    }
}

fn main() -> ExitCode {
    // SAFETY: nothing else runs yet, and fontconfig hasn't been initialised.
    unsafe { env::set_var("FONTCONFIG_FILE", repo_tests().join("fonts/fonts.conf")) };

    let goldens = repo_tests().join("goldens");
    let cases = read_json(&goldens.join("cases.json"));
    let dirs = ThemeDirs::new(vec![repo_tests()]);
    let context = context(&cases);
    let font = pango::FontDescription::from_string(cases["font"].as_str().unwrap());
    let title = cases["title"].as_str().unwrap();
    let client_width = cases["client_width"].as_i64().unwrap() as i32;
    let client_height = cases["client_height"].as_i64().unwrap() as i32;
    let colors = palette(&goldens);
    let mini_icon = icon(&goldens, &cases, "mini_icon");
    let icon = icon(&goldens, &cases, "icon");
    // Only the test's own filter, as `cargo test --test goldens -- <filter>` passes it.
    let filter = env::args().skip(1).find(|arg| !arg.starts_with('-'));

    let mut compared = 0;
    let mut geometry_failures = Vec::new();
    let mut pixel_failures = Vec::new();
    let mut strip_failures = Vec::new();
    for theme_name in strings(&cases["themes"]) {
        let theme = Theme::load(theme_name, &dirs).unwrap_or_else(|e| panic!("{e}"));
        for scale in cases["scales"].as_array().unwrap() {
            let scale = scale.as_u64().unwrap();
            for case in cases["cases"].as_array().unwrap() {
                let case = resolve(&cases["base"], case);
                let stem = format!("{theme_name}/{}@{scale}x", case.name);
                if filter
                    .as_ref()
                    .is_some_and(|filter| !stem.contains(filter.as_str()))
                {
                    continue;
                }
                let title = Title::new(
                    &context,
                    &font,
                    theme.title_scale(case.frame_type, &case.flags),
                    title,
                );
                let geometry = theme.frame_geometry(
                    case.frame_type,
                    title.text_height,
                    &case.flags,
                    client_width,
                    client_height,
                    &case.layout,
                );
                let expected = read_json(&goldens.join(format!("frames/{stem}.json")));
                let actual = geometry_json(theme_name, &case, scale, title.text_height, &geometry);
                let mut differs = Vec::new();
                differences("", &expected, &actual, &mut differs);
                if !differs.is_empty() {
                    geometry_failures.push(format!("{stem}.json:\n  {}", differs.join("\n  ")));
                }

                let frame = Frame {
                    frame_type: case.frame_type,
                    flags: case.flags,
                    client_width,
                    client_height,
                    title: &title,
                    button_layout: &case.layout,
                    button_states: case.button_states,
                    mini_icon: Some(&mini_icon),
                    icon: Some(&icon),
                    colors: &colors,
                };
                let scale = scale as i32;
                let whole = theme.render_frame(&frame, scale).unwrap();
                let golden = goldens.join(format!("frames/{stem}.png"));
                if let Err(e) = compare_pixels(&golden, &whole, tolerance(theme_name, scale)) {
                    pixel_failures.push(format!("{stem}.png: {e}"));
                }
                if let Err(e) = compare_strips(&theme, &frame, &geometry, scale, &whole) {
                    strip_failures.push(format!("{stem}: {e}"));
                }
                compared += 1;
            }
        }
    }

    println!(
        "goldens: {compared} cases; {} differ from Marco's geometry, {} from its pixels, \
         and {} have strips that differ from the whole frame",
        geometry_failures.len(),
        pixel_failures.len(),
        strip_failures.len(),
    );
    let failures: Vec<&String> = geometry_failures
        .iter()
        .chain(&pixel_failures)
        .chain(&strip_failures)
        .collect();
    if failures.is_empty() {
        ExitCode::SUCCESS
    } else {
        for failure in failures {
            eprintln!("{failure}");
        }
        ExitCode::FAILURE
    }
}
