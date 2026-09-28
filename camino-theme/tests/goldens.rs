//! Camino's theme engine against Marco's goldens (14, ADR 0009): every case in
//! `tests/goldens/cases.json`, for every golden theme at every scale, computed with the
//! pinned font and compared with what `marco-frame-dump` recorded.
//!
//! Geometry has to match exactly (26).
//!
//! This test has its own `main`, so it can point fontconfig at the vendored font before
//! any thread starts.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use camino_theme::{
    Border, ButtonFunction, ButtonLayout, FrameFlags, FrameGeometry, FrameType, Rect, Theme,
    ThemeDirs, Title, cairo, pango, title_context,
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
    Case {
        name: case["name"].as_str().unwrap(),
        frame_type: FrameType::from_name(type_name)
            .unwrap_or_else(|| panic!("cases: unknown frame type {type_name:?}")),
        flags,
        flag_names,
        layout: ButtonLayout::parse(layout),
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

    let mut compared = 0;
    let mut failures = Vec::new();
    for theme_name in strings(&cases["themes"]) {
        let theme = Theme::load(theme_name, &dirs).unwrap_or_else(|e| panic!("{e}"));
        for scale in cases["scales"].as_array().unwrap() {
            let scale = scale.as_u64().unwrap();
            for case in cases["cases"].as_array().unwrap() {
                let case = resolve(&cases["base"], case);
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
                let stem = format!("{theme_name}/{}@{scale}x", case.name);
                let expected = read_json(&goldens.join(format!("frames/{stem}.json")));
                let actual = geometry_json(theme_name, &case, scale, title.text_height, &geometry);
                let mut differs = Vec::new();
                differences("", &expected, &actual, &mut differs);
                if !differs.is_empty() {
                    failures.push(format!("{stem}.json:\n  {}", differs.join("\n  ")));
                }
                compared += 1;
            }
        }
    }

    println!(
        "golden geometry: {compared} cases, {} differ from Marco",
        failures.len()
    );
    if failures.is_empty() {
        ExitCode::SUCCESS
    } else {
        for failure in &failures {
            eprintln!("{failure}");
        }
        ExitCode::FAILURE
    }
}
