//! The committed Marco goldens (14, ADR 0009) are complete: every case of every golden
//! theme at every scale has its image and geometry, and the palette and metadata are
//! there. The geometry and pixel comparisons read these files; this only checks the set.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use camino_theme::GtkState;
use serde_json::Value;

const GOLDEN_THEMES: [&str; 10] = [
    "Menta",
    "ClearlooksRe",
    "Atlanta",
    "eOS",
    "HighContrast",
    "BlackMATE",
    "TraditionalOk",
    "Arc",
    "Mint-Y",
    "Yaru",
];

fn goldens() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/goldens")
}

fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn cases() -> Value {
    read_json(&goldens().join("cases.json"))
}

fn strings(value: &Value) -> Vec<&str> {
    value
        .as_array()
        .expect("an array")
        .iter()
        .map(|v| v.as_str().expect("a string"))
        .collect()
}

/// A PNG's size, from its IHDR chunk.
fn png_size(path: &Path) -> (u64, u64) {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(
        &bytes[..8],
        b"\x89PNG\r\n\x1a\n",
        "{} is not a PNG",
        path.display()
    );
    let be = |at: usize| u64::from(u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()));
    (be(16), be(20))
}

fn is_colour(value: &Value) -> bool {
    value.as_array().is_some_and(|c| {
        c.len() == 4
            && c.iter()
                .all(|x| x.as_f64().is_some_and(|x| (0.0..=1.0).contains(&x)))
    })
}

#[test]
fn the_case_list_covers_the_golden_themes_with_unique_names() {
    let cases = cases();
    assert_eq!(strings(&cases["themes"]), GOLDEN_THEMES);
    assert_eq!(cases["scales"], serde_json::json!([1, 2]));

    let names: Vec<&str> = cases["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| case["name"].as_str().unwrap())
        .collect();
    let unique: BTreeSet<&str> = names.iter().copied().collect();
    assert_eq!(unique.len(), names.len(), "case names repeat: {names:?}");
}

#[test]
fn every_case_has_an_image_and_geometry_of_matching_size() {
    let cases = cases();
    let frames = goldens().join("frames");
    let mut missing = Vec::new();
    let mut expected = BTreeSet::new();

    for theme in strings(&cases["themes"]) {
        for case in cases["cases"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            for scale in cases["scales"].as_array().unwrap() {
                let scale = scale.as_u64().unwrap();
                let stem = frames.join(theme).join(format!("{name}@{scale}x"));
                let (png, json) = (stem.with_extension("png"), stem.with_extension("json"));
                expected.insert(png.clone());
                expected.insert(json.clone());
                if !png.exists() || !json.exists() {
                    missing.push(stem.display().to_string());
                    continue;
                }

                let geometry = read_json(&json);
                assert_eq!(geometry["theme"], theme, "{}", json.display());
                assert_eq!(geometry["case"], name, "{}", json.display());
                assert_eq!(geometry["scale"], scale, "{}", json.display());
                let width = geometry["width"].as_u64().unwrap();
                let height = geometry["height"].as_u64().unwrap();
                assert_eq!(
                    png_size(&png),
                    (width * scale, height * scale),
                    "{} doesn't match its geometry",
                    png.display()
                );
            }
        }
    }
    assert!(
        missing.is_empty(),
        "missing goldens:\n{}",
        missing.join("\n")
    );

    // A case taken out of the list takes its files with it.
    let mut stray = Vec::new();
    for entry in fs::read_dir(&frames).unwrap() {
        let path = entry.unwrap().path();
        let files = if path.is_dir() {
            fs::read_dir(&path)
                .unwrap()
                .map(|file| file.unwrap().path())
                .collect()
        } else {
            vec![path]
        };
        stray.extend(
            files
                .into_iter()
                .filter(|file| !expected.contains(file))
                .map(|file| file.display().to_string()),
        );
    }
    assert!(stray.is_empty(), "files for no case:\n{}", stray.join("\n"));
}

#[test]
fn the_palette_has_every_state_of_both_variants() {
    let palette = read_json(&goldens().join("palette.json"));
    assert_eq!(palette["gtk_theme"], cases()["gtk_theme"]);
    for variant in ["default", "dark"] {
        let colours = &palette["variants"][variant];
        for state in GtkState::ALL.map(|state| state.name().to_ascii_lowercase()) {
            for part in ["background", "text"] {
                let colour = &colours["states"][&state][part];
                assert!(is_colour(colour), "{variant} {state} {part}: {colour}");
            }
        }
        let named = colours["named"].as_object().expect("a named table");
        assert!(!named.is_empty(), "{variant} has no named colours");
        for (name, colour) in named {
            assert!(
                colour.is_null() || is_colour(colour),
                "{variant} {name}: {colour}"
            );
        }
    }
}

#[test]
fn the_metadata_records_what_rendered_the_goldens() {
    let metadata = read_json(&goldens().join("metadata.json"));
    for key in [
        "marco_version",
        "marco_commit",
        "mate_themes_version",
        "gtk_theme",
    ] {
        assert!(
            metadata[key].as_str().is_some_and(|v| !v.is_empty()),
            "metadata.json has no {key}"
        );
    }
    assert_eq!(metadata["gtk_theme"], cases()["gtk_theme"]);
}
