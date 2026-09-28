//! The ten golden themes (14, ADR 0009) load, from the copies vendored under
//! `tests/themes/`.

use std::path::Path;

use camino_theme::{Theme, ThemeDirs};

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
