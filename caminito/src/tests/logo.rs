//! The logo (109): `data/icons/caminito.svg`, installed in the hicolor theme as the session
//! entry's icon. Like the keybinding manifests, it's data and needs no fixture.

use std::fs;

fn data_file(path: &str) -> String {
    let path = format!("{}/../data/{path}", env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("{path}: {err}"))
}

/// The value of `name="…"` in an element's text.
fn attribute<'a>(element: &'a str, name: &str) -> Option<&'a str> {
    let opening = format!(" {name}=\"");
    let start = element.find(&opening)? + opening.len();
    let len = element[start..].find('"')?;
    Some(&element[start..start + len])
}

#[test]
fn the_logo_is_square_for_an_icon_theme() {
    let logo = data_file("icons/caminito.svg");
    let svg = &logo[logo.find("<svg ").unwrap()..];
    let svg = &svg[..svg.find('>').unwrap()];
    let view_box: Vec<f64> = attribute(svg, "viewBox")
        .unwrap()
        .split_whitespace()
        .map(|n| n.parse().unwrap())
        .collect();
    assert_eq!(view_box[2], view_box[3], "{svg}");
    assert_eq!(attribute(svg, "width"), attribute(svg, "height"), "{svg}");
}

#[test]
fn the_logo_carries_no_content_credentials() {
    // A C2PA manifest hashes the drawing it was made for, which the square canvas changed.
    assert!(
        !data_file("icons/caminito.svg")
            .to_lowercase()
            .contains("c2pa")
    );
}

#[test]
fn the_session_entry_has_the_logo_as_its_icon() {
    let entry = data_file("mate-caminito.desktop");
    assert!(entry.lines().any(|line| line == "Icon=caminito"), "{entry}");
}
