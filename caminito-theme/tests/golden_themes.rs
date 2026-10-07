//! The ten golden themes (14, ADR 0009) load, from the copies vendored under
//! `tests/themes/`.

use std::path::Path;

use caminito_theme::{Theme, ThemeDirs};

/// Each golden theme, and the format version Marco loads it as.
const GOLDEN_THEMES: [(&str, u32); 10] = [
    ("Menta", 3000),
    ("ClearlooksRe", 1000),
    ("Atlanta", 1000),
    ("eOS", 3000),
    ("HighContrast", 1000),
    ("BlackMATE", 3000),
    ("TraditionalOk", 1000),
    ("Arc", 3000),
    ("Mint-Y", 3000),
    // Yaru installs its v1 file as metacity-theme-3.xml too.
    ("Yaru", 3000),
];

fn vendored() -> ThemeDirs {
    ThemeDirs::new(vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests")])
}

#[test]
fn every_golden_theme_loads() {
    let dirs = vendored();
    let failures: Vec<String> = GOLDEN_THEMES
        .iter()
        .filter_map(|&(name, format_version)| match Theme::load(name, &dirs) {
            Ok(theme) if theme.format_version == format_version => None,
            Ok(theme) => Some(format!(
                "{name} loaded as format {}, expected {format_version}",
                theme.format_version
            )),
            Err(error) => Some(format!("{name}: {error}")),
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The names the golden themes' `gtk:custom` colours use are the ones Caminito asks the
/// palette helper for. `tools/regenerate-goldens` recorded them by searching the theme
/// files, so it also has the two that only constants no draw op uses name: Menta's
/// `C_title_focused_hilight_dark` and Mint-Y's `C_wm_bg_unfocused`.
#[test]
fn the_golden_themes_use_the_custom_colours_the_palette_recorded() {
    let dirs = vendored();
    let mut names = std::collections::BTreeSet::new();
    for (name, _) in GOLDEN_THEMES {
        let theme = Theme::load(name, &dirs).unwrap();
        names.extend(theme.custom_color_names());
    }

    let palette = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/goldens/palette.json");
    let palette: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(palette).unwrap()).unwrap();
    let recorded: std::collections::BTreeSet<String> = palette["variants"]["default"]["named"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    let unused: std::collections::BTreeSet<String> = ["wm_bg_unfocused", "wm_title_highlight_dark"]
        .map(String::from)
        .into();
    assert_eq!(names, &recorded - &unused);
}
